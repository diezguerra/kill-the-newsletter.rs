mod models;

use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    routing::{get, post},
    AddExtensionLayer, Json, Router
};
use dotenv_codegen::dotenv;
use r2d2::ManageConnection;
use r2d2_sqlite::SqliteConnectionManager;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;

use crate::models::feed::{NewFeed, Feed, get_title_by_reference};
use crate::models::entry::{Entry, find_reference};

const WEB_URL: &str = dotenv!("WEB_URL");
const EMAIL_DOMAIN: &str = dotenv!("EMAIL_DOMAIN");

async fn create_feed(
    item: Json<NewFeed>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> impl IntoResponse {
    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    println!("{:?}", item);
    item.save(&mut conn)
}

#[derive(Template)]
#[template(path = "atom.xml", ext = "xml")]
struct AtomTemplate {
    web_url: Box<String>,
    email_domain: Box<String>,
    feed_title: Box<String>,
    feed_reference: Box<String>,
    entries: Vec<Entry>,
}

async fn get_reference(
    Path(reference): Path<String>,
    Extension(pool_arc): Extension<Arc<r2d2::Pool<SqliteConnectionManager>>>,
) -> AtomTemplate {
    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    let entries = match find_reference(&reference, &mut conn) {
        Ok(entries) => entries,
        Err(_) => panic!("oh no")//return (StatusCode::NOT_FOUND, "Not found")
    };
    println!("Entries {:#?}", entries);
    if entries.len() == 0 {
        //return (StatusCode::NOT_FOUND, &"Not found");
    }

    let title = match get_title_by_reference(&reference, &mut conn) {
        Ok(title) => title,
        _ => String::from("No feed title found")
    };

    println!("title! {:#?}", title);

    AtomTemplate {
        web_url: Box::new(String::from(WEB_URL)),
        email_domain: Box::new(String::from(EMAIL_DOMAIN)),
        feed_title: Box::new(title),
        feed_reference: Box::new(reference),
        entries: entries
    }
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
        .route("/:reference", get(get_reference))
        .layer(AddExtensionLayer::new(pool_arc));

    let addr: SocketAddrV4 = "127.0.0.1:7878".parse().unwrap();

    let addr = SocketAddr::from(addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
