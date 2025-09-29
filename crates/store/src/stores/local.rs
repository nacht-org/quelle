use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
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
    ExtensionVisibility, PublishOptions, PublishRequirements, PublishResult, PublishUpdateOptions, UnpublishOptions,
    UnpublishResult,
};
use crate::store_manifest::{ExtensionSummary, StoreManifest, UrlPattern};
use crate::stores::traits::{BaseStore, CacheStats, CacheableStore, ReadableStore, WritableStore};
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
/// Local filesystem-based extension store
pub struct LocalStore {
    root_path: PathBuf,
    layout: PackageLayout,
    cache: RwLock<HashMap<String, Vec<ExtensionInfo>>>,
    cache_timestamp: RwLock<Option<Instant>>,
    validator: ValidationEngine,
    name: String,
    cache_enabled: bool,
    readonly: bool,
}

impl LocalStore {
    /// Create a new local store
    pub fn new<P: AsRef<Path>>(root_path: P) -> Result<Self> {
        let root_path = root_path.as_ref().to_path_buf();
        let name = root_path
            .file_name()
            .unwrap_or_else(|| root_path.as_os_str())
            .to_string_lossy()
            .to_string();

        Ok(Self {
            root_path,
            layout: PackageLayout::default(),
            cache: RwLock::new(HashMap::new()),
            cache_timestamp: RwLock::new(None),
            validator: create_default_validator(),
            name,
            cache_enabled: true,
            readonly: false,
        })
    }

    /// Create store with custom name
    pub fn with_name<P: AsRef<Path>>(root_path: P, name: String) -> Result<Self> {
        let root_path = root_path.as_ref().to_path_buf();

        Ok(Self {
            root_path,
            layout: PackageLayout::default(),
            cache: RwLock::new(HashMap::new()),
            cache_timestamp: RwLock::new(None),
            validator: create_default_validator(),
            name,
            cache_enabled: true,
            readonly: false,
        })
    }

    /// Disable caching for this store
    pub fn with_cache_disabled(mut self) -> Self {
        self.cache_enabled = false;
        self
    }

