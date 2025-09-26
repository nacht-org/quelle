use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, info, warn};

use crate::error::{Result, StoreError};
use crate::models::{ExtensionPackage, InstallOptions, InstalledExtension};

/// Query parameters for finding installed extensions
#[derive(Debug, Clone, Default)]
pub struct InstallationQuery {
    pub name_pattern: Option<String>,
    pub installed_after: Option<DateTime<Utc>>,
    pub installed_before: Option<DateTime<Utc>>,
    pub from_store: Option<String>,
    pub version_pattern: Option<String>,
    pub auto_update_only: Option<bool>,
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
    pub stores_used: Vec<String>,
    pub auto_update_enabled: usize,
    pub last_updated: Option<DateTime<Utc>>,
}

/// Validation issue found during installation validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub extension_name: String,
    pub issue_type: ValidationIssueType,
    pub description: String,
    pub severity: IssueSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationIssueType {
    MissingFiles,
    CorruptedFiles,
    InvalidManifest,
    PathMismatch,
    ChecksumMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Core trait for managing installed extensions registry
#[async_trait]
pub trait RegistryStore: Send + Sync {
    /// Get the installation directory managed by this registry
    fn install_dir(&self) -> &Path;

    /// Set a new installation directory
    async fn set_install_dir(&mut self, path: PathBuf) -> Result<()>;

    /// Install an extension package to the registry
    async fn install_extension(
        &mut self,
        package: ExtensionPackage,
        options: &InstallOptions,
    ) -> Result<InstalledExtension>;

    /// Uninstall an extension from the registry
    async fn uninstall_extension(&mut self, name: &str) -> Result<bool>;

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

/// JSON-based registry data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRegistry {
    extensions: HashMap<String, InstalledExtension>,
    last_updated: DateTime<Utc>,
    version: String,
    stats: InstallationStats,
}

impl Default for JsonRegistry {
    fn default() -> Self {
        Self {
            extensions: HashMap::new(),
            last_updated: Utc::now(),
            version: "1.0".to_string(),
            stats: InstallationStats {
                total_extensions: 0,
                total_size: 0,
                stores_used: Vec::new(),
                auto_update_enabled: 0,
                last_updated: None,
            },
        }
    }
}

/// Local file-system based registry store implementation
pub struct LocalRegistryStore {
    registry_path: PathBuf,
    backup_path: PathBuf,
    install_dir: PathBuf,
    registry: JsonRegistry,
}

impl LocalRegistryStore {
    /// Create a new LocalRegistryStore
    pub async fn new<P: AsRef<Path>>(install_dir: P) -> Result<Self> {
        let install_dir = install_dir.as_ref().to_path_buf();
        let registry_path = install_dir.join("registry.json");
        let backup_path = install_dir.join("registry.json.backup");

        // Ensure install directory exists
        fs::create_dir_all(&install_dir).await?;

        let mut store = Self {
            registry_path,
            backup_path,
            install_dir,
            registry: JsonRegistry::default(),
        };

        // Load existing registry if it exists
        store.load_registry().await?;
        Ok(store)
    }

    /// Load registry from disk
    async fn load_registry(&mut self) -> Result<()> {
        if !self.registry_path.exists() {
            info!("No existing registry found, creating new one");
            return Ok(());
        }

        match fs::read_to_string(&self.registry_path).await {
            Ok(content) => {
                self.registry = serde_json::from_str(&content)
                    .map_err(|e| StoreError::CorruptedRegistry(e.to_string()))?;
                debug!(
                    "Loaded registry with {} extensions",
                    self.registry.extensions.len()
                );
            }
            Err(_) => {
                warn!("Failed to load registry, checking backup");
                if self.backup_path.exists() {
                    let backup_content = fs::read_to_string(&self.backup_path).await?;
                    self.registry = serde_json::from_str(&backup_content)
                        .map_err(|e| StoreError::CorruptedRegistry(e.to_string()))?;
                    info!("Restored registry from backup");
                }
            }
        }
        Ok(())
    }

    /// Save registry to disk with atomic backup
    async fn save_registry(&mut self) -> Result<()> {
        // Update stats before saving
        self.registry.stats = self.calculate_stats().await;
        self.registry.last_updated = Utc::now();

        let content =
            serde_json::to_string_pretty(&self.registry).map_err(StoreError::SerializationError)?;

        // Create backup if registry exists
        if self.registry_path.exists() {
            if let Err(e) = fs::copy(&self.registry_path, &self.backup_path).await {
                warn!("Failed to create backup: {}", e);
            }
        }

        // Write new registry
        fs::write(&self.registry_path, content).await?;
        debug!("Registry saved successfully");
        Ok(())
    }

    /// Calculate current installation statistics
    async fn calculate_stats(&self) -> InstallationStats {
        let total_extensions = self.registry.extensions.len();
        let mut total_size = 0u64;
        let mut stores_used = std::collections::HashSet::new();
        let mut auto_update_enabled = 0;
        let mut last_updated = None;

        for extension in self.registry.extensions.values() {
            total_size += extension.size.unwrap_or(0);
            stores_used.insert(extension.source_store.clone());
            if extension.auto_update {
                auto_update_enabled += 1;
            }
            if let Some(updated) = extension.last_updated {
                if last_updated.map_or(true, |lu| updated > lu) {
                    last_updated = Some(updated);
                }
            }
        }

        InstallationStats {
            total_extensions,
            total_size,
            stores_used: stores_used.into_iter().collect(),
            auto_update_enabled,
            last_updated,
        }
    }

    /// Check if an extension matches the given query
    fn matches_query(&self, extension: &InstalledExtension, query: &InstallationQuery) -> bool {
        if let Some(ref pattern) = query.name_pattern {
            if !extension.name.contains(pattern) {
                return false;
            }
        }

        if let Some(after) = query.installed_after {
            if extension.installed_at < after {
                return false;
            }
        }

        if let Some(before) = query.installed_before {
            if extension.installed_at > before {
                return false;
            }
        }

        if let Some(ref store) = query.from_store {
            if extension.source_store != *store {
                return false;
            }
        }

        if let Some(ref version_pattern) = query.version_pattern {
            if !extension.version.contains(version_pattern) {
                return false;
            }
        }

        if let Some(auto_update_only) = query.auto_update_only {
            if extension.auto_update != auto_update_only {
                return false;
            }
        }

        true
    }

    /// Validate extension name for security
    fn validate_extension_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(StoreError::InvalidExtensionName(
                "Name cannot be empty".to_string(),
            ));
        }

        if name.contains("..") || name.contains('/') || name.contains('\\') {
            return Err(StoreError::InvalidExtensionName(
                "Name contains invalid path characters".to_string(),
            ));
        }

        if name.len() > 255 {
            return Err(StoreError::InvalidExtensionName(
                "Name too long".to_string(),
            ));
        }

        Ok(())
    }

    /// Get the installation path for an extension
    fn extension_install_path(&self, name: &str) -> PathBuf {
        self.install_dir.join(name)
    }
}

