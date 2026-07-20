use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use protocol::{Request, RequestMethod, Response, ResponseStatus};
use rmp_serde::Serializer;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::server;
use crate::store::make_store;

struct TestServer {
    addr: SocketAddr,
    cancel_token: CancellationToken,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind to an available port");
        let addr = listener
            .local_addr()
            .expect("bound test listener should have a local address");
        let store = Arc::new(Mutex::new(make_store()));
        let cancel_token = CancellationToken::new();
        let handle = server::Server::new(store, cancel_token.clone(), listener).start();

        Self {
            addr,
            cancel_token,
            handle,
        }
    }

    async fn shutdown(self) {
        self.cancel_token.cancel();
        self.handle
            .await
            .expect("test server task should shut down cleanly");
    }
}

async fn send_request(addr: SocketAddr, request: Request) -> Response {
    let mut request_buf = Vec::new();
    request
        .serialize(&mut Serializer::new(&mut request_buf))
        .expect("test request should serialize");

    send_raw(addr, &request_buf).await
}

async fn send_raw(addr: SocketAddr, bytes: &[u8]) -> Response {
    let mut stream = TcpStream::connect(addr)
        .await
        .expect("client should connect to test server");
    stream
        .write_all(bytes)
        .await
        .expect("client should write request bytes");

    let mut response_buf = Vec::new();
    stream
        .read_to_end(&mut response_buf)
        .await
        .expect("client should read response bytes");

    rmp_serde::from_slice(&response_buf).expect("server response should be valid MessagePack")
}

#[tokio::test]
async fn creates_and_gets_value() {
    let server = TestServer::start().await;
    let key = "created-key".to_string();
    let value = b"created-value".to_vec();

    let response = send_request(
        server.addr,
        Request {
            method: RequestMethod::Set,
            key: key.clone(),
            value: Some(value.clone()),
        },
    )
    .await;

    assert!(matches!(response.status, ResponseStatus::Ok));
    assert_eq!(response.value, None);

    let response = send_request(
        server.addr,
        Request {
            method: RequestMethod::Get,
            key,
            value: None,
        },
    )
    .await;

    assert!(matches!(response.status, ResponseStatus::Ok));
    assert_eq!(response.value, Some(value));

    server.shutdown().await;
}

#[tokio::test]
async fn deletes_value() {
    let server = TestServer::start().await;
    let key = "deleted-key".to_string();

    let response = send_request(
        server.addr,
        Request {
            method: RequestMethod::Set,
            key: key.clone(),
            value: Some(b"temporary-value".to_vec()),
        },
    )
    .await;
    assert!(matches!(response.status, ResponseStatus::Ok));

    let response = send_request(
        server.addr,
        Request {
            method: RequestMethod::Set,
            key: key.clone(),
            value: None,
        },
    )
    .await;
    assert!(matches!(response.status, ResponseStatus::Ok));

    let response = send_request(
        server.addr,
        Request {
            method: RequestMethod::Get,
            key,
            value: None,
        },
    )
    .await;

    assert!(matches!(response.status, ResponseStatus::Error));
    assert_eq!(response.value, None);

    server.shutdown().await;
}

#[tokio::test]
async fn returns_error_for_missing_value() {
    let server = TestServer::start().await;

    let response = send_request(
        server.addr,
        Request {
            method: RequestMethod::Get,
            key: "missing-key".to_string(),
            value: None,
        },
    )
    .await;

    assert!(matches!(response.status, ResponseStatus::Error));
    assert_eq!(response.value, None);

    server.shutdown().await;
}

#[tokio::test]
async fn returns_error_for_malformed_request() {
    let server = TestServer::start().await;

    let response = send_raw(server.addr, b"not messagepack").await;

    assert!(matches!(response.status, ResponseStatus::Error));
    assert_eq!(response.value, None);
    assert!(response.message.starts_with("Malformed request:"));

    server.shutdown().await;
}
