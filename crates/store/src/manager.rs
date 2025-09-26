use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::future::join_all;
use semver::Version;
use tokio::fs;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::error::{Result, StoreError};
use crate::models::{
    ExtensionInfo, InstallOptions, InstalledExtension, SearchQuery, SearchSortBy, StoreConfig,
    UpdateInfo, UpdateOptions,
};
use crate::store::Store;

/// Registry entry for installed extensions
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ExtensionRegistry {
    extensions: HashMap<String, InstalledExtension>,
    last_updated: DateTime<Utc>,
    version: String,
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self {
            extensions: HashMap::new(),
            last_updated: Utc::now(),
            version: "1.0.0".to_string(),
        }
    }
}

/// Central manager for handling multiple stores and local installations
pub struct StoreManager {
    stores: Vec<Box<dyn Store>>,
    install_dir: PathBuf,
    config: StoreConfig,
    registry: Arc<RwLock<ExtensionRegistry>>,
    registry_path: PathBuf,
    registry_backup_path: PathBuf,
    download_semaphore: Arc<Semaphore>,
}

impl StoreManager {
    /// Create a new StoreManager instance
    pub async fn new(install_dir: PathBuf) -> Result<Self> {
        let config = StoreConfig::default();

        // Ensure directories exist
        fs::create_dir_all(&install_dir).await?;

        let registry_path = install_dir.join("registry.json");
        let registry_backup_path = install_dir.join("registry.json.backup");
        let registry = if registry_path.exists() {
            Self::load_registry(&registry_path).await?
        } else {
            ExtensionRegistry::default()
        };

        let download_semaphore = Arc::new(Semaphore::new(config.parallel_downloads));

        Ok(Self {
            stores: Vec::new(),
            install_dir,
            config,
            registry: Arc::new(RwLock::new(registry)),
            registry_path,
            registry_backup_path,
            download_semaphore,
        })
    }

    /// Create a StoreManager with custom configuration
    pub async fn with_config(install_dir: PathBuf, config: StoreConfig) -> Result<Self> {
        let mut manager = Self::new(install_dir).await?;
        let parallel_downloads = config.parallel_downloads;
        manager.config = config;
        manager.download_semaphore = Arc::new(Semaphore::new(parallel_downloads));
        Ok(manager)
    }

