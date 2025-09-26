use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use chrono::DateTime;
use semver::Version;
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::error::{Result, StoreError};
use crate::manifest::ExtensionManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, InstallOptions, InstalledExtension,
    PackageLayout, SearchQuery, SearchSortBy, StoreHealth, StoreInfo, UpdateInfo, UpdateOptions,
};
use crate::store::{capabilities, Store};

/// Local file system based store implementation
pub struct LocalStore {
    root_path: PathBuf,
    layout: PackageLayout,
    info: StoreInfo,
    cache: std::sync::RwLock<HashMap<String, Vec<ExtensionInfo>>>,
    cache_timestamp: std::sync::RwLock<Option<SystemTime>>,
}

impl LocalStore {
    /// Create a new LocalStore instance
    pub fn new<P: AsRef<Path>>(root_path: P) -> Result<Self> {
        let root_path = root_path.as_ref().to_path_buf();

        // Ensure the root directory exists
        if !root_path.exists() {
            fs::create_dir_all(&root_path)?;
        }

        let info = StoreInfo::new("local".to_string(), "local".to_string())
            .with_url(format!("file://{}", root_path.display()))
            .trusted();

        Ok(Self {
            root_path,
            layout: PackageLayout::default(),
            info,
            cache: std::sync::RwLock::new(HashMap::new()),
            cache_timestamp: std::sync::RwLock::new(None),
        })
    }

    /// Create a LocalStore with a custom package layout
    pub fn with_layout<P: AsRef<Path>>(root_path: P, layout: PackageLayout) -> Result<Self> {
        let mut store = Self::new(root_path)?;
        store.layout = layout;
        Ok(store)
    }

    /// Get the path to an extension directory
    fn extension_path(&self, name: &str) -> PathBuf {
        self.root_path.join("extensions").join(name)
    }

    /// Get the path to a specific version of an extension
    fn extension_version_path(&self, name: &str, version: &str) -> PathBuf {
        self.extension_path(name).join(version)
    }

    /// Get the path to the extensions directory
    fn extensions_root(&self) -> PathBuf {
        self.root_path.join("extensions")
    }

    /// Scan and cache extension information
    async fn scan_extensions(&self) -> Result<HashMap<String, Vec<ExtensionInfo>>> {
        let extensions_root = self.extensions_root();
        if !extensions_root.exists() {
            fs::create_dir_all(&extensions_root)?;
            return Ok(HashMap::new());
        }

        let mut extensions = HashMap::new();

        for entry in WalkDir::new(&extensions_root)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
        {
            let entry = entry.map_err(|e| StoreError::IoError(e.into()))?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check if this looks like a version directory
            if let Some(version_name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(extension_name) = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                {
                    // Skip if parent is extensions root (this is an extension directory)
                    if path.parent().unwrap() == extensions_root {
                        continue;
                    }

                    match self
                        .load_extension_info(extension_name, Some(version_name))
                        .await
                    {
                        Ok(info) => {
                            extensions
                                .entry(extension_name.to_string())
                                .or_insert_with(Vec::new)
                                .push(info);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to load extension {}/{}: {}",
                                extension_name, version_name, e
                            );
                        }
                    }
                }
            }
        }

        // Sort versions for each extension
        for versions in extensions.values_mut() {
            versions.sort_by(|a, b| {
                Version::parse(&a.version)
                    .and_then(|v_a| Version::parse(&b.version).map(|v_b| v_b.cmp(&v_a)))
                    .unwrap_or_else(|_| b.version.cmp(&a.version))
            });
        }

