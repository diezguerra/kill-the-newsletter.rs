mod models;

use axum::{
    AddExtensionLayer,
    extract::Extension,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};

use r2d2_sqlite::SqliteConnectionManager;
use r2d2::ManageConnection;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;

use crate::models::feed::NewFeed;

async fn post_root(item: Json<NewFeed>, Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>) -> impl IntoResponse {
    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    println!("{:?}", item);
    let result = item.save(&mut conn);
    println!("Result: {}", result);
    "true"
}

async fn get_root() -> impl IntoResponse {
    Json("asdf")
}

#[tokio::main]
async fn main() {

    let sqlite_file = "my.db";
    let sqlite_connection_manager = SqliteConnectionManager::file(sqlite_file);
    //let sqlite_connection_manager = SqliteConnectionManager::memory();

    sqlite_connection_manager.connect().unwrap().execute_batch(&std::fs::read_to_string("migration.sql").expect("No migration!")).expect("Couldn't migrate");

    let sqlite_pool = r2d2::Pool::new(sqlite_connection_manager)
        .expect("Failed to create r2d2 SQLite connection pool");
    let pool_arc = Arc::new(sqlite_pool);

    let app = Router::new()
        .route("/", post(post_root))
        .route("/", get(get_root))
        .layer(AddExtensionLayer::new(pool_arc));

    let addr: SocketAddrV4 = "127.0.0.1:7878".parse().unwrap();

    let addr = SocketAddr::from(addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
