pub mod docs;
pub mod extensions;

use aide::{
    axum::{ApiRouter, routing::get_with},
    transform::TransformOperation,
};
use axum::http::StatusCode;
use schemars::JsonSchema;
use serde::Serialize;

pub fn routes<S>() -> ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    ApiRouter::new().api_route("/health", get_with(health, health_docs))
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(example = HealthResponse::ok())]
pub struct HealthResponse {
    status: HealthStatus,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
}

impl HealthResponse {
    pub fn ok() -> Self {
        Self {
            status: HealthStatus::Ok,
        }
    }
}

pub async fn health() -> (StatusCode, axum::Json<HealthResponse>) {
    (StatusCode::OK, axum::Json(HealthResponse::ok()))
}

fn health_docs(op: TransformOperation) -> TransformOperation {
    op.id("health_check")
        .summary("Health check")
        .description("Returns 200 OK when the service is up and running.")
        .tag("System")
        .response::<200, axum::Json<HealthResponse>>()
}
