use std::sync::Arc;

use aide::axum::{ApiRouter, routing::get_with};
use axum::{
    Json,
    extract::{Query, State},
};
use quelle_types::Novel;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{error::ApiResult, state::AppState};

pub fn routes() -> ApiRouter<Arc<AppState>> {
    ApiRouter::new().api_route("/", get_with(get_novel, get_novel_docs))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNovelQuery {
    url: String,
}

pub async fn get_novel(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GetNovelQuery>,
) -> ApiResult<Json<Novel>> {
    let extension_session = state.registry.get_extension(&query.url).await?;

    let novel = extension_session
        .call(async move |extension| {
            extension
                .fetch_novel_info(&query.url)
                .await
                .map_err(|e| eyre::eyre!(e))
        })
        .await?
        .map_err(|wit_err| wit_err.into_report())?;

    Ok(Json(novel))
}

pub fn get_novel_docs(
    op: aide::transform::TransformOperation,
) -> aide::transform::TransformOperation {
    op.id("get_novel")
        .summary("Get novel details")
        .description("Returns detailed information about a specific novel.")
        .tag("Novels")
}
