//! Registry Configuration - Extension source management
//!
//! This module provides configuration structures for managing extension sources,
//! including their types, priorities, and capabilities. Extension sources are
//! configured through the CLI and applied to the StoreManager at runtime.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    error::{Result, StoreError},
    stores::{local::LocalStore, traits::CacheableStore, ReadableStore, WritableStore},
};

#[cfg(feature = "git")]
use crate::stores::git::GitStore;
#[cfg(feature = "git")]
use crate::stores::providers::git::{GitAuth, GitReference};

#[cfg(feature = "github")]
use crate::stores::github::GitHubStore;

/// Type of extension store with associated data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum StoreType {
    /// Local file system store
    Local { path: PathBuf },
    /// Git repository store
    #[cfg(feature = "git")]
    Git {
        url: String,
        cache_dir: PathBuf,
        #[serde(default)]
        reference: GitReference,
        #[serde(default)]
        auth: GitAuth,
    },
    /// GitHub repository store (API-based reads, git-based writes)
    #[cfg(feature = "github")]
    GitHub {
        owner: String,
        repo: String,
        cache_dir: PathBuf,
        #[serde(default)]
        reference: GitReference,
        #[serde(default)]
        auth: GitAuth,
    },
}

impl StoreType {
    /// Get the string representation of the store type
    pub fn as_str(&self) -> &'static str {
        match self {
            StoreType::Local { .. } => "local",
            #[cfg(feature = "git")]
            StoreType::Git { .. } => "git",
            #[cfg(feature = "github")]
            StoreType::GitHub { .. } => "github",
        }
    }

    /// Get the path for Local store type or cache_dir for Git/GitHub store type
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            StoreType::Local { path } => Some(path),
            #[cfg(feature = "git")]
            StoreType::Git { cache_dir, .. } => Some(cache_dir),
            #[cfg(feature = "github")]
            StoreType::GitHub { cache_dir, .. } => Some(cache_dir),
        }
    }

    /// Get the URL for Git store type
    #[cfg(feature = "git")]
    pub fn url(&self) -> Option<&str> {
        match self {
            StoreType::Local { .. } => None,
            StoreType::Git { url, .. } => Some(url),
            #[cfg(feature = "github")]
            StoreType::GitHub { .. } => None, // GitHub stores use owner/repo instead
        }
    }

    /// Get the GitHub repository info (owner, repo)
    #[cfg(feature = "github")]
    pub fn github_repo(&self) -> Option<(&str, &str)> {
        match self {
            StoreType::Local { .. } => None,
            #[cfg(feature = "git")]
            StoreType::Git { .. } => None,
            StoreType::GitHub { owner, repo, .. } => Some((owner, repo)),
        }
    }
}

