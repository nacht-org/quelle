use eyre::Result;
use quelle_engine::ExtensionEngine;
use quelle_store::{StoreManager, registry::LocalRegistryStore};
use std::path::PathBuf;

use crate::config::get_default_data_dir;

/// Create a store manager with the default storage location
pub async fn create_store_manager() -> Result<StoreManager> {
    let storage_path = get_default_data_dir().join("library");
    create_store_manager_with_path(storage_path).await
}

/// Create a store manager with a custom storage path
pub async fn create_store_manager_with_path(storage_path: PathBuf) -> Result<StoreManager> {
    let registry_dir = storage_path.join("extensions");
    let registry_store = Box::new(LocalRegistryStore::new(&registry_dir).await?);
    StoreManager::new(registry_store)
        .await
        .map_err(eyre::Report::from)
}

/// Create an extension engine with Chrome executor (fallback to Reqwest if Chrome fails)
pub fn create_extension_engine() -> Result<ExtensionEngine> {
    create_extension_engine_with_executor_choice(true)
}

/// Create an extension engine with Reqwest executor
#[allow(dead_code)]
pub fn create_extension_engine_reqwest() -> Result<ExtensionEngine> {
    create_extension_engine_with_executor_choice(false)
}

/// Create an extension engine with choice of executor
pub fn create_extension_engine_with_executor_choice(
    prefer_chrome: bool,
) -> Result<ExtensionEngine> {
    if prefer_chrome {
        // Try Chrome first, fallback to Reqwest if it fails
        match try_create_chrome_engine() {
            Ok(engine) => {
                tracing::info!("Using HeadlessChrome executor for extensions");
                Ok(engine)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to create Chrome executor, falling back to Reqwest: {}",
                    e
                );
                create_reqwest_engine()
            }
        }
    } else {
        create_reqwest_engine()
    }
}

/// Try to create engine with Chrome executor
fn try_create_chrome_engine() -> Result<ExtensionEngine> {
    let http_executor = std::sync::Arc::new(quelle_engine::http::HeadlessChromeExecutor::new());
    ExtensionEngine::new(http_executor).map_err(eyre::Report::from)
}

/// Create engine with Reqwest executor
fn create_reqwest_engine() -> Result<ExtensionEngine> {
    tracing::info!("Using Reqwest executor for extensions");
    let http_executor = std::sync::Arc::new(quelle_engine::http::ReqwestExecutor::new());
    ExtensionEngine::new(http_executor).map_err(eyre::Report::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage_path = temp_dir.path().to_path_buf();

        let manager = create_store_manager_with_path(storage_path).await;
        assert!(manager.is_ok());
    }

    #[test]
    fn test_engine_creation() {
        let engine = create_extension_engine();
        assert!(engine.is_ok());
    }

    #[test]
    fn test_config_storage_path() {
        use crate::config::Config;

        let config = Config::default();
        let storage_path = config.get_storage_path();
        let path_str = storage_path.to_string_lossy();
        assert!(path_str.contains("quelle"));
        assert!(path_str.ends_with("library"));
    }
}
