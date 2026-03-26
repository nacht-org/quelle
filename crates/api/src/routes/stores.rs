use std::{collections::HashMap, sync::Arc};

use aide::axum::{ApiRouter, routing::get_with};
use axum::{Json, extract::State};
use quelle_types::Timestamp;
use schemars::JsonSchema;
use serde::Serialize;

use crate::{error::ApiResult, state::AppState};

pub fn routes() -> ApiRouter<Arc<AppState>> {
    ApiRouter::new().api_route("/", get_with(get_stores, get_stores_docs))
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StoreInfo {
    /// Store identifier (must match the store's internal name)
    pub store_name: String,

    /// Registry-assigned store type identifier
    pub store_type: String,

    /// Connection URL or path to the store
    pub url: Option<String>,

    /// Human-readable description
    pub description: Option<String>,

    /// Priority for store ordering (higher = more preferred)
    pub priority: u32,

    /// Whether this source is trusted by the registry
    pub trusted: bool,

    /// Whether this source is enabled for operations
    pub enabled: bool,

    /// When this source configuration was created in the registry
    pub created_at: Timestamp,

    /// Last time this source was successfully accessed
    pub last_accessed: Option<Timestamp>,

    /// Additional configuration specific to the store type
    pub config: HashMap<String, serde_json::Value>,
}

#[axum::debug_handler]
async fn get_stores(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<StoreInfo>>> {
    let store_manager = state.store_manager.lock().await;
    let stores = store_manager.list_extension_stores();
    let stores = stores
        .into_iter()
        .map(|store| {
            let config = store.config();
            StoreInfo {
                store_name: config.store_name.clone(),
                store_type: config.store_type.clone(),
                url: config.url.clone(),
                description: config.description.clone(),
                priority: config.priority,
                trusted: config.trusted,
                enabled: config.enabled,
                created_at: config.created_at,
                last_accessed: config.last_accessed,
                config: config.config.clone(),
            }
        })
        .collect();

    Ok(Json(stores))
}

fn get_stores_docs(
    op: aide::transform::TransformOperation<'_>,
) -> aide::transform::TransformOperation<'_> {
    op.id("list_stores")
        .summary("List extension stores")
        .description(
            "Returns all configured extension stores, including their type, URL, and other metadata.",
        )
        .tag("Stores")
}
