//! Filesystem utilities

use eyre::{Result, eyre};
use std::fs;
use std::path::Path;

pub fn exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().exists()
}

pub fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| eyre!("Failed to create directory: {}", e))
}

pub fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    fs::write(path, contents).map_err(|e| eyre!("Failed to write file {}: {}", path.display(), e))
}

pub fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|e| eyre!("Failed to read file {}: {}", path.display(), e))
}
