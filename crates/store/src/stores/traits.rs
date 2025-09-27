//! Store capability traits for better separation of concerns
//!
//! This module defines the core traits that different store implementations can implement
//! to provide specific capabilities. This allows for a more modular and extensible
//! store system that can support different backends (local, git, http, etc.).

use std::any::Any;

use async_trait::async_trait;

use crate::error::Result;
use crate::manifest::ExtensionManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, InstalledExtension, SearchQuery,
    StoreHealth, UpdateInfo,
};
use crate::publish::{
    PublishOptions, PublishPermissions, PublishRequirements, PublishResult, PublishStats,
    PublishUpdateOptions, RateLimitStatus, UnpublishOptions, UnpublishResult, ValidationReport,
};
use crate::store_manifest::StoreManifest;

/// Core store interface that all stores must implement
#[async_trait]
pub trait BaseStore: Send + Sync + Any {
    /// Get the store manifest containing identity and basic information
    async fn get_store_manifest(&self) -> Result<StoreManifest>;

    /// Check the health status of this store
    async fn health_check(&self) -> Result<StoreHealth>;

    /// Get a list of all capabilities supported by this store
    fn capabilities(&self) -> Vec<String>;

    /// Check if this store supports the given capability
    fn supports_capability(&self, capability: &str) -> bool {
        self.capabilities().contains(&capability.to_string())
    }

    /// Get the store type identifier (local, git, http, etc.)
    fn store_type(&self) -> &'static str;

    /// Get store name/identifier
    fn name(&self) -> &str;
}

/// Store that can be read from (discovery, search, download)
#[async_trait]
pub trait ReadableStore: BaseStore {
    /// Find extensions that can handle the given URL
    /// Returns (id, name) pairs
    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>>;

    /// Find extensions that support a specific domain
    async fn find_extensions_for_domain(&self, domain: &str) -> Result<Vec<String>>;

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

    /// Get the manifest for a specific extension version
    async fn get_manifest(&self, name: &str, version: Option<&str>) -> Result<ExtensionManifest>;

    /// Get the metadata for a specific extension version
    async fn get_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>>;

    /// Get the complete extension package including all files
    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage>;

    /// Get the latest version available for an extension
    async fn get_latest_version(&self, id: &str) -> Result<Option<String>>;

    /// List all available versions for an extension
    async fn list_versions(&self, id: &str) -> Result<Vec<String>>;

    /// Check if a specific version exists for an extension
    async fn version_exists(&self, id: &str, version: &str) -> Result<bool>;
}

/// Store that can be written to (publish extensions)
#[async_trait]
pub trait WritableStore: BaseStore {
    /// Get publishing requirements for this store
    fn publish_requirements(&self) -> PublishRequirements;

    /// Check if a user can publish to this store
    async fn can_publish(&self, extension_id: &str) -> Result<PublishPermissions>;

    /// Get current rate limit status
    async fn get_rate_limit_status(&self, user_id: &str) -> Result<RateLimitStatus>;

    /// Publish an extension package
    async fn publish(
        &self,
        package: ExtensionPackage,
        options: PublishOptions,
    ) -> Result<PublishResult>;

    /// Update an existing published extension
    async fn update_published(
        &self,
        extension_id: &str,
        package: ExtensionPackage,
        options: PublishUpdateOptions,
    ) -> Result<PublishResult>;

    /// Unpublish an extension
    async fn unpublish(
        &self,
        extension_id: &str,
        options: UnpublishOptions,
    ) -> Result<UnpublishResult>;

    /// Get publishing statistics
    async fn get_publish_stats(&self) -> Result<PublishStats>;

    /// Validate a package before publishing
    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<ValidationReport>;
}

/// Store that supports update checking
#[async_trait]
pub trait UpdatableStore: ReadableStore {
    /// Check for updates for the given installed extensions
    async fn check_updates(&self, installed: &[InstalledExtension]) -> Result<Vec<UpdateInfo>>;
}

