//! HTML to Typst markup converter for document export.
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
        self.footnotes.clear();
        let document = Html::parse_document(html);
        let mut result = String::new();
        self.convert_element(&document.root_element(), &mut result)?;
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
                result.push_str("#emph[");
                self.convert_element(element, result)?;
                result.push(']');
            }
            "strong" | "b" => {
                result.push_str("#strong[");
                self.convert_element(element, result)?;
                result.push(']');
            }

            // Code
            "code" => {
                result.push_str("#raw(\"");
                // For raw text, we need to get the text content directly without escaping
                let code_text = element.text().collect::<String>();
                result.push_str(&code_text.replace("\"", "\\\""));
                result.push_str("\")");
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
                        result.push('[');
                        self.convert_element(element, result)?;
                        let footnote_num = self.footnotes.len() + 1;
                        result.push_str(&format!("]#{}", footnote_num));
                        self.footnotes.push(href.to_string());
                    } else {
                        result.push_str("#link(\"");
                        result.push_str(&self.escape_typst_text(href));
                        result.push_str("\")[");
                        self.convert_element(element, result)?;
                        result.push(']');
                    }
                } else {
                    self.convert_element(element, result)?;
                }
            }

            // Images
            "img" => {
                if let Some(alt) = element.value().attr("alt") {
                    result.push_str(&format!("#emph[{}]", self.escape_typst_text(alt)));
                } else {
                    result.push_str("#emph[[Image]]");
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
        // Important: backslashes must be escaped first to avoid double-escaping
        // the escape sequences we add for other characters
        text.replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('*', "\\*")
            .replace('`', "\\`")
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
        assert!(result.contains("#strong[paragraph]"));
        assert!(result.contains("#emph[emphasis]"));
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
        assert!(result.contains("#raw(\"code\")"));
        assert!(result.contains("```"));
    }

    #[test]
    fn test_escape_special_chars() {
        let converter = HtmlToTypstConverter::new();
        let result = converter.escape_typst_text(
            "Test [brackets] and #hash with _underscores_ and *asterisks* plus `backticks`",
        );
        assert_eq!(result, "Test \\[brackets\\] and \\#hash with \\_underscores\\_ and \\*asterisks\\* plus \\`backticks\\`");
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
        assert!(result.contains("#emph[A beautiful sunset]"));
    }

    #[test]
    fn test_problematic_underscore_text() {
        // Test the specific case that was causing issues
        let html =
            r#"<p>[Fractured] was_hard_—the average stat for a healthy adult male was 1.0.</p>"#;
        let result = convert_html_to_typst(html).unwrap();

        // Should escape underscores in regular text
        assert!(result.contains("was\\_hard\\_—the"));
        // Should escape brackets
        assert!(result.contains("\\[Fractured\\]"));

        println!("Result: {}", result);
    }

    #[test]
    fn test_mixed_formatting_with_underscores() {
        let html = r#"<p>This has <em>emphasis_with_underscores</em> and normal_text_with_underscores.</p>"#;
        let result = convert_html_to_typst(html).unwrap();

        // Underscores in emphasized text should be escaped
        assert!(result.contains("#emph[emphasis\\_with\\_underscores]"));
        // Underscores in normal text should be escaped
        assert!(result.contains("normal\\_text\\_with\\_underscores"));

        println!("Mixed formatting result: {}", result);
    }

    #[test]
    fn test_html_parsing_vs_plain_text() {
        // Test 1: Pure HTML with the problematic text
        let html_wrapped = r#"<p>[Fractured] was_hard_—the average stat</p>"#;
        let result1 = convert_html_to_typst(html_wrapped).unwrap();
        println!("HTML wrapped: {}", result1);

        // Test 2: Mixed HTML and plain text (simulating real content)
        let mixed_content = r#"
        <div>
            <p>Some paragraph</p>
            [Fractured] was_hard_—the average stat for a healthy adult male was 1.0.
            <p>Another paragraph</p>
        </div>
        "#;
        let result2 = convert_html_to_typst(mixed_content).unwrap();
        println!("Mixed content: {}", result2);

        // Test 3: Plain text only
        let plain_text = "[Fractured] was_hard_—the average stat";
        let result3 = convert_html_to_typst(plain_text).unwrap();
        println!("Plain text: {}", result3);

        // All should have escaped underscores
        assert!(result1.contains("was\\_hard\\_"));
        assert!(result2.contains("was\\_hard\\_"));
        assert!(result3.contains("was\\_hard\\_"));
    }

    #[test]
    fn test_escape_function_directly() {
        let converter = HtmlToTypstConverter::new();

        // Test the exact problematic text
        let test_text = "[Fractured] was_hard_—the average stat";
        let escaped = converter.escape_typst_text(test_text);
        println!("Direct escape test:");
        println!("Input:  {}", test_text);
        println!("Output: {}", escaped);

        // Check each character type
        assert!(
            escaped.contains("\\[Fractured\\]"),
            "Brackets should be escaped"
        );
        assert!(
            escaped.contains("was\\_hard\\_"),
            "Underscores should be escaped"
        );

        // Test individual characters
        assert_eq!(converter.escape_typst_text("_"), "\\_");
        assert_eq!(converter.escape_typst_text("["), "\\[");
        assert_eq!(converter.escape_typst_text("]"), "\\]");
        assert_eq!(converter.escape_typst_text("*"), "\\*");
        assert_eq!(converter.escape_typst_text("`"), "\\`");
    }

    #[test]
    fn test_exact_problematic_html() {
        // Test HTML content that contains the problematic text
        let html = r#"<p>If it weren't for his current situation, Maria would've burst into birdsong from excitement.</p>
<p>[Fractured] was_hard_—the average stat for a healthy adult male was 1.0. Maria's starting attributes? Off the charts. Her race? Totally broken. Her talents? Literally game-changing.</p>"#;

        let result = convert_html_to_typst(html).unwrap();
        println!("Exact problematic HTML test:");
        println!("Input HTML (relevant part): [Fractured] was_hard_—the average stat...");
        println!("Result: {}", result);

        // Should have escaped underscores
        assert!(
            result.contains("was\\_hard\\_"),
            "Expected escaped underscores, got: {}",
            result
        );
        // Should have escaped brackets
        assert!(
            result.contains("\\[Fractured\\]"),
            "Expected escaped brackets, got: {}",
            result
        );
    }
}
