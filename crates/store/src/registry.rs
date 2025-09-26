//! Registry store implementation for managing installed extensions
//!
//! This module provides the `RegistryStore` trait and implementations for managing
//! the authoritative state of installed extensions. The registry store acts as the
//! single source of truth for installation status, replacing the manager's local registry.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, warn};

use crate::error::{Result, StoreError};
use crate::models::{InstalledExtension, PackageLayout, StoreHealth, StoreInfo};
use crate::store::Store;

/// Query parameters for searching installed extensions
#[derive(Debug, Clone, Default)]
pub struct InstallationQuery {
    pub name_pattern: Option<String>,
    pub installed_after: Option<DateTime<Utc>>,
    pub installed_before: Option<DateTime<Utc>>,
    pub from_store: Option<String>,
    pub version_pattern: Option<String>,
    pub auto_update_only: bool,
}

impl InstallationQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name_pattern(mut self, pattern: String) -> Self {
        self.name_pattern = Some(pattern);
        self
    }

    pub fn installed_after(mut self, date: DateTime<Utc>) -> Self {
        self.installed_after = Some(date);
        self
    }

    pub fn from_store(mut self, store: String) -> Self {
        self.from_store = Some(store);
        self
    }
}

/// Statistics about installed extensions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationStats {
    pub total_extensions: usize,
    pub total_size: u64,
    pub stores_used: HashMap<String, usize>,
    pub auto_update_enabled: usize,
    pub last_updated: DateTime<Utc>,
}

/// Issues found during installation validation
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub extension_name: String,
    pub issue_type: ValidationIssueType,
    pub description: String,
    pub severity: IssueSeverity,
}

#[derive(Debug, Clone)]
pub enum ValidationIssueType {
    MissingFiles,
    CorruptedFiles,
    InvalidManifest,
    PathMismatch,
    ChecksumMismatch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Enhanced store trait that can maintain installation registry state
#[async_trait]
pub trait RegistryStore: Store {
    /// Register a new installation in the registry
    async fn register_installation(&mut self, installation: InstalledExtension) -> Result<()>;

    /// Remove an installation from the registry
    async fn unregister_installation(&mut self, name: &str) -> Result<bool>;

    /// Update an existing installation record
    async fn update_installation(&mut self, installation: InstalledExtension) -> Result<()>;

    /// List all installed extensions
    async fn list_installed(&self) -> Result<Vec<InstalledExtension>>;

    /// Get a specific installed extension
    async fn get_installed(&self, name: &str) -> Result<Option<InstalledExtension>>;

    /// Find installed extensions matching the query
    async fn find_installed(&self, query: &InstallationQuery) -> Result<Vec<InstalledExtension>>;

    /// Get statistics about installed extensions
    async fn get_installation_stats(&self) -> Result<InstallationStats>;

    /// Validate all registered installations
    async fn validate_installations(&self) -> Result<Vec<ValidationIssue>>;

    /// Remove orphaned registry entries (extensions no longer on disk)
    async fn cleanup_orphaned(&mut self) -> Result<u32>;

    /// Check if an extension is registered as installed
    async fn is_installed(&self, name: &str) -> Result<bool> {
        Ok(self.get_installed(name).await?.is_some())
    }

    /// Get the registry file path (if applicable)
    fn registry_path(&self) -> Option<PathBuf>;
}

/// JSON-based registry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRegistry {
    extensions: HashMap<String, InstalledExtension>,
    last_updated: DateTime<Utc>,
    version: String,
    stats: Option<InstallationStats>,
}

impl Default for JsonRegistry {
    fn default() -> Self {
        Self {
            extensions: HashMap::new(),
            last_updated: Utc::now(),
            version: "1.1.0".to_string(),
            stats: None,
        }
    }
}

/// Local JSON-based registry store implementation
pub struct LocalRegistryStore {
    registry_path: PathBuf,
    backup_path: PathBuf,
    registry: JsonRegistry,
    info: StoreInfo,
    layout: PackageLayout,
}

