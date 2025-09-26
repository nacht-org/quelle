//! Config Store - Persistence for registry configuration
//!
//! This module provides traits and implementations for persisting registry configuration,
//! including extension store preferences. When stores are added via CLI, their
//! configuration is saved so they can be restored on the next startup.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::error::{Result, StoreError};

/// Type of extension store with associated data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum StoreType {
    /// Local file system store
    Local { path: PathBuf },
}

impl StoreType {
    /// Get the string representation of the store type
    pub fn as_str(&self) -> &'static str {
        match self {
            StoreType::Local { .. } => "local",
        }
    }

    /// Get the path for Local store type
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            StoreType::Local { path } => Some(path),
        }
    }
}

impl std::fmt::Display for StoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Registry configuration containing extension sources and other settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub extension_sources: Vec<ExtensionSource>,
    // Future: add other registry settings here
    // pub default_timeout: Duration,
    // pub parallel_downloads: usize,
    // pub auto_update: bool,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            extension_sources: Vec::new(),
        }
    }
}

impl RegistryConfig {
    pub fn add_source(&mut self, source: ExtensionSource) {
        // Remove existing source with same name
        self.extension_sources.retain(|s| s.name != source.name);

        // Add the new source
        self.extension_sources.push(source);

        // Sort by priority (lower numbers = higher priority)
        self.extension_sources.sort_by_key(|s| s.priority);
    }

    pub fn remove_source(&mut self, name: &str) -> bool {
        let initial_len = self.extension_sources.len();
        self.extension_sources.retain(|s| s.name != name);
        initial_len != self.extension_sources.len()
    }

    pub fn has_source(&self, name: &str) -> bool {
        self.extension_sources.iter().any(|s| s.name == name)
    }

    /// Apply configuration to an existing StoreManager
    pub async fn apply(&self, store_manager: &mut crate::StoreManager) -> Result<()> {
        // Add all configured extension sources
        for source in &self.extension_sources {
            if source.enabled {
                match crate::source::create_store_from_source(source).await {
                    Ok(store) => {
                        tracing::info!("Restored store: {} ({})", source.name, source.store_type);
                        store_manager.add_boxed_extension_store(store);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to restore store '{}': {}", source.name, e);
                    }
                }
            }
        }

        Ok(())
    }
}

/// Trait for persisting registry configuration
#[async_trait::async_trait]
pub trait ConfigStore: Send + Sync {
    /// Load registry configuration
    async fn load(&self) -> Result<RegistryConfig>;

    /// Save registry configuration
    async fn save(&self, config: &RegistryConfig) -> Result<()>;
}

/// Configuration for an extension source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSource {
    pub name: String,
    pub store_type: StoreType,
    pub enabled: bool,
    pub priority: u32,
    pub trusted: bool,
    pub added_at: DateTime<Utc>,
}

impl ExtensionSource {
    pub fn new(name: String, store_type: StoreType) -> Self {
        Self {
            name,
            store_type,
            enabled: true,
            priority: 100,
            trusted: false,
            added_at: Utc::now(),
        }
    }

    pub fn local(name: String, path: PathBuf) -> Self {
        Self {
            name,
            store_type: StoreType::Local { path },
            enabled: true,
            priority: 100,
            trusted: false,
            added_at: Utc::now(),
        }
    }

    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    pub fn trusted(mut self) -> Self {
        self.trusted = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn get_path(&self) -> Option<&PathBuf> {
        self.store_type.path()
    }
}

/// Local file-based implementation of ConfigStore
pub struct LocalConfigStore {
    config_file: PathBuf,
}

impl LocalConfigStore {
    /// Create a new LocalConfigStore with a custom config file path
    pub async fn new<P: Into<PathBuf>>(config_file: P) -> Result<Self> {
        let config_file = config_file.into();

        // Ensure the parent directory exists
        if let Some(parent) = config_file.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| StoreError::IoOperation {
                    operation: "create config directory".to_string(),
                    path: parent.to_path_buf(),
                    source: e,
                })?;
        }

        Ok(Self { config_file })
    }

    /// Create a new LocalConfigStore using OS-specific default directories
    pub async fn new_with_defaults() -> Result<Self> {
        let config_dir = get_default_config_dir()?;
        let config_file = config_dir.join("config.json");
        Self::new(config_file).await
    }

    /// Get the path to the config file
    pub fn config_file_path(&self) -> &PathBuf {
        &self.config_file
    }
}

#[async_trait::async_trait]
impl ConfigStore for LocalConfigStore {
    async fn load(&self) -> Result<RegistryConfig> {
        if !self.config_file.exists() {
            return Ok(RegistryConfig::default());
        }

        let contents =
            fs::read_to_string(&self.config_file)
                .await
                .map_err(|e| StoreError::IoOperation {
                    operation: "read registry config".to_string(),
                    path: self.config_file.clone(),
                    source: e,
                })?;

        let config: RegistryConfig = serde_json::from_str(&contents).map_err(|e| {
            StoreError::SerializationErrorWithContext {
                operation: "deserialize registry config".to_string(),
                source: e,
            }
        })?;

        Ok(config)
    }

