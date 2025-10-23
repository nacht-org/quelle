use std::collections::HashMap;

use std::time::Duration;

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::error::StoreError::InvalidPackage;
use crate::registry::manifest::ExtensionManifest;

/// Information about an available extension in a store
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: Version,
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

/// Minimal extension information for listing and search operations
/// This is a clean interface type without implementation-specific details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionListing {
    pub id: String,
    pub name: String,
    pub version: Version,
    pub description: Option<String>,
    pub author: String,
    pub tags: Vec<String>,
    pub last_updated: Option<DateTime<Utc>>,
    pub store_source: String,
}

impl ExtensionListing {
    /// Convert from ExtensionSummary to ExtensionListing
    /// This eliminates implementation-specific details like manifest_path and manifest_checksum
    pub fn from_summary(
        summary: &crate::manager::store_manifest::ExtensionVersion,
        store_source: String,
    ) -> Self {
        Self {
            id: summary.id.clone(),
            name: summary.name.clone(),
            version: summary.version.clone(),
            description: None, // ExtensionSummary doesn't have description
            author: "Unknown".to_string(), // ExtensionSummary doesn't have author
            tags: summary.langs.clone(), // Use langs as tags for now
            last_updated: Some(summary.last_updated),
            store_source,
        }
    }

    /// Convert to full ExtensionInfo with additional optional fields
    pub fn to_extension_info(self) -> ExtensionInfo {
        ExtensionInfo {
            id: self.id,
            name: self.name,
            version: self.version,
            description: self.description,
            author: self.author,
            tags: self.tags,
            last_updated: self.last_updated,
            download_count: None,
            size: None,
            homepage: None,
            repository: None,
            license: None,
            store_source: self.store_source,
        }
    }
}

impl ExtensionInfo {
    /// Convert from ExtensionSummary to ExtensionInfo
    /// This eliminates implementation-specific details like manifest_path and manifest_checksum
    pub fn from_summary(
        summary: &crate::manager::store_manifest::ExtensionVersion,
        store_source: String,
    ) -> Self {
        Self {
            id: summary.id.clone(),
            name: summary.name.clone(),
            version: summary.version.clone(),
            description: None, // ExtensionSummary doesn't have description
            author: "Unknown".to_string(), // ExtensionSummary doesn't have author
            tags: summary.langs.clone(), // Use langs as tags for now
            last_updated: Some(summary.last_updated),
            download_count: None,
            size: None,
            homepage: None,
            repository: None,
            license: None,
            store_source,
        }
    }
}

/// Complete extension package with all files and metadata
#[derive(Debug, Clone)]
pub struct ExtensionPackage {
    pub manifest: ExtensionManifest,
    pub wasm_component: Vec<u8>,
    pub metadata: Option<ExtensionMetadata>,
    pub assets: HashMap<String, Vec<u8>>, // Additional files (docs, examples, etc.)
    pub source_store: String,
}

impl ExtensionPackage {
    pub fn new(manifest: ExtensionManifest, wasm_component: Vec<u8>, source_store: String) -> Self {
        Self {
            manifest,
            wasm_component,
            metadata: None,
            assets: HashMap::new(),
            source_store,
        }
    }

