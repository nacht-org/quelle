use dashmap::{DashMap, mapref::one::Ref};
use quelle_engine::registry::ExtensionSession;

/// A registry that manages extension sessions, keyed by extension id. It uses the store manager to find and install extensions as needed, and caches sessions for reuse.
pub struct ExtensionRegistry<'a> {
    engine: &'a quelle_engine::ExtensionEngine,
    store_manager: &'a mut quelle_store::StoreManager,
    extensions: DashMap<String, ExtensionSession<'a>>,
}

impl<'a> ExtensionRegistry<'a> {
    pub fn new(
        engine: &'a quelle_engine::ExtensionEngine,
        store_manager: &'a mut quelle_store::StoreManager,
    ) -> Self {
        Self {
            engine,
            store_manager,
            extensions: DashMap::new(),
        }
    }

    pub async fn get_extension(
        &mut self,
        url: &str,
    ) -> eyre::Result<Ref<'_, String, ExtensionSession<'a>>> {
        let installed = self
            .store_manager
            .find_and_install_for_url(url)
            .await
            .map_err(|e| eyre::eyre!("{}", e))?;

        if !self.extensions.contains_key(&installed.id) {
            let wasm_bytes = self
                .store_manager
                .registry_store()
                .get_extension_wasm_bytes(&installed.id)
                .await
                .map_err(|e| eyre::eyre!(e))?;

            let session = ExtensionSession::new(self.engine, wasm_bytes);
            self.extensions.insert(installed.id, session);
        }

        let extension = self
            .extensions
            .get(url)
            .ok_or_else(|| eyre::eyre!("Failed to retrieve extension session for URL: {}", url))?;

        Ok(extension)
    }
}
