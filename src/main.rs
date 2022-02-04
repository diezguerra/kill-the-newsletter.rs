use std::result::Result as Rs;
use async_std::{
    io::BufReader,
    prelude::*,
    task,
    net::{TcpStream, TcpListener, ToSocketAddrs},
};

type Result<T> = Rs<T, Box<dyn std::error::Error + Send + Sync>>;

async fn bind_server(addr: impl ToSocketAddrs) -> Result<()> {

    let listener = TcpListener::bind(addr).await?;
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let peer = stream.peer_addr()?;
        println!("Accepting from: {}", peer);
        let _handle = task::spawn(echo_server(stream));
    }

    Ok(())
}

async fn echo_server(mut stream: TcpStream) -> Result<()> {
    loop {
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).await?;
        stream.write_all(format!("Received {}", line).as_bytes()).await?;
    }
}

fn main() -> Result<()> {

    const SMTP_ADDRESS: &str = "127.0.0.1:7878";
    const HTTP_ADDRESS: &str = "127.0.0.1:7879";

    let smtp = task::spawn(bind_server(SMTP_ADDRESS));
    let http = task::spawn(bind_server(HTTP_ADDRESS));
    task::block_on(smtp)?;
    task::block_on(http)?;
    Ok(())
}
