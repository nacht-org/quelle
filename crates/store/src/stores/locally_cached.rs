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
use crate::publish::{
    PublishOptions, PublishRequirements, PublishResult, PublishUpdateOptions, UnpublishOptions,
    UnpublishResult, ValidationReport,
};
use crate::stores::{
    local::LocalStore,
    providers::{
        traits::{StoreProvider, SyncResult},
        GitProvider,
    },
    traits::{BaseStore, CacheableStore, ReadableStore, WritableStore},
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
impl<T: StoreProvider> WritableStore for LocallyCachedStore<T> {
    fn publish_requirements(&self) -> PublishRequirements {
        self.local_store.publish_requirements()
    }

    async fn publish(
        &self,
        package: ExtensionPackage,
        options: PublishOptions,
    ) -> Result<PublishResult> {
        // Ensure we're synced first
        self.ensure_synced().await?;

        // Delegate to local store for the actual publishing
        self.local_store.publish(package, options).await
    }

    async fn update_published(
        &self,
        extension_id: &str,
        package: ExtensionPackage,
        options: PublishUpdateOptions,
    ) -> Result<PublishResult> {
        // Ensure we're synced first
        self.ensure_synced().await?;

        // Delegate to local store
        self.local_store
            .update_published(extension_id, package, options)
            .await
    }

    async fn unpublish(
        &self,
        extension_id: &str,
        options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        // Ensure we're synced first
        self.ensure_synced().await?;

        // Delegate to local store
        self.local_store.unpublish(extension_id, options).await
    }

    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<ValidationReport> {
        // Delegate to local store for validation
        self.local_store.validate_package(package, options).await
    }
}

impl LocallyCachedStore<GitProvider> {
    /// Enhanced publish method that includes git workflow
    pub async fn publish_with_git(
        &self,
        package: ExtensionPackage,
        options: PublishOptions,
    ) -> Result<PublishResult> {
        // Ensure we're synced first
        self.ensure_synced().await?;

        // Check git repository status if writable
        if self.provider.is_writable() {
            let status = self.provider.check_repository_status().await?;
            if !status.is_publishable() {
                if let Some(reason) = status.publish_blocking_reason() {
                    return Err(crate::error::StoreError::InvalidPackage {
                        reason: format!("Cannot publish to git repository: {}", reason),
                    });
                }
            }
        }

        // Publish to local store first
        let result = self.local_store.publish(package.clone(), options).await?;

        // If git is writable, perform git operations
        if self.provider.is_writable() {
            if let Err(e) = self.git_publish_workflow(&package).await {
                // Log warning but don't fail the publish operation
                tracing::warn!("Git workflow failed after successful publish: {}", e);
            }
        }

        Ok(result)
    }

    /// Enhanced unpublish method that includes git workflow
    pub async fn unpublish_with_git(
        &self,
        extension_id: &str,
        options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        // Ensure we're synced first
        self.ensure_synced().await?;

        // Check git repository status if writable
        if self.provider.is_writable() {
            let status = self.provider.check_repository_status().await?;
            if !status.is_publishable() {
                if let Some(reason) = status.publish_blocking_reason() {
                    return Err(crate::error::StoreError::InvalidPackage {
                        reason: format!("Cannot unpublish from git repository: {}", reason),
                    });
                }
            }
        }

        // Unpublish from local store first
        let result = self.local_store.unpublish(extension_id, options).await?;

        // If git is writable, perform git operations
        if self.provider.is_writable() {
            if let Err(e) = self
                .git_unpublish_workflow(extension_id, &result.version)
                .await
            {
                tracing::warn!("Git workflow failed after successful unpublish: {}", e);
            }
        }

        Ok(result)
    }

    /// Git workflow for publishing operations
    async fn git_publish_workflow(&self, package: &ExtensionPackage) -> Result<()> {
        let write_config = self.provider.write_config.as_ref().ok_or_else(|| {
            crate::error::StoreError::InvalidPackage {
                reason: "Git write configuration not available".to_string(),
            }
        })?;

        // Determine which files were affected
        let extension_dir = self.sync_dir.join(&package.manifest.id);
        let affected_files = vec![
            self.sync_dir.join("store.json"), // Store manifest always updated
            extension_dir,
        ];

        // Stage changes
        self.provider.git_add(&affected_files).await?;

        // Create commit message
        let commit_message = write_config
            .commit_message_template
            .replace("{action}", "Add")
            .replace("{extension_id}", &package.manifest.id)
            .replace("{version}", &package.manifest.version.to_string());

        // Commit changes
        self.provider.git_commit(&commit_message).await?;

        // Push if auto-push is enabled
        if write_config.auto_push {
            self.provider.git_push().await?;
        }

        Ok(())
    }

    /// Git workflow for unpublishing operations
    async fn git_unpublish_workflow(&self, extension_id: &str, version: &str) -> Result<()> {
        let write_config = self.provider.write_config.as_ref().ok_or_else(|| {
            crate::error::StoreError::InvalidPackage {
                reason: "Git write configuration not available".to_string(),
            }
        })?;

        // Only stage the store manifest since the extension directory was removed
        let affected_files = vec![self.sync_dir.join("store.json")];

        // Stage changes
        self.provider.git_add(&affected_files).await?;

        // Create commit message
        let commit_message = write_config
            .commit_message_template
            .replace("{action}", "Remove")
            .replace("{extension_id}", extension_id)
            .replace("{version}", version);

        // Commit changes
        self.provider.git_commit(&commit_message).await?;

        // Push if auto-push is enabled
        if write_config.auto_push {
            self.provider.git_push().await?;
        }

        Ok(())
    }

    /// Initialize a git store with proper metadata that includes git repository information
    pub async fn initialize_store(
        &self,
        store_name: String,
        description: Option<String>,
    ) -> Result<()> {
        use crate::store_manifest::StoreManifest;
        use crate::stores::local::LocalStoreManifest;

        // Get git-specific information
        let git_url = self.provider.url().to_string();
        let git_description = self.provider.description();

        // Use provided description or fall back to git description
        let final_description = description.unwrap_or_else(|| git_description);

        // Create git-specific manifest with repository URL
        let base_manifest = StoreManifest::new(store_name, "git".to_string(), "1.0.0".to_string())
            .with_url(git_url)
            .with_description(final_description);

        let local_manifest = LocalStoreManifest::new(base_manifest);

        // Use the shared write function from local store
        self.local_store.write_store_manifest(local_manifest).await
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
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        self.ensure_synced().await?;
        self.local_store.get_extension_package(name, version).await
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
        // Force a sync by clearing the cache and then syncing
        {
            let mut sync_state = self.sync_state.lock().await;
            sync_state.last_sync = None;
        }

        // Force sync
        self.provider.sync(&self.sync_dir).await?;

        // Update sync time
        {
            let mut sync_state = self.sync_state.lock().await;
            sync_state.last_sync = Some(std::time::Instant::now());
        }

        // Refresh local store cache
        self.local_store.refresh_cache().await
    }

    async fn clear_cache(&self) -> Result<()> {
        // Clear the sync cache
        let mut sync_state = self.sync_state.lock().await;
        sync_state.last_sync = None;

        // Delegate to local store for its cache clearing
        self.local_store.clear_cache(None).await
    }

    async fn cache_stats(&self) -> Result<crate::stores::traits::CacheStats> {
        self.local_store.cache_stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::providers::traits::SyncResult;
    use std::path::Path;
    use tempfile::TempDir;

    // Mock provider for testing
    struct MockProvider {
        should_sync: bool,
        sync_result: SyncResult,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                should_sync: true,
                sync_result: SyncResult::no_changes(),
            }
        }

        fn with_changes(changes: Vec<String>) -> Self {
            Self {
                should_sync: true,
                sync_result: SyncResult::with_changes(changes),
            }
        }
    }

    #[async_trait]
    impl StoreProvider for MockProvider {
        async fn sync(&self, _sync_dir: &Path) -> Result<SyncResult> {
            Ok(self.sync_result.clone())
        }

        async fn needs_sync(&self, _sync_dir: &Path) -> Result<bool> {
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
    async fn test_locally_cached_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::new();
        let store = LocallyCachedStore::new(
            provider,
            temp_dir.path().to_path_buf(),
            "test-store".to_string(),
        )
        .unwrap();

        assert_eq!(store.name, "test-store");
        assert_eq!(store.sync_dir(), temp_dir.path());
    }

    #[tokio::test]
    async fn test_sync_caching() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::with_changes(vec!["file1.json".to_string()]);
        let store = LocallyCachedStore::new(
            provider,
            temp_dir.path().to_path_buf(),
            "test-store".to_string(),
        )
        .unwrap();

        // First sync should work
        let result1 = store.ensure_synced().await.unwrap();
        assert!(result1.is_some());
        assert!(result1.unwrap().updated);

        // Second sync should be cached
        let result2 = store.ensure_synced().await.unwrap();
        assert!(result2.is_none());
    }

    #[tokio::test]
    async fn test_initialize_store() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::new();
        let store = LocallyCachedStore::new(
            provider,
            temp_dir.path().to_path_buf(),
            "test-store".to_string(),
        )
        .unwrap();

        // Initialize the store
        // Test that we can call initialize_store on MockProvider (it will delegate to local store)
        let local_store = store.local_store();
        local_store
            .initialize_store(
                "test-store".to_string(),
                Some("Test description".to_string()),
            )
            .await
            .unwrap();

        // Check that store.json was created
        let manifest_path = temp_dir.path().join("store.json");
        assert!(manifest_path.exists());
    }

    #[tokio::test]
    async fn test_git_initialize_store() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let git_url = "https://github.com/example/store.git";

        let provider = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );

        let store = LocallyCachedStore::new(
            provider,
            temp_dir.path().to_path_buf(),
            "git-test-store".to_string(),
        )
        .unwrap();

        // Initialize the git store with specific metadata
        store
            .initialize_store(
                "My Git Store".to_string(),
                Some("A store backed by Git repository".to_string()),
            )
            .await
            .unwrap();

        // Check that store.json was created with git-specific information
        let manifest_path = temp_dir.path().join("store.json");
        assert!(manifest_path.exists());

        // Read and verify the manifest content
        let content = fs::read_to_string(&manifest_path).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(manifest["name"], "My Git Store");
        assert_eq!(manifest["store_type"], "git");
        assert_eq!(manifest["url"], git_url);
        assert_eq!(manifest["description"], "A store backed by Git repository");
    }
}
