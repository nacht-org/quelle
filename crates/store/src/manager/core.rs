use std::collections::{HashMap, HashSet};

use std::sync::Arc;

use futures::future::join_all;
use semver::Version;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use super::store_manifest::ExtensionVersion;
use crate::error::{Result, StoreError};
use crate::models::{
    ExtensionInfo, ExtensionListing, InstallOptions, InstalledExtension, SearchQuery, SearchSortBy,
    StoreConfig, UpdateInfo, UpdateOptions,
};
use crate::registry::{manifest::ExtensionManifest, RegistryStore};
use crate::stores::{config::RegistryStoreConfig, ReadableStore};

/// Wrapper combining a Store with its registry configuration
pub struct ManagedStore {
    store: Box<dyn ReadableStore>,
    config: RegistryStoreConfig,
}

impl ManagedStore {
    fn new(store: Box<dyn ReadableStore>, config: RegistryStoreConfig) -> Self {
        Self { store, config }
    }

    pub fn store(&self) -> &dyn ReadableStore {
        self.store.as_ref()
    }

    pub fn config(&self) -> &RegistryStoreConfig {
        &self.config
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
    pub async fn add_extension_store<S: ReadableStore + 'static>(
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
        store: Box<dyn ReadableStore>,
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

    /// Clear all extension stores
    pub async fn clear_extension_stores(&mut self) -> Result<()> {
        self.extension_stores.clear();
        Ok(())
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
        let failed_stores = Vec::new();

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
    pub async fn search_all_stores(&self, query: &SearchQuery) -> Result<Vec<ExtensionListing>> {
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

        Ok(self.deduplicate_extensions_and_sort(all_results, &query.sort_by))
    }

    /// Search for novels using installed extensions only
    pub async fn search_novels_with_installed_extensions(
        &self,
        query: &SearchQuery,
    ) -> Result<Vec<quelle_engine::bindings::quelle::extension::novel::BasicNovel>> {
        use quelle_engine::bindings::quelle::extension::novel::{
            AppliedFilter, ComplexSearchQuery, FilterValue, SimpleSearchQuery,
        };
        use quelle_engine::{http::ReqwestExecutor, ExtensionEngine};
        use std::collections::HashMap;
        use std::sync::Arc;

        let installed_extensions = self.list_installed().await?;

        if installed_extensions.is_empty() {
            return Ok(Vec::new());
        }

        let mut search_futures = Vec::new();

        for installed_ext in installed_extensions {
            // Get WASM bytes for this extension
            let wasm_bytes = match self
                .registry_store
                .get_extension_wasm_bytes(&installed_ext.id)
                .await
            {
                Ok(bytes) => bytes,
                Err(e) => {
                    warn!(
                        "Failed to get WASM bytes for extension '{}': {}",
                        installed_ext.name, e
                    );
                    continue;
                }
            };

            let query_clone = query.clone();
            let ext_name = installed_ext.name.clone();

            let future = async move {
                // Create HTTP executor
                let executor = Arc::new(ReqwestExecutor::new());

                // Create engine and runner for this extension
                let engine = match ExtensionEngine::new(executor) {
                    Ok(engine) => engine,
                    Err(e) => {
                        warn!(
                            "Failed to create engine for extension '{}': {}",
                            ext_name, e
                        );
                        return Vec::new();
                    }
                };

                let runner = match engine.new_runner_from_bytes(&wasm_bytes).await {
                    Ok(runner) => runner,
                    Err(e) => {
                        warn!(
                            "Failed to create runner for extension '{}': {}",
                            ext_name, e
                        );
                        return Vec::new();
                    }
                };

                // Determine if we need complex or simple search
                let has_complex_filters = query_clone.author.is_some()
                    || !query_clone.tags.is_empty()
                    || !query_clone.categories.is_empty();

                let search_result = if has_complex_filters {
                    // Build complex search query
                    let mut filters = Vec::new();

                    if let Some(ref author) = query_clone.author {
                        filters.push(AppliedFilter {
                            filter_id: "author".to_string(),
                            value: FilterValue::Text(author.clone()),
                        });
                    }

                    if !query_clone.tags.is_empty() {
                        filters.push(AppliedFilter {
                            filter_id: "tags".to_string(),
                            value: FilterValue::MultiSelect(query_clone.tags.clone()),
                        });
                    }

                    if !query_clone.categories.is_empty() {
                        filters.push(AppliedFilter {
                            filter_id: "categories".to_string(),
                            value: FilterValue::MultiSelect(query_clone.categories.clone()),
                        });
                    }

                    if let Some(ref text) = query_clone.text {
                        filters.push(AppliedFilter {
                            filter_id: "query".to_string(),
                            value: FilterValue::Text(text.clone()),
                        });
                    }

                    let complex_query = ComplexSearchQuery {
                        filters,
                        page: Some(1),
                        limit: query_clone.limit.map(|l| l as u32),
                        sort_by: None,
                        sort_order: None,
                    };

                    runner.complex_search(&complex_query).await
                } else {
                    // Use simple search for basic queries
                    let simple_query = SimpleSearchQuery {
                        query: query_clone.text.unwrap_or_default(),
                        page: Some(1),
                        limit: query_clone.limit.map(|l| l as u32),
                    };

                    runner.simple_search(&simple_query).await
                };

                // Process search result
                match search_result {
                    Ok((_, Ok(search_result))) => {
                        debug!(
                            "Extension '{}' returned {} results",
                            ext_name,
                            search_result.novels.len()
                        );
                        search_result.novels
                    }
                    Ok((_, Err(e))) => {
                        warn!("Search failed for extension '{}': {:?}", ext_name, e);
                        Vec::new()
                    }
                    Err(e) => {
                        warn!(
                            "Engine error during search for extension '{}': {}",
                            ext_name, e
                        );
                        Vec::new()
                    }
                }
            };

            search_futures.push(future);
        }

        let results = join_all(search_futures).await;
        let mut all_novels = Vec::new();

        for mut novels in results {
            all_novels.append(&mut novels);
        }

        // Deduplicate novels by URL
        let mut seen_urls = HashMap::new();
        let mut deduplicated = Vec::new();

        for novel in all_novels {
            if !seen_urls.contains_key(&novel.url) {
                seen_urls.insert(novel.url.clone(), true);
                deduplicated.push(novel);
            }
        }

        // Sort results by title for consistency
        deduplicated.sort_by(|a, b| a.title.cmp(&b.title));

        // Apply limit and offset
        let start_index = query.offset.unwrap_or(0);
        let end_index = if let Some(limit) = query.limit {
            std::cmp::min(start_index + limit, deduplicated.len())
        } else {
            deduplicated.len()
        };

        if start_index >= deduplicated.len() {
            Ok(Vec::new())
        } else {
            Ok(deduplicated[start_index..end_index].to_vec())
        }
    }

    /// List all extensions from all stores
    pub async fn list_all_extensions(&self) -> Result<Vec<ExtensionListing>> {
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
                        Ok::<Vec<ExtensionListing>, crate::error::StoreError>(extensions)
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

        Ok(all_extensions)
    }

    /// Get extension information from the best available store
    pub async fn get_extension_info(&self, id: &str) -> Result<Vec<ExtensionInfo>> {
        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            match managed_store.store.get_extension_info(id).await {
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

        Err(StoreError::ExtensionNotFound(id.to_string()))
    }

    /// Get extension manifest from the best available store
    pub async fn get_extension_manifest(
        &self,
        id: &str,
        version: Option<&Version>,
    ) -> Result<ExtensionManifest> {
        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            match managed_store
                .store
                .get_extension_manifest(id, version)
                .await
            {
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

        Err(StoreError::ExtensionNotFound(id.to_string()))
    }

    /// Find an extension that can handle the given URL
    /// Returns (id, store_name) if found
    pub async fn find_extension_for_url(&self, url: &str) -> Result<Option<(String, String)>> {
        // Check each store for URL matching using the store's implementation
        for managed_store in &self.extension_stores {
            if !managed_store.config.enabled {
                continue;
            }

            if let Ok(matching_extensions) = managed_store.store.find_extensions_for_url(url).await
            {
                if !matching_extensions.is_empty() {
                    // Return the first match with highest priority - now returns (id, name)
                    let (id, _name) = &matching_extensions[0];
                    return Ok(Some((id.clone(), managed_store.config.store_name.clone())));
                }
            }
        }

        Ok(None)
    }

    // Installation Operations

    /// Install an extension from the best available store
    pub async fn install(
        &mut self,
        id: &str,
        version: Option<&Version>,
        options: Option<InstallOptions>,
    ) -> Result<InstalledExtension> {
        let options = options.unwrap_or_default();

        // Check if already installed and handle accordingly
        if let Some(installed) = self.registry_store.get_installed(id).await? {
            if let Some(requested_version) = version {
                if &installed.version == requested_version && !options.force_reinstall {
                    info!("Extension {}@{} already installed", id, requested_version);
                    return Ok(installed);
                }
            } else if !options.force_reinstall {
                info!(
                    "Extension {} already installed with version {}",
                    id, installed.version
                );
                return Ok(installed);
            }
        }

        // Acquire download semaphore
        let _permit = self.download_semaphore.acquire().await.unwrap();

        info!("Installing extension: {}", id);
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

            match managed_store.store.get_extension_package(id, version).await {
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
                                id, installed.version, store_name
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

        Err(last_error.unwrap_or_else(|| StoreError::ExtensionNotFound(id.to_string())))
    }

    /// Install multiple extensions in parallel
    pub async fn batch_install(
        &mut self,
        requests: &[(String, Option<Version>)],
        options: Option<InstallOptions>,
    ) -> Result<Vec<Result<InstalledExtension>>> {
        let options = options.unwrap_or_default();
        let mut results = Vec::new();

        info!(
            "Starting batch installation of {} extensions",
            requests.len()
        );

        // Process installations sequentially to avoid conflicts with mutable state
        for (id, version) in requests {
            info!("Installing extension: {} (version: {:?})", id, version);

            let install_result = self
                .install(id, version.as_ref(), Some(options.clone()))
                .await;

            match &install_result {
                Ok(installed) => {
                    info!(
                        "Successfully installed: {}@{} from {}",
                        installed.name, installed.version, installed.source_store
                    );
                }
                Err(e) => {
                    error!("Failed to install {}: {}", id, e);
                }
            }

            results.push(install_result);
        }

        let successful = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.len() - successful;

        info!(
            "Batch installation completed: {} successful, {} failed",
            successful, failed
        );

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
                    store.check_extension_updates(&installed_slice),
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
        id: &str,
        options: Option<UpdateOptions>,
    ) -> Result<InstalledExtension> {
        let options = options.unwrap_or_default();

        let installed = self
            .get_installed(id)
            .await?
            .ok_or_else(|| StoreError::ExtensionNotFound(id.to_string()))?;

        // Find the store that originally provided this extension
        // Find the source store to check for updates
        let source_store = self.get_extension_store(&installed.source_store);

        let managed_store = match source_store {
            Some(store) => store,
            None => {
                warn!(
                    "Source store '{}' not found for extension '{}'",
                    installed.source_store, id
                );
                // Try to update from any available store
                return self
                    .install(id, None, Some(InstallOptions::default()))
                    .await;
            }
        };

        info!(
            "Updating extension '{}' from store '{}'",
            id, managed_store.config.store_name
        );

        // For now, we'll get the latest version and reinstall
        let latest_version = match managed_store.store.get_extension_latest_version(id).await? {
            Some(version) => version,
            None => return Err(StoreError::ExtensionNotFound(id.to_string())),
        };

        let install_options = InstallOptions {
            auto_update: options.update_dependencies,
            force_reinstall: options.force_update,
            skip_verification: false,
        };

        match managed_store
            .store
            .get_extension_package(id, Some(&latest_version))
            .await
        {
            Ok(package) => {
                match self
                    .registry_store
                    .install_extension(package, &install_options)
                    .await
                {
                    Ok(updated) => {
                        info!("Successfully updated {} to version {}", id, updated.version);
                        self.registry_store
                            .update_installation(updated.clone())
                            .await?;
                        Ok(updated)
                    }
                    Err(e) => {
                        error!("Failed to update extension '{}': {}", id, e);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                error!("Failed to get package for '{}': {}", id, e);
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
                .update(update_info.extension_id(), options.clone())
                .await;

            update_results.push(result);
        }

        Ok(update_results)
    }

    // Registry Management (delegated to registry store)

    /// Get information about an installed extension
    pub async fn get_installed(&self, id: &str) -> Result<Option<InstalledExtension>> {
        self.registry_store.get_installed(id).await
    }

    /// List all installed extensions
    pub async fn list_installed(&self) -> Result<Vec<InstalledExtension>> {
        self.registry_store.list_installed().await
    }

    /// Remove an installed extension
    pub async fn uninstall(&mut self, id: &str) -> Result<bool> {
        info!("Uninstalling extension '{}'", id);
        let removed = self.registry_store.uninstall_extension(id).await?;

        if removed {
            info!("Successfully uninstalled extension '{}'", id);
        } else {
            warn!("Extension '{}' was not installed", id);
        }

        Ok(removed)
    }

    // Private helper methods

    fn deduplicate_extensions(
        &self,
        mut extensions: Vec<ExtensionVersion>,
    ) -> Vec<ExtensionVersion> {
        // Remove duplicates based on id + version
        let mut seen: HashSet<String> = HashSet::new();
        extensions.retain(|ext| {
            let key = format!("{}@{}", ext.id, ext.version);
            if seen.contains(&key) {
                false
            } else {
                seen.insert(key);
                true
            }
        });

        extensions
    }

    fn deduplicate_extension_listings(
        &self,
        mut extensions: Vec<ExtensionListing>,
    ) -> Vec<ExtensionListing> {
        // Remove duplicates based on id + version
        let mut seen: HashSet<String> = HashSet::new();
        extensions.retain(|ext| {
            let key = format!("{}@{}", ext.id, ext.version);
            if seen.contains(&key) {
                false
            } else {
                seen.insert(key);
                true
            }
        });

        extensions
    }

    fn deduplicate_extensions_and_sort(
        &self,
        extensions: Vec<ExtensionListing>,
        sort_by: &SearchSortBy,
    ) -> Vec<ExtensionListing> {
        let mut deduplicated = self.deduplicate_extension_listings(extensions);

        match sort_by {
            SearchSortBy::Name => deduplicated.sort_by(|a, b| a.name.cmp(&b.name)),
            SearchSortBy::Version => deduplicated.sort_by(|a, b| b.version.cmp(&a.version)),
            SearchSortBy::LastUpdated => {
                deduplicated.sort_by(|a, b| match (a.last_updated, b.last_updated) {
                    (Some(a_time), Some(b_time)) => b_time.cmp(&a_time),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                })
            }
            SearchSortBy::Author => deduplicated.sort_by(|a, b| a.author.cmp(&b.author)),
            SearchSortBy::Size => {
                // ExtensionListing doesn't have size field, sort by name instead
                deduplicated.sort_by(|a, b| a.name.cmp(&b.name))
            }
            SearchSortBy::DownloadCount => {
                // ExtensionListing doesn't have download_count field, sort by name instead
                deduplicated.sort_by(|a, b| a.name.cmp(&b.name))
            }
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
            if let Some(existing_store) = seen.get(update.extension_id()) {
                // Check if existing update is from a trusted store
                let existing_trusted = self
                    .get_extension_store(existing_store)
                    .map(|ms| ms.config.trusted)
                    .unwrap_or(false);
                let current_trusted = self
                    .get_extension_store(update.store_source())
                    .map(|ms| ms.config.trusted)
                    .unwrap_or(false);

                if current_trusted && !existing_trusted {
                    seen.insert(
                        update.extension_id().to_string(),
                        update.store_source().to_string(),
                    );
                    true
                } else {
                    false
                }
            } else {
                seen.insert(
                    update.extension_id().to_string(),
                    update.store_source().to_string(),
                );
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

    #[tokio::test]
    async fn test_batch_install_empty_list() {
        let temp_dir = TempDir::new().unwrap();
        let registry_dir = temp_dir.path().join("registry");

        let registry_store = Box::new(
            crate::registry::LocalRegistryStore::new(registry_dir)
                .await
                .unwrap(),
        );
        let mut manager = StoreManager::new(registry_store).await.unwrap();

        // Test batch install with empty list
        let requests = vec![];
        let results = manager.batch_install(&requests, None).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_batch_install_with_nonexistent_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let registry_dir = temp_dir.path().join("registry");

        let registry_store = Box::new(
            crate::registry::LocalRegistryStore::new(registry_dir)
                .await
                .unwrap(),
        );
        let mut manager = StoreManager::new(registry_store).await.unwrap();

        // Test batch install with non-existent extensions
        let requests = vec![
            ("nonexistent1".to_string(), None),
            ("nonexistent2".to_string(), Some(Version::new(1, 0, 0))),
        ];

        let results = manager.batch_install(&requests, None).await.unwrap();
        assert_eq!(results.len(), 2);

        // Both should fail since extensions don't exist
        assert!(results[0].is_err());
        assert!(results[1].is_err());
    }

    #[tokio::test]
    async fn test_search_novels_with_no_installed_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let registry_dir = temp_dir.path().join("registry");

        let registry_store = Box::new(
            crate::registry::LocalRegistryStore::new(registry_dir)
                .await
                .unwrap(),
        );
        let manager = StoreManager::new(registry_store).await.unwrap();

        // Test search with no installed extensions
        let query = SearchQuery::new().with_text("test query".to_string());
        let results = manager
            .search_novels_with_installed_extensions(&query)
            .await
            .unwrap();

        // Should return empty results when no extensions are installed
        assert_eq!(results.len(), 0);
    }
}
