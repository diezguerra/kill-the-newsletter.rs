//! # SMTP State Machine
//!
//! First and clumsy attempt at building a state machine to keep track of
//! SMTP back and forth communication. Seems to work for simple cases...

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, trace};

use crate::database::Pool;
use crate::models::Feed;
use crate::smtp::app::{Email, SMTPResult};
use crate::smtp::parse::recipient_reference_from_rcpt;
use crate::vars::email_domain;

const MAX_EMAIL_BYTES: usize = 10 * 1024 * 1024; // 10 MB
/// Maximum bytes accepted for a single line (command or DATA body line)
/// before we give up on the client. Generous enough for real SMTP
/// commands and typical email header/body lines, but bounded so a
/// client can't force unbounded memory growth by never sending a `\n`.
const MAX_LINE_BYTES: usize = 8 * 1024;
/// How long we'll wait for a single line to complete before giving up.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, PartialEq)]
pub enum State {
    Connected,
    Greeted,
    MailFrom,
    RcptTo,
    Data,
    Done,
    Failed,
    Quit,
}

#[derive(Debug)]
pub enum Event {
    HealthCheck,
    Greeting,
    NoTls,
    MailFrom,
    Recipient { rcpt: String },
    Data,
    EndOfFile { buf: String },
    Fail { cmd: String },
    NoOp,
    Quit,
}

impl State {
    pub fn next(self, event: &Event) -> State {
        match (self, event) {
            (State::Connected, Event::Greeting) => State::Greeted,
            (state, Event::NoTls) => state,
            (state, Event::HealthCheck) => state,
            (state, Event::NoOp) => state,
            (State::Connected, _) => State::Failed,
            (State::Greeted, Event::MailFrom) => State::MailFrom,
            (State::Greeted, _) => State::Failed,
            (State::MailFrom, Event::Recipient { rcpt: _ }) => State::RcptTo,
            (State::MailFrom, _) => State::Failed,
            (State::RcptTo, Event::Data) => State::Data,
            (State::RcptTo, _) => State::Failed,
            (State::Data, Event::EndOfFile { buf: _ }) => State::Done,
            (State::Data, _) => State::Failed,
            (_, Event::Fail { cmd: _ }) => State::Failed,
            (_, Event::Quit) => State::Quit,
            (_, _) => State::Quit,
        }
    }

    async fn send_command(
        stream: &mut BufReader<&mut TcpStream>,
        command: &str,
    ) {
        debug!("Send SMTP command: {}", command);
        let _ = stream
            .write_all(format!("{}\r\n", command).as_bytes())
            .await;
    }

    /// Reads a single `\n`-terminated line, bounded in both size and time:
    /// gives up with an `Err` if `MAX_LINE_BYTES` is exceeded before a
    /// newline arrives, or if no newline arrives within `READ_TIMEOUT`.
    /// This guards against a client that sends an unbounded line (memory
    /// exhaustion) or that opens a connection and then goes idle forever
    /// (connection-slot exhaustion).
    async fn read_line(
        stream: &mut BufReader<&mut TcpStream>,
        buf: &mut String,
    ) -> Result<(), String> {
        let read_fut = async {
            let mut bytes = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                match stream.read_exact(&mut byte).await {
                    Ok(_) => {
                        bytes.push(byte[0]);
                        if byte[0] == b'\n' {
                            return Ok(bytes);
                        }
                        if bytes.len() > MAX_LINE_BYTES {
                            return Err(format!(
                                "Line exceeded {} byte limit",
                                MAX_LINE_BYTES
                            ));
                        }
                    }
                    Err(e) => return Err(format!("Read error: {}", e)),
                }
            }
        };

