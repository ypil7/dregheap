#[cfg(test)]
mod e2e {
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;
    use tokio::{net::TcpStream, task::JoinHandle};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_util::sync::CancellationToken;
    use crate::store::make_store;

    use crate::server;

    async fn get_dreg_server(cancel_token: CancellationToken) -> JoinHandle<()> {
        let ip_addr = format!("0.0.0.0:{}", 6767);

        let listener = TcpListener::bind(&ip_addr)
            .await
            .unwrap_or_else(|e| panic!("Failed to bind TCP Listener to: {}, error: {}", &ip_addr, e));

        let store = Arc::new(Mutex::new(make_store()));
        server::Server::new(store, cancel_token.clone(), listener).start()
    }

    #[tokio::test]
    async fn simple_flow() {
        // Setup
        let cancel_token = CancellationToken::new();
        let server_handle = get_dreg_server(cancel_token.clone()).await;

        let mut stream = TcpStream::connect("localhost:6767").await.unwrap();
        stream.write_all("This is a bullshit test".as_bytes()).await.unwrap();
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();

        // Cleanup
        cancel_token.cancel();
        server_handle.await.unwrap();
    }
}
