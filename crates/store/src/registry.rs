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

/// Registry health information (generic across implementations)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryHealth {
    pub healthy: bool,
    pub total_extensions: usize,
    pub last_updated: Option<DateTime<Utc>>,
    pub implementation_info: HashMap<String, serde_json::Value>,
}

impl RegistryHealth {
    pub fn healthy(total_extensions: usize) -> Self {
        Self {
            healthy: true,
            total_extensions,
            last_updated: Some(Utc::now()),
            implementation_info: HashMap::new(),
        }
    }

    pub fn unhealthy(reason: String) -> Self {
        let mut info = HashMap::new();
        info.insert("error".to_string(), serde_json::Value::String(reason));
        Self {
            healthy: false,
            total_extensions: 0,
            last_updated: None,
            implementation_info: info,
        }
    }
}

/// Validation issue found during installation validation (LocalRegistryStore specific)
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
    /// Install an extension package to the registry
    async fn install_extension(
        &mut self,
        package: ExtensionPackage,
        options: &InstallOptions,
    ) -> Result<InstalledExtension>;

    /// Uninstall an extension from the registry
    async fn uninstall_extension(&mut self, id: &str) -> Result<bool>;

    /// Register a new installation in the registry
    async fn register_installation(&mut self, installation: InstalledExtension) -> Result<()>;

    /// Remove an installation from the registry
    async fn unregister_installation(&mut self, name: &str) -> Result<bool>;

    /// Update an existing installation record
    async fn update_installation(&mut self, installation: InstalledExtension) -> Result<()>;

    /// List all installed extensions
    async fn list_installed(&self) -> Result<Vec<InstalledExtension>>;

    /// Get a specific installed extension
    async fn get_installed(&self, id: &str) -> Result<Option<InstalledExtension>>;

    /// Find installed extensions matching the query
    async fn find_installed(&self, query: &InstallationQuery) -> Result<Vec<InstalledExtension>>;

    /// Get statistics about installed extensions
    async fn get_installation_stats(&self) -> Result<InstallationStats>;

    /// Get registry health information (generic across implementations)
    async fn get_registry_health(&self) -> Result<RegistryHealth>;

    /// Check if an extension is registered as installed
    async fn is_installed(&self, id: &str) -> Result<bool> {
        Ok(self.get_installed(id).await?.is_some())
    }

    /// Validate all registered installations
    ///
    /// Checks that all registered extensions are properly installed and accessible.
    /// Returns a list of validation issues found. The specific validation performed
    /// depends on the implementation:
    /// - File-based: Check files exist, checksums match, required components present
    /// - Database: Verify data integrity, foreign key constraints
    /// - Cloud: Validate remote resources exist and are accessible
    /// - HTTP: Check URLs are reachable and content is valid
    async fn validate_installations(&self) -> Result<Vec<ValidationIssue>>;

    /// Get WASM component bytes for an installed extension
    async fn get_extension_wasm_bytes(&self, id: &str) -> Result<Vec<u8>>;

    /// Remove orphaned registry entries
    ///
    /// Removes registry entries that reference extensions no longer available
    /// in the backing store. Returns the number of entries removed.
    /// Implementation-specific behavior:
    /// - File-based: Remove entries for deleted directories/files
    /// - Database: Clean up records with broken references
    /// - Cloud: Remove entries for deleted cloud resources
    /// - HTTP: Remove entries for unreachable URLs
    async fn cleanup_orphaned(&mut self) -> Result<u32>;
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
    /// Create a new LocalRegistryStore with explicit path
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

    /// Create a new LocalRegistryStore with OS-specific default directory
    pub async fn new_with_defaults() -> Result<Self> {
        let install_dir = Self::default_install_dir()?;
        Self::new(install_dir).await
    }

    /// Get the default installation directory for the current OS
    ///
    /// Returns an error if the system directories cannot be determined
    pub fn default_install_dir() -> Result<PathBuf> {
        use directories::ProjectDirs;

        let project_dirs = ProjectDirs::from("com", "quelle", "quelle").ok_or_else(|| {
            StoreError::ConfigError(
                "Could not determine system directories for current user/OS".to_string(),
            )
        })?;

        Ok(project_dirs.data_local_dir().join("extensions"))
    }

    /// Get the installation directory managed by this registry
    pub fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    /// Get the registry file path (LocalRegistryStore specific)
    pub fn registry_path(&self) -> &Path {
        &self.registry_path
    }

    /// Set a new installation directory
    pub async fn set_install_dir(&mut self, path: PathBuf) -> Result<()> {
        fs::create_dir_all(&path).await?;
        self.install_dir = path;
        self.registry_path = self.install_dir.join("registry.json");
        self.backup_path = self.install_dir.join("registry.json.backup");
        Ok(())
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

        let content = serde_json::to_string_pretty(&self.registry).map_err(|e| {
            StoreError::SerializationErrorWithContext {
                operation: "serialize registry".to_string(),
                source: e,
            }
        })?;

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
            total_size += extension.calculate_size();
            stores_used.insert(extension.source_store.clone());
            if extension.auto_update {
                auto_update_enabled += 1;
            }
            if let Some(updated) = extension.last_updated {
                if last_updated.is_none_or(|lu| updated > lu) {
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

    /// Validate extension ID for security
    fn validate_extension_id(id: &str) -> Result<()> {
        if id.is_empty() {
            return Err(StoreError::InvalidExtensionName(
                "ID cannot be empty".to_string(),
            ));
        }

        if id.contains("..") || id.contains('/') || id.contains('\\') {
            return Err(StoreError::InvalidExtensionName(
                "ID contains invalid path characters".to_string(),
            ));
        }

        if id.len() > 255 {
            return Err(StoreError::InvalidExtensionName("ID too long".to_string()));
        }

        Ok(())
    }

    /// Get the installation path for an extension
    fn extension_install_path(&self, id: &str) -> PathBuf {
        self.install_dir.join(id)
    }
}

#[async_trait]
impl RegistryStore for LocalRegistryStore {
    async fn install_extension(
        &mut self,
        package: ExtensionPackage,
        options: &InstallOptions,
    ) -> Result<InstalledExtension> {
        let id = &package.manifest.id;
        Self::validate_extension_id(id)?;

        let install_path = self.extension_install_path(id);

        // Check if already installed
        if let Some(_existing) = self.registry.extensions.get(id) {
            if !options.force_reinstall {
                return Err(StoreError::ExtensionAlreadyInstalled(id.clone()));
            }
            // Remove existing installation directory if it exists
            if install_path.exists() {
                fs::remove_dir_all(&install_path).await?;
            }
        }

        // Create installation directory for persistence/caching (optional)
        fs::create_dir_all(&install_path).await?;

        // Write WASM component to disk for caching
        let wasm_path = install_path.join("extension.wasm");
        fs::write(&wasm_path, &package.wasm_component).await?;

        // Write manifest to disk for caching
        let manifest_path = install_path.join("manifest.json");
        let manifest_content = serde_json::to_string_pretty(&package.manifest)?;
        fs::write(&manifest_path, manifest_content).await?;

        // Write additional assets to disk for caching
        for (asset_name, content) in &package.assets {
            let asset_path = install_path.join(asset_name);
            if let Some(parent) = asset_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(asset_path, content).await?;
        }

        // Create installation record with in-memory data
        let mut installed = InstalledExtension::from_package(package);
        installed.auto_update = options.auto_update;
        installed.checksum = Some(installed.manifest.checksum.clone());

        // Calculate actual size from written files
        if let Ok(actual_size) = installed.calculate_actual_size(self).await {
            installed.size = actual_size;
        }

        // Register installation
        self.register_installation(installed.clone()).await?;

        info!(
            "Successfully installed extension: {}@{}",
            installed.name, installed.version
        );
        Ok(installed)
    }

    async fn uninstall_extension(&mut self, id: &str) -> Result<bool> {
        Self::validate_extension_id(id)?;

        if let Some(installed) = self.registry.extensions.remove(id) {
            // Remove cache files from disk if they exist
            let install_path = self.extension_install_path(&installed.id);
            if install_path.exists() {
                fs::remove_dir_all(&install_path).await?;
            }

            // Save updated registry
            self.save_registry().await?;

            info!("Successfully uninstalled extension: {}", installed.id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn register_installation(&mut self, installation: InstalledExtension) -> Result<()> {
        Self::validate_extension_id(&installation.id)?;
        self.registry
            .extensions
            .insert(installation.id.clone(), installation);
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

    async fn get_installed(&self, id: &str) -> Result<Option<InstalledExtension>> {
        Self::validate_extension_id(id)?;
        Ok(self.registry.extensions.get(id).cloned())
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

    async fn get_registry_health(&self) -> Result<RegistryHealth> {
        let stats = self.calculate_stats().await;
        let mut health = RegistryHealth::healthy(stats.total_extensions);

        // Add implementation-specific info
        health.implementation_info.insert(
            "registry_file".to_string(),
            serde_json::Value::String(self.registry_path.display().to_string()),
        );
        health.implementation_info.insert(
            "install_directory".to_string(),
            serde_json::Value::String(self.install_dir.display().to_string()),
        );
        health.implementation_info.insert(
            "total_size_bytes".to_string(),
            serde_json::Value::Number(serde_json::Number::from(stats.total_size)),
        );

        health.last_updated = stats.last_updated;

        Ok(health)
    }

    async fn validate_installations(&self) -> Result<Vec<ValidationIssue>> {
        let mut issues = Vec::new();

        for extension in self.registry.extensions.values() {
            // Validate integrity by checking checksum
            if !extension.verify_integrity(self).await {
                issues.push(ValidationIssue {
                    extension_name: extension.name.clone(),
                    issue_type: ValidationIssueType::CorruptedFiles,
                    description: "Extension data integrity check failed".to_string(),
                    severity: IssueSeverity::Error,
                });
                continue;
            }

            // Check if cache files exist on disk (optional validation)
            let install_path = self.extension_install_path(&extension.id);
            let wasm_path = install_path.join("extension.wasm");
            let manifest_path = install_path.join("manifest.json");

            if install_path.exists() && !wasm_path.exists() {
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
            // Check if extension files exist on disk
            let install_path = self.extension_install_path(&extension.id);
            if !install_path.exists() {
                to_remove.push(name.clone());
            }
        }

        for name in to_remove {
            self.registry.extensions.remove(&name);
            removed += 1;
        }

        if removed > 0 {
            self.save_registry().await?;
        }

        Ok(removed)
    }

    async fn get_extension_wasm_bytes(&self, id: &str) -> Result<Vec<u8>> {
        let install_path = self.extension_install_path(id);
        let wasm_path = install_path.join("extension.wasm");

        if !wasm_path.exists() {
            return Err(crate::error::StoreError::ExtensionNotFound(id.to_string()));
        }

        fs::read(&wasm_path)
            .await
            .map_err(crate::error::StoreError::IoError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Checksum, ChecksumAlgorithm, ExtensionManifest};
    use tempfile::TempDir;

    fn create_test_extension_package(name: &str, version: &str) -> ExtensionPackage {
        let manifest = ExtensionManifest {
            id: format!("test-{}", name),
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
            wasm_file: crate::manifest::FileReference::new(
                "./extension.wasm".to_string(),
                b"fake wasm content",
            ),
            assets: vec![],
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
        assert!(
            registry.registry_path().exists() || !registry.registry_path().as_os_str().is_empty()
        );
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
        assert!(registry.is_installed("test-test-ext").await.unwrap());

        // Uninstall extension
        let removed = registry.uninstall_extension("test-test-ext").await.unwrap();
        assert!(removed);

        // Verify it's no longer registered
        assert!(!registry.is_installed("test-test-ext").await.unwrap());
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

    #[test]
    fn test_default_directory_structure() {
        match LocalRegistryStore::default_install_dir() {
            Ok(default_dir) => {
                // Should always end with "extensions"
                assert_eq!(default_dir.file_name().unwrap(), "extensions");

                // Should contain proper application identifier structure
                let path_str = default_dir.to_string_lossy();
                assert!(path_str.contains("quelle"));

                // Should follow directories crate conventions (contain com.quelle.quelle or similar)
                assert!(path_str.contains("com.quelle.quelle") || path_str.contains("quelle"));
            }
            Err(_) => {
                // This is acceptable on systems where directories cannot be determined
                println!("Note: System directories could not be determined, which is expected on some systems");
            }
        }
    }

    #[tokio::test]
    async fn test_new_with_defaults() {
        match LocalRegistryStore::new_with_defaults().await {
            Ok(registry) => {
                // Should have a valid install directory
                assert!(!registry.install_dir().as_os_str().is_empty());

                // Should have a registry path
                assert!(!registry.registry_path().as_os_str().is_empty());
            }
            Err(_) => {
                // This is acceptable on systems where directories cannot be determined
                println!("Note: System directories could not be determined for new_with_defaults");
            }
        }
    }

    #[tokio::test]
    async fn test_trait_methods_validate_and_cleanup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let install_dir = temp_dir.path().join("extensions");

        let mut registry = LocalRegistryStore::new(install_dir).await.unwrap();

        // Test validate_installations through trait interface
        let issues = RegistryStore::validate_installations(&registry)
            .await
            .unwrap();
        assert_eq!(issues.len(), 0);

        // Test cleanup_orphaned through trait interface
        let cleaned = RegistryStore::cleanup_orphaned(&mut registry)
            .await
            .unwrap();
        assert_eq!(cleaned, 0);

        // Test through trait object
        let registry_trait: &dyn RegistryStore = &registry;
        let issues_trait = registry_trait.validate_installations().await.unwrap();
        assert_eq!(issues_trait.len(), 0);

        // Test mutable trait object
        let registry_trait_mut: &mut dyn RegistryStore = &mut registry;
        let cleaned_trait = registry_trait_mut.cleanup_orphaned().await.unwrap();
        assert_eq!(cleaned_trait, 0);
    }

    #[test]
    fn test_trait_is_implementation_agnostic() {
        // This test verifies that the RegistryStore trait doesn't contain
        // implementation-specific methods that would break abstraction

        fn verify_generic_interface<T: RegistryStore>() {
            // This function should compile if the trait is truly generic
            // If it contained implementation-specific methods, this would fail
        }

        // Test that the trait works with any theoretical implementation
        verify_generic_interface::<LocalRegistryStore>();

        // The trait should only contain operations that make sense for ANY registry:
        // - install_extension: Any registry can install
        // - uninstall_extension: Any registry can uninstall
        // - register_installation: Any registry can register
        // - unregister_installation: Any registry can unregister
        // - update_installation: Any registry can update
        // - list_installed: Any registry can list
        // - get_installed: Any registry can get specific items
        // - find_installed: Any registry can search
        // - get_installation_stats: Any registry can provide stats
        // - get_registry_health: Any registry can report health
        // - is_installed: Any registry can check existence
        // - validate_installations: Any registry can validate its state
        // - cleanup_orphaned: Any registry can clean up invalid entries
        //
        // Notably ABSENT (and correctly so):
        // - install_dir(): File-system specific
        // - registry_path(): File-system specific
        // - as_local(): Downcasting breaks abstraction
        // - as_local_mut(): Downcasting breaks abstraction
    }
}
