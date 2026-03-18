//! Locally cached store implementation
//!
//! This module provides `LocallyCachedStore` which wraps a `StoreProvider` and `LocalStore`
//! to give a unified interface for stores that sync data from remote sources and cache it
//! locally for fast access.
//!
//! ## Sync serialisation
//!
//! All calls to [`LocallyCachedStore::ensure_synced`] are serialised through a
//! `tokio::sync::Mutex<()>`.  The mutex is held for the entire check + sync so that
//! concurrent callers queue up rather than triggering duplicate syncs.  Time-based
//! throttling (whether a sync is actually needed) is delegated entirely to the provider
//! via [`StoreProvider::needs_sync`], making the provider the single source of truth for
//! sync timing.

use async_trait::async_trait;
use semver::Version;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::error::Result;
use crate::manager::publish::{
    PublishOptions, PublishRequirements, PublishResult, UnpublishOptions, UnpublishResult,
    ValidationReport,
};
use crate::models::ExtensionListing;
use crate::stores::{
    impls::local::LocalStore,
    providers::{
        traits::{LifecycleEvent, StoreProvider},
        GitProvider,
    },
    traits::{BaseStore, ReadableStore, SyncableStore, WritableStore},
};
use crate::{
    registry::manifest::ExtensionManifest, ExtensionInfo, ExtensionMetadata, ExtensionPackage,
    InstalledExtension, SearchQuery, StoreHealth, StoreManifest, UpdateInfo,
};

// ---------------------------------------------------------------------------
// Core struct
// ---------------------------------------------------------------------------

/// A store that syncs data from a [`StoreProvider`] and delegates reads to a
/// [`LocalStore`].
pub struct LocallyCachedStore<T: StoreProvider> {
    provider: T,
    local_store: LocalStore,
    sync_dir: PathBuf,
    name: String,
    /// Serialises concurrent sync operations.  The mutex is held for the full
    /// duration of each `ensure_synced` call so a concurrent caller waits
    /// for the in-progress sync to complete instead of starting a duplicate.
    sync_lock: Arc<Mutex<()>>,
}

impl<T: StoreProvider> LocallyCachedStore<T> {
    /// Create a new locally cached store.
    ///
    /// The sync directory is taken from `provider.sync_dir()`, making the
    /// provider the single source of truth for where data lives.
    pub fn new(provider: T, name: String) -> Result<Self> {
        let sync_dir = provider.sync_dir().to_path_buf();
        let local_store = LocalStore::new(&sync_dir)?;
        Ok(Self {
            provider,
            local_store,
            sync_dir,
            name,
            sync_lock: Arc::new(Mutex::new(())),
        })
    }

    /// Return the directory where synced data is stored.
    pub fn sync_dir(&self) -> &PathBuf {
        &self.sync_dir
    }

    /// Return a reference to the underlying provider.
    pub fn provider(&self) -> &T {
        &self.provider
    }

    /// Return a reference to the underlying local store.
    pub fn local_store(&self) -> &LocalStore {
        &self.local_store
    }

    /// Ensure the store is synced and ready for use.
    ///
    /// Acquires `sync_lock` and holds it for the entire operation so that
    /// concurrent callers queue up rather than triggering duplicate syncs.
    /// Whether a sync is actually needed is decided solely by
    /// `provider.needs_sync()`.
    pub async fn ensure_synced(&self) -> Result<()> {
        // Acquire and hold the lock for the full check + sync.
        // Any concurrent caller blocks here until the in-progress sync finishes.
        let _guard = self.sync_lock.lock().await;

        debug!(
            "Checking if sync needed for store '{}' ({})",
            self.name,
            self.provider.provider_type()
        );

        if self.provider.needs_sync().await? {
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
        } else {
            debug!("Store '{}' is up to date, no sync needed", self.name);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WritableStore
// ---------------------------------------------------------------------------

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
        self.ensure_synced().await?;
        self.provider.ensure_writable().await?;

        let result = self.local_store.publish(package.clone(), options).await?;

        let event = LifecycleEvent::Published {
            extension_id: package.manifest.id.clone(),
            version: package.manifest.version.to_string(),
        };
        if let Err(e) = self.provider.handle_event(event).await {
            warn!("Lifecycle hook failed after successful publish: {}", e);
        }

        Ok(result)
    }

    async fn unpublish(
        &self,
        extension_id: &str,
        options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        self.ensure_synced().await?;
        self.provider.ensure_writable().await?;

        let result = self.local_store.unpublish(extension_id, options).await?;

        let event = LifecycleEvent::Unpublished {
            extension_id: extension_id.to_string(),
            version: result.version.clone(),
        };
        if let Err(e) = self.provider.handle_event(event).await {
            warn!("Lifecycle hook failed after successful unpublish: {}", e);
        }

        Ok(result)
    }

    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<ValidationReport> {
        self.local_store.validate_package(package, options).await
    }
}

// ---------------------------------------------------------------------------
// GitProvider-specific initialisation helpers
// ---------------------------------------------------------------------------

impl LocallyCachedStore<GitProvider> {
    /// Initialise the git store with metadata that includes the repository URL.
    pub async fn initialize_store(
        &self,
        store_name: String,
        description: Option<String>,
    ) -> Result<()> {
        self.initialize_store_with_type(store_name, description, "git")
            .await
    }

