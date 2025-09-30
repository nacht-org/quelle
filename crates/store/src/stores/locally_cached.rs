//! Locally cached store implementation
//!
//! This module provides LocallyCachedStore which wraps a StoreProvider and LocalStore
//! to provide a unified interface for stores that sync data from remote sources
//! and cache it locally for fast access.

use async_trait::async_trait;
use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::error::Result;
use crate::stores::{
    local::LocalStore,
    providers::traits::{StoreProvider, SyncResult},
    traits::{BaseStore, CacheableStore, ReadableStore},
};
use crate::{
    manifest::ExtensionManifest, ExtensionInfo, ExtensionMetadata, ExtensionPackage,
    InstalledExtension, SearchQuery, StoreHealth, StoreManifest, UpdateInfo,
};

/// A store that syncs data from a provider and uses LocalStore for access
pub struct LocallyCachedStore<T: StoreProvider> {
    provider: T,
    local_store: LocalStore,
    sync_dir: PathBuf,
    name: String,
}

impl<T: StoreProvider> LocallyCachedStore<T> {
    /// Create a new locally cached store
    pub fn new(provider: T, sync_dir: PathBuf, name: String) -> Result<Self> {
        let local_store = LocalStore::new(&sync_dir)?;
        Ok(Self {
            provider,
            local_store,
            sync_dir,
            name,
        })
    }

    /// Get the sync directory
    pub fn sync_dir(&self) -> &PathBuf {
        &self.sync_dir
    }

    /// Get the provider
    pub fn provider(&self) -> &T {
        &self.provider
    }

    /// Get the underlying local store
    pub fn local_store(&self) -> &LocalStore {
        &self.local_store
    }

    /// Ensure the store is synced and ready for use
    async fn ensure_synced(&self) -> Result<Option<SyncResult>> {
        debug!(
            "Checking if sync needed for store '{}' ({})",
            self.name,
            self.provider.provider_type()
        );

        match self.provider.sync_if_needed(&self.sync_dir).await {
            Ok(Some(result)) => {
                info!(
                    "Synced store '{}': {} changes, {} warnings",
                    self.name,
                    result.changes.len(),
                    result.warnings.len()
                );

                for warning in &result.warnings {
                    warn!("Sync warning for '{}': {}", self.name, warning);
                }

                Ok(Some(result))
            }
            Ok(None) => {
                debug!("Store '{}' is up to date, no sync needed", self.name);
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to sync store '{}': {}", self.name, e);
                Err(e)
            }
        }
    }
}

#[async_trait]
impl<T: StoreProvider> BaseStore for LocallyCachedStore<T> {
    async fn get_store_manifest(&self) -> Result<StoreManifest> {
        self.ensure_synced().await?;
        self.local_store.get_store_manifest().await
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        // First try to sync
        match self.ensure_synced().await {
            Ok(_) => {
                // If sync succeeded, check local store health
                self.local_store.health_check().await
            }
            Err(e) => {
                // If sync failed, return unhealthy status
                Ok(StoreHealth {
                    healthy: false,
                    last_check: chrono::Utc::now(),
                    response_time: None,
                    error: Some(format!("Sync failed: {}", e)),
                    extension_count: None,
                    store_version: None,
                })
            }
        }
    }
}

#[async_trait]
impl<T: StoreProvider> ReadableStore for LocallyCachedStore<T> {
    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        self.ensure_synced().await?;
        self.local_store.find_extensions_for_url(url).await
    }

    async fn find_extensions_for_domain(&self, domain: &str) -> Result<Vec<String>> {
        self.ensure_synced().await?;
        self.local_store.find_extensions_for_domain(domain).await
    }

    async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>> {
        self.ensure_synced().await?;
        self.local_store.list_extensions().await
    }

    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
        self.ensure_synced().await?;
        self.local_store.search_extensions(query).await
    }

    async fn get_extension_info(&self, name: &str) -> Result<Vec<ExtensionInfo>> {
        self.ensure_synced().await?;
        self.local_store.get_extension_info(name).await
    }

    async fn get_extension_version_info(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo> {
        self.ensure_synced().await?;
        self.local_store
            .get_extension_version_info(name, version)
            .await
    }

    async fn get_extension_manifest(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest> {
        self.ensure_synced().await?;
        self.local_store.get_extension_manifest(name, version).await
    }

    async fn get_extension_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        self.ensure_synced().await?;
        self.local_store.get_extension_metadata(name, version).await
    }

    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        self.ensure_synced().await?;
        self.local_store.get_extension_package(id, version).await
    }

    async fn get_extension_latest_version(&self, id: &str) -> Result<Option<String>> {
        self.ensure_synced().await?;
        self.local_store.get_extension_latest_version(id).await
    }

    async fn list_extension_versions(&self, id: &str) -> Result<Vec<String>> {
        self.ensure_synced().await?;
        self.local_store.list_extension_versions(id).await
    }

    async fn check_extension_version_exists(&self, id: &str, version: &str) -> Result<bool> {
        self.ensure_synced().await?;
        self.local_store
            .check_extension_version_exists(id, version)
            .await
    }

    async fn check_extension_updates(
        &self,
        installed: &[InstalledExtension],
    ) -> Result<Vec<UpdateInfo>> {
        self.ensure_synced().await?;
        self.local_store.check_extension_updates(installed).await
    }
}

#[async_trait]
impl<T: StoreProvider> CacheableStore for LocallyCachedStore<T> {
    async fn refresh_cache(&self) -> Result<()> {
        // Force a sync from the provider
        debug!("Force refreshing cache for store '{}'", self.name);
        self.provider.sync(&self.sync_dir).await?;

        // Then refresh the local store cache
        self.local_store.refresh_cache().await
    }

    async fn clear_cache(&self) -> Result<()> {
        self.local_store.clear_cache(None).await
    }

    async fn cache_stats(&self) -> Result<crate::stores::traits::CacheStats> {
        self.local_store.cache_stats().await
    }
}

impl<T: StoreProvider> std::fmt::Debug for LocallyCachedStore<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocallyCachedStore")
            .field("name", &self.name)
            .field("sync_dir", &self.sync_dir)
            .field("provider_type", &self.provider.provider_type())
            .field("provider_description", &self.provider.description())
            .finish()
    }
}

impl<T: StoreProvider> std::fmt::Display for LocallyCachedStore<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LocallyCachedStore({}: {})",
            self.name,
            self.provider.description()
        )
    }
}
