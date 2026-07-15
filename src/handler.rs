use rmp_serde::Serializer;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::protocol;

pub async fn handle_request(mut socket: TcpStream) {
    let mut receive_buf = Vec::<u8>::new();
    if let Err(_) = socket.read(&mut receive_buf).await {}
    let _request: protocol::Request = rmp_serde::from_slice(&receive_buf).unwrap();

    // process request

    let response = protocol::Response {
        status: protocol::ResponseStatus::Ok,
        message: "to be implemented".to_string(),
        value: None,
    };
    let mut send_buf = Vec::<u8>::new();
    response
        .serialize(&mut Serializer::new(&mut send_buf))
        .unwrap();
    if let Err(_) = socket.write(&mut send_buf).await {}
}
