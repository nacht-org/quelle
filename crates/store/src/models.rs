use std::collections::HashMap;

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::manifest::ExtensionManifest;

/// Information about an available extension in a store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: String,
    pub tags: Vec<String>,
    pub last_updated: Option<DateTime<Utc>>,
    pub download_count: Option<u64>,
    pub size: Option<u64>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub store_source: String, // Which store this info came from
}

/// Complete extension package with all files and metadata
#[derive(Debug, Clone)]
pub struct ExtensionPackage {
    pub manifest: ExtensionManifest,
    pub wasm_component: Vec<u8>,
    pub metadata: Option<ExtensionMetadata>,
    pub assets: HashMap<String, Vec<u8>>, // Additional files (docs, examples, etc.)
    pub package_layout: PackageLayout,
    pub source_store: String,
}

impl ExtensionPackage {
    pub fn new(manifest: ExtensionManifest, wasm_component: Vec<u8>, source_store: String) -> Self {
        Self {
            manifest,
            wasm_component,
            metadata: None,
            assets: HashMap::new(),
            package_layout: PackageLayout::default(),
            source_store,
        }
    }

    pub fn with_metadata(mut self, metadata: ExtensionMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn with_layout(mut self, layout: PackageLayout) -> Self {
        self.package_layout = layout;
        self
    }

    pub fn add_asset(&mut self, name: String, content: Vec<u8>) {
        self.assets.insert(name, content);
    }

    pub fn calculate_total_size(&self) -> u64 {
        let mut size = self.wasm_component.len() as u64;
        for asset in self.assets.values() {
            size += asset.len() as u64;
        }
        size
    }

    /// Create an ExtensionPackage from a WASM file by extracting its metadata
    pub async fn from_wasm_file(
        wasm_path: impl AsRef<std::path::Path>,
        source_store: String,
    ) -> Result<Self> {
        use quelle_engine::ExtensionEngine;
        use std::sync::Arc;

        let wasm_path = wasm_path.as_ref();

        // Read the wasm file
        let wasm_content = tokio::fs::read(wasm_path).await.map_err(|e| {
            crate::error::StoreError::IoOperation {
                operation: "read wasm file".to_string(),
                path: wasm_path.to_path_buf(),
                source: e,
            }
        })?;

        // Create a headless executor for metadata extraction
        // Note: We use a minimal executor since we only need metadata
        let executor = Arc::new(quelle_engine::http::HeadlessChromeExecutor::new());
        let engine = ExtensionEngine::new(executor).map_err(|e| {
            crate::error::StoreError::InvalidPackage {
                reason: format!("Failed to create engine: {}", e),
            }
        })?;

        // Create a runner from the wasm content
        let runner = engine
            .new_runner_from_bytes(&wasm_content)
            .await
            .map_err(|e| crate::error::StoreError::InvalidPackage {
                reason: format!("Failed to create runner from wasm: {}", e),
            })?;

        // Extract metadata
        let (_runner, extension_meta) =
            runner
                .meta()
                .await
                .map_err(|e| crate::error::StoreError::InvalidPackage {
                    reason: format!("Failed to extract metadata from wasm: {}", e),
                })?;

        // Convert the engine metadata to our manifest format
        let manifest = ExtensionManifest {
            id: extension_meta.id.clone(),
            name: extension_meta.name.clone(),
            version: extension_meta.version.clone(),
            author: extension_meta.id.clone(), // Use ID as author for now
            langs: extension_meta.langs,
            base_urls: extension_meta.base_urls,
            rds: extension_meta
                .rds
                .into_iter()
                .map(|rd| match rd {
                    quelle_engine::bindings::quelle::extension::source::ReadingDirection::Ltr => {
                        crate::manifest::ReadingDirection::Ltr
                    }
                    quelle_engine::bindings::quelle::extension::source::ReadingDirection::Rtl => {
                        crate::manifest::ReadingDirection::Rtl
                    }
                })
                .collect(),
            attrs: extension_meta
                .attrs
                .into_iter()
                .map(|attr| match attr {
                    quelle_engine::bindings::quelle::extension::source::SourceAttr::Fanfiction => {
                        crate::manifest::Attribute::Fanfiction
                    }
                })
                .collect(),
            checksum: crate::manifest::Checksum::from_data(
                crate::manifest::ChecksumAlgorithm::Blake3,
                &wasm_content,
            ),
            signature: None,
        };

        // Create the package with only the WASM file - no automatic asset collection for security
        let package = ExtensionPackage::new(manifest, wasm_content, source_store);

        Ok(package)
    }

    /// Create an ExtensionPackage from a directory containing a manifest and wasm file
    pub async fn from_directory(
        dir_path: impl AsRef<std::path::Path>,
        source_store: String,
    ) -> Result<Self> {
        let dir_path = dir_path.as_ref();

        // Look for manifest.json
        let manifest_path = dir_path.join("manifest.json");
        if !manifest_path.exists() {
            return Err(crate::error::StoreError::InvalidPackage {
                reason: "No manifest.json found in directory".to_string(),
            });
        }

        // Read and parse manifest
        let manifest_content = tokio::fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| crate::error::StoreError::IoOperation {
                operation: "read manifest".to_string(),
                path: manifest_path.clone(),
                source: e,
            })?;

        let manifest: ExtensionManifest = serde_json::from_str(&manifest_content).map_err(|e| {
            crate::error::StoreError::InvalidManifestFile {
                path: manifest_path.clone(),
                source: e,
            }
        })?;

        // Look for wasm file - try common names
        let wasm_candidates = [
            format!("{}.wasm", manifest.name),
            "extension.wasm".to_string(),
            "main.wasm".to_string(),
        ];

        let mut wasm_content = None;
        for candidate in &wasm_candidates {
            let wasm_path = dir_path.join(candidate);
            if wasm_path.exists() {
                wasm_content = Some(tokio::fs::read(&wasm_path).await.map_err(|e| {
                    crate::error::StoreError::IoOperation {
                        operation: "read wasm file".to_string(),
                        path: wasm_path,
                        source: e,
                    }
                })?);
                break;
            }
        }

        let wasm_component =
            wasm_content.ok_or_else(|| crate::error::StoreError::InvalidPackage {
                reason: format!(
                    "No wasm file found. Looked for: {}",
                    wasm_candidates.join(", ")
                ),
            })?;

        // Create the package
        let package = ExtensionPackage::new(manifest, wasm_component, source_store);

        Ok(package)
    }
}

