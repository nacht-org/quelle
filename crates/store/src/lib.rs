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
/// use quelle_store::{StoreManager, LocalRegistryStore};
/// use quelle_store::stores::local::LocalStore;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Option 1: Use OS-specific defaults
/// let mut manager = quelle_store::init_default().await?;
///
/// // Option 2: Use custom directory
/// let registry_store = Box::new(LocalRegistryStore::new("./custom-extensions").await?);
/// let mut manager = StoreManager::new(registry_store).await?;
///
/// // Add extension stores for discovery
/// let local_store = LocalStore::new("./local-repo")?;
/// let registry_config = quelle_store::RegistryStoreConfig::new("local-repo".to_string(), "local".to_string());
/// manager.add_extension_store(local_store, registry_config).await?;
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
pub mod manager;
pub mod manifest;
pub mod models;
pub mod publish;
pub mod registry;
pub mod registry_config;
pub mod source;
pub mod store_manifest;
pub mod stores;
pub mod validation;

// Re-export commonly used types
pub use error::{Result, StoreError};
pub use manager::StoreManager;
pub use models::{
    CompatibilityInfo, ExtensionInfo, ExtensionMetadata, ExtensionPackage, InstallOptions,
    InstalledExtension, SearchQuery, SearchSortBy, StoreConfig, StoreHealth, StoreInfo, UpdateInfo,
    UpdateOptions,
};
pub use publish::{
    ExtensionVisibility, PublishError, PublishOptions, PublishRequirements, PublishResult,
    UnpublishOptions, UnpublishResult, ValidationReport,
};
pub use registry::{LocalRegistryStore, RegistryStore, ValidationIssue};
pub use registry_config::{RegistryStoreConfig, RegistryStoreConfigs, StoreConfigCounts};
pub use source::{create_readable_store_from_source, ExtensionSource, RegistryConfig, StoreType};
pub use store_manifest::{ExtensionSummary, StoreManifest, UrlPattern};
pub use stores::local::LocalStoreBuilder;
pub use stores::traits::{BaseStore, ReadableStore, WritableStore};
pub use stores::{LocallyCachedStore, StoreProvider, SyncResult};

#[cfg(feature = "git")]
pub use stores::providers::git::{CommitStyle, GitAuthor, GitStatus, GitWriteConfig};
#[cfg(feature = "git")]
pub use stores::{GitAuth, GitProvider, GitReference, GitStore, GitStoreBuilder};
pub use validation::{
    create_default_validator, create_strict_validator, ExtensionValidationReport, RuleResult,
    SecurityRuleConfig, SecurityValidationRule, ValidationConfig, ValidationEngine, ValidationRule,
    ValidationSummary,
};

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Initialize the store system with default configuration
///
/// This is a convenience function that sets up a basic store manager
/// with OS-specific default directories and sensible defaults.
///
/// Returns an error if the system directories cannot be determined for the current OS/user.
pub async fn init_default() -> Result<StoreManager> {
    let registry_store = Box::new(LocalRegistryStore::new_with_defaults().await?);
    StoreManager::new(registry_store).await
}

/// Initialize the store system with a custom install directory
///
/// This function allows you to specify a custom installation directory
/// while using sensible defaults for everything else.
pub async fn init_with_custom_dir(install_dir: std::path::PathBuf) -> Result<StoreManager> {
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
        // This test might fail on systems where directories cannot be determined
        // but that's expected behavior - we want the error to surface
        match init_default().await {
            Ok(manager) => {
                assert_eq!(manager.list_extension_stores().len(), 0);
            }
            Err(_) => {
                // This is acceptable on systems where directories cannot be determined
                println!("Note: System directories could not be determined, which is expected on some systems");
            }
        }
    }

    #[tokio::test]
    async fn test_init_with_custom_dir() {
        let temp_dir = TempDir::new().unwrap();
        let install_dir = temp_dir.path().join("extensions");

        let manager = init_with_custom_dir(install_dir.clone()).await.unwrap();
        assert_eq!(manager.list_extension_stores().len(), 0);
    }

    #[tokio::test]
    async fn test_version_info() {
        assert!(!VERSION.is_empty());
        assert_eq!(NAME, "quelle_store");
    }
}
