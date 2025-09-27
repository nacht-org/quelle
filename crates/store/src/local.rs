use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::Utc;
use semver::Version;
use tokio::fs;
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

use crate::error::{LocalStoreError, Result, StoreError};
use crate::manifest::ExtensionManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, InstalledExtension, PackageLayout,
    SearchQuery, StoreHealth, UpdateInfo,
};
use crate::publish::{
    ExtensionVisibility, PublishError, PublishOptions, PublishPermissions, PublishRequirements,
    PublishResult, PublishStats, PublishUpdateOptions, PublishableStore, RateLimitStatus,
    RateLimits, UnpublishOptions, UnpublishResult, ValidationReport,
};
use crate::store::{capabilities, Store};
use crate::store_manifest::{ExtensionSummary, StoreManifest, UrlPattern};
use crate::validation::{create_default_validator, ValidationEngine};

/// Local store manifest that extends the base StoreManifest with URL routing
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub(crate) struct LocalStoreManifest {
    /// Base store manifest
    #[serde(flatten)]
    pub base: StoreManifest,

    /// URL Routing & Domain Support
    pub url_patterns: Vec<UrlPattern>,
    pub supported_domains: Vec<String>,

    /// Extension Index for Fast Lookups
    pub extension_count: u32,
    pub extensions: Vec<ExtensionSummary>,
}

impl LocalStoreManifest {
    /// Create a new local store manifest
    pub fn new(base: StoreManifest) -> Self {
        Self {
            base,
            url_patterns: Vec::new(),
            supported_domains: Vec::new(),
            extension_count: 0,
            extensions: Vec::new(),
        }
    }

    /// Add a URL pattern for extension matching
    fn add_url_pattern(&mut self, url_prefix: String, extension: String, priority: u8) {
        // Check if pattern already exists
        if let Some(pattern) = self
            .url_patterns
            .iter_mut()
            .find(|p| p.url_prefix == url_prefix)
        {
            // Add extension if not already present
            if !pattern.extensions.contains(&extension) {
                pattern.extensions.insert(extension);
            }
        } else {
            // Create new pattern
            self.url_patterns.push(UrlPattern {
                url_prefix,
                extensions: [extension].into_iter().collect(),
                priority,
            });
        }

        // Sort patterns by priority (higher first)
        self.url_patterns
            .sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Add an extension summary to the manifest
    pub(crate) fn add_extension(&mut self, extension: ExtensionSummary) {
        // Update supported domains from extension base URLs
        for base_url in &extension.base_urls {
            if let Ok(parsed) = url::Url::parse(base_url) {
                if let Some(domain) = parsed.domain() {
                    let domain = domain.to_string();
                    if !self.supported_domains.contains(&domain) {
                        self.supported_domains.push(domain);
                    }
                }
            }

            self.add_url_pattern(base_url.clone(), extension.name.clone(), 100);
        }

        self.extensions.push(extension);
        self.extension_count = self.extensions.len() as u32;
        self.supported_domains.sort();
        self.base.last_updated = chrono::Utc::now();
    }

    /// Find extensions that can handle the given URL
    /// Returns (id, name) pairs
    pub(crate) fn find_extensions_for_url(&self, url: &str) -> Vec<(String, String)> {
        let mut matches = Vec::new();

        // Check URL patterns first (sorted by priority)
        for pattern in &self.url_patterns {
            if url.starts_with(&pattern.url_prefix) {
                // Convert extension names in patterns to (id, name) pairs
                for ext_name in &pattern.extensions {
                    // Find the extension to get its ID
                    if let Some(ext) = self.extensions.iter().find(|e| &e.name == ext_name) {
                        matches.push((ext.id.clone(), ext.name.clone()));
                    }
                }
            }
        }

        // If no pattern matches, check individual extension base URLs
        if matches.is_empty() {
            for ext in &self.extensions {
                for base_url in &ext.base_urls {
                    if url.starts_with(base_url) {
                        matches.push((ext.id.clone(), ext.name.clone()));
                    }
                }
            }
        }

        // Remove duplicates while preserving order
        let mut unique_matches = Vec::new();
        for m in matches {
            if !unique_matches.contains(&m) {
                unique_matches.push(m);
            }
        }

        unique_matches
    }

    /// Get extensions that support a specific domain
    pub(crate) fn find_extensions_for_domain(&self, domain: &str) -> Vec<String> {
        let mut matches = Vec::new();

        for ext in &self.extensions {
            for base_url in &ext.base_urls {
                if let Ok(parsed) = url::Url::parse(base_url) {
                    if let Some(url_domain) = parsed.domain() {
                        if url_domain == domain {
                            matches.push(ext.name.clone());
                            break;
                        }
                    }
                }
            }
        }

        matches
    }
}

/// Local file system-based store implementation
pub struct LocalStore {
    root_path: PathBuf,
    layout: PackageLayout,
    cache: RwLock<HashMap<String, Vec<ExtensionInfo>>>,
    cache_timestamp: RwLock<Option<Instant>>,
    validator: ValidationEngine,
}

impl LocalStore {
    /// Create a new LocalStore instance
    pub fn new<P: AsRef<Path>>(root_path: P) -> Result<Self> {
        let root_path = root_path.as_ref().to_path_buf();

        Ok(Self {
            root_path,
            layout: PackageLayout::default(),
            cache: RwLock::new(HashMap::new()),
            cache_timestamp: RwLock::new(None),
            validator: create_default_validator(),
        })
    }

