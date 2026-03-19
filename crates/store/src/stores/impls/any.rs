//! `AnyStore` — a single owned value that can act as any store trait object.
//!
//! [`AnyStore`] is an enum that holds exactly **one** concrete store instance and
//! implements [`BaseStore`], [`ReadableStore`], [`WritableStore`], and
//! [`SyncableStore`] by delegation.  This means callers only ever need to
//! construct one store object and can then obtain any trait-object view from it
//! without rebuilding the underlying store.
//!
//! # Usage
//!
//! ```rust,ignore
//! let any = ExtensionSource::local("my-store".into(), path).build()?;
//!
//! // Use as a readable source:
//! manager.add_extension_store(any.into_readable(), config).await?;
//!
//! // Or check write support before converting:
//! if any.is_writable() {
//!     let w: Box<dyn WritableStore> = Box::new(any);
//! }
//! ```

use async_trait::async_trait;
use semver::Version;

use crate::error::Result;
use crate::manager::publish::{
    PublishOptions, PublishRequirements, PublishResult, UnpublishOptions, UnpublishResult,
    ValidationReport,
};
use crate::manager::store_manifest::StoreManifest;
use crate::models::{
    ExtensionInfo, ExtensionListing, ExtensionMetadata, ExtensionPackage, InstalledExtension,
    SearchQuery, StoreHealth, UpdateInfo,
};
use crate::registry::manifest::ExtensionManifest;
use crate::stores::impls::local::LocalStore;
use crate::stores::traits::{BaseStore, CacheStats, ReadableStore, SyncableStore, WritableStore};

#[cfg(feature = "git")]
use crate::stores::impls::GitStore;

#[cfg(feature = "github")]
use crate::stores::impls::GitHubStore;

// ---------------------------------------------------------------------------
// AnyStore
// ---------------------------------------------------------------------------

/// A single concrete store that can satisfy all store-trait roles.
///
/// Use [`AnyStore::is_writable`] to check whether the wrapped store supports
/// publishing before calling any [`WritableStore`] method; those methods will
/// return [`StoreError::UnsupportedOperation`] for non-writable configurations
/// rather than panicking.
pub enum AnyStore {
    /// A plain local-filesystem store.
    Local(LocalStore),

    /// A git-backed store (requires the `git` feature).
    #[cfg(feature = "git")]
    Git(GitStore),

    /// A GitHub-backed store (requires the `github` feature).
    #[cfg(feature = "github")]
    GitHub(GitHubStore),
}

impl AnyStore {
    /// Returns `true` if this store supports write operations (publish /
    /// unpublish).
    pub fn is_writable(&self) -> bool {
        match self {
            AnyStore::Local(s) => !s.is_readonly(),
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.is_writable(),
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.is_writable(),
        }
    }

    /// Consume `self` and return a heap-allocated [`ReadableStore`] trait object.
    pub fn into_readable(self) -> Box<dyn ReadableStore> {
        Box::new(self)
    }

    /// Consume `self` and return a heap-allocated [`WritableStore`] trait object,
    /// or `None` when the store is not configured for writing.
    pub fn into_writable(self) -> Option<Box<dyn WritableStore>> {
        if self.is_writable() {
            Some(Box::new(self))
        } else {
            None
        }
    }

    /// Consume `self` and return a heap-allocated [`SyncableStore`] trait object.
    pub fn into_syncable(self) -> Box<dyn SyncableStore> {
        Box::new(self)
    }
}

// ---------------------------------------------------------------------------
// BaseStore
// ---------------------------------------------------------------------------

#[async_trait]
impl BaseStore for AnyStore {
    async fn get_store_manifest(&self) -> Result<StoreManifest> {
        match self {
            AnyStore::Local(s) => s.get_store_manifest().await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.get_store_manifest().await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.get_store_manifest().await,
        }
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        match self {
            AnyStore::Local(s) => s.health_check().await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.health_check().await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.health_check().await,
        }
    }
}

// ---------------------------------------------------------------------------
// ReadableStore
// ---------------------------------------------------------------------------

