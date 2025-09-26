use std::path::Path;

use async_trait::async_trait;

use crate::error::Result;
use crate::manifest::ExtensionManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, InstallOptions, InstalledExtension,
    PackageLayout, SearchQuery, StoreHealth, StoreInfo, UpdateInfo, UpdateOptions,
};

/// Core trait defining the interface for all store implementations
#[async_trait]
pub trait Store: Send + Sync {
    /// Get information about this store
    fn store_info(&self) -> &StoreInfo;

    /// Get the package layout used by this store
    fn package_layout(&self) -> &PackageLayout;

    /// Check the health status of this store
    async fn health_check(&self) -> Result<StoreHealth>;

    // Discovery Operations

    /// List all available extensions in this store
    async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>>;

    /// Search for extensions matching the given query
    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionInfo>>;

    /// Get information about all versions of a specific extension
    async fn get_extension_info(&self, name: &str) -> Result<Vec<ExtensionInfo>>;

    /// Get information about a specific version of an extension
    async fn get_extension_version_info(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo>;

    // Manifest and Metadata Operations

    /// Get the manifest for a specific extension version
    async fn get_manifest(&self, name: &str, version: Option<&str>) -> Result<ExtensionManifest>;

    /// Get the metadata for a specific extension version
    async fn get_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>>;

    // Package Operations

    /// Get the raw WASM bytes for an extension
    async fn get_extension_wasm(&self, name: &str, version: Option<&str>) -> Result<Vec<u8>>;

    /// Get the complete extension package including all files
    async fn get_extension_package(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage>;

    /// Install an extension to the specified directory
    async fn install_extension(
        &self,
        name: &str,
        version: Option<&str>,
        target_dir: &Path,
        options: &InstallOptions,
    ) -> Result<InstalledExtension>;

    // Update Operations

    /// Check for updates for the given installed extensions
    async fn check_updates(&self, installed: &[InstalledExtension]) -> Result<Vec<UpdateInfo>>;

    /// Get the latest version available for an extension
    async fn get_latest_version(&self, name: &str) -> Result<Option<String>>;

    /// Update an extension to the latest version
    async fn update_extension(
        &self,
        name: &str,
        target_dir: &Path,
        options: &UpdateOptions,
    ) -> Result<InstalledExtension>;

    // Version Management

    /// List all available versions for an extension
    async fn list_versions(&self, name: &str) -> Result<Vec<String>>;

    /// Check if a specific version exists for an extension
    async fn version_exists(&self, name: &str, version: &str) -> Result<bool>;

    // Capability Queries

    /// Check if this store supports the given capability
    fn supports_capability(&self, capability: &str) -> bool;

    /// Get a list of all capabilities supported by this store
    fn capabilities(&self) -> Vec<String>;

    // Optional Operations (default implementations provided)

    /// Download and cache an extension package for faster access
    async fn cache_extension(&self, name: &str, version: Option<&str>) -> Result<()> {
        // Default implementation: just verify the package exists
        let _ = self.get_extension_package(name, version).await?;
        Ok(())
    }

    /// Clear cached data for an extension
    async fn clear_cache(&self, name: Option<&str>) -> Result<()> {
        // Default implementation: no-op since not all stores have caches
        let _ = name;
        Ok(())
    }

    /// Validate the integrity of an extension package
    async fn validate_extension(&self, name: &str, version: Option<&str>) -> Result<bool> {
        // Default implementation: verify checksum if available
        let manifest = self.get_manifest(name, version).await?;
        let wasm_bytes = self.get_extension_wasm(name, version).await?;

        // Verify checksum using the enhanced checksum system
        let checksum_valid = manifest.checksum.verify(&wasm_bytes);

        // Also verify signature if present and supported
        if let Some(signature) = &manifest.signature {
            // For now, just log that signature verification would happen
            // Full signature verification would require public key infrastructure
            tracing::debug!(
                "Signature verification requested for {}@{:?} with key {}",
                name,
                version,
                signature.public_key_id
            );
        }

        Ok(checksum_valid)
    }
}

/// Store capabilities that can be queried
pub mod capabilities {
    pub const SEARCH: &str = "search";
    pub const VERSIONING: &str = "versioning";
    pub const METADATA: &str = "metadata";
    pub const CACHING: &str = "caching";
    pub const UPDATE_CHECK: &str = "update_check";
    pub const BATCH_OPERATIONS: &str = "batch_operations";
    pub const STREAMING: &str = "streaming";
    pub const AUTHENTICATION: &str = "authentication";
    pub const PRIVATE_EXTENSIONS: &str = "private_extensions";
    pub const SIGNATURES: &str = "signatures";
    pub const DEPENDENCIES: &str = "dependencies";
    pub const ROLLBACK: &str = "rollback";
}

/// Helper trait for stores that support batch operations
#[async_trait]
pub trait BatchStore: Store {
    /// Install multiple extensions in parallel
    async fn batch_install(
        &self,
        requests: &[(String, Option<String>)], // (name, version) pairs
        target_dir: &Path,
        options: &InstallOptions,
    ) -> Result<Vec<Result<InstalledExtension>>>;

    /// Get multiple extension packages in parallel
    async fn batch_get_packages(
        &self,
        requests: &[(String, Option<String>)], // (name, version) pairs
    ) -> Result<Vec<Result<ExtensionPackage>>>;
}

/// Helper trait for stores that support streaming operations
#[async_trait]
pub trait StreamingStore: Store {
    /// Stream extension list with pagination
    async fn stream_extensions(
        &self,
        page_size: usize,
    ) -> Result<Box<dyn futures::Stream<Item = Result<ExtensionInfo>> + Unpin + Send>>;

    /// Stream search results with pagination
    async fn stream_search(
        &self,
        query: &SearchQuery,
        page_size: usize,
    ) -> Result<Box<dyn futures::Stream<Item = Result<ExtensionInfo>> + Unpin + Send>>;
}

/// Helper trait for stores that support authentication
#[async_trait]
pub trait AuthenticatedStore: Store {
    /// Authenticate with the store using provided credentials
    async fn authenticate(&mut self, credentials: &StoreCredentials) -> Result<()>;

    /// Check if currently authenticated
    fn is_authenticated(&self) -> bool;

    /// Log out and clear authentication
    async fn logout(&mut self) -> Result<()>;
}

/// Store authentication credentials
#[derive(Debug, Clone)]
pub enum StoreCredentials {
    Token(String),
    UsernamePassword {
        username: String,
        password: String,
    },
    ApiKey(String),
    Certificate {
        cert_path: String,
        key_path: String,
    },
    OAuth {
        client_id: String,
        client_secret: String,
        token: Option<String>,
    },
}

/// Store factory for creating store instances from configuration
pub trait StoreFactory {
    type Store: Store;
    type Config;
    type Error;

    /// Create a new store instance from configuration
    fn create_store(config: Self::Config) -> std::result::Result<Self::Store, Self::Error>;

    /// Validate configuration before creating store
    fn validate_config(config: &Self::Config) -> std::result::Result<(), Self::Error>;

    /// Get the store type identifier
    fn store_type() -> &'static str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    // Mock store for testing
    struct MockStore {
        info: StoreInfo,
        layout: PackageLayout,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                info: StoreInfo::new("mock".to_string(), "test".to_string()),
                layout: PackageLayout::default(),
            }
        }
    }

