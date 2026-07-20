use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RequestMethod {
    Get = 1,
    Set = 2,
    Delete = 3,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: RequestMethod,
    pub key: String,
    pub value: Option<Vec<u8>>,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResponseStatus {
    Ok = 1,
    Error = 2,
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResponseErrorCode {
    NotFound = 1,
    MalformedRequest = 2,
    InvalidRequest = 3,
    Internal = 4,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: ResponseStatus,
    pub message: String,
    pub value: Option<Vec<u8>>,
    pub error_code: Option<ResponseErrorCode>,
}
