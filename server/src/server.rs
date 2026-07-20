use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::handler;
use crate::store::AsyncStore;

pub struct Server {
    store: AsyncStore,
    cancel_token: CancellationToken,
    listener: TcpListener,
}

impl Server {
    pub fn new(
        store: AsyncStore,
        cancel_token: CancellationToken,
        listener: TcpListener,
    ) -> Server {
        Server {
            store,
            cancel_token,
            listener,
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
                        tracker.spawn(handler::handle_request(socket, store_handle));
                    }
                }
            }

            tracker.close();
            tracker.wait().await;
            log::info!("Shutting down server");
        })
    }
}