#[async_trait]
impl ReadableStore for AnyStore {
    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        match self {
            AnyStore::Local(s) => s.find_extensions_for_url(url).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.find_extensions_for_url(url).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.find_extensions_for_url(url).await,
        }
    }

    async fn list_extensions(&self) -> Result<Vec<ExtensionListing>> {
        match self {
            AnyStore::Local(s) => s.list_extensions().await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.list_extensions().await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.list_extensions().await,
        }
    }

    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionListing>> {
        match self {
            AnyStore::Local(s) => s.search_extensions(query).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.search_extensions(query).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.search_extensions(query).await,
        }
    }

    async fn get_extension_info(&self, id: &str) -> Result<Vec<ExtensionInfo>> {
        match self {
            AnyStore::Local(s) => s.get_extension_info(id).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.get_extension_info(id).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.get_extension_info(id).await,
        }
    }

    async fn get_extension_version_info(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<ExtensionInfo> {
        match self {
            AnyStore::Local(s) => s.get_extension_version_info(id, version).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.get_extension_version_info(id, version).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.get_extension_version_info(id, version).await,
        }
    }

    async fn get_extension_manifest(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<ExtensionManifest> {
        match self {
            AnyStore::Local(s) => s.get_extension_manifest(id, version).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.get_extension_manifest(id, version).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.get_extension_manifest(id, version).await,
        }
    }

    async fn get_extension_metadata(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<Option<ExtensionMetadata>> {
        match self {
            AnyStore::Local(s) => s.get_extension_metadata(id, version).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.get_extension_metadata(id, version).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.get_extension_metadata(id, version).await,
        }
    }

    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<ExtensionPackage> {
        match self {
            AnyStore::Local(s) => s.get_extension_package(id, version).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.get_extension_package(id, version).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.get_extension_package(id, version).await,
        }
    }

    async fn get_extension_latest_version(&self, id: &str) -> Result<Option<Version>> {
        match self {
            AnyStore::Local(s) => s.get_extension_latest_version(id).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.get_extension_latest_version(id).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.get_extension_latest_version(id).await,
        }
    }

    async fn list_extension_versions(&self, id: &str) -> Result<Vec<Version>> {
        match self {
            AnyStore::Local(s) => s.list_extension_versions(id).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.list_extension_versions(id).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.list_extension_versions(id).await,
        }
    }

    async fn check_extension_version_exists(&self, id: &str, version: &Version) -> Result<bool> {
        match self {
            AnyStore::Local(s) => s.check_extension_version_exists(id, version).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.check_extension_version_exists(id, version).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.check_extension_version_exists(id, version).await,
        }
    }

    async fn check_extension_updates(
        &self,
        installed: &[InstalledExtension],
    ) -> Result<Vec<UpdateInfo>> {
        match self {
            AnyStore::Local(s) => s.check_extension_updates(installed).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.check_extension_updates(installed).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.check_extension_updates(installed).await,
        }
    }
}

// ---------------------------------------------------------------------------
// WritableStore
// ---------------------------------------------------------------------------

/// Non-writable stores return [`StoreError::UnsupportedOperation`] from every
/// [`WritableStore`] method.  Callers should check [`AnyStore::is_writable`]
/// before attempting write operations, or use [`AnyStore::into_writable`] which
/// returns `None` for non-writable stores.
#[async_trait]
impl WritableStore for AnyStore {
    fn publish_requirements(&self) -> PublishRequirements {
        match self {
            AnyStore::Local(s) => s.publish_requirements(),
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.publish_requirements(),
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.publish_requirements(),
        }
    }

    async fn publish(
        &self,
        package: ExtensionPackage,
        options: PublishOptions,
    ) -> Result<PublishResult> {
        match self {
            AnyStore::Local(s) => s.publish(package, options).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.publish(package, options).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.publish(package, options).await,
        }
    }

    async fn unpublish(
        &self,
        extension_id: &str,
        options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        match self {
            AnyStore::Local(s) => s.unpublish(extension_id, options).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.unpublish(extension_id, options).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.unpublish(extension_id, options).await,
        }
    }

    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<ValidationReport> {
        match self {
            AnyStore::Local(s) => s.validate_package(package, options).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.validate_package(package, options).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.validate_package(package, options).await,
        }
    }
}

// ---------------------------------------------------------------------------
// SyncableStore
// ---------------------------------------------------------------------------

#[async_trait]
impl SyncableStore for AnyStore {
    async fn force_sync(&self) -> Result<()> {
        match self {
            AnyStore::Local(s) => s.force_sync().await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => s.force_sync().await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => s.force_sync().await,
        }
    }

    async fn clear_cache(&self) -> Result<()> {
        match self {
            AnyStore::Local(s) => SyncableStore::clear_cache(s).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => SyncableStore::clear_cache(s).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => SyncableStore::clear_cache(s).await,
        }
    }

    async fn cache_stats(&self) -> Result<CacheStats> {
        match self {
            AnyStore::Local(s) => SyncableStore::cache_stats(s).await,
            #[cfg(feature = "git")]
            AnyStore::Git(s) => SyncableStore::cache_stats(s).await,
            #[cfg(feature = "github")]
            AnyStore::GitHub(s) => SyncableStore::cache_stats(s).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

impl From<LocalStore> for AnyStore {
    fn from(s: LocalStore) -> Self {
        AnyStore::Local(s)
    }
}

#[cfg(feature = "git")]
impl From<GitStore> for AnyStore {
    fn from(s: GitStore) -> Self {
        AnyStore::Git(s)
    }
}

#[cfg(feature = "github")]
impl From<GitHubStore> for AnyStore {
    fn from(s: GitHubStore) -> Self {
        AnyStore::GitHub(s)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::impls::local::LocalStore;
    use tempfile::TempDir;

    fn make_local(dir: &TempDir) -> AnyStore {
        AnyStore::Local(LocalStore::builder(dir.path()).readonly().build().unwrap())
    }

    #[test]
    fn test_local_store_is_not_writable_by_default() {
        let dir = TempDir::new().unwrap();
        let store = make_local(&dir);
        // LocalStore built with .readonly() is not writable
        assert!(!store.is_writable());
    }

    #[test]
    fn test_into_readable_returns_box() {
        let dir = TempDir::new().unwrap();
        let store = make_local(&dir);
        // Should compile and not panic
        let _readable: Box<dyn ReadableStore> = store.into_readable();
    }

    #[test]
    fn test_into_writable_returns_none_for_readonly() {
        let dir = TempDir::new().unwrap();
        let store = make_local(&dir);
        assert!(store.into_writable().is_none());
    }

    #[test]
    fn test_from_local_store() {
        let dir = TempDir::new().unwrap();
        let local = LocalStore::new(dir.path()).unwrap();
        let any: AnyStore = local.into();
        assert!(matches!(any, AnyStore::Local(_)));
    }

    #[tokio::test]
    async fn test_health_check_delegates() {
        let dir = TempDir::new().unwrap();
        let store = make_local(&dir);
        // Should not panic; result depends on filesystem state
        let _ = store.health_check().await;
    }
}