impl LocalRegistryStore {
    /// Create a new LocalRegistryStore
    pub async fn new<P: AsRef<Path>>(registry_dir: P) -> Result<Self> {
        let registry_dir = registry_dir.as_ref().to_path_buf();

        // Ensure registry directory exists
        fs::create_dir_all(&registry_dir).await?;

        let registry_path = registry_dir.join("registry.json");
        let backup_path = registry_dir.join("registry.json.backup");

        // Load existing registry or create new one
        let registry = if registry_path.exists() {
            Self::load_registry(&registry_path).await?
        } else {
            JsonRegistry::default()
        };

        let info = StoreInfo::new("local-registry".to_string(), "registry".to_string())
            .with_url(format!("file://{}", registry_path.display()))
            .trusted();

        Ok(Self {
            registry_path,
            backup_path,
            registry,
            info,
            layout: PackageLayout::default(),
        })
    }

    /// Load registry from file
    async fn load_registry(path: &Path) -> Result<JsonRegistry> {
        let content = fs::read_to_string(path).await?;
        let registry: JsonRegistry = serde_json::from_str(&content)?;
        debug!(
            "Loaded registry with {} extensions",
            registry.extensions.len()
        );
        Ok(registry)
    }

    /// Save registry to file with atomic write
    async fn save_registry(&self) -> Result<()> {
        // Create backup first
        if self.registry_path.exists() {
            if let Err(e) = fs::copy(&self.registry_path, &self.backup_path).await {
                warn!("Failed to create registry backup: {}", e);
            }
        }

        // Update timestamp and stats
        let mut registry = self.registry.clone();
        registry.last_updated = Utc::now();
        registry.stats = Some(self.calculate_stats(&registry.extensions));

        // Write to temporary file first
        let temp_path = self.registry_path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(&registry)?;
        fs::write(&temp_path, content).await?;

        // Atomic move
        fs::rename(&temp_path, &self.registry_path).await?;

        debug!(
            "Registry saved with {} extensions",
            registry.extensions.len()
        );
        Ok(())
    }

    /// Calculate installation statistics
    fn calculate_stats(
        &self,
        extensions: &HashMap<String, InstalledExtension>,
    ) -> InstallationStats {
        let mut stores_used: HashMap<String, usize> = HashMap::new();
        let mut total_size = 0u64;
        let mut auto_update_count = 0;

        for ext in extensions.values() {
            *stores_used.entry(ext.installed_from.clone()).or_insert(0) += 1;
            total_size += ext.install_size;
            if ext.auto_update {
                auto_update_count += 1;
            }
        }

        InstallationStats {
            total_extensions: extensions.len(),
            total_size,
            stores_used,
            auto_update_enabled: auto_update_count,
            last_updated: Utc::now(),
        }
    }

    /// Check if installation query matches an extension
    fn matches_query(&self, ext: &InstalledExtension, query: &InstallationQuery) -> bool {
        // Name pattern matching
        if let Some(pattern) = &query.name_pattern {
            if !ext.name.contains(pattern) {
                return false;
            }
        }

        // Date range filtering
        if let Some(after) = query.installed_after {
            if ext.installed_at < after {
                return false;
            }
        }

        if let Some(before) = query.installed_before {
            if ext.installed_at > before {
                return false;
            }
        }

        // Store filtering
        if let Some(store) = &query.from_store {
            if ext.installed_from != *store {
                return false;
            }
        }

        // Version pattern matching
        if let Some(version_pattern) = &query.version_pattern {
            if !ext.version.contains(version_pattern) {
                return false;
            }
        }

        // Auto-update filter
        if query.auto_update_only && !ext.auto_update {
            return false;
        }

        true
    }
}

#[async_trait]
impl Store for LocalRegistryStore {
    fn store_info(&self) -> &StoreInfo {
        &self.info
    }

    fn package_layout(&self) -> &PackageLayout {
        &self.layout
    }