    /// Create a LocalStore with custom package layout
    pub fn with_layout(mut self, layout: PackageLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Create a LocalStore with a custom name (deprecated - use initialize_store)
    pub fn with_name(self, _name: String) -> Self {
        // Store name is now managed in the manifest
        self
    }

    /// Initialize the store with proper metadata
    pub async fn initialize_store(
        &self,
        store_name: String,
        description: Option<String>,
    ) -> Result<()> {
        let manifest_path = self.root_path.join("store.json");

        // Don't overwrite existing manifest
        if manifest_path.exists() {
            return Ok(());
        }

        // Create initial manifest with provided metadata
        let base_manifest =
            StoreManifest::new(store_name, "local".to_string(), "1.0.0".to_string())
                .with_url(format!("file://{}", self.root_path.display()))
                .with_description(
                    description.unwrap_or_else(|| "Local extension store".to_string()),
                );

        let local_manifest = LocalStoreManifest::new(base_manifest);

        let content = serde_json::to_string_pretty(&local_manifest)
            .map_err(StoreError::SerializationError)?;

        // Ensure directory exists
        if let Some(parent) = manifest_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| StoreError::IoError(e))?;
        }

        fs::write(&manifest_path, content)
            .await
            .map_err(|e| StoreError::IoOperation {
                operation: "write initial store manifest".to_string(),
                path: manifest_path,
                source: e,
            })?;

        Ok(())
    }

