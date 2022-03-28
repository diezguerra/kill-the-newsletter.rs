mod database;
mod models;
mod smtp;
mod time;
mod vars;
mod web;

use ktn::tracing::setup_tracing;
use std::error::Error;
use std::net::{SocketAddr, SocketAddrV4};
use tokio::net::TcpListener;
use tokio::signal;
use tracing::error;

use crate::database::get_db_pool;
use crate::smtp::app::serve_smtp;
use crate::web::build_app;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_tracing();

    let pool = get_db_pool().await?;

    let http_addr: SocketAddrV4 = "0.0.0.0:8080".parse().unwrap();
    let http_addr = SocketAddr::from(http_addr);

    let http_listener = axum::Server::bind(&http_addr);
    let http_app = build_app(pool.clone());
    let smtp_listener = TcpListener::bind("0.0.0.0:2525").await.unwrap();

    // Serve HTTP and SMTP, and end the program whenever either of those
    // futures returns (fails) or if a system interrupt is received.
    tokio::select! {
        _ = http_listener.serve(http_app) => {
            error!("HTTP service exited prematurely");
        }
        _ = serve_smtp(&smtp_listener, pool.clone()) => {
            error!("SMTP service exited prematurely");
        }
        _ = signal::ctrl_c() => {
            error!("SIGINT Received, shutting down...");
        }
    }

    Ok(())
}
