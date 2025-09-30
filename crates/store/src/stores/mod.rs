//! Store implementations and factory for the Quelle extension store system
//!
//! This module provides a unified interface for creating and managing different types
//! of extension stores (local, git, http, etc.) with proper separation of concerns
//! and extensible patterns.

pub mod traits;

// Store implementations
pub mod local;
pub mod locally_cached;
pub mod providers;

#[cfg(feature = "git")]
pub mod git;

// Future store implementations
// pub mod http;
// pub mod s3;

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{Result, StoreError};

// Re-export commonly used traits
pub use traits::{
    AuthCredentials, AuthStatus, AuthenticatedStore, BaseStore, CacheStats, CacheableStore,
    ReadWriteStore, ReadableStore, RefInfo, RefType, VersionedStore, WritableStore,
};

// Re-export provider types and locally cached store
pub use locally_cached::LocallyCachedStore;
pub use providers::{StoreProvider, SyncResult};

#[cfg(feature = "git")]
pub use providers::{GitAuth, GitProvider, GitReference};

#[cfg(feature = "git")]
pub use git::GitStore;

/// Configuration for creating different types of stores
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StoreConfig {
    /// Local filesystem store
    Local {
        path: PathBuf,
        name: String,
        trusted: bool,
        #[serde(default)]
        cache_enabled: bool,
        #[serde(default)]
        readonly: bool,
    },
    /// Git repository store (future implementation)
    Git {
        url: String,
        name: String,
        trusted: bool,
        branch: Option<String>,
        #[serde(default)]
        cache_enabled: bool,
        auth: Option<GitAuthConfig>,
    },
    /// HTTP-based store (future implementation)
    Http {
        base_url: String,
        name: String,
        trusted: bool,
        #[serde(default)]
        cache_enabled: bool,
        auth: Option<HttpAuthConfig>,
        #[serde(default)]
        readonly: bool,
    },
}

/// Git authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GitAuthConfig {
    SshKey {
        private_key_path: PathBuf,
        passphrase: Option<String>,
    },
    Token {
        token: String,
    },
    UserPassword {
        username: String,
        password: String,
    },
}

/// HTTP authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HttpAuthConfig {
    Bearer { token: String },
    Basic { username: String, password: String },
    ApiKey { key: String, header: Option<String> },
}

/// Store factory for creating different types of stores
pub struct StoreFactory {
    /// Registry of store creation functions
    creators: HashMap<String, Box<dyn StoreCreator>>,
}

/// Trait for store creators that can be registered with the factory
#[async_trait]
pub trait StoreCreator: Send + Sync {
    /// Create a store from the given configuration
    async fn create(&self, config: StoreConfig) -> Result<Box<dyn BaseStore>>;

    /// Get the store type this creator handles
    fn store_type(&self) -> &'static str;

    /// Validate the configuration before creation
    fn validate_config(&self, config: &StoreConfig) -> Result<()>;
}

impl StoreFactory {
    /// Create a new store factory with default creators
    pub fn new() -> Self {
        let mut factory = Self {
            creators: HashMap::new(),
        };

        // Register built-in store creators
        factory.register_creator(Box::new(LocalStoreCreator));

        // Future creators will be registered here:
        // factory.register_creator(Box::new(GitStoreCreator));
        // factory.register_creator(Box::new(HttpStoreCreator));

        factory
    }

    /// Register a new store creator
    pub fn register_creator(&mut self, creator: Box<dyn StoreCreator>) {
        let store_type = creator.store_type().to_string();
        self.creators.insert(store_type, creator);
    }

    /// Create a store from configuration
    pub async fn create_store(&self, config: StoreConfig) -> Result<Box<dyn BaseStore>> {
        let store_type = match &config {
            StoreConfig::Local { .. } => "local",
            StoreConfig::Git { .. } => "git",
            StoreConfig::Http { .. } => "http",
        };

        let creator = self
            .creators
            .get(store_type)
            .ok_or_else(|| StoreError::UnsupportedStoreType(store_type.to_string()))?;

        creator.validate_config(&config)?;
        creator.create(config).await
    }