    /// Validate extension id to prevent path traversal attacks
    fn validate_extension_id(&self, id: &str) -> std::result::Result<(), LocalStoreError> {
        if id.is_empty() {
            return Err(LocalStoreError::InvalidStructure(
                "Extension id cannot be empty".to_string(),
            ));
        }

        // Check for path traversal attempts
        if id.contains("..") || id.contains('/') || id.contains('\\') {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Invalid extension id '{}': contains path separators or traversal sequences",
                id
            )));
        }

        // Check for reserved names and characters
        if id.starts_with('.') || id.contains('\0') {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Invalid extension id '{}': starts with dot or contains null bytes",
                id
            )));
        }

        // Prevent extremely long names that could cause filesystem issues
        if id.len() > 255 {
            return Err(LocalStoreError::InvalidStructure(format!(
                "Extension id '{}' is too long (max 255 characters)",
                id
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
    fn extension_path(&self, id: &str) -> std::result::Result<PathBuf, LocalStoreError> {
        self.validate_extension_id(id)?;
        Ok(self.extensions_root().join(id))
    }

    /// Get the path to a specific version of an extension
    fn extension_version_path(
        &self,
        id: &str,
        version: &str,
    ) -> std::result::Result<PathBuf, LocalStoreError> {
        self.validate_extension_id(id)?;
        self.validate_version_string(version)?;
        Ok(self.extension_path(id)?.join(version))
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
                if self.validate_extension_id(extension_name).is_err() {
                    warn!("Skipping invalid extension id: {}", extension_name);
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
            id: manifest.id.clone(),
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
            store_source: "local".to_string(),
        })
    }

    /// Get the latest version for an extension (internal helper)
    async fn get_latest_version_internal(
        &self,
        id: &str,
    ) -> std::result::Result<Option<String>, LocalStoreError> {
        let extension_dir = self.extension_path(id)?;
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
    async fn get_store_manifest(&self) -> Result<StoreManifest> {
        let manifest_path = self.root_path.join("store.json");

        // Try to read existing manifest
        if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path).await?;
            let local_manifest: LocalStoreManifest =
                serde_json::from_str(&content).map_err(StoreError::from)?;
            return Ok(local_manifest.base);
        }

        // If no manifest exists, the store hasn't been properly initialized
        Err(StoreError::InvalidPackage {
            reason: "Store manifest not found. Use initialize_store() to create a properly configured store.".to_string(),
        })
    }

    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        let local_manifest = self.get_local_store_manifest().await?;
        Ok(local_manifest.find_extensions_for_url(url))
    }

    async fn find_extensions_for_domain(&self, domain: &str) -> Result<Vec<String>> {
        let local_manifest = self.get_local_store_manifest().await?;
        Ok(local_manifest.find_extensions_for_domain(domain))
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        let start = Instant::now();

        // Check if root directory exists and is accessible
        let metadata = match fs::metadata(&self.root_path).await {
            Ok(metadata) => metadata,
            Err(e) => {
                return Ok(StoreHealth::unhealthy(format!(
                    "Cannot access store directory: {}",
                    e
                )))
            }
        };

        // Ensure it's actually a directory
        if !metadata.is_dir() {
            return Ok(StoreHealth::unhealthy(
                "Store path is not a directory".to_string(),
            ));
        }

        // Try to read directory contents to validate it's a proper store
        let mut dir_entries = match fs::read_dir(&self.root_path).await {
            Ok(entries) => entries,
            Err(e) => {
                return Ok(StoreHealth::unhealthy(format!(
                    "Cannot read store directory: {}",
                    e
                )))
            }
        };

        let mut has_extensions = false;
        let mut extension_count = 0;
        let mut validation_errors = Vec::new();

        // Check directory structure and validate extensions
        while let Some(entry) =
            dir_entries
                .next_entry()
                .await
                .map_err(|e| StoreError::IoOperation {
                    operation: "read directory entry".to_string(),
                    path: self.root_path.clone(),
                    source: e,
                })?
        {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                let extension_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");

                // Validate extension id
                if let Err(e) = self.validate_extension_id(extension_name) {
                    validation_errors
                        .push(format!("Invalid extension id '{}': {}", extension_name, e));
                    continue;
                }

                // Check if extension has valid structure (at least one version directory)
                match fs::read_dir(&entry_path).await {
                    Ok(mut version_entries) => {
                        let mut has_versions = false;
                        while let Some(version_entry) =
                            version_entries.next_entry().await.map_err(|_| {
                                StoreError::InvalidPackage {
                                    reason: "Cannot read version directory".to_string(),
                                }
                            })?
                        {
                            if version_entry.path().is_dir() {
                                let version_path = version_entry.path();
                                let version_name = version_path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("unknown");

                                // Validate version string
                                if let Err(e) = self.validate_version_string(version_name) {
                                    validation_errors.push(format!(
                                        "Invalid version '{}' in extension '{}': {}",
                                        version_name, extension_name, e
                                    ));
                                    continue;
                                }

                                // Check if version has required files (manifest.json)
                                let manifest_path =
                                    version_entry.path().join(&self.layout.manifest_file);
                                if !manifest_path.exists() {
                                    validation_errors.push(format!(
                                        "Missing manifest in {}@{}",
                                        extension_name, version_name
                                    ));
                                } else {
                                    has_versions = true;
                                    extension_count += 1;
                                }
                            }
                        }

                        if has_versions {
                            has_extensions = true;
                        }
                    }
                    Err(_) => {
                        validation_errors.push(format!(
                            "Cannot read extension directory: {}",
                            extension_name
                        ));
                    }
                }
            }
        }

        let response_time = start.elapsed();

        // Return health status based on validation
        if validation_errors.is_empty() {
            Ok(StoreHealth::healthy()
                .with_response_time(response_time)
                .with_extension_count(extension_count))
        } else if has_extensions {
            // Some extensions are valid, but there are issues
            Ok(StoreHealth::healthy()
                .with_response_time(response_time)
                .with_extension_count(extension_count))
        } else {
            // No valid extensions found or serious structural issues
            Ok(StoreHealth::unhealthy(format!(
                "Invalid store structure: {}",
                validation_errors.join("; ")
            )))
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

    async fn get_manifest(&self, id: &str, version: Option<&str>) -> Result<ExtensionManifest> {
        let version = match version {
            Some(v) => v.to_string(),
            None => self
                .get_latest_version_internal(id)
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::ExtensionNotFound(id.to_string()))?,
        };

        let version_path = self
            .extension_version_path(id, &version)
            .map_err(StoreError::from)?;
        let manifest_path = version_path.join(&self.layout.manifest_file);

        if !manifest_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!("{}@{}", id, version)));
        }

        let manifest_content = fs::read_to_string(&manifest_path).await?;
        let manifest: ExtensionManifest = serde_json::from_str(&manifest_content)?;

        Ok(manifest)
    }

    async fn get_metadata(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        let version = match version {
            Some(v) => v.to_string(),
            None => match self
                .get_latest_version_internal(id)
                .await
                .map_err(StoreError::from)?
            {
                Some(v) => v,
                None => return Ok(None),
            },
        };

        let version_path = self
            .extension_version_path(id, &version)
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

    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        let manifest = self.get_manifest(id, version).await?;
        let wasm_component = self.get_extension_wasm_internal(id, version).await?;
        let metadata = self.get_metadata(id, version).await?;

        let mut package = ExtensionPackage::new(manifest, wasm_component, "local".to_string())
            .with_layout(self.layout.clone());

        if let Some(metadata) = metadata {
            package = package.with_metadata(metadata);
        }

        // Load additional assets if assets directory exists
        let version_str = version.unwrap_or(&package.manifest.version);
        let version_path = self
            .extension_version_path(id, version_str)
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
                        "local".to_string(),
                    );
                    updates.push(update_info);
                }
            }
        }

        Ok(updates)
    }

    async fn get_latest_version(&self, id: &str) -> Result<Option<String>> {
        self.get_latest_version_internal(id)
            .await
            .map_err(StoreError::from)
    }

    async fn list_versions(&self, id: &str) -> Result<Vec<String>> {
        let extension_dir = self.extension_path(id).map_err(StoreError::from)?;
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

    async fn version_exists(&self, id: &str, version: &str) -> Result<bool> {
        let version_path = self
            .extension_version_path(id, version)
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
}

impl LocalStore {
    /// Get the local store manifest (internal method)
    async fn get_local_store_manifest(&self) -> Result<LocalStoreManifest> {
        let manifest_path = self.root_path.join("store.json");
        if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path).await?;
            let manifest: LocalStoreManifest =
                serde_json::from_str(&content).map_err(StoreError::from)?;
            Ok(manifest)
        } else {
            Err(StoreError::InvalidPackage {
                reason: "Store manifest not found. Use initialize_store() to create a new store with proper metadata".to_string(),
            })
        }
    }

    /// Generate a local store manifest from the current state of the store
    async fn generate_local_store_manifest(&self) -> Result<LocalStoreManifest> {
        // Try to preserve existing store metadata from manifest file
        let manifest_path = self.root_path.join("store.json");
        let base_manifest = if manifest_path.exists() {
            if let Ok(content) = fs::read_to_string(&manifest_path).await {
                if let Ok(existing_manifest) = serde_json::from_str::<LocalStoreManifest>(&content)
                {
                    // Preserve existing base manifest metadata but update URL and timestamp
                    let mut base = existing_manifest
                        .base
                        .with_url(format!("file://{}", self.root_path.display()));
                    base.touch();
                    // Ensure store_type is always "local" for local stores
                    base.store_type = "local".to_string();
                    base
                } else {
                    // If we can't parse existing manifest, we can't recover metadata
                    return Err(StoreError::InvalidPackage {
                        reason: "Existing store manifest is corrupted and cannot be parsed"
                            .to_string(),
                    });
                }
            } else {
                // If we can't read existing manifest, we can't recover metadata
                return Err(StoreError::IoError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Cannot read existing store manifest",
                )));
            }
        } else {
            // No existing manifest - this should only happen during initial store creation
            // The initialize_store method should be called first to create proper metadata
            return Err(StoreError::InvalidPackage {
                reason: "Store manifest not found. Use initialize_store() to create a new store with proper metadata".to_string(),
            });
        };

        // Always create a fresh manifest with current extensions
        // (don't preserve old extensions list as it may be stale)
        let mut local_manifest = LocalStoreManifest::new(base_manifest);

        // Scan for extensions and build manifest
        let extensions = self.list_extensions().await?;

        for ext_info in extensions {
            // Get the extension manifest to extract base_urls
            if let Ok(ext_manifest) = self
                .get_manifest(&ext_info.name, Some(&ext_info.version))
                .await
            {
                let summary = ExtensionSummary {
                    id: ext_manifest.id.clone(),
                    name: ext_info.name.clone(),
                    version: ext_info.version.clone(),
                    base_urls: ext_manifest.base_urls.clone(),
                    langs: ext_manifest.langs.clone(),
                    last_updated: ext_info.last_updated.unwrap_or_else(|| Utc::now()),
                };

                local_manifest.add_extension(summary);
            }
        }
        Ok(local_manifest)
    }

    /// Save the store manifest to disk
    pub async fn save_store_manifest(&self) -> Result<()> {
        let manifest = self.generate_local_store_manifest().await?;
        let manifest_path = self.root_path.join("store.json");

        let content =
            serde_json::to_string_pretty(&manifest).map_err(StoreError::SerializationError)?;

        fs::write(&manifest_path, content)
            .await
            .map_err(|e| StoreError::IoOperation {
                operation: "write store manifest".to_string(),
                path: manifest_path,
                source: e,
            })?;

        Ok(())
    }

    /// Get the package layout used by this store (LocalStore specific)
    pub fn package_layout(&self) -> &PackageLayout {
        &self.layout
    }

    /// Internal method to get WASM bytes
    async fn get_extension_wasm_internal(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<Vec<u8>> {
        let version = match version {
            Some(v) => v.to_string(),
            None => self
                .get_latest_version_internal(id)
                .await?
                .ok_or_else(|| StoreError::ExtensionNotFound(id.to_string()))?,
        };

        let version_path = self.extension_version_path(id, &version)?;
        let wasm_path = version_path.join(&self.layout.wasm_file);

        if !wasm_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "WASM file not found for {}@{}",
                id, version
            )));
        }

        let wasm_bytes = fs::read(&wasm_path).await?;
        Ok(wasm_bytes)
    }

    /// Get the raw WASM bytes for an extension (LocalStore specific)
    pub async fn get_extension_wasm(&self, name: &str, version: Option<&str>) -> Result<Vec<u8>> {
        self.validate_extension_id(name).map_err(StoreError::from)?;
        if let Some(v) = version {
            self.validate_version_string(v).map_err(StoreError::from)?;
        }

        let version = match version {
            Some(v) => v.to_string(),
            None => self
                .get_latest_version_internal(name)
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| {
                    StoreError::ExtensionNotFound(format!("No versions found for {}", name))
                })?,
        };

        let version_path = self.extension_version_path(name, &version)?;
        let wasm_path = version_path.join(&self.layout.wasm_file);

        if !wasm_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "WASM file not found for {}@{}",
                name, version
            )));
        }

        let wasm_bytes = fs::read(&wasm_path).await?;

        // Verify checksum if available
        if !self.verify_extension_integrity(name, &version).await? {
            return Err(StoreError::ChecksumMismatch(format!(
                "{}@{}",
                name, version
            )));
        }

        Ok(wasm_bytes)
    }

    /// Download and cache an extension package for faster access (LocalStore specific)
    pub async fn cache_extension(&self, name: &str, version: Option<&str>) -> Result<()> {
        // Just verify the package exists and cache will be updated
        let _ = self.get_extension_package(name, version).await?;
        Ok(())
    }

    /// Clear cached data for an extension (LocalStore specific)
    pub async fn clear_cache(&self, name: Option<&str>) -> Result<()> {
        use tracing::info;
        match name {
            Some(extension_name) => {
                // Clear cache for specific extension
                let mut cache = self.cache.write().unwrap();
                cache.remove(extension_name);
                info!("Cleared cache for extension '{}'", extension_name);
            }
            None => {
                // Clear all cache
                let mut cache = self.cache.write().unwrap();
                let count = cache.len();
                cache.clear();
                info!("Cleared cache for {} extensions", count);
            }
        }
        Ok(())
    }

    /// Validate the integrity of an extension package (LocalStore specific)
    pub async fn validate_extension(&self, name: &str, version: Option<&str>) -> Result<bool> {
        let version = match version {
            Some(v) => v.to_string(),
            None => self
                .get_latest_version_internal(name)
                .await
                .map_err(StoreError::from)?
                .ok_or_else(|| StoreError::ExtensionNotFound(name.to_string()))?,
        };

        self.verify_extension_integrity(name, &version)
            .await
            .map_err(StoreError::from)
    }
}

