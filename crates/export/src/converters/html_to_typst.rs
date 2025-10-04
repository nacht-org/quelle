//! HTML to Typst converter using proper HTML parsing
//!
//! This module provides a fallback HTML to Typst conversion when Pandoc is not available.
//! It uses the `scraper` crate for proper HTML parsing and maps HTML elements to their
//! Typst equivalents.

use scraper::{Html, Node, Selector};
use std::collections::HashMap;

/// Configuration for HTML to Typst conversion
#[derive(Debug, Clone)]
pub struct ConversionConfig {
    /// Whether to preserve whitespace
    pub preserve_whitespace: bool,
    /// Maximum heading level to convert (1-6)
    pub max_heading_level: u8,
    /// Whether to convert links to footnotes
    pub links_as_footnotes: bool,
    /// Custom element mappings
    pub custom_mappings: HashMap<String, String>,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            preserve_whitespace: false,
            max_heading_level: 6,
            links_as_footnotes: false,
            custom_mappings: HashMap::new(),
        }
    }
}

/// HTML to Typst converter
pub struct HtmlToTypstConverter {
    config: ConversionConfig,
    footnotes: Vec<String>,
}

impl HtmlToTypstConverter {
    /// Create a new converter with default configuration
    pub fn new() -> Self {
        Self {
            config: ConversionConfig::default(),
            footnotes: Vec::new(),
        }
    }

    /// Create a new converter with custom configuration
    pub fn with_config(config: ConversionConfig) -> Self {
        Self {
            config,
            footnotes: Vec::new(),
        }
    }

    /// Convert HTML string to Typst markup
    pub fn convert(&mut self, html: &str) -> Result<String, Box<dyn std::error::Error>> {
        // Reset footnotes for each conversion
        self.footnotes.clear();

        // Parse HTML
        let document = Html::parse_document(html);

        // Convert the document
        let mut result = String::new();
        self.convert_element(&document.root_element(), &mut result)?;

        // Add footnotes if any
        if !self.footnotes.is_empty() {
            result.push_str("\n\n");
            for (i, footnote) in self.footnotes.iter().enumerate() {
                result.push_str(&format!("#{} {}\n", i + 1, footnote));
            }
        }

        // Clean up excessive whitespace
        let result = self.clean_whitespace(&result);

        Ok(result)
    }

