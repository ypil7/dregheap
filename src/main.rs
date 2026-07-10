use tokio::net::{TcpListener};
use clap::Parser;

mod cli;

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();
    let ip_addr = format!("0.0.0.0:{}", args.port);

    let listener = TcpListener::bind(&ip_addr).await
        .unwrap_or_else( |e| panic!("Failed to bind TCP Listener to: {}, error: {}", &ip_addr, e));

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        if let Err(e) = socket.try_write(&[1,2,4]) {
            panic!("Failed writing bites to buffer: {}", e)
        }
    }
}