    #[async_trait]
    impl Store for MockStore {
        fn store_info(&self) -> &StoreInfo {
            &self.info
        }

        fn package_layout(&self) -> &PackageLayout {
            &self.layout
        }

        async fn health_check(&self) -> Result<StoreHealth> {
            Ok(StoreHealth::healthy())
        }

        async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>> {
            Ok(vec![])
        }

        async fn search_extensions(&self, _query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
            Ok(vec![])
        }

        async fn get_extension_info(&self, _name: &str) -> Result<Vec<ExtensionInfo>> {
            Ok(vec![])
        }

        async fn get_extension_version_info(
            &self,
            _name: &str,
            _version: Option<&str>,
        ) -> Result<ExtensionInfo> {
            Err(crate::error::StoreError::ExtensionNotFound(
                "mock".to_string(),
            ))
        }

        async fn get_manifest(
            &self,
            _name: &str,
            _version: Option<&str>,
        ) -> Result<ExtensionManifest> {
            Err(crate::error::StoreError::ExtensionNotFound(
                "mock".to_string(),
            ))
        }

        async fn get_metadata(
            &self,
            _name: &str,
            _version: Option<&str>,
        ) -> Result<Option<ExtensionMetadata>> {
            Ok(None)
        }

        async fn get_extension_wasm(&self, _name: &str, _version: Option<&str>) -> Result<Vec<u8>> {
            Err(crate::error::StoreError::ExtensionNotFound(
                "mock".to_string(),
            ))
        }

        async fn get_extension_package(
            &self,
            _name: &str,
            _version: Option<&str>,
        ) -> Result<ExtensionPackage> {
            Err(crate::error::StoreError::ExtensionNotFound(
                "mock".to_string(),
            ))
        }

        async fn install_extension(
            &self,
            _name: &str,
            _version: Option<&str>,
            _target_dir: &Path,
            _options: &InstallOptions,
        ) -> Result<InstalledExtension> {
            Err(crate::error::StoreError::ExtensionNotFound(
                "mock".to_string(),
            ))
        }

        async fn check_updates(
            &self,
            _installed: &[InstalledExtension],
        ) -> Result<Vec<UpdateInfo>> {
            Ok(vec![])
        }

        async fn get_latest_version(&self, _name: &str) -> Result<Option<String>> {
            Ok(None)
        }

        async fn update_extension(
            &self,
            _name: &str,
            _target_dir: &Path,
            _options: &UpdateOptions,
        ) -> Result<InstalledExtension> {
            Err(crate::error::StoreError::ExtensionNotFound(
                "mock".to_string(),
            ))
        }

        async fn list_versions(&self, _name: &str) -> Result<Vec<String>> {
            Ok(vec![])
        }

        async fn version_exists(&self, _name: &str, _version: &str) -> Result<bool> {
            Ok(false)
        }

        fn supports_capability(&self, _capability: &str) -> bool {
            false
        }

        fn capabilities(&self) -> Vec<String> {
            vec![]
        }
    }

    #[tokio::test]
    async fn test_mock_store_creation() {
        let store = MockStore::new();
        assert_eq!(store.store_info().name, "mock");
        assert_eq!(store.store_info().store_type, "test");
    }

    #[tokio::test]
    async fn test_mock_store_health_check() {
        let store = MockStore::new();
        let health = store.health_check().await.unwrap();
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn test_mock_store_capabilities() {
        let store = MockStore::new();
        assert_eq!(store.capabilities().len(), 0);
        assert!(!store.supports_capability(capabilities::SEARCH));
    }
}
