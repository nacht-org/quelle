use std::sync::Arc;

use aide::{
    axum::{ApiRouter, routing::get_with},
    transform::TransformOperation,
};
use axum::{
    Json,
    extract::{Query, State},
};
use quelle_types::{ChapterContent, Novel};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{error::ApiResult, state::AppState};

pub fn routes() -> ApiRouter<Arc<AppState>> {
    ApiRouter::new()
        .api_route("/novel", get_with(get_novel, get_novel_docs))
        .api_route("/chapter", get_with(get_chapter, get_chapter_docs))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetNovelQuery {
    url: String,
}

#[axum::debug_handler]
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

pub fn get_novel_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.id("get_novel")
        .summary("Get novel details")
        .description("Returns detailed information about a specific novel.")
        .tag("Fetch")
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetChapterQuery {
    url: String,
}

pub async fn get_chapter(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GetChapterQuery>,
) -> ApiResult<Json<ChapterContent>> {
    let extension_session = state.registry.get_extension(&query.url).await?;

    let chapter_content = extension_session
        .call(async move |extension| {
            extension
                .fetch_chapter(&query.url)
                .await
                .map_err(|e| eyre::eyre!(e))
        })
        .await?
        .map_err(|wit_err| wit_err.into_report())?;

    Ok(Json(chapter_content))
}

pub fn get_chapter_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.id("get_chapter")
        .summary("Get chapter content")
        .description("Returns the content of a specific chapter.")
        .tag("Fetch")
}