/// Rich metadata about an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    pub description: String,
    pub long_description: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub documentation: Option<String>,
    pub changelog: Option<String>,
    pub license: Option<String>,
    pub compatibility: CompatibilityInfo,
}

/// Compatibility requirements for an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityInfo {
    pub min_engine_version: Option<String>,
    pub max_engine_version: Option<String>,
    pub platforms: Option<Vec<String>>,
    pub required_features: Vec<String>,
}

/// Information about an installed extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledExtension {
    pub id: String,
    pub name: String,
    pub version: String,
    pub manifest: ExtensionManifest,
    pub wasm_component: Vec<u8>,
    pub metadata: Option<ExtensionMetadata>,
    pub assets: HashMap<String, Vec<u8>>, // Additional files (docs, examples, etc.)
    pub installed_at: DateTime<Utc>,
    pub last_updated: Option<DateTime<Utc>>,
    pub source_store: String, // Store where this was installed from
    pub auto_update: bool,
    pub checksum: Option<crate::manifest::Checksum>, // For integrity verification
}

impl InstalledExtension {
    pub fn new(
        id: String,
        name: String,
        version: String,
        manifest: ExtensionManifest,
        wasm_component: Vec<u8>,
        source_store: String,
    ) -> Self {
        Self {
            id,
            name,
            version,
            manifest,
            wasm_component,
            metadata: None,
            assets: HashMap::new(),
            installed_at: Utc::now(),
            last_updated: Some(Utc::now()),
            source_store,
            auto_update: false,
            checksum: None,
        }
    }

