//! PDF export format implementation using Typst.

use async_trait::async_trait;
use chrono::Utc;
use comemo::Prehashed;
use std::collections::HashMap;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::process::Command;
use typst::foundations::{Bytes, Datetime, Smart};
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::{Library, World};

use crate::converters::{convert_html_to_typst, HtmlToTypstConverter};
use crate::error::{ExportError, Result};
use crate::traits::Exporter;
use crate::types::{ExportOptions, ExportResult, FormatInfo};
use quelle_storage::{BookStorage, ChapterContent, Novel, NovelId};

/// PDF exporter using Typst.
pub struct PdfExporter {
    font_book: Prehashed<FontBook>,
    fonts: Vec<Font>,
}

impl PdfExporter {
    /// Create a new PDF exporter.
    pub fn new() -> Self {
        let mut font_book = FontBook::new();
        let fonts: Vec<Font> = typst_assets::fonts()
            .map(|data| Font::new(Bytes::from_static(data), 0).unwrap())
            .collect();

        // Register fonts in the font book
        for font in &fonts {
            font_book.push(font.info().clone());
        }

        Self {
            font_book: Prehashed::new(font_book),
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
        let mut tracer = typst::eval::Tracer::new();
        let document = typst::compile(&world, &mut tracer).map_err(|errors| {
            let error_msg = errors
                .into_iter()
                .map(|e| format!("{}", e.message))
                .collect::<Vec<_>>()
                .join(", ");
            eprintln!("Typst compilation errors: {}", error_msg);
            ExportError::FormatError {
                message: format!("Typst compilation failed: {}", error_msg),
            }
        })?;

        // Check for warnings
        let warnings = tracer.warnings();
        if !warnings.is_empty() {
            eprintln!("Typst compilation warnings:");
            for warning in warnings {
                eprintln!("  {}", warning.message);
            }
        }

        // Convert to PDF bytes
        let pdf_bytes = typst_pdf::pdf(&document, Smart::Auto, None);

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

    // Document setup
    content.push_str("#set document(\n");
    content.push_str(&format!("  title: \"{}\",\n", escape_typst(&novel.title)));
    if !novel.authors.is_empty() {
        content.push_str(&format!(
            "  author: ({}),\n",
            novel
                .authors
                .iter()
                .map(|a| format!("\"{}\"", escape_typst(a)))
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

    // Text formatting
    content.push_str("#set text(\n");
    content.push_str("  font: \"Linux Libertine\",\n");
    content.push_str("  size: 11pt,\n");
    content.push_str("  lang: \"en\",\n");
    content.push_str(")\n\n");

    // Paragraph formatting
    content.push_str("#set par(\n");
    content.push_str("  justify: true,\n");
    content.push_str("  first-line-indent: 1em,\n");
    content.push_str(")\n\n");

    // Heading formatting
    content.push_str("#show heading: it => {\n");
    content.push_str("  set text(font: \"Linux Biolinum\")\n");
    content.push_str("  it\n");
    content.push_str("}\n\n");

    // Title page
    content.push_str("#align(center)[\n");
    content.push_str(&format!("= {}\n\n", escape_typst(&novel.title)));

    if !novel.authors.is_empty() {
        content.push_str("#v(1em)\n");
        content.push_str(&format!(
            "#text(size: 14pt)[{}]\n\n",
            escape_typst(&novel.authors.join(", "))
        ));
    }

    if !novel.description.is_empty() {
        content.push_str("#v(2em)\n");
        content.push_str("#text(size: 12pt, style: \"italic\")[\n");
        content.push_str(&escape_typst(&novel.description.join(" ")));
        content.push_str("\n]\n\n");
    }
    content.push_str("]\n\n");

    // Page break before content
    content.push_str("#pagebreak()\n\n");

    // Table of contents
    if chapters.len() > 1 {
        content.push_str("#outline(\n");
        content.push_str("  title: \"Table of Contents\",\n");
        content.push_str("  indent: true,\n");
        content.push_str(")\n\n");
        content.push_str("#pagebreak()\n\n");
    }

    // Chapters
    for (index, (title, chapter_content)) in chapters.iter().enumerate() {
        if index > 0 {
            content.push_str("#pagebreak()\n\n");
        }

        // Chapter heading
        content.push_str(&format!("== {}\n\n", escape_typst(title)));

        // Convert HTML to Typst using Pandoc
        match html_to_typst(&chapter_content.data).await {
            Ok(typst_content) => {
                if !typst_content.trim().is_empty() {
                    // Clean up the Pandoc output to avoid file references
                    let cleaned_content = clean_pandoc_output(&typst_content);
                    content.push_str(&cleaned_content);
                } else {
                    content.push_str("_[No content available]_");
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to convert HTML to Typst for chapter '{}': {}",
                    title, e
                );
                eprintln!("Falling back to basic HTML sanitization...");
                // Fallback to sanitized HTML
                let chapter_text = sanitize_html(&chapter_content.data);
                if !chapter_text.trim().is_empty() {
                    content.push_str(&escape_typst(&chapter_text));
                } else {
                    content.push_str("_[No content available]_");
                }
            }
        }
        content.push_str("\n\n");
    }

    Ok(content)
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
        .replace('"', "\\\"")
}

/// Convert HTML content to Typst markup using Pandoc.
async fn html_to_typst(html: &str) -> Result<String> {
    // Check if user wants to skip Pandoc
    if std::env::var("QUELLE_NO_PANDOC").is_ok() {
        log::info!("Using HTML converter fallback (QUELLE_NO_PANDOC is set)");
        return use_fallback_converter(html);
    }

    // Check if pandoc is available
    let pandoc_check = Command::new("pandoc").arg("--version").output().await;

    if pandoc_check.is_err() {
        log::warn!("Pandoc is not available, falling back to built-in HTML converter");
        log::info!("For better conversion quality, install Pandoc:\n  • macOS: brew install pandoc\n  • Ubuntu/Debian: apt install pandoc\n  • Windows: Download from https://pandoc.org/installing.html");
        return use_fallback_converter(html);
    }

    // Create a temporary HTML file with basic structure
    let html_content = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Chapter</title>
</head>
<body>
{}
</body>
</html>"#,
        html
    );

    // Run pandoc to convert HTML to Typst
    let mut pandoc_cmd = Command::new("pandoc");
    pandoc_cmd
        .arg("--from=html")
        .arg("--to=typst")
        .arg("--wrap=none")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let mut child = pandoc_cmd.spawn().map_err(|e| ExportError::FormatError {
        message: format!("Failed to spawn pandoc process: {}", e),
    })?;

    // Write HTML content to pandoc's stdin
    if let Some(stdin) = child.stdin.as_mut() {
        use tokio::io::AsyncWriteExt;
        stdin
            .write_all(html_content.as_bytes())
            .await
            .map_err(|e| ExportError::FormatError {
                message: format!("Failed to write to pandoc stdin: {}", e),
            })?;
        stdin
            .shutdown()
            .await
            .map_err(|e| ExportError::FormatError {
                message: format!("Failed to close pandoc stdin: {}", e),
            })?;
    }

    // Wait for pandoc to complete and get output
    let output = child
        .wait_with_output()
        .await
        .map_err(|e| ExportError::FormatError {
            message: format!("Failed to wait for pandoc process: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ExportError::FormatError {
            message: format!("Pandoc conversion failed: {}", stderr),
        });
    }

    let typst_content = String::from_utf8_lossy(&output.stdout);
    Ok(typst_content.to_string())
}

/// Use the fallback HTML converter when Pandoc is not available
fn use_fallback_converter(html: &str) -> Result<String> {
    let mut converter = HtmlToTypstConverter::new();

    converter
        .convert(html)
        .map_err(|e| ExportError::FormatError {
            message: format!("HTML conversion failed: {}", e),
        })
}

/// Clean up Pandoc output to remove problematic elements.
fn clean_pandoc_output(content: &str) -> String {
    let mut result = content.to_string();

    // Remove nested #block[] structures that Pandoc sometimes generates
    while result.contains("#block[\n#block[") {
        result = result.replace("#block[\n#block[", "#block[");
        result = result.replace("]\n]", "]");
    }

    // Clean up excessive empty blocks
    result = result.replace("#block[\n]", "");
    result = result.replace("#block[\n\n]", "");

    // Handle image references and other file inclusions
    result
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("#figure(image(")
                || trimmed.starts_with("#image(")
                || trimmed.contains("image(\"")
            {
                Some("_[Image removed]_".to_string())
            } else if trimmed.is_empty() {
                None // Remove excessive empty lines
            } else {
                Some(line.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove HTML tags and convert to plain text (legacy fallback).
///
/// This function is kept for backward compatibility, but the new
/// HtmlToTypstConverter should be preferred as it provides much better
/// HTML parsing and Typst conversion.
pub fn sanitize_html(html: &str) -> String {
    // Use the new converter for better results
    match convert_html_to_typst(html) {
        Ok(typst_content) => typst_content,
        Err(_) => {
            // Fallback to simple tag removal if converter fails
            let re = regex::Regex::new(r"<[^>]*>").unwrap();
            let result = re.replace_all(html, "").to_string();

            // Basic entity decoding
            result
                .replace("&nbsp;", " ")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
                .replace("&quot;", "\"")
                .replace("&#39;", "'")
        }
    }
}

/// Minimal Typst World implementation for compiling documents.
struct TypstWorld {
    main_source: Source,
    library: Prehashed<Library>,
    font_book: Prehashed<FontBook>,
    fonts: Vec<Font>,
    files: HashMap<FileId, Source>,
}

impl TypstWorld {
    fn new(content: String, font_book: Prehashed<FontBook>, fonts: Vec<Font>) -> Self {
        let main_source = Source::new(FileId::new(None, VirtualPath::new("main.typ")), content);

        let library = Prehashed::new(Library::builder().build());
        let mut files = HashMap::new();
        files.insert(main_source.id(), main_source.clone());

        Self {
            main_source,
            library,
            font_book,
            fonts,
            files,
        }
    }
}

impl World for TypstWorld {
    fn library(&self) -> &Prehashed<Library> {
        &self.library
    }

    fn book(&self) -> &Prehashed<FontBook> {
        &self.font_book
    }

    fn main(&self) -> Source {
        self.main_source.clone()
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
    fn test_sanitize_html() {
        // The sanitize_html function now uses the HTML to Typst converter
        // which produces Typst markup instead of plain text
        let result = sanitize_html("<p>Hello <b>world</b>!</p>");
        assert!(result.contains("Hello"));
        assert!(result.contains("*world*")); // Bold becomes *text* in Typst

        let result2 = sanitize_html("Line 1<br/>Line 2");
        assert!(result2.contains("Line 1"));
        assert!(result2.contains("Line 2"));

        let result3 = sanitize_html("&amp; &lt;test&gt;");
        assert!(result3.contains("&"));
        // HTML entities are decoded by the parser, so < and > become literal characters
        // which then get escaped for Typst
        assert!(result3.contains("\\<test\\>") || result3.contains("<test>"));
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
