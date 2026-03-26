use std::sync::Arc;

use aide::axum::{ApiRouter, routing::get};
use axum::{Json, extract::State};
use quelle_store::{InstalledExtension, models::ExtensionListing};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{error::ApiResult, state::AppState};

pub fn router() -> ApiRouter<Arc<AppState>> {
    ApiRouter::new().api_route("/", get(get_extensions))
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Extensions {
    installed: Vec<InstalledExtension>,
    listing: Vec<ExtensionListing>,
}

#[axum::debug_handler]
pub async fn get_extensions(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<Extensions>>> {
    let (installed, listing) = {
        let store_manager = state.store_manager.lock().await;

        let installed = store_manager
            .registry_store()
            .list_installed()
            .await
            .map_err(|e| eyre::eyre!(e))?;

        let listing = store_manager
            .list_all_extensions()
            .await
            .map_err(|e| eyre::eyre!(e))?;

        (installed, listing)
    };

    Ok(Json(vec![Extensions { installed, listing }]))
}
