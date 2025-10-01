//! EPUB export format implementation with simplified, SRP-focused design.

use async_trait::async_trait;
use chrono::Utc;
use epub_builder::{EpubBuilder, EpubContent, ReferenceType, ZipLibrary};
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::error::{ExportError, Result};
use crate::traits::Exporter;
use crate::types::{ExportOptions, ExportResult, FormatInfo};

use quelle_storage::{BookStorage, ChapterContent, Novel, NovelId, WitNovelStatus};

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

        // Load novel
        let novel = load_novel(storage, novel_id).await?;

        // Create EPUB builder
        let mut epub_builder = EpubBuilder::new(ZipLibrary::new()?)?;

        // Apply metadata
        build_metadata(&mut epub_builder, &novel)?;

        // Add cover image if available and requested
        if options.include_images {
            add_cover_image(&mut epub_builder, storage, &novel).await?;
        }

        // Add title page
        let title_page_html = build_title_page(&novel);
        epub_builder.add_content(
            EpubContent::new("title.xhtml", title_page_html.as_bytes())
                .title("Title Page")
                .reftype(ReferenceType::TitlePage),
        )?;

        // Add table of contents
        let toc_html = build_toc_page(&novel);
        epub_builder.add_content(
            EpubContent::new("toc.xhtml", toc_html.as_bytes())
                .title("Table of Contents")
                .reftype(ReferenceType::Toc),
        )?;

        // Add chapters
        let chapters_processed = add_chapters(&mut epub_builder, storage, novel_id, &novel).await?;

        // Generate EPUB
        let mut epub_data = Vec::new();
        epub_builder.generate(&mut epub_data)?;

        // Write output
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

/// Load novel from storage with error handling.
async fn load_novel(storage: &dyn BookStorage, novel_id: &NovelId) -> Result<Novel> {
    storage
        .get_novel(novel_id)
        .await?
        .ok_or_else(|| ExportError::NovelNotFound {
            novel_id: novel_id.to_string(),
        })
}

/// Build and apply EPUB metadata from novel data.
fn build_metadata(epub_builder: &mut EpubBuilder<ZipLibrary>, novel: &Novel) -> Result<()> {
    epub_builder.set_title(&novel.title);
    epub_builder.set_authors(novel.authors.clone());

    // Set language (use first available or default to English)
    let language = novel.langs.first().map(|s| s.as_str()).unwrap_or("en");
    epub_builder.set_lang(language);

    // Set description if available
    if !novel.description.is_empty() {
        epub_builder.set_description(novel.description.clone());
    }

    // Add generator information
    epub_builder.metadata("generator", "Quelle EPUB Exporter")?;

    Ok(())
}

/// Add cover image to EPUB if available.
async fn add_cover_image(
    epub_builder: &mut EpubBuilder<ZipLibrary>,
    storage: &dyn BookStorage,
    novel: &Novel,
) -> Result<()> {
    if let Some(cover_url) = &novel.cover {
        if let Some(asset_id) = storage.find_asset_by_url(cover_url).await? {
            if let Some(asset_data) = storage.get_asset_data(&asset_id).await? {
                epub_builder.add_content(
                    EpubContent::new("cover.jpg", asset_data.as_slice())
                        .title("Cover")
                        .reftype(ReferenceType::Cover),
                )?;
            }
        }
    }
    Ok(())
}

/// Build the title page HTML with novel metadata.
fn build_title_page(novel: &Novel) -> String {
    let title = escape_html(&novel.title);
    let authors = if novel.authors.is_empty() {
        "Unknown Author".to_string()
    } else {
        escape_html(&novel.authors.join(", "))
    };

    let mut html = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>{}</title>
</head>
<body>
    <h1>{}</h1>
    <p>by {}</p>
"#,
        title, title, authors
    );

    // Add description if available
    if !novel.description.is_empty() {
        html.push_str("\n    <h2>Description</h2>\n");
        for paragraph in &novel.description {
            let escaped_para = escape_html(paragraph);
            html.push_str(&format!("    <p>{}</p>\n", escaped_para));
        }
    }

    // Add tags/genres if available
    let tags = extract_tags(novel);
    if !tags.is_empty() {
        html.push_str("\n    <h2>Tags</h2>\n    <p>");
        for (i, tag) in tags.iter().enumerate() {
            if i > 0 {
                html.push_str(", ");
            }
            html.push_str(&escape_html(tag));
        }
        html.push_str("</p>\n");
    }

    // Add publication info
    html.push_str("\n    <h2>Publication Info</h2>\n");

    let status = format_novel_status(novel.status);
    html.push_str(&format!("    <p>Status: {}</p>\n", status));

    let total_chapters: usize = novel.volumes.iter().map(|v| v.chapters.len()).sum();
    html.push_str(&format!("    <p>Chapters: {}</p>\n", total_chapters));
    html.push_str(&format!("    <p>Volumes: {}</p>\n", novel.volumes.len()));

    if !novel.url.is_empty() {
        let escaped_url = escape_html(&novel.url);
        html.push_str(&format!("    <p>Source: {}</p>\n", escaped_url));
    }

    // Add rating if available
    if let Some(rating) = extract_rating(novel) {
        html.push_str(&format!("    <p>Rating: {}</p>\n", escape_html(&rating)));
    }

    html.push_str("\n    <p><em>Generated by Quelle EPUB Exporter</em></p>\n");
    html.push_str("</body>\n</html>");

    html
}

