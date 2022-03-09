use regex::Regex;
use std::error::Error;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

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

pub async fn serve_smtp(listener: &TcpListener) -> Result<(), Box<dyn Error>> {
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            match serve_smtp_request(&mut socket).await {
                Ok(_) => debug!("SMTP request succeeded!"),
                Err(e) => error!("SMTP response failed: {:#?}", e),
            }
        });
    }
}

async fn serve_smtp_request(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn Error>> {
    let mut stream = BufReader::new(stream);

    let mut state = State::Connected;

    let mut recipient = String::new();
    let mut email = String::new();

    loop {
        let event: Event = state.run(&mut stream).await;
        state = state.next(&event);
        match event {
            Event::Recipient { recipient: rcpt } => {
                debug!("RCPT TO={}", rcpt.trim());
                recipient.push_str(rcpt.trim());
            }
            Event::EndOfFile { buf } => {
                //debug!("DATA={}", buf.trim());
                email.push_str(buf.trim());
            }
            Event::Fail { msg } => return Err(msg.into()),
            Event::Quit => break,
            _ => {}
        }
    }

    let email_find = Regex::new(EMAIL_REGEX).unwrap();
    let recipient = match email_find.find(&recipient) {
        Some(m) => m.as_str(),
        _ => "invalid@email.address",
    };

    debug!("Received email for {}", recipient);

    let parsed: ParsedEmail = parse(email.as_bytes());

    if !recipient.ends_with(EMAIL_DOMAIN) && !parsed.to.ends_with(EMAIL_DOMAIN)
    {
        debug!("Received invalid inbox, discard message");
    } else {
        // Store in DB
        debug!("Storing email {}", parsed);
    }

    Ok(())
}
