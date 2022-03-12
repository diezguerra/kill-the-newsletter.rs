//! Main entrypoint for the web application

use axum::{
    extract::Extension,
    response::Redirect,
    routing::{get, post},
    Router,
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
        .route(
            "/favicon.ico",
            get(|| async {
                Redirect::permanent("/static/favicon.ico".parse().unwrap())
            }),
        )
        .route("/:reference", get(handlers::get_feed_created))
        .route("/feeds/:reference", get(handlers::get_feed))
        .nest("/static", get(serve_static::handler))
        .layer(Extension(pool_arc))
        .layer(TraceLayer::new_for_http())
        .into_make_service()
}
