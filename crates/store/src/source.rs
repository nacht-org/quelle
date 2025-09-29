//! Registry Configuration - Extension source management
//!
//! This module provides configuration structures for managing extension sources,
//! including their types, priorities, and capabilities. Extension sources are
//! configured through the CLI and applied to the StoreManager at runtime.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{Result, StoreError},
    stores::{local::LocalStore, traits::CacheableStore},
    ReadableStore, WritableStore,
};

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
    #[serde(default)]
    pub extension_sources: Vec<ExtensionSource>,
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

    pub fn list_writable_sources(&self) -> Result<Vec<Box<dyn WritableStore>>> {
        self.extension_sources
            .iter()
            .filter(|s| s.enabled)
            .flat_map(|s| s.as_writable().transpose())
            .collect()
    }

    pub fn get_writable_source(&self, name: &str) -> Result<Option<Box<dyn WritableStore>>> {
        if let Some(source) = self
            .extension_sources
            .iter()
            .filter(|s| s.enabled)
            .find(|s| s.name == name)
        {
            source.as_writable()
        } else {
            Ok(None)
        }
    }

    /// Apply configuration to an existing StoreManager
    pub async fn apply(&self, store_manager: &mut crate::StoreManager) -> Result<()> {
        // Add all configured extension sources
        for source in &self.extension_sources {
            if source.enabled {
                match crate::source::create_readable_store_from_source(source).await {
                    Ok(store) => {
                        tracing::info!("Restored store: {} ({})", source.name, source.store_type);
                        let registry_config = crate::registry_config::RegistryStoreConfig::new(
                            source.name.clone(),
                            source.store_type.to_string(),
                        );
                        store_manager
                            .add_boxed_extension_store(store, registry_config)
                            .await?;
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

    pub fn as_readable(&self) -> Result<Box<dyn ReadableStore>> {
        match &self.store_type {
            StoreType::Local { path } => {
                let local_store =
                    LocalStore::new(path).map_err(|e| StoreError::StoreCreationError {
                        store_type: "local".to_string(),
                        source: Box::new(e),
                    })?;
                Ok(Box::new(local_store))
            }
        }
    }

    pub fn as_writable(&self) -> Result<Option<Box<dyn WritableStore>>> {
        match &self.store_type {
            StoreType::Local { path } => {
                let local_store =
                    LocalStore::new(path).map_err(|e| StoreError::StoreCreationError {
                        store_type: "local".to_string(),
                        source: Box::new(e),
                    })?;
                Ok(Some(Box::new(local_store)))
            }
        }
    }

    pub fn as_cacheable(&self) -> Result<Option<Box<dyn CacheableStore>>> {
        match &self.store_type {
            StoreType::Local { path } => {
                let local_store =
                    LocalStore::new(path).map_err(|e| StoreError::StoreCreationError {
                        store_type: "local".to_string(),
                        source: Box::new(e),
                    })?;
                Ok(Some(Box::new(local_store)))
            }
        }
    }
}

/// Helper function to create a store from an ExtensionSource configuration
pub async fn create_readable_store_from_source(
    source: &ExtensionSource,
) -> Result<Box<dyn ReadableStore>> {
    match &source.store_type {
        StoreType::Local { path } => {
            let local_store =
                LocalStore::new(path).map_err(|e| StoreError::StoreCreationError {
                    store_type: "local".to_string(),
                    source: Box::new(e),
                })?;

            Ok(Box::new(local_store))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(config.extension_sources.len(), 3);

        // Should be sorted by priority (lower = higher priority)
        assert_eq!(config.extension_sources[0].name, "store2"); // priority 50
        assert_eq!(config.extension_sources[1].name, "store3"); // priority 75
        assert_eq!(config.extension_sources[2].name, "store1"); // priority 100
    }
}
