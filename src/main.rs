use axum::{
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::{SocketAddr, SocketAddrV4};

#[derive(Debug, Serialize, Deserialize)]
struct Hello {
    message: String,
}

async fn post_root(item: Json<Hello>) -> impl IntoResponse {
    item
}

async fn get_root() -> impl IntoResponse {
    Json("asdf")
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", post(post_root))
        .route("/", get(get_root));

    let addr: SocketAddrV4 = "127.0.0.1:7878".parse().unwrap();

    let addr = SocketAddr::from(addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
