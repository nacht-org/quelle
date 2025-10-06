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
    PublishOptions, PublishRequirements, PublishResult, UnpublishOptions, UnpublishResult,
    ValidationReport,
};
use crate::stores::{
    local::LocalStore,
    providers::{
        traits::{LifecycleEvent, StoreProvider},
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
    ///
    /// The sync directory is determined by the provider's `sync_dir()` method.
    /// This ensures a single source of truth for where data is stored.
    pub fn new(provider: T, name: String) -> Result<Self> {
        let sync_dir = provider.sync_dir().to_path_buf();
        let local_store = LocalStore::new(&sync_dir)?;
        Ok(Self {
            provider,
            local_store,
            sync_dir,
            name,
            sync_state: Arc::new(Mutex::new(SyncState { last_sync: None })),
        })
    }

    /// Create a new locally cached store with a custom sync directory
    ///
    /// **Warning:** This is an advanced method. The sync_dir must match the provider's
    /// internal directory or behavior may be undefined. Use `new()` instead unless you
    /// have a specific reason to override the directory.
    #[deprecated(
        since = "0.1.0",
        note = "Use new() instead - provider manages its own directory"
    )]
    pub fn with_custom_sync_dir(provider: T, sync_dir: PathBuf, name: String) -> Result<Self> {
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
    pub async fn ensure_synced(&self) -> Result<()> {
        const SYNC_CACHE_DURATION: Duration = Duration::from_secs(30);

        // Acquire sync state lock - this serves as both cache check and concurrency protection
        let sync_state = self.sync_state.lock().await;

        // Check if we've synced recently
        if let Some(sync_time) = sync_state.last_sync {
            if sync_time.elapsed() < SYNC_CACHE_DURATION {
                debug!(
                    "Skipping sync for store '{}' - synced {} seconds ago",
                    self.name,
                    sync_time.elapsed().as_secs()
                );
                return Ok(());
            }
        }

        // Release the lock before syncing
        drop(sync_state);

        debug!(
            "Checking if sync needed for store '{}' ({})",
            self.name,
            self.provider.provider_type()
        );

        // Check if sync is needed
        if self.provider.needs_sync().await? {
            // Perform sync
            let result = self.provider.sync().await?;

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
            let mut sync_state = self.sync_state.lock().await;
            sync_state.last_sync = Some(Instant::now());
        } else {
            debug!("Store '{}' is up to date, no sync needed", self.name);

            // Update last sync time even when no sync was needed
            let mut sync_state = self.sync_state.lock().await;
            sync_state.last_sync = Some(Instant::now());
        }

        Ok(())
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

        // Check if provider supports writing and is in valid state
        self.provider.ensure_writable().await?;

        // Delegate to local store for the actual publishing
        let result = self.local_store.publish(package.clone(), options).await?;

        // Call lifecycle hook
        let event = LifecycleEvent::Published {
            extension_id: package.manifest.id.clone(),
            version: package.manifest.version.to_string(),
        };

        if let Err(e) = self.provider.handle_event(event).await {
            tracing::warn!("Lifecycle hook failed after successful publish: {}", e);
        }

        Ok(result)
    }

    async fn unpublish(
        &self,
        extension_id: &str,
        options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        // Ensure we're synced first
        self.ensure_synced().await?;

        // Check if provider supports writing and is in valid state
        self.provider.ensure_writable().await?;

        // Delegate to local store
        let result = self.local_store.unpublish(extension_id, options).await?;

        // Call lifecycle hook
        let event = LifecycleEvent::Unpublished {
            extension_id: extension_id.to_string(),
            version: result.version.clone(),
        };

        if let Err(e) = self.provider.handle_event(event).await {
            tracing::warn!("Lifecycle hook failed after successful unpublish: {}", e);
        }

        Ok(result)
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
        let final_description = description.unwrap_or(git_description);

        // Create git-specific manifest with repository URL
        let base_manifest =
            StoreManifest::new(store_name.clone(), "git".to_string(), "1.0.0".to_string())
                .with_url(git_url)
                .with_description(final_description);

        let local_manifest = LocalStoreManifest::new(base_manifest);

        // Use the shared write function from local store
        self.local_store
            .write_store_manifest(local_manifest)
            .await?;

        // If git is writable, commit and push the initialization
        if self.provider.is_writable() {
            tracing::info!(
                "Starting git initialization workflow for store: {}",
                store_name
            );
            if let Err(e) = self.git_initialize_workflow(&store_name).await {
                tracing::warn!("Git workflow failed after successful initialization: {}", e);
            } else {
                tracing::info!("Git initialization workflow completed successfully");
            }
        } else {
            tracing::info!(
                "Git store '{}' initialized successfully. To enable automatic git commits and pushes, configure GitWriteConfig with author info and commit settings.",
                store_name
            );
        }

        Ok(())
    }

    /// Git workflow for store initialization
    async fn git_initialize_workflow(&self, store_name: &str) -> Result<()> {
        tracing::debug!("Starting git initialization workflow");

        let write_config = self.provider.write_config.as_ref().ok_or_else(|| {
            tracing::error!("No write configuration available for git provider");
            crate::error::StoreError::InvalidPackage {
                reason: "Git write configuration not available".to_string(),
            }
        })?;

        tracing::debug!("Adding all changes to git staging area");
        // Add all changes (store.json and any other files)
        self.provider.git_add_all().await?;
        tracing::debug!("Successfully added changes to git staging area");

        // Create commit message for initialization
        let commit_message = format!("Initialize git store: {}", store_name);

        // Commit changes
        tracing::debug!("Committing changes with message: {}", commit_message);
        self.provider.git_commit(&commit_message).await?;
        tracing::info!("Successfully committed initialization changes");

        // Push if auto-push is enabled and authentication is available
        if write_config.auto_push {
            tracing::debug!("Auto-push is enabled, attempting to push to remote");
            if let Err(e) = self.provider.git_push().await {
                tracing::warn!("Failed to push initialization to remote repository: {}. Consider configuring authentication for automatic pushing.", e);
            } else {
                tracing::info!("Successfully pushed initialization to remote repository");
            }
        } else {
            tracing::debug!("Auto-push is disabled, skipping push to remote");
        }

        Ok(())
    }

    /// Diagnostic method to check git store configuration
    pub fn diagnose_git_config(&self) -> GitStoreDiagnostic {
        let is_writable = self.provider.is_writable();
        let has_write_config = self.provider.write_config.is_some();
        let auth_type = "Unknown".to_string(); // Can't access private auth field

        let auto_push = self
            .provider
            .write_config
            .as_ref()
            .map(|config| config.auto_push);

        GitStoreDiagnostic {
            is_writable,
            has_write_config,
            auth_type,
            auto_push,
            git_url: self.provider.url().to_string(),
        }
    }
}

