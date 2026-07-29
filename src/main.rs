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
use tokio::signal::unix::{signal as unix_signal, SignalKind};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::database::get_db_pool;
use crate::smtp::app::serve_smtp;
use crate::web::build_app;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    setup_tracing();

    let pool = get_db_pool().await?;
    sqlx::migrate!("./migrations").run(&pool).await?;

    let http_port: u16 = std::env::var("HTTP_PORT")
        .unwrap_or_else(|_| "8081".to_owned())
        .parse()?;
    let smtp_port: u16 = std::env::var("SMTP_PORT")
        .unwrap_or_else(|_| "2525".to_owned())
        .parse()?;

    let http_addr: SocketAddr = SocketAddr::from(
        format!("0.0.0.0:{}", http_port).parse::<SocketAddrV4>()?,
    );
    let http_listener = TcpListener::bind(http_addr).await?;
    let http_app = build_app(pool.clone());
    let smtp_listener =
        TcpListener::bind(format!("0.0.0.0:{}", smtp_port)).await?;
    let mut sigterm = unix_signal(SignalKind::terminate())?;

    // Shared shutdown token: cancelled once on signal, observed by both the
    // HTTP and SMTP servers so neither drops in-flight work when the other
    // finishes shutting down first.
    let shutdown_token = CancellationToken::new();

    let signal_watcher = {
        let shutdown_token = shutdown_token.clone();
        async move {
            tokio::select! {
                _ = signal::ctrl_c() => {
                    info!("SIGINT received, shutting down...");
                }
                _ = sigterm.recv() => {
                    info!("SIGTERM received, shutting down...");
                }
            }
            shutdown_token.cancel();
        }
    };

    let http_future = {
        let shutdown_token = shutdown_token.clone();
        async move {
            let result = axum::serve(
                http_listener,
                http_app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(
                async move { shutdown_token.cancelled().await },
            )
            .await;

            if let Err(e) = result {
                error!("HTTP service exited prematurely: {}", e);
            }
        }
    };

    let smtp_future = async {
        if let Err(e) =
            serve_smtp(&smtp_listener, pool.clone(), shutdown_token.clone())
                .await
        {
            error!("SMTP service exited prematurely: {}", e);
        }
    };

    // Wait for both services to finish shutting down gracefully.
    tokio::join!(signal_watcher, http_future, smtp_future);

    Ok(())
}