    async fn health_check(&self) -> Result<StoreHealth> {
        let start = std::time::Instant::now();

        // Check if registry file is accessible
        match fs::metadata(&self.registry_path).await {
            Ok(_) => {
                let response_time = start.elapsed();
                let extension_count = self.registry.extensions.len();

                Ok(StoreHealth::healthy()
                    .with_response_time(response_time)
                    .with_extension_count(extension_count))
            }
            Err(e) => Ok(StoreHealth::unhealthy(format!(
                "Registry file not accessible: {}",
                e
            ))),
        }
    }

    async fn list_extensions(&self) -> Result<Vec<crate::models::ExtensionInfo>> {
        // Registry store doesn't provide extension discovery - it only tracks installations
        // This could be enhanced to return info about installed extensions
        Ok(Vec::new())
    }

    async fn search_extensions(
        &self,
        _query: &crate::models::SearchQuery,
    ) -> Result<Vec<crate::models::ExtensionInfo>> {
        // Registry store doesn't provide extension search - it only tracks installations
        Ok(Vec::new())
    }

    async fn get_extension_info(&self, _name: &str) -> Result<Vec<crate::models::ExtensionInfo>> {
        // Registry store doesn't provide extension info - it only tracks installations
        Ok(Vec::new())
    }

    async fn get_extension_version_info(
        &self,
        name: &str,
        _version: Option<&str>,
    ) -> Result<crate::models::ExtensionInfo> {
        Err(StoreError::ExtensionNotFound(format!(
            "Registry store does not provide extension info for {}",
            name
        )))
    }

    async fn get_manifest(
        &self,
        name: &str,
        _version: Option<&str>,
    ) -> Result<crate::manifest::ExtensionManifest> {
        // Try to get manifest from installed extension
        if let Some(installed) = self.registry.extensions.get(name) {
            Ok(installed.manifest.clone())
        } else {
            Err(StoreError::ExtensionNotFound(name.to_string()))
        }
    }

    async fn get_metadata(
        &self,
        _name: &str,
        _version: Option<&str>,
    ) -> Result<Option<crate::models::ExtensionMetadata>> {
        // Registry store doesn't provide metadata
        Ok(None)
    }

    async fn get_extension_wasm(&self, name: &str, _version: Option<&str>) -> Result<Vec<u8>> {
        // Try to load WASM from installed extension
        if let Some(installed) = self.registry.extensions.get(name) {
            let wasm_path = installed.get_wasm_path();
            if wasm_path.exists() {
                fs::read(&wasm_path).await.map_err(StoreError::from)
            } else {
                Err(StoreError::ExtensionNotFound(format!(
                    "WASM file not found for {}",
                    name
                )))
            }
        } else {
            Err(StoreError::ExtensionNotFound(name.to_string()))
        }
    }

    async fn get_extension_package(
        &self,
        name: &str,
        version: Option<&str>,
    ) -> Result<crate::models::ExtensionPackage> {
        if let Some(installed) = self.registry.extensions.get(name) {
            let manifest = installed.manifest.clone();
            let wasm_component = self.get_extension_wasm(name, version).await?;

            let package = crate::models::ExtensionPackage::new(
                manifest,
                wasm_component,
                self.info.name.clone(),
            )
            .with_layout(installed.package_layout.clone());

            Ok(package)
        } else {
            Err(StoreError::ExtensionNotFound(name.to_string()))
        }
    }

    async fn install_extension(
        &self,
        _name: &str,
        _version: Option<&str>,
        _target_dir: &Path,
        _options: &crate::models::InstallOptions,
    ) -> Result<InstalledExtension> {
        Err(StoreError::UnsupportedOperation(
            "Registry store cannot install extensions directly".to_string(),
        ))
    }

    async fn check_updates(
        &self,
        _installed: &[InstalledExtension],
    ) -> Result<Vec<crate::models::UpdateInfo>> {
        // Registry store doesn't check for updates
        Ok(Vec::new())
    }

    async fn get_latest_version(&self, _name: &str) -> Result<Option<String>> {
        // Registry store doesn't provide version information
        Ok(None)
    }

    async fn update_extension(
        &self,
        _name: &str,
        _target_dir: &Path,
        _options: &crate::models::UpdateOptions,
    ) -> Result<InstalledExtension> {
        Err(StoreError::UnsupportedOperation(
            "Registry store cannot update extensions directly".to_string(),
        ))
    }

