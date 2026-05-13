//! Generate command for creating new extensions from template

use eyre::Result;

use crate::generator::templates;
use crate::utils::{find_project_root, fs, validation};

pub async fn handle(
    name: String,
    display_name: String,
    base_url: String,
    language: String,
    reading_direction: String,
    force: bool,
) -> Result<()> {
    let name = validation::validate_extension_name(name)?;
    let display_name = validation::validate_display_name(display_name)?;
    let base_url = validation::validate_base_url(base_url)?;
    let language = validation::validate_language(language)?;
    let reading_direction = validation::validate_reading_direction(reading_direction)?;

    let project_root = find_project_root(&std::env::current_dir()?)?;
    let output_dir = project_root.join("extensions").join(&name);

    if fs::exists(&output_dir) && !force {
        eprintln!(
            "Extension '{}' already exists. Use --force to overwrite.",
            name
        );
        return Ok(());
    }

    println!("Generating extension '{}'...", name);

    let mut replacements = std::collections::HashMap::new();
    replacements.insert("EXTENSION_NAME".to_string(), name.clone());
    replacements.insert("EXTENSION_DISPLAY_NAME".to_string(), display_name);
    replacements.insert("BASE_URL".to_string(), base_url);
    replacements.insert("LANGUAGE".to_string(), language);
    replacements.insert("READING_DIRECTION".to_string(), reading_direction);

    fs::create_dir_all(&output_dir)?;

    let cargo_content = templates::create_cargo_toml_template(&replacements);
    fs::write_file(&output_dir.join("Cargo.toml"), cargo_content)?;
    println!("  Cargo.toml");

    let src_dir = output_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    let lib_content = templates::create_lib_rs_template(&replacements);
    fs::write_file(&src_dir.join("lib.rs"), lib_content)?;
    println!("  src/lib.rs");

    println!("Extension '{}' generated at {}", name, output_dir.display());
    println!();
    println!("Next steps:");
    println!("  1. Edit src/lib.rs and implement the TODO sections");
    println!("  2. Test: quelle_dev test {} --url <URL>", name);
    println!("  3. Validate: quelle_dev validate {}", name);

    Ok(())
}
