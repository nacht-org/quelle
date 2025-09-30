//! Store providers for syncing data from various sources to local storage
//!
//! This module provides the StoreProvider trait and related types for managing
//! how data is synchronized from different sources (git repos, HTTP endpoints, etc.)
//! into local storage that can be read by LocalStore.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::Path;

use crate::error::Result;

/// Result of a sync operation
#[derive(Debug, Clone)]
pub struct SyncResult {
    /// Whether any changes were made during sync
    pub updated: bool,
    /// List of changes made (files added/updated/removed)
    pub changes: Vec<String>,
    /// Any non-fatal warnings that occurred
    pub warnings: Vec<String>,
    /// Time when sync completed
    pub completed_at: DateTime<Utc>,
    /// Bytes transferred during sync (if applicable)
    pub bytes_transferred: Option<u64>,
}

impl SyncResult {
    /// Create a new SyncResult indicating no changes
    pub fn no_changes() -> Self {
        Self {
            updated: false,
            changes: Vec::new(),
            warnings: Vec::new(),
            completed_at: Utc::now(),
            bytes_transferred: None,
        }
    }

    /// Create a new SyncResult indicating changes were made
    pub fn with_changes(changes: Vec<String>) -> Self {
        Self {
            updated: true,
            changes,
            warnings: Vec::new(),
            completed_at: Utc::now(),
            bytes_transferred: None,
        }
    }

    /// Add a warning to the result
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }

    /// Set bytes transferred
    pub fn with_bytes_transferred(mut self, bytes: u64) -> Self {
        self.bytes_transferred = Some(bytes);
        self
    }
}

/// Trait for providers that can sync data from various sources to local storage
#[async_trait]
pub trait StoreProvider: Send + Sync {
    /// Sync/update the local store from the source to the given directory
    /// This should handle initial setup (clone/download) as well as updates
    async fn sync(&self, sync_dir: &Path) -> Result<SyncResult>;

    /// Check if sync is needed (based on time, changes, etc.)
    async fn needs_sync(&self, sync_dir: &Path) -> Result<bool>;

    /// Sync only if needed - default implementation
    async fn sync_if_needed(&self, sync_dir: &Path) -> Result<Option<SyncResult>> {
        if self.needs_sync(sync_dir).await? {
            Ok(Some(self.sync(sync_dir).await?))
        } else {
            Ok(None)
        }
    }

    /// Get a human-readable description of this provider
    fn description(&self) -> String;

    /// Get the provider type identifier
    fn provider_type(&self) -> &'static str;
}
