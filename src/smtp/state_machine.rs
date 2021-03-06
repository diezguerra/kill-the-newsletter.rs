//! # SMTP State Machine
//!
//! First and clumsy attempt at building a state machine to keep track of
//! SMTP back and forth communication. Seems to work for simple cases...

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, trace};

use crate::smtp::app::{Email, SMTPResult};
use crate::vars::EMAIL_DOMAIN;

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

    async fn read_line(
        stream: &mut BufReader<&mut TcpStream>,
        buf: &mut String,
    ) -> Result<(), String> {
        match stream.read_line(buf).await {
            Ok(_) => Ok(()),
            Err(e) => Err(format!(
                "Line[:20]: {} Error: {} ",
                buf.get(..std::cmp::min(20, buf.len())).unwrap().to_owned(),
                e
            )),
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
                State::send_command(stream, &format!("220 {}", EMAIL_DOMAIN))
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
            stream.read_line(&mut buf).await.unwrap();
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
    ) -> Result<SMTPResult, String> {
        tracing::Span::current().record(
            "peer",
            &&stream.get_ref().peer_addr().unwrap().to_string()[..],
        );
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
                    email.rcpt.push_str(rcpt.trim());
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
