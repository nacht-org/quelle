//! Source Store - Persistence for extension store configurations
//!
//! This module provides traits and implementations for persisting the configuration
//! of extension stores. When stores are added via CLI, their configuration is saved
//! so they can be restored on the next startup.

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

/// Trait for persisting extension store configurations
#[async_trait::async_trait]
pub trait SourceStore: Send + Sync {
    /// Load all saved extension source configurations
    async fn load_sources(&self) -> Result<Vec<ExtensionSource>>;

    /// Save all extension source configurations
    async fn save_sources(&self, sources: &[ExtensionSource]) -> Result<()>;

    /// Add a new extension source configuration
    async fn add_source(&self, source: &ExtensionSource) -> Result<()>;

    /// Remove an extension source configuration by name
    async fn remove_source(&self, name: &str) -> Result<bool>;

    /// Check if a source with the given name exists
    async fn has_source(&self, name: &str) -> Result<bool>;
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

/// Local file-based implementation of SourceStore
pub struct LocalSourceStore {
    config_file: PathBuf,
}

impl LocalSourceStore {
    /// Create a new LocalSourceStore with a custom config file path
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

    /// Create a new LocalSourceStore using OS-specific default directories
    pub async fn new_with_defaults() -> Result<Self> {
        let config_dir = get_default_config_dir()?;
        let config_file = config_dir.join("sources.json");
        Self::new(config_file).await
    }

    /// Get the path to the config file
    pub fn config_file_path(&self) -> &PathBuf {
        &self.config_file
    }
}

#[async_trait::async_trait]
impl SourceStore for LocalSourceStore {
    async fn load_sources(&self) -> Result<Vec<ExtensionSource>> {
        if !self.config_file.exists() {
            return Ok(Vec::new());
        }

        let contents =
            fs::read_to_string(&self.config_file)
                .await
                .map_err(|e| StoreError::IoOperation {
                    operation: "read sources config".to_string(),
                    path: self.config_file.clone(),
                    source: e,
                })?;

        let sources: Vec<ExtensionSource> = serde_json::from_str(&contents).map_err(|e| {
            StoreError::SerializationErrorWithContext {
                operation: "deserialize sources config".to_string(),
                source: e,
            }
        })?;

        Ok(sources)
    }

    async fn save_sources(&self, sources: &[ExtensionSource]) -> Result<()> {
        let contents = serde_json::to_string_pretty(sources).map_err(|e| {
            StoreError::SerializationErrorWithContext {
                operation: "serialize sources config".to_string(),
                source: e,
            }
        })?;

        fs::write(&self.config_file, contents)
            .await
            .map_err(|e| StoreError::IoOperation {
                operation: "write sources config".to_string(),
                path: self.config_file.clone(),
                source: e,
            })?;

        Ok(())
    }

    async fn add_source(&self, source: &ExtensionSource) -> Result<()> {
        let mut sources = self.load_sources().await?;

        // Remove existing source with same name
        sources.retain(|s| s.name != source.name);

        // Add the new source
        sources.push(source.clone());

        // Sort by priority (lower numbers = higher priority)
        sources.sort_by_key(|s| s.priority);

        self.save_sources(&sources).await
    }

    async fn remove_source(&self, name: &str) -> Result<bool> {
        let mut sources = self.load_sources().await?;
        let initial_len = sources.len();

        sources.retain(|s| s.name != name);
        let removed = sources.len() != initial_len;

        if removed {
            self.save_sources(&sources).await?;
        }

        Ok(removed)
    }

    async fn has_source(&self, name: &str) -> Result<bool> {
        let sources = self.load_sources().await?;
        Ok(sources.iter().any(|s| s.name == name))
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
    async fn test_local_source_store_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("test_sources.json");

        let source_store = LocalSourceStore::new(config_file).await.unwrap();

        // Initially no sources
        let sources = source_store.load_sources().await.unwrap();
        assert_eq!(sources.len(), 0);

        // Add a source
        let source = ExtensionSource::local("test-store".to_string(), PathBuf::from("/test/path"));
        source_store.add_source(&source).await.unwrap();

        // Verify it was saved
        let sources = source_store.load_sources().await.unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "test-store");
        assert_eq!(
            sources[0].store_type,
            StoreType::Local {
                path: PathBuf::from("/test/path")
            }
        );

        // Check if source exists
        assert!(source_store.has_source("test-store").await.unwrap());
        assert!(!source_store.has_source("non-existent").await.unwrap());

        // Remove the source
        let removed = source_store.remove_source("test-store").await.unwrap();
        assert!(removed);

        // Verify it was removed
        let sources = source_store.load_sources().await.unwrap();
        assert_eq!(sources.len(), 0);

        // Try removing non-existent source
        let removed = source_store.remove_source("non-existent").await.unwrap();
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
        let config_file = temp_dir.path().join("test_sources.json");
        let source_store = LocalSourceStore::new(config_file).await.unwrap();

        // Add sources with different priorities
        let source1 = ExtensionSource::local("store1".to_string(), PathBuf::from("/path1"))
            .with_priority(100);
        let source2 =
            ExtensionSource::local("store2".to_string(), PathBuf::from("/path2")).with_priority(50);
        let source3 =
            ExtensionSource::local("store3".to_string(), PathBuf::from("/path3")).with_priority(75);

        source_store.add_source(&source1).await.unwrap();
        source_store.add_source(&source2).await.unwrap();
        source_store.add_source(&source3).await.unwrap();

        let sources = source_store.load_sources().await.unwrap();
        assert_eq!(sources.len(), 3);

        // Should be sorted by priority (lower = higher priority)
        assert_eq!(sources[0].name, "store2"); // priority 50
        assert_eq!(sources[1].name, "store3"); // priority 75
        assert_eq!(sources[2].name, "store1"); // priority 100
    }
}