    async fn list_versions(&self, _name: &str) -> Result<Vec<String>> {
        // Registry store doesn't provide version listings
        Ok(Vec::new())
    }

    async fn version_exists(&self, name: &str, version: &str) -> Result<bool> {
        // Check if this specific version is installed
        if let Some(installed) = self.registry.extensions.get(name) {
            Ok(installed.version == version)
        } else {
            Ok(false)
        }
    }

    fn supports_capability(&self, capability: &str) -> bool {
        match capability {
            crate::store::capabilities::METADATA => false, // No metadata discovery
            crate::store::capabilities::SEARCH => false,   // No extension search
            crate::store::capabilities::VERSIONING => false, // No version management
            crate::store::capabilities::UPDATE_CHECK => false, // No update checking
            "registry" => true,                            // Registry management
            "installation_tracking" => true,               // Installation tracking
            _ => false,
        }
    }

    fn capabilities(&self) -> Vec<String> {
        vec!["registry".to_string(), "installation_tracking".to_string()]
    }
}

#[async_trait]
impl RegistryStore for LocalRegistryStore {
    async fn register_installation(&mut self, installation: InstalledExtension) -> Result<()> {
        debug!(
            "Registering installation: {}@{}",
            installation.name, installation.version
        );

        self.registry
            .extensions
            .insert(installation.name.clone(), installation);
        self.save_registry().await?;

        Ok(())
    }

    async fn unregister_installation(&mut self, name: &str) -> Result<bool> {
        debug!("Unregistering installation: {}", name);

        let removed = self.registry.extensions.remove(name).is_some();
        if removed {
            self.save_registry().await?;
        }

        Ok(removed)
    }

    async fn update_installation(&mut self, installation: InstalledExtension) -> Result<()> {
        if self.registry.extensions.contains_key(&installation.name) {
            debug!(
                "Updating installation: {}@{}",
                installation.name, installation.version
            );
            self.registry
                .extensions
                .insert(installation.name.clone(), installation);
            self.save_registry().await?;
            Ok(())
        } else {
            Err(StoreError::ExtensionNotFound(installation.name))
        }
    }

    async fn list_installed(&self) -> Result<Vec<InstalledExtension>> {
        Ok(self.registry.extensions.values().cloned().collect())
    }

    async fn get_installed(&self, name: &str) -> Result<Option<InstalledExtension>> {
        Ok(self.registry.extensions.get(name).cloned())
    }

    async fn find_installed(&self, query: &InstallationQuery) -> Result<Vec<InstalledExtension>> {
        let mut results = Vec::new();

        for ext in self.registry.extensions.values() {
            if self.matches_query(ext, query) {
                results.push(ext.clone());
            }
        }

        // Sort by installation date (newest first)
        results.sort_by(|a, b| b.installed_at.cmp(&a.installed_at));

        Ok(results)
    }

    async fn get_installation_stats(&self) -> Result<InstallationStats> {
        Ok(self.calculate_stats(&self.registry.extensions))
    }