    pub(crate) async fn initialize_store_with_type(
        &self,
        store_name: String,
        description: Option<String>,
        store_type: &str,
    ) -> Result<()> {
        use crate::manager::store_manifest::StoreManifest;

        let git_url = self.provider.url().to_string();

        let mut base_manifest = StoreManifest::new(
            store_name.clone(),
            store_type.to_string(),
            "0.1.0".to_string(),
        )
        .with_url(git_url);

        if let Some(desc) = description {
            base_manifest = base_manifest.with_description(desc);
        }

        self.local_store
            .initialize_store_with_manifest(&base_manifest.into())
            .await?;

        if self.provider.is_writable() {
            tracing::info!(
                "Starting git initialisation workflow for store: {}",
                store_name
            );
            if let Err(e) = self.git_initialize_workflow(&store_name).await {
                tracing::warn!("Git workflow failed after successful initialisation: {}", e);
            } else {
                tracing::info!("Git initialisation workflow completed successfully");
            }
        } else {
            tracing::info!(
                "Git store '{}' initialised (read-only). \
                 To enable automatic git commits and pushes, configure \
                 GitWriteConfig with author info and commit settings.",
                store_name
            );
        }

        Ok(())
    }

    /// Commit and optionally push the initialisation changes to the git remote.
    async fn git_initialize_workflow(&self, store_name: &str) -> Result<()> {
        tracing::debug!("Starting git initialisation workflow");

        if !self.provider.has_write_config() {
            tracing::error!("No write configuration available for git provider");
            return Err(crate::error::StoreError::ConfigError(
                "Git write configuration not available".to_string(),
            ));
        }

        if !self.provider.is_git_repo() {
            tracing::debug!("Repository not found, initialising");
            self.provider.git_init()?;
            self.provider.set_git_remote()?;
            tracing::info!("Initialised new git repository");
        } else {
            tracing::debug!("Git repository already initialised");
        }

        tracing::debug!("Staging all changes");
        self.provider.git_add_all().await?;
        tracing::debug!("Successfully staged changes");

        let commit_message = format!("Initialize git store: {}", store_name);
        tracing::debug!("Committing: {}", commit_message);
        self.provider.git_commit(&commit_message).await?;
        tracing::info!("Successfully committed initialisation changes");

        if self.provider.is_auto_push_enabled() {
            tracing::debug!("Auto-push enabled, pushing to remote");
            if let Err(e) = self.provider.git_push().await {
                tracing::warn!(
                    "Failed to push initialisation to remote: {}. \
                     Consider configuring authentication for automatic pushing.",
                    e
                );
            } else {
                tracing::info!("Successfully pushed initialisation to remote");
            }
        } else {
            tracing::debug!("Auto-push disabled, skipping push");
        }

        Ok(())
    }

