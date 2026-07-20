use core::panic;
use dreg_server::store::make_store;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::signal;
use tokio_util::sync::CancellationToken;

use dreg_server::*;

#[tokio::main]
async fn main() {
    let cfg = config::load_config().unwrap();
    cfg.validate()
        .unwrap_or_else(|e| panic!("Invalid config: {}", e));

    logforth::starter_log::stdout().apply();

    let ip_addr = format!("0.0.0.0:{}", cfg.port);

    let listener = TcpListener::bind(&ip_addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind TCP Listener to: {}, error: {}", &ip_addr, e));

    let store = Arc::new(Mutex::new(make_store()));
    let cancel_token = CancellationToken::new();
    let server_handle = server::Server::new(store, cancel_token.clone(), listener).start();

    log::info!("Starting server on port: {}", cfg.port);

    terminate_signal().await;

    cancel_token.cancel();

    // wait for all tasks to finish
    server_handle.await.unwrap();
}

async fn terminate_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    };
}
