//! Locally cached store implementation
//!
//! This module provides LocallyCachedStore which wraps a StoreProvider and LocalStore
//! to provide a unified interface for stores that sync data from remote sources
//! and cache it locally for fast access.

use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
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

/// Synchronized sync state combining last sync time and mutex protection
#[derive(Debug)]
struct SyncState {
    last_sync: Option<Instant>,
}

/// A store that syncs data from a provider and uses LocalStore for access
pub struct LocallyCachedStore<T: StoreProvider> {
    provider: T,
    local_store: LocalStore,
    sync_dir: PathBuf,
    name: String,
    /// Combined sync state with mutex protection
    sync_state: Arc<Mutex<SyncState>>,
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
            sync_state: Arc::new(Mutex::new(SyncState { last_sync: None })),
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

    /// Ensure the store is synced and ready for use with time-based caching
    pub async fn ensure_synced(&self) -> Result<Option<SyncResult>> {
        const SYNC_CACHE_DURATION: Duration = Duration::from_secs(30);

        // Acquire sync state lock - this serves as both cache check and concurrency protection
        let mut sync_state = self.sync_state.lock().await;

        // Check if we've synced recently
        if let Some(sync_time) = sync_state.last_sync {
            if sync_time.elapsed() < SYNC_CACHE_DURATION {
                debug!(
                    "Skipping sync for store '{}' - synced {} seconds ago",
                    self.name,
                    sync_time.elapsed().as_secs()
                );
                return Ok(None);
            }
        }

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

                // Update last sync time
                sync_state.last_sync = Some(Instant::now());

                Ok(Some(result))
            }
            Ok(None) => {
                debug!("Store '{}' is up to date, no sync needed", self.name);

                // Still update last sync time to prevent redundant checks
                sync_state.last_sync = Some(Instant::now());

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
        // Force a sync from the provider (bypass cache)
        debug!("Force refreshing cache for store '{}'", self.name);

        // Acquire sync state lock and force sync
        let mut sync_state = self.sync_state.lock().await;

        // Clear last sync time to force fresh sync
        sync_state.last_sync = None;

        // Force sync while holding the lock
        self.provider.sync(&self.sync_dir).await?;

        // Update last sync time
        sync_state.last_sync = Some(Instant::now());

        // Release the lock before refreshing local store cache
        drop(sync_state);

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;
    use tokio::sync::Mutex;
    use tokio::time::sleep;

    /// Mock provider for testing sync behavior
    #[derive(Debug)]
    pub struct MockProvider {
        sync_count: Arc<Mutex<u32>>,
        sync_delay: Duration,
        should_sync: bool,
    }

    impl MockProvider {
        pub fn new() -> Self {
            Self {
                sync_count: Arc::new(Mutex::new(0)),
                sync_delay: Duration::from_millis(10),
                should_sync: true,
            }
        }

        pub fn with_delay(mut self, delay: Duration) -> Self {
            self.sync_delay = delay;
            self
        }

        pub fn with_sync_needed(mut self, should_sync: bool) -> Self {
            self.should_sync = should_sync;
            self
        }

        pub async fn get_sync_count(&self) -> u32 {
            *self.sync_count.lock().await
        }
    }

    #[async_trait::async_trait]
    impl StoreProvider for MockProvider {
        async fn sync(&self, _sync_dir: &std::path::Path) -> Result<SyncResult> {
            // Simulate sync work
            sleep(self.sync_delay).await;

            // Increment sync counter
            {
                let mut count = self.sync_count.lock().await;
                *count += 1;
            }

            Ok(SyncResult::with_changes(vec![
                "Mock sync completed".to_string()
            ]))
        }

        async fn needs_sync(&self, _sync_dir: &std::path::Path) -> Result<bool> {
            Ok(self.should_sync)
        }

        fn description(&self) -> String {
            "Mock provider for testing".to_string()
        }

        fn provider_type(&self) -> &'static str {
            "mock"
        }
    }

