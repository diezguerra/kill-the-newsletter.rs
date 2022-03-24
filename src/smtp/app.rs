//! # SMTP server main entry point
//!
//! Receives a listener and spawns a green thread for each open connection,
//! uses the [`State`] machine to tease out the newsletter email from the
//! client, then parses and stores the entry if it's valid.

use r2d2_sqlite::SqliteConnectionManager;
use regex::Regex;
use std::error::Error;
use std::sync::Arc;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

use crate::database::get_db_pool;
use crate::models::Entry;
use crate::smtp::parse::{parse, ParsedEmail};
use crate::smtp::state_machine::{Event, State};
use crate::vars::EMAIL_DOMAIN;

// Yanked blindly from https://emailregex.com/
const EMAIL_REGEX: &str = concat!(
    r#"(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)"#,
    r#"*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-"#,
    r#"\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\"#,
    r#".)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:25[0-5]|2[0-4][0-9]|[0"#,
    r#"1]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?|[a-z"#,
    r#"0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|"#,
    r#"\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])"#
);

struct Email {
    rcpt: String,
    body: String,
}

pub async fn serve_smtp(listener: &TcpListener) -> Result<(), Box<dyn Error>> {
    let pool_arc = get_db_pool();

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let pool = pool_arc.clone();
        tokio::spawn(async move {
            match serve_smtp_request(&mut socket, pool).await {
                Ok(msg) => {
                    if !msg.starts_with("Health check") {
                        info!("{}", msg)
                    }
                }
                Err(e) => error!("{}", e),
            }
        });
    }
}

async fn serve_smtp_request(
    stream: &mut TcpStream,
    pool: Arc<r2d2::Pool<SqliteConnectionManager>>,
) -> Result<String, Box<dyn Error>> {
    let mut stream = BufReader::new(stream);

    let mut state = State::Connected;

    let mut email = Email {
        rcpt: String::new(),
        body: String::new(),
    };

    loop {
        let event: Event = state.run(&mut stream).await;
        state = state.next(&event);
        match event {
            Event::HealthCheck => return Ok("Health check passed".to_owned()),
            Event::Recipient { rcpt } => {
                email.rcpt.push_str(rcpt.trim());
            }
            Event::EndOfFile { buf } => {
                email.body.push_str(buf.trim());
            }
            Event::Fail { cmd } => return Err(cmd.into()),
            Event::Quit => break,
            _ => {}
        }
    }

    if email.rcpt.is_empty() && email.body.is_empty() {
        return Ok("Empty envelope received and discarded".to_owned());
    }

    let email_find = Regex::new(EMAIL_REGEX).unwrap();
    let recipient = match email_find.find(&email.rcpt) {
        Some(m) => m.as_str(),
        _ => "invalid@email.address",
    };

    debug!("Received email for {}", recipient);

    let parsed: ParsedEmail = parse(email.body.as_bytes());

    let parsed_to = match email_find.find(&parsed.to) {
        Some(m) => m.as_str(),
        _ => "invalid@email.address",
    };

    debug!("Parsed envelope addressed to {}", parsed_to);

    let received = Entry {
        id: 0, // this won't be used
        created_at: parsed.date,
        reference: parsed_to.split('@').next().unwrap_or("").to_owned(),
        title: parsed.subject,
        author: parsed.from,
        content: parsed.body,
    };

    if !(recipient.ends_with(EMAIL_DOMAIN) || parsed.to.ends_with(EMAIL_DOMAIN))
    {
        Ok(format!(
            "Email for {} received and discarded. Parsed entry: {}",
            recipient, received
        ))
    } else {
        // Store in DB
        if let Ok(mut conn) = pool.get() {
            match received.save(&mut conn) {
                Ok(_) => Ok(format!(
                    "Email for {} received and stored as {}",
                    recipient, received
                )),
                Err(e) => Err(format!(
                    "Couldn't INSERT email for {} {} ({})",
                    recipient, received, e
                )
                .into()),
            }
        } else {
            Err(format!(
                "Couldn't get DB connection, dropping email for {} {}",
                recipient, received
            )
            .into())
        }
    }
}
