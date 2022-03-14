//! Main entrypoint for the web application

use axum::{
    extract::Extension,
    routing::{get, post},
    Router,
};
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;
use tower_http::trace::{
    DefaultMakeSpan, DefaultOnFailure, DefaultOnRequest, DefaultOnResponse,
    TraceLayer,
};
use tower_http::LatencyUnit;
use tracing::Level;

use crate::web::{handlers, serve_static};

pub fn build_app(
    pool_arc: Arc<r2d2::Pool<SqliteConnectionManager>>,
) -> axum::routing::IntoMakeService<Router> {
    Router::new()
        .route("/", get(handlers::get_index))
        .route("/", post(handlers::create_feed))
        .route("/feeds/:reference", get(handlers::get_feed))
        .route("/:reference", get(serve_static::handler))
        .nest("/static", get(serve_static::handler))
        .layer(Extension(pool_arc))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(false))
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                )
                .on_failure(
                    DefaultOnFailure::new()
                        .level(Level::INFO)
                        .latency_unit(LatencyUnit::Micros),
                ),
        )
        .into_make_service()
}