impl std::fmt::Display for StoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Registry configuration containing extension sources and other settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryConfig {
    #[serde(default)]
    pub extension_sources: Vec<ExtensionSource>,
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
                match super::source::create_readable_store_from_source(source).await {
                    Ok(store) => {
                        tracing::info!("Restored store: {} ({})", source.name, source.store_type);
                        let registry_config = super::registry_config::RegistryStoreConfig::new(
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

    #[cfg(feature = "git")]
    pub fn git(name: String, url: String, cache_dir: PathBuf) -> Self {
        Self {
            name,
            store_type: StoreType::Git {
                url,
                cache_dir,
                reference: GitReference::Default,
                auth: GitAuth::None,
            },
            enabled: true,
            priority: 100,
            trusted: false,
            added_at: Utc::now(),
        }
    }

    #[cfg(feature = "git")]
    pub fn git_with_config(
        name: String,
        url: String,
        cache_dir: PathBuf,
        reference: GitReference,
        auth: GitAuth,
    ) -> Self {
        Self {
            name,
            store_type: StoreType::Git {
                url,
                cache_dir,
                reference,
                auth,
            },
            enabled: true,
            priority: 100,
            trusted: false,
            added_at: Utc::now(),
        }
    }

    #[cfg(feature = "github")]
    pub fn github(name: String, owner: String, repo: String, cache_dir: PathBuf) -> Self {
        Self {
            name,
            store_type: StoreType::GitHub {
                owner,
                repo,
                cache_dir,
                reference: GitReference::Default,
                auth: GitAuth::None,
            },
            enabled: true,
            priority: 100,
            trusted: false,
            added_at: Utc::now(),
        }
    }

    #[cfg(feature = "github")]
    pub fn github_with_config(
        name: String,
        owner: String,
        repo: String,
        cache_dir: PathBuf,
        reference: GitReference,
        auth: GitAuth,
    ) -> Self {
        Self {
            name,
            store_type: StoreType::GitHub {
                owner,
                repo,
                cache_dir,
                reference,
                auth,
            },
            enabled: true,
            priority: 100,
            trusted: false,
            added_at: Utc::now(),
        }
    }

    #[cfg(feature = "git")]
    pub fn official(stores_dir: &Path) -> Self {
        ExtensionSource::git(
            "official".to_string(),
            "https://github.com/nacht-org/extensions".to_string(),
            stores_dir.join("official"),
        )
        .with_priority(50) // Higher priority than default (100)
        .trusted() // Mark as trusted since it's official
    }

    #[cfg(feature = "github")]
    pub fn official_github(stores_dir: &Path) -> Self {
        ExtensionSource::github(
            "official".to_string(),
            "nacht-org".to_string(),
            "extensions".to_string(),
            stores_dir.join("official"),
        )
        .with_priority(50) // Higher priority than default (100)
        .trusted() // Mark as trusted since it's official
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
            #[cfg(feature = "git")]
            StoreType::Git {
                url,
                cache_dir,
                reference,
                auth,
            } => {
                let git_store = GitStore::builder(url.clone())
                    .auth(auth.clone())
                    .reference(reference.clone())
                    .fetch_interval(std::time::Duration::from_secs(300))
                    .shallow(true)
                    .cache_dir(cache_dir.clone())
                    .name(self.name.clone())
                    .build()
                    .map_err(|e| StoreError::StoreCreationError {
                        store_type: "git".to_string(),
                        source: Box::new(e),
                    })?;
                Ok(Box::new(git_store))
            }
            #[cfg(feature = "github")]
            StoreType::GitHub {
                owner,
                repo,
                cache_dir,
                reference,
                auth,
            } => {
                let github_store = GitHubStore::builder(owner.clone(), repo.clone())
                    .auth(auth.clone())
                    .reference(reference.clone())
                    .cache_dir(cache_dir.clone())
                    .name(self.name.clone())
                    .build()
                    .map_err(|e| StoreError::StoreCreationError {
                        store_type: "github".to_string(),
                        source: Box::new(e),
                    })?;
                Ok(Box::new(github_store))
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
            #[cfg(feature = "git")]
            StoreType::Git {
                url,
                cache_dir,
                reference,
                auth,
            } => {
                let git_store = GitStore::builder(url.clone())
                    .auth(auth.clone())
                    .reference(reference.clone())
                    .fetch_interval(std::time::Duration::from_secs(300))
                    .shallow(true)
                    .writable()
                    .cache_dir(cache_dir.clone())
                    .name(self.name.clone())
                    .build()
                    .map_err(|e| StoreError::StoreCreationError {
                        store_type: "git".to_string(),
                        source: Box::new(e),
                    })?;
                // Git stores can be writable if properly configured
                Ok(Some(Box::new(git_store)))
            }
            #[cfg(feature = "github")]
            StoreType::GitHub {
                owner,
                repo,
                cache_dir,
                reference,
                auth,
            } => {
                let github_store = GitHubStore::builder(owner.clone(), repo.clone())
                    .auth(auth.clone())
                    .reference(reference.clone())
                    .cache_dir(cache_dir.clone())
                    .name(self.name.clone())
                    .writable()
                    .build()
                    .map_err(|e| StoreError::StoreCreationError {
                        store_type: "github".to_string(),
                        source: Box::new(e),
                    })?;
                // GitHub stores can be writable for publishing operations
                Ok(Some(Box::new(github_store)))
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
            #[cfg(feature = "git")]
            StoreType::Git {
                url,
                cache_dir,
                reference,
                auth,
            } => {
                let git_store = GitStore::builder(url.clone())
                    .auth(auth.clone())
                    .reference(reference.clone())
                    .fetch_interval(std::time::Duration::from_secs(300))
                    .shallow(true)
                    .cache_dir(cache_dir.clone())
                    .name(self.name.clone())
                    .build()
                    .map_err(|e| StoreError::StoreCreationError {
                        store_type: "git".to_string(),
                        source: Box::new(e),
                    })?;
                Ok(Some(Box::new(git_store)))
            }
            #[cfg(feature = "github")]
            StoreType::GitHub {
                owner,
                repo,
                cache_dir,
                reference,
                auth,
            } => {
                let github_store = GitHubStore::builder(owner.clone(), repo.clone())
                    .auth(auth.clone())
                    .reference(reference.clone())
                    .cache_dir(cache_dir.clone())
                    .name(self.name.clone())
                    .build()
                    .map_err(|e| StoreError::StoreCreationError {
                        store_type: "github".to_string(),
                        source: Box::new(e),
                    })?;
                Ok(Some(Box::new(github_store)))
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
        #[cfg(feature = "git")]
        StoreType::Git {
            url,
            cache_dir,
            reference,
            auth,
        } => {
            let git_store = GitStore::builder(url.clone())
                .auth(auth.clone())
                .reference(reference.clone())
                .fetch_interval(std::time::Duration::from_secs(300))
                .shallow(true)
                .cache_dir(cache_dir.clone())
                .name(source.name.clone())
                .build()
                .map_err(|e| StoreError::StoreCreationError {
                    store_type: "git".to_string(),
                    source: Box::new(e),
                })?;

            Ok(Box::new(git_store))
        }
        #[cfg(feature = "github")]
        StoreType::GitHub {
            owner,
            repo,
            cache_dir,
            reference,
            auth,
        } => {
            let github_store = GitHubStore::builder(owner.clone(), repo.clone())
                .auth(auth.clone())
                .reference(reference.clone())
                .cache_dir(cache_dir.clone())
                .name(source.name.clone())
                .build()
                .map_err(|e| StoreError::StoreCreationError {
                    store_type: "github".to_string(),
                    source: Box::new(e),
                })?;

            Ok(Box::new(github_store))
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