    async fn save(&self, config: &RegistryConfig) -> Result<()> {
        let contents = serde_json::to_string_pretty(config).map_err(|e| {
            StoreError::SerializationErrorWithContext {
                operation: "serialize registry config".to_string(),
                source: e,
            }
        })?;

        fs::write(&self.config_file, contents)
            .await
            .map_err(|e| StoreError::IoOperation {
                operation: "write registry config".to_string(),
                path: self.config_file.clone(),
                source: e,
            })?;

        Ok(())
    }
}

/// Get the default configuration directory for the current OS
fn get_default_config_dir() -> Result<PathBuf> {
    use directories::ProjectDirs;

    if let Some(proj_dirs) = ProjectDirs::from("", "", "quelle") {
        Ok(proj_dirs.config_dir().to_path_buf())
    } else {
        Err(StoreError::ConfigurationError {
            message: "Could not determine configuration directory".to_string(),
        })
    }
}

/// Helper function to create a store from an ExtensionSource configuration
pub async fn create_store_from_source(
    source: &ExtensionSource,
) -> Result<Box<dyn crate::store::Store>> {
    match &source.store_type {
        StoreType::Local { path } => {
            let local_store = crate::local::LocalStore::new(path)
                .map_err(|e| StoreError::StoreCreationError {
                    store_type: "local".to_string(),
                    source: Box::new(e),
                })?
                .with_name(source.name.clone());

            Ok(Box::new(local_store))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_local_config_store_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config.json");

        let config_store = LocalConfigStore::new(config_file).await.unwrap();

        // Initially empty config
        let config = config_store.load().await.unwrap();
        assert_eq!(config.extension_sources.len(), 0);

        // Add a source
        let mut config = config_store.load().await.unwrap();
        let source = ExtensionSource::local("test-store".to_string(), PathBuf::from("/test/path"));
        config.add_source(source);
        config_store.save(&config).await.unwrap();

        // Verify it was saved
        let config = config_store.load().await.unwrap();
        assert_eq!(config.extension_sources.len(), 1);
        assert_eq!(config.extension_sources[0].name, "test-store");
        assert_eq!(
            config.extension_sources[0].store_type,
            StoreType::Local {
                path: PathBuf::from("/test/path")
            }
        );

        // Check if source exists
        assert!(config.has_source("test-store"));
        assert!(!config.has_source("non-existent"));

        // Remove the source
        let mut config = config_store.load().await.unwrap();
        let removed = config.remove_source("test-store");
        assert!(removed);
        config_store.save(&config).await.unwrap();

        // Verify it was removed
        let config = config_store.load().await.unwrap();
        assert_eq!(config.extension_sources.len(), 0);

        // Try removing non-existent source
        let mut config = config_store.load().await.unwrap();
        let removed = config.remove_source("non-existent");
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_extension_source_creation() {
        let source = ExtensionSource::local("my-store".to_string(), PathBuf::from("/some/path"))
            .with_priority(50)
            .trusted();

        assert_eq!(source.name, "my-store");
        assert_eq!(
            source.store_type,
            StoreType::Local {
                path: PathBuf::from("/some/path")
            }
        );
        assert_eq!(source.priority, 50);
        assert!(source.trusted);
        assert!(source.enabled);
        assert_eq!(source.get_path(), Some(&PathBuf::from("/some/path")));
    }

    #[tokio::test]
    async fn test_source_priority_sorting() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_config.json");
        let config_store = LocalConfigStore::new(config_file).await.unwrap();

        let mut config = RegistryConfig::default();

        // Add sources with different priorities
        let source1 = ExtensionSource::local("store1".to_string(), PathBuf::from("/path1"))
            .with_priority(100);
        let source2 =
            ExtensionSource::local("store2".to_string(), PathBuf::from("/path2")).with_priority(50);
        let source3 =
            ExtensionSource::local("store3".to_string(), PathBuf::from("/path3")).with_priority(75);

        config.add_source(source1);
        config.add_source(source2);
        config.add_source(source3);

        config_store.save(&config).await.unwrap();
        let loaded_config = config_store.load().await.unwrap();

        assert_eq!(loaded_config.extension_sources.len(), 3);

        // Should be sorted by priority (lower = higher priority)
        assert_eq!(loaded_config.extension_sources[0].name, "store2"); // priority 50
        assert_eq!(loaded_config.extension_sources[1].name, "store3"); // priority 75
        assert_eq!(loaded_config.extension_sources[2].name, "store1"); // priority 100
    }
}
