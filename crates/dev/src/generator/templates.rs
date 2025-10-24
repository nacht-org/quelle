//! Template generation for new extensions

use std::collections::HashMap;

/// Create Cargo.toml template content
pub fn create_cargo_toml_template(replacements: &HashMap<String, String>) -> String {
    let template = r#"[package]
name = "extension_{{EXTENSION_NAME}}"
version = "0.1.0"
edition = "2021"

[dependencies]
quelle_extension = { path = "../../crates/extension" }
chrono = { workspace = true }
once_cell = { workspace = true }
tracing = { workspace = true }
eyre = { workspace = true }

[lib]
crate-type = ["cdylib"]
"#;

    apply_replacements(template, replacements)
}

/// Create lib.rs template content with minimal implementation and todo!() macros
pub fn create_lib_rs_template(replacements: &HashMap<String, String>) -> String {
    let template = r#"use once_cell::sync::Lazy;
use quelle_extension::prelude::*;

register_extension!(Extension);

const BASE_URL: &str = "{{BASE_URL}}";

const META: Lazy<SourceMeta> = Lazy::new(|| SourceMeta {
    id: String::from("{{LANGUAGE}}.{{EXTENSION_NAME}}"),
    name: String::from("{{EXTENSION_DISPLAY_NAME}}"),
    langs: vec![String::from("{{LANGUAGE}}")],
    version: String::from(env!("CARGO_PKG_VERSION")),
    base_urls: vec![BASE_URL.to_string()],
    rds: vec![ReadingDirection::{{READING_DIRECTION}}],
    attrs: vec![],
    capabilities: SourceCapabilities {
        search: Some(SearchCapabilities {
            supports_simple_search: true,
            supports_complex_search: false,
            ..Default::default()
        }),
    },
});

pub struct Extension {
    client: Client,
}

impl QuelleExtension for Extension {
    fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn meta(&self) -> SourceMeta {
        META.clone()
    }

    fn fetch_novel_info(&self, url: String) -> Result<Novel, eyre::Report> {
        // TODO: Implement novel info scraping for your target website
        // 1. Make HTTP request to the URL
        // 2. Parse HTML response
        // 3. Extract novel information (title, authors, description, etc.)
        // 4. Extract chapters and organize into volumes
        // 5. Extract additional metadata (genres, tags, ratings, etc.)
        todo!("Implement novel info scraping for your target website")
    }

    fn fetch_chapter(&self, url: String) -> Result<ChapterContent, eyre::Report> {
        // TODO: Implement chapter content scraping for your target website
        // 1. Make HTTP request to the chapter URL
        // 2. Parse HTML response
        // 3. Extract chapter content using appropriate selectors
        // 4. Return ChapterContent with the extracted data
        todo!("Implement chapter content scraping for your target website")
    }

    fn simple_search(&self, query: SimpleSearchQuery) -> Result<SearchResult, eyre::Report> {
        // TODO: Implement search functionality for your target website
        // 1. Build search URL with query parameters
        // 2. Make HTTP request to search endpoint
        // 3. Parse HTML response
        // 4. Extract search results (novels list)
        // 5. Handle pagination if supported
        // 6. Return SearchResult with novels and pagination info
        todo!("Implement search functionality for your target website")
    }
}
"#;

    apply_replacements(template, replacements)
}

/// Apply template replacements to a template string
fn apply_replacements(template: &str, replacements: &HashMap<String, String>) -> String {
    let mut result = template.to_string();

    for (key, value) in replacements {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_replacements() -> HashMap<String, String> {
        let mut replacements = HashMap::new();
        replacements.insert("EXTENSION_NAME".to_string(), "test_site".to_string());
        replacements.insert(
            "EXTENSION_DISPLAY_NAME".to_string(),
            "Test Site".to_string(),
        );
        replacements.insert("BASE_URL".to_string(), "https://example.com".to_string());
        replacements.insert("LANGUAGE".to_string(), "en".to_string());
        replacements.insert("READING_DIRECTION".to_string(), "Ltr".to_string());
        replacements
    }

    #[test]
    fn test_apply_replacements() {
        let template = "Hello {{NAME}}, welcome to {{SITE}}!";
        let mut replacements = HashMap::new();
        replacements.insert("NAME".to_string(), "World".to_string());
        replacements.insert("SITE".to_string(), "Quelle".to_string());

        let result = apply_replacements(template, &replacements);
        assert_eq!(result, "Hello World, welcome to Quelle!");
    }

    #[test]
    fn test_cargo_toml_template() {
        let replacements = create_test_replacements();
        let content = create_cargo_toml_template(&replacements);

        assert!(content.contains("name = \"extension_test_site\""));
        assert!(content.contains("quelle_extension"));
        assert!(content.contains("crate-type = [\"cdylib\"]"));
    }

    #[test]
    fn test_lib_rs_template() {
        let replacements = create_test_replacements();
        let content = create_lib_rs_template(&replacements);

        assert!(content.contains("const BASE_URL: &str = \"https://example.com\";"));
        assert!(content.contains("id: String::from(\"en.test_site\")"));
        assert!(content.contains("name: String::from(\"Test Site\")"));
        assert!(content.contains("ReadingDirection::Ltr"));
        assert!(content.contains("todo!(\"Implement novel info scraping"));
        assert!(content.contains("todo!(\"Implement chapter content scraping"));
        assert!(content.contains("todo!(\"Implement search functionality"));
    }

    #[test]
    fn test_template_contains_todos() {
        let replacements = create_test_replacements();
        let content = create_lib_rs_template(&replacements);

        // Count the number of todo!() macros
        let todo_count = content.matches("todo!(").count();
        assert_eq!(todo_count, 3); // Should have exactly 3 todo!() macros
    }

    #[test]
    fn test_no_placeholder_left_behind() {
        let replacements = create_test_replacements();
        let cargo_content = create_cargo_toml_template(&replacements);
        let lib_content = create_lib_rs_template(&replacements);

        // Ensure no unreplaced placeholders remain
        assert!(!cargo_content.contains("{{"));
        assert!(!cargo_content.contains("}}"));
        assert!(!lib_content.contains("{{"));
        assert!(!lib_content.contains("}}"));
    }
}
