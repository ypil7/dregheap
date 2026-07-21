use rmp_serde::Serializer;
use serde::Serialize;
use std::io::ErrorKind;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::store::AsyncStore;
use protocol::RequestMethod::{Delete, Get, Set};
use protocol::{self, Request, Response, ResponseErrorCode, ResponseStatus};

const MAX_REQUEST_BYTES: u32 = 1024 * 1024;

enum ReadRequestOutcome {
    Request(Request),
    ProtocolError(String),
    Close,
}

// Controller
pub async fn handle_request(
    mut socket: TcpStream,
    store: AsyncStore,
    read_timeout: Duration,
    write_timeout: Duration,
) {
    loop {
        let response = match read_request_with_timeout(&mut socket, read_timeout).await {
            ReadRequestOutcome::Request(request) => process_request(request, store.clone()),
            ReadRequestOutcome::ProtocolError(e) => {
                let response = error_response(ResponseErrorCode::MalformedRequest, e);
                if let Err(e) =
                    write_response_with_timeout(&mut socket, &response, write_timeout).await
                {
                    log::error!("failed writing response to socket: {}", e);
                }
                break;
            }
            ReadRequestOutcome::Close => break,
        };

        if let Err(e) = write_response_with_timeout(&mut socket, &response, write_timeout).await {
            log::error!("failed writing response to socket: {}", e);
            break;
        }
    }
}

async fn read_request_with_timeout(
    socket: &mut TcpStream,
    read_timeout: Duration,
) -> ReadRequestOutcome {
    match timeout(read_timeout, read_request(socket)).await {
        Ok(outcome) => outcome,
        Err(_) => {
            log::debug!("closing connection after request read timeout");
            ReadRequestOutcome::Close
        }
    }
}

async fn read_request(socket: &mut TcpStream) -> ReadRequestOutcome {
    let mut length_buf = [0u8; 4];
    if should_close(socket.read_exact(&mut length_buf).await) {
        return ReadRequestOutcome::Close;
    }

    let request_len = u32::from_be_bytes(length_buf);
    if request_len > MAX_REQUEST_BYTES {
        return ReadRequestOutcome::ProtocolError(format!(
            "request is too large: {} bytes exceeds {} byte limit",
            request_len, MAX_REQUEST_BYTES
        ));
    }

    let mut receive_buf = vec![0; request_len as usize];
    if should_close(socket.read_exact(&mut receive_buf).await) {
        return ReadRequestOutcome::Close;
    }

    match rmp_serde::from_slice(&receive_buf) {
        Ok(request) => ReadRequestOutcome::Request(request),
        Err(e) => ReadRequestOutcome::ProtocolError(format!("malformed request body: {}", e)),
    }
}

fn should_close(read_result: std::io::Result<usize>) -> bool {
    match read_result {
        Ok(_) => false,
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => true,
        Err(e) => {
            log::debug!("closing connection after read error: {}", e);
            true
        }
    }
}

async fn write_response_with_timeout(
    socket: &mut (impl AsyncWrite + Unpin),
    response: &Response,
    write_timeout: Duration,
) -> std::io::Result<()> {
    match timeout(write_timeout, write_response(socket, response)).await {
        Ok(result) => result,
        Err(_) => Err(std::io::Error::new(
            ErrorKind::TimedOut,
            "response write timed out",
        )),
    }
}

async fn write_response(
    socket: &mut (impl AsyncWrite + Unpin),
    response: &Response,
) -> std::io::Result<()> {
    let mut send_buf = Vec::<u8>::new();
    response
        .serialize(&mut Serializer::new(&mut send_buf))
        .unwrap();

    let response_len =
        u32::try_from(send_buf.len()).expect("serialized response should fit in u32");
    socket.write_all(&response_len.to_be_bytes()).await?;
    socket.write_all(&send_buf).await
}

// Service
pub fn process_request(req: Request, store: AsyncStore) -> Response {
    match req.method {
        Set => {
            let Some(value) = req.value else {
                return error_response(
                    ResponseErrorCode::InvalidRequest,
                    "Set requests must include a value",
                );
            };

            let mut store = store.lock().unwrap();
            if let Err(e) = store.set(req.key, Some(value)) {
                error_response(ResponseErrorCode::Internal, e.to_string())
            } else {
                ok_response("Success", None)
            }
        }
        Get => {
            let store = store.lock().unwrap();
            match store.get(&req.key) {
                Ok(val) => ok_response("found value", Some(val)),
                Err(e) => error_response(ResponseErrorCode::NotFound, e.to_string()),
            }
        }
        Delete => {
            let mut store = store.lock().unwrap();
            match store.set(req.key, None) {
                Ok(()) => ok_response("Deleted", None),
                Err(e) => error_response(ResponseErrorCode::Internal, e.to_string()),
            }
        }
    }
}

fn ok_response(message: impl Into<String>, value: Option<Vec<u8>>) -> Response {
    Response {
        status: ResponseStatus::Ok,
        message: message.into(),
        value,
        error_code: None,
    }
}

fn error_response(code: ResponseErrorCode, message: impl Into<String>) -> Response {
    Response {
        status: ResponseStatus::Error,
        message: message.into(),
        value: None,
        error_code: Some(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    struct PendingWriter;

    impl AsyncWrite for PendingWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Pending
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn write_response_times_out_when_writer_stalls() {
        let mut writer = PendingWriter;
        let response = ok_response("Success", None);

        let err = write_response_with_timeout(&mut writer, &response, Duration::from_millis(1))
            .await
            .expect_err("stalled writer should time out");

        assert_eq!(err.kind(), ErrorKind::TimedOut);
    }
}
