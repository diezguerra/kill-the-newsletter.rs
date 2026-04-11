//! Main entrypoint for the web application

use std::time::Duration;

use axum::{
    extract::Request,
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::{
    compression::CompressionLayer,
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
        .layer(CompressionLayer::new())
}
