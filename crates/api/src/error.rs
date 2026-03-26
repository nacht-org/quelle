use aide::OperationOutput;
use axum::{http::StatusCode, response::IntoResponse};
use schemars::JsonSchema;
use serde::Serialize;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Internal error: {0}")]
    InternalError(#[from] eyre::Report),
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ErrorBody {
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = error_to_response(&self);
        (status, axum::Json(body)).into_response()
    }
}

fn error_to_response(error: &ApiError) -> (StatusCode, ErrorBody) {
    match error {
        ApiError::InternalError(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorBody {
                message: "An internal error occurred".to_string(),
            },
        ),
    }
}

impl OperationOutput for ApiError {
    type Inner = ErrorBody;
}
