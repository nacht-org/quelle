//! Extension validation commands for ensuring code quality and correctness

use eyre::{Result, eyre};
use std::path::Path;

use crate::server::DevServer;
use crate::utils::{find_extension_path, fs};

/// Handle extension validation command
pub async fn handle(extension_name: String, extended: bool) -> Result<()> {
    println!("Validating extension '{}'", extension_name);

    let extension_path = find_extension_path(&extension_name)?;

    // Basic validation checks
    validate_directory_structure(&extension_path)?;
    validate_cargo_toml(&extension_path)?;
    validate_source_files(&extension_path)?;

    // Build validation
    validate_build(&extension_name, &extension_path).await?;

    if extended {
        println!("Running extended validation...");
        validate_extension_runtime(&extension_name, &extension_path).await?;
        validate_naming_conventions(&extension_path)?;
    }

    println!(
        "Success: Extension '{}' passed all validation checks",
        extension_name
    );
    Ok(())
}

/// Validate that the extension directory has the correct structure
fn validate_directory_structure(extension_path: &Path) -> Result<()> {
    println!("Checking directory structure...");

    // Check required files exist
    let cargo_toml = extension_path.join("Cargo.toml");
    if !fs::exists(&cargo_toml) {
        return Err(eyre!("Missing Cargo.toml file"));
    }

    let src_dir = extension_path.join("src");
    if !fs::exists(&src_dir) {
        return Err(eyre!("Missing src directory"));
    }

    let lib_rs = src_dir.join("lib.rs");
    if !fs::exists(&lib_rs) {
        return Err(eyre!("Missing src/lib.rs file"));
    }

    println!("   ✓ Required files present");
    Ok(())
}

/// Validate Cargo.toml configuration
fn validate_cargo_toml(extension_path: &Path) -> Result<()> {
    println!("Checking Cargo.toml configuration...");

    let cargo_toml_path = extension_path.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path)?;

    // Check for required sections
    if !content.contains("[lib]") {
        return Err(eyre!("Cargo.toml is missing [lib] section"));
    }

    if !content.contains("crate-type = [\"cdylib\"]") {
        return Err(eyre!("Cargo.toml must specify crate-type = [\"cdylib\"]"));
    }

    if !content.contains("quelle_extension") {
        return Err(eyre!("Cargo.toml is missing quelle_extension dependency"));
    }

    // Check package metadata
    if !content.contains("[package.metadata.component]") {
        return Err(eyre!("Cargo.toml is missing component metadata"));
    }

    println!("   ✓ Cargo.toml configuration valid");
    Ok(())
}

/// Validate source files for basic correctness
fn validate_source_files(extension_path: &Path) -> Result<()> {
    println!("🦀 Checking source files...");

    let lib_rs_path = extension_path.join("src/lib.rs");
    let content = fs::read_to_string(&lib_rs_path)?;

    // Check for required imports and macros
    if !content.contains("register_extension!") {
        return Err(eyre!("lib.rs is missing register_extension! macro"));
    }

    if !content.contains("impl QuelleExtension") {
        return Err(eyre!("lib.rs is missing QuelleExtension implementation"));
    }

    // Check for required methods
    let required_methods = [
        "fn new()",
        "fn meta()",
        "fn fetch_novel_info(",
        "fn fetch_chapter(",
        "fn simple_search(",
    ];

    for method in &required_methods {
        if !content.contains(method) {
            return Err(eyre!("lib.rs is missing required method: {}", method));
        }
    }

    // Check for todo!() macros (should exist in template-generated extensions)
    let todo_count = content.matches("todo!(").count();
    if todo_count > 0 {
        println!(
            "   Warning: Found {} todo!() macros - remember to implement these",
            todo_count
        );
    }

    println!("   ✓ Source files structure valid");
    Ok(())
}

/// Validate that the extension builds successfully
async fn validate_build(_extension_name: &str, extension_path: &Path) -> Result<()> {
    println!("Checking build process...");

    // Try to build the extension
    let output = tokio::process::Command::new("cargo")
        .args([
            "check",
            "--manifest-path",
            &format!("{}/Cargo.toml", extension_path.display()),
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Extension fails to compile:\n{}", stderr));
    }

    // Try WASM build
    let output = tokio::process::Command::new("cargo")
        .args([
            "component",
            "build",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
            &format!("{}/Cargo.toml", extension_path.display()),
        ])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(eyre!("Extension fails to build for WASM:\n{}", stderr));
    }

    println!("   ✓ Extension builds successfully");
    Ok(())
}

/// Validate extension runtime behavior (extended validation)
async fn validate_extension_runtime(extension_name: &str, extension_path: &Path) -> Result<()> {
    println!("Checking runtime behavior...");

    // Create a dev server to test the extension
    let mut dev_server = DevServer::new(
        extension_name.to_string(),
        extension_path.to_path_buf(),
        false, // Don't use Chrome for validation
    )
    .await?;

    // Build and load the extension
    dev_server.build_extension().await?;
    dev_server.load_extension().await?;

    println!("   Warning: Runtime validation not fully implemented yet");
    println!("   Success: Extension loads without crashes");
    Ok(())
}

/// Validate extension metadata against naming conventions
fn validate_naming_conventions(extension_path: &Path) -> Result<()> {
    let cargo_toml_path = extension_path.join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path)?;

    // Extract package name
    let package_name = content
        .lines()
        .find(|line| line.starts_with("name = "))
        .and_then(|line| line.split('"').nth(1))
        .ok_or_else(|| eyre!("Could not find package name in Cargo.toml"))?;

    // Validate naming convention
    if !package_name.starts_with("extension_") {
        return Err(eyre!("Package name should start with 'extension_'"));
    }

    let extension_name = package_name.strip_prefix("extension_").unwrap();

    // Check extension name format
    if !extension_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
    {
        return Err(eyre!(
            "Extension name should only contain alphanumeric characters and underscores"
        ));
    }

    Ok(())
}
