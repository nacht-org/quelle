use quelle_domain::registry::ExtensionRegistry;
use quelle_engine::ExtensionEngine;
use quelle_store::StoreManager;

use crate::settings::Settings;

pub struct AppState {
    pub settings: Settings,
    pub engine: ExtensionEngine,
    pub store_manager: StoreManager,
    pub registry: ExtensionRegistry<'static>,
}