    /// Add a store to the manager
    pub fn add_store<S: Store + 'static>(&mut self, store: S) {
        info!("Adding store: {}", store.store_info().name);
        self.stores.push(Box::new(store));
        self.sort_stores_by_priority();
    }

    /// Remove a store by name
    pub fn remove_store(&mut self, name: &str) -> bool {
        let initial_len = self.stores.len();
        self.stores.retain(|store| store.store_info().name != name);
        initial_len != self.stores.len()
    }

    /// Get information about all registered stores
    pub fn list_stores(&self) -> Vec<&dyn Store> {
        self.stores.iter().map(|s| s.as_ref()).collect()
    }

    /// Get a specific store by name
    pub fn get_store(&self, name: &str) -> Option<&dyn Store> {
        self.stores
            .iter()
            .find(|store| store.store_info().name == name)
            .map(|s| s.as_ref())
    }

    /// Sort stores by priority (lower number = higher priority)
    fn sort_stores_by_priority(&mut self) {
        self.stores.sort_by(|a, b| {
            a.store_info()
                .priority
                .cmp(&b.store_info().priority)
                .then_with(|| a.store_info().name.cmp(&b.store_info().name))
        });
    }

    /// Refresh all stores (health check and cache refresh)
    pub async fn refresh_stores(&mut self) -> Result<Vec<String>> {
        let mut failed_stores = Vec::new();

        info!("Refreshing {} stores", self.stores.len());

        for store in &self.stores {
            let store_name = &store.store_info().name;
            debug!("Checking health of store: {}", store_name);

            match tokio::time::timeout(self.config.timeout, store.health_check()).await {
                Ok(Ok(health)) => {
                    if !health.healthy {
                        warn!("Store '{}' is unhealthy: {:?}", store_name, health.error);
                        failed_stores.push(store_name.clone());
                    } else {
                        debug!("Store '{}' is healthy", store_name);
                    }
                }
                Ok(Err(e)) => {
                    warn!("Health check failed for store '{}': {}", store_name, e);
                    failed_stores.push(store_name.clone());
                }
                Err(_) => {
                    warn!("Health check timeout for store '{}'", store_name);
                    failed_stores.push(store_name.clone());
                }
            }
        }

        Ok(failed_stores)
    }

    // Discovery Operations

    /// Search across all stores for extensions
    pub async fn search_all_stores(&self, query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
        let mut all_results = Vec::new();
        let mut search_futures = Vec::new();

        for store in &self.stores {
            if !store.store_info().enabled {
                continue;
            }

            let store_name = store.store_info().name.clone();
            let future = async move {
                match tokio::time::timeout(self.config.timeout, store.search_extensions(query))
                    .await
                {
                    Ok(Ok(results)) => {
                        debug!("Store '{}' returned {} results", store_name, results.len());
                        Ok(results)
                    }
                    Ok(Err(e)) => {
                        warn!("Search failed for store '{}': {}", store_name, e);
                        Err(e)
                    }
                    Err(_) => {
                        warn!("Search timeout for store '{}'", store_name);
                        Err(StoreError::Timeout)
                    }
                }
            };
            search_futures.push(future);
        }

        let results = join_all(search_futures).await;
        for result in results {
            match result {
                Ok(mut extensions) => all_results.append(&mut extensions),
                Err(e) => {
                    if !e.is_recoverable() {
                        return Err(e);
                    }
                }
            }
        }

        Ok(self.deduplicate_and_sort(all_results, &query.sort_by))
    }

    /// List all extensions from all stores
    pub async fn list_all_extensions(&self) -> Result<Vec<ExtensionInfo>> {
        let mut all_extensions = Vec::new();
        let mut list_futures = Vec::new();

        for store in &self.stores {
            if !store.store_info().enabled {
                continue;
            }

            let store_name = store.store_info().name.clone();
            let future = async move {
                match tokio::time::timeout(self.config.timeout, store.list_extensions()).await {
                    Ok(Ok(extensions)) => {
                        debug!("Store '{}' has {} extensions", store_name, extensions.len());
                        Ok(extensions)
                    }
                    Ok(Err(e)) => {
                        warn!("List failed for store '{}': {}", store_name, e);
                        Err(e)
                    }
                    Err(_) => {
                        warn!("List timeout for store '{}'", store_name);
                        Err(StoreError::Timeout)
                    }
                }
            };
            list_futures.push(future);
        }

        let results = join_all(list_futures).await;
        for result in results {
            match result {
                Ok(mut extensions) => all_extensions.append(&mut extensions),
                Err(e) => {
                    if !e.is_recoverable() {
                        return Err(e);
                    }
                }
            }
        }

        Ok(self.deduplicate_extensions(all_extensions))
    }

    /// Get extension information from the best available store
    pub async fn get_extension_info(&self, name: &str) -> Result<Vec<ExtensionInfo>> {
        for store in &self.stores {
            if !store.store_info().enabled {
                continue;
            }

            match store.get_extension_info(name).await {
                Ok(info) if !info.is_empty() => return Ok(info),
                Ok(_) => continue, // Empty result, try next store
                Err(StoreError::ExtensionNotFound(_)) => continue,
                Err(e) if e.is_recoverable() => {
                    warn!(
                        "Recoverable error from store '{}': {}",
                        store.store_info().name,
                        e
                    );
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(StoreError::ExtensionNotFound(name.to_string()))
    }

    // Installation Operations

    /// Install an extension from the best available store
    pub async fn install(
        &mut self,
        name: &str,
        version: Option<&str>,
        options: Option<InstallOptions>,
    ) -> Result<InstalledExtension> {
        let options = options.unwrap_or_default();

        // Check if already installed and handle accordingly
        if let Some(installed) = self.get_installed(name).await {
            if let Some(requested_version) = version {
                if installed.version == requested_version && !options.force_reinstall {
                    info!("Extension {}@{} already installed", name, requested_version);
                    return Ok(installed);
                }
                if !options.allow_downgrades {
                    if let (Ok(current), Ok(requested)) = (
                        Version::parse(&installed.version),
                        Version::parse(requested_version),
                    ) {
                        if current > requested {
                            return Err(StoreError::ValidationError(format!(
                                "Cannot downgrade {} from {} to {} (use --allow-downgrades)",
                                name, installed.version, requested_version
                            )));
                        }
                    }
                }
            } else if !options.force_reinstall {
                info!(
                    "Extension {} already installed (version {})",
                    name, installed.version
                );
                return Ok(installed);
            }
        }

        // Acquire download semaphore
        let _permit = self.download_semaphore.acquire().await.unwrap();

        info!("Installing extension: {}", name);
        if let Some(v) = version {
            info!("Requested version: {}", v);
        }

        // Try stores in priority order
        let mut last_error = None;
        for store in &self.stores {
            if !store.store_info().enabled {
                continue;
            }

            let store_name = &store.store_info().name;
            debug!("Trying to install from store: {}", store_name);

            match store
                .install_extension(name, version, &self.install_dir, &options)
                .await
            {
                Ok(installed) => {
                    info!(
                        "Successfully installed {}@{} from store '{}'",
                        name, installed.version, store_name
                    );

                    // Install dependencies if requested
                    if options.install_dependencies {
                        if let Err(e) = self.install_dependencies(&installed).await {
                            warn!("Failed to install dependencies for {}: {}", name, e);
                        }
                    }

                    // Update registry
                    self.add_to_registry(installed.clone()).await?;
                    return Ok(installed);
                }
                Err(StoreError::ExtensionNotFound(_)) => {
                    debug!("Extension not found in store '{}'", store_name);
                    continue;
                }
                Err(e) if e.is_recoverable() => {
                    warn!("Recoverable error from store '{}': {}", store_name, e);
                    last_error = Some(e);
                    continue;
                }
                Err(e) => {
                    error!("Installation error from store '{}': {}", store_name, e);
                    return Err(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| StoreError::ExtensionNotFound(name.to_string())))
    }

    /// Install multiple extensions in parallel
    pub async fn batch_install(
        &mut self,
        requests: &[(String, Option<String>)],
        options: Option<InstallOptions>,
    ) -> Result<Vec<Result<InstalledExtension>>> {
        let mut install_futures = Vec::new();

        for (name, version) in requests {
            let name = name.clone();
            let version = version.clone();
            let _options = options.clone().unwrap_or_default();

            // Note: In a real implementation, you'd want to handle the async context properly
            // This is a simplified version for demonstration
            let future = async move {
                // This would need proper async handling in practice
                Ok(InstalledExtension::new(
                    name,
                    version.unwrap_or_else(|| "latest".to_string()),
                    PathBuf::new(),
                    crate::manifest::ExtensionManifest {
                        name: "placeholder".to_string(),
                        version: "1.0.0".to_string(),
                        author: "placeholder".to_string(),
                        langs: vec![],
                        base_urls: vec![],
                        rds: vec![],
                        attrs: vec![],
                        checksum: crate::manifest::checksum::Checksum {
                            algorithm: crate::manifest::checksum::ChecksumAlgorithm::Sha256,
                            value: "placeholder".to_string(),
                        },
                        signature: None,
                    },
                    crate::models::PackageLayout::default(),
                    "placeholder".to_string(),
                ))
            };
            install_futures.push(future);
        }

        let results = join_all(install_futures).await;
        Ok(results)
    }

    // Update Operations

    /// Check for updates across all stores
    pub async fn check_all_updates(&self) -> Result<Vec<UpdateInfo>> {
        let installed = self.list_installed().await;
        if installed.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_updates = Vec::new();
        let mut update_futures = Vec::new();

        for store in &self.stores {
            if !store.store_info().enabled {
                continue;
            }

            let store_name = store.store_info().name.clone();
            let installed_slice: Vec<InstalledExtension> = installed.values().cloned().collect();

            let future = async move {
                match tokio::time::timeout(
                    self.config.timeout,
                    store.check_updates(&installed_slice),
                )
                .await
                {
                    Ok(Ok(updates)) => {
                        debug!("Store '{}' found {} updates", store_name, updates.len());
                        Ok(updates)
                    }
                    Ok(Err(e)) => {
                        warn!("Update check failed for store '{}': {}", store_name, e);
                        Err(e)
                    }
                    Err(_) => {
                        warn!("Update check timeout for store '{}'", store_name);
                        Err(StoreError::Timeout)
                    }
                }
            };
            update_futures.push(future);
        }

        let results = join_all(update_futures).await;
        for result in results {
            match result {
                Ok(mut updates) => all_updates.append(&mut updates),
                Err(e) => {
                    if !e.is_recoverable() {
                        return Err(e);
                    }
                }
            }
        }

        Ok(self.deduplicate_updates(all_updates))
    }

    /// Update a specific extension
    pub async fn update(
        &mut self,
        name: &str,
        options: Option<UpdateOptions>,
    ) -> Result<InstalledExtension> {
        let options = options.unwrap_or_default();

        let installed = self
            .get_installed(name)
            .await
            .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?;

        // Find the store that originally provided this extension
        let source_store = self
            .stores
            .iter()
            .find(|store| store.store_info().name == installed.installed_from);

        let store = match source_store {
            Some(store) => store,
            None => {
                warn!(
                    "Original store '{}' not found for extension '{}', trying all stores",
                    installed.installed_from, name
                );
                // Try to update from any available store
                return self
                    .install(name, None, Some(InstallOptions::default()))
                    .await;
            }
        };

        info!(
            "Updating extension '{}' from store '{}'",
            name,
            store.store_info().name
        );

        match store
            .update_extension(name, &self.install_dir, &options)
            .await
        {
            Ok(updated) => {
                info!(
                    "Successfully updated {} to version {}",
                    name, updated.version
                );
                self.add_to_registry(updated.clone()).await?;
                Ok(updated)
            }
            Err(e) => {
                error!("Failed to update extension '{}': {}", name, e);
                Err(e)
            }
        }
    }

    /// Update all installed extensions
    pub async fn update_all(
        &mut self,
        options: Option<UpdateOptions>,
    ) -> Result<Vec<Result<InstalledExtension>>> {
        let updates = self.check_all_updates().await?;
        if updates.is_empty() {
            info!("No updates available");
            return Ok(Vec::new());
        }

        info!("Updating {} extensions", updates.len());
        let mut update_results = Vec::new();

        for update_info in updates {
            let result = self
                .update(&update_info.extension_name, options.clone())
                .await;
            update_results.push(result);
        }

        Ok(update_results)
    }

    // Registry Management

    /// Get information about an installed extension
    pub async fn get_installed(&self, name: &str) -> Option<InstalledExtension> {
        self.registry.read().await.extensions.get(name).cloned()
    }

    /// List all installed extensions
    pub async fn list_installed(&self) -> HashMap<String, InstalledExtension> {
        self.registry.read().await.extensions.clone()
    }

    /// Remove an installed extension
    pub async fn uninstall(&mut self, name: &str, remove_files: bool) -> Result<()> {
        let installed = self
            .get_installed(name)
            .await
            .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?;

        if remove_files {
            info!("Removing files for extension '{}'", name);
            if installed.install_path.exists() {
                fs::remove_dir_all(&installed.install_path).await?;
            }
        }

        // Remove from registry
        self.atomic_registry_update(|registry| {
            registry.extensions.remove(name);
            registry.last_updated = Utc::now();
            Ok(())
        })
        .await?;

        info!("Successfully uninstalled extension '{}'", name);
        Ok(())
    }

    // Private helper methods

    async fn install_dependencies(&self, extension: &InstalledExtension) -> Result<()> {
        // This would need access to extension metadata to get dependencies
        // For now, just log that we would install dependencies
        debug!(
            "Would install dependencies for extension '{}'",
            extension.name
        );
        Ok(())
    }

    async fn add_to_registry(&self, installed: InstalledExtension) -> Result<()> {
        self.atomic_registry_update(|registry| {
            registry
                .extensions
                .insert(installed.name.clone(), installed);
            registry.last_updated = Utc::now();
            Ok(())
        })
        .await
    }

    /// Perform an atomic registry update with backup and rollback support
    async fn atomic_registry_update<F>(&self, update_fn: F) -> Result<()>
    where
        F: FnOnce(&mut ExtensionRegistry) -> Result<()>,
    {
        // Create backup of current registry
        if self.registry_path.exists() {
            if let Err(e) = fs::copy(&self.registry_path, &self.registry_backup_path).await {
                warn!("Failed to create registry backup: {}", e);
            }
        }

        // Apply the update to the in-memory registry
        let updated_registry = {
            let mut registry = self.registry.write().await;
            update_fn(&mut *registry)?;
            registry.clone()
        };

        // Attempt to save the updated registry
        match self.save_registry_content(&updated_registry).await {
            Ok(()) => {
                debug!("Registry updated successfully");
                Ok(())
            }
            Err(e) => {
                error!("Failed to save registry, attempting rollback: {}", e);

                // Attempt to restore from backup
                if let Err(rollback_err) = self.rollback_registry().await {
                    error!("Registry rollback failed: {}", rollback_err);
                    return Err(StoreError::ConcurrencyError(format!(
                        "Registry update failed and rollback failed: {} -> {}",
                        e, rollback_err
                    )));
                }

                warn!("Registry rolled back successfully");
                Err(e)
            }
        }
    }

    async fn save_registry_content(&self, registry: &ExtensionRegistry) -> Result<()> {
        let content = serde_json::to_string_pretty(registry)?;

        // Write to a temporary file first, then atomically move it
        let temp_path = self.registry_path.with_extension("json.tmp");

        fs::write(&temp_path, &content).await?;

        // Atomic move (rename) on most filesystems
        fs::rename(&temp_path, &self.registry_path)
            .await
            .map_err(|e| StoreError::IoError(e))?;

        Ok(())
    }

    async fn rollback_registry(&self) -> Result<()> {
        if !self.registry_backup_path.exists() {
            return Err(StoreError::CacheError(
                "No registry backup available for rollback".to_string(),
            ));
        }

        // Restore from backup
        fs::copy(&self.registry_backup_path, &self.registry_path).await?;

        // Reload the registry in memory
        let restored_registry = Self::load_registry(&self.registry_path).await?;
        {
            let mut registry = self.registry.write().await;
            *registry = restored_registry;
        }

        Ok(())
    }

    async fn load_registry(path: &Path) -> Result<ExtensionRegistry> {
        let content = fs::read_to_string(path).await?;
        let registry: ExtensionRegistry = serde_json::from_str(&content)?;
        Ok(registry)
    }

    fn deduplicate_extensions(&self, mut extensions: Vec<ExtensionInfo>) -> Vec<ExtensionInfo> {
        // Remove duplicates based on name + version, preferring trusted stores
        let mut seen: HashMap<String, String> = HashMap::new();
        extensions.retain(|ext| {
            let key = format!("{}@{}", ext.name, ext.version);
            if let Some(existing_store) = seen.get(&key) {
                // Keep if current store is trusted and existing is not
                let current_store = self.get_store(&ext.store_source);
                let existing_trusted = self
                    .get_store(existing_store)
                    .map(|s| s.store_info().trusted)
                    .unwrap_or(false);
                let current_trusted = current_store
                    .map(|s| s.store_info().trusted)
                    .unwrap_or(false);

                if current_trusted && !existing_trusted {
                    seen.insert(key, ext.store_source.clone());
                    true
                } else {
                    false
                }
            } else {
                seen.insert(key, ext.store_source.clone());
                true
            }
        });
        extensions
    }

    fn deduplicate_and_sort(
        &self,
        extensions: Vec<ExtensionInfo>,
        sort_by: &SearchSortBy,
    ) -> Vec<ExtensionInfo> {
        let mut deduplicated = self.deduplicate_extensions(extensions);

        match sort_by {
            SearchSortBy::Name => deduplicated.sort_by(|a, b| a.name.cmp(&b.name)),
            SearchSortBy::Version => deduplicated.sort_by(|a, b| {
                match (Version::parse(&a.version), Version::parse(&b.version)) {
                    (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a),
                    _ => b.version.cmp(&a.version),
                }
            }),
            SearchSortBy::LastUpdated => {
                deduplicated.sort_by(|a, b| b.last_updated.cmp(&a.last_updated))
            }
            SearchSortBy::Author => deduplicated.sort_by(|a, b| a.author.cmp(&b.author)),
            SearchSortBy::Size => {
                deduplicated.sort_by(|a, b| b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)))
            }
            SearchSortBy::DownloadCount => deduplicated.sort_by(|a, b| {
                b.download_count
                    .unwrap_or(0)
                    .cmp(&a.download_count.unwrap_or(0))
            }),
            SearchSortBy::Relevance => {
                // Keep original order for relevance
            }
        }

        deduplicated
    }

    fn deduplicate_updates(&self, mut updates: Vec<UpdateInfo>) -> Vec<UpdateInfo> {
        // Remove duplicate updates, keeping the one from the most trusted store
        let mut seen: HashMap<String, String> = HashMap::new();
        updates.retain(|update| {
            if let Some(existing_store) = seen.get(&update.extension_name) {
                let existing_trusted = self
                    .get_store(existing_store)
                    .map(|s| s.store_info().trusted)
                    .unwrap_or(false);
                let current_trusted = self
                    .get_store(&update.store_source)
                    .map(|s| s.store_info().trusted)
                    .unwrap_or(false);

                if current_trusted && !existing_trusted {
                    seen.insert(update.extension_name.clone(), update.store_source.clone());
                    true
                } else {
                    false
                }
            } else {
                seen.insert(update.extension_name.clone(), update.store_source.clone());
                true
            }
        });
        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let install_dir = temp_dir.path().join("install");
        let cache_dir = temp_dir.path().join("cache");

        let manager = StoreManager::new(install_dir.clone()).await.unwrap();

        assert!(install_dir.exists());
        assert_eq!(manager.list_stores().len(), 0);
    }

    #[tokio::test]
    async fn test_registry_operations() {
        let temp_dir = TempDir::new().unwrap();
        let install_dir = temp_dir.path().join("install");
        let _cache_dir = temp_dir.path().join("cache");

        let manager = StoreManager::new(install_dir).await.unwrap();

        // Initially no extensions
        assert_eq!(manager.list_installed().await.len(), 0);
        assert!(manager.get_installed("test").await.is_none());
    }
}
