use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::manifest::ExtensionManifest;

/// Information about an available extension in a store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
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
    pub dependencies: Vec<ExtensionDependency>,
    pub compatibility: CompatibilityInfo,
    pub extra: HashMap<String, serde_json::Value>, // For extensibility
}

/// Extension dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionDependency {
    pub name: String,
    pub version_requirement: String, // e.g., "^1.0.0", ">=1.2.0"
    pub optional: bool,
    pub features: Vec<String>, // Optional features to enable
}

impl ExtensionDependency {
    pub fn new(name: String, version_requirement: String) -> Self {
        Self {
            name,
            version_requirement,
            optional: false,
            features: Vec::new(),
        }
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }
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
    pub id: Uuid,
    pub name: String,
    pub version: String,
    pub install_path: PathBuf,
    pub manifest: ExtensionManifest,
    pub package_layout: PackageLayout,
    pub installed_at: DateTime<Utc>,
    pub installed_from: String,    // Store identifier
    pub dependencies: Vec<String>, // Names of installed dependencies
    pub auto_update: bool,
    pub install_size: u64,
}

impl InstalledExtension {
    pub fn new(
        name: String,
        version: String,
        install_path: PathBuf,
        manifest: ExtensionManifest,
        package_layout: PackageLayout,
        installed_from: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            version,
            install_path,
            manifest,
            package_layout,
            installed_at: Utc::now(),
            installed_from,
            dependencies: Vec::new(),
            auto_update: true,
            install_size: 0,
        }
    }

    pub fn get_wasm_path(&self) -> PathBuf {
        self.install_path.join(&self.package_layout.wasm_file)
    }

    pub fn get_manifest_path(&self) -> PathBuf {
        self.install_path.join(&self.package_layout.manifest_file)
    }

    pub fn get_metadata_path(&self) -> Option<PathBuf> {
        self.package_layout
            .metadata_file
            .as_ref()
            .map(|file| self.install_path.join(file))
    }

    pub fn load_wasm_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        std::fs::read(self.get_wasm_path())
    }

    pub fn verify_integrity(&self) -> Result<bool, std::io::Error> {
        let wasm_bytes = self.load_wasm_bytes()?;
        let calculated_hash = format!("{:x}", Sha256::digest(&wasm_bytes));

        // Compare with manifest checksum if available
        Ok(self.manifest.checksum.value == calculated_hash)
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
    pub install_dependencies: bool,
    pub allow_downgrades: bool,
    pub force_reinstall: bool,
    pub skip_verification: bool,
    pub target_directory: Option<PathBuf>,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            install_dependencies: true,
            allow_downgrades: false,
            force_reinstall: false,
            skip_verification: false,
            target_directory: None,
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