        Ok(extensions)
    }

    /// Load extension info for a specific version
    async fn load_extension_info(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo> {
        let version = match version {
            Some(v) => v.to_string(),
            None => self
                .get_latest_version_internal(name)
                .await?
                .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?,
        };

        let version_path = self.extension_version_path(name, &version);
        let manifest_path = version_path.join(&self.layout.manifest_file);

        if !manifest_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "{}@{}",
                name, version
            )));
        }

        let manifest_content = fs::read_to_string(&manifest_path)?;
        let manifest: ExtensionManifest = serde_json::from_str(&manifest_content)
            .map_err(|e| StoreError::InvalidManifest(name.to_string(), e.to_string()))?;

        // Load metadata if available
        let metadata = if let Some(metadata_file) = &self.layout.metadata_file {
            let metadata_path = version_path.join(metadata_file);
            if metadata_path.exists() {
                let metadata_content = fs::read_to_string(&metadata_path).ok();
                metadata_content
                    .and_then(|content| serde_json::from_str::<ExtensionMetadata>(&content).ok())
            } else {
                None
            }
        } else {
            None
        };

        // Get file statistics
        let wasm_path = version_path.join(&self.layout.wasm_file);
        let size = wasm_path.metadata().ok().map(|m| m.len());
        let last_updated = manifest_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| {
                DateTime::from_timestamp(
                    t.duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            });

        Ok(ExtensionInfo {
            name: name.to_string(),
            version,
            description: metadata.as_ref().map(|m| m.description.clone()),
            author: manifest.author,
            tags: metadata
                .as_ref()
                .map(|m| m.keywords.clone())
                .unwrap_or_default(),
            last_updated,
            download_count: None,
            size,
            homepage: metadata.as_ref().and_then(|m| m.homepage.clone()),
            repository: metadata.as_ref().and_then(|m| m.repository.clone()),
            license: metadata.as_ref().and_then(|m| m.license.clone()),
            store_source: self.info.name.clone(),
        })
    }

    /// Get the latest version of an extension
    async fn get_latest_version_internal(&self, name: &str) -> Result<Option<String>> {
        let extension_dir = self.extension_path(name);
        if !extension_dir.exists() {
            return Ok(None);
        }

        let mut versions = Vec::new();
        for entry in fs::read_dir(&extension_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(version_name) = entry.file_name().to_str() {
                    versions.push(version_name.to_string());
                }
            }
        }

        if versions.is_empty() {
            return Ok(None);
        }

        // Sort versions semantically if possible, otherwise lexicographically
        versions.sort_by(|a, b| {
            match (Version::parse(a), Version::parse(b)) {
                (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // Descending order
                _ => b.cmp(a),                       // Lexicographic descending
            }
        });

        Ok(versions.into_iter().next())
    }

    /// Refresh the extension cache
    async fn refresh_cache(&self) -> Result<()> {
        debug!("Refreshing LocalStore cache");
        let extensions = self.scan_extensions().await?;

        {
            let mut cache = self.cache.write().unwrap();
            *cache = extensions;
        }

        {
            let mut timestamp = self.cache_timestamp.write().unwrap();
            *timestamp = Some(SystemTime::now());
        }

        Ok(())
    }

    /// Check if cache is valid (not older than 5 minutes)
    fn is_cache_valid(&self) -> bool {
        if let Some(timestamp) = *self.cache_timestamp.read().unwrap() {
            timestamp.elapsed().unwrap_or_default().as_secs() < 300
        } else {
            false
        }
    }

    /// Get cached extensions, refreshing if needed
    async fn get_cached_extensions(&self) -> Result<HashMap<String, Vec<ExtensionInfo>>> {
        if !self.is_cache_valid() {
            self.refresh_cache().await?;
        }

        Ok(self.cache.read().unwrap().clone())
    }

    /// Search extensions with the given query
    fn search_cached_extensions(
        &self,
        extensions: &HashMap<String, Vec<ExtensionInfo>>,
        query: &SearchQuery,
    ) -> Vec<ExtensionInfo> {
        let mut results = Vec::new();

        for versions in extensions.values() {
            for ext_info in versions {
                let mut matches = true;

                // Text search in name, description, author
                if let Some(text) = &query.text {
                    let text_lower = text.to_lowercase();
                    let matches_name = ext_info.name.to_lowercase().contains(&text_lower);
                    let matches_desc = ext_info
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&text_lower))
                        .unwrap_or(false);
                    let matches_author = ext_info.author.to_lowercase().contains(&text_lower);

                    if !matches_name && !matches_desc && !matches_author {
                        matches = false;
                    }
                }

                // Author filter
                if let Some(author) = &query.author {
                    if !ext_info.author.eq_ignore_ascii_case(author) {
                        matches = false;
                    }
                }

                // Tags filter
                if !query.tags.is_empty() {
                    let has_matching_tag = query.tags.iter().any(|tag| {
                        ext_info
                            .tags
                            .iter()
                            .any(|ext_tag| ext_tag.eq_ignore_ascii_case(tag))
                    });
                    if !has_matching_tag {
                        matches = false;
                    }
                }

                // Version filters
                if let Some(min_version) = &query.min_version {
                    if let (Ok(current), Ok(min)) = (
                        Version::parse(&ext_info.version),
                        Version::parse(min_version),
                    ) {
                        if current < min {
                            matches = false;
                        }
                    }
                }

                if let Some(max_version) = &query.max_version {
                    if let (Ok(current), Ok(max)) = (
                        Version::parse(&ext_info.version),
                        Version::parse(max_version),
                    ) {
                        if current > max {
                            matches = false;
                        }
                    }
                }

                if matches {
                    results.push(ext_info.clone());
                }
            }
        }

        // Sort results
        match query.sort_by {
            SearchSortBy::Name => results.sort_by(|a, b| a.name.cmp(&b.name)),
            SearchSortBy::Version => results.sort_by(|a, b| {
                match (Version::parse(&a.version), Version::parse(&b.version)) {
                    (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a),
                    _ => b.version.cmp(&a.version),
                }
            }),
            SearchSortBy::LastUpdated => {
                results.sort_by(|a, b| b.last_updated.cmp(&a.last_updated))
            }
            SearchSortBy::Author => results.sort_by(|a, b| a.author.cmp(&b.author)),
            SearchSortBy::Size => {
                results.sort_by(|a, b| b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)))
            }
            SearchSortBy::DownloadCount => {
                // LocalStore doesn't track download counts
                results.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SearchSortBy::Relevance => {
                // Simple relevance: exact name matches first
                if let Some(text) = &query.text {
                    results.sort_by(|a, b| {
                        let a_exact = a.name.eq_ignore_ascii_case(text);
                        let b_exact = b.name.eq_ignore_ascii_case(text);
                        match (a_exact, b_exact) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a.name.cmp(&b.name),
                        }
                    });
                }
            }
        }

        // Apply limit and offset
        if let Some(offset) = query.offset {
            results = results.into_iter().skip(offset).collect();
        }

        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        results
    }

    /// Verify the integrity of an extension package
    async fn verify_extension_integrity(&self, name: &str, version: Option<&str>) -> Result<bool> {
        let manifest = self.get_manifest(name, version).await?;
        let wasm_bytes = self.get_extension_wasm(name, version).await?;

        let calculated_hash = format!("{:x}", Sha256::digest(&wasm_bytes));
        Ok(manifest.checksum.value == calculated_hash)
    }
}

