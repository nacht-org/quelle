//! Core trait for export functionality.

use async_trait::async_trait;
use tokio::io::AsyncWrite;

use crate::error::Result;
use crate::types::{ExportOptions, ExportResult, FormatInfo};
use quelle_storage::{BookStorage, NovelId};

/// Core trait for exporting novels to different formats.
#[async_trait]
pub trait Exporter: Send + Sync {
    /// Get format information.
    fn format_info(&self) -> FormatInfo;

    /// Export a novel from storage to a writer.
    async fn export(
        &self,
        storage: &dyn BookStorage,
        novel_id: &NovelId,
        writer: Box<dyn AsyncWrite + Send + Unpin>,
        options: &ExportOptions,
    ) -> Result<ExportResult>;
}
