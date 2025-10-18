//! Internal file operations trait and shared processor for file-based stores
//!
//! This module provides the core abstraction for reading files from different sources
//! (filesystem, HTTP, etc.) and a shared processor that implements common store
//! operations using these file operations.

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tracing::warn;

use crate::error::{Result, StoreError};
use crate::manifest::ExtensionManifest;
use crate::models::{ExtensionInfo, ExtensionMetadata, SearchQuery};
use crate::store_manifest::StoreManifest;

/// Internal trait for abstracting file operations across different store backends
#[async_trait]
pub(crate) trait FileOperations: Send + Sync {
    /// Read a file as bytes from the store
    async fn read_file(&self, path: &str) -> Result<Vec<u8>>;

    /// Check if a file exists
    async fn file_exists(&self, path: &str) -> Result<bool>;

    /// List files in a directory
    async fn list_directory(&self, path: &str) -> Result<Vec<String>>;
}

/// File metadata information
#[derive(Debug, Clone)]
pub(crate) struct FileMetadata {
    pub size: u64,
    pub is_directory: bool,
}

/// Shared processor for file-based store operations
///
/// This struct contains all the common logic for reading and processing
/// store files, regardless of where those files come from (filesystem, HTTP, etc.).
pub(crate) struct FileBasedProcessor<F: FileOperations> {
    file_ops: F,
    store_name: String,
}

impl<F: FileOperations> FileBasedProcessor<F> {
    /// Create a new file-based processor
    pub fn new(file_ops: F, store_name: String) -> Self {
        Self {
            file_ops,
            store_name,
        }
    }

    /// Get the file operations implementation
    pub fn file_ops(&self) -> &F {
        &self.file_ops
    }

    /// Get the store name
    pub fn store_name(&self) -> &str {
        &self.store_name
    }

