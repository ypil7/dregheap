use rmp_serde::Serializer;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::store::AsyncStore;
use protocol::RequestMethod::{Get, Set};
use protocol::{self, Request, Response};

// Controller
pub async fn handle_request(mut socket: TcpStream, store: AsyncStore) {
    let mut receive_buf = vec![0u8; 4096];
    let bytes_read = match socket.read(&mut receive_buf).await {
        Ok(bytes_read) => bytes_read,
        Err(e) => {
            log::error!("failed reading from socket: {}", e);
            return;
        }
    };

    let response = match rmp_serde::from_slice(&receive_buf[..bytes_read]) {
        Ok(request) => process_request(request, store),
        Err(e) => Response {
            status: protocol::ResponseStatus::Error,
            message: format!("Malformed request: {}", e),
            value: None,
        },
    };

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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::{assert_eq, assert_matches};

    use super::*;
    use crate::store;

    #[test]
    fn set_then_get_returns_value() {
        let store = Arc::new(Mutex::new(store::make_store()));

        let test_key = "test1".to_string();
        let test_value: Option<Vec<u8>> = Some("some random test value".into());

        // Given
        let req = Request {
            method: protocol::RequestMethod::Set,
            key: test_key.clone(),
            value: test_value.clone(),
        };

        // When
        let res = process_request(req, store.clone());

        // Then
        assert_matches!(res.status, protocol::ResponseStatus::Ok);

        // Given, dependent on last request
        let req = Request {
            method: protocol::RequestMethod::Get,
            key: test_key.clone(),
            value: None,
        };

        // When
        let res = process_request(req, store.clone());

        // Then
        assert_matches!(res.status, protocol::ResponseStatus::Ok);
        assert_eq!(res.value, test_value);
    }

    #[test]
    fn get_missing_key_errors() {
        let store = Arc::new(Mutex::new(store::make_store()));

        let test_key = "test2".to_string();
        let test_value: Option<Vec<u8>> = Some("some other random test value".into());

        // Given
        let req = Request {
            method: protocol::RequestMethod::Get,
            key: test_key.clone(),
            value: test_value.clone(),
        };

        // When
        let res = process_request(req, store.clone());

        // Then
        assert_matches!(res.status, protocol::ResponseStatus::Error);
    }

    #[test]
    fn set_with_none_deletes_value() {
        let store = Arc::new(Mutex::new(store::make_store()));

        let test_key = "test3".to_string();
        let test_value: Option<Vec<u8>> = Some("yet another random test value".into());

        // Given
        let req = Request {
            method: protocol::RequestMethod::Set,
            key: test_key.clone(),
            value: test_value.clone(),
        };
        _ = process_request(req, store.clone());

        // And
        let req = Request {
            method: protocol::RequestMethod::Set,
            key: test_key.clone(),
            value: None,
        };
        _ = process_request(req, store.clone());

        // When
        let req = Request {
            method: protocol::RequestMethod::Get,
            key: test_key.clone(),
            value: test_value.clone(),
        };
        let res = process_request(req, store.clone());

        // Then
        assert_matches!(res.status, protocol::ResponseStatus::Error);
    }
}