/// Diagnostic information about git store configuration
#[derive(Debug, Clone)]
pub struct GitStoreDiagnostic {
    pub is_writable: bool,
    pub has_write_config: bool,
    pub auth_type: String,
    pub auto_push: Option<bool>,
    pub git_url: String,
}

impl GitStoreDiagnostic {
    pub fn can_commit_and_push(&self) -> bool {
        self.is_writable && self.auto_push.unwrap_or(false)
    }

    pub fn issues(&self) -> Vec<String> {
        let mut issues = Vec::new();

        if !self.has_write_config {
            issues.push(
                "No GitWriteConfig configured - commits and pushes will be skipped".to_string(),
            );
        }

        if self.has_write_config && !self.auto_push.unwrap_or(false) {
            issues.push(
                "Auto-push is disabled - commits will be made locally but not pushed".to_string(),
            );
        }

        issues
    }

    pub fn recommendations(&self) -> Vec<String> {
        let mut recommendations = Vec::new();

        if !self.has_write_config {
            recommendations
                .push("Add GitWriteConfig with author info and commit template".to_string());
        }

        recommendations
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
        self.provider.sync().await?;

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
    use crate::stores::providers::traits::{Capability, StoreProvider, SyncResult};
    use std::path::Path;
    use tempfile::TempDir;

    // Mock provider for testing
    struct MockProvider {
        sync_dir: PathBuf,
        should_sync: bool,
        changes: Vec<String>,
    }

    impl MockProvider {
        fn new(sync_dir: PathBuf) -> Self {
            Self {
                sync_dir,
                should_sync: true,
                changes: vec![],
            }
        }

        fn with_changes(sync_dir: PathBuf, changes: Vec<String>) -> Self {
            Self {
                sync_dir,
                should_sync: true,
                changes,
            }
        }
    }

    #[async_trait]
    impl StoreProvider for MockProvider {
        fn sync_dir(&self) -> &Path {
            &self.sync_dir
        }

        async fn sync(&self) -> Result<SyncResult> {
            if self.changes.is_empty() {
                Ok(SyncResult::no_changes())
            } else {
                Ok(SyncResult::with_changes(self.changes.clone()))
            }
        }

        async fn needs_sync(&self) -> Result<bool> {
            Ok(self.should_sync)
        }

        fn description(&self) -> String {
            "Mock provider for testing".to_string()
        }

        fn provider_type(&self) -> &'static str {
            "mock"
        }

        fn supports_capability(&self, _capability: Capability) -> bool {
            false // Mock provider is read-only
        }
    }

    #[tokio::test]
    async fn test_locally_cached_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::new(temp_dir.path().to_path_buf());
        let store = LocallyCachedStore::new(provider, "test-store".to_string()).unwrap();