    async fn validate_installations(&self) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        for ext in self.registry.extensions.values() {
            // Check if installation directory exists
            if !ext.install_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: ext.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: format!(
                        "Installation directory not found: {}",
                        ext.install_path.display()
                    ),
                    severity: IssueSeverity::Error,
                });
                continue;
            }

            // Check if WASM file exists
            let wasm_path = ext.get_wasm_path();
            if !wasm_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: ext.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: "WASM component file not found".to_string(),
                    severity: IssueSeverity::Critical,
                });
            }

            // Check if manifest file exists
            let manifest_path = ext.get_manifest_path();
            if !manifest_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: ext.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: "Manifest file not found".to_string(),
                    severity: IssueSeverity::Error,
                });
            } else {
                // Validate manifest can be parsed
                if let Err(_) = fs::read_to_string(&manifest_path)
                    .await
                    .and_then(|content| {
                        serde_json::from_str::<crate::manifest::ExtensionManifest>(&content)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                    })
                {
                    issues.push(ValidationIssue {
                        extension_name: ext.name.clone(),
                        issue_type: ValidationIssueType::InvalidManifest,
                        description: "Manifest file is corrupted or invalid".to_string(),
                        severity: IssueSeverity::Error,
                    });
                }
            }

            // Validate checksum if possible
            if wasm_path.exists() {
                if let Ok(wasm_content) = fs::read(&wasm_path).await {
                    if !ext.manifest.checksum.verify(&wasm_content) {
                        issues.push(ValidationIssue {
                            extension_name: ext.name.clone(),
                            issue_type: ValidationIssueType::ChecksumMismatch,
                            description: "WASM file checksum does not match manifest".to_string(),
                            severity: IssueSeverity::Warning,
                        });
                    }
                }
            }
        }

        Ok(issues)
    }

    async fn cleanup_orphaned(&mut self) -> Result<u32> {
        let mut removed_count = 0;
        let mut to_remove = Vec::new();

        for (name, ext) in &self.registry.extensions {
            if !ext.install_path.exists() {
                warn!("Found orphaned registry entry for extension: {}", name);
                to_remove.push(name.clone());
            }
        }

        for name in to_remove {
            self.registry.extensions.remove(&name);
            removed_count += 1;
        }

        if removed_count > 0 {
            self.save_registry().await?;
            debug!("Cleaned up {} orphaned registry entries", removed_count);
        }

        Ok(removed_count)
    }

    fn registry_path(&self) -> Option<PathBuf> {
        Some(self.registry_path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Checksum, ChecksumAlgorithm, ExtensionManifest};
    use tempfile::TempDir;

    fn create_test_installed_extension(name: &str, version: &str) -> InstalledExtension {
        let manifest = ExtensionManifest {
            name: name.to_string(),
            version: version.to_string(),
            author: "test-author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![],
            attrs: vec![],
            checksum: Checksum {
                algorithm: ChecksumAlgorithm::Sha256,
                value: "test_hash".to_string(),
            },
            signature: None,
        };

        InstalledExtension::new(
            name.to_string(),
            version.to_string(),
            PathBuf::from(format!("/tmp/{}", name)),
            manifest,
            PackageLayout::default(),
            "test-store".to_string(),
        )
    }

    #[tokio::test]
    async fn test_registry_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let store = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        assert_eq!(store.store_info().store_type, "registry");
        // Registry file is created on first save, so just check the path is valid
        assert!(store.registry_path().is_some());

        // Test that we can save and then the file exists
        store.save_registry().await.unwrap();
        assert!(store.registry_path().unwrap().exists());
    }

    #[tokio::test]
    async fn test_register_and_get_installation() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        let ext = create_test_installed_extension("test-ext", "1.0.0");
        store.register_installation(ext.clone()).await.unwrap();

        let retrieved = store.get_installed("test-ext").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().version, "1.0.0");
    }

    #[tokio::test]
    async fn test_unregister_installation() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        let ext = create_test_installed_extension("test-ext", "1.0.0");
        store.register_installation(ext).await.unwrap();

        let removed = store.unregister_installation("test-ext").await.unwrap();
        assert!(removed);

        let retrieved = store.get_installed("test-ext").await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_installation_query() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        let ext1 = create_test_installed_extension("ext-a", "1.0.0");
        let ext2 = create_test_installed_extension("ext-b", "2.0.0");

        store.register_installation(ext1).await.unwrap();
        store.register_installation(ext2).await.unwrap();

        let query = InstallationQuery::new().with_name_pattern("ext-a".to_string());
        let results = store.find_installed(&query).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "ext-a");
    }

    #[tokio::test]
    async fn test_installation_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut store = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        let ext1 = create_test_installed_extension("ext-a", "1.0.0");
        let ext2 = create_test_installed_extension("ext-b", "2.0.0");

        store.register_installation(ext1).await.unwrap();
        store.register_installation(ext2).await.unwrap();

        let stats = store.get_installation_stats().await.unwrap();
        assert_eq!(stats.total_extensions, 2);
        assert_eq!(stats.stores_used.get("test-store"), Some(&2));
    }
}
