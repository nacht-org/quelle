use std::sync::Arc;

use quelle_domain::registry::ExtensionRegistry;
use quelle_engine::ExtensionEngine;
use quelle_store::StoreManager;
use tokio::sync::Mutex;

use crate::settings::Settings;

pub struct AppState {
    pub settings: Settings,
    pub engine: Arc<ExtensionEngine>,
    pub store_manager: Arc<Mutex<StoreManager>>,
    pub registry: ExtensionRegistry,
}
