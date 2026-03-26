use std::sync::Arc;

use aide::{
    axum::{ApiRouter, routing::get_with},
    transform::TransformOperation,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use quelle_types::{ChapterContent, Novel, SearchResult};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{error::ApiResult, state::AppState};

pub fn routes() -> ApiRouter<Arc<AppState>> {
    ApiRouter::new()
        .api_route("/novel", get_with(get_novel, get_novel_docs))
        .api_route("/chapter", get_with(get_chapter, get_chapter_docs))
        .api_route(
            "/search/{extension_id}",
            get_with(simple_search, simple_search_docs),
        )
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

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SimpleSearchQuery {
    query: String,
    page: Option<u32>,
    limit: Option<u32>,
}

pub async fn simple_search(
    State(state): State<Arc<AppState>>,
    Path(extension_id): Path<String>,
    Query(query): Query<SimpleSearchQuery>,
) -> ApiResult<Json<SearchResult>> {
    let extension_session = state.registry.get_extension_by_id(&extension_id).await?;

    let params = quelle_engine::bindings::SimpleSearchQuery {
        query: query.query,
        page: query.page,
        limit: query.limit,
    };

    let results = extension_session
        .call(async move |extension| {
            extension
                .simple_search(&params)
                .await
                .map_err(|e| eyre::eyre!(e))
        })
        .await?
        .map_err(|wit_err| wit_err.into_report())?;

    Ok(Json(results))
}

pub fn simple_search_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.id("simple_search")
        .summary("Simple search for novels")
        .description("Performs a simple search for novels based on a query string.")
        .tag("Fetch")
}
