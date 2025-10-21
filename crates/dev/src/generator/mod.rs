//! Extension generator for creating new extensions from templates

use eyre::Result;
use std::collections::HashMap;

pub mod prompts;
pub mod templates;

use crate::utils::{fs, validation as util_validation};

/// Handle extension generation command
pub async fn handle(
    name: Option<String>,
    display_name: Option<String>,
    base_url: Option<String>,
    language: Option<String>,
    reading_direction: Option<String>,
    force: bool,
) -> Result<()> {
    let config = if should_use_interactive_mode(&name, &display_name, &base_url) {
        prompts::interactive_generation()?
    } else {
        ExtensionConfig::from_args(name, display_name, base_url, language, reading_direction)?
    };

    generate_extension(config, force).await
}

/// Configuration for generating a new extension
#[derive(Debug, Clone)]
pub struct ExtensionConfig {
    pub name: String,
    pub display_name: String,
    pub base_url: String,
    pub language: String,
    pub reading_direction: String,
}

impl ExtensionConfig {
    /// Create configuration from command line arguments
    pub fn from_args(
        name: Option<String>,
        display_name: Option<String>,
        base_url: Option<String>,
        language: Option<String>,
        reading_direction: Option<String>,
    ) -> Result<Self> {
        let name = name.ok_or_else(|| eyre::eyre!("Extension name is required"))?;
        let display_name = display_name.ok_or_else(|| eyre::eyre!("Display name is required"))?;
        let base_url = base_url.ok_or_else(|| eyre::eyre!("Base URL is required"))?;

        let name = util_validation::validate_extension_name(name)?;
        let display_name = util_validation::validate_display_name(display_name)?;
        let base_url = util_validation::validate_base_url(base_url)?;
        let language =
            util_validation::validate_language(language.unwrap_or_else(|| "en".to_string()))?;
        let reading_direction = util_validation::validate_reading_direction(
            reading_direction.unwrap_or_else(|| "ltr".to_string()),
        )?;

        Ok(Self {
            name,
            display_name,
            base_url,
            language,
            reading_direction,
        })
    }

    /// Create template replacements map
    pub fn to_replacements(&self) -> HashMap<String, String> {
        let mut replacements = HashMap::new();
        replacements.insert("EXTENSION_NAME".to_string(), self.name.clone());
        replacements.insert(
            "EXTENSION_DISPLAY_NAME".to_string(),
            self.display_name.clone(),
        );
        replacements.insert("BASE_URL".to_string(), self.base_url.clone());
        replacements.insert("LANGUAGE".to_string(), self.language.clone());
        replacements.insert(
            "READING_DIRECTION".to_string(),
            self.reading_direction.clone(),
        );
        replacements
    }
}

/// Generate a new extension with the given configuration
async fn generate_extension(config: ExtensionConfig, force: bool) -> Result<()> {
    let project_root = crate::utils::find_project_root(&std::env::current_dir()?)?;
    let extensions_dir = project_root.join("extensions");
    let output_dir = extensions_dir.join(&config.name);

    // Check if extension already exists
    if fs::exists(&output_dir) && !force
        && !prompts::confirm_overwrite(&config.name)? {
            println!("Error: Extension generation cancelled");
            return Ok(());
        }

    println!("Generating extension '{}'...", config.name);

    // Create output directory
    fs::create_dir_all(&output_dir)?;

    // Generate files
    generate_cargo_toml(&config, &output_dir).await?;
    generate_lib_rs(&config, &output_dir).await?;

    println!(
        "Success: Extension '{}' generated successfully!",
        config.name
    );
    println!("   Location: {}", output_dir.display());
    println!();
    println!("Next steps:");
    println!("   1. Edit the selectors in src/lib.rs");
    println!("   2. Build: just build-extension {}", config.name);
    println!("   3. Test: just dev-server {}", config.name);
    println!("   4. Publish: just publish {}", config.name);

    Ok(())
}

/// Generate Cargo.toml file
async fn generate_cargo_toml(config: &ExtensionConfig, output_dir: &std::path::Path) -> Result<()> {
    let replacements = config.to_replacements();
    let content = templates::create_cargo_toml_template(&replacements);

    let cargo_toml_path = output_dir.join("Cargo.toml");
    fs::write_file(&cargo_toml_path, content)?;
    println!("   ✓ Cargo.toml");

    Ok(())
}

/// Generate lib.rs file
async fn generate_lib_rs(config: &ExtensionConfig, output_dir: &std::path::Path) -> Result<()> {
    let replacements = config.to_replacements();
    let content = templates::create_lib_rs_template(&replacements);

    let src_dir = output_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    let lib_rs_path = src_dir.join("lib.rs");
    fs::write_file(&lib_rs_path, content)?;
    println!("   ✓ src/lib.rs");

    Ok(())
}

/// Determine if interactive mode should be used
fn should_use_interactive_mode(
    name: &Option<String>,
    display_name: &Option<String>,
    base_url: &Option<String>,
) -> bool {
    name.is_none() || display_name.is_none() || base_url.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_config_from_args() {
        let config = ExtensionConfig::from_args(
            Some("test_site".to_string()),
            Some("Test Site".to_string()),
            Some("https://example.com".to_string()),
            Some("en".to_string()),
            Some("ltr".to_string()),
        );

        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.name, "test_site");
        assert_eq!(config.display_name, "Test Site");
        assert_eq!(config.base_url, "https://example.com");
        assert_eq!(config.language, "en");
        assert_eq!(config.reading_direction, "Ltr");
    }

    #[test]
    fn test_extension_config_defaults() {
        let config = ExtensionConfig::from_args(
            Some("test_site".to_string()),
            Some("Test Site".to_string()),
            Some("https://example.com".to_string()),
            None, // Should default to "en"
            None, // Should default to "ltr"
        );

        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.language, "en");
        assert_eq!(config.reading_direction, "Ltr");
    }

    #[test]
    fn test_to_replacements() {
        let config = ExtensionConfig {
            name: "test_site".to_string(),
            display_name: "Test Site".to_string(),
            base_url: "https://example.com".to_string(),
            language: "en".to_string(),
            reading_direction: "Ltr".to_string(),
        };

        let replacements = config.to_replacements();
        assert_eq!(
            replacements.get("EXTENSION_NAME"),
            Some(&"test_site".to_string())
        );
        assert_eq!(
            replacements.get("EXTENSION_DISPLAY_NAME"),
            Some(&"Test Site".to_string())
        );
        assert_eq!(
            replacements.get("BASE_URL"),
            Some(&"https://example.com".to_string())
        );
        assert_eq!(replacements.get("LANGUAGE"), Some(&"en".to_string()));
        assert_eq!(
            replacements.get("READING_DIRECTION"),
            Some(&"Ltr".to_string())
        );
    }

    #[test]
    fn test_should_use_interactive_mode() {
        assert!(should_use_interactive_mode(&None, &None, &None));
        assert!(should_use_interactive_mode(
            &Some("test".to_string()),
            &None,
            &None
        ));
        assert!(!should_use_interactive_mode(
            &Some("test".to_string()),
            &Some("Test".to_string()),
            &Some("https://example.com".to_string())
        ));
    }
}