    /// Create from an ExtensionPackage
    pub fn from_package(package: ExtensionPackage) -> Self {
        Self {
            id: package.manifest.id.clone(),
            name: package.manifest.name.clone(),
            version: package.manifest.version.clone(),
            manifest: package.manifest,
            wasm_component: package.wasm_component,
            metadata: package.metadata,
            assets: package.assets,
            installed_at: Utc::now(),
            last_updated: Some(Utc::now()),
            source_store: package.source_store,
            auto_update: false,
            checksum: None,
        }
    }

    /// Get the WASM component bytes
    pub fn get_wasm_bytes(&self) -> &[u8] {
        &self.wasm_component
    }

    /// Get the manifest
    pub fn get_manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    /// Calculate the total size of the installation
    pub fn calculate_size(&self) -> u64 {
        let mut size = self.wasm_component.len() as u64;
        for asset in self.assets.values() {
            size += asset.len() as u64;
        }
        size
    }

    /// Add an asset to the installation
    pub fn add_asset(&mut self, name: String, content: Vec<u8>) {
        self.assets.insert(name, content);
    }

    /// Get an asset by name
    pub fn get_asset(&self, name: &str) -> Option<&[u8]> {
        self.assets.get(name).map(|v| v.as_slice())
    }

    /// Update the installation timestamp
    pub fn mark_updated(&mut self) {
        self.last_updated = Some(Utc::now());
    }

    /// Verify the integrity of the installation if checksum is available
    pub fn verify_integrity(&self) -> bool {
        if let Some(ref checksum) = self.checksum {
            checksum.verify(&self.wasm_component)
        } else {
            // No checksum available, assume valid
            true
        }
    }

    /// Convert to ExtensionPackage for operations that need the package format
    pub fn to_package(&self) -> ExtensionPackage {
        ExtensionPackage {
            manifest: self.manifest.clone(),
            wasm_component: self.wasm_component.clone(),
            metadata: self.metadata.clone(),
            assets: self.assets.clone(),
            package_layout: PackageLayout::default(),
            source_store: self.source_store.clone(),
        }
    }
}

/// Information about available updates
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub extension_name: String,
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub changelog_url: Option<String>,
    pub breaking_changes: bool,
    pub security_update: bool,
    pub update_size: Option<u64>,
    pub store_source: String,
}

impl UpdateInfo {
    pub fn new(
        extension_name: String,
        current_version: String,
        latest_version: String,
        store_source: String,
    ) -> Self {
        let update_available = current_version != latest_version;

        Self {
            extension_name,
            current_version,
            latest_version,
            update_available,
            changelog_url: None,
            breaking_changes: false,
            security_update: false,
            update_size: None,
            store_source,
        }
    }

    pub fn with_changelog(mut self, url: String) -> Self {
        self.changelog_url = Some(url);
        self
    }

    pub fn mark_breaking(mut self) -> Self {
        self.breaking_changes = true;
        self
    }

    pub fn mark_security_update(mut self) -> Self {
        self.security_update = true;
        self
    }
}

/// Search query parameters
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: Option<String>,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub author: Option<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub sort_by: SearchSortBy,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub include_prerelease: bool,
}

impl SearchQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_text(mut self, text: String) -> Self {
        self.text = Some(text);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_author(mut self, author: String) -> Self {
        self.author = Some(author);
        self
    }