    /// Convert a single HTML element to Typst
    fn convert_element(
        &mut self,
        element: &scraper::ElementRef,
        result: &mut String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for child in element.children() {
            match child.value() {
                Node::Element(_elem) => {
                    let child_ref = scraper::ElementRef::wrap(child).unwrap();
                    self.convert_html_element(&child_ref, result)?;
                }
                Node::Text(text) => {
                    let text_content = text.text.trim();
                    if !text_content.is_empty() {
                        result.push_str(&self.escape_typst_text(text_content));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Convert specific HTML elements to Typst
    fn convert_html_element(
        &mut self,
        element: &scraper::ElementRef,
        result: &mut String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tag_name = element.value().name();

        match tag_name {
            // Headings
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag_name.chars().last().unwrap().to_digit(10).unwrap() as u8;
                if level <= self.config.max_heading_level {
                    result.push_str(&format!("\n{} ", "=".repeat(level as usize)));
                    self.convert_element(element, result)?;
                    result.push('\n');
                }
            }

            // Paragraphs
            "p" => {
                result.push('\n');
                self.convert_element(element, result)?;
                result.push_str("\n\n");
            }

            // Line break
            "br" => {
                result.push_str("\\\n");
            }

            // Emphasis and strong
            "em" | "i" => {
                result.push_str("_");
                self.convert_element(element, result)?;
                result.push_str("_");
            }
            "strong" | "b" => {
                result.push_str("*");
                self.convert_element(element, result)?;
                result.push_str("*");
            }

            // Code
            "code" => {
                result.push_str("`");
                self.convert_element(element, result)?;
                result.push_str("`");
            }
            "pre" => {
                result.push_str("\n```\n");
                self.convert_element(element, result)?;
                result.push_str("\n```\n");
            }

            // Lists
            "ul" => {
                result.push('\n');
                self.convert_list(element, result, false)?;
                result.push('\n');
            }
            "ol" => {
                result.push('\n');
                self.convert_list(element, result, true)?;
                result.push('\n');
            }
            "li" => {
                // This will be handled by convert_list
                self.convert_element(element, result)?;
            }

            // Links
            "a" => {
                if let Some(href) = element.value().attr("href") {
                    if self.config.links_as_footnotes {
                        result.push_str("[");
                        self.convert_element(element, result)?;
                        let footnote_num = self.footnotes.len() + 1;
                        result.push_str(&format!("]#{}", footnote_num));
                        self.footnotes.push(href.to_string());
                    } else {
                        result.push_str("#link(\"");
                        result.push_str(&self.escape_typst_text(href));
                        result.push_str("\")[");
                        self.convert_element(element, result)?;
                        result.push_str("]");
                    }
                } else {
                    self.convert_element(element, result)?;
                }
            }

            // Images
            "img" => {
                if let Some(alt) = element.value().attr("alt") {
                    result.push_str(&format!("_{}_", self.escape_typst_text(alt)));
                } else {
                    result.push_str("_[Image]_");
                }
            }

            // Tables (basic support)
            "table" => {
                result.push_str("\n#table(\n");
                result.push_str("  columns: auto,\n");
                self.convert_table(element, result)?;
                result.push_str(")\n");
            }

            // Blockquotes
            "blockquote" => {
                result.push_str("\n#quote[\n");
                self.convert_element(element, result)?;
                result.push_str("\n]\n");
            }

            // Horizontal rule
            "hr" => {
                result.push_str("\n#line(length: 100%)\n");
            }

            // Spans and divs - just process content
            "span" | "div" | "section" | "article" | "main" | "header" | "footer" | "nav" => {
                self.convert_element(element, result)?;
            }

            // Skip these elements entirely
            "script" | "style" | "meta" | "link" | "title" | "head" => {
                // Skip these elements
            }

            // Default: just process content
            _ => {
                // Check for custom mappings
                if let Some(mapping) = self.config.custom_mappings.get(tag_name) {
                    result.push_str(mapping);
                    self.convert_element(element, result)?;
                } else {
                    // Just process the content without wrapper
                    self.convert_element(element, result)?;
                }
            }
        }

        Ok(())
    }

    /// Convert HTML lists to Typst
    fn convert_list(
        &mut self,
        element: &scraper::ElementRef,
        result: &mut String,
        ordered: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let li_selector = Selector::parse("li").unwrap();
        let mut item_count = 1;

        for li in element.select(&li_selector) {
            if ordered {
                result.push_str(&format!("{}. ", item_count));
                item_count += 1;
            } else {
                result.push_str("- ");
            }

            let mut item_content = String::new();
            self.convert_element(&li, &mut item_content)?;

            // Clean up the item content and add it
            let item_content = item_content.trim();
            result.push_str(item_content);
            result.push('\n');
        }

        Ok(())
    }

    /// Convert HTML tables to Typst (basic support)
    fn convert_table(
        &mut self,
        element: &scraper::ElementRef,
        result: &mut String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tr_selector = Selector::parse("tr").unwrap();
        let td_selector = Selector::parse("td, th").unwrap();

        for tr in element.select(&tr_selector) {
            let cells: Vec<String> = tr
                .select(&td_selector)
                .map(|td| {
                    let mut cell_content = String::new();
                    self.convert_element(&td, &mut cell_content).unwrap_or(());
                    cell_content.trim().to_string()
                })
                .collect();

            if !cells.is_empty() {
                result.push_str("  ");
                for (i, cell) in cells.iter().enumerate() {
                    if i > 0 {
                        result.push_str(", ");
                    }
                    result.push_str(&format!("[{}]", cell));
                }
                result.push_str(",\n");
            }
        }

        Ok(())
    }

    /// Escape special Typst characters in text
    fn escape_typst_text(&self, text: &str) -> String {
        text.replace('\\', "\\\\")
            .replace('[', "\\[")
            .replace(']', "\\]")
            .replace('{', "\\{")
            .replace('}', "\\}")
            .replace('#', "\\#")
            .replace('$', "\\$")
            .replace('@', "\\@")
            .replace('<', "\\<")
            .replace('>', "\\>")
    }

    /// Clean up excessive whitespace
    fn clean_whitespace(&self, text: &str) -> String {
        if self.config.preserve_whitespace {
            return text.to_string();
        }

        // Remove excessive blank lines (more than 2 consecutive)
        let mut result = text.to_string();
        while result.contains("\n\n\n\n") {
            result = result.replace("\n\n\n\n", "\n\n\n");
        }

        // Clean up trailing whitespace on lines
        result = result
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n");

        // Ensure the document starts and ends cleanly
        result.trim().to_string()
    }
}

impl Default for HtmlToTypstConverter {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to convert HTML to Typst with default settings
pub fn convert_html_to_typst(html: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut converter = HtmlToTypstConverter::new();
    converter.convert(html)
}

/// Convenience function to convert HTML to Typst with custom configuration
pub fn convert_html_to_typst_with_config(
    html: &str,
    config: ConversionConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut converter = HtmlToTypstConverter::with_config(config);
    converter.convert(html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_conversion() {
        let html = r#"
            <p>This is a <strong>paragraph</strong> with <em>emphasis</em>.</p>
            <p>Another paragraph with a <a href="https://example.com">link</a>.</p>
        "#;

        let result = convert_html_to_typst(html).unwrap();
        assert!(result.contains("*paragraph*"));
        assert!(result.contains("_emphasis_"));
        assert!(result.contains("#link("));
    }

    #[test]
    fn test_headings() {
        let html = r#"
            <h1>Main Title</h1>
            <h2>Subtitle</h2>
            <h3>Section</h3>
        "#;

        let result = convert_html_to_typst(html).unwrap();
        assert!(result.contains("= Main Title"));
        assert!(result.contains("== Subtitle"));
        assert!(result.contains("=== Section"));
    }

    #[test]
    fn test_lists() {
        let html = r#"
            <ul>
                <li>First item</li>
                <li>Second item</li>
            </ul>
            <ol>
                <li>Numbered first</li>
                <li>Numbered second</li>
            </ol>
        "#;

        let result = convert_html_to_typst(html).unwrap();
        assert!(result.contains("- First item"));
        assert!(result.contains("- Second item"));
        assert!(result.contains("1. Numbered first"));
        assert!(result.contains("2. Numbered second"));
    }

    #[test]
    fn test_code() {
        let html = r#"
            <p>Inline <code>code</code> example.</p>
            <pre>Block code
example</pre>
        "#;

        let result = convert_html_to_typst(html).unwrap();
        assert!(result.contains("`code`"));
        assert!(result.contains("```"));
    }

    #[test]
    fn test_escape_special_chars() {
        let converter = HtmlToTypstConverter::new();
        let result = converter.escape_typst_text("Test [brackets] and #hash");
        assert_eq!(result, "Test \\[brackets\\] and \\#hash");
    }

    #[test]
    fn test_footnotes_config() {
        let html = r#"<p>Check this <a href="https://example.com">link</a>.</p>"#;

        let mut config = ConversionConfig::default();
        config.links_as_footnotes = true;

        let result = convert_html_to_typst_with_config(html, config).unwrap();
        assert!(result.contains("[link]#1"));
        assert!(result.contains("#1 https://example.com"));
    }

    #[test]
    fn test_blockquote() {
        let html = r#"<blockquote>This is a quote.</blockquote>"#;
        let result = convert_html_to_typst(html).unwrap();
        assert!(result.contains("#quote["));
    }

    #[test]
    fn test_image_alt_text() {
        let html = r#"<img src="image.jpg" alt="A beautiful sunset" />"#;
        let result = convert_html_to_typst(html).unwrap();
        assert!(result.contains("_A beautiful sunset_"));
    }
}
