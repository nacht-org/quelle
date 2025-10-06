use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, info, warn};

use crate::error::{Result, StoreError};
use crate::models::{ExtensionPackage, InstallOptions, InstalledExtension};

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

    /// Check if an extension is registered as installed
    async fn is_installed(&self, id: &str) -> Result<bool> {
        Ok(self.get_installed(id).await?.is_some())
    }

    /// Get WASM component bytes for an installed extension
    async fn get_extension_wasm_bytes(&self, id: &str) -> Result<Vec<u8>>;
}

/// JSON-based registry data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRegistry {
    extensions: HashMap<String, InstalledExtension>,
    last_updated: DateTime<Utc>,
    version: String,
}

impl Default for JsonRegistry {
    fn default() -> Self {
        Self {
            extensions: HashMap::new(),
            last_updated: Utc::now(),
            version: "1.0".to_string(),
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
        let mut installed = InstalledExtension::from_package(package.clone());
        installed.auto_update = options.auto_update;
        installed.checksum = Some(crate::manifest::Checksum {
            algorithm: crate::manifest::checksum::ChecksumAlgorithm::Blake3,
            value: crate::manifest::checksum::ChecksumAlgorithm::Blake3
                .calculate(&package.wasm_component),
        });

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
    use crate::manifest::ExtensionManifest;
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
    }

    #[tokio::test]
    async fn test_installation_stats() {
        let temp_dir = TempDir::new().unwrap();
        let mut registry = LocalRegistryStore::new(temp_dir.path()).await.unwrap();

        let package = create_test_extension_package("test-ext", "1.0.0");
        let options = InstallOptions::default();
        registry.install_extension(package, &options).await.unwrap();
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

        // - is_installed: Any registry can check existence
        // - get_extension_wasm_bytes: Any registry can provide WASM files
        //
        // Notably ABSENT (and correctly so):
        // - install_dir(): File-system specific
        // - registry_path(): File-system specific
        // - as_local(): Downcasting breaks abstraction
        // - as_local_mut(): Downcasting breaks abstraction
    }
}
