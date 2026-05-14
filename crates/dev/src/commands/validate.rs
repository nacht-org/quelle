//! Extension validation command

use eyre::{Result, eyre};
use std::path::Path;

use crate::utils::{find_extension_path, fs};

pub async fn handle(extension_name: String) -> Result<()> {
    println!("Validating extension '{}'", extension_name);

    let extension_path = find_extension_path(&extension_name)?;

    validate_directory_structure(&extension_path)?;
    validate_cargo_toml(&extension_path)?;
    validate_source_files(&extension_path)?;
    validate_build(&extension_name, &extension_path).await?;

    println!(
        "Extension '{}' passed all validation checks",
        extension_name
    );
    Ok(())
}

fn validate_directory_structure(extension_path: &Path) -> Result<()> {
    println!("Checking directory structure...");

    if !fs::exists(extension_path.join("Cargo.toml")) {
        return Err(eyre!("Missing Cargo.toml"));
    }
    if !fs::exists(extension_path.join("src")) {
        return Err(eyre!("Missing src directory"));
    }
    if !fs::exists(extension_path.join("src/lib.rs")) {
        return Err(eyre!("Missing src/lib.rs"));
    }

    println!("  Required files present");
    Ok(())
}

fn validate_cargo_toml(extension_path: &Path) -> Result<()> {
    println!("Checking Cargo.toml...");

    let content = fs::read_to_string(extension_path.join("Cargo.toml"))?;

    if !content.contains("[lib]") {
        return Err(eyre!("Cargo.toml is missing [lib] section"));
    }
    if !content.contains("crate-type = [\"cdylib\"]") {
        return Err(eyre!("Cargo.toml must specify crate-type = [\"cdylib\"]"));
    }
    if !content.contains("quelle_extension") {
        return Err(eyre!("Cargo.toml is missing quelle_extension dependency"));
    }

    println!("  Cargo.toml valid");
    Ok(())
}

fn validate_source_files(extension_path: &Path) -> Result<()> {
    println!("Checking source files...");

    let content = fs::read_to_string(extension_path.join("src/lib.rs"))?;

    if !content.contains("register_extension!") {
        return Err(eyre!("lib.rs is missing register_extension! macro"));
    }
    if !content.contains("impl QuelleExtension") {
        return Err(eyre!("lib.rs is missing QuelleExtension implementation"));
    }

    let todo_count = content.matches("todo!(").count();
    if todo_count > 0 {
        println!(
            "  Warning: {} todo!() macro(s) not yet implemented",
            todo_count
        );
    } else {
        println!("  Source files valid");
    }

    Ok(())
}

async fn validate_build(_extension_name: &str, extension_path: &Path) -> Result<()> {
    println!("Checking build...");

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

    println!("  Extension builds successfully");
    Ok(())
}
