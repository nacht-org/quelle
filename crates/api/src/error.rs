use aide::{
    OperationOutput,
    generate::GenContext,
    openapi::{Operation, Response, StatusCode},
};
use axum::{http::StatusCode as HttpStatusCode, response::IntoResponse};
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
    #[schemars(example = "An internal error occurred")]
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = error_to_response(&self);
        (status, axum::Json(body)).into_response()
    }
}

fn error_to_response(error: &ApiError) -> (HttpStatusCode, ErrorBody) {
    match error {
        ApiError::InternalError(_) => (
            HttpStatusCode::INTERNAL_SERVER_ERROR,
            ErrorBody {
                message: "An internal error occurred".to_string(),
            },
        ),
    }
}

impl OperationOutput for ApiError {
    type Inner = ErrorBody;

    fn operation_response(ctx: &mut GenContext, operation: &mut Operation) -> Option<Response> {
        // Reuse the Json<ErrorBody> implementation so we get the correct
        // application/json content block without depending on indexmap directly.
        let mut res = axum::Json::<ErrorBody>::operation_response(ctx, operation)?;

        res.description = "An internal server error occurred.".to_string();

        // Inject a concrete example into every content entry that was produced.
        let example_value = serde_json::json!({ "message": "An internal error occurred" });
        for media in res.content.values_mut() {
            media.example = Some(example_value.clone());
        }

        Some(res)
    }

    fn inferred_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<StatusCode>, Response)> {
        match Self::operation_response(ctx, operation) {
            Some(res) => vec![(Some(StatusCode::Code(500)), res)],
            None => vec![],
        }
    }
}