#[async_trait]
impl RegistryStore for LocalRegistryStore {
    fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    async fn set_install_dir(&mut self, path: PathBuf) -> Result<()> {
        fs::create_dir_all(&path).await?;
        self.install_dir = path;
        self.registry_path = self.install_dir.join("registry.json");
        self.backup_path = self.install_dir.join("registry.json.backup");
        Ok(())
    }

    async fn install_extension(
        &mut self,
        package: ExtensionPackage,
        options: &InstallOptions,
    ) -> Result<InstalledExtension> {
        let name = &package.manifest.name;
        Self::validate_extension_name(name)?;

        let install_path = self.extension_install_path(name);

        // Check if already installed
        if let Some(_existing) = self.registry.extensions.get(name) {
            if !options.force_reinstall {
                return Err(StoreError::ExtensionAlreadyInstalled(name.clone()));
            }
            // Remove existing installation
            if install_path.exists() {
                fs::remove_dir_all(&install_path).await?;
            }
        }

        // Create installation directory
        fs::create_dir_all(&install_path).await?;

        // Write WASM component
        let wasm_path = install_path.join("extension.wasm");
        fs::write(&wasm_path, &package.wasm_component).await?;

        // Write manifest
        let manifest_path = install_path.join("manifest.json");
        let manifest_content = serde_json::to_string_pretty(&package.manifest)?;
        fs::write(&manifest_path, manifest_content).await?;

        // Write additional assets
        for (asset_name, content) in &package.assets {
            let asset_path = install_path.join(asset_name);
            if let Some(parent) = asset_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(asset_path, content).await?;
        }

        // Calculate size before moving package
        let package_size = package.calculate_total_size();

        // Create installation record
        let installed = InstalledExtension {
            name: name.clone(),
            version: package.manifest.version.clone(),
            install_path,
            installed_at: Utc::now(),
            last_updated: Some(Utc::now()),
            source_store: package.source_store,
            auto_update: options.auto_update,
            size: Some(package_size),
            checksum: Some(package.manifest.checksum.clone()),
        };

        // Register installation
        self.register_installation(installed.clone()).await?;

        info!(
            "Successfully installed extension: {}@{}",
            name, installed.version
        );
        Ok(installed)
    }

