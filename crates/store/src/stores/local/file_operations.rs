//! Local filesystem file operations implementation

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

use crate::error::{Result, StoreError};
use crate::stores::file_operations::FileOperations;

/// File operations implementation for local filesystem access
pub(crate) struct LocalFileOperations {
    root_path: PathBuf,
}

impl LocalFileOperations {
    /// Create a new local file operations instance
    pub fn new<P: AsRef<Path>>(root_path: P) -> Self {
        Self {
            root_path: root_path.as_ref().to_path_buf(),
        }
    }

    /// Get the root path
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Convert a relative path to an absolute path within the root
    fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        // Normalize the path and prevent directory traversal
        let normalized = path.replace('\\', "/");
        let path_components: Vec<&str> = normalized
            .split('/')
            .filter(|component| !component.is_empty() && *component != ".")
            .collect();

        // Check for directory traversal attempts
        for component in &path_components {
            if *component == ".." {
                return Err(StoreError::InvalidPath(format!(
                    "Directory traversal not allowed: {}",
                    path
                )));
            }
        }

        let mut full_path = self.root_path.clone();
        for component in path_components {
            full_path.push(component);
        }

        Ok(full_path)
    }
}

#[async_trait]
impl FileOperations for LocalFileOperations {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let full_path = self.resolve_path(path)?;

        debug!(
            "Reading file from local filesystem: {}",
            full_path.display()
        );

        if !full_path.exists() {
            return Err(StoreError::ExtensionNotFound(format!(
                "File not found: {}",
                path
            )));
        }

        if !full_path.is_file() {
            return Err(StoreError::InvalidPath(format!(
                "Path is not a file: {}",
                path
            )));
        }

        fs::read(&full_path)
            .await
            .map_err(|e| StoreError::IoOperation {
                operation: "read file".to_string(),
                path: full_path.clone(),
                source: e,
            })
    }

    async fn file_exists(&self, path: &str) -> Result<bool> {
        let full_path = self.resolve_path(path)?;
        Ok(full_path.exists())
    }

    async fn list_directory(&self, path: &str) -> Result<Vec<String>> {
        let full_path = self.resolve_path(path)?;

        debug!("Listing directory: {}", full_path.display());

        if !full_path.exists() {
            return Ok(Vec::new());
        }

        if !full_path.is_dir() {
            return Err(StoreError::InvalidPath(format!(
                "Path is not a directory: {}",
                path
            )));
        }

        let mut entries = fs::read_dir(&full_path)
            .await
            .map_err(|e| StoreError::IoOperation {
                operation: "read directory".to_string(),
                path: full_path.clone(),
                source: e,
            })?;

        let mut result = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StoreError::IoOperation {
                operation: "read directory entry".to_string(),
                path: full_path.clone(),
                source: e,
            })?
        {
            if let Some(name) = entry.file_name().to_str() {
                // Skip hidden files and directories
                if !name.starts_with('.') {
                    result.push(name.to_string());
                }
            }
        }

        result.sort();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_read_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = b"Hello, World!";

        fs::write(&file_path, content).await.unwrap();

        let ops = LocalFileOperations::new(temp_dir.path());
        let result = ops.read_file("test.txt").await.unwrap();

        assert_eq!(result, content);
    }

    #[tokio::test]
    async fn test_read_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let ops = LocalFileOperations::new(temp_dir.path());

        let result = ops.read_file("nonexistent.txt").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        fs::write(&file_path, b"content").await.unwrap();

        let ops = LocalFileOperations::new(temp_dir.path());

        assert!(ops.file_exists("test.txt").await.unwrap());
        assert!(!ops.file_exists("nonexistent.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_list_directory() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("sub");

        fs::create_dir(&sub_dir).await.unwrap();
        fs::write(temp_dir.path().join("file1.txt"), b"content1")
            .await
            .unwrap();
        fs::write(temp_dir.path().join("file2.txt"), b"content2")
            .await
            .unwrap();
        fs::write(temp_dir.path().join(".hidden"), b"hidden")
            .await
            .unwrap();

        let ops = LocalFileOperations::new(temp_dir.path());
        let mut entries = ops.list_directory("").await.unwrap();
        entries.sort();

        assert_eq!(entries, vec!["file1.txt", "file2.txt", "sub"]);
    }

    #[tokio::test]
    async fn test_list_nonexistent_directory() {
        let temp_dir = TempDir::new().unwrap();
        let ops = LocalFileOperations::new(temp_dir.path());

        let result = ops.list_directory("nonexistent").await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_directory_traversal_protection() {
        let temp_dir = TempDir::new().unwrap();
        let ops = LocalFileOperations::new(temp_dir.path());

        let result = ops.read_file("../../../etc/passwd").await;
        assert!(result.is_err());

        let result = ops.read_file("sub/../../../etc/passwd").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_path_normalization() {
        let temp_dir = TempDir::new().unwrap();
        let ops = LocalFileOperations::new(temp_dir.path());

        // Test various path formats
        let path1 = ops.resolve_path("file.txt").unwrap();
        let path2 = ops.resolve_path("./file.txt").unwrap();
        let path3 = ops.resolve_path("sub/file.txt").unwrap();

        assert_eq!(path1, temp_dir.path().join("file.txt"));
        assert_eq!(path2, temp_dir.path().join("file.txt"));
        assert_eq!(path3, temp_dir.path().join("sub").join("file.txt"));
    }

    #[tokio::test]
    async fn test_read_directory_as_file_fails() {
        let temp_dir = TempDir::new().unwrap();
        let sub_dir = temp_dir.path().join("sub");

        fs::create_dir(&sub_dir).await.unwrap();

        let ops = LocalFileOperations::new(temp_dir.path());
        let result = ops.read_file("sub").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_file_as_directory_fails() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("file.txt");

        fs::write(&file_path, b"content").await.unwrap();

        let ops = LocalFileOperations::new(temp_dir.path());
        let result = ops.list_directory("file.txt").await;

        assert!(result.is_err());
    }
}
