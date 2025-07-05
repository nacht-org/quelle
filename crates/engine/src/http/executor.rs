use async_trait::async_trait;

use crate::bindings::quelle::extension::http;

#[async_trait]
pub trait HttpExecutor: Send + Sync {
    async fn execute(&self, request: http::Request) -> Result<http::Response, http::ResponseError>;
}
