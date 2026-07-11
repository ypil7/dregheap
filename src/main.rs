use tokio::net::{TcpListener};

mod config;

#[tokio::main]
async fn main() {
    let cfg = config::load_config().unwrap();

    logforth::starter_log::stdout().apply();

    let ip_addr = format!("0.0.0.0:{}", cfg.port);
    

    let listener = TcpListener::bind(&ip_addr).await
        .unwrap_or_else( |e| panic!("Failed to bind TCP Listener to: {}, error: {}", &ip_addr, e));

    log::info!("Starting server on port: {}", cfg.port);

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        if let Err(e) = socket.try_write(&[1,2,4]) {
            panic!("Failed writing bites to buffer: {}", e)
        }
    }
}