/// Build the table of contents HTML page.
fn build_toc_page(novel: &Novel) -> String {
    let title = escape_html(&novel.title);

    let mut html = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>Table of Contents</title>
</head>
<body>
    <h1>Table of Contents</h1>
    <h2>{}</h2>
"#,
        title
    );

    // Add front matter links
    html.push_str("    <ul>\n");
    html.push_str("        <li><a href=\"title.xhtml\">Title Page</a></li>\n");

    // Add chapters organized by volume
    for volume in &novel.volumes {
        if volume.chapters.is_empty() {
            continue;
        }

        let volume_name = escape_html(&volume.name);
        if novel.volumes.len() > 1 {
            html.push_str(&format!("        <li>{}\n            <ul>\n", volume_name));
        }

        for chapter in &volume.chapters {
            let chapter_title = escape_html(&chapter.title);
            let chapter_file = format!("chapter_{}.xhtml", chapter.index);

            let indent = if novel.volumes.len() > 1 {
                "                "
            } else {
                "        "
            };
            html.push_str(&format!(
                "{}    <li><a href=\"{}\">{}</a></li>\n",
                indent, chapter_file, chapter_title
            ));
        }

        if novel.volumes.len() > 1 {
            html.push_str("            </ul>\n        </li>\n");
        }
    }

    html.push_str("    </ul>\n");
    html.push_str("</body>\n</html>");

    html
}

