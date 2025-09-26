use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use semver::Version;
use tokio::fs;
use tracing::{debug, error, warn};
use walkdir::WalkDir;

use crate::error::{LocalStoreError, Result, StoreError};
use crate::manifest::ExtensionManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, InstallOptions, InstalledExtension,
    PackageLayout, SearchQuery, StoreHealth, StoreInfo, UpdateInfo, UpdateOptions,
};
use crate::store::{capabilities, Store};

/// Local file system-based store implementation
pub struct LocalStore {
    root_path: PathBuf,
    layout: PackageLayout,
    info: StoreInfo,
    cache: RwLock<HashMap<String, Vec<ExtensionInfo>>>,
    cache_timestamp: RwLock<Option<Instant>>,
}

impl LocalStore {
    /// Create a new LocalStore instance
    pub fn new<P: AsRef<Path>>(root_path: P) -> Result<Self> {
        let root_path = root_path.as_ref().to_path_buf();
        let info = StoreInfo::new("local".to_string(), "local".to_string())
            .with_url(format!("file://{}", root_path.display()));

        Ok(Self {
            root_path,
            layout: PackageLayout::default(),
            info,
            cache: RwLock::new(HashMap::new()),
            cache_timestamp: RwLock::new(None),
        })
    }

    /// Create a LocalStore with custom package layout
    pub fn with_layout(mut self, layout: PackageLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Validate extension name to prevent path traversal attacks
    fn validate_extension_name(&self, name: &str) -> std::result::Result<(), LocalStoreError> {
        if name.is_empty() {
            return Err(LocalStoreError::InvalidStructure(
                "Extension name cannot be empty".to_string(),
            ));
        }

        // Check for path traversal attempts
        if name.contains("..") || name.contains('/') || name.contains('\\') {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Invalid extension name '{}': contains path separators or traversal sequences",
                name
            )));
        }

        // Check for reserved names and characters
        if name.starts_with('.') || name.contains('\0') {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Invalid extension name '{}': starts with dot or contains null bytes",
                name
            )));
        }

        // Prevent extremely long names that could cause filesystem issues
        if name.len() > 255 {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Extension name '{}' is too long (max 255 characters)",
                name
            )));
        }

        Ok(())
    }

    /// Validate version string to prevent path traversal attacks
    fn validate_version_string(&self, version: &str) -> std::result::Result<(), LocalStoreError> {
        if version.is_empty() {
            return Err(LocalStoreError::InvalidStructure(
                "Version string cannot be empty".to_string(),
            ));
        }

        // Check for path traversal attempts
        if version.contains("..") || version.contains('/') || version.contains('\\') {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Invalid version '{}': contains path separators or traversal sequences",
                version
            )));
        }

        // Check for null bytes and other problematic characters
        if version.contains('\0') {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Invalid version '{}': contains null bytes",
                version
            )));
        }

        // Prevent extremely long versions
        if version.len() > 100 {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Version '{}' is too long (max 100 characters)",
                version
            )));
        }

        Ok(())
    }

    /// Get the path to an extension directory
    fn extension_path(&self, name: &str) -> std::result::Result<PathBuf, LocalStoreError> {
        self.validate_extension_name(name)?;
        Ok(self.extensions_root().join(name))
    }

    /// Get the path to a specific version of an extension
    fn extension_version_path(
        &self,
        name: &str,
        version: &str,
    ) -> std::result::Result<PathBuf, LocalStoreError> {
        self.validate_extension_name(name)?;
        self.validate_version_string(version)?;
        Ok(self.extension_path(name)?.join(version))
    }

    /// Get the path to the extensions directory
    fn extensions_root(&self) -> PathBuf {
        self.root_path.join("extensions")
    }

    /// Scan and cache extension information
    async fn scan_extensions(&self) -> Result<HashMap<String, Vec<ExtensionInfo>>> {
        let extensions_root = self.extensions_root();
        if !extensions_root.exists() {
            return Ok(HashMap::new());
        }

        let mut extensions = HashMap::new();

        for entry in WalkDir::new(&extensions_root)
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_dir())
        {
            let path = entry.path();
            let version_dir = path.file_name().and_then(|n| n.to_str());
            let extension_dir = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str());

            if let (Some(extension_name), Some(version)) = (extension_dir, version_dir) {
                // Validate names before processing
                if self.validate_extension_name(extension_name).is_err() {
                    warn!("Skipping invalid extension name: {}", extension_name);
                    continue;
                }

                if self.validate_version_string(version).is_err() {
                    warn!(
                        "Skipping invalid version: {} for extension {}",
                        version, extension_name
                    );
                    continue;
                }

                match self
                    .load_extension_info(extension_name, Some(version))
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
                            "Failed to load extension info for {}@{}: {}",
                            extension_name, version, e
                        );
                    }
                }
            }
        }

        // Sort versions for each extension
        for versions in extensions.values_mut() {
            versions.sort_by(|a, b| {
                Version::parse(&b.version)
                    .unwrap_or_else(|_| Version::new(0, 0, 0))
                    .cmp(&Version::parse(&a.version).unwrap_or_else(|_| Version::new(0, 0, 0)))
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
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?,
        };

        let version_path = self
            .extension_version_path(name, &version)
            .map_err(StoreError::from)?;
        let manifest_path = version_path.join(&self.layout.manifest_file);

        if !manifest_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "{}@{}",
                name, version
            )));
        }

        let manifest_content = fs::read_to_string(&manifest_path).await?;
        let manifest: ExtensionManifest = serde_json::from_str(&manifest_content)?;

        // Get file sizes
        let wasm_path = version_path.join(&self.layout.wasm_file);
        let size = if wasm_path.exists() {
            match fs::metadata(&wasm_path).await {
                Ok(metadata) => Some(metadata.len()),
                Err(_) => None,
            }
        } else {
            None
        };

        // Get last modified time
        let last_updated = match fs::metadata(&manifest_path).await {
            Ok(metadata) => metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| {
                    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        .unwrap_or_else(|| Utc::now())
                }),
            Err(_) => None,
        };

        Ok(ExtensionInfo {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            description: None, // Could be loaded from metadata if available
            author: manifest.author.clone(),
            tags: Vec::new(), // Could be loaded from metadata
            last_updated,
            download_count: None,
            size,
            homepage: None,
            repository: None,
            license: None,
            store_source: self.info.name.clone(),
        })
    }

    /// Get the latest version for an extension (internal helper)
    async fn get_latest_version_internal(
        &self,
        name: &str,
    ) -> std::result::Result<Option<String>, LocalStoreError> {
        let extension_dir = self.extension_path(name)?;
        if !extension_dir.exists() {
            return Ok(None);
        }

        let mut versions = Vec::new();
        let mut entries = fs::read_dir(&extension_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(version_name) = entry.file_name().to_str() {
                    // Validate version string
                    if self.validate_version_string(version_name).is_ok() {
                        versions.push(version_name.to_string());
                    }
                }
            }
        }

        if versions.is_empty() {
            return Ok(None);
        }

        // Sort by semantic version, fallback to string comparison
        versions.sort_by(|a, b| {
            match (Version::parse(a), Version::parse(b)) {
                (Ok(va), Ok(vb)) => vb.cmp(&va), // Descending order
                _ => b.cmp(a),                   // Fallback to string comparison
            }
        });

        Ok(versions.into_iter().next())
    }

    /// Refresh the extension cache
    async fn refresh_cache(&self) -> Result<()> {
        debug!("Refreshing local store cache");
        match self.scan_extensions().await {
            Ok(extensions) => {
                {
                    let mut cache = self.cache.write().unwrap();
                    *cache = extensions;
                }
                {
                    let mut timestamp = self.cache_timestamp.write().unwrap();
                    *timestamp = Some(Instant::now());
                }
                let cache_size = self.cache.read().unwrap().len();
                debug!("Cache refreshed with {} extensions", cache_size);
                Ok(())
            }
            Err(e) => {
                error!("Failed to refresh cache: {}", e);
                Err(e)
            }
        }
    }

    /// Check if the cache is valid (not expired)
    fn is_cache_valid(&self) -> bool {
        if let Some(timestamp) = *self.cache_timestamp.read().unwrap() {
            timestamp.elapsed() < Duration::from_secs(300)
        } else {
            false
        }
    }

    /// Get cached extensions, refreshing if necessary
    async fn get_cached_extensions(&self) -> Result<HashMap<String, Vec<ExtensionInfo>>> {
        if !self.is_cache_valid() {
            self.refresh_cache().await?;
        }
        Ok(self.cache.read().unwrap().clone())
    }

    /// Search through cached extensions
    fn search_cached_extensions(&self, query: &SearchQuery) -> Vec<ExtensionInfo> {
        let mut results = Vec::new();
        let cache = self.cache.read().unwrap();

        for extensions in cache.values() {
            for ext in extensions {
                let mut matches = true;

                // Text search in name, description, and author
                if let Some(text) = &query.text {
                    let text_lower = text.to_lowercase();
                    let name_match = ext.name.to_lowercase().contains(&text_lower);
                    let desc_match = ext
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&text_lower))
                        .unwrap_or(false);
                    let author_match = ext.author.to_lowercase().contains(&text_lower);

                    matches = matches && (name_match || desc_match || author_match);
                }

                // Author filter
                if let Some(author) = &query.author {
                    matches = matches && ext.author.to_lowercase().contains(&author.to_lowercase());
                }

                // Tag search (basic implementation)
                if !query.tags.is_empty() {
                    let tag_match = query.tags.iter().any(|tag| {
                        ext.tags
                            .iter()
                            .any(|ext_tag| ext_tag.to_lowercase().contains(&tag.to_lowercase()))
                    });
                    matches = matches && tag_match;
                }

                // Version filtering
                if let Some(min_version) = &query.min_version {
                    if let (Ok(ext_ver), Ok(min_ver)) =
                        (Version::parse(&ext.version), Version::parse(min_version))
                    {
                        matches = matches && ext_ver >= min_ver;
                    }
                }

                if let Some(max_version) = &query.max_version {
                    if let (Ok(ext_ver), Ok(max_ver)) =
                        (Version::parse(&ext.version), Version::parse(max_version))
                    {
                        matches = matches && ext_ver <= max_ver;
                    }
                }

                if matches {
                    results.push(ext.clone());
                }
            }
        }

        // Sort results
        match query.sort_by {
            crate::models::SearchSortBy::Name => {
                results.sort_by(|a, b| a.name.cmp(&b.name));
            }
            crate::models::SearchSortBy::Version => {
                results.sort_by(|a, b| {
                    match (Version::parse(&a.version), Version::parse(&b.version)) {
                        (Ok(va), Ok(vb)) => vb.cmp(&va),
                        _ => b.version.cmp(&a.version),
                    }
                });
            }
            crate::models::SearchSortBy::Author => {
                results.sort_by(|a, b| a.author.cmp(&b.author));
            }
            crate::models::SearchSortBy::LastUpdated => {
                results.sort_by(|a, b| {
                    b.last_updated
                        .unwrap_or_else(|| Utc::now())
                        .cmp(&a.last_updated.unwrap_or_else(|| Utc::now()))
                });
            }
            crate::models::SearchSortBy::DownloadCount => {
                results.sort_by(|a, b| {
                    b.download_count
                        .unwrap_or(0)
                        .cmp(&a.download_count.unwrap_or(0))
                });
            }
            crate::models::SearchSortBy::Size => {
                results.sort_by(|a, b| b.size.unwrap_or(0).cmp(&a.size.unwrap_or(0)));
            }
            crate::models::SearchSortBy::Relevance => {
                // For now, just sort by name as a fallback
                results.sort_by(|a, b| a.name.cmp(&b.name));
            }
        }

        // Apply limit and offset
        if let Some(offset) = query.offset {
            if offset < results.len() {
                results = results.into_iter().skip(offset).collect();
            } else {
                results.clear();
            }
        }

        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        results
    }

    /// Verify the integrity of an extension package
    async fn verify_extension_integrity(&self, name: &str, version: &str) -> Result<bool> {
        let version_path = self
            .extension_version_path(name, version)
            .map_err(StoreError::from)?;
        let manifest_path = version_path.join(&self.layout.manifest_file);
        let wasm_path = version_path.join(&self.layout.wasm_file);

        // Check if required files exist
        if !manifest_path.exists() || !wasm_path.exists() {
            return Ok(false);
        }

        // Load manifest and verify checksum
        let manifest_content = fs::read_to_string(&manifest_path).await?;
        let manifest: ExtensionManifest = serde_json::from_str(&manifest_content)?;

        let wasm_content = fs::read(&wasm_path).await?;

        // Verify checksum using enhanced system
        Ok(manifest.checksum.verify(&wasm_content))
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
        let start = Instant::now();

        // Check if root directory exists and is accessible
        match fs::metadata(&self.root_path).await {
            Ok(_) => {
                let response_time = start.elapsed();
                let extension_count = self.cache.read().unwrap().values().map(|v| v.len()).sum();

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
            if let Some(latest) = versions.first() {
                all_extensions.push(latest.clone());
            }
        }

        Ok(all_extensions)
    }

    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
        let _ = self.get_cached_extensions().await?; // Ensure cache is fresh
        Ok(self.search_cached_extensions(query))
    }

    async fn get_extension_info(&self, name: &str) -> Result<Vec<ExtensionInfo>> {
        let extensions = self.get_cached_extensions().await?;
        extensions
            .get(name)
            .cloned()
            .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))
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
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?,
        };

        let version_path = self
            .extension_version_path(name, &version)
            .map_err(StoreError::from)?;
        let manifest_path = version_path.join(&self.layout.manifest_file);

        if !manifest_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "{}@{}",
                name, version
            )));
        }

        let manifest_content = fs::read_to_string(&manifest_path).await?;
        let manifest: ExtensionManifest = serde_json::from_str(&manifest_content)?;

        Ok(manifest)
    }

    async fn get_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        let version = match version {
            Some(v) => v.to_string(),
            None => match self
                .get_latest_version_internal(name)
                .await
                .map_err(StoreError::from)?
            {
                Some(v) => v,
                None => return Ok(None),
            },
        };

        let version_path = self
            .extension_version_path(name, &version)
            .map_err(StoreError::from)?;

        if let Some(metadata_file) = &self.layout.metadata_file {
            let metadata_path = version_path.join(metadata_file);

            if metadata_path.exists() {
                let metadata_content = fs::read_to_string(&metadata_path).await?;
                let metadata: ExtensionMetadata = serde_json::from_str(&metadata_content)?;
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
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?,
        };

        let version_path = self
            .extension_version_path(name, &version)
            .map_err(StoreError::from)?;
        let wasm_path = version_path.join(&self.layout.wasm_file);

        if !wasm_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "WASM file for {}@{}",
                name, version
            )));
        }

        let wasm_content = fs::read(&wasm_path).await?;
        Ok(wasm_content)
    }

    async fn get_extension_package(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        let manifest = self.get_manifest(name, version).await?;
        let wasm_component = self.get_extension_wasm(name, version).await?;
        let metadata = self.get_metadata(name, version).await?;

        let mut package = ExtensionPackage::new(manifest, wasm_component, self.info.name.clone())
            .with_layout(self.layout.clone());

        if let Some(metadata) = metadata {
            package = package.with_metadata(metadata);
        }

        // Load additional assets if assets directory exists
        let version_str = version.unwrap_or(&package.manifest.version);
        let version_path = self
            .extension_version_path(name, version_str)
            .map_err(StoreError::from)?;

        if let Some(assets_dir) = &self.layout.assets_dir {
            let assets_path = version_path.join(assets_dir);
            if assets_path.exists() {
                for entry in WalkDir::new(&assets_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().is_file())
                {
                    let asset_path = entry.path();
                    if let Ok(relative_path) = asset_path.strip_prefix(&assets_path) {
                        if let Some(asset_name) = relative_path.to_str() {
                            match fs::read(asset_path).await {
                                Ok(content) => {
                                    package.add_asset(asset_name.to_string(), content);
                                }
                                Err(e) => {
                                    warn!("Failed to read asset {}: {}", asset_name, e);
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
        // Validate inputs
        self.validate_extension_name(name)
            .map_err(StoreError::from)?;
        if let Some(v) = version {
            self.validate_version_string(v).map_err(StoreError::from)?;
        }

        let package = self.get_extension_package(name, version).await?;
        let install_version = version.unwrap_or(&package.manifest.version);

        // Create target directory structure
        let extension_install_dir = target_dir.join(name).join(install_version);
        fs::create_dir_all(&extension_install_dir).await?;

        // Install manifest
        let manifest_path = extension_install_dir.join(&package.package_layout.manifest_file);
        let manifest_content = serde_json::to_string_pretty(&package.manifest)?;
        fs::write(&manifest_path, manifest_content).await?;

        // Install WASM component
        let wasm_path = extension_install_dir.join(&package.package_layout.wasm_file);
        fs::write(&wasm_path, &package.wasm_component).await?;

        // Install metadata if available
        if let Some(metadata) = &package.metadata {
            if let Some(metadata_file) = &package.package_layout.metadata_file {
                let metadata_path = extension_install_dir.join(metadata_file);
                let metadata_content = serde_json::to_string_pretty(metadata)?;
                fs::write(&metadata_path, metadata_content).await?;
            }
        }

        // Install assets
        if !package.assets.is_empty() {
            if let Some(assets_dir) = &package.package_layout.assets_dir {
                let assets_path = extension_install_dir.join(assets_dir);
                fs::create_dir_all(&assets_path).await?;

                for (asset_name, content) in &package.assets {
                    let asset_path = assets_path.join(asset_name);
                    if let Some(parent) = asset_path.parent() {
                        fs::create_dir_all(parent).await?;
                    }
                    fs::write(&asset_path, content).await?;
                }
            }
        }

        // Verify installation if not skipped
        if !options.skip_verification {
            if !self
                .verify_extension_integrity(name, install_version)
                .await?
            {
                return Err(StoreError::ChecksumMismatch(format!(
                    "{}@{}",
                    name, install_version
                )));
            }
        }

        let install_size = package.calculate_total_size();

        let installed = InstalledExtension::new(
            name.to_string(),
            install_version.to_string(),
            extension_install_dir,
            package.manifest,
            package.package_layout,
            self.info.name.clone(),
        );

        Ok(InstalledExtension {
            install_size,
            ..installed
        })
    }

    async fn check_updates(&self, installed: &[InstalledExtension]) -> Result<Vec<UpdateInfo>> {
        let mut updates = Vec::new();

        for ext in installed {
            if let Ok(Some(latest_version)) = self
                .get_latest_version_internal(&ext.name)
                .await
                .map_err(StoreError::from)
            {
                if latest_version != ext.version {
                    let update_info = UpdateInfo::new(
                        ext.name.clone(),
                        ext.version.clone(),
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
        self.get_latest_version_internal(name)
            .await
            .map_err(StoreError::from)
    }

    async fn update_extension(
        &self,
        name: &str,
        target_dir: &Path,
        options: &UpdateOptions,
    ) -> Result<InstalledExtension> {
        let latest_version = self
            .get_latest_version(name)
            .await?
            .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?;

        let install_options = InstallOptions {
            install_dependencies: options.update_dependencies,
            force_reinstall: options.force_update,
            skip_verification: false,
            ..Default::default()
        };

        self.install_extension(name, Some(&latest_version), target_dir, &install_options)
            .await
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>> {
        let extension_dir = self.extension_path(name).map_err(StoreError::from)?;
        if !extension_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        let mut entries = fs::read_dir(&extension_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(version_name) = entry.file_name().to_str() {
                    // Validate version string
                    if self.validate_version_string(version_name).is_ok() {
                        versions.push(version_name.to_string());
                    }
                }
            }
        }

        // Sort versions
        versions.sort_by(|a, b| {
            match (Version::parse(a), Version::parse(b)) {
                (Ok(va), Ok(vb)) => vb.cmp(&va), // Descending order
                _ => b.cmp(a),                   // Fallback to string comparison
            }
        });

        Ok(versions)
    }

    async fn version_exists(&self, name: &str, version: &str) -> Result<bool> {
        let version_path = self
            .extension_version_path(name, version)
            .map_err(StoreError::from)?;
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
            capabilities::ROLLBACK => false,
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
        ]
    }

    async fn validate_extension(&self, name: &str, version: Option<&str>) -> Result<bool> {
        let version = match version {
            Some(v) => v.to_string(),
            None => self
                .get_latest_version_internal(name)
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?,
        };

        self.verify_extension_integrity(name, &version).await
    }

    async fn clear_cache(&self, name: Option<&str>) -> Result<()> {
        match name {
            Some(ext_name) => {
                let mut cache = self.cache.write().unwrap();
                cache.remove(ext_name);
                debug!("Cleared cache for extension: {}", ext_name);
            }
            None => {
                {
                    let mut cache = self.cache.write().unwrap();
                    cache.clear();
                }
                {
                    let mut timestamp = self.cache_timestamp.write().unwrap();
                    *timestamp = None;
                }
                debug!("Cleared entire local store cache");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_extension(dir: &Path, name: &str, version: &str) -> std::io::Result<()> {
        let ext_dir = dir.join("extensions").join(name).join(version);
        std::fs::create_dir_all(&ext_dir)?;

        let manifest = ExtensionManifest {
            name: name.to_string(),
            version: version.to_string(),
            author: "test-author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![],
            attrs: vec![],
            checksum: crate::manifest::Checksum {
                algorithm: crate::manifest::ChecksumAlgorithm::Sha256,
                value: "test_hash".to_string(),
            },
            signature: None,
        };

        let manifest_content = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(ext_dir.join("manifest.json"), manifest_content)?;
        std::fs::write(ext_dir.join("extension.wasm"), b"fake wasm content")?;

        Ok(())
    }

    #[tokio::test]
    async fn test_local_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::new(temp_dir.path()).unwrap();
        assert_eq!(store.store_info().store_type, "local");
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
        create_test_extension(temp_dir.path(), "test_ext", "1.0.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();
        let extensions = store.list_extensions().await.unwrap();

        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "test_ext");
    }

    #[tokio::test]
    async fn test_get_manifest() {
        let temp_dir = TempDir::new().unwrap();
        create_test_extension(temp_dir.path(), "test_ext", "1.0.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();
        let manifest = store.get_manifest("test_ext", Some("1.0.0")).await.unwrap();

        assert_eq!(manifest.name, "test_ext");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_version_management() {
        let temp_dir = TempDir::new().unwrap();
        create_test_extension(temp_dir.path(), "test_ext", "1.0.0").unwrap();
        create_test_extension(temp_dir.path(), "test_ext", "1.1.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();
        let versions = store.list_versions("test_ext").await.unwrap();

        assert_eq!(versions.len(), 2);
        assert!(versions.contains(&"1.0.0".to_string()));
        assert!(versions.contains(&"1.1.0".to_string()));

        let exists = store.version_exists("test_ext", "1.0.0").await.unwrap();
        assert!(exists);

        let not_exists = store.version_exists("test_ext", "2.0.0").await.unwrap();
        assert!(!not_exists);
    }

    #[tokio::test]
    async fn test_search_functionality() {
        let temp_dir = TempDir::new().unwrap();
        create_test_extension(temp_dir.path(), "novel_scraper", "1.0.0").unwrap();
        create_test_extension(temp_dir.path(), "manga_reader", "1.0.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();

        let query = SearchQuery::new().with_text("novel".to_string());
        let results = store.search_extensions(&query).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "novel_scraper");
    }

    #[tokio::test]
    async fn test_path_traversal_protection() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::new(temp_dir.path()).unwrap();

        // Test various path traversal attempts
        let malicious_names = vec![
            "../evil",
            "..\\evil",
            "test/../evil",
            "test\\..\\evil",
            ".evil",
            "test\0evil",
        ];

        for name in malicious_names {
            assert!(store.validate_extension_name(name).is_err());
        }

        // Test valid names
        let valid_names = vec!["test", "test_extension", "test-extension", "test123"];
        for name in valid_names {
            assert!(store.validate_extension_name(name).is_ok());
        }
    }
}
