use std::error::Error;
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, error, info};

use crate::smtp::state_machine::{Event, State};

pub async fn serve_smtp(listener: &TcpListener) -> Result<(), Box<dyn Error>> {
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            match serve_smtp_request(&mut socket).await {
                Ok(_) => debug!("SMTP response succeeded!"),
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
            Event::RcptTo { rcpt_to } => {
                debug!("RCPT TO={}", rcpt_to.trim());
                recipient.push_str(rcpt_to.trim());
            }
            Event::EndOfFile { buf } => {
                debug!("DATA={}", buf.trim());
                email.push_str(buf.trim());
            }
            Event::Quit => break,
            _ => {}
        }
    }

    info!("An email was received to address {}", recipient.trim());

    Ok(())
}
