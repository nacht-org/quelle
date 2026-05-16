use std::sync::Arc;

use aide::{
    axum::{ApiRouter, routing::get_with},
    transform::TransformOperation,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use quelle_store::{InstalledExtension, models::ExtensionListing};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{error::ApiResult, state::AppState};

pub fn routes() -> ApiRouter<Arc<AppState>> {
    use aide::axum::routing::{delete_with, post_with};

    ApiRouter::new()
        .api_route("/", get_with(get_extensions, get_extensions_docs))
        .api_route(
            "/{id}/install",
            post_with(install_extension, install_extension_docs),
        )
        .api_route(
            "/{id}/uninstall",
            delete_with(uninstall_extension, uninstall_extension_docs),
        )
        .api_route(
            "/{id}/reinstall",
            post_with(reinstall_extension, reinstall_extension_docs),
        )
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

fn get_extensions_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.id("list_extensions")
        .summary("List extensions")
        .description(
            "Returns all installed extensions and their availability in configured stores.",
        )
        .tag("Extensions")
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InstallExtensionBody {
    /// Specific version to install. If omitted, the latest available version is used.
    version: Option<String>,
}

#[axum::debug_handler]
pub async fn install_extension(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<InstallExtensionBody>>,
) -> ApiResult<Json<InstalledExtension>> {
    let store_manager = state.store_manager.lock().await;

    let (version, options) = match body {
        Some(Json(body)) => {
            let version = body
                .version
                .as_deref()
                .map(|v| v.parse())
                .transpose()
                .map_err(|e| eyre::eyre!("Invalid version: {}", e))?;

            (version, quelle_store::models::InstallOptions::default())
        }
        None => (None, quelle_store::models::InstallOptions::default()),
    };

    let installed = store_manager
        .install(&id, version.as_ref(), Some(options))
        .await
        .map_err(|e| eyre::eyre!(e))?;

    Ok(Json(installed))
}

fn install_extension_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.id("install_extension")
        .summary("Install an extension")
        .description("Installs an extension by ID from the best available store. Optionally specify a version and whether to force reinstall.")
        .tag("Extensions")
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReinstallExtensionBody {
    /// Specific version to reinstall. If omitted, the latest available version is used.
    version: Option<String>,
}

#[axum::debug_handler]
pub async fn reinstall_extension(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<ReinstallExtensionBody>>,
) -> ApiResult<Json<InstalledExtension>> {
    let store_manager = state.store_manager.lock().await;

    let version = match body {
        Some(Json(body)) => body
            .version
            .as_deref()
            .map(|v| v.parse())
            .transpose()
            .map_err(|e| eyre::eyre!("Invalid version: {}", e))?,
        None => None,
    };

    let options = quelle_store::models::InstallOptions {
        force_reinstall: true,
        ..Default::default()
    };

    let installed = store_manager
        .install(&id, version.as_ref(), Some(options))
        .await
        .map_err(|e| eyre::eyre!(e))?;

    Ok(Json(installed))
}

fn reinstall_extension_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.id("reinstall_extension")
        .summary("Reinstall an extension")
        .description("Forces a reinstallation of an extension by ID, even if it is already installed at the requested version. Optionally specify a version; defaults to the latest available.")
        .tag("Extensions")
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UninstallResponse {
    /// Whether the extension was found and removed.
    removed: bool,
}

#[axum::debug_handler]
pub async fn uninstall_extension(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Response> {
    let store_manager = state.store_manager.lock().await;

    let removed = store_manager
        .uninstall(&id)
        .await
        .map_err(|e| eyre::eyre!(e))?;

    let status = if removed {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };

    Ok((status, Json(UninstallResponse { removed })).into_response())
}

fn uninstall_extension_docs(op: TransformOperation<'_>) -> TransformOperation<'_> {
    op.id("uninstall_extension")
        .summary("Uninstall an extension")
        .description(
            "Removes an installed extension by ID. Returns 404 if the extension was not installed.",
        )
        .tag("Extensions")
}