/// Add all chapters to the EPUB.
async fn add_chapters(
    epub_builder: &mut EpubBuilder<ZipLibrary>,
    storage: &dyn BookStorage,
    novel_id: &NovelId,
    novel: &Novel,
) -> Result<u32> {
    let mut chapters_processed = 0u32;

    for volume in &novel.volumes {
        for chapter in &volume.chapters {
            if let Some(content) = storage
                .get_chapter_content(novel_id, volume.index, &chapter.url)
                .await?
            {
                let chapter_html = build_chapter_html(&chapter.title, &content);
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

    Ok(chapters_processed)
}

/// Build HTML for a single chapter.
fn build_chapter_html(title: &str, content: &ChapterContent) -> String {
    let escaped_title = escape_html(title);
    let sanitized_content = sanitize_html(&content.data);

    format!(
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
        escaped_title, escaped_title, sanitized_content
    )
}

/// Extract tags/genres from novel metadata.
fn extract_tags(novel: &Novel) -> Vec<String> {
    let mut tags = Vec::new();

    for metadata in &novel.metadata {
        if metadata.name == "subject" || metadata.name == "tag" {
            let tag = metadata.value.trim();
            if !tag.is_empty() && !tags.contains(&tag.to_string()) {
                tags.push(tag.to_string());
            }
        }
    }

    tags.sort();
    tags
}

/// Extract rating from novel metadata.
fn extract_rating(novel: &Novel) -> Option<String> {
    for metadata in &novel.metadata {
        if metadata.name == "rating" {
            let rating = metadata.value.trim();
            if !rating.is_empty() {
                return Some(rating.to_string());
            }
        }
    }
    None
}

/// Format novel status for display.
fn format_novel_status(status: WitNovelStatus) -> &'static str {
    match status {
        WitNovelStatus::Ongoing => "Ongoing",
        WitNovelStatus::Completed => "Completed",
        WitNovelStatus::Hiatus => "On Hiatus",
        WitNovelStatus::Dropped => "Dropped",
        WitNovelStatus::Stub => "Stub",
        WitNovelStatus::Unknown => "Unknown",
    }
}

/// Escape HTML special characters for safe inclusion in HTML content.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Basic HTML sanitization - removes dangerous elements and scripts.
fn sanitize_html(html: &str) -> String {
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
    use quelle_engine::bindings::quelle::extension::novel::{Chapter, Volume};
    use quelle_storage::{ChapterContent, Novel, WitNovelStatus};

    fn create_test_novel() -> Novel {
        Novel {
            url: "https://example.com/novel".to_string(),
            title: "Test Novel & Adventure".to_string(),
            authors: vec!["Author One".to_string(), "Author Two".to_string()],
            description: vec![
                "First paragraph".to_string(),
                "Second paragraph".to_string(),
            ],
            cover: None,
            langs: vec!["en".to_string()],
            status: WitNovelStatus::Ongoing,
            metadata: vec![
                quelle_engine::bindings::quelle::extension::novel::Metadata {
                    name: "subject".to_string(),
                    value: "Fantasy".to_string(),
                    ns: quelle_engine::bindings::quelle::extension::novel::Namespace::Dc,
                    others: vec![],
                },
                quelle_engine::bindings::quelle::extension::novel::Metadata {
                    name: "subject".to_string(),
                    value: "Adventure".to_string(),
                    ns: quelle_engine::bindings::quelle::extension::novel::Namespace::Dc,
                    others: vec![],
                },
                quelle_engine::bindings::quelle::extension::novel::Metadata {
                    name: "tag".to_string(),
                    value: "Magic System".to_string(),
                    ns: quelle_engine::bindings::quelle::extension::novel::Namespace::Opf,
                    others: vec![],
                },
                quelle_engine::bindings::quelle::extension::novel::Metadata {
                    name: "rating".to_string(),
                    value: "4.5 (123 ratings)".to_string(),
                    ns: quelle_engine::bindings::quelle::extension::novel::Namespace::Opf,
                    others: vec![],
                },
            ],
            volumes: vec![Volume {
                name: "Volume 1".to_string(),
                index: 0,
                chapters: vec![
                    Chapter {
                        title: "Chapter 1".to_string(),
                        index: 0,
                        url: "chapter1".to_string(),
                        updated_at: None,
                    },
                    Chapter {
                        title: "Chapter 2".to_string(),
                        index: 1,
                        url: "chapter2".to_string(),
                        updated_at: None,
                    },
                ],
            }],
        }
    }

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
    fn test_build_title_page() {
        let novel = create_test_novel();
        let html = build_title_page(&novel);

        assert!(html.contains("Test Novel &amp; Adventure"));
        assert!(html.contains("Author One, Author Two"));
        assert!(html.contains("First paragraph"));
        assert!(html.contains("Second paragraph"));
        assert!(html.contains("Adventure, Fantasy, Magic System"));
        assert!(html.contains("Rating: 4.5 (123 ratings)"));
        assert!(html.contains("Status: Ongoing"));
        assert!(html.contains("Chapters: 2"));
        assert!(html.contains("Volumes: 1"));
        assert!(html.contains("https://example.com/novel"));
    }

    #[test]
    fn test_build_toc_page() {
        let novel = create_test_novel();
        let html = build_toc_page(&novel);

        assert!(html.contains("Table of Contents"));
        assert!(html.contains("Test Novel &amp; Adventure"));
        assert!(html.contains("title.xhtml"));
        assert!(html.contains("chapter_0.xhtml"));
        assert!(html.contains("chapter_1.xhtml"));
        assert!(html.contains("Chapter 1"));
        assert!(html.contains("Chapter 2"));
    }

    #[test]
    fn test_build_chapter_html() {
        let content = ChapterContent {
            data: "<p>This is chapter content with <script>alert('xss')</script> potential issues.</p>".to_string(),
        };

        let html = build_chapter_html("Test Chapter & More", &content);

        assert!(html.contains("Test Chapter &amp; More"));
        assert!(html.contains("chapter content"));
        // Should sanitize script tags
        assert!(!html.contains("<script>alert('xss')</script>"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("Hello & <world>"), "Hello &amp; &lt;world&gt;");
        assert_eq!(escape_html("\"Test\""), "&quot;Test&quot;");
    }

    #[test]
    fn test_sanitize_html() {
        assert_eq!(
            sanitize_html("<script>alert('xss')</script>"),
            "&lt;script>alert('xss')&lt;/script&gt;"
        );
        assert_eq!(
            sanitize_html("<p onclick='bad()'>Good content</p>"),
            "<p data-onclick='bad()'>Good content</p>"
        );
    }

    #[test]
    fn test_extract_tags() {
        let novel = create_test_novel();
        let tags = extract_tags(&novel);
        assert_eq!(tags, vec!["Adventure", "Fantasy", "Magic System"]);

        // Test novel with no metadata
        let empty_novel = Novel {
            url: "".to_string(),
            title: "Empty".to_string(),
            authors: vec![],
            description: vec![],
            cover: None,
            langs: vec![],
            status: WitNovelStatus::Unknown,
            metadata: vec![],
            volumes: vec![],
        };
        let empty_tags = extract_tags(&empty_novel);
        assert_eq!(empty_tags, Vec::<String>::new());
    }

    #[test]
    fn test_extract_rating() {
        let novel = create_test_novel();
        let rating = extract_rating(&novel);
        assert_eq!(rating, Some("4.5 (123 ratings)".to_string()));

        // Test novel with no rating
        let no_rating_novel = Novel {
            url: "".to_string(),
            title: "No Rating".to_string(),
            authors: vec![],
            description: vec![],
            cover: None,
            langs: vec![],
            status: WitNovelStatus::Unknown,
            metadata: vec![],
            volumes: vec![],
        };
        let no_rating = extract_rating(&no_rating_novel);
        assert_eq!(no_rating, None);
    }

    #[test]
    fn test_format_novel_status() {
        assert_eq!(format_novel_status(WitNovelStatus::Ongoing), "Ongoing");
        assert_eq!(format_novel_status(WitNovelStatus::Completed), "Completed");
        assert_eq!(format_novel_status(WitNovelStatus::Hiatus), "On Hiatus");
        assert_eq!(format_novel_status(WitNovelStatus::Dropped), "Dropped");
        assert_eq!(format_novel_status(WitNovelStatus::Stub), "Stub");
        assert_eq!(format_novel_status(WitNovelStatus::Unknown), "Unknown");
    }
}
