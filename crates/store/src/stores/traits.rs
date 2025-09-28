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
pub trait BaseStore: Send + Sync {
    /// Get the store manifest containing identity and basic information
    async fn get_store_manifest(&self) -> Result<StoreManifest>;

    /// Check the health status of this store
    async fn health_check(&self) -> Result<StoreHealth>;
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
    async fn get_extension_manifest(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest>;

    /// Get the metadata for a specific extension version
    async fn get_extension_metadata(
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
    async fn get_extension_latest_version(&self, id: &str) -> Result<Option<String>>;

    /// List all available versions for an extension
    async fn list_extension_versions(&self, id: &str) -> Result<Vec<String>>;

    /// Check if a specific version exists for an extension
    async fn check_extension_version_exists(&self, id: &str, version: &str) -> Result<bool>;

    /// Check for updates for the given installed extensions
    async fn check_extension_updates(
        &self,
        installed: &[InstalledExtension],
    ) -> Result<Vec<UpdateInfo>>;
}

/// Store that can be written to (publish extensions)
#[async_trait]
pub trait WritableStore: BaseStore {
    /// Get publishing requirements for this store
    fn publish_requirements(&self) -> PublishRequirements;

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

    /// Validate a package before publishing
    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<ValidationReport>;
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
