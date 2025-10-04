//! Utility functions and helpers for development tools

use eyre::{Result, eyre};
use std::path::PathBuf;

pub mod debug;
pub mod fs;
pub mod validation;

/// Find the extension directory for a given extension name
pub fn find_extension_path(extension_name: &str) -> Result<PathBuf> {
    let extension_path = PathBuf::from("extensions").join(extension_name);
    if !extension_path.exists() {
        return Err(eyre!(
            "Extension '{}' not found in extensions/ directory",
            extension_name
        ));
    }
    Ok(extension_path)
}

/// Find the project root directory by looking for workspace Cargo.toml
pub fn find_project_root(start_dir: &std::path::Path) -> Result<std::path::PathBuf> {
    let mut current = start_dir;
    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") && content.contains("extensions") {
                    return Ok(current.to_path_buf());
                }
            }
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => return Err(eyre!("Could not find project root with workspace")),
        }
    }
}
