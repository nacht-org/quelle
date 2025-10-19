//! Local filesystem store implementation using FileBasedProcessor

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{info, warn};

use super::file_operations::LocalFileOperations;
use crate::error::{Result, StoreError};
use crate::manager::publish::{
    PublishOptions, PublishRequirements, PublishResult, UnpublishOptions, UnpublishResult,
    ValidationReport,
};
use crate::manager::store_manifest::{ExtensionSummary, StoreManifest, UrlPattern};
use crate::models::{
    ExtensionInfo, ExtensionListing, ExtensionMetadata, ExtensionPackage, InstalledExtension,
    SearchQuery, StoreHealth, UpdateInfo,
};
use crate::registry::manifest::{ExtensionManifest, LocalExtensionManifest};
use crate::stores::file_operations::FileBasedProcessor;
use crate::stores::traits::{BaseStore, CacheableStore, ReadableStore, WritableStore};

/// Local store manifest that extends the base StoreManifest with URL routing
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct LocalStoreManifest {
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

    /// Add extension info to the manifest
    pub(crate) fn add_extension(&mut self, manifest: &ExtensionManifest, manifest_path: String) {
        // Update URL patterns
        for base_url in &manifest.base_urls {
            self.add_url_pattern(base_url.clone(), manifest.id.clone(), 100);
        }

        // Update supported domains
        for base_url in &manifest.base_urls {
            if let Ok(url) = url::Url::parse(base_url) {
                if let Some(domain) = url.domain() {
                    if !self.supported_domains.contains(&domain.to_string()) {
                        self.supported_domains.push(domain.to_string());
                    }
                }
            }
        }

        // Add extension summary
        let summary = ExtensionSummary {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            base_urls: manifest.base_urls.clone(),
            langs: manifest.langs.clone(),
            last_updated: chrono::Utc::now(),
            manifest_path,
            manifest_checksum: format!(
                "blake3:{}",
                blake3::hash(&serde_json::to_vec(manifest).unwrap_or_default()).to_hex()
            ),
        };

        // Remove existing entry if present
        self.extensions.retain(|e| e.id != manifest.id);
        self.extensions.push(summary);

        self.extension_count = self.extensions.len() as u32;
    }

    /// Find extensions that can handle the given URL
    pub(crate) fn find_extensions_for_url(&self, url: &str) -> Vec<(String, String)> {
        let mut matches = Vec::new();

        for pattern in &self.url_patterns {
            if url.starts_with(&pattern.url_prefix) {
                for extension_id in &pattern.extensions {
                    if let Some(ext) = self.extensions.iter().find(|e| &e.id == extension_id) {
                        matches.push((ext.id.clone(), ext.name.clone()));
                    }
                }
            }
        }

        matches
    }
}

/// Local filesystem store using FileBasedProcessor
pub struct LocalStore {
    processor: FileBasedProcessor<LocalFileOperations>,
    root_path: PathBuf,
    readonly: bool,
    name: String,
}

pub struct LocalStoreBuilder {
    root_path: PathBuf,
    name: Option<String>,
    readonly: bool,
    cache_enabled: bool,
}

impl LocalStoreBuilder {
    /// Create a new builder for the given root path
    pub fn new<P: AsRef<Path>>(root_path: P) -> Self {
        Self {
            root_path: root_path.as_ref().to_path_buf(),
            name: None,
            readonly: false,
            cache_enabled: true,
        }
    }

    /// Set the store name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Disable caching (for compatibility, but FileBasedProcessor doesn't use this currently)
    pub fn no_cache(mut self) -> Self {
        self.cache_enabled = false;
        self
    }

    /// Enable caching (default)
    pub fn cache(mut self) -> Self {
        self.cache_enabled = true;
        self
    }

    /// Make the store readonly
    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    /// Make the store writable (default)
    pub fn writable(mut self) -> Self {
        self.readonly = false;
        self
    }

    /// Build the store
    pub fn build(self) -> Result<LocalStore> {
        let name = self.name.unwrap_or_else(|| {
            self.root_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("local-store")
                .to_string()
        });

        let file_ops = LocalFileOperations::new(&self.root_path);
        let processor = FileBasedProcessor::new(file_ops, name.clone());

        Ok(LocalStore {
            processor,
            root_path: self.root_path,
            readonly: self.readonly,
            name,
        })
    }
}

