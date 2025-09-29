//! Backend implementations for the book storage system.
//!
//! This module contains different storage backend implementations,
//! such as filesystem, database, and cloud storage backends.

pub mod filesystem;

// Re-export the main filesystem backend for convenience
pub use filesystem::FilesystemStorage;
