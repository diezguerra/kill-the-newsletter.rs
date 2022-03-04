use std::error::Error;
use tokio::io::AsyncWriteExt;
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

async fn serve_smtp_request(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn Error>> {
    loop {
        stream.readable().await?;
        let mut buf = [0; 4096];
        stream.try_read(&mut buf); // if ?'ed, gets Kind( WouldBlock,)
        if std::str::from_utf8(&buf)?.starts_with("QUIT") {
            stream.write_all(b"221 BYE").await?;
            break;
        }
        stream.write_all(&buf).await?;
    }
    Ok(())
}
