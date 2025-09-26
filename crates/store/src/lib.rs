//! Quelle Store - Extension package management and repository system
//!
//! This crate provides a comprehensive store system for managing extensions in the Quelle
//! e-book scraper. It supports multiple store backends including local file systems,
//! Git repositories, and HTTP-based registries.
//!
//! # Features
//!
//! - **Multiple Store Types**: Local, Git, HTTP, and S3 backends
//! - **Package Management**: Install, update, and remove extensions
//! - **Version Management**: Semantic versioning support with dependency resolution
//! - **Search and Discovery**: Find extensions across multiple stores
//! - **Caching**: Efficient caching for better performance
//! - **Security**: Checksum verification and optional signature validation
//!
//! # Examples
//!
//! ## Basic Usage
//!
/// ```rust
/// use quelle_store::{StoreManager, LocalRegistryStore, local::LocalStore};
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a registry store (manages installation state)
/// let registry_store: Box<dyn quelle_store::RegistryStore> =
///     Box::new(LocalRegistryStore::new("./extensions").await?);
///
/// // Create a store manager
/// let mut manager = StoreManager::new(registry_store).await?;
///
/// // Add a local extension store (for discovery)
/// let local_store = LocalStore::new("./local-repo")?;
/// manager.add_extension_store(local_store);
///
/// // Install an extension
/// let installed = manager.install("dragontea", None, None).await?;
/// println!("Installed: {}@{}", installed.name, installed.version);
/// # Ok(())
/// # }
/// ```
///
/// ## Search and Discovery
///
/// ```rust
/// use quelle_store::{StoreManager, SearchQuery, SearchSortBy};
///
/// # async fn search_example(manager: &StoreManager) -> Result<(), Box<dyn std::error::Error>> {
/// // Search for novel scrapers
/// let query = SearchQuery::new()
///     .with_text("novel".to_string())
///     .with_tags(vec!["scraper".to_string()])
///     .sort_by(SearchSortBy::Relevance)
///     .limit(10);
///
/// let results = manager.search_all_stores(&query).await?;
/// for ext in results {
///     println!("Found: {} by {} - {}", ext.name, ext.author, ext.description.unwrap_or_default());
/// }
/// # Ok(())
/// # }
/// ```
pub mod error;
pub mod local;
pub mod manager;
pub mod manifest;
pub mod models;
pub mod registry;
pub mod store;

// Additional store implementations will be added as separate modules:
// - git.rs for Git repository stores
// - http.rs for HTTP-based stores
// - s3.rs for S3 bucket stores

// Re-export commonly used types
pub use error::{Result, StoreError};
pub use manager::StoreManager;
pub use models::{
    CompatibilityInfo, ExtensionDependency, ExtensionInfo, ExtensionMetadata, ExtensionPackage,
    InstallOptions, InstalledExtension, PackageLayout, SearchQuery, SearchSortBy, StoreConfig,
    StoreHealth, StoreInfo, UpdateInfo, UpdateOptions,
};
pub use registry::{
    InstallationQuery, InstallationStats, LocalRegistryStore, RegistryStore, ValidationIssue,
};
pub use store::{capabilities, Store};

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Initialize the store system with default configuration
///
/// This is a convenience function that sets up a basic store manager
/// with sensible defaults for most use cases.
pub async fn init_default(install_dir: std::path::PathBuf) -> Result<StoreManager> {
    let registry_store = Box::new(LocalRegistryStore::new(install_dir).await?);
    StoreManager::new(registry_store).await
}

/// Create a store manager with custom configuration
pub async fn init_with_config(
    registry_store: Box<dyn RegistryStore>,
    config: StoreConfig,
) -> Result<StoreManager> {
    StoreManager::with_config(registry_store, config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_init_default() {
        let temp_dir = TempDir::new().unwrap();
        let install_dir = temp_dir.path().join("extensions");

        let manager = init_default(install_dir.clone()).await.unwrap();

        assert!(manager.install_dir().exists());
        assert_eq!(manager.list_extension_stores().len(), 0);
    }

    #[tokio::test]
    async fn test_version_info() {
        assert!(!VERSION.is_empty());
        assert_eq!(NAME, "quelle_store");
    }
}
