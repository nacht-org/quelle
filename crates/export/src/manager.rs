//! Export manager for coordinating EPUB export.

use tokio::io::AsyncWrite;

use crate::error::{ExportError, Result};
use crate::traits::Exporter;
use crate::types::{ExportOptions, ExportResult, FormatInfo};
use quelle_storage::{BookStorage, NovelId};

/// Manager for export operations.
pub struct ExportManager {
    exporters: std::collections::HashMap<String, Box<dyn Exporter>>,
}

impl ExportManager {
    /// Create new export manager.
    pub fn new() -> Self {
        Self {
            exporters: std::collections::HashMap::new(),
        }
    }

    /// Register an exporter.
    pub fn register<E: Exporter + 'static>(&mut self, exporter: E) -> Result<()> {
        let format_info = exporter.format_info();
        let format_id = format_info.id.clone();

        if self.exporters.contains_key(&format_id) {
            return Err(ExportError::InvalidConfiguration {
                message: format!("Exporter for format '{}' already registered", format_id),
            });
        }

        self.exporters.insert(format_id, Box::new(exporter));
        Ok(())
    }

    /// Export a novel to the specified format.
    pub async fn export(
        &self,
        format: &str,
        storage: &dyn BookStorage,
        novel_id: &NovelId,
        writer: Box<dyn AsyncWrite + Send + Unpin>,
        options: &ExportOptions,
    ) -> Result<ExportResult> {
        let exporter =
            self.exporters
                .get(format)
                .ok_or_else(|| ExportError::UnsupportedFormat {
                    format: format.to_string(),
                })?;

        exporter.export(storage, novel_id, writer, options).await
    }

    /// Get format information.
    pub fn format_info(&self, format: &str) -> Option<FormatInfo> {
        self.exporters.get(format).map(|e| e.format_info())
    }

    /// List available formats.
    pub fn available_formats(&self) -> Vec<FormatInfo> {
        let mut formats: Vec<_> = self.exporters.values().map(|e| e.format_info()).collect();
        formats.sort_by(|a, b| a.id.cmp(&b.id));
        formats
    }

    /// Check if format is supported.
    pub fn supports_format(&self, format: &str) -> bool {
        self.exporters.contains_key(format)
    }
}

impl Default for ExportManager {
    fn default() -> Self {
        Self::new()
    }
}
