use rmp_serde::Serializer;
use serde::Serialize;
use std::io::ErrorKind;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Instant, timeout, timeout_at};
use tokio_util::sync::CancellationToken;

use crate::errors::Error;
use crate::store::AsyncStore;
use protocol::RequestMethod::{Delete, Get, Set};
use protocol::{self, Request, Response, ResponseErrorCode, ResponseStatus};

const MAX_REQUEST_BYTES: u32 = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct ConnectionConfig {
    pub read_timeout: Duration,
    pub write_timeout: Duration,
    pub idle_connection_ttl: Duration,
    pub max_connection_lifetime: Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            read_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(10),
            idle_connection_ttl: Duration::from_secs(3 * 60),
            max_connection_lifetime: Duration::from_secs(30 * 60),
        }
    }
}

enum ReadRequestOutcome {
    Request(Request),
    ProtocolError(String),
    Close,
}

// Controller
pub async fn handle_request(
    mut socket: TcpStream,
    cancel_token: CancellationToken,
    store: AsyncStore,
    connection_config: ConnectionConfig,
) {
    let connected_at = Instant::now();
    let mut idle_deadline = connected_at + connection_config.idle_connection_ttl;
    let lifetime_deadline = connected_at + connection_config.max_connection_lifetime;

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                break;
            },
            close = async {
                let read_deadline = Instant::now() + connection_config.read_timeout;
                let request_deadline =
                    earliest_deadline(read_deadline, idle_deadline, lifetime_deadline);

                let response = match timeout_at(request_deadline, read_request(&mut socket)).await {
                    Ok(ReadRequestOutcome::Request(request)) => process_request(request, store.clone()),
                    Ok(ReadRequestOutcome::ProtocolError(e)) => {
                        let response = error_response(ResponseErrorCode::MalformedRequest, e);
                        if let Err(e) =
                            write_response_with_timeout(&mut socket, &response, connection_config.write_timeout).await
                        {
                            log::error!("failed writing response to socket: {}", e);
                        }
                        return true;
                    }
                    Ok(ReadRequestOutcome::Close) => return true,
                    Err(_) => {
                        log_timeout_close(read_deadline, idle_deadline, lifetime_deadline);
                        return true;
                    }
                };

                if let Err(e) = write_response_with_timeout(&mut socket, &response, connection_config.write_timeout).await {
                    log::error!("failed writing response to socket: {}", e);
                    return true;
                }

                if Instant::now() >= lifetime_deadline {
                    log::debug!("closing connection after max connection lifetime ttl");
                    return true;
                }

                idle_deadline = Instant::now() + connection_config.idle_connection_ttl;
                false
            } => {
                if close { break; };
            }
        }
    }
}

fn earliest_deadline(
    read_deadline: Instant,
    idle_deadline: Instant,
    lifetime_deadline: Instant,
) -> Instant {
    read_deadline.min(idle_deadline).min(lifetime_deadline)
}

fn log_timeout_close(read_deadline: Instant, idle_deadline: Instant, lifetime_deadline: Instant) {
    let now = Instant::now();
    if now >= lifetime_deadline {
        log::debug!("closing connection after max connection lifetime ttl");
    } else if now >= idle_deadline {
        log::debug!("closing connection after idle connection ttl");
    } else if now >= read_deadline {
        log::debug!("closing connection after request read timeout");
    } else {
        log::debug!("closing connection after timeout");
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
        .map_err(|e| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                format!("failed serializing response: {}", e),
            )
        })?;

    let response_len = u32::try_from(send_buf.len()).map_err(|_| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!(
                "response is too large: {} bytes exceeds u32 limit",
                send_buf.len()
            ),
        )
    })?;
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

            let Ok(mut store) = store.lock() else {
                return error_response(ResponseErrorCode::Internal, "store lock is poisoned");
            };
            if let Err(e) = store.set(req.key, Some(value)) {
                match e {
                    Error::CacheEntryTooLarge(message) => {
                        error_response(ResponseErrorCode::InvalidRequest, message)
                    }
                    e => error_response(ResponseErrorCode::Internal, e.to_string()),
                }
            } else {
                ok_response("Success", None)
            }
        }
        Get => {
            let Ok(mut store) = store.lock() else {
                return error_response(ResponseErrorCode::Internal, "store lock is poisoned");
            };
            match store.get(&req.key) {
                Ok(val) => ok_response("found value", Some(val)),
                Err(e) => error_response(ResponseErrorCode::NotFound, e.to_string()),
            }
        }
        Delete => {
            let Ok(mut store) = store.lock() else {
                return error_response(ResponseErrorCode::Internal, "store lock is poisoned");
            };
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

    #[test]
    fn process_request_returns_internal_error_when_store_lock_is_poisoned() {
        let store = std::sync::Arc::new(std::sync::Mutex::new(crate::store::make_store()));

        let poison_store = store.clone();
        let _ = std::panic::catch_unwind(move || {
            let _guard = poison_store.lock().unwrap();
            panic!("poison store lock");
        });

        let response = process_request(
            Request {
                method: Get,
                key: "key".to_string(),
                value: None,
            },
            store,
        );

        assert_eq!(response.status, ResponseStatus::Error);
        assert_eq!(response.error_code, Some(ResponseErrorCode::Internal));
    }
}
