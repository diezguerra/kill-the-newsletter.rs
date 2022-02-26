mod models;

use axum::{
    extract::Extension,
    response::IntoResponse,
    routing::{get, post},
    AddExtensionLayer, Json, Router,
};

use r2d2::ManageConnection;
use r2d2_sqlite::SqliteConnectionManager;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;

use crate::models::feed::NewFeed;

async fn create_feed(
    item: Json<NewFeed>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> impl IntoResponse {
    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    println!("{:?}", item);
    item.save(&mut conn)
}

async fn get_root() -> impl IntoResponse {
    Json("asdf")
}

fn populate_if_needed(mngr: &SqliteConnectionManager) {
    mngr.connect()
        .unwrap()
        .execute_batch(&std::fs::read_to_string("migration.sql").unwrap())
        .expect("Couldn't run initial migration");
}

#[tokio::main]
async fn main() {
    let sqlite_file = "my.db";
    let sqlite_connection_manager = SqliteConnectionManager::file(sqlite_file);
    //let sqlite_connection_manager = SqliteConnectionManager::memory();

    populate_if_needed(&sqlite_connection_manager);

    let sqlite_pool = r2d2::Pool::new(sqlite_connection_manager)
        .expect("Failed to create r2d2 SQLite connection pool");
    let pool_arc = Arc::new(sqlite_pool);

    let app = Router::new()
        .route("/", post(create_feed))
        .route("/", get(get_root))
        .layer(AddExtensionLayer::new(pool_arc));

    let addr: SocketAddrV4 = "127.0.0.1:7878".parse().unwrap();

    let addr = SocketAddr::from(addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
