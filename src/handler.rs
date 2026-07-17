use rmp_serde::Serializer;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::protocol::RequestMethod::{Get, Set};
use crate::protocol::{self, Request, Response};
use crate::store::AsyncStore;

// Controller
pub async fn handle_request(mut socket: TcpStream, store: AsyncStore) {
    let mut receive_buf = vec![0u8; 4096];
    if let Err(e) = socket.read(&mut receive_buf).await {
        panic!("failed reading from socket: {}", e)
    };

    let request: protocol::Request = rmp_serde::from_slice(&receive_buf).unwrap();

    // process request
    let response = process_request(request, store);

    let mut send_buf = Vec::<u8>::new();
    response
        .serialize(&mut Serializer::new(&mut send_buf))
        .unwrap();
    if socket.write(&send_buf).await.is_err() {}
}

// Service
fn process_request(req: Request, store: AsyncStore) -> Response {
    match req.method {
        Set => {
            let mut store = store.lock().unwrap();
            if let Err(e) = store.set(req.key, req.value) {
                Response {
                    status: protocol::ResponseStatus::Error,
                    message: e.to_string(),
                    value: None,
                }
            } else {
                Response {
                    status: protocol::ResponseStatus::Ok,
                    message: "Success".to_string(),
                    value: None,
                }
            }
        }
        Get => {
            let store = store.lock().unwrap();
            match store.get(&req.key) {
                Ok(val) => protocol::Response {
                    status: protocol::ResponseStatus::Ok,
                    message: "found value".into(),
                    value: Some(val.clone()),
                },
                Err(e) => protocol::Response {
                    status: protocol::ResponseStatus::Error,
                    message: e.to_string(),
                    value: None,
                },
            }
        }
    }
}
