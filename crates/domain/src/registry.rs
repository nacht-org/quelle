use std::sync::Arc;

use dashmap::{DashMap, mapref::one::Ref};
use quelle_engine::{ExtensionEngine, registry::ExtensionSession};
use quelle_store::StoreManager;
use tokio::sync::Mutex;

/// A registry that manages extension sessions, keyed by extension id. It uses the store manager to find and install extensions as needed, and caches sessions for reuse.
pub struct ExtensionRegistry {
    engine: Arc<ExtensionEngine>,
    store_manager: Arc<Mutex<StoreManager>>,
    extensions: DashMap<String, ExtensionSession>,
}

impl ExtensionRegistry {
    pub fn new(engine: Arc<ExtensionEngine>, store_manager: Arc<Mutex<StoreManager>>) -> Self {
        Self {
            engine,
            store_manager,
            extensions: DashMap::new(),
        }
    }

    pub async fn get_extension(
        &self,
        url: &str,
    ) -> eyre::Result<Ref<'_, String, ExtensionSession>> {
        let installed = {
            let mut store_manager = self.store_manager.lock().await;
            store_manager
                .find_and_install_for_url(url)
                .await
                .map_err(|e| eyre::eyre!("{}", e))?
        };

        if !self.extensions.contains_key(&installed.id) {
            let wasm_bytes = {
                let store_manager = self.store_manager.lock().await;
                store_manager
                    .registry_store()
                    .get_extension_wasm_bytes(&installed.id)
                    .await
                    .map_err(|e| eyre::eyre!(e))?
            };

            let session = ExtensionSession::new(Arc::clone(&self.engine), wasm_bytes);
            self.extensions.insert(installed.id.clone(), session);
        }

        let extension = self
            .extensions
            .get(&installed.id)
            .ok_or_else(|| eyre::eyre!("Failed to retrieve extension session for URL: {}", url))?;

        Ok(extension)
    }
}
