use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    // Bind the listener to the address
    let listener = TcpListener::bind("127.0.0.1:2525").await.unwrap();

    loop {
        // The second item contains the IP and port of the new connection.
        let (mut socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            process(&mut socket).await;
        });
    }
}

async fn process(stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    loop {
        stream.readable().await?;
        let mut buf = [0; 4096];
        stream.try_read(&mut buf);
        stream.write_all(&buf).await?;
    }
    Ok(())
}
