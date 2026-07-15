use core::panic;
use tokio::net::TcpListener;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use dregheap::*;

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

    log::info!("Starting server on port: {}", cfg.port);

    let store = store::new_store();

    let cancel_token = CancellationToken::new();
    let dispatch_token = cancel_token.clone();

    let dispatch = tokio::spawn(async move {
        let tracker = TaskTracker::new();

        loop {
            tokio::select! {
                _ = dispatch_token.cancelled() => {
                    break;
                }
                res = listener.accept() => {
                    let (socket, _) = res.unwrap();
                    let store_handle = store.clone();
                    tracker.spawn(handler::handle_request(socket, store_handle));
                }
            }
        }

        tracker.close();
        tracker.wait().await;
        log::info!("Shutting down server");
    });

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

    cancel_token.cancel();

    dispatch.await.unwrap();
}