        match tokio::time::timeout(READ_TIMEOUT, read_fut).await {
            Ok(Ok(bytes)) => {
                buf.push_str(&String::from_utf8_lossy(&bytes));
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Timed out waiting for client data".to_owned()),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn recv_response(
        &self,
        stream: &mut BufReader<&mut TcpStream>,
    ) -> Result<String, String> {
        let mut buf = String::new();

        match *self {
            // If we're receiving data, we loop until we find the lone period
            // character that signals EOF, or till the pipe is broken. As we
            // loop, we push what we get into the main buffer and clear the
            // local one.
            State::Data => {
                let mut loop_buf = String::new();
                let mut loop_count: usize = 0;

                loop {
                    match State::read_line(stream, &mut loop_buf).await {
                        Ok(()) => {}
                        Err(e) => {
                            debug!("Failure while reading email DATA");
                            return Err(e);
                        }
                    };

                    match loop_buf.as_str() {
                        ".\r\n" => {
                            debug!(
                                "ESC found. loops={}, len={}, trimmed={}",
                                loop_count,
                                buf.len(),
                                buf.trim().len()
                            );
                            break;
                        }
                        _ => {
                            buf.push_str(&loop_buf);
                            loop_buf.clear();
                            loop_count += 1;

                            if buf.len() > MAX_EMAIL_BYTES {
                                return Err(format!(
                                    "Email body exceeded {} byte limit",
                                    MAX_EMAIL_BYTES
                                ));
                            }
                        }
                    }
                }
            }
            // If we're not receiving DATA, we just read a one-line command
            _ => State::read_line(stream, &mut buf).await?,
        }

        Ok(buf)
    }

    // We respond to the latest command based on the current State, then
    // process the response and generate an event with or without payload
    async fn step(&self, stream: &mut BufReader<&mut TcpStream>) -> Event {
        match *self {
            State::Connected => {
                State::send_command(stream, &format!("220 {}", email_domain()))
                    .await;
            }
            State::Greeted | State::MailFrom | State::RcptTo => {
                State::send_command(stream, "250 OK").await;
            }
            State::Data => {
                State::send_command(
                    stream,
                    "354 End data with <CR><LF>.<CR><LF>",
                )
                .await;
            }
            State::Failed => {
                State::send_command(stream, "502 Not Implemented").await;
                return Event::Quit;
            }
            State::Done => {
                State::send_command(stream, "250 OK").await;
                return Event::Quit;
            }
            State::Quit => {
                State::send_command(stream, "250 OK").await;
                State::send_command(stream, "QUIT ").await;
                return Event::Quit;
            }
        }

        let mut buf = String::new();
        match self.recv_response(stream).await {
            Ok(resp) => match *self {
                State::Data => return Event::EndOfFile { buf: resp },
                _ => buf.push_str(&resp),
            },
            Err(e) => {
                // Healthcheck so fast the pipe is closed by the time we read
                if *self == State::Connected
                    && e.contains("Connection reset by peer")
                {
                    return Event::HealthCheck;
                } else {
                    return Event::Fail { cmd: e };
                }
            }
        };

        // No command (TCP healthcheck)
        if buf.trim().is_empty() {
            State::send_command(stream, "500 Command Unrecognized").await;
            return Event::HealthCheck;
        }

        debug!(
            "Read SMTP command: {} (len={},trimmed={})",
            buf.trim(),
            buf.len(),
            buf.trim().len()
        );

        let mut buf = buf.trim().to_string();

        // SMTP clients shouldn't unilaterally request TLS without being
        // explicitly told ESMTP and STARTTLS is fair game, but some are
        // pretty cheeky, so:
        if buf.len() >= 8 && buf[..8].eq_ignore_ascii_case("STARTTLS") {
            State::send_command(stream, "454 TLS Not available").await;
            buf.clear();
            if let Err(e) = State::read_line(stream, &mut buf).await {
                return Event::Fail { cmd: e };
            }
        }

        let command = buf.split(' ').next().unwrap().to_ascii_uppercase();
        match command.trim() {
            "EHLO" | "HELO" => Event::Greeting,
            "STARTTLS" => Event::NoTls,
            "MAIL" => Event::MailFrom,
            "RCPT" => Event::Recipient { rcpt: buf },
            "DATA" => Event::Data,
            "NOOP" => {
                State::send_command(stream, "250 OK").await;
                Event::NoOp
            }
            "QUIT" | "RSET" => Event::Quit,
            _ => match *self {
                State::Done | State::Quit => Event::Quit,
                _ => Event::Fail {
                    cmd: command.trim().to_owned(),
                },
            },
        }
    }

    #[tracing::instrument(skip_all, fields(peer))]
    pub async fn run(
        mut self,
        stream: &mut BufReader<&mut TcpStream>,
        pool: &Pool,
    ) -> Result<SMTPResult, String> {
        let peer = stream
            .get_ref()
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_owned());
        tracing::Span::current().record("peer", &peer[..]);
        let mut email = Email {
            rcpt: String::new(),
            body: String::new(),
        };

        loop {
            let event: Event = self.step(stream).await;
            self = self.next(&event);
            match event {
                Event::HealthCheck => {
                    trace!("SMTP Health check");
                    return Ok(SMTPResult::HealthCheck);
                }
                Event::Recipient { rcpt } => {
                    let candidate = rcpt.trim().to_owned();
                    let known = match recipient_reference_from_rcpt(&candidate)
                    {
                        Some(reference) => Feed::exists(&reference, pool)
                            .await
                            .unwrap_or(false),
                        None => false,
                    };

                    if known {
                        email.rcpt.push_str(&candidate);
                    } else {
                        debug!("RCPT TO rejected, unknown feed: {}", candidate);
                        State::send_command(stream, "550 5.1.1 No such user")
                            .await;
                        return Ok(SMTPResult::Rejected);
                    }
                }
                Event::EndOfFile { buf } => {
                    email.body.push_str(buf.trim());
                }
                Event::Fail { cmd } => return Err(cmd),
                Event::Quit => break,
                _ => {}
            }
        }
        Ok(SMTPResult::Success { email: Some(email) })
    }
}
