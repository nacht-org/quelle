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

/// Lifecycle events that can occur during store operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// An extension version was published
    Published {
        extension_id: String,
        version: String,
    },
    /// An extension version was unpublished
    Unpublished {
        extension_id: String,
        version: String,
    },
}

impl LifecycleEvent {
    /// Get the extension ID for this event
    pub fn extension_id(&self) -> &str {
        match self {
            Self::Published { extension_id, .. } => extension_id,
            Self::Unpublished { extension_id, .. } => extension_id,
        }
    }

    /// Get the version for this event
    pub fn version(&self) -> &str {
        match self {
            Self::Published { version, .. } => version,
            Self::Unpublished { version, .. } => version,
        }
    }

    /// Check if this is a publish event
    pub fn is_publish(&self) -> bool {
        matches!(self, Self::Published { .. })
    }

    /// Check if this is an unpublish event
    pub fn is_unpublish(&self) -> bool {
        matches!(self, Self::Unpublished { .. })
    }
}

/// Provider capabilities that can be queried
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Capability {
    /// Provider supports write operations (publish/unpublish)
    Write,
    /// Provider supports incremental syncing
    IncrementalSync,
    /// Provider supports authentication
    Authentication,
    /// Provider can push changes to remote
    RemotePush,
    /// Provider supports caching
    Caching,
    /// Provider supports background sync
    BackgroundSync,
}

impl Capability {
    /// Get the string identifier for this capability
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::IncrementalSync => "incremental_sync",
            Self::Authentication => "authentication",
            Self::RemotePush => "remote_push",
            Self::Caching => "caching",
            Self::BackgroundSync => "background_sync",
        }
    }

    /// Parse a capability from a string identifier
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "write" => Some(Self::Write),
            "incremental_sync" => Some(Self::IncrementalSync),
            "authentication" => Some(Self::Authentication),
            "remote_push" => Some(Self::RemotePush),
            "caching" => Some(Self::Caching),
            "background_sync" => Some(Self::BackgroundSync),
            _ => None,
        }
    }
}

/// Trait for providers that can sync data from various sources to local storage
#[async_trait]
pub trait StoreProvider: Send + Sync {
    /// Get the directory where this provider syncs data
    /// This is the authoritative location for the provider's local cache
    fn sync_dir(&self) -> &Path;

    /// Sync/update the local store from the source
    /// This should handle initial setup (clone/download) as well as updates
    async fn sync(&self) -> Result<SyncResult>;

    /// Check if sync is needed (based on time, changes, etc.)
    async fn needs_sync(&self) -> Result<bool>;

    /// Get a human-readable description of this provider
    fn description(&self) -> String;

    /// Get the provider type identifier
    fn provider_type(&self) -> &'static str;

    /// Check if this provider supports a specific capability
    ///
    /// Returns `true` if the capability is supported, `false` otherwise.
    /// Providers should implement this to declare their capabilities.
    fn supports_capability(&self, capability: Capability) -> bool;

    /// Handle lifecycle events (publish/unpublish)
    ///
    /// This unified hook is called after successful publish or unpublish operations.
    /// Providers can use this to:
    /// - Commit changes to version control
    /// - Push to remote repositories
    /// - Trigger webhooks or notifications
    /// - Update indexes or caches
    ///
    /// The default implementation does nothing (no-op for read-only providers).
    async fn handle_event(&self, _event: LifecycleEvent) -> Result<()> {
        Ok(())
    }

    /// Ensure the provider is in a valid state for write operations
    ///
    /// This is called before any write operation to validate that writing is possible.
    /// Providers should check:
    /// - Authentication status
    /// - Network connectivity (if needed)
    /// - Repository state (dirty working tree, etc.)
    /// - Permissions
    ///
    /// Returns `Ok(())` if writing is allowed, or an error describing why it's blocked.
    ///
    /// The default implementation checks if the provider supports write capability.
    async fn ensure_writable(&self) -> Result<()> {
        if !self.supports_capability(Capability::Write) {
            return Err(crate::error::StoreError::InvalidPackage {
                reason: "Provider does not support write operations".to_string(),
            });
        }
        Ok(())
    }
}