#[async_trait]
impl Store for LocalStore {
    fn store_info(&self) -> &StoreInfo {
        &self.info
    }

    fn package_layout(&self) -> &PackageLayout {
        &self.layout
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        let start_time = std::time::Instant::now();

        // Check if root directory is accessible
        match fs::metadata(&self.root_path) {
            Ok(_) => {
                let response_time = start_time.elapsed();
                let extensions = self.get_cached_extensions().await?;
                let extension_count = extensions.values().map(|v| v.len()).sum();

                Ok(StoreHealth::healthy()
                    .with_response_time(response_time)
                    .with_extension_count(extension_count))
            }
            Err(e) => Ok(StoreHealth::unhealthy(format!(
                "Cannot access store directory: {}",
                e
            ))),
        }
    }

    async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>> {
        let extensions = self.get_cached_extensions().await?;
        let mut all_extensions = Vec::new();

        for versions in extensions.values() {
            all_extensions.extend(versions.clone());
        }

        Ok(all_extensions)
    }

    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
        let extensions = self.get_cached_extensions().await?;
        Ok(self.search_cached_extensions(&extensions, query))
    }

    async fn get_extension_info(&self, name: &str) -> Result<Vec<ExtensionInfo>> {
        let extensions = self.get_cached_extensions().await?;
        Ok(extensions.get(name).cloned().unwrap_or_default())
    }

    async fn get_extension_version_info(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo> {
        self.load_extension_info(name, version).await
    }

    async fn get_manifest(&self, name: &str, version: Option<&str>) -> Result<ExtensionManifest> {
        let version = match version {
            Some(v) => v.to_string(),
            None => self
                .get_latest_version_internal(name)
                .await?
                .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?,
        };

        let manifest_path = self
            .extension_version_path(name, &version)
            .join(&self.layout.manifest_file);

        if !manifest_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "{}@{}",
                name, version
            )));
        }

        let content = fs::read_to_string(&manifest_path)?;
        serde_json::from_str(&content)
            .map_err(|e| StoreError::InvalidManifest(name.to_string(), e.to_string()))
    }

    async fn get_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        let version = match version {
            Some(v) => v.to_string(),
            None => match self.get_latest_version_internal(name).await? {
                Some(v) => v,
                None => return Ok(None),
            },
        };

        if let Some(metadata_file) = &self.layout.metadata_file {
            let metadata_path = self
                .extension_version_path(name, &version)
                .join(metadata_file);

            if metadata_path.exists() {
                let content = fs::read_to_string(&metadata_path)?;
                let metadata: ExtensionMetadata = serde_json::from_str(&content)
                    .map_err(|e| StoreError::InvalidManifest(name.to_string(), e.to_string()))?;
                Ok(Some(metadata))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    async fn get_extension_wasm(&self, name: &str, version: Option<&str>) -> Result<Vec<u8>> {
        let version = match version {
            Some(v) => v.to_string(),
            None => self
                .get_latest_version_internal(name)
                .await?
                .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?,
        };

        let wasm_path = self
            .extension_version_path(name, &version)
            .join(&self.layout.wasm_file);

        if !wasm_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "{}@{}",
                name, version
            )));
        }

        Ok(fs::read(&wasm_path)?)
    }

    async fn get_extension_package(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        let manifest = self.get_manifest(name, version).await?;
        let wasm_component = self.get_extension_wasm(name, version).await?;
        let metadata = self.get_metadata(name, version).await?;

        let actual_version = version.unwrap_or(&manifest.version);
        let version_path = self.extension_version_path(name, actual_version);

        let mut package = ExtensionPackage::new(manifest, wasm_component, self.info.name.clone())
            .with_layout(self.layout.clone());

        if let Some(metadata) = metadata {
            package = package.with_metadata(metadata);
        }

        // Load additional assets if assets directory exists
        if let Some(assets_dir) = &self.layout.assets_dir {
            let assets_path = version_path.join(assets_dir);
            if assets_path.exists() {
                for entry in WalkDir::new(&assets_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if entry.file_type().is_file() {
                        if let Ok(relative_path) = entry.path().strip_prefix(&assets_path) {
                            if let Some(path_str) = relative_path.to_str() {
                                if let Ok(content) = fs::read(entry.path()) {
                                    package.add_asset(path_str.to_string(), content);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(package)
    }

    async fn install_extension(
        &self,
        name: &str,
        version: Option<&str>,
        target_dir: &Path,
        options: &InstallOptions,
    ) -> Result<InstalledExtension> {
        // Get the package to install
        let package = self.get_extension_package(name, version).await?;
        let install_version = &package.manifest.version;

        // Create target directory structure
        let extension_install_path = if let Some(target) = &options.target_directory {
            target.join(name).join(install_version)
        } else {
            target_dir.join(name).join(install_version)
        };

        // Check if already installed and not forcing reinstall
        if extension_install_path.exists() && !options.force_reinstall {
            // Verify existing installation
            let existing_manifest_path =
                extension_install_path.join(&package.package_layout.manifest_file);
            if existing_manifest_path.exists() {
                if let Ok(existing_manifest_content) = fs::read_to_string(&existing_manifest_path) {
                    if let Ok(existing_manifest) =
                        serde_json::from_str::<ExtensionManifest>(&existing_manifest_content)
                    {
                        if existing_manifest.version == *install_version {
                            info!("Extension {}@{} already installed", name, install_version);
                            return Ok(InstalledExtension::new(
                                name.to_string(),
                                install_version.clone(),
                                extension_install_path,
                                existing_manifest,
                                package.package_layout,
                                self.info.name.clone(),
                            ));
                        }
                    }
                }
            }
        }

        // Create installation directory
        fs::create_dir_all(&extension_install_path)?;

        // Write WASM component
        let wasm_install_path = extension_install_path.join(&package.package_layout.wasm_file);
        fs::write(&wasm_install_path, &package.wasm_component)?;

        // Write manifest
        let manifest_install_path =
            extension_install_path.join(&package.package_layout.manifest_file);
        let manifest_content = serde_json::to_string_pretty(&package.manifest)?;
        fs::write(&manifest_install_path, manifest_content)?;

        // Write metadata if available
        if let Some(metadata) = &package.metadata {
            if let Some(metadata_file) = &package.package_layout.metadata_file {
                let metadata_install_path = extension_install_path.join(metadata_file);
                let metadata_content = serde_json::to_string_pretty(metadata)?;
                fs::write(&metadata_install_path, metadata_content)?;
            }
        }

        // Write additional assets
        if !package.assets.is_empty() {
            if let Some(assets_dir) = &package.package_layout.assets_dir {
                let assets_install_path = extension_install_path.join(assets_dir);
                fs::create_dir_all(&assets_install_path)?;

                for (asset_name, asset_content) in &package.assets {
                    let asset_path = assets_install_path.join(asset_name);
                    if let Some(parent) = asset_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&asset_path, asset_content)?;
                }
            }
        }

        // Verify installation if not skipping verification
        if !options.skip_verification {
            let verification_result = self
                .verify_extension_integrity(name, Some(install_version))
                .await;
            if let Ok(false) = verification_result {
                return Err(StoreError::ChecksumMismatch(format!(
                    "{}@{}",
                    name, install_version
                )));
            }
        }

        let mut installed = InstalledExtension::new(
            name.to_string(),
            install_version.clone(),
            extension_install_path,
            package.manifest.clone(),
            package.package_layout.clone(),
            self.info.name.clone(),
        );

        // Calculate install size
        installed.install_size = package.calculate_total_size();

        info!(
            "Successfully installed extension {}@{}",
            name, install_version
        );
        Ok(installed)
    }

    async fn check_updates(&self, installed: &[InstalledExtension]) -> Result<Vec<UpdateInfo>> {
        let mut updates = Vec::new();

        for installed_ext in installed {
            // Skip if not from this store
            if installed_ext.installed_from != self.info.name {
                continue;
            }

            if let Ok(Some(latest_version)) = self.get_latest_version(&installed_ext.name).await {
                if latest_version != installed_ext.version {
                    let update_info = UpdateInfo::new(
                        installed_ext.name.clone(),
                        installed_ext.version.clone(),
                        latest_version,
                        self.info.name.clone(),
                    );
                    updates.push(update_info);
                }
            }
        }

        Ok(updates)
    }

    async fn get_latest_version(&self, name: &str) -> Result<Option<String>> {
        self.get_latest_version_internal(name).await
    }

    async fn update_extension(
        &self,
        name: &str,
        target_dir: &Path,
        options: &UpdateOptions,
    ) -> Result<InstalledExtension> {
        let install_options = InstallOptions {
            install_dependencies: options.update_dependencies,
            allow_downgrades: false,
            force_reinstall: options.force_update,
            skip_verification: false,
            target_directory: None,
        };

        self.install_extension(name, None, target_dir, &install_options)
            .await
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>> {
        let extension_dir = self.extension_path(name);
        if !extension_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        for entry in fs::read_dir(&extension_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(version_name) = entry.file_name().to_str() {
                    // Verify this version has required files
                    let version_path = entry.path();
                    let manifest_path = version_path.join(&self.layout.manifest_file);
                    let wasm_path = version_path.join(&self.layout.wasm_file);

                    if manifest_path.exists() && wasm_path.exists() {
                        versions.push(version_name.to_string());
                    }
                }
            }
        }

        // Sort versions semantically
        versions.sort_by(|a, b| {
            match (Version::parse(a), Version::parse(b)) {
                (Ok(v_a), Ok(v_b)) => v_b.cmp(&v_a), // Descending order (latest first)
                _ => b.cmp(a),                       // Lexicographic descending
            }
        });

        Ok(versions)
    }

    async fn version_exists(&self, name: &str, version: &str) -> Result<bool> {
        let version_path = self.extension_version_path(name, version);
        let manifest_path = version_path.join(&self.layout.manifest_file);
        let wasm_path = version_path.join(&self.layout.wasm_file);

        Ok(manifest_path.exists() && wasm_path.exists())
    }

    fn supports_capability(&self, capability: &str) -> bool {
        match capability {
            capabilities::SEARCH => true,
            capabilities::VERSIONING => true,
            capabilities::METADATA => true,
            capabilities::CACHING => true,
            capabilities::UPDATE_CHECK => true,
            capabilities::BATCH_OPERATIONS => false,
            capabilities::STREAMING => false,
            capabilities::AUTHENTICATION => false,
            capabilities::PRIVATE_EXTENSIONS => false,
            capabilities::SIGNATURES => false,
            capabilities::DEPENDENCIES => false,
            capabilities::ROLLBACK => true,
            _ => false,
        }
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            capabilities::SEARCH.to_string(),
            capabilities::VERSIONING.to_string(),
            capabilities::METADATA.to_string(),
            capabilities::CACHING.to_string(),
            capabilities::UPDATE_CHECK.to_string(),
            capabilities::ROLLBACK.to_string(),
        ]
    }

    async fn validate_extension(&self, name: &str, version: Option<&str>) -> Result<bool> {
        self.verify_extension_integrity(name, version).await
    }

    async fn clear_cache(&self, name: Option<&str>) -> Result<()> {
        if let Some(extension_name) = name {
            let mut cache = self.cache.write().unwrap();
            cache.remove(extension_name);
        } else {
            let mut cache = self.cache.write().unwrap();
            cache.clear();
            let mut timestamp = self.cache_timestamp.write().unwrap();
            *timestamp = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_extension(temp_dir: &Path, name: &str, version: &str) -> Result<()> {
        let ext_path = temp_dir.join("extensions").join(name).join(version);
        fs::create_dir_all(&ext_path)?;

        // Create manifest
        let manifest = ExtensionManifest {
            name: name.to_string(),
            version: version.to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![],
            attrs: vec![],
            checksum: crate::manifest::Checksum {
                algorithm: crate::manifest::ChecksumAlgorithm::Sha256,
                value: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                    .to_string(),
            },
        };

        let manifest_content = serde_json::to_string_pretty(&manifest)?;
        fs::write(ext_path.join("manifest.json"), manifest_content)?;

        // Create empty WASM file
        fs::write(ext_path.join("extension.wasm"), b"")?;

        Ok(())
    }

    #[tokio::test]
    async fn test_local_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::new(temp_dir.path()).unwrap();

        assert_eq!(store.store_info().store_type, "local");
        assert!(store.store_info().trusted);
    }

    #[tokio::test]
    async fn test_health_check() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::new(temp_dir.path()).unwrap();

        let health = store.health_check().await.unwrap();
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn test_list_extensions() {
        let temp_dir = TempDir::new().unwrap();
        create_test_extension(temp_dir.path(), "test-ext", "1.0.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();
        let extensions = store.list_extensions().await.unwrap();

        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "test-ext");
        assert_eq!(extensions[0].version, "1.0.0");
    }

    #[tokio::test]
    async fn test_get_manifest() {
        let temp_dir = TempDir::new().unwrap();
        create_test_extension(temp_dir.path(), "test-ext", "1.0.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();
        let manifest = store.get_manifest("test-ext", Some("1.0.0")).await.unwrap();

        assert_eq!(manifest.name, "test-ext");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_version_management() {
        let temp_dir = TempDir::new().unwrap();
        create_test_extension(temp_dir.path(), "test-ext", "1.0.0").unwrap();
        create_test_extension(temp_dir.path(), "test-ext", "2.0.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();

        let versions = store.list_versions("test-ext").await.unwrap();
        assert_eq!(versions, vec!["2.0.0", "1.0.0"]); // Latest first

        let latest = store.get_latest_version("test-ext").await.unwrap();
        assert_eq!(latest, Some("2.0.0".to_string()));

        assert!(store.version_exists("test-ext", "1.0.0").await.unwrap());
        assert!(store.version_exists("test-ext", "2.0.0").await.unwrap());
        assert!(!store.version_exists("test-ext", "3.0.0").await.unwrap());
    }

    #[tokio::test]
    async fn test_search_functionality() {
        let temp_dir = TempDir::new().unwrap();
        create_test_extension(temp_dir.path(), "novel-scraper", "1.0.0").unwrap();
        create_test_extension(temp_dir.path(), "manga-reader", "1.0.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();

        let query = SearchQuery::new().with_text("novel".to_string());
        let results = store.search_extensions(&query).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "novel-scraper");
    }
}