    pub fn sort_by(mut self, sort: SearchSortBy) -> Self {
        self.sort_by = sort;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Search result sorting options
#[derive(Debug, Clone, Default)]
pub enum SearchSortBy {
    #[default]
    Relevance,
    Name,
    Version,
    LastUpdated,
    DownloadCount,
    Size,
    Author,
}

/// Package file layout configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageLayout {
    pub wasm_file: String,
    pub manifest_file: String,
    pub metadata_file: Option<String>,
    pub assets_dir: Option<String>,
}

impl Default for PackageLayout {
    fn default() -> Self {
        Self {
            wasm_file: "extension.wasm".to_string(),
            manifest_file: "manifest.json".to_string(),
            metadata_file: Some("metadata.json".to_string()),
            assets_dir: Some("assets".to_string()),
        }
    }
}

impl PackageLayout {
    pub fn new(wasm_file: String, manifest_file: String) -> Self {
        Self {
            wasm_file,
            manifest_file,
            metadata_file: None,
            assets_dir: None,
        }
    }

    pub fn with_metadata_file(mut self, metadata_file: String) -> Self {
        self.metadata_file = Some(metadata_file);
        self
    }

    pub fn with_assets_dir(mut self, assets_dir: String) -> Self {
        self.assets_dir = Some(assets_dir);
        self
    }
}

/// Store information and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreInfo {
    pub name: String,
    pub store_type: String,
    pub url: Option<String>,
    pub description: Option<String>,
    pub priority: u32,
    pub trusted: bool,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub config: HashMap<String, serde_json::Value>,
}

impl StoreInfo {
    pub fn new(name: String, store_type: String) -> Self {
        Self {
            name,
            store_type,
            url: None,
            description: None,
            priority: 100,
            trusted: false,
            enabled: true,
            created_at: Utc::now(),
            config: HashMap::new(),
        }
    }

    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn trusted(mut self) -> Self {
        self.trusted = true;
        self
    }
}

/// Store health status
#[derive(Debug, Clone)]
pub struct StoreHealth {
    pub healthy: bool,
    pub last_check: DateTime<Utc>,
    pub response_time: Option<Duration>,
    pub error: Option<String>,
    pub extension_count: Option<usize>,
    pub store_version: Option<String>,
    pub capabilities: Vec<String>,
}

impl StoreHealth {
    pub fn healthy() -> Self {
        Self {
            healthy: true,
            last_check: Utc::now(),
            response_time: None,
            error: None,
            extension_count: None,
            store_version: None,
            capabilities: Vec::new(),
        }
    }

    pub fn unhealthy(error: String) -> Self {
        Self {
            healthy: false,
            last_check: Utc::now(),
            response_time: None,
            error: Some(error),
            extension_count: None,
            store_version: None,
            capabilities: Vec::new(),
        }
    }

    pub fn with_response_time(mut self, duration: Duration) -> Self {
        self.response_time = Some(duration);
        self
    }

    pub fn with_extension_count(mut self, count: usize) -> Self {
        self.extension_count = Some(count);
        self
    }
}

/// Store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    pub auto_update_check: bool,
    pub parallel_downloads: usize,
    pub cache_ttl: Duration,
    pub verify_checksums: bool,
    pub allow_prereleases: bool,
    pub max_download_size: Option<u64>,
    pub timeout: Duration,
    pub retry_attempts: u32,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            auto_update_check: true,
            parallel_downloads: 3,
            cache_ttl: Duration::from_secs(3600), // 1 hour
            verify_checksums: true,
            allow_prereleases: false,
            max_download_size: Some(100 * 1024 * 1024), // 100MB
            timeout: Duration::from_secs(30),
            retry_attempts: 3,
        }
    }
}

/// Installation options
#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub auto_update: bool,
    pub force_reinstall: bool,
    pub skip_verification: bool,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            auto_update: false,
            force_reinstall: false,
            skip_verification: false,
        }
    }
}

/// Update options
#[derive(Debug, Clone)]
pub struct UpdateOptions {
    pub include_prereleases: bool,
    pub update_dependencies: bool,
    pub force_update: bool,
    pub backup_current: bool,
}

impl Default for UpdateOptions {
    fn default() -> Self {
        Self {
            include_prereleases: false,
            update_dependencies: true,
            force_update: false,
            backup_current: true,
        }
    }
}
