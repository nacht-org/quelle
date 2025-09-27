use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures::future::join_all;
use semver::Version;

use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use crate::error::{Result, StoreError};
use crate::local::LocalStore;
use crate::manifest::ExtensionManifest;
use crate::models::{
    ExtensionInfo, InstallOptions, InstalledExtension, SearchQuery, SearchSortBy, StoreConfig,
    UpdateInfo, UpdateOptions,
};
use crate::publish::{
    PublishOptions, PublishPermissions, PublishRequirements, PublishResult, PublishStats,
    PublishableStore, UnpublishOptions, UnpublishResult,
};
use crate::registry::{
    InstallationQuery, InstallationStats, RegistryHealth, RegistryStore, ValidationIssue,
};
use crate::registry_config::RegistryStoreConfig;
use crate::store::Store;

/// Wrapper combining a Store with its registry configuration
pub struct ManagedStore {
    store: Box<dyn Store>,
    config: RegistryStoreConfig,
}

impl ManagedStore {
    fn new(store: Box<dyn Store>, config: RegistryStoreConfig) -> Self {
        Self { store, config }
    }

    pub fn store(&self) -> &dyn Store {
        self.store.as_ref()
    }

    pub fn config(&self) -> &RegistryStoreConfig {
        &self.config
    }

    /// Try to get publish requirements if this store supports publishing
    pub fn get_publish_requirements(&self) -> Option<PublishRequirements> {
        // Try to downcast to LocalStore (currently the only PublishableStore implementation)
        let store_any = self.store.as_ref() as &dyn std::any::Any;
        if let Some(local_store) = store_any.downcast_ref::<LocalStore>() {
            Some(local_store.publish_requirements())
        } else {
            None
        }
    }

    /// Try to check publish permissions if this store supports publishing
    pub async fn can_publish(&self, extension_name: &str) -> Option<Result<PublishPermissions>> {
        let store_any = self.store.as_ref() as &dyn std::any::Any;
        if let Some(local_store) = store_any.downcast_ref::<LocalStore>() {
            Some(local_store.can_publish(extension_name).await)
        } else {
            None
        }
    }

    /// Try to get publish stats if this store supports publishing
    pub async fn get_publish_stats(&self) -> Option<Result<PublishStats>> {
        let store_any = self.store.as_ref() as &dyn std::any::Any;
        if let Some(local_store) = store_any.downcast_ref::<LocalStore>() {
            Some(local_store.get_publish_stats().await)
        } else {
            None
        }
    }

    /// Check if this store supports publishing operations
    pub fn supports_publishing(&self) -> bool {
        let store_any = self.store.as_ref() as &dyn std::any::Any;
        store_any.downcast_ref::<LocalStore>().is_some()
    }
}

