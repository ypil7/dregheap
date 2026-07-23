use std::sync::{Arc, Mutex};
use std::{assert_eq, assert_matches};

use protocol::{Request, RequestMethod, ResponseErrorCode, ResponseStatus};

use crate::handler::process_request;
use crate::store;

#[test]
fn set_then_get_returns_value() {
    let store = Arc::new(Mutex::new(store::make_store()));

    let test_key = "test1".to_string();
    let test_value: Option<Vec<u8>> = Some("some random test value".into());

    // Given
    let req = Request {
        method: RequestMethod::Set,
        key: test_key.clone(),
        value: test_value.clone(),
    };

    // When
    let res = process_request(req, store.clone());

    // Then
    assert_matches!(res.status, ResponseStatus::Ok);

    // Given, dependent on last request
    let req = Request {
        method: RequestMethod::Get,
        key: test_key.clone(),
        value: None,
    };

    // When
    let res = process_request(req, store.clone());

    // Then
    assert_matches!(res.status, ResponseStatus::Ok);
    assert_eq!(res.error_code, None);
    assert_eq!(res.value, test_value);
}

#[test]
fn get_missing_key_errors() {
    let store = Arc::new(Mutex::new(store::make_store()));

    let test_key = "test2".to_string();
    let test_value: Option<Vec<u8>> = Some("some other random test value".into());

    // Given
    let req = Request {
        method: RequestMethod::Get,
        key: test_key.clone(),
        value: test_value.clone(),
    };

    // When
    let res = process_request(req, store.clone());

    // Then
    assert_matches!(res.status, ResponseStatus::Error);
    assert_eq!(res.error_code, Some(ResponseErrorCode::NotFound));
}

#[test]
fn delete_removes_value() {
    let store = Arc::new(Mutex::new(store::make_store()));

    let test_key = "test3".to_string();
    let test_value: Option<Vec<u8>> = Some("yet another random test value".into());

    // Given
    let req = Request {
        method: RequestMethod::Set,
        key: test_key.clone(),
        value: test_value.clone(),
    };
    _ = process_request(req, store.clone());

    // And
    let req = Request {
        method: RequestMethod::Delete,
        key: test_key.clone(),
        value: None,
    };
    _ = process_request(req, store.clone());

    // When
    let req = Request {
        method: RequestMethod::Get,
        key: test_key.clone(),
        value: test_value.clone(),
    };
    let res = process_request(req, store.clone());

    // Then
    assert_matches!(res.status, ResponseStatus::Error);
    assert_eq!(res.error_code, Some(ResponseErrorCode::NotFound));
}

#[test]
fn set_with_none_errors() {
    let store = Arc::new(Mutex::new(store::make_store()));

    // Given
    let req = Request {
        method: RequestMethod::Set,
        key: "test4".to_string(),
        value: None,
    };

    // When
    let res = process_request(req, store);

    // Then
    assert_matches!(res.status, ResponseStatus::Error);
    assert_eq!(res.error_code, Some(ResponseErrorCode::InvalidRequest));
}

#[test]
fn set_oversized_value_errors_as_invalid_request() {
    let max_weight = store::CACHE_ENTRY_OVERHEAD + "key".len() + 1;
    let store: store::AsyncStore = Arc::new(Mutex::new(Box::new(store::DregStore::new(Some(
        max_weight,
    )))));

    // Given
    let req = Request {
        method: RequestMethod::Set,
        key: "key".to_string(),
        value: Some(b"too large".to_vec()),
    };

    // When
    let res = process_request(req, store);

    // Then
    assert_matches!(res.status, ResponseStatus::Error);
    assert_eq!(res.error_code, Some(ResponseErrorCode::InvalidRequest));
}