#[async_trait]
impl PublishableStore for LocalStore {
    async fn publish_extension(
        &mut self,
        package: ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<PublishResult> {
        let id = &package.manifest.id;
        let name = &package.manifest.name;
        let version = &package.manifest.version;

        info!(
            "Publishing extension {} (id: {})@{} to local store",
            name, id, version
        );

        // Check if version already exists
        if !options.overwrite_existing {
            if self.version_exists(id, version).await? {
                return Err(crate::error::StoreError::PublishError(
                    PublishError::VersionAlreadyExists(version.clone()),
                ));
            }
        }

        // Validate package if not skipped
        if !options.skip_validation {
            let validation_report = self.validator.validate(&package).await?;
            if !validation_report.passed {
                let critical_count = validation_report
                    .issues
                    .iter()
                    .filter(|i| matches!(i.severity, crate::registry::IssueSeverity::Critical))
                    .count();
                if critical_count > 0 {
                    return Err(crate::error::StoreError::PublishError(
                        PublishError::ValidationFailed(critical_count),
                    ));
                }
            }
        }

        // Create extension and version directories using id
        let extension_dir = self.extensions_root().join(id);
        let version_path = extension_dir.join(version);

        fs::create_dir_all(&version_path)
            .await
            .map_err(|e| crate::error::StoreError::IoError(e))?;

        // Write manifest
        let manifest_path = version_path.join(&self.layout.manifest_file);
        let manifest_content = serde_json::to_string_pretty(&package.manifest)
            .map_err(|e| crate::error::StoreError::SerializationError(e))?;
        fs::write(&manifest_path, manifest_content)
            .await
            .map_err(|e| crate::error::StoreError::IoError(e))?;

        // Write WASM component
        let wasm_path = version_path.join(&self.layout.wasm_file);
        fs::write(&wasm_path, &package.wasm_component)
            .await
            .map_err(|e| crate::error::StoreError::IoError(e))?;

        // Write assets
        for (asset_name, content) in &package.assets {
            let asset_path = version_path.join(asset_name);
            if let Some(parent) = asset_path.parent() {
                fs::create_dir_all(parent)
                    .await
                    .map_err(|e| crate::error::StoreError::IoError(e))?;
            }
            fs::write(asset_path, content)
                .await
                .map_err(|e| crate::error::StoreError::IoError(e))?;
        }

        // Calculate package size
        let package_size = package.calculate_total_size();
        let content_hash = package.manifest.checksum.value.clone();

        // Clear cache to ensure fresh data
        {
            let mut cache = self.cache.write().unwrap();
            cache.remove(name);
            *self.cache_timestamp.write().unwrap() = None;
        }

        // Create publication ID
        let publication_id = format!("{}@{}-{}", name, version, Utc::now().timestamp());

        let result = PublishResult::success(
            version.clone(),
            format!("file://{}", wasm_path.display()),
            publication_id,
            package_size,
            content_hash,
        );

        info!("Successfully published extension {}@{}", name, version);

        // Update store manifest after successful publish
        if let Err(e) = self.save_store_manifest().await {
            warn!(
                "Failed to update store manifest after publishing {}: {}",
                name, e
            );
        }

        Ok(result)
    }

    async fn update_extension(
        &mut self,
        _name: &str,
        package: ExtensionPackage,
        options: &PublishUpdateOptions,
    ) -> Result<PublishResult> {
        // For local stores, update is the same as publish with overwrite
        let mut publish_options = options.publish_options.clone();
        publish_options.overwrite_existing = true;

        self.publish_extension(package, &publish_options).await
    }

    async fn unpublish_extension(
        &mut self,
        id: &str,
        version: &str,
        _options: &UnpublishOptions,
    ) -> Result<UnpublishResult> {
        info!("Unpublishing extension {}@{}", id, version);

        let version_path = self
            .extension_version_path(id, version)
            .map_err(crate::error::StoreError::from)?;

        if !version_path.exists() {
            return Err(crate::error::StoreError::ExtensionNotFound(format!(
                "{}@{}",
                id, version
            )));
        }

        // Remove the version directory
        fs::remove_dir_all(&version_path)
            .await
            .map_err(|e| crate::error::StoreError::IoError(e))?;

        // Check if extension directory is now empty
        let extension_path = self.root_path.join(id);
        if extension_path.exists() {
            let mut entries = fs::read_dir(&extension_path)
                .await
                .map_err(|e| crate::error::StoreError::IoError(e))?;

            let has_entries = entries
                .next_entry()
                .await
                .map_err(|e| crate::error::StoreError::IoError(e))?
                .is_some();

            if !has_entries {
                fs::remove_dir(&extension_path)
                    .await
                    .map_err(|e| crate::error::StoreError::IoError(e))?;
            }
        }

        // Clear cache
        {
            let mut cache = self.cache.write().unwrap();
            cache.remove(id);
            *self.cache_timestamp.write().unwrap() = None;
        }

        // Update store manifest after successful unpublish
        if let Err(e) = self.save_store_manifest().await {
            return Err(crate::error::StoreError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Failed to update store manifest after unpublishing {}: {}",
                    id, e
                ),
            )));
        }

        Ok(UnpublishResult {
            version: version.to_string(),
            unpublished_at: Utc::now(),
            tombstone_created: false,
            users_notified: None,
        })
    }

    async fn validate_publish(
        &self,
        package: &ExtensionPackage,
        options: &PublishOptions,
    ) -> Result<ValidationReport> {
        // Use the integrated validation engine for comprehensive validation
        let validation_report = self.validator.validate(package).await?;

        // Additional store-specific validations
        let mut additional_issues = Vec::new();
        let start = Instant::now();

        // Check package requirements
        let requirements = self.publish_requirements();

        // Check package size
        if let Some(max_size) = requirements.max_package_size {
            let package_size = package.calculate_total_size();
            if package_size > max_size {
                additional_issues.push(crate::registry::ValidationIssue {
                    extension_name: package.manifest.name.clone(),
                    issue_type: crate::registry::ValidationIssueType::InvalidManifest,
                    description: format!(
                        "Package size {} exceeds maximum {}",
                        package_size, max_size
                    ),
                    severity: crate::registry::IssueSeverity::Critical,
                });
            }
        }

        // Check visibility support
        if !requirements
            .supported_visibility
            .contains(&options.visibility)
        {
            additional_issues.push(crate::registry::ValidationIssue {
                extension_name: package.manifest.name.clone(),
                issue_type: crate::registry::ValidationIssueType::InvalidManifest,
                description: format!("Visibility {:?} not supported", options.visibility),
                severity: crate::registry::IssueSeverity::Error,
            });
        }

        // Combine validation engine results with store-specific validation
        let mut all_issues = validation_report.issues;
        all_issues.extend(additional_issues);

        let validation_duration = validation_report.validation_duration + start.elapsed();
        let passed = validation_report.passed
            && !all_issues
                .iter()
                .any(|i| matches!(i.severity, crate::registry::IssueSeverity::Critical));

        Ok(ValidationReport {
            passed,
            issues: all_issues,
            validation_duration,
            validator_version: env!("CARGO_PKG_VERSION").to_string(),
            metadata: HashMap::new(),
        })
    }

    fn publish_requirements(&self) -> PublishRequirements {
        PublishRequirements {
            requires_authentication: false,
            requires_signing: false,
            max_package_size: Some(100 * 1024 * 1024), // 100MB
            supported_visibility: vec![ExtensionVisibility::Public, ExtensionVisibility::Unlisted],
            ..Default::default()
        }
    }

    async fn can_publish(&self, _extension_id: &str) -> Result<PublishPermissions> {
        Ok(PublishPermissions {
            can_publish: true,
            can_update: true,
            can_unpublish: true,
            allowed_extensions: None, // All extensions allowed
            max_package_size: Some(100 * 1024 * 1024), // 100MB
            rate_limits: RateLimits {
                publications_per_hour: None,
                publications_per_day: None,
                bandwidth_per_day: None,
            },
        })
    }

    async fn get_publish_stats(&self) -> Result<PublishStats> {
        let extensions = self.list_extensions().await?;
        let total_extensions = extensions.len() as u64;

        // Calculate approximate storage (this is a rough estimate)
        let mut total_storage = 0u64;
        for _extension in &extensions {
            // Rough estimate based on typical extension sizes
            total_storage += 1024 * 1024; // 1MB per extension as estimate
        }

        Ok(PublishStats {
            total_extensions,
            total_storage_used: total_storage,
            recent_publications: 0, // Local store doesn't track this
            storage_quota: None,    // No quota for local store
            rate_limit_status: RateLimitStatus {
                publications_remaining: None,
                reset_time: None,
                is_limited: false,
            },
            store_specific: HashMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::checksum::{Checksum, ChecksumAlgorithm};
    use tempfile::TempDir;

    fn create_test_extension(dir: &Path, name: &str, version: &str) -> std::io::Result<()> {
        let ext_dir = dir.join("extensions").join(name).join(version);
        std::fs::create_dir_all(&ext_dir)?;

        let manifest = ExtensionManifest {
            id: format!("test-{}", name),
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
        let manifest = store.get_store_manifest().await.unwrap();
        assert_eq!(manifest.store_type, "local");
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
            assert!(store.validate_extension_id(name).is_err());
        }

        // Test valid names
        let valid_names = vec!["test", "test_extension", "test-extension", "test123"];
        for name in valid_names {
            assert!(store.validate_extension_id(name).is_ok());
        }
    }

    #[tokio::test]
    async fn test_publish_extension() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = LocalStore::new(temp_dir.path()).unwrap();

        // Valid WASM magic number + version + minimal content
        let valid_wasm = [
            0x00, 0x61, 0x73, 0x6d, // WASM magic number
            0x01, 0x00, 0x00, 0x00, // WASM version 1
            0x00, // Minimal content
        ];

        // Create a test extension package
        let manifest = ExtensionManifest {
            id: "test-extension".to_string(),
            name: "test-extension".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![crate::manifest::ReadingDirection::Ltr],
            attrs: vec![],
            checksum: Checksum::from_data(ChecksumAlgorithm::Sha256, &valid_wasm),
            signature: None,
        };

        let package =
            ExtensionPackage::new(manifest, valid_wasm.to_vec(), "test-store".to_string());

        let options = PublishOptions::default();

        // Test publishing
        let result = store.publish_extension(package, &options).await.unwrap();

        assert_eq!(result.version, "1.0.0");
        assert!(result.download_url.contains("test-extension"));
        assert_eq!(result.package_size, valid_wasm.len() as u64);

        // Verify the extension was actually published
        let extension_info = store
            .get_extension_version_info("test-extension", Some("1.0.0"))
            .await
            .unwrap();
        assert_eq!(extension_info.name, "test-extension");
        assert_eq!(extension_info.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_publish_validation() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::new(temp_dir.path()).unwrap();

        // Create an invalid package (empty name)
        let manifest = ExtensionManifest {
            id: "".to_string(),   // Invalid empty id
            name: "".to_string(), // Invalid empty name
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![crate::manifest::ReadingDirection::Ltr],
            attrs: vec![],
            checksum: Checksum::from_data(ChecksumAlgorithm::Sha256, b"test wasm content"),
            signature: None,
        };

        // Invalid WASM content (empty)
        let package = ExtensionPackage::new(
            manifest,
            vec![], // Empty content will fail validation
            "test-store".to_string(),
        );

        let options = PublishOptions::default();

        // Test validation
        let validation = store.validate_publish(&package, &options).await.unwrap();

        assert!(!validation.passed);
        assert!(!validation.issues.is_empty());
        assert!(validation.has_critical_issues());
    }

    #[tokio::test]
    async fn test_unpublish_extension() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = LocalStore::new(temp_dir.path()).unwrap();

        // Valid WASM magic number + version + minimal content
        let valid_wasm = [
            0x00, 0x61, 0x73, 0x6d, // WASM magic number
            0x01, 0x00, 0x00, 0x00, // WASM version 1
            0x00, // Minimal content
        ];

        // First publish an extension
        let manifest = ExtensionManifest {
            id: "test-extension".to_string(),
            name: "test-extension".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![crate::manifest::ReadingDirection::Ltr],
            attrs: vec![],
            checksum: Checksum::from_data(ChecksumAlgorithm::Sha256, &valid_wasm),
            signature: None,
        };

        let package =
            ExtensionPackage::new(manifest, valid_wasm.to_vec(), "test-store".to_string());

        let options = PublishOptions::default();
        store.publish_extension(package, &options).await.unwrap();

        // Verify it exists
        assert!(store
            .version_exists("test-extension", "1.0.0")
            .await
            .unwrap());

        // Now unpublish it
        let unpublish_options = UnpublishOptions {
            access_token: None,
            reason: Some("Test unpublish".to_string()),
            keep_record: false,
            notify_users: false,
        };

        let result = store
            .unpublish_extension("test-extension", "1.0.0", &unpublish_options)
            .await
            .unwrap();

        assert_eq!(result.version, "1.0.0");
        assert!(!result.tombstone_created);

        // Verify it no longer exists
        assert!(!store
            .version_exists("test-extension", "1.0.0")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_validation_integration() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = LocalStore::new(temp_dir.path()).unwrap();

        // Test 1: Valid extension should pass validation and publish
        let valid_wasm = [
            0x00, 0x61, 0x73, 0x6d, // WASM magic number
            0x01, 0x00, 0x00, 0x00, // WASM version 1
            0x00, // Minimal content
        ];

        let valid_manifest = ExtensionManifest {
            id: "valid-extension".to_string(),
            name: "valid-extension".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![crate::manifest::ReadingDirection::Ltr],
            attrs: vec![],
            checksum: Checksum::from_data(ChecksumAlgorithm::Sha256, &valid_wasm),
            signature: None,
        };

        let valid_package = ExtensionPackage::new(
            valid_manifest,
            valid_wasm.to_vec(),
            "test-store".to_string(),
        );

        let result = store
            .publish_extension(valid_package, &PublishOptions::default())
            .await;
        assert!(
            result.is_ok(),
            "Valid extension should publish successfully"
        );

        // Test 2: Invalid extension should fail validation
        let invalid_wasm = [0x12, 0x34, 0x56, 0x78]; // Invalid magic number

        let invalid_manifest = ExtensionManifest {
            id: "invalid-extension".to_string(),
            name: "invalid-extension".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![crate::manifest::ReadingDirection::Ltr],
            attrs: vec![],
            checksum: Checksum::from_data(ChecksumAlgorithm::Sha256, &invalid_wasm),
            signature: None,
        };

        let invalid_package = ExtensionPackage::new(
            invalid_manifest,
            invalid_wasm.to_vec(),
            "test-store".to_string(),
        );

        let result = store
            .publish_extension(invalid_package, &PublishOptions::default())
            .await;
        assert!(result.is_err(), "Invalid extension should fail to publish");

        // Verify the error is a validation error
        match result.unwrap_err() {
            crate::error::StoreError::PublishError(
                crate::publish::PublishError::ValidationFailed(_),
            ) => {
                // Expected error type
            }
            other => panic!("Expected ValidationFailed error, got: {:?}", other),
        }

        // Test 3: Extension with forbidden files should fail validation
        let forbidden_manifest = ExtensionManifest {
            id: "forbidden-extension".to_string(),
            name: "forbidden-extension".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![crate::manifest::ReadingDirection::Ltr],
            attrs: vec![],
            checksum: Checksum::from_data(ChecksumAlgorithm::Sha256, &valid_wasm),
            signature: None,
        };

        let mut forbidden_package = ExtensionPackage::new(
            forbidden_manifest,
            valid_wasm.to_vec(),
            "test-store".to_string(),
        );

        // Add forbidden file
        forbidden_package
            .assets
            .insert("malware.exe".to_string(), vec![0x4d, 0x5a]); // PE header

        let result = store
            .publish_extension(forbidden_package, &PublishOptions::default())
            .await;
        assert!(
            result.is_err(),
            "Extension with forbidden files should fail to publish"
        );

        // Test 4: Skip validation should allow invalid content
        let invalid_manifest_skip = ExtensionManifest {
            id: "skip-validation".to_string(),
            name: "skip-validation".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![crate::manifest::ReadingDirection::Ltr],
            attrs: vec![],
            checksum: Checksum::from_data(ChecksumAlgorithm::Sha256, &invalid_wasm),
            signature: None,
        };

        let invalid_package_skip = ExtensionPackage::new(
            invalid_manifest_skip,
            invalid_wasm.to_vec(),
            "test-store".to_string(),
        );

        let skip_options = PublishOptions {
            skip_validation: true,
            ..Default::default()
        };

        let result = store
            .publish_extension(invalid_package_skip, &skip_options)
            .await;
        assert!(
            result.is_ok(),
            "Extension should publish when validation is skipped"
        );
    }
}
