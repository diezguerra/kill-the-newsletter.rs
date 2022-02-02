use std::str;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

fn main() {
    // Listen for incoming TCP connections on localhost port 7878
	const BIND_ADDRESS: &str = "127.0.0.1:7878";

    let listener = TcpListener::bind(BIND_ADDRESS).unwrap();

	println!("Listening on {}...", BIND_ADDRESS);

    // Block forever, handling each request that arrives at this IP address
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    // Read the first 1024 bytes of data from the stream
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();

    let response = format!("Received {}", str::from_utf8(&buffer).unwrap_or(""));

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}
