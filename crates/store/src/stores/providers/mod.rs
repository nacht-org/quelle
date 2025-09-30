//! Store providers module
//!
//! This module contains different provider implementations for syncing data
//! from various sources to local storage.

pub mod traits;

#[cfg(feature = "git")]
pub mod git;

pub use traits::{StoreProvider, SyncResult};

#[cfg(feature = "git")]
pub use git::{GitAuth, GitProvider, GitReference};
