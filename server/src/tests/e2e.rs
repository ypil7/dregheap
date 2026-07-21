use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use protocol::{Request, RequestMethod, Response, ResponseStatus};
use rmp_serde::Serializer;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;
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
        timeout(Duration::from_secs(1), self.handle)
            .await
            .expect("test server should shut down within timeout")
            .expect("test server task should shut down cleanly");
    }
}

async fn send_request(addr: SocketAddr, request: Request) -> Response {
    let mut request_buf = Vec::new();
    request
        .serialize(&mut Serializer::new(&mut request_buf))
        .expect("test request should serialize");

    send_body(addr, &request_buf).await
}

async fn send_body(addr: SocketAddr, body: &[u8]) -> Response {
    let mut stream = TcpStream::connect(addr)
        .await
        .expect("client should connect to test server");
    let body_len = u32::try_from(body.len()).expect("test request body should fit in u32");
    stream
        .write_all(&body_len.to_be_bytes())
        .await
        .expect("client should write request length");
    stream
        .write_all(body)
        .await
        .expect("client should write request body");

    read_response(&mut stream).await
}

async fn read_response(stream: &mut TcpStream) -> Response {
    let mut length_buf = [0u8; 4];
    stream
        .read_exact(&mut length_buf)
        .await
        .expect("client should read response length");

    let response_len = u32::from_be_bytes(length_buf) as usize;
    let mut response_buf = vec![0; response_len];
    stream
        .read_exact(&mut response_buf)
        .await
        .expect("client should read response body");

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
    assert_eq!(response.error_code, None);

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
    assert_eq!(response.error_code, None);

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
    assert_eq!(response.error_code, None);

    let response = send_request(
        server.addr,
        Request {
            method: RequestMethod::Delete,
            key: key.clone(),
            value: None,
        },
    )
    .await;
    assert!(matches!(response.status, ResponseStatus::Ok));
    assert_eq!(response.error_code, None);

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
    assert_eq!(
        response.error_code,
        Some(protocol::ResponseErrorCode::NotFound)
    );

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
    assert_eq!(
        response.error_code,
        Some(protocol::ResponseErrorCode::NotFound)
    );

    server.shutdown().await;
}

#[tokio::test]
async fn returns_error_for_malformed_request() {
    let server = TestServer::start().await;

    let response = send_body(server.addr, b"not messagepack").await;

    assert!(matches!(response.status, ResponseStatus::Error));
    assert_eq!(response.value, None);
    assert_eq!(
        response.error_code,
        Some(protocol::ResponseErrorCode::MalformedRequest)
    );
    assert!(response.message.starts_with("malformed request body:"));

    server.shutdown().await;
}

#[tokio::test]
async fn closes_when_client_disconnects_before_request() {
    let server = TestServer::start().await;

    let stream = TcpStream::connect(server.addr)
        .await
        .expect("client should connect to test server");
    drop(stream);

    server.shutdown().await;
}

#[tokio::test]
async fn closes_when_client_disconnects_during_request_length() {
    let server = TestServer::start().await;

    let mut stream = TcpStream::connect(server.addr)
        .await
        .expect("client should connect to test server");
    stream
        .write_all(&[0])
        .await
        .expect("client should write partial request length");
    drop(stream);

    server.shutdown().await;
}

#[tokio::test]
async fn closes_after_malformed_request_response() {
    let server = TestServer::start().await;

    let mut stream = TcpStream::connect(server.addr)
        .await
        .expect("client should connect to test server");
    let body = b"not messagepack";
    stream
        .write_all(&(body.len() as u32).to_be_bytes())
        .await
        .expect("client should write request length");
    stream
        .write_all(body)
        .await
        .expect("client should write request body");

    let response = read_response(&mut stream).await;
    assert!(matches!(response.status, ResponseStatus::Error));
    assert_eq!(
        response.error_code,
        Some(protocol::ResponseErrorCode::MalformedRequest)
    );

    let mut buf = [0u8; 1];
    let bytes_read = timeout(Duration::from_secs(1), stream.read(&mut buf))
        .await
        .expect("server should close connection after malformed request")
        .expect("client should read connection close");
    assert_eq!(bytes_read, 0);

    server.shutdown().await;
}
