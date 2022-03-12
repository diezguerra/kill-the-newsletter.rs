//! Main entrypoint for the web application

use axum::{
    routing::{get, post},
    AddExtensionLayer, Router,
};
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::web::{handlers, serve_static};

pub fn build_app(
    pool_arc: Arc<r2d2::Pool<SqliteConnectionManager>>,
) -> axum::routing::IntoMakeService<Router> {
    Router::new()
        .route("/", get(handlers::get_index))
        .route("/", post(handlers::create_feed))
        .route("/:reference", get(handlers::get_feed_created))
        .route("/feeds/:reference", get(handlers::get_feed))
        .nest("/static", get(serve_static::handler))
        .layer(AddExtensionLayer::new(pool_arc))
        .layer(TraceLayer::new_for_http())
        .into_make_service()
}
