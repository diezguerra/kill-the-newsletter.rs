//! # SMTP State Machine
//!
//! First and clumsy attempt at building a state machine to keep track of
//! SMTP back and forth communication. Seems to work for simple cases...

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::debug;

#[derive(Debug, PartialEq)]
pub enum State {
    Connected,
    Greeted,
    MailFrom,
    RcptTo,
    Data,
    Failed,
    Done,
    Quit,
}

#[derive(Debug)]
pub enum Event {
    Greeting,
    HealthCheck,
    NoTls,
    MailFrom,
    Recipient { rcpt: String },
    Data,
    EndOfFile { buf: String },
    Reset,
    Fail { msg: String },
    Quit,
}

impl State {
    pub fn next(self, event: &Event) -> State {
        match (self, event) {
            (State::Connected, Event::Greeting) => State::Greeted,
            (state, Event::NoTls) => state,
            (State::Connected, _) => State::Failed,
            (State::Greeted, Event::MailFrom) => State::MailFrom,
            (State::Greeted, _) => State::Failed,
            (State::MailFrom, Event::Recipient { rcpt: _ }) => State::RcptTo,
            (State::MailFrom, _) => State::Failed,
            (State::RcptTo, Event::Data) => State::Data,
            (State::RcptTo, _) => State::Failed,
            (State::Data, Event::EndOfFile { buf: _ }) => State::Done,
            (State::Data, _) => State::Failed,
            (_, Event::Fail { msg: _ }) => State::Failed,
            (_, Event::Quit) => State::Quit,
            (_, Event::Reset) => State::Quit,
            (_, _) => State::Quit,
        }
    }

    async fn send_command(
        stream: &mut BufReader<&mut TcpStream>,
        command: &str,
    ) {
        let _ = stream
            .write_all(format!("{}\r\n", command).as_bytes())
            .await;
    }

    // We respond to the latest command based on the current State, then
    // process the response and generate an event with or without payload
    pub async fn run(&self, stream: &mut BufReader<&mut TcpStream>) -> Event {
        match *self {
            State::Connected => {
                debug!("Sending 220 READY");
                State::send_command(stream, "220 SHOWMEWHATCHAGOT").await;
            }
            State::Greeted | State::MailFrom | State::RcptTo => {
                debug!("Responding 250 OK to {:#?}", *self);
                State::send_command(stream, "250 YEASURE").await;
            }
            State::Data => {
                debug!("Responding 354 DALEMAMBO to {:#?}", *self);
                State::send_command(stream, "354 DALEMAMBO").await;
            }
            State::Failed => {
                debug!("Failed Event, QUITting {:#?}", *self);
                State::send_command(stream, "QUIT ").await;
                return Event::Fail {
                    msg: "Wrong command order".to_owned(),
                };
            }
            State::Done => {
                debug!("Responding OK & QUIT from {:#?}", *self);
                State::send_command(stream, "250 DULYNOTED").await;
            }
            State::Quit => {
                debug!("Responding OK & QUIT from {:#?}", *self);
                State::send_command(stream, "250 BYEFELICIA").await;
                State::send_command(stream, "QUIT ").await;
                return Event::Quit;
            }
        }

        let mut buf = String::new();

        // If we're receiving data, we loop until we find the lone period
        // character that signals EOF, or till the pipe is broken. As we
        // loop, we push what we get into the main buffer and clear clear the
        // local one.
        if *self == State::Data {
            let mut loop_buf = String::new();

            loop {
                stream.read_line(&mut loop_buf).await.unwrap();
                match loop_buf.as_str() {
                    ".\r\n" => {
                        debug!("Found the escape seq, returning buffer");
                        return Event::EndOfFile { buf };
                    }
                    "" => {
                        debug!("Broken Pipe?");
                        return Event::Fail {
                            msg: "Broken pipe".to_owned(),
                        };
                    }
                    _ => {
                        //debug!("Collected line, continuing");
                        buf.push_str(&loop_buf);
                        loop_buf.clear();
                    }
                }
            }
            // If we're not receiving DATA, we just read a one-line command
        } else {
            match stream.read_line(&mut buf).await {
                Ok(_) => {
                    debug!(
                        "read SMTP command: {}, (len: {})",
                        buf.trim(),
                        buf.len()
                    );
                }
                Err(_) => {
                    if !buf.is_empty() {
                        return Event::Fail {
                            msg: format!(
                                "Invalid Command: {}",
                                buf.get(..std::cmp::max(20, buf.len()))
                                    .unwrap()
                            ),
                        };
                    }
                }
            }

            // No command (TCP healthcheck)
            if buf.trim().is_empty() {
                return Event::HealthCheck;
            }
        }

        // SMTP clients shouldn't unilaterally request TLS without being
        // explicitly told ESMTP and STARTTLS is fair game, but some are
        // pretty cheeky, so
        if buf.len() >= 8 && buf[..8].eq_ignore_ascii_case("STARTTLS") {
            State::send_command(stream, "454 TLSTOOHARDTOIMPL").await;
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
            "QUIT" => Event::Quit,
            "RSET" => Event::Reset,
            _ => match *self {
                State::Done | State::Quit => Event::Quit,
                _ => Event::Fail {
                    msg: format!("Invalid command: {}", command.trim()),
                },
            },
        }
    }
}