/// Store that supports caching for better performance
#[async_trait]
pub trait CacheableStore: BaseStore {
    /// Refresh the store cache
    async fn refresh_cache(&self) -> Result<()>;

    /// Clear the store cache
    async fn clear_cache(&self) -> Result<()>;

    /// Get cache statistics
    async fn cache_stats(&self) -> Result<CacheStats>;
}

/// Store that requires authentication
#[async_trait]
pub trait AuthenticatedStore: BaseStore {
    /// Authenticate with the store
    async fn authenticate(&self, credentials: AuthCredentials) -> Result<()>;

    /// Check if currently authenticated
    async fn is_authenticated(&self) -> Result<bool>;

    /// Get current authentication status
    async fn auth_status(&self) -> Result<AuthStatus>;
}

/// Store that supports versioned content (like Git)
#[async_trait]
pub trait VersionedStore: ReadableStore {
    /// Get the commit/revision hash for a specific version
    async fn get_revision_hash(&self, id: &str, version: &str) -> Result<String>;

    /// List all branches/tags available
    async fn list_refs(&self) -> Result<Vec<RefInfo>>;

    /// Get changelog between versions
    async fn get_changelog(&self, id: &str, from_version: &str, to_version: &str)
        -> Result<String>;
}

/// Combined trait for stores that support both reading and writing
pub trait ReadWriteStore: ReadableStore + WritableStore {}

/// Blanket implementation for stores that implement both traits
impl<T> ReadWriteStore for T where T: ReadableStore + WritableStore {}

/// Store capabilities constants
pub mod capabilities {
    pub const READ: &str = "read";
    pub const WRITE: &str = "write";
    pub const SEARCH: &str = "search";
    pub const VERSIONING: &str = "versioning";
    pub const METADATA: &str = "metadata";
    pub const CACHING: &str = "caching";
    pub const AUTHENTICATION: &str = "authentication";
    pub const UPDATE_CHECKING: &str = "update_checking";
    pub const PUBLISHING: &str = "publishing";
    pub const GIT_INTEGRATION: &str = "git_integration";
    pub const UPDATE_CHECK: &str = "update_check";
    pub const BATCH_OPERATIONS: &str = "batch_operations";
    pub const STREAMING: &str = "streaming";
    pub const PRIVATE_EXTENSIONS: &str = "private_extensions";
    pub const SIGNATURES: &str = "signatures";
    pub const DEPENDENCIES: &str = "dependencies";
    pub const ROLLBACK: &str = "rollback";
}

/// Cache statistics for cacheable stores
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entries: usize,
    pub size_bytes: u64,
    pub hit_rate: f64,
    pub last_refresh: Option<chrono::DateTime<chrono::Utc>>,
}

/// Authentication credentials for authenticated stores
#[derive(Debug, Clone)]
pub enum AuthCredentials {
    /// Username and password authentication
    UserPassword { username: String, password: String },
    /// Token-based authentication
    Token { token: String },
    /// SSH key authentication
    SshKey {
        private_key: String,
        passphrase: Option<String>,
    },
    /// OAuth token
    OAuth {
        access_token: String,
        refresh_token: Option<String>,
    },
}

/// Authentication status
#[derive(Debug, Clone)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub user_id: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub permissions: Vec<String>,
}

/// Reference information for versioned stores
#[derive(Debug, Clone)]
pub struct RefInfo {
    pub name: String,
    pub ref_type: RefType,
    pub hash: String,
    pub message: Option<String>,
}

/// Type of version reference
#[derive(Debug, Clone)]
pub enum RefType {
    Branch,
    Tag,
    Commit,
}

/// Helper trait for downcasting store trait objects to concrete types
pub trait StoreExt {
    /// Try to downcast to a specific store implementation
    fn as_any(&self) -> &dyn Any;
}

// Default implementation of StoreExt for all BaseStore implementations
impl<T: BaseStore> StoreExt for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
