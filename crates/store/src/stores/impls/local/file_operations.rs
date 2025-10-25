//! Local filesystem file operations implementation

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
        // Canonicalize the root path to resolve any symlinks and get absolute path
        let canonical_root = root_path
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| root_path.as_ref().to_path_buf());

        Self {
            root_path: canonical_root,
        }
    }

    /// Get the root path
    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Convert a relative path to an absolute path within the root
    /// This function ensures no directory traversal is possible
    fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        // Reject any path that contains null bytes (security issue)
        if path.contains('\0') {
            return Err(StoreError::InvalidPath(
                "Path contains null bytes".to_string(),
            ));
        }

        // Handle absolute paths - check if they're within root after canonicalization
        let path_buf = PathBuf::from(path);
        let target_path = if path_buf.is_absolute() {
            // This is an absolute path - use it directly and check bounds later
            path_buf
        } else {
            // Relative path - build from root
            self.build_relative_path(path)?
        };

        // Canonicalize and verify it's within root bounds
        self.verify_path_within_root(target_path, path)
    }

    /// Build a relative path from components, rejecting traversal attempts
    fn build_relative_path(&self, path: &str) -> Result<PathBuf> {
        // Handle empty path - should resolve to root directory
        if path.is_empty() {
            return Ok(self.root_path.clone());
        }

        let mut full_path = self.root_path.clone();

        // Normalize separators and split into components
        let normalized = path.replace('\\', "/");
        for component in normalized.split('/') {
            // Skip empty components and current directory references
            if component.is_empty() || component == "." {
                continue;
            }

            // Explicitly reject parent directory references
            if component == ".." {
                return Err(StoreError::InvalidPath(format!(
                    "Directory traversal not allowed: {}",
                    path
                )));
            }

            // Add the component
            full_path.push(component);
        }

        Ok(full_path)
    }

    /// Verify that a path (absolute or relative) is within the root directory
    fn verify_path_within_root(
        &self,
        target_path: PathBuf,
        original_path: &str,
    ) -> Result<PathBuf> {
        // Try to canonicalize the target path
        if let Ok(canonical) = target_path.canonicalize() {
            // Check if the canonical path is within root
            if canonical.starts_with(&self.root_path) {
                Ok(canonical)
            } else {
                Err(StoreError::InvalidPath(format!(
                    "Path resolves outside root directory: {}",
                    original_path
                )))
            }
        } else {
            // If canonicalization fails (file doesn't exist), check the path structure
            if target_path.is_absolute() {
                // For absolute paths that don't exist, check if they would be within root
                if target_path.starts_with(&self.root_path) {
                    Ok(target_path)
                } else {
                    Err(StoreError::InvalidPath(format!(
                        "Absolute path outside root directory: {}",
                        original_path
                    )))
                }
            } else {
                // For relative paths, ensure they start with root (they should after build_relative_path)
                if target_path.starts_with(&self.root_path) {
                    Ok(target_path)
                } else {
                    Err(StoreError::InvalidPath(format!(
                        "Path outside root directory: {}",
                        original_path
                    )))
                }
            }
        }
    }
}

impl FileOperations for LocalFileOperations {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
        let full_path = self.resolve_path(path)?;

        debug!(
            "Reading file from local filesystem: {}",
            full_path.display()
        );

