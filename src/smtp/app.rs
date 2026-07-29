//! # SMTP server main entry point
//!
//! Receives a listener and spawns a green thread for each open connection,
//! uses the [`State`] machine to tease out the newsletter email from the
//! client, then parses and stores the entry if it's valid.

use std::error::Error;
use std::time::Duration;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tracing::{error, info, span, warn};

use crate::database::Pool;
use crate::models::Entry;
use crate::smtp::state_machine::State;

/// Hard cap on total session lifetime, as defense in depth on top of the
/// per-line read timeout inside the state machine — bounds how long a
/// misbehaving or idle client can occupy a connection slot.
const SESSION_TIMEOUT: Duration = Duration::from_secs(300);

pub struct Email {
    pub rcpt: String,
    pub body: String,
}

pub enum SMTPResult {
    HealthCheck,
    /// The client's RCPT TO targeted an unknown feed and was rejected with
    /// a 550 response; there is no envelope to save.
    Rejected,
    Success {
        email: Option<Email>,
    },
}

pub async fn serve_smtp(
    listener: &TcpListener,
    pool: Pool,
) -> Result<(), Box<dyn Error>> {
    loop {
        let (mut socket, peer) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                warn!("SMTP accept error: {}", e);
                continue;
            }
        };
        let pool_arc = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_smtp_request(&mut socket, &pool_arc).await {
                error!("SMTP Handler Error from {}: {}", peer, e);
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

    let run_result =
        match timeout(SESSION_TIMEOUT, state.run(&mut stream, pool)).await {
            Ok(r) => r,
            Err(_) => {
                return Err("SMTP session exceeded the time limit".to_owned())
            }
        };

    let envelope: Email = match run_result {
        Ok(SMTPResult::HealthCheck) => return Ok(SMTPResult::HealthCheck),
        Ok(SMTPResult::Rejected) => return Ok(SMTPResult::Rejected),
        Err(e) => return Err(e),
        Ok(SMTPResult::Success { email: Some(e) }) => e,
        Ok(SMTPResult::Success { email: None }) => {
            return Ok(SMTPResult::Rejected)
        }
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
        Ok(()) => {
            info!("Email stored as {}", entry);
            Ok(SMTPResult::Success { email: None })
        }
        Err(e) => Err(format!("Couldn't INSERT email {} ({})", entry, e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use tokio::io::{
        AsyncBufReadExt, AsyncWriteExt, BufReader as TokioBufReader,
    };
    use tokio::net::TcpListener;

    const TEST_DOMAIN: &str = "ktnrs.integration-test";

    /// Connects to a Postgres instance and applies migrations, or returns
    /// `None` if `DATABASE_URL` isn't set / no DB is reachable. This lets
    /// these end-to-end tests run for real locally (e.g. against a throwaway
    /// container) while degrading to a no-op skip in CI, where no DB is
    /// configured for the test process.
    async fn test_pool() -> Option<Pool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        let pool = PgPoolOptions::new()
            .acquire_timeout(Duration::from_secs(3))
            .max_connections(5)
            .connect(&url)
            .await
            .ok()?;
        sqlx::migrate!("./migrations").run(&pool).await.ok()?;
        Some(pool)
    }

    /// Inserts a feed directly, bypassing the web layer, for test setup.
    async fn insert_feed(reference: &str, pool: &Pool) {
        sqlx::query(
            r#"INSERT INTO "feeds" ("reference", "title") VALUES ($1, $2)
               ON CONFLICT (reference) DO NOTHING"#,
        )
        .bind(reference)
        .bind(format!("Feed {}", reference))
        .execute(pool)
        .await
        .expect("test feed insert should succeed");
    }

    async fn spawn_test_server(pool: Pool) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind ephemeral test port");
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = serve_smtp(&listener, pool).await;
        });
        addr
    }

    /// Drives one line of an SMTP conversation: writes `line`, reads and
    /// returns the server's response line.
    async fn exchange(
        stream: &mut TokioBufReader<TcpStream>,
        line: &str,
    ) -> String {
        stream
            .get_mut()
            .write_all(format!("{}\r\n", line).as_bytes())
            .await
            .unwrap();
        let mut resp = String::new();
        stream.read_line(&mut resp).await.unwrap();
        resp
    }

    #[tokio::test]
    async fn rcpt_to_unknown_feed_is_rejected() {
        std::env::set_var("EMAIL_DOMAIN", TEST_DOMAIN);
        let Some(pool) = test_pool().await else {
            eprintln!("skipping: no DATABASE_URL / DB unreachable");
            return;
        };

        let addr = spawn_test_server(pool).await;
        let raw = TcpStream::connect(addr).await.unwrap();
        let mut stream = TokioBufReader::new(raw);

        let mut greeting = String::new();
        stream.read_line(&mut greeting).await.unwrap();
        assert!(greeting.starts_with("220"));

        assert!(exchange(&mut stream, "EHLO tester")
            .await
            .starts_with("250"));
        assert!(exchange(&mut stream, "MAIL FROM:<sender@example.com>")
            .await
            .starts_with("250"));

        let rcpt_resp = exchange(
            &mut stream,
            &format!("RCPT TO:<doesnotexist@{}>", TEST_DOMAIN),
        )
        .await;
        assert!(
            rcpt_resp.starts_with("550"),
            "expected 550 for unknown feed, got {:?}",
            rcpt_resp
        );
    }

    #[tokio::test]
    async fn mail_is_routed_by_envelope_recipient_not_forged_to_header() {
        std::env::set_var("EMAIL_DOMAIN", TEST_DOMAIN);
        let Some(pool) = test_pool().await else {
            eprintln!("skipping: no DATABASE_URL / DB unreachable");
            return;
        };

        let real_reference = "envtest-real-0001";
        let forged_reference = "envtest-forged-0002";
        insert_feed(real_reference, &pool).await;
        insert_feed(forged_reference, &pool).await;

        let addr = spawn_test_server(pool.clone()).await;
        let raw = TcpStream::connect(addr).await.unwrap();
        let mut stream = TokioBufReader::new(raw);

        let mut greeting = String::new();
        stream.read_line(&mut greeting).await.unwrap();

        assert!(exchange(&mut stream, "EHLO tester")
            .await
            .starts_with("250"));
        assert!(exchange(&mut stream, "MAIL FROM:<sender@example.com>")
            .await
            .starts_with("250"));
        // Envelope RCPT targets the REAL feed...
        let rcpt_resp = exchange(
            &mut stream,
            &format!("RCPT TO:<{}@{}>", real_reference, TEST_DOMAIN),
        )
        .await;
        assert!(rcpt_resp.starts_with("250"));
        assert!(exchange(&mut stream, "DATA").await.starts_with("354"));

        // ...but the message's To: header (sender-controlled) claims the
        // FORGED feed. If routing still trusted the header, this entry
        // would land on `forged_reference` instead.
        let body = format!(
            "From: attacker@example.com\r\n\
             To: {forged}@{domain}\r\n\
             Subject: Envelope routing test\r\n\
             \r\n\
             body\r\n\
             .\r\n",
            forged = forged_reference,
            domain = TEST_DOMAIN
        );
        stream.get_mut().write_all(body.as_bytes()).await.unwrap();
        let mut data_resp = String::new();
        stream.read_line(&mut data_resp).await.unwrap();
        assert!(data_resp.starts_with("250"));

        // Give the spawned handler a moment to finish the INSERT.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let real_entries = Entry::find_by_reference(real_reference, &pool)
            .await
            .unwrap();
        let forged_entries = Entry::find_by_reference(forged_reference, &pool)
            .await
            .unwrap();

        assert!(
            real_entries
                .iter()
                .any(|e| e.title == "Envelope routing test"),
            "entry should have been routed to the envelope recipient"
        );
        assert!(
            !forged_entries
                .iter()
                .any(|e| e.title == "Envelope routing test"),
            "entry must NOT be routed based on the forged To: header"
        );
    }
}
