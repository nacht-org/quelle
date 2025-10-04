//! Interactive prompts for extension generation

use eyre::Result;

use super::ExtensionConfig;
use crate::utils::{fs, validation};

/// Run interactive extension generation process
pub fn interactive_generation() -> Result<ExtensionConfig> {
    println!("🎯 Interactive Extension Generator");
    println!("Let's create a new Quelle extension together!");
    println!();

    let name = prompt_for_extension_name()?;
    let display_name = prompt_for_display_name(&name)?;
    let base_url = prompt_for_base_url()?;
    let language = prompt_for_language()?;
    let reading_direction = prompt_for_reading_direction()?;

    println!();
    println!("📋 Extension Summary:");
    println!("  Name: {}", name);
    println!("  Display Name: {}", display_name);
    println!("  Base URL: {}", base_url);
    println!("  Language: {}", language);
    println!("  Reading Direction: {}", reading_direction);
    println!();

    if !fs::prompt_confirmation("Generate extension with these settings?")? {
        return Err(eyre::eyre!("Extension generation cancelled by user"));
    }

    Ok(ExtensionConfig {
        name,
        display_name,
        base_url,
        language,
        reading_direction,
    })
}

/// Prompt user for extension name
pub fn prompt_for_extension_name() -> Result<String> {
    loop {
        let input = fs::prompt_input("📝 Extension name (lowercase, no spaces)")?;

        match validation::validate_extension_name(input) {
            Ok(name) => return Ok(name),
            Err(e) => {
                println!("❌ {}", e);
                println!("   Try again with only letters, numbers, and underscores");
                continue;
            }
        }
    }
}

/// Prompt user for display name with smart default
pub fn prompt_for_display_name(extension_name: &str) -> Result<String> {
    let suggested = extension_name
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    loop {
        let input = fs::prompt_input_with_default("✨ Display name for the extension", &suggested)?;

        match validation::validate_display_name(input) {
            Ok(name) => return Ok(name),
            Err(e) => {
                println!("❌ {}", e);
                continue;
            }
        }
    }
}

/// Prompt user for base URL
pub fn prompt_for_base_url() -> Result<String> {
    loop {
        let input = fs::prompt_input("🌐 Base URL (https://example.com)")?;

        if input.trim().is_empty() {
            println!("❌ Base URL cannot be empty");
            continue;
        }

        match validation::validate_base_url(input) {
            Ok(url) => return Ok(url),
            Err(e) => {
                println!("❌ {}", e);
                continue;
            }
        }
    }
}

/// Prompt user for language code with default
pub fn prompt_for_language() -> Result<String> {
    loop {
        let input = fs::prompt_input_with_default("🌍 Language code", "en")?;

        match validation::validate_language(input) {
            Ok(lang) => return Ok(lang),
            Err(e) => {
                println!("❌ {}", e);
                println!("   Use 2-letter ISO language codes like: en, ja, fr, de, es");
                continue;
            }
        }
    }
}

/// Prompt user for reading direction with default
pub fn prompt_for_reading_direction() -> Result<String> {
    loop {
        let input = fs::prompt_input_with_default("📖 Reading direction (ltr/rtl)", "ltr")?;

        match validation::validate_reading_direction(input) {
            Ok(dir) => return Ok(dir),
            Err(e) => {
                println!("❌ {}", e);
                println!("   Use 'ltr' for left-to-right or 'rtl' for right-to-left");
                continue;
            }
        }
    }
}

/// Confirm overwriting existing extension
pub fn confirm_overwrite(extension_name: &str) -> Result<bool> {
    println!("⚠️  Extension '{}' already exists.", extension_name);
    fs::prompt_confirmation("Do you want to overwrite it?")
}

/// Show completion message with next steps
pub fn show_completion_message(extension_name: &str, output_path: &std::path::Path) {
    println!();
    println!("🎉 Success! Your extension has been generated.");
    println!();
    println!("📁 Location: {}", output_path.display());
    println!();
    println!("🚀 Next Steps:");
    println!("   1. Open src/lib.rs and implement the TODO sections");
    println!("   2. Test your extension:");
    println!(
        "      cargo run -p quelle_cli -- dev server {}",
        extension_name
    );
    println!("   3. Build for production:");
    println!("      just build-extension {}", extension_name);
    println!();
    println!("💡 Tips:");
    println!("   • Use browser developer tools to find CSS selectors");
    println!("   • Test with the dev server's interactive commands");
    println!("   • Check existing extensions for reference patterns");
    println!();
    println!("📚 Documentation: docs/EXTENSION_DEVELOPMENT.md");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_display_name_suggestion() {
        // Simulate the display name generation logic
        let extension_name = "novel_updates";
        let suggested = extension_name
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        assert_eq!(suggested, "Novel Updates");
    }

    #[test]
    fn test_generate_display_name_single_word() {
        let extension_name = "scribblehub";
        let suggested = extension_name
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                    }
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        assert_eq!(suggested, "Scribblehub");
    }
}
