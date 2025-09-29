//! Core types for export operations.

use chrono::Duration;

/// Information about an export format.
#[derive(Debug, Clone)]
pub struct FormatInfo {
    /// Unique identifier (e.g., "epub").
    pub id: String,
    /// Human-readable name (e.g., "EPUB E-book").
    pub name: String,
    /// MIME type for HTTP responses.
    pub mime_type: Option<String>,
}

impl FormatInfo {
    /// Create new format info.
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            mime_type: None,
        }
    }

    /// Set MIME type.
    pub fn with_mime_type(mut self, mime_type: String) -> Self {
        self.mime_type = Some(mime_type);
        self
    }
}

/// Export configuration options.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Whether to include images.
    pub include_images: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_images: true,
        }
    }
}

impl ExportOptions {
    /// Create new options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Disable images.
    pub fn without_images(mut self) -> Self {
        self.include_images = false;
        self
    }
}

/// Progress information during export.
#[derive(Debug, Clone)]
pub struct ExportProgress {
    /// Total chapters.
    pub chapters_total: u32,
    /// Chapters processed.
    pub chapters_processed: u32,
}

impl ExportProgress {
    /// Create new progress.
    pub fn new(chapters_total: u32) -> Self {
        Self {
            chapters_total,
            chapters_processed: 0,
        }
    }

    /// Calculate percentage (0.0 to 100.0).
    pub fn percentage(&self) -> f64 {
        if self.chapters_total == 0 {
            return 100.0;
        }
        (self.chapters_processed as f64 / self.chapters_total as f64) * 100.0
    }
}

/// Result of an export operation.
#[derive(Debug, Clone)]
pub struct ExportResult {
    /// Whether export succeeded.
    pub success: bool,
    /// Number of chapters processed.
    pub chapters_processed: u32,
    /// Total output size in bytes.
    pub total_size: u64,
    /// Time taken.
    pub export_duration: Duration,
}

impl ExportResult {
    /// Create successful result.
    pub fn success(chapters_processed: u32, total_size: u64, duration: Duration) -> Self {
        Self {
            success: true,
            chapters_processed,
            total_size,
            export_duration: duration,
        }
    }

    /// Create failed result.
    pub fn failure(duration: Duration) -> Self {
        Self {
            success: false,
            chapters_processed: 0,
            total_size: 0,
            export_duration: duration,
        }
    }
}
