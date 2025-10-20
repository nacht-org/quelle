//! Store implementations and factory for the Quelle extension store system
//!
//! This module provides a unified interface for creating and managing different types
//! of extension stores (local, git, http, etc.) with proper separation of concerns
//! and extensible patterns.

pub mod traits;

// Internal modules
pub(crate) mod file_operations;

// Store implementations
pub mod impls;
pub mod providers;

// Store configuration
pub mod config;

// Re-export commonly used traits
pub use traits::{
    BaseStore, CacheStats, CacheableStore, ReadWriteStore, ReadableStore, WritableStore,
};

// Re-export store implementations and provider types
pub use impls::{LocalStore, LocalStoreBuilder, LocallyCachedStore};
pub use providers::{StoreProvider, SyncResult};

#[cfg(feature = "git")]
pub use impls::{GitStore, GitStoreBuilder};

#[cfg(feature = "github")]
pub use impls::{GitHubStore, GitHubStoreBuilder};

// Re-export store configuration and source types
pub use config::{create_readable_store_from_source, ExtensionSource, RegistryConfig, StoreType};
pub use config::{RegistryStoreConfig, RegistryStoreConfigs, StoreConfigCounts};

#[cfg(feature = "git")]
pub use providers::{GitAuth, GitProvider, GitReference};