        assert_eq!(store.name, "test-store");
    }

    #[tokio::test]
    async fn test_sync_caching() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::with_changes(
            temp_dir.path().to_path_buf(),
            vec!["file1.json".to_string()],
        );
        let store = LocallyCachedStore::new(provider, "test-store".to_string()).unwrap();

        // First sync should work
        store.ensure_synced().await.unwrap();

        // Second sync should be cached (will skip due to time-based caching)
        store.ensure_synced().await.unwrap();
    }

    #[tokio::test]
    async fn test_initialize_store() {
        let temp_dir = TempDir::new().unwrap();
        let provider = MockProvider::new(temp_dir.path().to_path_buf());
        let store = LocallyCachedStore::new(provider, "test-store".to_string()).unwrap();

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

        let store = LocallyCachedStore::new(provider, "test-git-store".to_string()).unwrap();

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

    #[tokio::test]
    async fn test_provider_write_methods_available() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};
        use crate::stores::providers::traits::{Capability, LifecycleEvent};

        let temp_dir = TempDir::new().unwrap();
        let git_url = "https://github.com/example/store.git";

        let provider = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );

        // Test that the provider capability checking works
        assert!(!provider.supports_capability(Capability::Write)); // No write config, so read-only

        // Test that ensure_writable works (should fail for read-only provider)
        let result = provider.ensure_writable().await;
        assert!(result.is_err());

        // Test that handle_event method exists and can be called
        // (they should do nothing for read-only providers)
        let publish_event = LifecycleEvent::Published {
            extension_id: "test-ext".to_string(),
            version: "1.0.0".to_string(),
        };
        let publish_result = provider.handle_event(publish_event).await;
        assert!(publish_result.is_ok());

        let unpublish_event = LifecycleEvent::Unpublished {
            extension_id: "test-ext".to_string(),
            version: "1.0.0".to_string(),
        };
        let unpublish_result = provider.handle_event(unpublish_event).await;
        assert!(unpublish_result.is_ok());
    }

    #[tokio::test]
    async fn test_git_add_all_error_handling() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};

        let temp_dir = TempDir::new().unwrap();
        let git_url = "https://github.com/example/store.git";

        let provider = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );

        // Test that git_add_all handles non-existent repository gracefully
        let result = provider.git_add_all().await;

        // Should fail because no git repository exists in temp_dir
        assert!(result.is_err());

        // The error should be related to opening the repository
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("repository") || error_msg.contains("not found"));
    }

    #[tokio::test]
    async fn test_system_credential_fallback() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};

        let temp_dir = TempDir::new().unwrap();
        let git_url = "https://github.com/example/store.git";

        // Create provider with GitAuth::None to test system credential fallback
        let provider = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None, // Should use system credentials
        );

        let store = LocallyCachedStore::new(provider, "test-git-store".to_string()).unwrap();

        // Test that the provider is configured to use system credentials
        // We can't test the actual push without a real git repo and credentials,
        // but we can verify the setup doesn't immediately fail
        let provider = store.provider();

        // GitAuth::None should be configured
        // We can't directly access the auth field, but we can test the behavior
        // The provider should be configured to use system credentials

        // The provider should indicate it can handle authentication
        // (will use system credentials when needed)
        assert!(!provider.is_writable()); // No write config set, so not writable yet

        // But if we add write config, it would be writable and use system auth
        let write_config = crate::stores::providers::git::GitWriteConfig {
            author: Some(crate::stores::providers::git::GitAuthor {
                name: "Test Author".to_string(),
                email: "test@example.com".to_string(),
            }),
            commit_style: crate::stores::providers::git::CommitStyle::Default,
            auto_push: true,
        };

        let provider_with_write = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        )
        .with_write_config(write_config);

        assert!(provider_with_write.is_writable());
    }

    #[tokio::test]
    async fn test_git_initialization_commits_and_pushes() {
        use crate::stores::providers::git::{
            GitAuth, GitAuthor, GitProvider, GitReference, GitWriteConfig,
        };
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let git_url = "https://github.com/example/test-store.git";

        let write_config = GitWriteConfig {
            author: Some(GitAuthor {
                name: "Test Author".to_string(),
                email: "test@example.com".to_string(),
            }),
            commit_style: crate::stores::providers::git::CommitStyle::Default,
            auto_push: true,
        };

        let provider = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        )
        .with_write_config(write_config);

        let store = LocallyCachedStore::new(provider, "test-git-store".to_string()).unwrap();

        // Verify the store is writable
        assert!(store.provider().is_writable());

        // Initialize the store - this should attempt to commit and push
        let result = store
            .initialize_store(
                "Test Store With Git".to_string(),
                Some("Testing git workflow during initialization".to_string()),
            )
            .await;

        // The initialization should succeed even if git operations fail
        // (since we don't have a real git repo)
        assert!(result.is_ok());

        // Verify the store.json was created
        let manifest_path = temp_dir.path().join("store.json");
        assert!(manifest_path.exists());

        // Verify the content is correct
        let content = fs::read_to_string(&manifest_path).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(manifest["name"], "Test Store With Git");
        assert_eq!(manifest["store_type"], "git");
        assert_eq!(manifest["url"], git_url);
        assert_eq!(
            manifest["description"],
            "Testing git workflow during initialization"
        );
    }

    #[tokio::test]
    async fn test_git_url_preserved_after_manifest_updates() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let git_url = "https://github.com/example/test-store.git";

        let provider = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );

        let store = LocallyCachedStore::new(provider, "test-git-store".to_string()).unwrap();

        // Initialize the git store
        store
            .initialize_store(
                "Git URL Test Store".to_string(),
                Some("Testing URL preservation".to_string()),
            )
            .await
            .unwrap();

        // Verify initial URL is correct
        let manifest_path = temp_dir.path().join("store.json");
        let content = fs::read_to_string(&manifest_path).unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert_eq!(manifest["url"], git_url);
        assert_eq!(manifest["store_type"], "git");

        // Simulate what happens during publish/unpublish - the local store saves its manifest
        // This should NOT overwrite the git URL with a file:// URL
        store.local_store().save_store_manifest().await.unwrap();

        // Verify URL is still the git URL, not a file:// URL
        let updated_content = fs::read_to_string(&manifest_path).unwrap();
        let updated_manifest: serde_json::Value = serde_json::from_str(&updated_content).unwrap();

        assert_eq!(updated_manifest["url"], git_url);
        assert_eq!(updated_manifest["store_type"], "git");

        // Make sure it's NOT a file:// URL
        let url_str = updated_manifest["url"].as_str().unwrap();
        assert!(
            !url_str.starts_with("file://"),
            "URL should not be a file:// path, got: {}",
            url_str
        );
        assert!(
            url_str.starts_with("https://"),
            "URL should be the original git URL, got: {}",
            url_str
        );
    }

    #[tokio::test]
    async fn test_git_initialization_without_write_config() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};

        let temp_dir = TempDir::new().unwrap();
        let git_url = "https://github.com/example/test-store.git";

        // Create provider WITHOUT write config - this is likely the issue
        let provider = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );

        let store = LocallyCachedStore::new(provider, "test-git-store".to_string()).unwrap();

        // Verify the store is NOT writable (this is the issue)
        assert!(!store.provider().is_writable());

        // Initialize the store - this should NOT attempt to commit and push
        let result = store
            .initialize_store(
                "Test Store No Write".to_string(),
                Some("Testing without write config".to_string()),
            )
            .await;

        // The initialization should still succeed
        assert!(result.is_ok());

        // Verify the store.json was created
        let manifest_path = temp_dir.path().join("store.json");
        assert!(manifest_path.exists());

        // But no git operations should have been attempted
        // (we would need to check logs to verify this)
    }

    #[tokio::test]
    async fn test_git_store_diagnostic() {
        use crate::stores::providers::git::{
            GitAuth, GitAuthor, GitProvider, GitReference, GitWriteConfig,
        };

        let temp_dir = TempDir::new().unwrap();
        let git_url = "https://github.com/example/diagnostic-test.git";

        // Test 1: Store without write config (not writable)
        let provider_no_write = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );

        let store_no_write =
            LocallyCachedStore::new(provider_no_write, "test-git-store-readonly".to_string())
                .unwrap();

        let diagnostic = store_no_write.diagnose_git_config();
        assert!(!diagnostic.is_writable);
        assert!(!diagnostic.has_write_config);
        assert!(!diagnostic.can_commit_and_push());
        assert!(!diagnostic.issues().is_empty());
        assert!(!diagnostic.recommendations().is_empty());

        // Test 2: Store with write config (writable)
        let write_config = GitWriteConfig {
            author: Some(GitAuthor {
                name: "Test Author".to_string(),
                email: "test@example.com".to_string(),
            }),
            commit_style: crate::stores::providers::git::CommitStyle::Default,
            auto_push: true,
        };

        let provider_with_write = GitProvider::new(
            git_url.to_string(),
            temp_dir.path().to_path_buf(),
            GitReference::Default,
            GitAuth::Token {
                token: "test_token".to_string(),
            },
        )
        .with_write_config(write_config);

        let store_with_write =
            LocallyCachedStore::new(provider_with_write, "test-git-store-writable".to_string())
                .unwrap();

        let diagnostic2 = store_with_write.diagnose_git_config();
        assert!(diagnostic2.is_writable);
        assert!(diagnostic2.has_write_config);
        assert!(diagnostic2.can_commit_and_push());
        assert_eq!(diagnostic2.git_url, git_url);
        assert_eq!(diagnostic2.auto_push, Some(true));
    }
}
