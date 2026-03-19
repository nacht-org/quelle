//! Extension engine construction helpers.
//!
//! Re-exports [`Executor`] and [`create_engine`] from `quelle_engine` for use
//! within this crate, plus a convenience no-argument wrapper for call sites
//! that do not need to select an executor explicitly.
//!
//! Also provides free async functions [`fetch_novel`] and [`fetch_chapter`] that
//! wire together the store (extension management) and the engine (content execution)
//! without coupling the two crates to each other.

pub use quelle_engine::{Executor, create_engine as create_extension_engine_with_executor};

/// Create an [`quelle_engine::ExtensionEngine`] using the default executor.
pub fn create_extension_engine() -> eyre::Result<quelle_engine::ExtensionEngine> {
    create_extension_engine_with_executor(Executor::default())
}

/// Find/install the right extension for `url`, then run it to fetch novel metadata.
///
/// This replaces the former `StoreManager::fetch_novel` method, keeping extension
/// management (`StoreManager`) and content execution (`ExtensionEngine`) decoupled.
pub async fn fetch_novel(
    engine: &quelle_engine::ExtensionEngine,
    store_manager: &mut quelle_store::StoreManager,
    url: &str,
) -> eyre::Result<quelle_types::Novel> {
    let installed = store_manager
        .find_and_install_for_url(url)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    let wasm_bytes = store_manager
        .registry_store()
        .get_extension_wasm_bytes(&installed.id)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    let runner = engine
        .new_runner_from_bytes(&wasm_bytes)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    let (_, result) = runner
        .fetch_novel_info(url)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    result.map_err(|wit_err| {
        let chain = wit_err
            .frames
            .iter()
            .map(|f| f.message.as_str())
            .collect::<Vec<_>>()
            .join(": ");
        eyre::eyre!("Extension error: {}", chain)
    })
}

/// Find/install the right extension for `url`, then run it to fetch chapter content.
///
/// This replaces the former `StoreManager::fetch_chapter` method, keeping extension
/// management (`StoreManager`) and content execution (`ExtensionEngine`) decoupled.
pub async fn fetch_chapter(
    engine: &quelle_engine::ExtensionEngine,
    store_manager: &mut quelle_store::StoreManager,
    url: &str,
) -> eyre::Result<quelle_types::ChapterContent> {
    let installed = store_manager
        .find_and_install_for_url(url)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    let wasm_bytes = store_manager
        .registry_store()
        .get_extension_wasm_bytes(&installed.id)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    let runner = engine
        .new_runner_from_bytes(&wasm_bytes)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    let (_, result) = runner
        .fetch_chapter(url)
        .await
        .map_err(|e| eyre::eyre!("{}", e))?;

    result.map_err(|wit_err| {
        let chain = wit_err
            .frames
            .iter()
            .map(|f| f.message.as_str())
            .collect::<Vec<_>>()
            .join(": ");
        eyre::eyre!("Extension error: {}", chain)
    })
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
