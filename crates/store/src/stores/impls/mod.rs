//! Store implementations
//!
//! This module contains the actual store implementations for different backends.

pub mod local;
pub mod locally_cached;

#[cfg(feature = "git")]
pub mod git;

#[cfg(feature = "github")]
pub mod github;

// Re-export commonly used implementations
pub use local::{LocalStore, LocalStoreBuilder};
pub use locally_cached::LocallyCachedStore;

#[cfg(feature = "git")]
pub use git::{GitStore, GitStoreBuilder};

#[cfg(feature = "github")]
pub use github::{GitHubStore, GitHubStoreBuilder};
