//! # SMTP server main entry point
//!
//! Receives a listener and spawns a green thread for each open connection,
//! uses the [`State`] machine to tease out the newsletter email from the
//! client, then parses and stores the entry if it's valid.

use r2d2_sqlite::SqliteConnectionManager;
use std::error::Error;
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, span};

use crate::database::get_db_pool;
use crate::models::Entry;
use crate::smtp::state_machine::State;

pub struct Email {
    pub rcpt: String,
    pub body: String,
}

pub enum SMTPResult {
    HealthCheck,
    Success { email: Option<Email> },
}

pub async fn serve_smtp(listener: &TcpListener) -> Result<(), Box<dyn Error>> {
    let pool_arc = get_db_pool();

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let pool = pool_arc.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_smtp_request(&mut socket, pool).await {
                error!("SMTP Handler Error: {}", e);
            }
        });
    }
    #[allow(unreachable_code)] // As we wait for the ! type..
    Ok(())
}

async fn handle_smtp_request(
    stream: &mut TcpStream,
    pool: Arc<r2d2::Pool<SqliteConnectionManager>>,
) -> Result<SMTPResult, String> {
    let mut stream = BufReader::new(stream);

    let state = State::Connected;

    let envelope: Email = match state.run(&mut stream).await {
        Ok(SMTPResult::HealthCheck) => return Ok(SMTPResult::HealthCheck),
        Err(e) => return Err(e),
        Ok(SMTPResult::Success { email }) => email.unwrap(),
    };

    let span = span!(
        tracing::Level::INFO,
        "saving_entry",
        email_rcpt = envelope.rcpt.as_str()
    );
    let _guard = span.enter();

    let entry: Entry = match envelope.try_into() {
        Ok(ent) => ent,
        Err(e) => return Err(e),
    };

    // Store in DB
    if let Ok(mut conn) = pool.get() {
        match entry.save(&mut conn) {
            Ok(_) => {
                info!("Email stored as {}", entry);
                Ok(SMTPResult::Success { email: None })
            }
            Err(e) => Err(format!("Couldn't INSERT email {} ({})", entry, e)),
        }
    } else {
        Err(format!(
            "Couldn't get DB connection, dropping entry {}",
            entry
        ))
    }
}
