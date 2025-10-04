//! Utility functions for CLI operations and common functionality.

use eyre::Result;
use quelle_engine::ExtensionEngine;
use quelle_storage::{
    traits::BookStorage,
    types::{NovelFilter, NovelId},
};
use quelle_store::{StoreManager, registry::LocalRegistryStore};
use std::path::PathBuf;
use url::Url;

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

/// Smart novel resolution - accepts URLs, IDs, or fuzzy title matching
pub async fn resolve_novel_id(input: &str, storage: &dyn BookStorage) -> Result<Option<NovelId>> {
    // Handle special case for "all"
    if input == "all" {
        return Ok(None); // Indicates "all novels"
    }

    // Try direct ID first (most efficient)
    let direct_id = NovelId::new(input.to_string());
    if storage.exists_novel(&direct_id).await.unwrap_or(false) {
        return Ok(Some(direct_id));
    }

    // Try URL resolution - let the storage handle URL normalization and lookup
    if let Ok(_url) = Url::parse(input)
        && let Ok(Some(found_novel)) = storage.find_novel_by_url(input).await {
            // We found the novel by URL, now find its ID in the novels list
            let novels = storage.list_novels(&NovelFilter::default()).await?;
            // Match by title and URL to find the correct ID
            if let Some(novel_summary) = novels.iter().find(|n| n.title == found_novel.title) {
                return Ok(Some(novel_summary.id.clone()));
            }
        }

    // Try fuzzy title matching
    let novels = storage.list_novels(&NovelFilter::default()).await?;

    // First try exact title match (case insensitive)
    if let Some(novel) = novels
        .iter()
        .find(|n| n.title.to_lowercase() == input.to_lowercase())
    {
        return Ok(Some(novel.id.clone()));
    }

    // Then try partial title match (case insensitive)
    let input_lower = input.to_lowercase();
    let matches: Vec<_> = novels
        .iter()
        .filter(|n| n.title.to_lowercase().contains(&input_lower))
        .collect();

    match matches.len() {
        0 => Ok(None),                        // No matches found
        1 => Ok(Some(matches[0].id.clone())), // Single match - return it
        _ => {
            // Multiple matches - show them to user and return None
            println!("🔍 Multiple novels found matching '{}':", input);
            for novel in matches.iter().take(10) {
                println!("  {} - {}", novel.id.as_str(), novel.title);
            }
            if matches.len() > 10 {
                println!("  ... and {} more", matches.len() - 10);
            }
            println!("💡 Please be more specific or use the exact ID");
            Ok(None)
        }
    }
}

/// Display helpful message when novel is not found
pub async fn show_novel_not_found_help(input: &str, storage: &dyn BookStorage) {
    println!("❌ Novel not found: '{}'", input);

    // Show some suggestions
    println!("💡 You can identify novels using:");
    println!("   • Novel ID (exact match)");
    println!("   • Novel URL (exact match)");
    println!("   • Novel title (partial match allowed)");
    println!();

    // Show a few examples from the library if available
    if let Ok(novels) = storage.list_novels(&NovelFilter::default()).await {
        if !novels.is_empty() {
            println!("📚 Available novels (showing first 3):");
            for novel in novels.iter().take(3) {
                println!("   {} - {}", novel.id.as_str(), novel.title);
            }
            if novels.len() > 3 {
                println!("   ... and {} more", novels.len() - 3);
            }
            println!("   Use 'quelle library list' to see all novels");
        } else {
            println!("📚 No novels in library yet. Use 'quelle add <url>' to add some!");
        }
    }
}
