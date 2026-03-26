use std::sync::Arc;

use aide::{
    axum::{ApiRouter, routing::get_with},
    transform::TransformOperation,
};
use axum::{Json, extract::State};
use quelle_store::{InstalledExtension, models::ExtensionListing};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{error::ApiResult, state::AppState};

pub fn routes() -> ApiRouter<Arc<AppState>> {
    ApiRouter::new().api_route("/", get_with(get_extensions, get_extensions_tranform))
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Extensions {
    /// The extensions that are currently installed on this system.
    installed: Vec<InstalledExtension>,
    /// All extensions available across configured stores.
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

fn get_extensions_tranform(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.id("list_extensions")
        .summary("List extensions")
        .description(
            "Returns all installed extensions and their availability in configured stores.",
        )
        .tag("Extensions")
}
