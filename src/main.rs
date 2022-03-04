mod database;
mod http;
mod models;
mod smtp;
mod time;
mod vars;

use crate::database::get_db_pool;
use crate::http::build_app;
use crate::smtp::serve_smtp;
use std::net::{SocketAddr, SocketAddrV4};
use tokio::net::TcpListener;
use tokio::signal;

#[tokio::main]
async fn main() {
    let pool = get_db_pool();

    let http_addr: SocketAddrV4 = "127.0.0.1:7878".parse().unwrap();
    let http_addr = SocketAddr::from(http_addr);

    let http_listener = axum::Server::bind(&http_addr);
    let http_app = build_app(pool);
    let smtp_listener = TcpListener::bind("127.0.0.1:2525").await.unwrap();

    tokio::select! {
        _ = http_listener.serve(http_app) => {
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
