use rmp_serde::Serializer;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::protocol;
use crate::protocol::RequestMethod::{Get, Set};
use crate::store::DregStore;

pub async fn handle_request(mut socket: TcpStream, store: DregStore) {
    let mut receive_buf = vec![0u8; 4096];
    if let Err(e) = socket.read(&mut receive_buf).await {
        panic!("failed reading from socket: {}", e)
    };

    let request: protocol::Request = rmp_serde::from_slice(&receive_buf).unwrap();

    // process request
    let response = match request.method {
        Set => {
            let mut store = store.lock().unwrap();
            if let Some(val) = request.value {
                store.insert(request.key.clone(), val);
                protocol::Response {
                    status: protocol::ResponseStatus::Ok,
                    message: "inserted value".into(),
                    value: None,
                }
            } else {
                store.remove(&request.key);
                protocol::Response {
                    status: protocol::ResponseStatus::Ok,
                    message: "deleted value".into(),
                    value: None,
                }
            }
        },
        Get => {
            let store = store.lock().unwrap();
            if let Some(val) = store.get(&request.key) {
                protocol::Response {
                    status: protocol::ResponseStatus::Ok,
                    message: "found value".into(),
                    value: Some(val.clone()),
                }
            } else {
                protocol::Response {
                    status: protocol::ResponseStatus::Error,
                    message: "not found".into(),
                    value: None
                }
            }
        }
    };

    let mut send_buf = Vec::<u8>::new();
    response
        .serialize(&mut Serializer::new(&mut send_buf))
        .unwrap();
    if socket.write(&send_buf).await.is_err() {}
}
