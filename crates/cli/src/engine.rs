//! Extension engine construction helpers.
//!
//! Re-exports [`Executor`] and [`create_engine`] from `quelle_engine` for use
//! within this crate, plus a convenience no-argument wrapper for call sites
//! that do not need to select an executor explicitly.
//!
//! Also provides free async functions [`fetch_novel`] and [`fetch_chapter`] that
//! wire together the store (extension management) and the engine (content execution)
//! without coupling the two crates to each other.

use dashmap::{DashMap, mapref::one::Ref};
use quelle_engine::registry::ExtensionSession;
pub use quelle_engine::{Executor, create_engine as create_extension_engine_with_executor};

/// Create an [`quelle_engine::ExtensionEngine`] using the default executor.
pub fn create_extension_engine() -> eyre::Result<quelle_engine::ExtensionEngine> {
    create_extension_engine_with_executor(Executor::default())
}

pub async fn create_extension_session<'a>(
    engine: &'a quelle_engine::ExtensionEngine,
    store_manager: &mut quelle_store::StoreManager,
    url: &str,
) -> eyre::Result<ExtensionSession<'a>> {
    let installed = store_manager
        .find_and_install_for_url(url)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    let wasm_bytes = store_manager
        .registry_store()
        .get_extension_wasm_bytes(&installed.id)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    let session = ExtensionSession::new(engine, wasm_bytes);

    Ok(session)
}

/// Find/install the right extension for `url`, then run it to fetch novel metadata.
///
/// This replaces the former `StoreManager::fetch_novel` method, keeping extension
/// management (`StoreManager`) and content execution (`ExtensionEngine`) decoupled.
pub async fn fetch_novel(
    extension: &ExtensionSession<'_>,
    url: &str,
) -> eyre::Result<quelle_types::Novel> {
    let result = extension
        .call(async move |runner| {
            runner
                .fetch_novel_info(url)
                .await
                .map_err(|e| eyre::eyre!(e))
        })
        .await?;

    result.map_err(|wit_err| wit_err.into_report())
}

/// Find/install the right extension for `url`, then run it to fetch chapter content.
///
/// This replaces the former `StoreManager::fetch_chapter` method, keeping extension
/// management (`StoreManager`) and content execution (`ExtensionEngine`) decoupled.
pub async fn fetch_chapter(
    extension: &ExtensionSession<'_>,
    url: &str,
) -> eyre::Result<quelle_types::ChapterContent> {
    let result = extension
        .call(async move |runner| {
            let (runner, result) = runner
                .fetch_chapter(url)
                .await
                .map_err(|e| eyre::eyre!("{}", e))?;
            Ok((runner, result))
        })
        .await?;

    result.map_err(|wit_err| wit_err.into_report())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let result = create_extension_engine();
        assert!(result.is_ok(), "Engine creation should succeed");
    }
}

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
