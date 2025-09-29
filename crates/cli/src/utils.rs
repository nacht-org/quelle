use directories::ProjectDirs;
use eyre::Result;
use quelle_engine::ExtensionEngine;
use quelle_store::{ConfigStore, LocalConfigStore, StoreManager, registry::LocalRegistryStore};
use std::path::PathBuf;

use crate::config::Config;

/// Create a store manager with the default storage location
pub async fn create_store_manager() -> Result<StoreManager> {
    let storage_path = get_default_storage_path();
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

/// Create a config store for registry configuration persistence
pub async fn create_config_store() -> Result<Box<dyn ConfigStore>> {
    let config_file = Config::get_config_path();
    Ok(Box::new(LocalConfigStore::new(config_file).await?))
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

/// Get the default storage path for Quelle data
pub fn get_default_storage_path() -> PathBuf {
    Config::get_data_dir().join("library")
}

/// Get storage path from CLI arguments, config, or use default
pub fn get_storage_path_from_args(storage_path_arg: Option<&String>, config: &Config) -> PathBuf {
    match storage_path_arg {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(&config.storage.path),
    }
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

    #[tokio::test]
    async fn test_config_store_creation() {
        let config_store = create_config_store().await;
        assert!(config_store.is_ok());
    }

    #[test]
    fn test_storage_path_helpers() {
        use crate::config::Config;

        let default_path = get_default_storage_path();
        let path_str = default_path.to_string_lossy();
        assert!(path_str.contains("quelle"));

        let config = Config::default();
        let custom_path = "/custom/path";
        let resolved_path = get_storage_path_from_args(Some(&custom_path.to_string()), &config);
        assert_eq!(resolved_path, PathBuf::from(custom_path));

        let default_resolved = get_storage_path_from_args(None, &config);
        assert_eq!(default_resolved, PathBuf::from(&config.storage.path));
    }
}
