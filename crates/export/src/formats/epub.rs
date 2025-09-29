//! EPUB export format implementation.

use async_trait::async_trait;
use chrono::Utc;
use epub_builder::{EpubBuilder, EpubContent, ReferenceType, ZipLibrary};

use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::error::{ExportError, Result};
use crate::traits::Exporter;
use crate::types::{ExportOptions, ExportResult, FormatInfo};
use quelle_storage::{BookStorage, NovelId};

/// EPUB format exporter.
pub struct EpubExporter;

impl EpubExporter {
    /// Create new EPUB exporter.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Exporter for EpubExporter {
    fn format_info(&self) -> FormatInfo {
        FormatInfo::new("epub".to_string(), "EPUB E-book".to_string())
            .with_mime_type("application/epub+zip".to_string())
    }

    async fn export(
        &self,
        storage: &dyn BookStorage,
        novel_id: &NovelId,
        mut writer: Box<dyn AsyncWrite + Send + Unpin>,
        options: &ExportOptions,
    ) -> Result<ExportResult> {
        let start_time = Utc::now();

        // Load novel metadata
        let novel =
            storage
                .get_novel(novel_id)
                .await?
                .ok_or_else(|| ExportError::NovelNotFound {
                    novel_id: novel_id.to_string(),
                })?;

        // Get chapter information
        let chapter_infos = storage.list_chapters(novel_id).await?;
        let _total_chapters = chapter_infos.len() as u32;

        // Create EPUB builder
        let mut epub_builder = EpubBuilder::new(ZipLibrary::new()?)?;

        // Set metadata
        epub_builder.metadata("title", &novel.title)?;
        for author in &novel.authors {
            epub_builder.metadata("creator", author)?;
        }
        if !novel.langs.is_empty() {
            epub_builder.metadata("language", &novel.langs[0])?;
        }
        if !novel.description.is_empty() {
            epub_builder.metadata("description", &novel.description.join(" "))?;
        }

        // Add cover image if available and requested
        if options.include_images {
            if let Some(cover_url) = &novel.cover {
                // Note: In a real implementation, you'd download the cover image
                // For now, we'll skip it
                let _ = cover_url;
            }
        }

        let mut chapters_processed = 0u32;

        // Process each volume and chapter
        for volume in &novel.volumes {
            for chapter in &volume.chapters {
                // Load chapter content
                if let Some(content) = storage
                    .get_chapter_content(novel_id, volume.index, &chapter.url)
                    .await?
                {
                    // Create chapter HTML
                    let chapter_html = format!(
                        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>{}</title>
</head>
<body>
    <h1>{}</h1>
    <div>
        {}
    </div>
</body>
</html>"#,
                        html_escape(&chapter.title),
                        html_escape(&chapter.title),
                        sanitize_html(&content.data)
                    );

                    // Add chapter to EPUB
                    let chapter_filename = format!("chapter_{}.xhtml", chapter.index);
                    epub_builder.add_content(
                        EpubContent::new(&chapter_filename, chapter_html.as_bytes())
                            .title(&chapter.title)
                            .reftype(ReferenceType::Text),
                    )?;

                    chapters_processed += 1;
                }
            }
        }

        // Generate EPUB data
        let mut epub_data = Vec::new();
        epub_builder.generate(&mut epub_data)?;

        // Write to output
        writer.write_all(&epub_data).await?;
        writer.flush().await?;

        let end_time = Utc::now();
        let duration = end_time.signed_duration_since(start_time);

        Ok(ExportResult::success(
            chapters_processed,
            epub_data.len() as u64,
            duration,
        ))
    }
}

impl Default for EpubExporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Escape HTML special characters.
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Basic HTML sanitization for chapter content.
fn sanitize_html(html: &str) -> String {
    // In a real implementation, this would properly parse and sanitize HTML
    // For now, we'll do basic cleaning
    html.replace("<script", "&lt;script")
        .replace("</script>", "&lt;/script&gt;")
        .replace("javascript:", "")
        .replace("vbscript:", "")
        .replace("onload=", "data-onload=")
        .replace("onclick=", "data-onclick=")
        .replace("onmouseover=", "data-onmouseover=")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exporter_creation() {
        let exporter = EpubExporter::new();
        let format_info = exporter.format_info();

        assert_eq!(format_info.id, "epub");
        assert_eq!(format_info.name, "EPUB E-book");
        assert_eq!(
            format_info.mime_type,
            Some("application/epub+zip".to_string())
        );
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("Hello & <world>"), "Hello &amp; &lt;world&gt;");
        assert_eq!(html_escape("\"Test\""), "&quot;Test&quot;");
    }

    #[test]
    fn test_sanitize_html() {
        assert_eq!(
            sanitize_html("<script>alert('xss')</script>"),
            "&lt;script&gt;alert('xss')&lt;/script&gt;"
        );
        assert_eq!(
            sanitize_html("<p onclick='bad()'>Good content</p>"),
            "<p data-onclick='bad()'>Good content</p>"
        );
    }
}
