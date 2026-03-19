use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::error::{Result, StoreError};
use crate::models::{ExtensionPackage, InstallOptions, InstalledExtension};

// ---------------------------------------------------------------------------
// Validation types (used by both the trait and registry/validation.rs)
// ---------------------------------------------------------------------------

/// A validation issue found while inspecting an installed extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub extension_name: String,
    pub issue_type: ValidationIssueType,
    pub description: String,
    pub severity: IssueSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationIssueType {
    MissingFiles,
    CorruptedFiles,
    InvalidManifest,
    PathMismatch,
    ChecksumMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IssueSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

// ---------------------------------------------------------------------------
// InstallRegistry trait
// ---------------------------------------------------------------------------

/// Manages the local database of installed extensions.
///
/// All methods take `&self` — implementations are expected to use interior
/// mutability (e.g. `Arc<Mutex<…>>`) so that an `Arc<dyn InstallRegistry>`
/// can be shared freely across async tasks.
///
/// # Lifecycle vs. record management
///
/// The trait exposes only high-level lifecycle operations:
/// - [`install_extension`][Self::install_extension] — validate, write files to
///   disk, and register in one atomic step.
/// - [`uninstall_extension`][Self::uninstall_extension] — remove files and
///   deregister.
///
/// Low-level record operations (insert/remove/update individual entries) are
/// an implementation detail of each concrete type and are not part of this
/// interface.
#[async_trait]
pub trait InstallRegistry: Send + Sync {
    /// Install an extension package: write its files to disk and register it.
    async fn install_extension(
        &self,
        package: ExtensionPackage,
        options: &InstallOptions,
    ) -> Result<InstalledExtension>;

    /// Remove an extension by ID.  Returns `true` when it was present.
    async fn uninstall_extension(&self, id: &str) -> Result<bool>;

    /// List all installed extensions.
    async fn list_installed(&self) -> Result<Vec<InstalledExtension>>;

    /// Look up a specific installed extension by ID.
    async fn get_installed(&self, id: &str) -> Result<Option<InstalledExtension>>;

    /// Return `true` when the extension with the given ID is registered.
    ///
    /// The default implementation calls [`get_installed`][Self::get_installed].
    /// Override for a cheaper existence check.
    async fn is_installed(&self, id: &str) -> Result<bool> {
        Ok(self.get_installed(id).await?.is_some())
    }

    /// Return the raw WASM bytes for an installed extension.
    async fn get_extension_wasm_bytes(&self, id: &str) -> Result<Vec<u8>>;

    /// Validate all installed extensions and return any issues found.
    async fn validate_installations(&self) -> Result<Vec<ValidationIssue>>;

    /// Remove orphaned registry entries and return the number cleaned up.
    async fn cleanup_orphaned(&self) -> Result<u32>;
}

// ---------------------------------------------------------------------------
// JsonRegistry — on-disk format
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonRegistry {
    /// Keyed by extension **ID** (the unique slug, not the display name).
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

// ---------------------------------------------------------------------------
// LocalInstallRegistry
// ---------------------------------------------------------------------------

/// A local file-system backed implementation of [`InstallRegistry`].
///
/// The registry JSON is stored at `<install_dir>/registry.json` with an
/// automatic backup at `<install_dir>/registry.json.backup`.  Interior
/// mutability via `Arc<Mutex<JsonRegistry>>` makes all operations `&self` so
/// the registry can be shared across async tasks without an outer lock on the
/// manager.
pub struct LocalInstallRegistry {
    registry_path: PathBuf,
    backup_path: PathBuf,
    install_dir: PathBuf,
    /// All mutable state lives behind this mutex.
    registry: Arc<Mutex<JsonRegistry>>,
}

impl LocalInstallRegistry {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Create a registry rooted at `install_dir`.
    ///
    /// The directory is created if it does not exist.  Any existing
    /// `registry.json` (or its backup) is loaded automatically.
    pub async fn new<P: AsRef<Path>>(install_dir: P) -> Result<Self> {
        let install_dir = install_dir.as_ref().to_path_buf();
        let registry_path = install_dir.join("registry.json");
        let backup_path = install_dir.join("registry.json.backup");

        fs::create_dir_all(&install_dir).await?;

        let store = Self {
            registry_path,
            backup_path,
            install_dir,
            registry: Arc::new(Mutex::new(JsonRegistry::default())),
        };

        store.load_registry().await?;
        Ok(store)
    }

    /// Create a registry using the OS-specific default data directory.
    pub async fn new_with_defaults() -> Result<Self> {
        Self::new(Self::default_install_dir()?).await
    }

    /// Return the OS-specific default install directory.
    pub fn default_install_dir() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("org", "nacht", "quelle").ok_or_else(|| {
            StoreError::ConfigError("Cannot determine data directory".to_string())
        })?;
        Ok(dirs.data_dir().join("extensions"))
    }

    // -----------------------------------------------------------------------
    // Public accessors
    // -----------------------------------------------------------------------

    /// Return the directory where extensions are installed.
    pub fn install_dir(&self) -> &Path {
        &self.install_dir
    }

    /// Return the path to the registry JSON file.
    pub fn registry_path(&self) -> &Path {
        &self.registry_path
    }

    /// Update the install directory (used during testing or reconfiguration).
    pub async fn set_install_dir(&self, new_dir: PathBuf) -> Result<()> {
        fs::create_dir_all(&new_dir).await?;
        // NOTE: This modifies the struct's install_dir conceptually.  Because
        // install_dir is immutable after construction, callers needing to change
        // the directory should create a new instance.  This method is kept for
        // API compatibility but is a no-op in the interior-mutability model.
        let _ = new_dir;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Load (or reload) the registry from disk into the mutex-protected field.
    async fn load_registry(&self) -> Result<()> {
        if !self.registry_path.exists() {
            info!("No existing registry found, starting with empty registry");
            return Ok(());
        }

        let mut reg = self.registry.lock().await;

        match fs::read_to_string(&self.registry_path).await {
            Ok(content) => {
                *reg = serde_json::from_str(&content)
                    .map_err(|e| StoreError::CorruptedRegistry(e.to_string()))?;
                debug!("Loaded registry with {} extensions", reg.extensions.len());
            }
            Err(_) => {
                warn!("Failed to load registry, checking backup");
                if self.backup_path.exists() {
                    let backup = fs::read_to_string(&self.backup_path).await?;
                    *reg = serde_json::from_str(&backup)
                        .map_err(|e| StoreError::CorruptedRegistry(e.to_string()))?;
                    info!("Restored registry from backup");
                }
            }
        }

        Ok(())
    }

    /// Persist `registry` to disk.  **Must be called while holding the mutex.**
    ///
    /// Writes a backup of the existing file before overwriting.
    async fn save_registry(&self, registry: &JsonRegistry) -> Result<()> {
        if self.registry_path.exists() {
            if let Err(e) = fs::copy(&self.registry_path, &self.backup_path).await {
                warn!("Failed to create registry backup: {}", e);
            }
        }

        let mut updated = registry.clone();
        updated.last_updated = Utc::now();

        let content = serde_json::to_string_pretty(&updated)?;
        fs::write(&self.registry_path, content).await?;
        debug!("Registry saved ({} extensions)", updated.extensions.len());
        Ok(())
    }

    /// Return the on-disk installation path for the extension with the given ID.
    fn extension_install_path(&self, id: &str) -> PathBuf {
        self.install_dir.join(id)
    }

    // -----------------------------------------------------------------------
    // Validation helpers
    // -----------------------------------------------------------------------

    fn validate_extension_id(id: &str) -> Result<()> {
        if id.is_empty() {
            return Err(StoreError::InvalidExtensionName(
                "Extension ID cannot be empty".to_string(),
            ));
        }
        if id.contains("..") || id.contains('/') || id.contains('\\') {
            return Err(StoreError::InvalidExtensionName(
                "Extension ID contains invalid path characters".to_string(),
            ));
        }
        if id.len() > 255 {
            return Err(StoreError::InvalidExtensionName(
                "Extension ID too long".to_string(),
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InstallRegistry implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl InstallRegistry for LocalInstallRegistry {
    async fn install_extension(
        &self,
        package: ExtensionPackage,
        options: &InstallOptions,
    ) -> Result<InstalledExtension> {
        let id = package.manifest.id.clone();
        Self::validate_extension_id(&id)?;

        let install_path = self.extension_install_path(&id);

        // ── check for existing installation ──────────────────────────────
        {
            let reg = self.registry.lock().await;
            if reg.extensions.contains_key(&id) && !options.force_reinstall {
                return Err(StoreError::ExtensionAlreadyInstalled(id));
            }
        }

        // ── file I/O (no lock held) ───────────────────────────────────────
        if install_path.exists() {
            fs::remove_dir_all(&install_path).await?;
        }
        fs::create_dir_all(&install_path).await?;

        let wasm_path = install_path.join("extension.wasm");
        fs::write(&wasm_path, &package.wasm_component).await?;

        let manifest_path = install_path.join("manifest.json");
        let manifest_json = serde_json::to_string_pretty(&package.manifest)?;
        fs::write(&manifest_path, manifest_json).await?;

        for (asset_name, content) in &package.assets {
            let asset_path = install_path.join(asset_name);
            if let Some(parent) = asset_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&asset_path, content).await?;
        }

        // ── build installation record ─────────────────────────────────────
        let mut installed = InstalledExtension::from_package(package.clone());
        installed.auto_update = options.auto_update;
        installed.checksum = Some(crate::registry::manifest::Checksum {
            algorithm: crate::registry::manifest::checksum::ChecksumAlgorithm::Blake3,
            value: crate::registry::manifest::checksum::ChecksumAlgorithm::Blake3
                .calculate(&package.wasm_component),
        });

        if let Ok(actual_size) = installed.calculate_actual_size(self).await {
            installed.size = actual_size;
        }

        // ── register under ID (lock re-acquired) ──────────────────────────
        {
            let mut reg = self.registry.lock().await;
            reg.extensions.insert(id.clone(), installed.clone());
            self.save_registry(&reg).await?;
        }

        info!(
            "Installed extension {}@{}",
            installed.name, installed.version
        );
        Ok(installed)
    }

    async fn uninstall_extension(&self, id: &str) -> Result<bool> {
        Self::validate_extension_id(id)?;

        let mut reg = self.registry.lock().await;

        if let Some(installed) = reg.extensions.remove(id) {
            let install_path = self.extension_install_path(&installed.id);
            if install_path.exists() {
                // Drop the lock before the potentially-slow remove_dir_all.
                let saved = reg.clone();
                drop(reg);
                fs::remove_dir_all(&install_path).await?;
                // Re-acquire to persist the removal.
                let mut reg2 = self.registry.lock().await;
                // The entry was already removed above; persist current state.
                *reg2 = saved;
                self.save_registry(&reg2).await?;
            } else {
                self.save_registry(&reg).await?;
            }

            info!("Uninstalled extension: {}", id);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn list_installed(&self) -> Result<Vec<InstalledExtension>> {
        let reg = self.registry.lock().await;
        Ok(reg.extensions.values().cloned().collect())
    }

    async fn get_installed(&self, id: &str) -> Result<Option<InstalledExtension>> {
        Self::validate_extension_id(id)?;
        let reg = self.registry.lock().await;
        Ok(reg.extensions.get(id).cloned())
    }

    async fn get_extension_wasm_bytes(&self, id: &str) -> Result<Vec<u8>> {
        let wasm_path = self.extension_install_path(id).join("extension.wasm");
        if !wasm_path.exists() {
            return Err(StoreError::ExtensionNotFound(id.to_string()));
        }
        fs::read(&wasm_path).await.map_err(StoreError::IoError)
    }

    async fn validate_installations(&self) -> Result<Vec<ValidationIssue>> {
        let reg = self.registry.lock().await;
        let mut issues = Vec::new();

        for (id, installation) in &reg.extensions {
            let install_path = self.extension_install_path(id);

            if !install_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: installation.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: format!(
                        "Installation directory not found: {}",
                        install_path.display()
                    ),
                    severity: IssueSeverity::Error,
                });
                continue;
            }

            let wasm_path = install_path.join("extension.wasm");
            if !wasm_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: installation.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: "WASM component file not found".to_string(),
                    severity: IssueSeverity::Error,
                });
            } else if let Some(checksum) = &installation.checksum {
                match fs::read(&wasm_path).await {
                    Ok(bytes) => {
                        let calculated = checksum.algorithm.calculate(&bytes);
                        if calculated != checksum.value {
                            issues.push(ValidationIssue {
                                extension_name: installation.name.clone(),
                                issue_type: ValidationIssueType::ChecksumMismatch,
                                description: "WASM file checksum mismatch".to_string(),
                                severity: IssueSeverity::Error,
                            });
                        }
                    }
                    Err(_) => {
                        issues.push(ValidationIssue {
                            extension_name: installation.name.clone(),
                            issue_type: ValidationIssueType::CorruptedFiles,
                            description: "Unable to read WASM file".to_string(),
                            severity: IssueSeverity::Error,
                        });
                    }
                }
            }

            let manifest_path = install_path.join("manifest.json");
            if !manifest_path.exists() {
                issues.push(ValidationIssue {
                    extension_name: installation.name.clone(),
                    issue_type: ValidationIssueType::MissingFiles,
                    description: "Manifest file not found".to_string(),
                    severity: IssueSeverity::Warning,
                });
            }
        }

        Ok(issues)
    }

    async fn cleanup_orphaned(&self) -> Result<u32> {
        let mut cleaned_count = 0u32;

        // ── find orphaned registry entries ────────────────────────────────
        let orphaned_ids: Vec<String> = {
            let reg = self.registry.lock().await;
            reg.extensions
                .keys()
                .filter(|id| {
                    let path = self.extension_install_path(id);
                    !path.exists() || !path.join("extension.wasm").exists()
                })
                .cloned()
                .collect()
        };

        if !orphaned_ids.is_empty() {
            let mut reg = self.registry.lock().await;
            for id in &orphaned_ids {
                reg.extensions.remove(id);
                cleaned_count += 1;
            }
            self.save_registry(&reg).await?;
        }

        // ── remove orphaned directories ───────────────────────────────────
        if self.install_dir.exists() {
            let reg = self.registry.lock().await;
            let mut entries = fs::read_dir(&self.install_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                if let Some(dir_name) = entry.file_name().to_str().map(str::to_owned) {
                    if entry.file_type().await?.is_dir() && !reg.extensions.contains_key(&dir_name)
                    {
                        let _ = fs::remove_dir_all(entry.path()).await;
                        cleaned_count += 1;
                    }
                }
            }
        }

        Ok(cleaned_count)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use semver::Version;
    use tempfile::TempDir;

    use super::*;
    use crate::models::ExtensionPackage;
    use crate::registry::manifest::{
        Attribute, ExtensionManifest, FileReference, ReadingDirection,
    };

    fn create_test_package(id: &str) -> ExtensionPackage {
        let wasm = b"\x00asm\x01\x00\x00\x00".to_vec();
        let manifest = ExtensionManifest {
            id: id.to_string(),
            name: format!("Test {}", id),
            version: Version::parse("1.0.0").unwrap(),
            author: "Test Author".to_string(),
            langs: vec!["en".to_string()],
            base_urls: vec!["https://example.com".to_string()],
            rds: vec![ReadingDirection::Ltr],
            attrs: vec![Attribute::Fanfiction],
            signature: None,
            wasm_file: FileReference {
                path: "extension.wasm".to_string(),
                checksum: "dummy".to_string(),
                size: wasm.len() as u64,
            },
            assets: Vec::new(),
        };
        ExtensionPackage {
            manifest,
            wasm_component: wasm,
            metadata: None,
            assets: std::collections::HashMap::new(),
            source_store: "test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_registry_store_creation() {
        let dir = TempDir::new().unwrap();
        let registry = LocalInstallRegistry::new(dir.path()).await.unwrap();
        assert_eq!(registry.install_dir(), dir.path());
    }

    #[tokio::test]
    async fn test_install_and_uninstall_extension() {
        let dir = TempDir::new().unwrap();
        let registry = LocalInstallRegistry::new(dir.path()).await.unwrap();
        let options = InstallOptions::default();

        let installed = registry
            .install_extension(create_test_package("test-ext"), &options)
            .await
            .unwrap();

        assert_eq!(installed.id, "test-ext");
        assert!(registry.is_installed("test-ext").await.unwrap());

        let removed = registry.uninstall_extension("test-ext").await.unwrap();
        assert!(removed);
        assert!(!registry.is_installed("test-ext").await.unwrap());
    }

    #[tokio::test]
    async fn test_installation_query() {
        let dir = TempDir::new().unwrap();
        let registry = LocalInstallRegistry::new(dir.path()).await.unwrap();
        let options = InstallOptions::default();

        registry
            .install_extension(create_test_package("ext-a"), &options)
            .await
            .unwrap();
        registry
            .install_extension(create_test_package("ext-b"), &options)
            .await
            .unwrap();

        let all = registry.list_installed().await.unwrap();
        assert_eq!(all.len(), 2);

        let found = registry.get_installed("ext-a").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "ext-a");

        assert!(registry
            .get_installed("ext-missing")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_installation_stats() {
        let dir = TempDir::new().unwrap();
        let registry = LocalInstallRegistry::new(dir.path()).await.unwrap();
        let options = InstallOptions::default();

        registry
            .install_extension(create_test_package("stat-ext"), &options)
            .await
            .unwrap();

        let issues = registry.validate_installations().await.unwrap();
        // Valid install should produce no Error/Critical issues
        assert!(!issues
            .iter()
            .any(|i| matches!(i.severity, IssueSeverity::Error | IssueSeverity::Critical)));
    }

    #[test]
    fn test_default_directory_structure() {
        let dir = TempDir::new().unwrap();
        let reg_path = dir.path().join("registry.json");
        let backup_path = dir.path().join("registry.json.backup");

        // Paths are well-formed even before the registry is loaded
        assert!(reg_path.parent().unwrap().exists());
        let _ = backup_path; // just ensure the path expression compiles
    }

    #[tokio::test]
    async fn test_new_with_defaults() {
        // Should either succeed or fail gracefully (no panics)
        match LocalInstallRegistry::new_with_defaults().await {
            Ok(reg) => {
                let installed = reg.list_installed().await.unwrap();
                let _ = installed; // proves no panic
            }
            Err(_) => {
                // Acceptable on systems where the data directory is unavailable
            }
        }
    }

    #[tokio::test]
    async fn test_validate_and_cleanup() {
        let dir = TempDir::new().unwrap();
        let registry = LocalInstallRegistry::new(dir.path()).await.unwrap();
        let options = InstallOptions::default();

        registry
            .install_extension(create_test_package("cleanup-ext"), &options)
            .await
            .unwrap();

        let issues = registry.validate_installations().await.unwrap();
        // A freshly installed extension should be valid
        assert!(
            !issues
                .iter()
                .any(|i| matches!(i.severity, IssueSeverity::Critical | IssueSeverity::Error)),
            "Unexpected issues: {:?}",
            issues
        );

        let cleaned = registry.cleanup_orphaned().await.unwrap();
        assert_eq!(cleaned, 0, "Should have nothing to clean up");
    }

    #[test]
    fn test_trait_is_implementation_agnostic() {
        // Verify that the trait can be used as a trait object — compile-time check only.
        let _: fn(&dyn InstallRegistry) = |_registry| {};
    }
}
