//! Internal file operations trait and shared processor for file-based stores
//!
//! This module provides the core abstraction for reading files from different sources
//! (filesystem, HTTP, etc.) and a shared processor that implements common store
//! operations using these file operations.

use semver::Version;
use serde::de::DeserializeOwned;
use std::borrow::Cow;

use tracing::{debug, warn};

use crate::error::{Result, StoreError};
use crate::manager::store_manifest::ExtensionVersion;
use crate::manager::store_manifest::StoreManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, SearchQuery, UpdateAvailableInfo,
    UpdateCheckFailedInfo, UpdateNotNeededInfo,
};
use crate::registry::manifest::{ExtensionManifest, FileReference, LocalExtensionManifest};
use crate::stores::impls::local::LocalStoreManifest;
use crate::{InstalledExtension, UpdateInfo};

/// Internal trait for abstracting file operations across different store backends
pub(crate) trait FileOperations: Send + Sync {
    /// Read a file as bytes from the store
    async fn read_file(&self, path: &str) -> Result<Vec<u8>>;

    /// Check if a file exists
    async fn file_exists(&self, path: &str) -> Result<bool>;

    /// List files in a directory
    async fn list_directory(&self, path: &str) -> Result<Vec<String>>;
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
    pub(crate) async fn read_json_file<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let content = self.file_ops.read_file(path).await?;
        serde_json::from_slice(&content).map_err(|e| {
            StoreError::ParseError(format!("Failed to parse JSON file {}: {}", path, e))
        })
    }

    /// Get the basic store manifest for BaseStore trait implementation
    pub async fn get_store_manifest(&self) -> Result<StoreManifest> {
        // Try to load as LocalStoreManifest first, then extract base
        match self
            .read_json_file::<LocalStoreManifest>("store.json")
            .await
        {
            Ok(local_manifest) => Ok(local_manifest.base),
            Err(_) => {
                // Fallback to basic StoreManifest
                self.read_json_file::<StoreManifest>("store.json")
                    .await
                    .map_err(|e| {
                        StoreError::ParseError(format!("Failed to load store manifest: {}", e))
                    })
            }
        }
    }

    /// Get the local store manifest for URL routing and extension listing
    pub async fn get_local_store_manifest(
        &self,
    ) -> Result<crate::stores::impls::local::store::LocalStoreManifest> {
        use crate::stores::impls::local::store::LocalStoreManifest;

        self.read_json_file::<LocalStoreManifest>("store.json")
            .await
            .map_err(|e| {
                StoreError::ParseError(format!("Failed to load local store manifest: {}", e))
            })
    }

    /// Resolve version (get latest if None provided)
    async fn resolve_version<'v>(
        &self,
        extension_id: &str,
        version: Option<&'v Version>,
    ) -> Result<Cow<'v, Version>> {
        match version {
            Some(v) => Ok(Cow::Borrowed(v)),
            None => {
                let latest = self.get_extension_latest_version(extension_id).await?;
                latest
                    .ok_or_else(|| {
                        StoreError::ExtensionNotFound(format!(
                            "No versions found for {}",
                            extension_id
                        ))
                    })
                    .map(Cow::Owned)
            }
        }
    }

    pub async fn get_extension_manifest(
        &self,
        extension_id: &str,
        version: Option<&Version>,
    ) -> Result<ExtensionManifest> {
        let manifest = self
            .get_local_extension_manifest(extension_id, version)
            .await?;

        Ok(manifest.into())
    }

    /// Get extension manifest
    pub async fn get_local_extension_manifest(
        &self,
        extension_id: &str,
        version: Option<&Version>,
    ) -> Result<LocalExtensionManifest> {
        let version = self.resolve_version(extension_id, version).await?;

        // Get the manifest path from the extension summary
        let summaries = self.list_extensions().await?;
        let manifest_path = summaries
            .iter()
            .find(|s| s.id == extension_id && &s.version == version.as_ref())
            .map(|s| s.manifest_path.clone())
            .ok_or_else(|| {
                StoreError::ExtensionNotFound(format!("{}@{}", extension_id, version))
            })?;

        self.read_json_file(&manifest_path).await
    }

    /// Get extension WASM with checksum verification
    pub async fn get_extension_wasm(
        &self,
        local_manifest: &LocalExtensionManifest,
    ) -> Result<Vec<u8>> {
        self.get_extension_file(local_manifest, &local_manifest.manifest.wasm_file)
            .await
    }

    async fn get_extension_file(
        &self,
        local_manifest: &LocalExtensionManifest,
        file: &FileReference,
    ) -> Result<Vec<u8>> {
        let file_path = local_manifest.path.join(&file.path);
        let file_path_str = file_path.to_str().ok_or_else(|| {
            StoreError::ParseError(format!("invalid file path for {}", file.path))
        })?;

        let file_bytes = self.file_ops.read_file(file_path_str).await?;

        // Verify checksum using manifest's file reference
        if !file.verify(&file_bytes) {
            return Err(StoreError::ChecksumMismatch(format!(
                "file checksum mismatch for {}@{}",
                local_manifest.manifest.id, local_manifest.manifest.version
            )));
        }

        Ok(file_bytes)
    }

    /// Get extension metadata
    pub async fn get_extension_metadata(
        &self,
        extension_id: &str,
        version: Option<&Version>,
    ) -> Result<Option<ExtensionMetadata>> {
        let version = self.resolve_version(extension_id, version).await?;

        // Get the manifest path from the extension summary
        let summaries = self.list_extensions().await?;
        let manifest_path = summaries
            .iter()
            .find(|s| s.id == extension_id && &s.version == version.as_ref())
            .map(|s| s.manifest_path.clone())
            .ok_or_else(|| {
                StoreError::ExtensionNotFound(format!("{}@{}", extension_id, version))
            })?;

        // File-based stores always use LocalExtensionManifest
        match self
            .read_json_file::<LocalExtensionManifest>(&manifest_path)
            .await
        {
            Ok(local_manifest) => Ok(local_manifest.metadata),
            Err(_) => Ok(None),
        }
    }

    /// List all extensions in the store
    pub async fn list_extensions(&self) -> Result<Vec<ExtensionVersion>> {
        let local_manifest = self.get_local_store_manifest().await?;
        Ok(local_manifest.get_latest_versions())
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
            let version = match Version::parse(&version) {
                Ok(v) => v,
                Err(e) => {
                    warn!(
                        "Invalid version format for {}@{}: {}",
                        extension_id, version, e
                    );
                    continue;
                }
            };

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
        version: Option<&Version>,
    ) -> Result<ExtensionInfo> {
        let manifest = self.get_extension_manifest(extension_id, version).await?;

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
    pub async fn search_extensions(&self, query: &SearchQuery) -> Result<Vec<ExtensionVersion>> {
        let all_extensions = self.list_extensions().await?;

        let filtered: Vec<ExtensionVersion> = all_extensions
            .into_iter()
            .filter(|ext| {
                // Text search in name and id
                if let Some(text) = &query.text {
                    let text_lower = text.to_lowercase();
                    let matches_name = ext.name.to_lowercase().contains(&text_lower);
                    let matches_id = ext.id.to_lowercase().contains(&text_lower);

                    if !matches_name && !matches_id {
                        return false;
                    }
                }

                // Language filter (using langs field from ExtensionSummary)
                if !query.tags.is_empty() {
                    // Treat tags as language filters for ExtensionSummary
                    let has_any_lang = query.tags.iter().any(|tag| ext.langs.contains(tag));
                    if !has_any_lang {
                        return false;
                    }
                }

                true
            })
            .collect();

        Ok(filtered)
    }

    /// Get the latest version for an extension
    pub async fn get_extension_latest_version(
        &self,
        extension_id: &str,
    ) -> Result<Option<Version>> {
        let manifest = self.get_local_store_manifest().await?;
        let latest_version = manifest
            .extensions
            .get(extension_id)
            .map(|versions| versions.latest.clone());
        Ok(latest_version)
    }

    /// List all available versions for an extension
    pub async fn list_extension_versions(&self, extension_id: &str) -> Result<Vec<Version>> {
        let manifest = self.get_local_store_manifest().await?;
        let Some(versions) = manifest.extensions.get(extension_id) else {
            return Err(StoreError::ExtensionNotFound(extension_id.to_string()));
        };

        let versions: Vec<Version> = versions.all_versions.keys().cloned().collect();

        Ok(versions)
    }

    /// Check if a specific version exists for an extension
    pub async fn check_extension_version_exists(
        &self,
        extension_id: &str,
        version: &Version,
    ) -> Result<bool> {
        let manifest = self.get_local_store_manifest().await?;
        let Some(versions) = manifest.extensions.get(extension_id) else {
            return Err(StoreError::ExtensionNotFound(extension_id.to_string()));
        };

        let version_exists = versions.all_versions.contains_key(version);

        Ok(version_exists)
    }

    /// Get the complete extension package including all files
    pub async fn get_extension_package(
        &self,
        id: &str,
        version: Option<&Version>,
        store_name: String,
    ) -> Result<ExtensionPackage> {
        let local_manifest = self.get_local_extension_manifest(id, version).await?;
        let wasm_bytes = self.get_extension_wasm(&local_manifest).await?;
        let metadata = local_manifest.metadata.clone();

        let mut package =
            ExtensionPackage::new(local_manifest.manifest.clone(), wasm_bytes, store_name);

        if let Some(meta) = metadata {
            package = package.with_metadata(meta);
        }

        // Load all assets
        for asset_ref in &local_manifest.manifest.assets {
            match self
                .get_extension_file(&local_manifest, &asset_ref.file)
                .await
            {
                Ok(content) => {
                    package.add_asset(asset_ref.file.path.clone(), content);
                }
                Err(e) => {
                    debug!("Failed to load asset {}: {}", asset_ref.file.path, e);
                    // Continue loading other assets
                }
            }
        }

        Ok(package)
    }

    pub async fn check_extension_updates(
        &self,
        installed: &[InstalledExtension],
        store_source: &str,
    ) -> Result<Vec<UpdateInfo>> {
        let mut results = Vec::new();

        for installed_ext in installed {
            let result = match self.get_extension_latest_version(&installed_ext.id).await {
                Ok(Some(latest_version)) => {
                    if latest_version > installed_ext.version {
                        UpdateInfo::UpdateAvailable(UpdateAvailableInfo {
                            extension_id: installed_ext.id.clone(),
                            current_version: installed_ext.version.clone(),
                            latest_version,
                            update_size: None,
                            store_source: store_source.to_string(),
                        })
                    } else {
                        UpdateInfo::NoUpdateNeeded(UpdateNotNeededInfo {
                            extension_id: installed_ext.id.clone(),
                            current_version: installed_ext.version.clone(),
                            store_source: store_source.to_string(),
                        })
                    }
                }
                Ok(None) => UpdateInfo::CheckFailed(UpdateCheckFailedInfo {
                    extension_id: installed_ext.id.clone(),
                    current_version: installed_ext.version.clone(),
                    store_source: store_source.to_string(),
                    error: "Extension not found in store".to_string(),
                }),
                Err(e) => UpdateInfo::CheckFailed(UpdateCheckFailedInfo {
                    extension_id: installed_ext.id.clone(),
                    current_version: installed_ext.version.clone(),
                    store_source: store_source.to_string(),
                    error: e.to_string(),
                }),
            };

            results.push(result);
        }

        Ok(results)
    }

    /// Find extensions that can handle the given URL
    pub async fn find_extensions_for_url(&self, url: &str) -> Result<Vec<(String, String)>> {
        match self.get_local_store_manifest().await {
            Ok(local_manifest) => Ok(local_manifest.find_extensions_for_url(url)),
            Err(_) => {
                // No LocalStoreManifest available, return empty
                Ok(Vec::new())
            }
        }
    }

    /// List extension assets by type
    pub async fn list_extension_assets(
        &self,
        extension_id: &str,
        version: Option<&Version>,
    ) -> Result<Vec<String>> {
        let manifest = self.get_extension_manifest(extension_id, version).await?;
        Ok(manifest
            .assets
            .iter()
            .map(|asset| asset.file.path.clone())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use crate::stores::impls::local::{
        index::{LocalStoreManifestIndex, UrlPattern},
        store::ExtensionVersions,
    };

    use super::*;
    use std::collections::{BTreeMap, HashMap};

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
            // Handle specific directory cases for testing
            match path {
                "extensions" => {
                    // Return extension names that exist
                    let mut extensions = Vec::new();
                    for file_path in self.files.keys() {
                        if file_path.starts_with("extensions/")
                            && file_path.contains("/manifest.json")
                        {
                            let parts: Vec<&str> = file_path.split('/').collect();
                            if parts.len() >= 2 {
                                let extension_name = parts[1];
                                if !extensions.contains(&extension_name.to_string()) {
                                    extensions.push(extension_name.to_string());
                                }
                            }
                        }
                    }
                    Ok(extensions)
                }
                path if path.starts_with("extensions/") => {
                    // Return versions for this extension
                    let extension_prefix = format!("{}/", path);
                    let mut versions = Vec::new();
                    for file_path in self.files.keys() {
                        if file_path.starts_with(&extension_prefix)
                            && file_path.ends_with("/manifest.json")
                        {
                            let remaining = &file_path[extension_prefix.len()..];
                            if let Some(slash_pos) = remaining.find('/') {
                                let version = &remaining[..slash_pos];
                                if !versions.contains(&version.to_string()) {
                                    versions.push(version.to_string());
                                }
                            }
                        }
                    }
                    Ok(versions)
                }
                _ => {
                    // Generic directory listing logic
                    let prefix = if path.ends_with('/') {
                        path.to_string()
                    } else {
                        format!("{}/", path)
                    };

                    let mut entries = std::collections::HashSet::new();

                    for file_path in self.files.keys() {
                        if file_path.starts_with(&prefix) {
                            let remaining = &file_path[prefix.len()..];
                            if let Some(slash_pos) = remaining.find('/') {
                                let first_part = &remaining[..slash_pos];
                                if !first_part.is_empty() {
                                    entries.insert(first_part.to_string());
                                }
                            }
                        }
                    }

                    Ok(entries.into_iter().collect())
                }
            }
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

    #[tokio::test]
    async fn test_find_extensions_for_url() {
        use crate::manager::store_manifest::{ExtensionVersion, StoreManifest};
        use crate::stores::impls::local::store::LocalStoreManifest;
        use std::collections::BTreeSet;

        let mut mock_ops = MockFileOperations::new();

        // Create a LocalStoreManifest with URL patterns
        let base_manifest = StoreManifest::new(
            "test-store".to_string(),
            "local".to_string(),
            "1.0.0".to_string(),
        );

        let mut url_pattern = UrlPattern {
            url_prefix: "https://example.com".to_string(),
            extensions: BTreeSet::new(),
        };
        url_pattern.extensions.insert("test-extension".to_string());

        let mut url_pattern2 = UrlPattern {
            url_prefix: "https://test.org".to_string(),
            extensions: BTreeSet::new(),
        };
        url_pattern2.extensions.insert("test-extension".to_string());

        let extension_summary = ExtensionVersion {
            id: "test-extension".to_string(),
            name: "Test Extension".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            base_urls: vec![
                "https://example.com".to_string(),
                "https://test.org".to_string(),
            ],
            langs: vec!["en".to_string()],
            last_updated: chrono::Utc::now(),
            manifest_path: "extensions/test-extension/1.0.0/manifest.json".to_string(),
            manifest_checksum: "test-checksum".to_string(),
        };

        let mut local_manifest = LocalStoreManifest {
            base: base_manifest,
            index: LocalStoreManifestIndex {
                url_patterns: vec![url_pattern, url_pattern2],
            },
            extensions: BTreeMap::new(),
        };

        let mut extension_versions = ExtensionVersions {
            latest: extension_summary.version.clone(),
            all_versions: BTreeMap::new(),
        };

        extension_versions
            .all_versions
            .insert(Version::parse("1.0.0").unwrap(), extension_summary);

        local_manifest
            .extensions
            .insert("test-extension".to_string(), extension_versions);

        // Add the LocalStoreManifest as store.json
        mock_ops.add_json_file("store.json", &local_manifest);

        let processor = FileBasedProcessor::new(mock_ops, "test-store".to_string());

        // Test matching URL
        let matches = processor
            .find_extensions_for_url("https://example.com/some/path")
            .await
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "test-extension");
        assert_eq!(matches[0].1, "Test Extension");

        // Test another matching URL
        let matches = processor
            .find_extensions_for_url("https://test.org/another/path")
            .await
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "test-extension");
        assert_eq!(matches[0].1, "Test Extension");

        // Test non-matching URL
        let matches = processor
            .find_extensions_for_url("https://nomatch.com/path")
            .await
            .unwrap();
        assert!(matches.is_empty());
    }
}