impl LocalStore {
    /// Create a new builder
    pub fn builder<P: AsRef<Path>>(root_path: P) -> LocalStoreBuilder {
        LocalStoreBuilder::new(root_path)
    }

    /// Create a new local store directly
    pub fn new<P: AsRef<Path>>(root_path: P) -> Result<Self> {
        Self::builder(root_path).build()
    }

    /// Get the root path
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Check if store is readonly
    pub fn is_readonly(&self) -> bool {
        self.readonly
    }

    /// Initialize a new store with basic structure
    pub async fn initialize_store(&self, name: String, description: Option<String>) -> Result<()> {
        if self.readonly {
            return Err(StoreError::PermissionDenied(
                "Cannot initialize readonly store".to_string(),
            ));
        }

        // Create basic directory structure
        let extensions_dir = self.root_path.join("extensions");
        tokio::fs::create_dir_all(&extensions_dir).await?;

        // Create store manifest
        let mut manifest = StoreManifest::new(name, "local".to_string(), "1.0.0".to_string())
            .with_url(format!("file://{}", self.root_path.display()));

        if let Some(desc) = description {
            manifest = manifest.with_description(desc);
        }

        let manifest_path = self.root_path.join("store.json");
        let manifest_content = serde_json::to_string_pretty(&manifest)?;
        tokio::fs::write(&manifest_path, manifest_content).await?;

        info!("Initialized local store at: {}", self.root_path.display());
        Ok(())
    }