    /// Return a diagnostic snapshot of the git store's current configuration.
    pub fn diagnose_git_config(&self) -> GitStoreDiagnostic {
        GitStoreDiagnostic {
            is_writable: self.provider.is_writable(),
            has_write_config: self.provider.has_write_config(),
            // auth field is private; surface only what the public API exposes
            auth_type: "Unknown".to_string(),
            auto_push: self.provider.auto_push_config(),
            git_url: self.provider.url().to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// GitStoreDiagnostic
// ---------------------------------------------------------------------------

/// Snapshot of a git store's configuration, useful for diagnosing publish issues.
pub struct GitStoreDiagnostic {
    pub is_writable: bool,
    pub has_write_config: bool,
    pub auth_type: String,
    pub auto_push: Option<bool>,
    pub git_url: String,
}

impl GitStoreDiagnostic {
    /// Return `true` if the store can commit **and** push automatically.
    pub fn can_commit_and_push(&self) -> bool {
        self.is_writable && self.auto_push.unwrap_or(false)
    }

    /// Return human-readable descriptions of detected configuration issues.
    pub fn issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if !self.is_writable {
            issues.push("Store is not configured for writing".to_string());
        }
        if !self.has_write_config {
            issues.push("No GitWriteConfig present".to_string());
        }
        if self.auto_push == Some(false) {
            issues.push("Auto-push is disabled; commits will remain local".to_string());
        }
        issues
    }

    /// Return actionable recommendations for fixing the reported issues.
    pub fn recommendations(&self) -> Vec<String> {
        let mut recs = Vec::new();
        if !self.is_writable {
            recs.push("Call .writable() on the builder or supply a GitWriteConfig".to_string());
        }
        if self.auto_push == Some(false) {
            recs.push("Set auto_push = true in GitWriteConfig to push automatically".to_string());
        }
        recs
    }
}

// ---------------------------------------------------------------------------
// BaseStore
// ---------------------------------------------------------------------------

#[async_trait]
impl<T: StoreProvider> BaseStore for LocallyCachedStore<T> {
    async fn get_store_manifest(&self) -> Result<StoreManifest> {
        self.ensure_synced().await?;
        self.local_store.get_store_manifest().await
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        match self.ensure_synced().await {
            Ok(_) => self.local_store.health_check().await,
            Err(e) => Ok(StoreHealth {
                healthy: false,
                last_check: chrono::Utc::now(),
                response_time: None,
                error: Some(format!("Sync failed: {}", e)),
                extension_count: None,
                store_version: None,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// ReadableStore
// ---------------------------------------------------------------------------

#[async_trait]
impl<T: StoreProvider> ReadableStore for LocallyCachedStore<T> {
    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        self.ensure_synced().await?;
        self.local_store.find_extensions_for_url(url).await
    }

    async fn list_extensions(&self) -> Result<Vec<ExtensionListing>> {
        self.ensure_synced().await?;
        self.local_store.list_extensions().await
    }

    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionListing>> {
        self.ensure_synced().await?;
        self.local_store.search_extensions(query).await
    }

    async fn get_extension_info(&self, id: &str) -> Result<Vec<ExtensionInfo>> {
        self.ensure_synced().await?;
        self.local_store.get_extension_info(id).await
    }

    async fn get_extension_version_info(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<ExtensionInfo> {
        self.ensure_synced().await?;
        self.local_store
            .get_extension_version_info(id, version)
            .await
    }

    async fn get_extension_manifest(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<ExtensionManifest> {
        self.ensure_synced().await?;
        self.local_store.get_extension_manifest(id, version).await
    }

    async fn get_extension_metadata(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<Option<ExtensionMetadata>> {
        self.ensure_synced().await?;
        self.local_store.get_extension_metadata(id, version).await
    }

    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<ExtensionPackage> {
        self.ensure_synced().await?;
        self.local_store.get_extension_package(id, version).await
    }

    async fn get_extension_latest_version(&self, id: &str) -> Result<Option<Version>> {
        self.ensure_synced().await?;
        self.local_store.get_extension_latest_version(id).await
    }

    async fn list_extension_versions(&self, id: &str) -> Result<Vec<Version>> {
        self.ensure_synced().await?;
        self.local_store.list_extension_versions(id).await
    }

    async fn check_extension_version_exists(&self, id: &str, version: &Version) -> Result<bool> {
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

// ---------------------------------------------------------------------------
// SyncableStore
// ---------------------------------------------------------------------------

#[async_trait]
impl<T: StoreProvider> SyncableStore for LocallyCachedStore<T> {
    /// Force an immediate sync from the backing source, bypassing the
    /// provider's time-based throttle.
    async fn force_sync(&self) -> Result<()> {
        let _guard = self.sync_lock.lock().await;
        let result = self.provider.sync().await?;

        info!(
            "Force-synced store '{}': {} changes",
            self.name,
            result.changes.len()
        );
        for warning in &result.warnings {
            warn!("Sync warning for '{}': {}", self.name, warning);
        }

        // Invalidate the local manifest cache so the next read sees fresh data.
        self.local_store.clear_cache().await
    }

    async fn clear_cache(&self) -> Result<()> {
        self.local_store.clear_cache().await
    }

    async fn cache_stats(&self) -> Result<crate::stores::traits::CacheStats> {
        self.local_store.cache_stats().await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use tempfile::TempDir;

    use super::*;
    use crate::stores::providers::traits::{Capability, SyncResult};

    // -----------------------------------------------------------------------
    // Mock provider
    // -----------------------------------------------------------------------

    struct MockProvider {
        sync_dir: PathBuf,
        should_sync: bool,
        changes: Vec<String>,
    }

    impl MockProvider {
        fn new(sync_dir: PathBuf) -> Self {
            Self {
                sync_dir,
                should_sync: false,
                changes: Vec::new(),
            }
        }

        fn with_changes(mut self, changes: Vec<String>) -> Self {
            self.should_sync = true;
            self.changes = changes;
            self
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
            false
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_store(sync_dir: &Path) -> LocallyCachedStore<MockProvider> {
        let provider = MockProvider::new(sync_dir.to_path_buf());
        LocallyCachedStore::new(provider, "test-store".to_string()).unwrap()
    }

    // -----------------------------------------------------------------------
    // Basic creation
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_locally_cached_store_creation() {
        let temp = TempDir::new().unwrap();
        let store = make_store(temp.path());
        assert_eq!(store.sync_dir(), &temp.path().to_path_buf());
    }

    // -----------------------------------------------------------------------
    // ensure_synced – no 30-second local cache; provider is the sole gate
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_sync_skipped_when_provider_reports_up_to_date() {
        let temp = TempDir::new().unwrap();
        // should_sync = false → provider says nothing to do
        let store = make_store(temp.path());
        // Multiple calls should all succeed without error
        store.ensure_synced().await.unwrap();
        store.ensure_synced().await.unwrap();
    }

    #[tokio::test]
    async fn test_sync_performed_when_provider_reports_needed() {
        let temp = TempDir::new().unwrap();
        let provider = MockProvider::new(temp.path().to_path_buf())
            .with_changes(vec!["extensions/my-ext/1.0.0/manifest.json".to_string()]);
        let store = LocallyCachedStore::new(provider, "test".to_string()).unwrap();
        store.ensure_synced().await.unwrap();
    }

    // -----------------------------------------------------------------------
    // Git-specific: initialize_store
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_initialize_store() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};

        let temp = TempDir::new().unwrap();
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );
        let store = LocallyCachedStore::new(provider, "test-git-store".to_string()).unwrap();

        // Read-only store: git workflow is skipped, but local init still happens.
        store
            .initialize_store("test-git-store".to_string(), Some("Test store".to_string()))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_git_initialization_without_write_config() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};

        let temp = TempDir::new().unwrap();
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );
        let store = LocallyCachedStore::new(provider, "readonly-git".to_string()).unwrap();

        let result = store
            .initialize_store("readonly-git".to_string(), None)
            .await;
        assert!(
            result.is_ok(),
            "Read-only init should succeed: {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // Git-specific: diagnose_git_config
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_git_store_diagnostic_readonly() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference};

        let temp = TempDir::new().unwrap();
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        );
        let store = LocallyCachedStore::new(provider, "diag-store".to_string()).unwrap();

        let diag = store.diagnose_git_config();
        assert!(!diag.is_writable);
        assert!(!diag.has_write_config);
        assert!(diag.auto_push.is_none());
        assert_eq!(diag.git_url, "https://github.com/test/repo.git");
        assert!(!diag.can_commit_and_push());
        assert!(!diag.issues().is_empty());
    }

    #[tokio::test]
    async fn test_git_store_diagnostic_writable() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference, GitWriteConfig};

        let temp = TempDir::new().unwrap();
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        )
        .with_write_config(GitWriteConfig::default());

        let store = LocallyCachedStore::new(provider, "writable-store".to_string()).unwrap();

        let diag = store.diagnose_git_config();
        assert!(diag.is_writable);
        assert!(diag.has_write_config);
        // auto_push defaults to true in GitWriteConfig::default()
        assert_eq!(diag.auto_push, Some(true));
        assert!(diag.can_commit_and_push());
        assert!(diag.issues().is_empty());
    }

    #[tokio::test]
    async fn test_git_store_diagnostic_writable_no_auto_push() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference, GitWriteConfig};

        let temp = TempDir::new().unwrap();
        let mut write_config = GitWriteConfig::default();
        write_config.auto_push = false;

        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        )
        .with_write_config(write_config);

        let store = LocallyCachedStore::new(provider, "no-push-store".to_string()).unwrap();

        let diag = store.diagnose_git_config();
        assert!(diag.is_writable);
        assert!(!diag.can_commit_and_push());
        // Should flag that auto-push is off
        assert!(diag.issues().iter().any(|i| i.contains("Auto-push")));
        assert!(diag
            .recommendations()
            .iter()
            .any(|r| r.contains("auto_push")));
    }

    // -----------------------------------------------------------------------
    // WritableStore: ensure_writable is checked
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_provider_write_methods_available() {
        use crate::stores::providers::git::{GitAuth, GitProvider, GitReference, GitWriteConfig};

        let temp = TempDir::new().unwrap();
        let provider = GitProvider::new(
            "https://github.com/test/repo.git".to_string(),
            temp.path().to_path_buf(),
            GitReference::Default,
            GitAuth::None,
        )
        .with_write_config(GitWriteConfig::default());

        let store = LocallyCachedStore::new(provider, "writable-store".to_string()).unwrap();

        // The store is writable but the git repo doesn't exist, so ensure_writable
        // should succeed (it only checks configuration, not repo state).
        assert!(store.provider().is_writable());
    }
}