    pub fn with_metadata(mut self, metadata: ExtensionMetadata) -> Self {
        self.metadata = Some(metadata);
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

        let executor = Arc::new(quelle_engine::http::HeadlessChromeExecutor::new());
        let engine = ExtensionEngine::new(executor).map_err(|e| InvalidPackage {
            reason: format!("Failed to create engine: {}", e),
        })?;

        let runner = engine
            .new_runner_from_bytes(&wasm_content)
            .await
            .map_err(|e| InvalidPackage {
                reason: format!("Failed to create runner from wasm: {}", e),
            })?;

        // Extract metadata
        let (_runner, extension_meta) = runner.meta().await.map_err(|e| InvalidPackage {
            reason: format!("Failed to extract metadata from wasm: {}", e),
        })?;

        // Parse the extension version as semver
        let version = Version::parse(&extension_meta.version).map_err(|e| InvalidPackage {
            reason: format!(
                "Invalid extension version '{}': {}",
                extension_meta.version, e
            ),
        })?;

        // Convert the engine metadata to our manifest format
        let manifest = ExtensionManifest {
            id: extension_meta.id.clone(),
            name: extension_meta.name.clone(),
            version,
            author: extension_meta.id.clone(), // Use ID as author for now
            langs: extension_meta.langs,
            base_urls: extension_meta.base_urls,
            rds: extension_meta
                .rds
                .into_iter()
                .map(|rd| match rd {
                    quelle_engine::bindings::quelle::extension::source::ReadingDirection::Ltr => {
                        crate::registry::manifest::ReadingDirection::Ltr
                    }
                    quelle_engine::bindings::quelle::extension::source::ReadingDirection::Rtl => {
                        crate::registry::manifest::ReadingDirection::Rtl
                    }
                })
                .collect(),
            attrs: extension_meta
                .attrs
                .into_iter()
                .map(|attr| match attr {
                    quelle_engine::bindings::quelle::extension::source::SourceAttr::Fanfiction => {
                        crate::registry::manifest::Attribute::Fanfiction
                    }
                })
                .collect(),

            signature: None,
            wasm_file: crate::registry::manifest::FileReference::new(
                "./extension.wasm".to_string(),
                &wasm_content,
            ),
            assets: vec![],
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
            return Err(InvalidPackage {
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

        let wasm_component = wasm_content.ok_or_else(|| InvalidPackage {
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
    pub version: Version,
    pub manifest: ExtensionManifest,
    pub metadata: Option<ExtensionMetadata>,
    pub size: u64, // Total size of the installation in bytes
    pub installed_at: DateTime<Utc>,
    pub last_updated: Option<DateTime<Utc>>,
    pub source_store: String, // Store where this was installed from
    pub auto_update: bool,
    pub checksum: Option<crate::registry::manifest::Checksum>, // For integrity verification
}

impl InstalledExtension {
    pub fn new(
        id: String,
        name: String,
        version: Version,
        manifest: ExtensionManifest,
        source_store: String,
    ) -> Self {
        Self {
            id,
            name,
            version,
            manifest,
            metadata: None,
            size: 0, // Will be calculated later
            installed_at: Utc::now(),
            last_updated: Some(Utc::now()),
            source_store,
            auto_update: false,
            checksum: None,
        }
    }

    /// Create from an ExtensionPackage
    pub fn from_package(package: ExtensionPackage) -> Self {
        let size = package.calculate_total_size();
        Self {
            id: package.manifest.id.clone(),
            name: package.manifest.name.clone(),
            version: package.manifest.version.clone(),
            manifest: package.manifest,
            metadata: package.metadata,
            size,
            installed_at: Utc::now(),
            last_updated: Some(Utc::now()),
            source_store: package.source_store,
            auto_update: false,
            checksum: None,
        }
    }

    /// Get the manifest
    pub fn get_manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    /// Calculate the total size of the installation
    pub fn calculate_size(&self) -> u64 {
        self.size
    }

    /// Calculate the total size by reading actual files
    pub async fn calculate_actual_size(
        &self,
        registry: &dyn crate::registry::RegistryStore,
    ) -> crate::error::Result<u64> {
        let mut total_size = 0u64;

        if let Ok(wasm_bytes) = registry.get_extension_wasm_bytes(&self.id).await {
            total_size += wasm_bytes.len() as u64;
        }

        // TODO: Add asset sizes when asset management is implemented

        Ok(total_size)
    }

    /// Add an asset to the installation (placeholder - assets stored on disk)
    pub fn add_asset(&mut self, _name: String, _content: Vec<u8>) {
        // Assets are now stored on disk, not in memory
    }

    /// Update the installation timestamp
    pub fn mark_updated(&mut self) {
        self.last_updated = Some(Utc::now());
    }

    /// Verify the integrity by checking checksum
    pub async fn verify_integrity(&self, registry: &dyn crate::registry::RegistryStore) -> bool {
        if let Some(ref checksum) = self.checksum {
            // Get WASM component bytes from disk
            if let Ok(wasm_bytes) = registry.get_extension_wasm_bytes(&self.id).await {
                return checksum.verify(&wasm_bytes);
            }
            false
        } else {
            // No checksum available, assume valid
            true
        }
    }

    /// Update the size field by calculating actual size
    pub async fn update_size(
        &mut self,
        registry: &dyn crate::registry::RegistryStore,
    ) -> crate::error::Result<()> {
        self.size = self.calculate_actual_size(registry).await?;
        self.last_updated = Some(Utc::now());
        Ok(())
    }

    /// Convert to ExtensionPackage for operations that need the package format
    pub fn to_package(&self) -> ExtensionPackage {
        ExtensionPackage {
            manifest: self.manifest.clone(),
            wasm_component: Vec::new(), // Would need to load from disk
            metadata: self.metadata.clone(),
            assets: HashMap::new(), // Would need to load from disk
            source_store: self.source_store.clone(),
        }
    }
}

/// Result of checking a single extension for updates
#[derive(Debug, Clone)]
pub enum UpdateInfo {
    UpdateAvailable(UpdateAvailableInfo),
    NoUpdateNeeded(UpdateNotNeededInfo),
    CheckFailed(UpdateCheckFailedInfo),
}

#[derive(Debug, Clone)]
pub struct UpdateAvailableInfo {
    pub extension_id: String,
    pub current_version: Version,
    pub latest_version: Version,
    pub update_size: Option<u64>,
    pub store_source: String,
}

#[derive(Debug, Clone)]
pub struct UpdateNotNeededInfo {
    pub extension_id: String,
    pub current_version: Version,
    pub store_source: String,
}

#[derive(Debug, Clone)]
pub struct UpdateCheckFailedInfo {
    pub extension_id: String,
    pub current_version: Version,
    pub store_source: String,
    pub error: String,
}

impl UpdateInfo {
    pub fn extension_id(&self) -> &str {
        match self {
            UpdateInfo::UpdateAvailable(UpdateAvailableInfo { extension_id, .. }) => extension_id,
            UpdateInfo::NoUpdateNeeded(UpdateNotNeededInfo { extension_id, .. }) => extension_id,
            UpdateInfo::CheckFailed(UpdateCheckFailedInfo { extension_id, .. }) => extension_id,
        }
    }

    pub fn current_version(&self) -> &Version {
        match self {
            UpdateInfo::UpdateAvailable(UpdateAvailableInfo {
                current_version, ..
            }) => current_version,
            UpdateInfo::NoUpdateNeeded(UpdateNotNeededInfo {
                current_version, ..
            }) => current_version,
            UpdateInfo::CheckFailed(UpdateCheckFailedInfo {
                current_version, ..
            }) => current_version,
        }
    }

    pub fn store_source(&self) -> &str {
        match self {
            UpdateInfo::UpdateAvailable(UpdateAvailableInfo { store_source, .. }) => store_source,
            UpdateInfo::NoUpdateNeeded(UpdateNotNeededInfo { store_source, .. }) => store_source,
            UpdateInfo::CheckFailed(UpdateCheckFailedInfo { store_source, .. }) => store_source,
        }
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
#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    pub auto_update: bool,
    pub force_reinstall: bool,
    pub skip_verification: bool,
}

/// Update options
#[derive(Debug, Clone)]
pub struct UpdateOptions {
    pub update_dependencies: bool,
    pub force_update: bool,
    pub backup_current: bool,
}

impl Default for UpdateOptions {
    fn default() -> Self {
        Self {
            update_dependencies: true,
            force_update: false,
            backup_current: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::LocalRegistryStore;
    use tempfile::TempDir;
    use tokio;

    #[tokio::test]
    async fn test_installed_extension_size_tracking() {
        // Create a test extension package
        let wasm_data = b"fake wasm content for testing";
        let manifest = crate::registry::manifest::ExtensionManifest {
            id: "test-ext".to_string(),
            name: "Test Extension".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![crate::registry::manifest::ReadingDirection::Ltr],
            attrs: vec![],

            signature: None,
            wasm_file: crate::registry::manifest::FileReference::new(
                "./extension.wasm".to_string(),
                wasm_data,
            ),
            assets: vec![],
        };

        let package = ExtensionPackage::new(manifest, wasm_data.to_vec(), "test-store".to_string());

        // Create InstalledExtension from package
        let installed = InstalledExtension::from_package(package);

        // Verify that size is captured from package
        assert!(installed.size > 0);
        assert_eq!(installed.calculate_size(), installed.size);
    }

    #[tokio::test]
    async fn test_installed_extension_integrity_verification() {
        // Create temporary registry
        let temp_dir = TempDir::new().unwrap();
        let registry_dir = temp_dir.path().join("registry");

        let registry = LocalRegistryStore::new(registry_dir).await.unwrap();

        // Create a test extension with checksum
        let wasm_data = b"test wasm content";
        let manifest = crate::registry::manifest::ExtensionManifest {
            id: "integrity-test".to_string(),
            name: "Integrity Test".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            author: "Test".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://test.com".to_string()],
            rds: vec![crate::registry::manifest::ReadingDirection::Ltr],
            attrs: vec![],

            signature: None,
            wasm_file: crate::registry::manifest::FileReference::new(
                "./extension.wasm".to_string(),
                wasm_data,
            ),
            assets: vec![],
        };

        let package = ExtensionPackage::new(manifest, wasm_data.to_vec(), "test".to_string());
        let mut installed = InstalledExtension::from_package(package.clone());
        installed.checksum = Some(crate::registry::manifest::Checksum {
            algorithm: crate::registry::manifest::checksum::ChecksumAlgorithm::Sha256,
            value: crate::registry::manifest::checksum::ChecksumAlgorithm::Sha256
                .calculate(wasm_data),
        });

        // Test integrity verification without files (should return false)
        let integrity_result = installed.verify_integrity(&registry).await;
        assert!(!integrity_result); // Should fail since WASM file doesn't exist on disk

        // Test integrity verification is properly handled
        // (The basic verify_integrity method was removed)
    }

    #[tokio::test]
    async fn test_size_calculation_from_disk() {
        // Create temporary registry
        let temp_dir = TempDir::new().unwrap();
        let registry_dir = temp_dir.path().join("registry");

        let registry = LocalRegistryStore::new(registry_dir).await.unwrap();

        // Create a test extension
        let installed = InstalledExtension::new(
            "size-test".to_string(),
            "Size Test".to_string(),
            Version::parse("1.0.0").unwrap(),
            crate::registry::manifest::ExtensionManifest {
                id: "size-test".to_string(),
                name: "Size Test".to_string(),
                version: Version::parse("1.0.0").unwrap(),
                author: "Test".to_string(),
                langs: vec!["en".to_string()],
                base_urls: vec!["https://test.com".to_string()],
                rds: vec![crate::registry::manifest::ReadingDirection::Ltr],
                attrs: vec![],

                signature: None,
                wasm_file: crate::registry::manifest::FileReference::new(
                    "./extension.wasm".to_string(),
                    b"fake wasm content",
                ),
                assets: vec![],
            },
            "test".to_string(),
        );

        // Test size calculation (should return 0 since no files exist)
        let disk_size = installed.calculate_actual_size(&registry).await.unwrap();
        assert_eq!(disk_size, 0);
    }
}
