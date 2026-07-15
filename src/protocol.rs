use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::errors::Result;

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone)]
#[repr(u8)]
pub enum RequestMethod {
    Get = 1,
    Set = 2,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Request {
    method: RequestMethod,
    key: String,
    value: Option<Vec<u8>>,
}

impl Request {
    pub fn validate(&self) -> Result<()> {
        // Check that value is Some() if method is Set and None() if method is Get
        todo!()
    }
}

#[derive(Serialize_repr, Deserialize_repr, Debug, Clone)]
#[repr(u8)]
pub enum ResponseStatus {
    Ok = 1,
    Error = 2,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    pub status: ResponseStatus,
    pub message: String,
    pub value: Option<Vec<u8>>,
}
