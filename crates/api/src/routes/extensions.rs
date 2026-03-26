use aide::axum::{ApiRouter, routing::get};
use axum::Json;
use quelle_store::{InstalledExtension, models::ExtensionListing};
use serde::Serialize;

use crate::error::ApiResult;

pub async fn router() -> ApiRouter {
    ApiRouter::new().api_route("/extensions", get(get_extensions))
}

#[derive(Debug, Serialize)]
pub struct Extensions {
    installed: Vec<InstalledExtension>,
    listing: Vec<ExtensionListing>,
}

#[axum::debug_handler]
pub async fn get_extensions() -> ApiResult<Json<Vec<Extensions>>> {
    Ok(Json(vec![Extensions {
        installed: vec![],
        listing: vec![],
    }]))
}