    #[tokio::test]
    async fn test_sync_efficiency_caching() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::new();
        let store = LocallyCachedStore::new(
            provider,
            temp_dir.path().to_path_buf(),
            "test-store".to_string(),
        )
        .unwrap();

        // Create a minimal store manifest for the local store to work
        let manifest_path = temp_dir.path().join("store.json");
        tokio::fs::write(
            &manifest_path,
            r#"{
            "name": "test-store",
            "version": "1.0.0",
            "description": "Test store",
            "extensions": []
        }"#,
        )
        .await
        .unwrap();

        // First call should trigger sync
        let start = Instant::now();
        let _ = store.list_extensions().await;
        let first_duration = start.elapsed();

        // Provider should have been called once
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 1);

        // Second call within cache window should NOT trigger sync
        let start = Instant::now();
        let _ = store.list_extensions().await;
        let second_duration = start.elapsed();

        // Provider should still only have been called once
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 1);

        // Second call should be much faster (no sync delay)
        assert!(second_duration < first_duration);

        // Wait for cache to expire
        sleep(Duration::from_secs(31)).await;

        // Third call should trigger sync again
        let _ = store.list_extensions().await;
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 2);
    }

    #[tokio::test]
    async fn test_concurrent_sync_prevention() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::new().with_delay(Duration::from_millis(100));
        let store = Arc::new(
            LocallyCachedStore::new(
                provider,
                temp_dir.path().to_path_buf(),
                "test-store".to_string(),
            )
            .unwrap(),
        );

        // Create a minimal store manifest
        let manifest_path = temp_dir.path().join("store.json");
        tokio::fs::write(
            &manifest_path,
            r#"{
            "name": "test-store",
            "version": "1.0.0",
            "description": "Test store",
            "extensions": []
        }"#,
        )
        .await
        .unwrap();

        // Launch multiple concurrent operations
        let mut handles = vec![];
        for _ in 0..5 {
            let store_clone = Arc::clone(&store);
            let handle = tokio::spawn(async move {
                let _ = store_clone.list_extensions().await;
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Despite 5 concurrent calls, sync should only happen once due to mutex
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 1);
    }

    #[tokio::test]
    async fn test_refresh_cache_forces_sync() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::new();
        let store = LocallyCachedStore::new(
            provider,
            temp_dir.path().to_path_buf(),
            "test-store".to_string(),
        )
        .unwrap();

        // Create a minimal store manifest
        let manifest_path = temp_dir.path().join("store.json");
        tokio::fs::write(
            &manifest_path,
            r#"{
            "name": "test-store",
            "version": "1.0.0",
            "description": "Test store",
            "extensions": []
        }"#,
        )
        .await
        .unwrap();

        // Normal operation triggers sync
        let _ = store.list_extensions().await;
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 1);

        // Another operation within cache window should not sync
        let _ = store.list_extensions().await;
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 1);

        // Force refresh should bypass cache and sync
        let _ = store.refresh_cache().await;
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 2);

        // Subsequent operations should not sync (cache updated by refresh)
        let _ = store.list_extensions().await;
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 2);
    }

    #[tokio::test]
    async fn test_no_sync_when_not_needed() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::new().with_sync_needed(false);
        let store = LocallyCachedStore::new(
            provider,
            temp_dir.path().to_path_buf(),
            "test-store".to_string(),
        )
        .unwrap();

        // Create a minimal store manifest
        let manifest_path = temp_dir.path().join("store.json");
        tokio::fs::write(
            &manifest_path,
            r#"{
            "name": "test-store",
            "version": "1.0.0",
            "description": "Test store",
            "extensions": []
        }"#,
        )
        .await
        .unwrap();

        // Operation should not trigger sync when provider says it's not needed
        let _ = store.list_extensions().await;
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 0);

        // Multiple operations should still not sync
        let _ = store.list_extensions().await;
        let _ = store.get_store_manifest().await;
        let sync_count = store.provider().get_sync_count().await;
        assert_eq!(sync_count, 0);
    }
}
