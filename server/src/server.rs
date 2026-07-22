use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::handler;
use crate::store::AsyncStore;

pub use crate::handler::ConnectionConfig;

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
                        let (socket, _) = match res {
                            Ok(r) => r,
                            Err(e) => {
                                log::error!("failed opening socket: {}", e);
                                continue;
                            }
                        };
                        let store_handle = self.store.clone();
                        tracker.spawn(handler::handle_request(
                            socket,
                            self.cancel_token.clone(),
                            store_handle,
                            self.connection_config,
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
