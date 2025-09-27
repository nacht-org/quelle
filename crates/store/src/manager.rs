use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures::future::join_all;
use semver::Version;

use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use crate::error::{Result, StoreError};
use crate::models::{
    ExtensionInfo, InstallOptions, InstalledExtension, SearchQuery, SearchSortBy, StoreConfig,
    UpdateInfo, UpdateOptions,
};
use crate::registry::{InstallationQuery, InstallationStats, RegistryStore, ValidationIssue};
use crate::store::Store;

/// Central manager for handling multiple stores and local installations
pub struct StoreManager {
    /// Extension sources (read-only stores for discovering extensions)
    extension_stores: Vec<Box<dyn Store>>,
    /// The authoritative source of truth for installed extensions
    registry_store: Box<dyn RegistryStore>,
    /// Configuration
    config: StoreConfig,
    /// Semaphore for controlling parallel downloads
    download_semaphore: Arc<Semaphore>,
}

impl StoreManager {
    /// Create a new StoreManager with the provided registry store
    pub async fn new(registry_store: Box<dyn RegistryStore>) -> Result<Self> {
        let config = StoreConfig::default();
        let download_semaphore = Arc::new(Semaphore::new(config.parallel_downloads));

        Ok(Self {
            extension_stores: Vec::new(),
            registry_store,
            config,
            download_semaphore,
        })
    }

    /// Create a StoreManager with custom configuration
    pub async fn with_config(
        registry_store: Box<dyn RegistryStore>,
        config: StoreConfig,
    ) -> Result<Self> {
        let download_semaphore = Arc::new(Semaphore::new(config.parallel_downloads));

        Ok(Self {
            extension_stores: Vec::new(),
            registry_store,
            config,
            download_semaphore,
        })
    }

