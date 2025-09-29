//! Quelle Export - EPUB export functionality for e-book content
//!
//! This crate provides EPUB export capabilities for novels stored in the
//! Quelle storage system.

pub mod error;
pub mod manager;
pub mod traits;
pub mod types;

pub mod formats;

// Re-export main types
pub use error::{ExportError, Result};
pub use manager::ExportManager;
pub use traits::Exporter;
pub use types::{ExportOptions, ExportProgress, ExportResult, FormatInfo};

// Re-export EPUB exporter
pub use formats::EpubExporter;

// Re-export storage types we work with
pub use quelle_storage::{BookStorage, ChapterContent, Novel, NovelId};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Create an export manager with EPUB exporter registered
pub fn default_export_manager() -> Result<ExportManager> {
    let mut manager = ExportManager::new();
    manager.register(EpubExporter::new())?;
    Ok(manager)
}