        if !full_path.exists() {
            return Err(StoreError::FileNotFound(format!(
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

        // Test basic parent directory traversal
        let result = ops.read_file("../../../etc/passwd").await;
        assert!(result.is_err());

        let result = ops.read_file("sub/../../../etc/passwd").await;
        assert!(result.is_err());

        // Test various other traversal attempts
        let result = ops
            .read_file("..\\..\\..\\windows\\system32\\config\\sam")
            .await;
        assert!(result.is_err());

        // Test absolute paths outside root (should fail)
        let result = ops.read_file("/etc/passwd").await;
        assert!(result.is_err());

        let result = ops.read_file("C:\\windows\\system32\\config\\sam").await;
        assert!(result.is_err());

        // Test absolute path that would be outside root even if it existed
        let outside_absolute = "/tmp/outside_file.txt";
        let result = ops.read_file(outside_absolute).await;
        assert!(result.is_err());

        // Test null byte injection
        let result = ops.read_file("file.txt\0../../../etc/passwd").await;
        assert!(result.is_err());

        // Test URL-encoded attempts
        let result = ops.read_file("..%2F..%2F..%2Fetc%2Fpasswd").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_path_normalization() {
        let temp_dir = TempDir::new().unwrap();
        let ops = LocalFileOperations::new(temp_dir.path());

        // Test various path formats - just verify they're within root
        let path1 = ops.resolve_path("file.txt").unwrap();
        let path2 = ops.resolve_path("./file.txt").unwrap();
        let path3 = ops.resolve_path("sub/file.txt").unwrap();

        // All paths should be within the root directory
        assert!(path1.starts_with(&ops.root_path()));
        assert!(path2.starts_with(&ops.root_path()));
        assert!(path3.starts_with(&ops.root_path()));

        // Paths should end with expected file names
        assert!(path1.ends_with("file.txt"));
        assert!(path2.ends_with("file.txt"));
        assert!(path3.ends_with("file.txt"));
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

    #[tokio::test]
    async fn test_symlink_security() {
        let temp_dir = TempDir::new().unwrap();
        let ops = LocalFileOperations::new(temp_dir.path());

        // Create a file outside the temp directory
        let outside_dir = TempDir::new().unwrap();
        let outside_file = outside_dir.path().join("secret.txt");
        fs::write(&outside_file, b"secret content").await.unwrap();

        // Try to create a symlink inside temp_dir that points outside
        let symlink_path = temp_dir.path().join("malicious_link");

        // On Unix systems, test symlink creation and access
        #[cfg(unix)]
        {
            if let Ok(_) = std::os::unix::fs::symlink(&outside_file, &symlink_path) {
                // The symlink exists, but accessing it through our API should fail
                let result = ops.read_file("malicious_link").await;
                assert!(
                    result.is_err(),
                    "Should not be able to read file via symlink pointing outside root"
                );
            }
        }

        // Test that we can't traverse via symlinked directories either
        let symlink_dir = temp_dir.path().join("malicious_dir");

        #[cfg(unix)]
        {
            if let Ok(_) = std::os::unix::fs::symlink(&outside_dir.path(), &symlink_dir) {
                let result = ops.read_file("malicious_dir/secret.txt").await;
                assert!(
                    result.is_err(),
                    "Should not be able to access files in symlinked directory outside root"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_edge_case_paths() {
        let temp_dir = TempDir::new().unwrap();
        let ops = LocalFileOperations::new(temp_dir.path());

        // Test empty path - should resolve to root but fail to read as file
        let result = ops.read_file("").await;
        assert!(result.is_err());

        // Test path with only dots and slashes
        let result = ops.read_file("./././.").await;
        assert!(result.is_err());

        // Test very long path with traversal attempts
        let long_path = "../".repeat(1000) + "etc/passwd";
        let result = ops.read_file(&long_path).await;
        assert!(result.is_err());

        // Test mixed separators
        let result = ops.read_file("sub\\..//..\\etc/passwd").await;
        assert!(result.is_err());

        // Test Unicode normalization attacks (if applicable)
        let result = ops.read_file("..\\u002e\\u002e\\etc\\passwd").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_canonicalization_security() {
        let temp_dir = TempDir::new().unwrap();

        // Create a subdirectory and file within the allowed area
        let sub_dir = temp_dir.path().join("sub");
        fs::create_dir(&sub_dir).await.unwrap();
        let allowed_file = sub_dir.join("allowed.txt");
        fs::write(&allowed_file, b"allowed content").await.unwrap();

        let ops = LocalFileOperations::new(temp_dir.path());

        // This should work - accessing file within root
        let result = ops.read_file("sub/allowed.txt").await;
        assert!(result.is_ok());

        // Test that resolve_path properly canonicalizes and stays within bounds
        let resolved = ops.resolve_path("sub/allowed.txt").unwrap();
        assert!(resolved.starts_with(&ops.root_path()));
        assert!(resolved.ends_with("allowed.txt"));

        // Test absolute path that points within root (should work)
        let absolute_path = allowed_file.to_string_lossy();
        let result = ops.read_file(&absolute_path).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"allowed content");
    }

    #[tokio::test]
    async fn test_absolute_paths_within_root() {
        let temp_dir = TempDir::new().unwrap();
        let ops = LocalFileOperations::new(temp_dir.path());

        // Create a file within the root
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"test content").await.unwrap();

        // Test absolute path that's within root - should work
        let absolute_path = file_path.to_string_lossy();
        let result = ops.read_file(&absolute_path).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"test content");

        // Test absolute path that's outside root - should fail
        let outside_path = "/etc/passwd";
        let result = ops.read_file(outside_path).await;
        assert!(result.is_err());
    }
}