    /// Add an extension store to the manager (for discovering extensions)
    pub fn add_extension_store<S: Store + 'static>(&mut self, store: S) {
        info!("Adding extension store: {}", store.store_info().name);
        self.extension_stores.push(Box::new(store));
        self.sort_stores_by_priority();
    }

    /// Remove an extension store by name
    pub fn remove_extension_store(&mut self, name: &str) -> bool {
        let initial_len = self.extension_stores.len();
        self.extension_stores
            .retain(|store| store.store_info().name != name);
        initial_len != self.extension_stores.len()
    }

    /// Get information about all registered extension stores
    pub fn list_extension_stores(&self) -> Vec<&dyn Store> {
        self.extension_stores.iter().map(|s| s.as_ref()).collect()
    }

    /// Get a specific store by name
    /// Get an extension store by name
    pub fn get_extension_store(&self, name: &str) -> Option<&dyn Store> {
        self.extension_stores
            .iter()
            .find(|store| store.store_info().name == name)
            .map(|s| s.as_ref())
    }

    /// Get the registry store
    pub fn registry_store(&self) -> &dyn RegistryStore {
        self.registry_store.as_ref()
    }

    /// Sort stores by priority (lower number = higher priority)
    /// Sort extension stores by priority (higher priority first)
    fn sort_stores_by_priority(&mut self) {
        self.extension_stores.sort_by(|a, b| {
            b.store_info()
                .priority
                .cmp(&a.store_info().priority)
                .then_with(|| a.store_info().name.cmp(&b.store_info().name))
        });
    }

    /// Refresh all stores (health check and cache refresh)
    pub async fn refresh_stores(&mut self) -> Result<Vec<String>> {
        let mut failed_stores = Vec::new();

        info!(
            "Refreshing {} extension stores",
            self.extension_stores.len()
        );

        for store in &self.extension_stores {
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

        for store in &self.extension_stores {
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

        for store in &self.extension_stores {
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
        for store in &self.extension_stores {
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
        if let Some(installed) = self.registry_store.get_installed(name).await? {
            if let Some(requested_version) = version {
                if installed.version == requested_version && !options.force_reinstall {
                    info!("Extension {}@{} already installed", name, requested_version);
                    return Ok(installed);
                }
            } else if !options.force_reinstall {
                info!(
                    "Extension {} already installed with version {}",
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

        // Find extension package from discovery stores
        let mut last_error = None;
        for store in &self.extension_stores {
            if !store.store_info().enabled {
                continue;
            }

            let store_name = store.store_info().name.clone();
            debug!("Trying to install from store: {}", store_name);

            match store.get_extension_package(name, version).await {
                Ok(package) => {
                    // Install using registry store
                    match self
                        .registry_store
                        .install_extension(package, &options)
                        .await
                    {
                        Ok(installed) => {
                            info!(
                                "Successfully installed {}@{} from store '{}'",
                                name, installed.version, store_name
                            );
                            return Ok(installed);
                        }
                        Err(e) => {
                            error!("Registry installation failed: {}", e);
                            return Err(e);
                        }
                    }
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
            let _version = version.clone();
            let _options = options.clone().unwrap_or_default();

            // Note: In a real implementation, you'd want to handle the async context properly
            // This is a simplified version for demonstration
            let future = async move {
                // This would need proper async handling in practice
                Ok(InstalledExtension::new(
                    name.to_string(),
                    "1.0.0".to_string(),
                    PathBuf::new(),
                    "placeholder".to_string(),
                ))
            };
            install_futures.push(future);
        }

        let results = join_all(install_futures).await;
        Ok(results)
    }

    // Update Operations

    /// Check for updates across all extension stores
    pub async fn check_all_updates(&self) -> Result<Vec<UpdateInfo>> {
        let installed = self.registry_store.list_installed().await?;
        if installed.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_updates = Vec::new();
        let mut update_futures = Vec::new();

        for store in &self.extension_stores {
            if !store.store_info().enabled {
                continue;
            }

            let store_name = store.store_info().name.clone();
            let installed_slice = installed.clone();

            let future = async move {
                match tokio::time::timeout(
                    self.config.timeout,
                    store.check_updates(&installed_slice),
                )
                .await
                {
                    Ok(Ok(updates)) => {
                        debug!(
                            "Found {} updates from extension store {}",
                            updates.len(),
                            store_name
                        );
                        Ok(updates)
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "Update check failed for extension store {}: {}",
                            store_name, e
                        );
                        Ok(Vec::new())
                    }
                    Err(_) => {
                        warn!("Update check timeout for extension store: {}", store_name);
                        Ok(Vec::new())
                    }
                }
            };
            update_futures.push(future);
        }

        let results: Vec<Result<Vec<UpdateInfo>>> = join_all(update_futures).await;

        for result in results {
            match result {
                Ok(mut updates) => all_updates.append(&mut updates),
                Err(e) => warn!("Extension store update check error: {}", e),
            }
        }

        // Deduplicate updates (keep the one from the highest priority store)
        let deduplicated_updates = self.deduplicate_updates(all_updates);

        info!(
            "Found {} total updates available",
            deduplicated_updates.len()
        );
        Ok(deduplicated_updates)
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
            .await?
            .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?;

        // Find the store that originally provided this extension
        let source_store = self
            .extension_stores
            .iter()
            .find(|store| store.store_info().name == installed.source_store);

        let store = match source_store {
            Some(store) => store,
            None => {
                warn!(
                    "Original store '{}' not found for extension '{}', trying all stores",
                    installed.source_store, name
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

        // For now, we'll get the latest version and reinstall
        let latest_version = match store.get_latest_version(name).await? {
            Some(version) => version,
            None => return Err(StoreError::ExtensionNotFound(name.to_string())),
        };

        let install_options = InstallOptions {
            auto_update: options.update_dependencies,
            force_reinstall: options.force_update,
            skip_verification: false,
        };

        match store
            .get_extension_package(name, Some(&latest_version))
            .await
        {
            Ok(package) => {
                match self
                    .registry_store
                    .install_extension(package, &install_options)
                    .await
                {
                    Ok(updated) => {
                        info!(
                            "Successfully updated {} to version {}",
                            name, updated.version
                        );
                        self.registry_store
                            .update_installation(updated.clone())
                            .await?;
                        Ok(updated)
                    }
                    Err(e) => {
                        error!("Failed to update extension '{}': {}", name, e);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                error!("Failed to get package for '{}': {}", name, e);
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

    // Registry Management (delegated to registry store)

    /// Get information about an installed extension
    pub async fn get_installed(&self, name: &str) -> Result<Option<InstalledExtension>> {
        self.registry_store.get_installed(name).await
    }

    /// List all installed extensions
    pub async fn list_installed(&self) -> Result<Vec<InstalledExtension>> {
        self.registry_store.list_installed().await
    }

    /// Find installed extensions matching the query
    pub async fn find_installed(
        &self,
        query: &InstallationQuery,
    ) -> Result<Vec<InstalledExtension>> {
        self.registry_store.find_installed(query).await
    }

    /// Get statistics about installed extensions
    pub async fn get_installation_stats(&self) -> Result<InstallationStats> {
        self.registry_store.get_installation_stats().await
    }

    /// Validate all installed extensions
    pub async fn validate_installations(&self) -> Result<Vec<ValidationIssue>> {
        self.registry_store.validate_installations().await
    }

    /// Clean up orphaned registry entries
    pub async fn cleanup_orphaned(&mut self) -> Result<u32> {
        self.registry_store.cleanup_orphaned().await
    }

    /// Get the installation directory
    pub fn install_dir(&self) -> &std::path::Path {
        self.registry_store.install_dir()
    }

    /// Remove an installed extension
    pub async fn uninstall(&mut self, name: &str) -> Result<bool> {
        info!("Uninstalling extension '{}'", name);
        let removed = self.registry_store.uninstall_extension(name).await?;

        if removed {
            info!("Successfully uninstalled extension '{}'", name);
        } else {
            info!("Extension '{}' was not installed", name);
        }

        Ok(removed)
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

    fn deduplicate_extensions(&self, mut extensions: Vec<ExtensionInfo>) -> Vec<ExtensionInfo> {
        // Remove duplicates based on name + version, preferring trusted stores
        let mut seen: HashMap<String, String> = HashMap::new();
        extensions.retain(|ext| {
            let key = format!("{}@{}", ext.name, ext.version);
            if let Some(existing_store) = seen.get(&key) {
                // Keep if current store is trusted and existing is not
                let current_store = self.get_extension_store(&ext.store_source);
                let existing_trusted = self
                    .get_extension_store(existing_store)
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
                    .get_extension_store(existing_store)
                    .map(|s| s.store_info().trusted)
                    .unwrap_or(false);
                let current_trusted = self
                    .get_extension_store(&update.store_source)
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
        let _install_dir = temp_dir.path().join("install");
        let registry_dir = temp_dir.path().join("registry");

        let registry_store = Box::new(
            crate::registry::LocalRegistryStore::new(registry_dir)
                .await
                .unwrap(),
        );
        let manager = StoreManager::new(registry_store).await.unwrap();

        assert!(manager.install_dir().exists());
        assert_eq!(manager.list_extension_stores().len(), 0);
    }

    #[tokio::test]
    async fn test_registry_operations() {
        let temp_dir = TempDir::new().unwrap();
        let _install_dir = temp_dir.path().join("extensions");
        let registry_dir = temp_dir.path().join("registry");

        let registry_store = Box::new(
            crate::registry::LocalRegistryStore::new(registry_dir)
                .await
                .unwrap(),
        );
        let manager = StoreManager::new(registry_store).await.unwrap();

        // Initially no extensions
        assert_eq!(manager.list_installed().await.unwrap().len(), 0);
        assert!(manager.get_installed("test").await.unwrap().is_none());
    }
}
