use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::handler;
use crate::store::AsyncStore;

#[derive(Debug, Clone, Copy)]
pub struct ConnectionConfig {
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(10),
        }
    }
}

pub struct Server {
    store: AsyncStore,
    cancel_token: CancellationToken,
    listener: TcpListener,
    connection_config: ConnectionConfig,
}

impl Server {
    pub fn new(
        store: AsyncStore,
        cancel_token: CancellationToken,
        listener: TcpListener,
        connection_config: ConnectionConfig,
    ) -> Server {
        Server {
            store,
            cancel_token,
            listener,
            connection_config,
        }
    }

    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let tracker = TaskTracker::new();

            loop {
                tokio::select! {
                    _ = self.cancel_token.cancelled() => {
                        break;
                    }
                    res = self.listener.accept() => {
                        let (socket, _) = res.unwrap();
                        let store_handle = self.store.clone();
                        tracker.spawn(handler::handle_request(
                            socket,
                            self.cancel_token.clone(),
                            store_handle,
                            self.connection_config.read_timeout,
                            self.connection_config.write_timeout,
                        ));
                    }
                }
            }

            tracker.close();
            tracker.wait().await;
            log::info!("Shutting down server");
        })
    }
}
