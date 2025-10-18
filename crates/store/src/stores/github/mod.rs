//! GitHub-based store implementation
//!
//! This module provides GitHub-specific store functionality including:
//! - GitHubStore: A read-only store that accesses GitHub repositories via raw URLs
//! - GitHubFileOperations: File operations implementation for GitHub repositories
//! - Builder patterns for constructing GitHub stores with proper configuration
//!
//! The GitHub store is designed to be lightweight and doesn't require GitHub API tokens
//! for read operations, though it can optionally use the API for improved default branch
//! detection and directory listings.

pub mod file_operations;
pub mod store;

// Re-export the main types for convenience
pub use file_operations::{GitHubFileOperations, GitHubFileOperationsBuilder};
pub use store::{GitHubStore, GitHubStoreBuilder};
