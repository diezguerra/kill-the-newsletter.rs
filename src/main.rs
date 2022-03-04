mod models;
mod time;
mod vars;

use askama::Template;
use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    AddExtensionLayer, Json, Router,
};
use r2d2::ManageConnection;
use r2d2_sqlite::SqliteConnectionManager;
use std::error::Error;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::signal;

use crate::models::entry::{find_reference, Entry};
use crate::models::feed::{get_title_by_reference, NewFeed};
use crate::time::filters;
use crate::vars::{EMAIL_DOMAIN, WEB_URL};

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
) -> (StatusCode, impl IntoResponse) {
    let pool = pool_arc.clone();
    let mut conn = pool.get().expect("Couldn't get database connection");
    let entries = match find_reference(&reference, &mut conn) {
        Ok(entries) => entries,
        Err(_) => return (StatusCode::NOT_FOUND, String::from("Not found")),
    };

    if entries.len() == 0 {
        return (StatusCode::NOT_FOUND, String::from("Not found"));
    }

    let title = match get_title_by_reference(&reference, &mut conn) {
        Ok(title) => title,
        _ => String::from("No feed title found"),
    };

    (
        StatusCode::OK,
        AtomTemplate {
            web_url: Box::new(String::from(WEB_URL)),
            email_domain: Box::new(String::from(EMAIL_DOMAIN)),
            feed_title: Box::new(title),
            feed_reference: Box::new(reference),
            entries: entries,
        }
        .render()
        .expect("Failed to render Atom"),
    )
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

    let smtp_listener = TcpListener::bind("127.0.0.1:2525").await.unwrap();

    let http_addr: SocketAddrV4 = "127.0.0.1:7878".parse().unwrap();
    let http_addr = SocketAddr::from(http_addr);
    let http_listener = axum::Server::bind(&http_addr);

    tokio::select! {
        _ = http_listener.serve(app.into_make_service()) => {
            eprintln!("HTTP service exited prematurely");
        }
        _ = serve_smtp(&smtp_listener) => {
            eprintln!("SMTP service exited prematurely");
        }
        _ = signal::ctrl_c() => {
            eprintln!("OMG CTRL-C!");
        }
    }
}

async fn serve_smtp(listener: &TcpListener) -> Result<(), Box<dyn Error>> {
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            match serve_smtp_request(&mut socket).await {
                Ok(_) => println!("SMTP response succeeded!"),
                Err(e) => println!("SMTP response failed: {:#?}", e),
            }
        });
    }
}

async fn serve_smtp_request(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn Error>> {
    loop {
        stream.readable().await?;
        let mut buf = [0; 4096];
        stream.try_read(&mut buf); // if ?'ed, gets Kind( WouldBlock,)
        if std::str::from_utf8(&buf)?.starts_with("QUIT") {
            stream.write_all(b"221 BYE").await?;
            break;
        }
        stream.write_all(&buf).await?;
    }
    Ok(())
}
