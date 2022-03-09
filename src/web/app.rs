use axum::{
    routing::{get, post},
    AddExtensionLayer, Router,
};
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

use crate::web::handlers;

pub fn build_app(
    pool_arc: Arc<r2d2::Pool<SqliteConnectionManager>>,
) -> axum::routing::IntoMakeService<Router> {
    Router::new()
        .route("/", get(handlers::get_index))
        .route("/", post(handlers::create_feed))
        .route("/:reference", get(handlers::get_feed_created))
        .route("/feeds/:reference", get(handlers::get_feed))
        .layer(AddExtensionLayer::new(pool_arc))
        .into_make_service()
}
