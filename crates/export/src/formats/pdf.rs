//! PDF export format implementation using Typst.

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use typst::foundations::{Bytes, Datetime};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, World};

use crate::converters::HtmlToTypstConverter;
use crate::error::{ExportError, Result};
use crate::traits::Exporter;
use crate::types::{ExportOptions, ExportResult, FormatInfo};
use quelle_storage::{BookStorage, ChapterContent, Novel, NovelId};

/// PDF exporter using Typst.
pub struct PdfExporter {
    font_book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

impl PdfExporter {
    /// Create a new PDF exporter.
    pub fn new() -> Self {
        let mut font_book = FontBook::new();
        let fonts: Vec<Font> = typst_assets::fonts()
            .map(|data| Font::new(Bytes::new(data), 0).unwrap())
            .collect();

        // Register fonts in the font book
        for font in &fonts {
            font_book.push(font.info().clone());
        }

        Self {
            font_book: LazyHash::new(font_book),
            fonts,
        }
    }
}

#[async_trait]
impl Exporter for PdfExporter {
    fn format_info(&self) -> FormatInfo {
        FormatInfo::new("pdf".to_string(), "PDF Document".to_string())
            .with_mime_type("application/pdf".to_string())
    }

    async fn export(
        &self,
        storage: &dyn BookStorage,
        novel_id: &NovelId,
        mut writer: Box<dyn AsyncWrite + Send + Unpin>,
        options: &ExportOptions,
    ) -> Result<ExportResult> {
        let start_time = Utc::now();

        // Load novel data
        let (novel, chapters) = load_novel_data(storage, novel_id).await?;

        // Generate Typst content
        let typst_content = generate_typst_content(&novel, &chapters, options).await?;

        // Create Typst world
        let world = TypstWorld::new(typst_content, self.font_book.clone(), self.fonts.clone());

        // Compile to PDF
        let warned = typst::compile(&world);
        let document = warned.output.map_err(|errors| {
            let error_msg = errors
                .into_iter()
                .map(|e| format!("{}", e.message))
                .collect::<Vec<_>>()
                .join(", ");
            tracing::error!("Typst compilation errors: {}", error_msg);
            ExportError::FormatError {
                message: format!("Typst compilation failed: {}", error_msg),
            }
        })?;

        // Check for warnings
        let warnings = warned.warnings;
        if !warnings.is_empty() {
            tracing::warn!("Typst compilation warnings:");
            for warning in warnings {
                tracing::warn!("  {}", warning.message);
            }
        }

        // Convert to PDF bytes
        let pdf_options = typst_pdf::PdfOptions::default();
        let pdf_result = typst_pdf::pdf(&document, &pdf_options);
        let pdf_bytes = pdf_result.map_err(|errors| {
            let error_msg = errors
                .into_iter()
                .map(|e| format!("{}", e.message))
                .collect::<Vec<_>>()
                .join(", ");
            ExportError::FormatError {
                message: format!("PDF generation failed: {}", error_msg),
            }
        })?;

        // Write PDF to output
        writer.write_all(&pdf_bytes).await?;
        writer.flush().await?;

        let end_time = Utc::now();
        let duration = end_time.signed_duration_since(start_time);

        Ok(ExportResult::success(
            chapters.len() as u32,
            pdf_bytes.len() as u64,
            duration,
        ))
    }
}

impl Default for PdfExporter {
    fn default() -> Self {
        Self::new()
    }
}

/// Load novel and chapter data from storage.
async fn load_novel_data(
    storage: &dyn BookStorage,
    novel_id: &NovelId,
) -> Result<(Novel, Vec<(String, ChapterContent)>)> {
    let novel = storage
        .get_novel(novel_id)
        .await?
        .ok_or_else(|| ExportError::NovelNotFound {
            novel_id: novel_id.as_str().to_string(),
        })?;

    let chapter_list = storage.list_chapters(novel_id).await?;
    let mut chapters = Vec::new();

    for chapter_meta in chapter_list {
        if let Some(content) = storage
            .get_chapter_content(
                novel_id,
                chapter_meta.volume_index,
                &chapter_meta.chapter_url,
            )
            .await?
        {
            chapters.push((chapter_meta.chapter_title, content));
        }
    }

    Ok((novel, chapters))
}

/// Generate Typst markup content for the novel.
pub async fn generate_typst_content(
    novel: &Novel,
    chapters: &[(String, ChapterContent)],
    _options: &ExportOptions,
) -> Result<String> {
    let mut content = String::new();

    tracing::info!(
        "Generating Typst content for novel '{}' with {} chapters",
        novel.title,
        chapters.len()
    );

    // Document setup
    content.push_str("#set document(\n");
    content.push_str(&format!("  title: \"{}\",\n", sanitize_title(&novel.title)));
    if !novel.authors.is_empty() {
        content.push_str(&format!(
            "  author: ({}),\n",
            novel
                .authors
                .iter()
                .map(|a| format!("\"{}\"", sanitize_title(a)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    content.push_str(")\n\n");

    // Page setup
    content.push_str("#set page(\n");
    content.push_str("  paper: \"a4\",\n");
    content.push_str("  margin: (x: 2cm, y: 2.5cm),\n");
    content.push_str("  numbering: \"1\",\n");
    content.push_str(")\n\n");

    // Text formatting - use default fonts for compatibility
    let language = novel.langs.first().map(|s| s.as_str()).unwrap_or("en");
    content.push_str("#set text(\n");
    content.push_str("  size: 12pt,\n");
    content.push_str(&format!("  lang: \"{}\",\n", language));
    content.push_str(")\n\n");

    // Paragraph formatting
    content.push_str("#set par(\n");
    content.push_str("  justify: true,\n");
    content.push_str(")\n\n");

    // Title page
    content.push_str(&format!("= {}\n\n", sanitize_title(&novel.title)));

    if !novel.authors.is_empty() {
        content.push_str(&format!("*By: {}*\n\n", novel.authors.join(", ")));
    }

    if !novel.description.is_empty() {
        let description_text = novel.description.join(" ");
        let clean_description = sanitize_title(&description_text);
        content.push_str(&format!("_{}_\n\n", clean_description));
    }

    content.push_str("#pagebreak()\n\n");

    // Table of contents
    if chapters.len() > 1 {
        content.push_str("#outline()\n\n");
        content.push_str("#pagebreak()\n\n");
    }

    // Chapters with HTML conversion
    for (index, (title, chapter_content)) in chapters.iter().enumerate() {
        if index > 0 {
            content.push_str("#pagebreak()\n\n");
        }

        // Chapter heading
        content.push_str(&format!("== {}\n\n", sanitize_title(title)));

        // Try HTML converter first, with safe fallback
        match use_fallback_converter(&chapter_content.data) {
            Ok(typst_content) => {
                if !typst_content.trim().is_empty() {
                    // Apply light sanitization only for Unicode characters
                    let safe_content = sanitize_for_typst(&typst_content);
                    content.push_str(&safe_content);
                } else {
                    content.push_str("_No content available_");
                }
            }
            Err(_e) => {
                // Fallback to simple HTML tag stripping
                let plain_text = strip_html_tags(&chapter_content.data);
                if !plain_text.trim().is_empty() {
                    // For fallback text, we need full escaping since it wasn't processed by the converter
                    let escaped_text = escape_typst(&plain_text);
                    let safe_text = sanitize_for_typst(&escaped_text);
                    content.push_str(&safe_text);
                } else {
                    content.push_str("_No content available_");
                }
            }
        }
        content.push_str("\n\n");
    }

    tracing::info!(
        "Generated Typst content: {} characters total",
        content.len()
    );

    Ok(content)
}

/// Sanitize title for Typst by removing problematic characters
fn sanitize_title(title: &str) -> String {
    title
        .replace('【', "[")
        .replace('】', "]")
        .replace('#', "")
        .replace('$', "")
        .replace('@', "")
        .replace('\\', "")
        .replace('{', "")
        .replace('}', "")
}

/// Sanitize text content for safe use in Typst documents
/// This is a light safety layer that only handles special Unicode characters
/// and whitespace cleanup. Regular Typst escaping should be done by the converter.
fn sanitize_for_typst(text: &str) -> String {
    text
        // Only handle special Unicode characters that might not be escaped properly
        .replace('【', "[")
        .replace('】', "]")
        // Clean up excessive whitespace
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Escape text for safe use in Typst markup.
pub fn escape_typst(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('#', "\\#")
        .replace('@', "\\@")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('$', "\\$")
        .replace('_', "\\_")
        .replace('*', "\\*")
        .replace('`', "\\`")
}

/// Use the fallback HTML converter when Pandoc is not available
fn use_fallback_converter(html: &str) -> Result<String> {
    let mut converter = HtmlToTypstConverter::new();

    // Convert and then apply basic validation
    let result = converter
        .convert(html)
        .map_err(|e| ExportError::FormatError {
            message: format!("HTML conversion failed: {}", e),
        })?;

    // Basic validation: check for severely unbalanced brackets
    let open_brackets = result.chars().filter(|&c| c == '[').count();
    let close_brackets = result.chars().filter(|&c| c == ']').count();

    if open_brackets.abs_diff(close_brackets) > 5 {
        return Err(ExportError::FormatError {
            message: "HTML converter produced unbalanced brackets".to_string(),
        });
    }

    Ok(result)
}

/// Strip HTML tags as a last resort fallback (simple regex-based removal).
fn strip_html_tags(html: &str) -> String {
    let mut result = html.to_string();

    // Handle line breaks BEFORE removing all tags
    result = result
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n");

    // Remove all HTML tags
    let re = regex::Regex::new(r"<[^>]*>").unwrap();
    result = re.replace_all(&result, "").to_string();

    // Basic entity decoding
    result
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Minimal Typst World implementation for compiling documents.
struct TypstWorld {
    main_id: FileId,
    library: LazyHash<Library>,
    font_book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    files: HashMap<FileId, Source>,
}

impl TypstWorld {
    fn new(content: String, font_book: LazyHash<FontBook>, fonts: Vec<Font>) -> Self {
        let main_id = FileId::new(None, VirtualPath::new("main.typ"));
        let main_source = Source::new(main_id, content);

        let library = LazyHash::new(Library::builder().build());
        let mut files = HashMap::new();
        files.insert(main_id, main_source.clone());

        Self {
            main_id,
            library,
            font_book,
            fonts,
            files,
        }
    }
}

impl World for TypstWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.font_book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> typst::diag::FileResult<Source> {
        self.files
            .get(&id)
            .cloned()
            .ok_or_else(|| typst::diag::FileError::NotFound(id.vpath().as_rootless_path().into()))
    }

    fn file(&self, _id: FileId) -> typst::diag::FileResult<Bytes> {
        Err(typst::diag::FileError::NotFound(
            std::path::Path::new("").into(),
        ))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        Some(Datetime::from_ymd_hms(2024, 1, 1, 0, 0, 0).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_typst() {
        assert_eq!(escape_typst("Hello #world"), "Hello \\#world");
        assert_eq!(escape_typst("Test [brackets]"), "Test \\[brackets\\]");
        assert_eq!(escape_typst("Math $x = 1$"), "Math \\$x = 1\\$");
        assert_eq!(escape_typst("Code `test`"), "Code \\`test\\`");
    }

    #[test]
    fn test_strip_html_tags() {
        // Test the simple HTML tag stripping fallback
        let result = strip_html_tags("<p>Hello <b>world</b>!</p>");
        assert_eq!(result, "Hello world!\n\n");

        let result2 = strip_html_tags("Line 1<br/>Line 2");
        assert_eq!(result2, "Line 1\nLine 2");

        let result3 = strip_html_tags("&amp; &lt;test&gt;");
        assert!(result3.contains("& <test>"));
    }

    #[test]
    fn test_exporter_creation() {
        let exporter = PdfExporter::new();
        let info = exporter.format_info();
        assert_eq!(info.id, "pdf");
        assert_eq!(info.name, "PDF Document");
        assert_eq!(info.mime_type, Some("application/pdf".to_string()));
    }
}
