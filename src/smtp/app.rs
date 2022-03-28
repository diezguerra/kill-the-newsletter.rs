//! # SMTP server main entry point
//!
//! Receives a listener and spawns a green thread for each open connection,
//! uses the [`State`] machine to tease out the newsletter email from the
//! client, then parses and stores the entry if it's valid.

use std::error::Error;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tracing::{error, info, span};

use crate::database::Pool;
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

pub async fn serve_smtp(
    listener: &TcpListener,
    pool: Pool,
) -> Result<(), Box<dyn Error>> {
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let pool_arc = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_smtp_request(&mut socket, &pool_arc).await {
                error!("SMTP Handler Error: {}", e);
            }
        });
    }
    #[allow(unreachable_code)] // As we wait for the ! type..
    Ok(())
}

async fn handle_smtp_request(
    stream: &mut TcpStream,
    pool: &Pool,
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

    match entry.save(pool).await {
        Ok(_) => {
            info!("Email stored as {}", entry);
            Ok(SMTPResult::Success { email: None })
        }
        Err(e) => Err(format!("Couldn't INSERT email {} ({})", entry, e)),
    }
}