    /// Set readonly mode
    pub fn with_readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    /// Create a LocalStore with custom package layout
    pub fn with_layout(mut self, layout: PackageLayout) -> Self {
        self.layout = layout;
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
                .map_err(StoreError::IoError)?;
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
                        .unwrap_or_else(Utc::now)
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
                        .unwrap_or_else(Utc::now)
                        .cmp(&a.last_updated.unwrap_or_else(Utc::now))
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
impl BaseStore for LocalStore {
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

    async fn health_check(&self) -> Result<StoreHealth> {
        use std::time::Instant;

        let start = Instant::now();

        // Check if root directory exists and is accessible
        if !self.root_path.exists() {
            return Ok(StoreHealth {
                healthy: false,
                last_check: chrono::Utc::now(),
                response_time: Some(start.elapsed()),
                error: Some(format!(
                    "Store directory does not exist: {}",
                    self.root_path.display()
                )),
                extension_count: Some(0),
                store_version: None,
            });
        }

        if !self.root_path.is_dir() {
            return Ok(StoreHealth {
                healthy: false,
                last_check: chrono::Utc::now(),
                response_time: Some(start.elapsed()),
                error: Some("Store path is not a directory".to_string()),
                extension_count: Some(0),
                store_version: None,
            });
        }

        // Try to count extensions
        let extensions_root = self.extensions_root();
        let mut extension_count = 0;

        if extensions_root.exists() {
            match tokio::fs::read_dir(&extensions_root).await {
                Ok(mut entries) => {
                    while let Some(entry) = entries.next_entry().await.map_err(StoreError::from)? {
                        if entry.file_type().await.map_err(StoreError::from)?.is_dir() {
                            extension_count += 1;
                        }
                    }
                }
                Err(_) => {
                    return Ok(StoreHealth {
                        healthy: false,
                        last_check: chrono::Utc::now(),
                        response_time: Some(start.elapsed()),
                        error: Some("Cannot read extensions directory".to_string()),
                        extension_count: Some(0),
                        store_version: None,
                    });
                }
            }
        }

        Ok(StoreHealth {
            healthy: true,
            last_check: chrono::Utc::now(),
            response_time: Some(start.elapsed()),
            error: None,
            extension_count: Some(extension_count),
            store_version: Some("1.0.0".to_string()),
        })
    }
}

#[async_trait]
impl ReadableStore for LocalStore {
    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        let local_manifest = self.get_local_store_manifest().await?;
        Ok(local_manifest.find_extensions_for_url(url))
    }

    async fn find_extensions_for_domain(&self, domain: &str) -> Result<Vec<String>> {
        let local_manifest = self.get_local_store_manifest().await?;
        Ok(local_manifest.find_extensions_for_domain(domain))
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

    async fn get_extension_manifest(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest> {
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

    async fn get_extension_metadata(
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
        let manifest = self.get_extension_manifest(id, version).await?;
        let wasm_component = self.get_extension_wasm_internal(id, version).await?;
        let metadata = self.get_extension_metadata(id, version).await?;

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

    async fn get_extension_latest_version(&self, id: &str) -> Result<Option<String>> {
        self.get_latest_version_internal(id)
            .await
            .map_err(StoreError::from)
    }

    async fn list_extension_versions(&self, id: &str) -> Result<Vec<String>> {
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

    async fn check_extension_version_exists(&self, id: &str, version: &str) -> Result<bool> {
        let version_path = self
            .extension_version_path(id, version)
            .map_err(StoreError::from)?;
        let manifest_path = version_path.join(&self.layout.manifest_file);
        let wasm_path = version_path.join(&self.layout.wasm_file);

        Ok(manifest_path.exists() && wasm_path.exists())
    }

    async fn check_extension_updates(
        &self,
        installed: &[InstalledExtension],
    ) -> Result<Vec<UpdateInfo>> {
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
                .get_extension_manifest(&ext_info.name, Some(&ext_info.version))
                .await
            {
                let summary = ExtensionSummary {
                    id: ext_manifest.id.clone(),
                    name: ext_info.name.clone(),
                    version: ext_info.version.clone(),
                    base_urls: ext_manifest.base_urls.clone(),
                    langs: ext_manifest.langs.clone(),
                    last_updated: ext_info.last_updated.unwrap_or_else(Utc::now),
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
            .await}
}

#[async_trait]
impl WritableStore for LocalStore {
    fn publish_requirements(&self) -> PublishRequirements {
        PublishRequirements {
            requires_authentication: false,
            requires_signing: false,
            max_package_size: Some(50 * 1024 * 1024), // 50MB
            allowed_file_extensions: vec![
                "wasm".to_string(),
                "json".to_string(),
                "md".to_string(),
                "txt".to_string(),
            ],
            forbidden_patterns: vec!["*.exe".to_string(), "*.dll".to_string()],
            required_metadata: vec!["name".to_string(), "version".to_string()],
            supported_visibility: vec![ExtensionVisibility::Public, ExtensionVisibility::Private],
            enforces_versioning: true,
            validation_rules: vec!["wasm-validation".to_string()],
            store_specific: std::collections::HashMap::new(),
        }
    }

    async fn publish(
        &self,
        package: ExtensionPackage,
        options: PublishOptions,
    ) -> Result<PublishResult> {
        if self.readonly {
            return Err(StoreError::PermissionDenied(
                "Cannot publish to readonly store".to_string(),
            ));
        }

        // Validate package first
        let validation = self.validate_package(&package, &options).await?;
        if !validation.passed {
            return Err(StoreError::ValidationFailed(format!(
                "Package validation failed: {}",
                validation.issues.len()
            )));
        }

        // Save the package to the store
        let extension_dir = self.extension_path(&package.manifest.id)?;
        let version_dir = extension_dir.join(&package.manifest.version);

        fs::create_dir_all(&version_dir).await?;

        // Write WASM component
        let wasm_path = version_dir.join("extension.wasm");
        fs::write(&wasm_path, &package.wasm_component).await?;

        // Write manifest
        let manifest_path = version_dir.join("manifest.json");
        let manifest_content = serde_json::to_string_pretty(&package.manifest)?;
        fs::write(&manifest_path, manifest_content).await?;

        // Write assets
        for (name, content) in &package.assets {
            let asset_path = version_dir.join(name);
            if let Some(parent) = asset_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(asset_path, content).await?;
        }

        // Clear cache to force refresh
        if self.cache_enabled {
            *self.cache.write().unwrap() = HashMap::new();
            *self.cache_timestamp.write().unwrap() = None;
        }

        Ok(PublishResult {
            extension_id: package.manifest.id.clone(),
            version: package.manifest.version.clone(),
            download_url: format!("file://{}", wasm_path.display()),
            published_at: chrono::Utc::now(),
            publication_id: format!("local-{}-{}", package.manifest.id, package.manifest.version),
            package_size: package.wasm_component.len() as u64,
            content_hash: {
                let mut hasher = DefaultHasher::new();
                package.wasm_component.hash(&mut hasher);
                format!("{:x}", hasher.finish())
            },
            warnings: Vec::new(),
            store_info: std::collections::HashMap::new(),
        })
    }

    async fn update_published(
        &self,
        _extension_id: &str,
        package: ExtensionPackage,
        _options: PublishUpdateOptions,
    ) -> Result<PublishResult> {
        // For local stores, updating is the same as publishing
        let publish_options = PublishOptions {
            overwrite_existing: true,
            pre_release: false,
            visibility: ExtensionVisibility::Public,
            access_token: None,
            signing_key: None,
            metadata: std::collections::HashMap::new(),
            skip_validation: false,
            timeout: None,
            create_backup: false,
            tags: Vec::new(),
            release_notes: None,
        };

        self.publish(package, publish_options).await
    }

    async fn unpublish(
        &self,
        extension_id: &str,
        options: UnpublishOptions,
    ) -> Result<UnpublishResult> {
        if self.readonly {
            return Err(StoreError::PermissionDenied(
                "Cannot unpublish from readonly store".to_string(),
            ));
        }

        let extension_dir = self.extension_path(extension_id)?;

        if let Some(version) = &options.version {
            // Remove specific version
            let version_dir = extension_dir.join(version);
            if version_dir.exists() {
                fs::remove_dir_all(&version_dir).await?;
                Ok(UnpublishResult {
                    extension_id: extension_id.to_string(),
                    version: version.clone(),
                    unpublished_at: chrono::Utc::now(),
                    tombstone_created: options.keep_record,
                    users_notified: if options.notify_users { Some(0) } else { None },
                })
            } else {
                Err(StoreError::ExtensionNotFound(format!(
                    "{}@{}",
                    extension_id, version
                )))
            }
        } else {
            // Remove entire extension
            if extension_dir.exists() {
                fs::remove_dir_all(&extension_dir).await?;
                Ok(UnpublishResult {
                    extension_id: extension_id.to_string(),
                    version: "all".to_string(),
                    unpublished_at: chrono::Utc::now(),
                    tombstone_created: options.keep_record,
                    users_notified: if options.notify_users { Some(0) } else { None },
                })
            } else {
                Err(StoreError::ExtensionNotFound(extension_id.to_string()))
            }
        }
    }

    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        _options: &PublishOptions,
    ) -> Result<crate::publish::ValidationReport> {
        let validation_result = self.validator.validate(package).await?;
        Ok(crate::publish::ValidationReport {
            passed: validation_result.passed,
            issues: validation_result.issues,
            validation_duration: validation_result.validation_duration,
            validator_version: env!("CARGO_PKG_VERSION").to_string(),
            metadata: std::collections::HashMap::new(),
        })
    }
}

#[async_trait]
impl CacheableStore for LocalStore {
    async fn refresh_cache(&self) -> Result<()> {
        if !self.cache_enabled {
            return Ok(());
        }

        let extensions = self.scan_extensions().await?;
        *self.cache.write().unwrap() = extensions;
        *self.cache_timestamp.write().unwrap() = Some(Instant::now());
        Ok(())
    }

    async fn clear_cache(&self) -> Result<()> {
        if !self.cache_enabled {
            return Ok(());
        }

        *self.cache.write().unwrap() = HashMap::new();
        *self.cache_timestamp.write().unwrap() = None;
        Ok(())
    }

    async fn cache_stats(&self) -> Result<CacheStats> {
        if !self.cache_enabled {
            return Ok(CacheStats {
                entries: 0,
                size_bytes: 0,
                hit_rate: 0.0,
                last_refresh: None,
            });
        }

        let cache = self.cache.read().unwrap();
        let timestamp = self.cache_timestamp.read().unwrap();

        let entries = cache.values().map(|v| v.len()).sum();

        // Rough estimate of cache size
        let size_bytes = cache
            .iter()
            .map(|(k, v)| {
                k.len()
                    + v.iter()
                        .map(|ext| ext.name.len() + ext.version.len())
                        .sum::<usize>()
            })
            .sum::<usize>() as u64;

        Ok(CacheStats {
            entries,
            size_bytes,
            hit_rate: 0.95, // Estimated hit rate for local stores
            last_refresh: timestamp.map(|_| chrono::Utc::now()),
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
        let manifest = store
            .get_extension_manifest("test_ext", Some("1.0.0"))
            .await
            .unwrap();

        assert_eq!(manifest.name, "test_ext");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[tokio::test]
    async fn test_version_management() {
        let temp_dir = TempDir::new().unwrap();
        create_test_extension(temp_dir.path(), "test_ext", "1.0.0").unwrap();
        create_test_extension(temp_dir.path(), "test_ext", "1.1.0").unwrap();

        let store = LocalStore::new(temp_dir.path()).unwrap();
        let versions = store.list_extension_versions("test_ext").await.unwrap();

        assert_eq!(versions.len(), 2);
        assert!(versions.contains(&"1.0.0".to_string()));
        assert!(versions.contains(&"1.1.0".to_string()));

        let exists = store
            .check_extension_version_exists("test_ext", "1.0.0")
            .await
            .unwrap();
        assert!(exists);

        let not_exists = store
            .check_extension_version_exists("test_ext", "2.0.0")
            .await
            .unwrap();
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
        let store = LocalStore::new(temp_dir.path()).unwrap();

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
        let result = store.publish(package.clone(), options).await.unwrap();

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
        let validation = store.validate_package(&package, &options).await.unwrap();

        assert!(!validation.passed);
        assert!(!validation.issues.is_empty());
        assert!(validation.has_critical_issues());
    }

    #[tokio::test]
    async fn test_unpublish_extension() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::new(temp_dir.path()).unwrap();

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
        store.publish(package, options).await.unwrap();

        // Verify it exists
        assert!(store
            .check_extension_version_exists("test-extension", "1.0.0")
            .await
            .unwrap());

        // Now unpublish it
        let unpublish_options = UnpublishOptions {
            access_token: None,
            version: Some("1.0.0".to_string()),
            reason: Some("Test unpublish".to_string()),
            keep_record: false,
            notify_users: false,
        };

        let result = store
            .unpublish("test-extension", unpublish_options)
            .await
            .unwrap();

        assert_eq!(result.version, "1.0.0");
        assert!(!result.tombstone_created);

        // Verify it no longer exists
        assert!(!store
            .check_extension_version_exists("test-extension", "1.0.0")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_validation_integration() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::new(temp_dir.path()).unwrap();

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
            .publish(valid_package, PublishOptions::default())
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
            .publish(invalid_package, PublishOptions::default())
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
            .publish(forbidden_package, PublishOptions::default())
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

        let result = store.publish(invalid_package_skip, skip_options).await;
        assert!(
            result.is_ok(),
            "Extension should publish when validation is skipped"
        );
    }
}
