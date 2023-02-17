use std::num::ParseIntError;

use serde::{Deserialize, Serialize};

use crate::http::BoxedRequestError;

#[derive(Serialize, Deserialize, thiserror::Error, Debug)]
pub enum FensterError {
    #[error("{0}")]
    RequestFailed(#[from] BoxedRequestError),

    #[error("{0}")]
    ParseFailed(#[from] ParseError),
}

#[derive(Serialize, Deserialize, thiserror::Error, Debug)]
pub enum ParseError {
    #[error("required element not found")]
    ElementNotFound,

    #[error("failed to serialize html tree to string")]
    SerializeFailed,

    #[error("failed to parse url")]
    FailedURLParse,

    #[error("failed to parse int from str")]
    ParseIntError,
}

impl From<ParseIntError> for FensterError {
    fn from(_: ParseIntError) -> Self {
        FensterError::ParseFailed(ParseError::ParseIntError)
    }
}