    /// Read and parse a JSON file
    async fn read_json_file<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let content = self.file_ops.read_file(path).await?;
        serde_json::from_slice(&content).map_err(|e| {
            StoreError::ParseError(format!("Failed to parse JSON file {}: {}", path, e))
        })
    }

    /// Get the store manifest
    pub async fn get_store_manifest(&self) -> Result<StoreManifest> {
        self.read_json_file("store.json").await
    }

    /// Resolve version (get latest if None provided)
    async fn resolve_version(&self, extension_id: &str, version: Option<&str>) -> Result<String> {
        match version {
            Some(v) => Ok(v.to_string()),
            None => {
                let latest = self.get_extension_latest_version(extension_id).await?;
                latest.ok_or_else(|| {
                    StoreError::ExtensionNotFound(format!("No versions found for {}", extension_id))
                })
            }
        }
    }

    /// Get extension manifest
    pub async fn get_extension_manifest(
        &self,
        extension_id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionManifest> {
        let version = self.resolve_version(extension_id, version).await?;
        let manifest_path = format!("extensions/{}/{}/manifest.json", extension_id, version);
        self.read_json_file(&manifest_path).await
    }

    /// Get extension WASM with checksum verification
    pub async fn get_extension_wasm(
        &self,
        extension_id: &str,
        version: Option<&str>,
    ) -> Result<Vec<u8>> {
        let manifest = self.get_extension_manifest(extension_id, version).await?;
        let wasm_path = format!(
            "extensions/{}/{}/{}",
            extension_id, manifest.version, manifest.wasm_file.path
        );

        let wasm_bytes = self.file_ops.read_file(&wasm_path).await?;

        // Verify checksum using manifest's file reference
        if !manifest.wasm_file.verify(&wasm_bytes) {
            return Err(StoreError::ChecksumMismatch(format!(
                "WASM file checksum mismatch for {}@{}",
                extension_id, manifest.version
            )));
        }

        Ok(wasm_bytes)
    }

    /// Get extension metadata
    pub async fn get_extension_metadata(
        &self,
        extension_id: &str,
        version: Option<&str>,
    ) -> Result<Option<ExtensionMetadata>> {
        let version = self.resolve_version(extension_id, version).await?;
        let metadata_path = format!("extensions/{}/{}/metadata.json", extension_id, version);

        // Metadata is optional
        match self.file_ops.file_exists(&metadata_path).await? {
            true => {
                let metadata: ExtensionMetadata = self.read_json_file(&metadata_path).await?;
                Ok(Some(metadata))
            }
            false => Ok(None),
        }
    }

    /// Get extension asset
    pub async fn get_extension_asset(
        &self,
        extension_id: &str,
        version: Option<&str>,
        asset_path: &str,
    ) -> Result<Vec<u8>> {
        let manifest = self.get_extension_manifest(extension_id, version).await?;

        // Find the asset in the manifest
        let asset_ref = manifest
            .assets
            .iter()
            .find(|asset| asset.path == asset_path)
            .ok_or_else(|| {
                StoreError::ExtensionNotFound(format!(
                    "Asset {} not found in manifest for {}@{}",
                    asset_path, extension_id, manifest.version
                ))
            })?;

        let full_asset_path = format!(
            "extensions/{}/{}/{}",
            extension_id, manifest.version, asset_path
        );

        let asset_bytes = self.file_ops.read_file(&full_asset_path).await?;

        // Verify checksum
        if !asset_ref.verify(&asset_bytes) {
            return Err(StoreError::ChecksumMismatch(format!(
                "Asset {} checksum mismatch for {}@{}",
                asset_path, extension_id, manifest.version
            )));
        }

        Ok(asset_bytes)
    }

    /// List all extensions in the store
    pub async fn list_extensions(&self) -> Result<Vec<ExtensionInfo>> {
        let extensions_dir = "extensions";

        if !self.file_ops.file_exists(extensions_dir).await? {
            return Ok(Vec::new());
        }

        let extension_names = self.file_ops.list_directory(extensions_dir).await?;
        let mut all_extensions = Vec::new();

        for extension_name in extension_names {
            match self.get_extension_info(&extension_name).await {
                Ok(mut versions) => all_extensions.append(&mut versions),
                Err(e) => {
                    warn!(
                        "Failed to load extension info for {}: {}",
                        extension_name, e
                    );
                    continue;
                }
            }
        }

        Ok(all_extensions)
    }

    /// Get information about all versions of a specific extension
    pub async fn get_extension_info(&self, extension_id: &str) -> Result<Vec<ExtensionInfo>> {
        let extension_dir = format!("extensions/{}", extension_id);

        if !self.file_ops.file_exists(&extension_dir).await? {
            return Err(StoreError::ExtensionNotFound(extension_id.to_string()));
        }

        let versions = self.file_ops.list_directory(&extension_dir).await?;
        let mut extension_infos = Vec::new();

        for version in versions {
            match self
                .get_extension_version_info(extension_id, Some(&version))
                .await
            {
                Ok(info) => extension_infos.push(info),
                Err(e) => {
                    warn!(
                        "Failed to load version info for {}@{}: {}",
                        extension_id, version, e
                    );
                    continue;
                }
            }
        }

        Ok(extension_infos)
    }

    /// Get information about a specific version of an extension
    pub async fn get_extension_version_info(
        &self,
        extension_id: &str,
        version: Option<&str>,
    ) -> Result<ExtensionInfo> {
        let manifest = self.get_extension_manifest(extension_id, version).await?;
        let _metadata = self
            .get_extension_metadata(extension_id, Some(&manifest.version))
            .await?;

        Ok(ExtensionInfo {
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            description: None, // ExtensionManifest doesn't have description field
            author: manifest.author.clone(),
            tags: Vec::new(), // We could extract from metadata if available
            last_updated: None,
            download_count: None,
            size: None,
            homepage: None, // ExtensionManifest doesn't have homepage field
            repository: None,
            license: None,
            store_source: self.store_name.clone(),
        })
    }

    /// Search extensions matching the given query
    pub async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionInfo>> {
        let all_extensions = self.list_extensions().await?;

        let filtered: Vec<ExtensionInfo> = all_extensions
            .into_iter()
            .filter(|ext| {
                // Text search in name and description
                if let Some(text) = &query.text {
                    let text_lower = text.to_lowercase();
                    let matches_name = ext.name.to_lowercase().contains(&text_lower);
                    let matches_desc = ext
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&text_lower))
                        .unwrap_or(false);

                    if !matches_name && !matches_desc {
                        return false;
                    }
                }

                // Author filter
                if let Some(author) = &query.author {
                    if &ext.author != author {
                        return false;
                    }
                }

                // Tag filter
                if !query.tags.is_empty() {
                    let has_all_tags = query.tags.iter().all(|tag| ext.tags.contains(tag));
                    if !has_all_tags {
                        return false;
                    }
                }

                true
            })
            .collect();

        Ok(filtered)
    }

    /// Get the latest version for an extension
    pub async fn get_extension_latest_version(&self, extension_id: &str) -> Result<Option<String>> {
        let versions = self.list_extension_versions(extension_id).await?;

        if versions.is_empty() {
            return Ok(None);
        }

        // For now, use simple string sorting. In the future, we could use semver parsing
        let mut sorted_versions = versions;
        sorted_versions.sort();

        Ok(sorted_versions.last().cloned())
    }

    /// List all available versions for an extension
    pub async fn list_extension_versions(&self, extension_id: &str) -> Result<Vec<String>> {
        let extension_dir = format!("extensions/{}", extension_id);

        if !self.file_ops.file_exists(&extension_dir).await? {
            return Ok(Vec::new());
        }

        self.file_ops.list_directory(&extension_dir).await
    }

    /// Check if a specific version exists for an extension
    pub async fn check_extension_version_exists(
        &self,
        extension_id: &str,
        version: &str,
    ) -> Result<bool> {
        let version_dir = format!("extensions/{}/{}", extension_id, version);
        let manifest_path = format!("{}/manifest.json", version_dir);

        self.file_ops.file_exists(&manifest_path).await
    }

    /// Find extensions that can handle the given URL
    pub async fn find_extensions_for_url(&self, _url: &str) -> Result<Vec<(String, String)>> {
        // For the basic StoreManifest, we can't do URL routing
        // This would need to be implemented by stores that have URL pattern support
        // For now, return empty - individual stores can override this
        Ok(Vec::new())
    }

    /// List extension assets by type
    pub async fn list_extension_assets(
        &self,
        extension_id: &str,
        version: Option<&str>,
    ) -> Result<Vec<String>> {
        let manifest = self.get_extension_manifest(extension_id, version).await?;
        Ok(manifest
            .assets
            .iter()
            .map(|asset| asset.path.clone())
            .collect())
    }

    /// Get extension assets filtered by type
    pub async fn get_extension_assets_by_type(
        &self,
        extension_id: &str,
        version: Option<&str>,
        asset_type: &str,
    ) -> Result<HashMap<String, Vec<u8>>> {
        let manifest = self.get_extension_manifest(extension_id, version).await?;
        let mut assets = HashMap::new();

        for asset_ref in &manifest.assets {
            if &asset_ref.asset_type == asset_type {
                match self
                    .get_extension_asset(extension_id, Some(&manifest.version), &asset_ref.path)
                    .await
                {
                    Ok(content) => {
                        assets.insert(asset_ref.path.clone(), content);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to load asset {} for {}@{}: {}",
                            asset_ref.path, extension_id, manifest.version, e
                        );
                    }
                }
            }
        }

        Ok(assets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Mock file operations for testing
    struct MockFileOperations {
        files: HashMap<String, Vec<u8>>,
    }

    impl MockFileOperations {
        fn new() -> Self {
            Self {
                files: HashMap::new(),
            }
        }

        fn add_file(&mut self, path: &str, content: Vec<u8>) {
            self.files.insert(path.to_string(), content);
        }

        fn add_json_file<T: serde::Serialize>(&mut self, path: &str, content: &T) {
            let json = serde_json::to_vec(content).unwrap();
            self.add_file(path, json);
        }
    }

    #[async_trait]
    impl FileOperations for MockFileOperations {
        async fn read_file(&self, path: &str) -> Result<Vec<u8>> {
            self.files
                .get(path)
                .cloned()
                .ok_or_else(|| StoreError::ExtensionNotFound(path.to_string()))
        }

        async fn file_exists(&self, path: &str) -> Result<bool> {
            Ok(self.files.contains_key(path))
        }

        async fn list_directory(&self, path: &str) -> Result<Vec<String>> {
            let prefix = if path.ends_with('/') {
                path.to_string()
            } else {
                format!("{}/", path)
            };

            let mut entries = std::collections::HashSet::new();

            for file_path in self.files.keys() {
                if file_path.starts_with(&prefix) {
                    let relative = &file_path[prefix.len()..];
                    if let Some(first_part) = relative.split('/').next() {
                        if !first_part.is_empty() {
                            entries.insert(first_part.to_string());
                        }
                    }
                }
            }

            Ok(entries.into_iter().collect())
        }
    }

    #[tokio::test]
    async fn test_file_based_processor_basic() {
        let mut mock_ops = MockFileOperations::new();

        // Add a simple store manifest
        let store_manifest = StoreManifest::new(
            "test-store".to_string(),
            "test".to_string(),
            "1.0.0".to_string(),
        )
        .with_description("Test store".to_string());
        mock_ops.add_json_file("store.json", &store_manifest);

        let processor = FileBasedProcessor::new(mock_ops, "test-store".to_string());

        let manifest = processor.get_store_manifest().await.unwrap();
        assert_eq!(manifest.name, "test-store");
        assert_eq!(manifest.store_type, "test");
    }

    #[tokio::test]
    async fn test_list_extensions_empty() {
        let mock_ops = MockFileOperations::new();
        let processor = FileBasedProcessor::new(mock_ops, "test-store".to_string());

        let extensions = processor.list_extensions().await.unwrap();
        assert!(extensions.is_empty());
    }
}