    /// Generate a local store manifest with URL routing information
    async fn generate_local_store_manifest(&self) -> Result<LocalStoreManifest> {
        // Create base manifest from scratch
        let base_manifest =
            StoreManifest::new(self.name.clone(), "local".to_string(), "1.0.0".to_string());
        let mut local_manifest = LocalStoreManifest::new(base_manifest);

        // Manually scan extensions directory to build the manifest
        let extensions_dir = self.root_path.join("extensions");
        if extensions_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&extensions_dir) {
                for entry in entries.flatten() {
                    if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                        let extension_id = entry.file_name().to_string_lossy().to_string();

                        // Find latest version
                        let extension_path = entry.path();
                        if let Ok(version_entries) = std::fs::read_dir(&extension_path) {
                            for version_entry in version_entries.flatten() {
                                if version_entry.file_type().map_or(false, |ft| ft.is_dir()) {
                                    let version =
                                        version_entry.file_name().to_string_lossy().to_string();

                                    // Try to load the extension manifest
                                    if let Ok(manifest) = self
                                        .processor
                                        .get_extension_manifest(&extension_id, Some(&version))
                                        .await
                                    {
                                        let manifest_path = format!(
                                            "extensions/{}/{}/manifest.json",
                                            extension_id, version
                                        );
                                        local_manifest.add_extension(&manifest, manifest_path);
                                        break; // Only add the first version found
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(local_manifest)
    }

    /// Save the local store manifest with URL routing information
    pub async fn save_store_manifest(&self) -> Result<()> {
        if self.readonly {
            return Err(StoreError::PermissionDenied(
                "Cannot save manifest in readonly store".to_string(),
            ));
        }

        let local_manifest = self.generate_local_store_manifest().await?;
        let manifest_path = self.root_path.join("store.json");
        let manifest_content = serde_json::to_string_pretty(&local_manifest)?;
        tokio::fs::write(&manifest_path, manifest_content).await?;

        info!(
            "Saved local store manifest with {} extensions",
            local_manifest.extension_count
        );
        Ok(())
    }
}

#[async_trait]
impl BaseStore for LocalStore {
    async fn get_store_manifest(&self) -> Result<StoreManifest> {
        self.processor.get_store_manifest().await
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        let start_time = SystemTime::now();

        // Check if root directory exists and is accessible
        if !self.root_path.exists() {
            return Ok(StoreHealth {
                healthy: false,
                last_check: chrono::Utc::now(),
                response_time: Some(start_time.elapsed().unwrap_or_default()),
                error: Some(format!(
                    "Store directory does not exist: {}",
                    self.root_path.display()
                )),
                extension_count: Some(0),
                store_version: None,
            });
        }

        // Try to read the store manifest
        let manifest_result = self.get_store_manifest().await;
        let is_healthy = manifest_result.is_ok();
        let error_message = if let Err(ref e) = manifest_result {
            Some(e.to_string())
        } else {
            None
        };

        // Count extensions if healthy
        let extension_count = if is_healthy {
            match self.list_extensions().await {
                Ok(extensions) => Some(extensions.len()),
                Err(_) => Some(0),
            }
        } else {
            Some(0)
        };

        Ok(StoreHealth {
            healthy: is_healthy,
            last_check: chrono::Utc::now(),
            response_time: Some(start_time.elapsed().unwrap_or_default()),
            error: error_message,
            extension_count,
            store_version: None,
        })
    }
}

#[async_trait]
impl ReadableStore for LocalStore {
    async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        self.processor.find_extensions_for_url(url).await
    }

    async fn list_extensions(&self) -> Result<Vec<ExtensionListing>> {
        let summaries = self.processor.list_extensions().await?;
        let store_source = self.name.clone();
        Ok(summaries
            .iter()
            .map(|summary| ExtensionListing::from_summary(summary, store_source.clone()))
            .collect())
    }

    async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionListing>> {
        let summaries = self.processor.search_extensions(query).await?;
        let store_source = self.name.clone();
        Ok(summaries
            .iter()
            .map(|summary| ExtensionListing::from_summary(summary, store_source.clone()))
            .collect())
    }

    async fn get_extension_info(&self, name: &str) -> Result<Vec<ExtensionInfo>> {
        self.processor.get_extension_info(name).await
    }

    async fn get_extension_version_info(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo> {
        self.processor
            .get_extension_version_info(name, version)
            .await
    }

    async fn get_extension_manifest(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest> {
        self.processor.get_extension_manifest(name, version).await
    }

    async fn get_extension_metadata(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        self.processor.get_extension_metadata(name, version).await
    }

    async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionPackage> {
        self.processor
            .get_extension_package(id, version, self.name.clone())
            .await
    }

    async fn get_extension_latest_version(&self, id: &str) -> Result<Option<String>> {
        self.processor.get_extension_latest_version(id).await
    }

    async fn list_extension_versions(&self, id: &str) -> Result<Vec<String>> {
        self.processor.list_extension_versions(id).await
    }

    async fn check_extension_version_exists(&self, id: &str, version: &str) -> Result<bool> {
        self.processor
            .check_extension_version_exists(id, version)
            .await
    }

    async fn check_extension_updates(
        &self,
        installed: &[InstalledExtension],
    ) -> Result<Vec<UpdateInfo>> {
        let mut updates = Vec::new();

        for installed_ext in installed {
            if let Ok(Some(latest_version)) =
                self.get_extension_latest_version(&installed_ext.id).await
            {
                if latest_version != installed_ext.version {
                    // Simple version comparison - in practice you'd want semver
                    if latest_version > installed_ext.version {
                        updates.push(UpdateInfo {
                            extension_name: installed_ext.id.clone(),
                            current_version: installed_ext.version.clone(),
                            latest_version,
                            update_available: true,
                            changelog_url: None,
                            breaking_changes: false, // Would need to analyze changes
                            security_update: false,
                            update_size: None,
                            store_source: self.name.clone(),
                        });
                    }
                }
            }
        }

        Ok(updates)
    }
}

#[async_trait]
impl WritableStore for LocalStore {
    fn publish_requirements(&self) -> PublishRequirements {
        PublishRequirements {
            requires_authentication: false,
            requires_signing: false,
            max_package_size: Some(100 * 1024 * 1024), // 100MB for local stores
            allowed_file_extensions: vec![
                "wasm".to_string(),
                "json".to_string(),
                "md".to_string(),
                "txt".to_string(),
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "svg".to_string(),
                "css".to_string(),
                "js".to_string(),
                "html".to_string(),
            ],
            forbidden_patterns: vec![
                "*.exe".to_string(),
                "*.dll".to_string(),
                "*.so".to_string(),
                "*.dylib".to_string(),
                "../*".to_string(),
            ],
            required_metadata: vec!["name".to_string(), "version".to_string()],
            supported_visibility: vec![crate::manager::publish::ExtensionVisibility::Public],
            enforces_versioning: true,
            validation_rules: Vec::new(),
        }
    }

    async fn publish(
        &self,
        package: ExtensionPackage,
        _options: PublishOptions,
    ) -> Result<PublishResult> {
        if self.readonly {
            return Err(StoreError::PermissionDenied(
                "Cannot publish to readonly store".to_string(),
            ));
        }

        let extension_id = &package.manifest.id;
        let version = &package.manifest.version;

        // Create extension directory structure
        let extension_dir = self.root_path.join("extensions").join(extension_id);
        let version_dir = extension_dir.join(version);

        // Check if version already exists
        if version_dir.exists() {
            return Err(StoreError::ValidationError(format!(
                "Extension {} version {} already exists",
                extension_id, version
            )));
        }

        tokio::fs::create_dir_all(&version_dir).await?;

        // Convert to LocalExtensionManifest and write manifest
        let local_manifest = LocalExtensionManifest {
            manifest: package.manifest.clone(),
            path: version_dir.clone(),
            metadata: package.metadata.clone(),
        };

        let manifest_path = version_dir.join("manifest.json");
        let manifest_content = serde_json::to_string_pretty(&local_manifest)?;

        tokio::fs::write(&manifest_path, manifest_content).await?;

        // Write WASM component
        let wasm_path = version_dir.join(&package.manifest.wasm_file.path);
        tokio::fs::write(&wasm_path, &package.wasm_component).await?;

        // Write assets
        for (asset_name, asset_content) in &package.assets {
            let asset_path = version_dir.join(asset_name);
            if let Some(parent) = asset_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&asset_path, asset_content).await?;
        }

        // Update store manifest with new extension
        if let Err(e) = self.save_store_manifest().await {
            warn!("Failed to update store manifest after publish: {}", e);
        }

        info!(
            "Published extension {}@{} to local store",
            extension_id, version
        );

        Ok(PublishResult {
            extension_id: extension_id.clone(),
            version: version.clone(),
            download_url: format!(
                "file://{}/extensions/{}/{}",
                self.root_path.display(),
                extension_id,
                version
            ),
            published_at: chrono::Utc::now(),
            publication_id: format!("{}@{}", extension_id, version),
            package_size: package.calculate_total_size(),
            content_hash: format!("blake3:{}", blake3::hash(&package.wasm_component).to_hex()),
            warnings: Vec::new(),
        })
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

        let extension_dir = self.root_path.join("extensions").join(extension_id);

        if !extension_dir.exists() {
            return Err(StoreError::ExtensionNotFound(extension_id.to_string()));
        }

        let removed_version = if let Some(version) = &options.version {
            // Remove specific version
            let version_dir = extension_dir.join(version);
            if !version_dir.exists() {
                return Err(StoreError::VersionNotFound {
                    extension: extension_id.to_string(),
                    version: version.clone(),
                });
            }
            tokio::fs::remove_dir_all(&version_dir).await?;
            version.clone()
        } else {
            // Remove all versions
            tokio::fs::remove_dir_all(&extension_dir).await?;
            "all".to_string()
        };

        // Update store manifest
        if let Err(e) = self.save_store_manifest().await {
            warn!("Failed to update store manifest after unpublish: {}", e);
        }

        info!(
            "Unpublished extension {} version {} from local store",
            extension_id, removed_version
        );

        Ok(UnpublishResult {
            extension_id: extension_id.to_string(),
            version: removed_version,
            unpublished_at: chrono::Utc::now(),
            tombstone_created: true,
            users_notified: Some(0),
        })
    }

    async fn validate_package(
        &self,
        package: &ExtensionPackage,
        _options: &PublishOptions,
    ) -> Result<ValidationReport> {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Basic validation
        if package.manifest.id.is_empty() {
            issues.push("Extension ID cannot be empty".to_string());
        }

        if package.manifest.name.is_empty() {
            issues.push("Extension name cannot be empty".to_string());
        }

        if package.manifest.version.is_empty() {
            issues.push("Extension version cannot be empty".to_string());
        }

        // WASM validation
        if package.wasm_component.is_empty() {
            issues.push("WASM component cannot be empty".to_string());
        }

        // File size check
        let total_size = package.calculate_total_size();
        const MAX_SIZE: u64 = 100 * 1024 * 1024; // 100MB
        if total_size > MAX_SIZE {
            issues.push(format!(
                "Package size ({} bytes) exceeds maximum allowed size ({} bytes)",
                total_size, MAX_SIZE
            ));
        }

        if total_size > 50 * 1024 * 1024 {
            // 50MB warning
            warnings.push(format!(
                "Package size is large ({} MB). Consider optimizing assets.",
                total_size / (1024 * 1024)
            ));
        }

        // Path traversal check
        for asset_name in package.assets.keys() {
            if asset_name.contains("..") {
                issues.push(format!(
                    "Asset path contains directory traversal: {}",
                    asset_name
                ));
            }
        }

        use crate::registry::core::{IssueSeverity, ValidationIssue, ValidationIssueType};

        let validation_issues: Vec<ValidationIssue> = issues
            .into_iter()
            .map(|msg| ValidationIssue {
                extension_name: package.manifest.id.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: msg,
                severity: IssueSeverity::Critical,
            })
            .chain(warnings.into_iter().map(|msg| ValidationIssue {
                extension_name: package.manifest.id.clone(),
                issue_type: ValidationIssueType::InvalidManifest,
                description: msg,
                severity: IssueSeverity::Warning,
            }))
            .collect();

        Ok(ValidationReport {
            passed: validation_issues
                .iter()
                .all(|issue| !matches!(issue.severity, IssueSeverity::Critical)),
            issues: validation_issues,
            validation_duration: std::time::Duration::from_millis(10),
            validator_version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }
}

#[async_trait]
impl CacheableStore for LocalStore {
    async fn refresh_cache(&self) -> Result<()> {
        // For local stores, refreshing means regenerating the store manifest
        self.save_store_manifest().await?;
        Ok(())
    }

    async fn clear_cache(&self) -> Result<()> {
        // Local stores don't have a cache to clear
        // But we can regenerate the store manifest
        Ok(())
    }

    async fn cache_stats(&self) -> Result<crate::stores::traits::CacheStats> {
        // Local stores don't have cache statistics
        Ok(crate::stores::traits::CacheStats {
            entries: 0,
            size_bytes: 0,
            hit_rate: 0.0,
            last_refresh: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::builder(temp_dir.path())
            .name("test-store")
            .build()
            .unwrap();

        assert_eq!(store.root_path(), temp_dir.path());
        assert!(!store.is_readonly());
    }

    #[tokio::test]
    async fn test_readonly_store() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::builder(temp_dir.path())
            .name("readonly-store")
            .readonly()
            .build()
            .unwrap();

        assert!(store.is_readonly());
    }

    #[tokio::test]
    async fn test_store_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::builder(temp_dir.path())
            .name("init-test")
            .build()
            .unwrap();

        store
            .initialize_store("test-store".to_string(), Some("Test store".to_string()))
            .await
            .unwrap();

        // Check that store.json was created
        let manifest_path = temp_dir.path().join("store.json");
        assert!(manifest_path.exists());

        // Check that extensions directory was created
        let extensions_dir = temp_dir.path().join("extensions");
        assert!(extensions_dir.exists());

        // Verify we can read the manifest
        let manifest = store.get_store_manifest().await.unwrap();
        assert_eq!(manifest.name, "test-store");
        assert_eq!(manifest.store_type, "local");
    }

    #[tokio::test]
    async fn test_health_check() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::builder(temp_dir.path())
            .name("health-test")
            .build()
            .unwrap();

        // Before initialization - should be unhealthy
        let health = store.health_check().await.unwrap();
        assert!(!health.healthy);

        // After initialization - should be healthy
        store
            .initialize_store("health-test".to_string(), None)
            .await
            .unwrap();

        let health = store.health_check().await.unwrap();
        assert!(health.healthy);
    }

    #[tokio::test]
    async fn test_empty_extension_list() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::builder(temp_dir.path())
            .name("empty-test")
            .build()
            .unwrap();

        store
            .initialize_store("empty-test".to_string(), None)
            .await
            .unwrap();

        let extensions = store.list_extensions().await.unwrap();
        assert!(extensions.is_empty());
    }

    #[tokio::test]
    async fn test_readonly_prevents_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalStore::builder(temp_dir.path())
            .name("readonly-init")
            .readonly()
            .build()
            .unwrap();

        let result = store
            .initialize_store("readonly-init".to_string(), None)
            .await;

        assert!(result.is_err());
        if let Err(StoreError::PermissionDenied(_)) = result {
            // Expected
        } else {
            panic!("Expected PermissionDenied error");
        }
    }
}
