mod database;
mod models;
mod smtp;
mod time;
mod vars;
mod web;

use crate::database::get_db_pool;
use crate::smtp::app::serve_smtp;
use crate::web::build_app;
use std::net::{SocketAddr, SocketAddrV4};
use tokio::net::TcpListener;
use tokio::signal;
use tracing::error;
use tracing_subscriber;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let pool = get_db_pool();

    let http_addr: SocketAddrV4 = "127.0.0.1:7878".parse().unwrap();
    let http_addr = SocketAddr::from(http_addr);

    let http_listener = axum::Server::bind(&http_addr);
    let http_app = build_app(pool);
    let smtp_listener = TcpListener::bind("127.0.0.1:2525").await.unwrap();

    tokio::select! {
        _ = http_listener.serve(http_app) => {
            error!("HTTP service exited prematurely");
        }
        _ = serve_smtp(&smtp_listener) => {
            error!("SMTP service exited prematurely");
        }
        _ = signal::ctrl_c() => {
            error!("OMG CTRL-C!");
        }
    }
}