    async fn uninstall_extension(&mut self, name: &str) -> Result<bool> {
        Self::validate_extension_name(name)?;

        if let Some(installed) = self.registry.extensions.remove(name) {
            // Remove files from disk
            if installed.install_path.exists() {
                fs::remove_dir_all(&installed.install_path).await?;
            }

            // Save updated registry
            self.save_registry().await?;

            info!("Successfully uninstalled extension: {}", name);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn register_installation(&mut self, installation: InstalledExtension) -> Result<()> {
        Self::validate_extension_name(&installation.name)?;
        self.registry
            .extensions
            .insert(installation.name.clone(), installation);
        self.save_registry().await?;
        Ok(())
    }

    async fn unregister_installation(&mut self, name: &str) -> Result<bool> {
        Self::validate_extension_name(name)?;
        let removed = self.registry.extensions.remove(name).is_some();
        if removed {
            self.save_registry().await?;
        }
        Ok(removed)
    }

    async fn update_installation(&mut self, installation: InstalledExtension) -> Result<()> {
        Self::validate_extension_name(&installation.name)?;
        self.registry
            .extensions
            .insert(installation.name.clone(), installation);
        self.save_registry().await?;
        Ok(())
    }

    async fn list_installed(&self) -> Result<Vec<InstalledExtension>> {
        Ok(self.registry.extensions.values().cloned().collect())
    }

    async fn get_installed(&self, name: &str) -> Result<Option<InstalledExtension>> {
        Self::validate_extension_name(name)?;
        Ok(self.registry.extensions.get(name).cloned())
    }

    async fn find_installed(&self, query: &InstallationQuery) -> Result<Vec<InstalledExtension>> {
        Ok(self
            .registry
            .extensions
            .values()
            .filter(|ext| self.matches_query(ext, query))
            .cloned()
            .collect())
    }

    async fn get_installation_stats(&self) -> Result<InstallationStats> {
        Ok(self.calculate_stats().await)
    }

    async fn validate_installations(&self) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        for extension in self.registry.extensions.values() {
            // Check if installation path exists
            if !extension.install_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: extension.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: "Installation directory not found".to_string(),
                    severity: IssueSeverity::Error,
                });
                continue;
            }

            // Check for required files
            let wasm_path = extension.install_path.join("extension.wasm");
            let manifest_path = extension.install_path.join("manifest.json");

            if !wasm_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: extension.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: "WASM component file missing".to_string(),
                    severity: IssueSeverity::Critical,
                });
            }

            if !manifest_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: extension.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: "Manifest file missing".to_string(),
                    severity: IssueSeverity::Error,
                });
            }

            // Validate checksum if available
            if let Some(ref checksum) = extension.checksum {
                if wasm_path.exists() {
                    if let Ok(wasm_content) = fs::read(&wasm_path).await {
                        if !checksum.verify(&wasm_content) {
                            issues.push(ValidationIssue {
                                extension_name: extension.name.clone(),
                                issue_type: ValidationIssueType::ChecksumMismatch,
                                description: "WASM component checksum verification failed"
                                    .to_string(),
                                severity: IssueSeverity::Critical,
                            });
                        }
                    }
                }
            }
        }

        Ok(issues)
    }

    async fn cleanup_orphaned(&mut self) -> Result<u32> {
        let mut removed = 0;
        let mut to_remove = Vec::new();

        for (name, extension) in &self.registry.extensions {
            if !extension.install_path.exists() {
                to_remove.push(name.clone());
            }
        }

        for name in to_remove {
            self.registry.extensions.remove(&name);
            removed += 1;
        }

        if removed > 0 {
            self.save_registry().await?;
            info!("Cleaned up {} orphaned registry entries", removed);
        }

        Ok(removed)
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

    fn create_test_extension_package(name: &str, version: &str) -> ExtensionPackage {
        let manifest = ExtensionManifest {
            name: name.to_string(),
            version: version.to_string(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![],
            attrs: vec![],
            checksum: Checksum {
                algorithm: ChecksumAlgorithm::Sha256,
                value: "dummy_hash".to_string(),
            },
            signature: None,
        };

        ExtensionPackage::new(
            manifest,
            b"dummy wasm content".to_vec(),
            "test-store".to_string(),
        )
    }

    #[tokio::test]
    async fn test_registry_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        assert_eq!(registry.install_dir(), temp_dir.path());
        assert!(registry.registry_path().is_some());
    }

    #[tokio::test]
    async fn test_install_and_uninstall_extension() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        let package = create_test_extension_package("test-ext", "1.0.0");
        let options = InstallOptions::default();

        // Install extension
        let installed = registry.install_extension(package, &options).await.unwrap();
        assert_eq!(installed.name, "test-ext");
        assert_eq!(installed.version, "1.0.0");

        // Verify it's registered
        assert!(registry.is_installed("test-ext").await.unwrap());

        // Uninstall extension
        let removed = registry.uninstall_extension("test-ext").await.unwrap();
        assert!(removed);

        // Verify it's no longer registered
        assert!(!registry.is_installed("test-ext").await.unwrap());
    }

    #[tokio::test]
    async fn test_installation_query() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        let package1 = create_test_extension_package("ext1", "1.0.0");
        let package2 = create_test_extension_package("ext2", "2.0.0");

        let options = InstallOptions::default();
        registry
            .install_extension(package1, &options)
            .await
            .unwrap();
        registry
            .install_extension(package2, &options)
            .await
            .unwrap();

        // Test name pattern query
        let query = InstallationQuery::new().with_name_pattern("ext1".to_string());
        let results = registry.find_installed(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "ext1");
    }

    #[tokio::test]
    async fn test_installation_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        let package = create_test_extension_package("test-ext", "1.0.0");
        let options = InstallOptions::default();
        registry.install_extension(package, &options).await.unwrap();

        let stats = registry.get_installation_stats().await.unwrap();
        assert_eq!(stats.total_extensions, 1);
        assert!(stats.stores_used.contains(&"test-store".to_string()));
    }
}