/// Central manager for handling multiple stores and local installations
pub struct StoreManager {
    /// Extension sources (read-only stores for discovering extensions)
    extension_stores: Vec<ManagedStore>,
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
    /// Add an extension store to the manager with registry configuration
    pub async fn add_extension_store<S: Store + 'static>(
        &mut self,
        store: S,
        registry_config: RegistryStoreConfig,
    ) -> Result<()> {
        let manifest = store.get_store_manifest().await?;
        info!("Adding extension store: {}", manifest.name);

        let managed_store = ManagedStore::new(Box::new(store), registry_config);
        self.extension_stores.push(managed_store);
        self.sort_stores_by_priority();
        Ok(())
    }

    /// Add a boxed extension store to the manager with registry configuration
    pub async fn add_boxed_extension_store(
        &mut self,
        store: Box<dyn Store>,
        registry_config: RegistryStoreConfig,
    ) -> Result<()> {
        let manifest = store.get_store_manifest().await?;
        info!("Adding extension store: {}", manifest.name);

        let managed_store = ManagedStore::new(store, registry_config);
        self.extension_stores.push(managed_store);
        self.sort_stores_by_priority();
        Ok(())
    }

    /// Remove an extension store by name
    pub fn remove_extension_store(&mut self, name: &str) -> bool {
        let initial_len = self.extension_stores.len();
        self.extension_stores
            .retain(|managed_store| managed_store.config.store_name != name);

        initial_len != self.extension_stores.len()
    }

    /// Get information about all registered extension stores
    /// Get list of store names
    pub fn list_extension_stores(&self) -> &[ManagedStore] {
        &self.extension_stores
    }

    /// Get a specific store's configuration by name
    pub fn get_extension_store_config(&self, name: &str) -> Option<&RegistryStoreConfig> {
        self.get_extension_store(name)
            .map(|managed_store| &managed_store.config)
    }

    /// Get the registry store
    pub fn registry_store(&self) -> &dyn RegistryStore {
        self.registry_store.as_ref()
    }

    /// Get a specific extension store by name
    pub fn get_extension_store(&self, name: &str) -> Option<&ManagedStore> {
        self.extension_stores
            .iter()
            .find(|managed_store| managed_store.config.store_name == name)
    }

    /// Sort stores by priority (lower number = higher priority)
    /// Sort stores by priority (higher priority first)
    fn sort_stores_by_priority(&mut self) {
        self.extension_stores.sort_by(|a, b| {
            b.config
                .priority
                .cmp(&a.config.priority)
                .then_with(|| a.config.store_name.cmp(&b.config.store_name))
        });
    }

    /// Refresh all stores (health check and cache refresh)
    pub async fn refresh_stores(&mut self) -> Result<Vec<String>> {
        let mut failed_stores = Vec::new();

        info!(
            "Refreshing {} extension stores",
            self.extension_stores.len()
        );
        let mut _failed_stores = Vec::new();

        for managed_store in &self.extension_stores {
            let store_name = &managed_store.config.store_name;
            debug!("Checking health of store: {}", store_name);

            match tokio::time::timeout(self.config.timeout, managed_store.store.health_check())
                .await
            {
                Ok(Ok(health)) => {
                    if !health.healthy {
                        warn!("Store '{}' is unhealthy: {:?}", store_name, health.error);
                        _failed_stores.push(store_name.clone());
                    } else {
                        debug!("Store '{}' is healthy", store_name);
                    }
                }
                Ok(Err(e)) => {
                    warn!("Health check failed for store '{}': {}", store_name, e);
                    _failed_stores.push(store_name.clone());
                }
                Err(_) => {
                    warn!("Health check timed out for store '{}'", store_name);
                    _failed_stores.push(store_name.clone());
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

        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            let store_name = managed_store.config.store_name.clone();
            let store = &managed_store.store;
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
        let mut futures = Vec::new();

        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            let store_name = managed_store.config.store_name.clone();
            let store = &managed_store.store;
            let future = async move {
                match tokio::time::timeout(self.config.timeout, store.list_extensions()).await {
                    Ok(Ok(extensions)) => {
                        debug!("Store '{}' has {} extensions", store_name, extensions.len());
                        Ok::<Vec<ExtensionInfo>, crate::error::StoreError>(extensions)
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "Failed to list extensions from store '{}': {}",
                            store_name, e
                        );
                        Ok(vec![])
                    }
                    Err(_) => {
                        warn!("Listing extensions timed out for store '{}'", store_name);
                        Ok(vec![])
                    }
                }
            };
            futures.push(future);
        }

        let results = join_all(futures).await;
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
        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            match managed_store.store.get_extension_info(name).await {
                Ok(info) if !info.is_empty() => return Ok(info),
                Ok(_) => continue, // Empty result, try next store
                Err(StoreError::ExtensionNotFound(_)) => continue,
                Err(e) if e.is_recoverable() => {
                    warn!(
                        "Recoverable error from store '{}': {}",
                        managed_store.config.store_name, e
                    );
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(StoreError::ExtensionNotFound(name.to_string()))
    }

    /// Get extension manifest from the best available store
    pub async fn get_extension_manifest(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest> {
        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            match managed_store.store.get_manifest(name, version).await {
                Ok(manifest) => return Ok(manifest),
                Err(StoreError::ExtensionNotFound(_)) => continue,
                Err(e) if e.is_recoverable() => {
                    warn!(
                        "Recoverable error from store '{}': {}",
                        managed_store.config.store_name, e
                    );
                    continue;
                }
                Err(e) => return Err(e),
            }
        }

        Err(StoreError::ExtensionNotFound(name.to_string()))
    }

    /// Find an extension that can handle the given URL
    pub async fn find_extension_for_url(&self, url: &str) -> Result<Option<(String, String)>> {
        // Check each store for URL matching using the store's implementation
        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            if let Ok(matching_extensions) = managed_store.store.find_extensions_for_url(url).await
            {
                if !matching_extensions.is_empty() {
                    // Return the first match with highest priority
                    return Ok(Some((
                        matching_extensions[0].clone(),
                        managed_store.config.store_name.clone(),
                    )));
                }
            }
        }

        // 2. Fallback: check individual extension manifests for base_urls
        let extensions = self.list_all_extensions().await?;

        for ext in extensions {
            // Try to get the manifest for this extension to check base_urls
            if let Ok(manifest) = self
                .get_extension_manifest(&ext.name, Some(&ext.version))
                .await
            {
                // Check if any of the extension's base URLs match the given URL
                for base_url in &manifest.base_urls {
                    if url.starts_with(base_url) {
                        return Ok(Some((ext.name.clone(), ext.store_source.clone())));
                    }
                }
            }
        }

        Ok(None)
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
        // Try installing from each store in priority order
        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            let store_name = managed_store.config.store_name.clone();
            debug!("Trying to install from store: {}", store_name);

            match managed_store
                .store
                .get_extension_package(name, version)
                .await
            {
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

        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            let store_name = managed_store.config.store_name.clone();
            let store = &managed_store.store;
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
        // Find the source store to check for updates
        let source_store = self.get_extension_store(&installed.source_store);

        let managed_store = match source_store {
            Some(store) => store,
            None => {
                warn!(
                    "Source store '{}' not found for extension '{}'",
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
            name, managed_store.config.store_name
        );

        // For now, we'll get the latest version and reinstall
        let latest_version = match managed_store.store.get_latest_version(name).await? {
            Some(version) => version,
            None => return Err(StoreError::ExtensionNotFound(name.to_string())),
        };

        let install_options = InstallOptions {
            auto_update: options.update_dependencies,
            force_reinstall: options.force_update,
            skip_verification: false,
        };

        match managed_store
            .store
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

    /// Get registry health information (generic across implementations)
    pub async fn get_registry_health(&self) -> Result<RegistryHealth> {
        self.registry_store.get_registry_health().await
    }

    /// Validate all installed extensions
    pub async fn validate_installations(&self) -> Result<Vec<ValidationIssue>> {
        self.registry_store.validate_installations().await
    }

    /// Clean up orphaned registry entries
    pub async fn cleanup_orphaned(&mut self) -> Result<u32> {
        self.registry_store.cleanup_orphaned().await
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
                // Check if existing extension is from a trusted store
                let existing_trusted = self
                    .get_extension_store(existing_store)
                    .map(|ms| ms.config.trusted)
                    .unwrap_or(false);
                let current_trusted = self
                    .get_extension_store(&ext.store_source)
                    .map(|ms| ms.config.trusted)
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
                // Check if existing update is from a trusted store
                let existing_trusted = self
                    .get_extension_store(existing_store)
                    .map(|ms| ms.config.trusted)
                    .unwrap_or(false);
                let current_trusted = self
                    .get_extension_store(&update.store_source)
                    .map(|ms| ms.config.trusted)
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

    /// Get publish requirements for a store if it supports publishing
    pub fn get_store_publish_requirements(&self, store_name: &str) -> Option<PublishRequirements> {
        self.get_extension_store(store_name)
            .and_then(|managed_store| managed_store.get_publish_requirements())
    }

    /// Check publish permissions for a store if it supports publishing
    pub async fn check_store_publish_permissions(
        &self,
        store_name: &str,
        extension_name: &str,
    ) -> Option<Result<PublishPermissions>> {
        if let Some(managed_store) = self.get_extension_store(store_name) {
            managed_store.can_publish(extension_name).await
        } else {
            None
        }
    }

    /// Get publish stats for a store if it supports publishing
    pub async fn get_store_publish_stats(&self, store_name: &str) -> Option<Result<PublishStats>> {
        if let Some(managed_store) = self.get_extension_store(store_name) {
            managed_store.get_publish_stats().await
        } else {
            None
        }
    }

    /// Check if a store supports publishing operations
    pub fn store_supports_publishing(&self, store_name: &str) -> bool {
        self.get_extension_store(store_name)
            .map(|managed_store| managed_store.supports_publishing())
            .unwrap_or(false)
    }

    /// Publish an extension to a store if it supports publishing
    pub async fn publish_extension_to_store(
        &mut self,
        store_name: &str,
        package: crate::models::ExtensionPackage,
        options: &PublishOptions,
    ) -> Option<Result<PublishResult>> {
        // Find the store in our collection
        for managed_store in &mut self.extension_stores {
            if managed_store.config.store_name == store_name {
                // Try to downcast to LocalStore for mutable operations
                let store_any = managed_store.store.as_mut() as &mut dyn std::any::Any;
                if let Some(local_store) = store_any.downcast_mut::<LocalStore>() {
                    return Some(local_store.publish_extension(package, options).await);
                }
            }
        }
        None
    }

    /// Unpublish an extension from a store if it supports publishing
    pub async fn unpublish_extension_from_store(
        &mut self,
        store_name: &str,
        name: &str,
        version: &str,
        options: &UnpublishOptions,
    ) -> Option<Result<UnpublishResult>> {
        for managed_store in &mut self.extension_stores {
            if managed_store.config.store_name == store_name {
                let store_any = managed_store.store.as_mut() as &mut dyn std::any::Any;
                if let Some(local_store) = store_any.downcast_mut::<LocalStore>() {
                    return Some(
                        local_store
                            .unpublish_extension(name, version, options)
                            .await,
                    );
                }
            }
        }
        None
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

        // Test that manager can be created successfully
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