    /// List all supported store types
    pub fn supported_types(&self) -> Vec<&str> {
        self.creators.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for StoreFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Store creator for local filesystem stores
struct LocalStoreCreator;

#[async_trait]
impl StoreCreator for LocalStoreCreator {
    async fn create(&self, config: StoreConfig) -> Result<Box<dyn BaseStore>> {
        match config {
            StoreConfig::Local {
                path,
                name: _,
                trusted: _,
                cache_enabled,
                readonly,
            } => {
                let mut store = local::LocalStore::new(&path)?;

                // Configure the store based on options
                if !cache_enabled {
                    store = store.with_cache_disabled();
                }

                if readonly {
                    store = store.with_readonly(true);
                }

                Ok(Box::new(store))
            }
            _ => Err(StoreError::InvalidConfiguration(
                "Expected Local store configuration".to_string(),
            )),
        }
    }

    fn store_type(&self) -> &'static str {
        "local"
    }

    fn validate_config(&self, config: &StoreConfig) -> Result<()> {
        match config {
            StoreConfig::Local { path, name, .. } => {
                if name.is_empty() {
                    return Err(StoreError::InvalidConfiguration(
                        "Store name cannot be empty".to_string(),
                    ));
                }

                if !path.exists() {
                    return Err(StoreError::InvalidConfiguration(format!(
                        "Store path does not exist: {}",
                        path.display()
                    )));
                }

                if !path.is_dir() {
                    return Err(StoreError::InvalidConfiguration(format!(
                        "Store path is not a directory: {}",
                        path.display()
                    )));
                }

                Ok(())
            }
            _ => Err(StoreError::InvalidConfiguration(
                "Expected Local store configuration".to_string(),
            )),
        }
    }
}

/// Helper functions for creating stores with common configurations
impl StoreConfig {
    /// Create a local store configuration
    pub fn local<P: Into<PathBuf>>(path: P, name: String) -> Self {
        Self::Local {
            path: path.into(),
            name,
            trusted: false,
            cache_enabled: true,
            readonly: false,
        }
    }

    /// Create a trusted local store configuration
    pub fn local_trusted<P: Into<PathBuf>>(path: P, name: String) -> Self {
        Self::Local {
            path: path.into(),
            name,
            trusted: true,
            cache_enabled: true,
            readonly: false,
        }
    }

    /// Create a readonly local store configuration
    pub fn local_readonly<P: Into<PathBuf>>(path: P, name: String) -> Self {
        Self::Local {
            path: path.into(),
            name,
            trusted: false,
            cache_enabled: true,
            readonly: true,
        }
    }

    /// Create a git store configuration (future implementation)
    pub fn git(url: String, name: String) -> Self {
        Self::Git {
            url,
            name,
            trusted: false,
            branch: None,
            cache_enabled: true,
            auth: None,
        }
    }

    /// Create an HTTP store configuration (future implementation)
    pub fn http(base_url: String, name: String) -> Self {
        Self::Http {
            base_url,
            name,
            trusted: false,
            cache_enabled: true,
            auth: None,
            readonly: true,
        }
    }
}

/// Convenience functions for creating stores
pub async fn create_local_store<P: Into<PathBuf>>(
    path: P,
    name: String,
) -> Result<Box<dyn BaseStore>> {
    let factory = StoreFactory::new();
    let config = StoreConfig::local(path, name);
    factory.create_store(config).await
}

pub async fn create_local_store_trusted<P: Into<PathBuf>>(
    path: P,
    name: String,
) -> Result<Box<dyn BaseStore>> {
    let factory = StoreFactory::new();
    let config = StoreConfig::local_trusted(path, name);
    factory.create_store(config).await
}

pub async fn create_local_store_readonly<P: Into<PathBuf>>(
    path: P,
    name: String,
) -> Result<Box<dyn BaseStore>> {
    let factory = StoreFactory::new();
    let config = StoreConfig::local_readonly(path, name);
    factory.create_store(config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_store_factory_creation() {
        let factory = StoreFactory::new();
        assert!(factory.supported_types().contains(&"local"));
    }

    #[tokio::test]
    async fn test_local_store_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = StoreConfig::local(temp_dir.path(), "test-store".to_string());

        let factory = StoreFactory::new();
        let store = factory.create_store(config).await.unwrap();

        let manifest = store.get_store_manifest().await.unwrap();

        assert_eq!(manifest.store_type, "local");
        assert_eq!(manifest.name, "test-store");
    }

    #[tokio::test]
    async fn test_invalid_store_type() {
        let factory = StoreFactory::new();
        let config = StoreConfig::Git {
            url: "https://github.com/test/repo.git".to_string(),
            name: "test-git".to_string(),
            trusted: false,
            branch: None,
            cache_enabled: true,
            auth: None,
        };

        // Git stores are not implemented yet, should fail
        let result = factory.create_store(config).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_config_helpers() {
        let config = StoreConfig::local("/tmp/test", "test".to_string());
        match config {
            StoreConfig::Local {
                path,
                name,
                trusted,
                ..
            } => {
                assert_eq!(path, PathBuf::from("/tmp/test"));
                assert_eq!(name, "test");
                assert!(!trusted);
            }
            _ => panic!("Expected Local config"),
        }
    }
}
