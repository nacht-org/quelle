//! Filesystem utilities for development tools

use eyre::{Result, eyre};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Check if a file or directory exists
pub fn exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().exists()
}

/// Create directory and all parent directories if they don't exist
pub fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| eyre!("Failed to create directory: {}", e))
}

/// Write content to a file, creating parent directories if necessary
pub fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref();

    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }

    fs::write(path, contents).map_err(|e| eyre!("Failed to write file {}: {}", path.display(), e))
}

/// Read entire file content as a string
pub fn read_to_string<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|e| eyre!("Failed to read file {}: {}", path.display(), e))
}

/// Remove a file or directory recursively
pub fn remove_path<P: AsRef<Path>>(path: P) -> Result<()> {
    let path = path.as_ref();

    if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|e| eyre!("Failed to remove directory {}: {}", path.display(), e))
    } else if path.is_file() {
        fs::remove_file(path).map_err(|e| eyre!("Failed to remove file {}: {}", path.display(), e))
    } else {
        Err(eyre!("Path does not exist: {}", path.display()))
    }
}

/// Check if directory is empty
pub fn is_dir_empty<P: AsRef<Path>>(path: P) -> Result<bool> {
    let path = path.as_ref();

    if !path.is_dir() {
        return Err(eyre!("Path is not a directory: {}", path.display()));
    }

    let mut entries = fs::read_dir(path)
        .map_err(|e| eyre!("Failed to read directory {}: {}", path.display(), e))?;

    Ok(entries.next().is_none())
}

/// Copy a file from source to destination, creating parent directories if needed
pub fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<()> {
    let from = from.as_ref();
    let to = to.as_ref();

    if let Some(parent) = to.parent() {
        create_dir_all(parent)?;
    }

    fs::copy(from, to).map_err(|e| {
        eyre!(
            "Failed to copy {} to {}: {}",
            from.display(),
            to.display(),
            e
        )
    })?;

    Ok(())
}

/// Get file size in bytes
pub fn file_size<P: AsRef<Path>>(path: P) -> Result<u64> {
    let path = path.as_ref();
    let metadata = fs::metadata(path)
        .map_err(|e| eyre!("Failed to get metadata for {}: {}", path.display(), e))?;
    Ok(metadata.len())
}

/// Check if path has specific extension
pub fn has_extension<P: AsRef<Path>>(path: P, ext: &str) -> bool {
    path.as_ref()
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case(ext))
        .unwrap_or(false)
}

/// Find files with specific extension recursively
pub fn find_files_with_extension<P: AsRef<Path>>(dir: P, ext: &str) -> Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let mut files = Vec::new();

    if !dir.is_dir() {
        return Err(eyre!("Path is not a directory: {}", dir.display()));
    }

    fn visit_dir(dir: &Path, ext: &str, files: &mut Vec<PathBuf>) -> Result<()> {
        let entries = fs::read_dir(dir)
            .map_err(|e| eyre!("Failed to read directory {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry = entry.map_err(|e| eyre!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                visit_dir(&path, ext, files)?;
            } else if has_extension(&path, ext) {
                files.push(path);
            }
        }
        Ok(())
    }

    visit_dir(dir, ext, &mut files)?;
    Ok(files)
}

/// Prompt user for confirmation with a yes/no question
pub fn prompt_confirmation(message: &str) -> Result<bool> {
    print!("{} (y/N): ", message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let answer = input.trim().to_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

/// Prompt user for input with a message
pub fn prompt_input(message: &str) -> Result<String> {
    print!("{}: ", message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

/// Prompt user for input with a default value
pub fn prompt_input_with_default(message: &str, default: &str) -> Result<String> {
    print!("{} [{}]: ", message, default);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim();
    if input.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(input.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_has_extension() {
        assert!(has_extension("test.rs", "rs"));
        assert!(has_extension("test.RS", "rs"));
        assert!(has_extension("test.toml", "toml"));
        assert!(!has_extension("test.rs", "toml"));
        assert!(!has_extension("test", "rs"));
    }

    #[test]
    fn test_file_operations() -> Result<()> {
        let temp_dir = env::temp_dir().join("quelle_dev_test");
        let test_file = temp_dir.join("test.txt");
        let content = "Hello, world!";

        // Clean up first
        if exists(&temp_dir) {
            remove_path(&temp_dir)?;
        }

        // Test write_file (should create directories)
        write_file(&test_file, content)?;
        assert!(exists(&test_file));

        // Test read_to_string
        let read_content = read_to_string(&test_file)?;
        assert_eq!(read_content, content);

        // Test file_size
        let size = file_size(&test_file)?;
        assert_eq!(size, content.len() as u64);

        // Test is_dir_empty
        assert!(!is_dir_empty(&temp_dir)?); // Should not be empty (contains test.txt)

        // Clean up
        remove_path(&temp_dir)?;
        assert!(!exists(&temp_dir));

        Ok(())
    }
}
