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
    Done,
    Quit,
}

#[derive(Debug)]
pub enum Event {
    Greeted,
    MailFrom,
    RcptTo { rcpt_to: String },
    Data,
    EndOfFile { buf: String },
    Reset,
    Quit,
}

impl State {
    pub fn next(self, event: &Event) -> State {
        match (self, event) {
            (State::Connected, Event::Greeted) => State::Greeted,
            (State::Greeted, Event::MailFrom) => State::MailFrom,
            (State::MailFrom, Event::RcptTo { rcpt_to: _ }) => State::RcptTo,
            (State::RcptTo, Event::Data) => State::Data,
            (State::Data, Event::EndOfFile { buf: _ }) => State::Done,
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

    pub async fn run(&self, stream: &mut BufReader<&mut TcpStream>) -> Event {
        match *self {
            State::Connected => {
                debug!("Sending 220 READY");
                State::send_command(stream, "220 READY").await;
            }
            State::Greeted | State::MailFrom | State::RcptTo => {
                debug!("Sending 250 OK to {:#?}", *self);
                State::send_command(stream, "250 OK").await;
            }
            State::Data => {
                debug!("Sending 354 DALEMAMBO to {:#?}", *self);
                State::send_command(stream, "354 DALEMAMBO").await;
            }
            State::Done | State::Quit => {
                debug!("Sending OK & QUIT to {:#?}", *self);
                State::send_command(stream, "250 BYE FELICIA").await;
                return Event::Quit;
            }
        }

        let mut buf = String::new();
        let mut loop_buf = String::new();
        if *self == State::Data {
            loop {
                stream.read_line(&mut loop_buf).await.unwrap();
                debug!("Looper collected: {}", loop_buf);
                match loop_buf.as_str() {
                    ".\r\n" => {
                        debug!("Found the escape seq, returning buffer");
                        return Event::EndOfFile { buf };
                    }
                    "" => {
                        debug!("Broken Pipe?");
                        return Event::Quit;
                    }
                    _ => {
                        debug!("Collected line, continuing");
                        buf.push_str(&loop_buf);
                        loop_buf.clear();
                    }
                }
            }
        } else {
            stream.read_line(&mut buf).await.unwrap();
            debug!("read SMTP command: {}, len: {}", buf.trim(), buf.len());
        }

        let command = buf.to_string();

        match &command[..4] {
            "EHLO" | "HELO" => Event::Greeted,
            "MAIL" => Event::MailFrom,
            "RCPT" => Event::RcptTo { rcpt_to: buf },
            "DATA" => Event::Data,
            "QUIT" => Event::Quit,
            "RSET" => Event::Reset,
            _ => Event::Quit,
        }
    }
}
