//! Main entrypoint for the web application

use std::time::Duration;

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::{
    compression::CompressionLayer,
    set_header::SetResponseHeaderLayer,
    trace::{
        DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer,
    },
    LatencyUnit,
};
use tracing::Level;

use crate::database::Pool;
use crate::web::{handlers, rate_limit::RateLimiter, serve_static};

pub fn build_app(pool: Pool) -> Router {
    // Allow 10 feed creations per IP per minute
    let feed_rate_limiter = RateLimiter::new(10, Duration::from_secs(60));

    let create_feed_route = Router::new()
        .route("/", post(handlers::create_feed))
        .layer(middleware::from_fn_with_state(
            feed_rate_limiter,
            crate::web::rate_limit::middleware,
        ));

    Router::new()
        .route("/health", get(handlers::health))
        .route("/", get(handlers::get_index))
        .merge(create_feed_route)
        .route("/feeds/{reference}", get(handlers::get_feed))
        .route("/{reference}", get(serve_static::handler))
        .nest(
            "/static",
            Router::new().route("/{*path}", get(serve_static::handler)),
        )
        .with_state(pool)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request| {
                    tracing::info_span!(
                        "http-request",
                        method = request.method().as_str(),
                        uri = request
                            .uri()
                            .path_and_query()
                            .map(|p| p.as_str())
                            .unwrap_or("/"),
                    )
                })
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                )
                .on_failure(
                    DefaultOnFailure::new()
                        .level(Level::ERROR)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self' https://stats.ktnrs.com; \
                 style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
                 connect-src 'self' https://stats.ktnrs.com; frame-ancestors 'none'",
            ),
        ))
        .layer(CompressionLayer::new())
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn health_response_has_security_headers() {
        // `connect_lazy` builds a Pool without establishing a real database
        // connection, which is enough to construct the Router: the `/health`
        // route never touches the pool.
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://user:pass@localhost/db")
            .expect("failed to build lazy pool");

        let app = build_app(pool);

        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let headers = response.headers();
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(
            headers.get("referrer-policy").unwrap(),
            "strict-origin-when-cross-origin"
        );
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            headers.get("content-security-policy").unwrap(),
            "default-src 'self'; script-src 'self' https://stats.ktnrs.com; \
             style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
             connect-src 'self' https://stats.ktnrs.com; frame-ancestors 'none'"
        );
    }
}
