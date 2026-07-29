//! Per-IP rate limiter middleware for Axum.
//!
//! Tracks request timestamps per IP in a sliding window.
//! Returns 429 Too Many Requests when the limit is exceeded.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};

/// Header Fly.io's edge proxy sets to the real client IP. Fly overwrites any
/// client-supplied value for this header, so — unlike `X-Forwarded-For` —
/// it's safe to trust in this deployment.
const FLY_CLIENT_IP_HEADER: &str = "Fly-Client-IP";

/// How often (in number of `is_allowed` calls) to sweep the whole map for
/// entries whose timestamp vec has drained to empty. Keeps memory bounded
/// under sustained traffic from many distinct IPs without doing a full-map
/// scan on every single request.
const SWEEP_INTERVAL: u64 = 128;

/// Shared sliding-window rate limiter state.
#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
    window: Duration,
    max_requests: usize,
    calls: Arc<AtomicU64>,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            window,
            max_requests,
            calls: Arc::new(AtomicU64::new(0)),
        }
    }

    fn is_allowed(&self, ip: IpAddr) -> bool {
        // Recover from a poisoned mutex instead of panicking forever: a
        // panic elsewhere while the lock was held must not turn every
        // future request into a 500.
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let cutoff = now - self.window;

        let timestamps = state.entry(ip).or_default();
        timestamps.retain(|&t| t > cutoff);

        let allowed = if timestamps.len() >= self.max_requests {
            false
        } else {
            timestamps.push(now);
            true
        };

        // Periodically sweep the whole map: prune every IP's timestamps
        // against the cutoff (not just the current IP's, which is all the
        // per-request path above does) and drop entries that end up empty.
        // Without this, every distinct IP that has ever made a request
        // stays in the map forever, which is an unbounded memory leak under
        // sustained traffic from many distinct IPs.
        let call_count = self.calls.fetch_add(1, Ordering::Relaxed) + 1;
        if call_count.is_multiple_of(SWEEP_INTERVAL) {
            state.retain(|_, timestamps| {
                timestamps.retain(|&t| t > cutoff);
                !timestamps.is_empty()
            });
        }

        allowed
    }

    /// Number of distinct IPs currently tracked in the map. Test-only
    /// visibility into internal state to verify the sweep behavior.
    #[cfg(test)]
    fn tracked_ip_count(&self) -> usize {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

/// Determine the client IP to rate-limit on.
///
/// The app runs on Fly.io behind Fly's edge proxy, so `ConnectInfo`'s
/// `SocketAddr` is the proxy's TCP peer address, not the real client —
/// using it directly would put every user behind the proxy into a single
/// shared bucket. Fly reliably sets `Fly-Client-IP` on every request
/// (overwriting any client-supplied value), so it's trusted here. This
/// intentionally does NOT look at `X-Forwarded-For`, which is
/// client-spoofable. Falls back to the TCP peer address when the header is
/// absent or unparseable, which covers local dev and tests where there's no
/// Fly proxy in front.
fn client_ip(headers: &HeaderMap, peer: SocketAddr) -> IpAddr {
    headers
        .get(FLY_CLIENT_IP_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
        .unwrap_or_else(|| peer.ip())
}

/// Axum middleware — apply with `axum::middleware::from_fn_with_state`.
pub async fn middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = client_ip(request.headers(), addr);
    if limiter.is_allowed(ip) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    fn peer_addr() -> SocketAddr {
        "10.0.0.1:12345".parse().unwrap()
    }

    async fn send(app: &Router, fly_client_ip: Option<&str>) -> StatusCode {
        let mut req = HttpRequest::builder().uri("/");
        if let Some(ip) = fly_client_ip {
            req = req.header(FLY_CLIENT_IP_HEADER, ip);
        }
        let mut req = req.body(Body::empty()).unwrap();

        // `ConnectInfo<SocketAddr>` is normally injected by
        // `into_make_service_with_connect_info` at the TCP-accept layer. To
        // exercise the middleware directly with `oneshot` (no real
        // listener), insert the same extension it would have set.
        req.extensions_mut().insert(ConnectInfo(peer_addr()));

        app.clone().oneshot(req).await.unwrap().status()
    }

    /// Router carrying the rate-limit middleware, driven directly via
    /// `oneshot` with a manually-inserted `ConnectInfo` extension (see
    /// `send` above).
    fn router(limiter: RateLimiter) -> Router {
        Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn_with_state(limiter, middleware))
    }

    #[tokio::test]
    async fn fly_client_ip_header_is_used_over_connect_info() {
        // max_requests = 1: the second request from the SAME Fly-Client-IP
        // must be rejected, even though ConnectInfo (the TCP peer / Fly
        // proxy address) is identical for both requests.
        let limiter = RateLimiter::new(1, Duration::from_secs(60));
        let app = router(limiter);

        let status1 = send(&app, Some("203.0.113.5")).await;
        let status2 = send(&app, Some("203.0.113.5")).await;

        assert_eq!(status1, StatusCode::OK);
        assert_eq!(status2, StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn falls_back_to_connect_info_when_header_absent() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60));
        let app = router(limiter);

        // No Fly-Client-IP header at all: both requests share the same
        // ConnectInfo peer, so the second should be rejected.
        let status1 = send(&app, None).await;
        let status2 = send(&app, None).await;

        assert_eq!(status1, StatusCode::OK);
        assert_eq!(status2, StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn different_fly_client_ips_are_tracked_independently() {
        let limiter = RateLimiter::new(1, Duration::from_secs(60));
        let app = router(limiter);

        // Same ConnectInfo peer (both go through the same proxy), but
        // different Fly-Client-IP values: neither should affect the other.
        let status_a1 = send(&app, Some("203.0.113.10")).await;
        let status_b1 = send(&app, Some("203.0.113.20")).await;
        let status_a2 = send(&app, Some("203.0.113.10")).await;
        let status_b2 = send(&app, Some("203.0.113.20")).await;

        assert_eq!(status_a1, StatusCode::OK);
        assert_eq!(status_b1, StatusCode::OK);
        assert_eq!(status_a2, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(status_b2, StatusCode::TOO_MANY_REQUESTS);
    }

    #[test]
    fn invalid_fly_client_ip_header_falls_back_to_peer() {
        let mut headers = HeaderMap::new();
        headers.insert(FLY_CLIENT_IP_HEADER, "not-an-ip".parse().unwrap());
        let peer = peer_addr();

        assert_eq!(client_ip(&headers, peer), peer.ip());
    }

    #[test]
    fn valid_fly_client_ip_header_is_preferred() {
        let mut headers = HeaderMap::new();
        headers.insert(FLY_CLIENT_IP_HEADER, "198.51.100.7".parse().unwrap());
        let peer = peer_addr();

        assert_eq!(
            client_ip(&headers, peer),
            "198.51.100.7".parse::<IpAddr>().unwrap()
        );
    }

    #[test]
    fn sweep_removes_stale_empty_entries() {
        // Very short window so timestamps expire almost immediately.
        let limiter = RateLimiter::new(1000, Duration::from_millis(1));

        // Drive many distinct IPs through the limiter. Each call prunes
        // only its own IP's vec (which will be empty on every call after
        // the window elapses, since the window is 1ms), but the map entry
        // itself only gets removed by the periodic sweep.
        for i in 0..(SWEEP_INTERVAL as u32 * 2) {
            let ip = IpAddr::from([10, 0, (i >> 8) as u8, (i & 0xff) as u8]);
            std::thread::sleep(Duration::from_millis(2));
            assert!(limiter.is_allowed(ip));
        }

        // The map must not have grown unbounded to `SWEEP_INTERVAL * 2`
        // entries: the sweep(s) triggered along the way should have culled
        // the ones whose vecs had already drained empty by the time of the
        // sweep, aside from a handful of most-recent entries.
        let tracked = limiter.tracked_ip_count();
        assert!(
            tracked < (SWEEP_INTERVAL as usize * 2),
            "expected sweep to bound map growth, but {tracked} entries are tracked"
        );
    }

    #[test]
    fn poisoned_mutex_does_not_permanently_break_the_limiter() {
        let limiter = RateLimiter::new(2, Duration::from_secs(60));
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        // Poison the mutex by panicking while holding the lock.
        let poison_limiter = limiter.clone();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = poison_limiter.state.lock().unwrap();
                panic!("simulated panic while holding the rate limiter lock");
            }));
        assert!(result.is_err());

        // The limiter must keep working after the mutex is poisoned instead
        // of panicking forever on every subsequent call.
        assert!(limiter.is_allowed(ip));
        assert!(limiter.is_allowed(ip));
        assert!(!limiter.is_allowed(ip));
    }
}
