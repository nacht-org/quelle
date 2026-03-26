pub mod extensions;

use axum::http::StatusCode;

pub async fn health() -> axum::http::StatusCode {
    StatusCode::OK
}
