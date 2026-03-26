use std::sync::Arc;

use aide::{
    axum::{ApiRouter, IntoApiResponse},
    openapi::OpenApi,
};
use axum::{Extension, Json};

use crate::state::AppState;

pub fn routes() -> ApiRouter<Arc<AppState>> {
    ApiRouter::new()
        .route("/openapi.json", axum::routing::get(serve_api))
        .route("/docs", axum::routing::get(serve_scalar))
}

async fn serve_api(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
    Json(api)
}

async fn serve_scalar() -> impl IntoApiResponse {
    axum::response::Html(
        r#"
<!doctype html>
<html>
  <head>
    <title>Quelle API Docs</title>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
  </head>
  <body>
    <script
      id="api-reference"
      data-url="/openapi.json"
      data-configuration='{"theme":"purple"}'
    ></script>
    <script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
  </body>
</html>
    "#,
    )
}
