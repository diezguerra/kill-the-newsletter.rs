use std::error::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

pub async fn serve_smtp(listener: &TcpListener) -> Result<(), Box<dyn Error>> {
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            match serve_smtp_request(&mut socket).await {
                Ok(_) => println!("SMTP response succeeded!"),
                Err(e) => println!("SMTP response failed: {:#?}", e),
            }
        });
    }
}

async fn serve_smtp_request_echo(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn Error>> {
    let mut stream = BufReader::new(stream);
    loop {
        let mut buf = String::new();
        stream.read_line(&mut buf).await?;

        if buf.starts_with("QUIT") {
            stream.write_all(b"221 BYE").await?;
            break;
        }
        stream.write_all(buf.as_bytes()).await?;
    }
    Ok(())
}

async fn serve_smtp_request(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn Error>> {
    let mut stream = BufReader::new(stream);
    loop {
        let mut buf = String::new();
        stream.read_line(&mut buf).await?;

        if buf.starts_with("QUIT") {
            stream.write_all(b"221 BYE").await?;
            break;
        }
        stream.write_all(buf.as_bytes()).await?;
    }
    Ok(())
}
