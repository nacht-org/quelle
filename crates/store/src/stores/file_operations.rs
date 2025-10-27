//! Internal file operations trait and shared processor for file-based stores
//!
//! This module provides the core abstraction for reading files from different sources
//! (filesystem, HTTP, etc.) and a shared processor that implements common store
//! operations using these file operations.

use semver::Version;
use serde::de::DeserializeOwned;
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::RwLock;

use tracing::{debug, warn};

use crate::error::{Result, StoreError};
use crate::manager::store_manifest::ExtensionVersion;
use crate::manager::store_manifest::StoreManifest;
use crate::models::{
    ExtensionInfo, ExtensionMetadata, ExtensionPackage, SearchQuery, UpdateAvailableInfo,
    UpdateCheckFailedInfo, UpdateNotNeededInfo,
};
use crate::registry::manifest::{ExtensionManifest, FileReference, LocalExtensionManifest};
use crate::stores::impls::local::store::LocalStoreManifest;
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
    store_cache: Arc<RwLock<Option<LocalStoreManifest>>>,
}

impl<F: FileOperations> FileBasedProcessor<F> {
    /// Create a new file-based processor
    pub fn new(file_ops: F, store_name: String) -> Self {
        Self {
            file_ops,
            store_name,
            store_cache: Arc::new(RwLock::new(None)),
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
        self.get_local_store_manifest()
            .await
            .map(|local_manifest| local_manifest.base)
    }

    /// Get the local store manifest for URL routing and extension listing
    pub async fn get_local_store_manifest(&self) -> Result<LocalStoreManifest> {
        {
            let read_guard = self.store_cache.read().await;
            if let Some(manifest) = &*read_guard {
                return Ok(manifest.clone());
            }
        }

        let manifest = self
            .read_json_file::<LocalStoreManifest>("store.json")
            .await?;

        let mut write_guard = self.store_cache.write().await;
        *write_guard = Some(manifest.clone());

        Ok(manifest)
    }

    pub async fn clear_cache(&self) {
        let mut write_guard = self.store_cache.write().await;
        *write_guard = None;
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

        // Get the manifest path from the local store manifest directly
        let local_manifest = self.get_local_store_manifest().await?;
        let manifest_path = local_manifest
            .extensions
            .get(extension_id)
            .and_then(|versions| versions.all_versions.get(version.as_ref()))
            .map(|summary| summary.manifest_path.clone())
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
        let local_manifest = self
            .get_local_extension_manifest(extension_id, version)
            .await?;

        Ok(local_manifest.metadata)
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
    use chrono::Utc;
    use semver::Version;

    use crate::models::CompatibilityInfo;
    use crate::registry::manifest::{
        Attribute, ExtensionManifest, FileReference, ReadingDirection,
    };
    use crate::stores::impls::local::{
        index::{LocalStoreManifestIndex, UrlPattern},
        store::ExtensionVersions,
    };

    use super::*;
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    use std::path::PathBuf;

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
                .ok_or_else(|| StoreError::ExtensionNotFound(format!("File not found: {}", path)))
        }

        async fn file_exists(&self, path: &str) -> Result<bool> {
            // Check for exact file match first
            if self.files.contains_key(path) {
                return Ok(true);
            }

            // Check if it's a directory by looking for files with this path as prefix
            let prefix = if path.ends_with('/') {
                path.to_string()
            } else {
                format!("{}/", path)
            };

            for file_path in self.files.keys() {
                if file_path.starts_with(&prefix) {
                    return Ok(true);
                }
            }

            Ok(false)
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
                    let remaining = &file_path[prefix.len()..];
                    if let Some(slash_pos) = remaining.find('/') {
                        let first_part = &remaining[..slash_pos];
                        if !first_part.is_empty() {
                            entries.insert(first_part.to_string());
                        }
                    } else if !remaining.is_empty() {
                        // File directly in directory
                        entries.insert(remaining.to_string());
                    }
                }
            }

            Ok(entries.into_iter().collect())
        }
    }

    /// Test fixture builder for creating test data
    struct TestFixture {
        mock_ops: MockFileOperations,
        local_manifest: LocalStoreManifest,
    }

    impl TestFixture {
        fn new() -> Self {
            let base_manifest = StoreManifest::new(
                "test-store".to_string(),
                "local".to_string(),
                "1.0.0".to_string(),
            );

            let local_manifest = LocalStoreManifest {
                base: base_manifest,
                index: LocalStoreManifestIndex {
                    url_patterns: vec![],
                },
                extensions: BTreeMap::new(),
            };

            Self {
                mock_ops: MockFileOperations::new(),
                local_manifest,
            }
        }

        /// Add an extension to the test fixture
        fn with_extension(
            mut self,
            id: &str,
            name: &str,
            version: &str,
            langs: Vec<String>,
        ) -> Self {
            let version = Version::parse(version).unwrap();

            let extension_summary = ExtensionVersion {
                id: id.to_string(),
                name: name.to_string(),
                version: version.clone(),
                base_urls: vec![],
                langs,
                last_updated: Utc::now(),
                manifest_path: format!("extensions/{}/{}/manifest.json", id, version),
                manifest_checksum: {
                    use sha2::{Digest, Sha256};
                    format!(
                        "{:x}",
                        Sha256::digest(format!("manifest-{}", id).as_bytes())
                    )
                },
            };

            let mut extension_versions = ExtensionVersions {
                latest: version.clone(),
                all_versions: BTreeMap::new(),
            };

            extension_versions
                .all_versions
                .insert(version, extension_summary);

            self.local_manifest
                .extensions
                .insert(id.to_string(), extension_versions);

            self
        }

        /// Add multiple versions of an extension
        fn with_extension_versions(
            mut self,
            id: &str,
            name: &str,
            versions: &[&str],
            langs: Vec<String>,
        ) -> Self {
            let parsed_versions: Vec<Version> = versions
                .iter()
                .map(|v| Version::parse(v).unwrap())
                .collect();

            let latest = parsed_versions.iter().max().unwrap().clone();

            let mut extension_versions = ExtensionVersions {
                latest: latest.clone(),
                all_versions: BTreeMap::new(),
            };

            for version in parsed_versions {
                let manifest_path = format!("extensions/{}/{}/manifest.json", id, version);

                let extension_summary = ExtensionVersion {
                    id: id.to_string(),
                    name: name.to_string(),
                    version: version.clone(),
                    base_urls: vec![],
                    langs: langs.clone(),
                    last_updated: Utc::now(),
                    manifest_path: manifest_path.clone(),
                    manifest_checksum: {
                        use sha2::{Digest, Sha256};
                        format!(
                            "{:x}",
                            Sha256::digest(format!("manifest-{}-{}", id, version).as_bytes())
                        )
                    },
                };

                // Create a basic extension manifest for this version
                let extension_manifest = ExtensionManifest {
                    id: id.to_string(),
                    name: name.to_string(),
                    version: version.clone(),
                    author: "Test Author".to_string(),
                    langs: langs.clone(),
                    base_urls: vec!["https://example.com".to_string()],
                    rds: vec![ReadingDirection::Ltr],
                    attrs: vec![Attribute::Fanfiction],
                    signature: None,
                    wasm_file: FileReference::new("extension.wasm".to_string(), b"fake wasm"),
                    assets: vec![],
                };

                // Create LocalExtensionManifest
                let local_manifest = LocalExtensionManifest {
                    manifest: extension_manifest,
                    path: PathBuf::from(format!("extensions/{}/{}", id, version)),
                    metadata: None,
                };

                // Add manifest file to mock filesystem
                self.mock_ops.add_json_file(&manifest_path, &local_manifest);

                // Add WASM file to mock filesystem
                self.mock_ops.add_file(
                    &format!("extensions/{}/{}/extension.wasm", id, version),
                    b"fake wasm".to_vec(),
                );

                extension_versions
                    .all_versions
                    .insert(version, extension_summary);
            }

            self.local_manifest
                .extensions
                .insert(id.to_string(), extension_versions);

            self
        }

        /// Add an extension with metadata for testing metadata operations
        fn with_extension_metadata(
            mut self,
            id: &str,
            name: &str,
            version: &str,
            langs: Vec<String>,
            metadata: ExtensionMetadata,
        ) -> Self {
            let version = Version::parse(version).unwrap();
            let manifest_path = format!("extensions/{}/{}/manifest.json", id, version);

            let extension_summary = ExtensionVersion {
                id: id.to_string(),
                name: name.to_string(),
                version: version.clone(),
                base_urls: vec![],
                langs,
                last_updated: Utc::now(),
                manifest_path: manifest_path.clone(),
                manifest_checksum: {
                    use sha2::{Digest, Sha256};
                    format!(
                        "{:x}",
                        Sha256::digest(format!("manifest-{}", id).as_bytes())
                    )
                },
            };

            // Create a basic extension manifest
            let extension_manifest = ExtensionManifest {
                id: id.to_string(),
                name: name.to_string(),
                version: version.clone(),
                author: "Test Author".to_string(),
                langs: vec!["en".to_string()],
                base_urls: vec!["https://example.com".to_string()],
                rds: vec![ReadingDirection::Ltr],
                attrs: vec![Attribute::Fanfiction],
                signature: None,
                wasm_file: FileReference {
                    path: "extension.wasm".to_string(),
                    checksum: "blake3:fake_checksum".to_string(),
                    size: 1024,
                },
                assets: vec![],
            };

            // Create LocalExtensionManifest with metadata
            let local_manifest = LocalExtensionManifest {
                manifest: extension_manifest,
                path: PathBuf::from(format!("extensions/{}/{}", id, version)),
                metadata: Some(metadata),
            };

            // Add to extensions map
            let mut extension_versions = ExtensionVersions {
                latest: version.clone(),
                all_versions: BTreeMap::new(),
            };
            extension_versions
                .all_versions
                .insert(version, extension_summary);

            self.local_manifest
                .extensions
                .insert(id.to_string(), extension_versions);

            // Add manifest file
            self.mock_ops.add_json_file(&manifest_path, &local_manifest);

            self
        }

        /// Add an extension with assets for testing asset operations
        fn with_extension_assets(
            mut self,
            id: &str,
            name: &str,
            version: &str,
            langs: Vec<String>,
            asset_files: Vec<(&str, &str)>, // (name, path) pairs
        ) -> Self {
            let version_parsed = Version::parse(version).unwrap();
            let manifest_path = format!("extensions/{}/{}/manifest.json", id, version);

            // Create asset references
            let assets: Vec<crate::registry::manifest::AssetReference> = asset_files
                .iter()
                .map(|(name, path)| crate::registry::manifest::AssetReference {
                    name: name.to_string(),
                    file: FileReference {
                        path: path.to_string(),
                        checksum: format!(
                            "blake3:{}",
                            blake3::hash(b"fake asset content").to_hex()
                        ),
                        size: 100,
                    },
                    asset_type: "generic".to_string(),
                })
                .collect();

            // Create extension manifest with assets
            let extension_manifest = ExtensionManifest {
                id: id.to_string(),
                name: name.to_string(),
                version: version_parsed.clone(),
                author: "Test Author".to_string(),
                langs: langs.clone(),
                base_urls: vec!["https://example.com".to_string()],
                rds: vec![ReadingDirection::Ltr],
                attrs: vec![Attribute::Fanfiction],
                signature: None,
                wasm_file: FileReference::new("extension.wasm".to_string(), b"fake wasm"),
                assets,
            };

            // Create LocalExtensionManifest
            let local_manifest = LocalExtensionManifest {
                manifest: extension_manifest,
                path: PathBuf::from(format!("extensions/{}/{}", id, version)),
                metadata: None,
            };

            // Add to extensions map
            let extension_summary = ExtensionVersion {
                id: id.to_string(),
                name: name.to_string(),
                version: version_parsed.clone(),
                base_urls: vec![],
                langs,
                last_updated: Utc::now(),
                manifest_path: manifest_path.clone(),
                manifest_checksum: {
                    use sha2::{Digest, Sha256};
                    format!(
                        "{:x}",
                        Sha256::digest(format!("manifest-{}", id).as_bytes())
                    )
                },
            };

            let mut extension_versions = ExtensionVersions {
                latest: version_parsed.clone(),
                all_versions: BTreeMap::new(),
            };
            extension_versions
                .all_versions
                .insert(version_parsed, extension_summary);

            self.local_manifest
                .extensions
                .insert(id.to_string(), extension_versions);

            // Add manifest file
            self.mock_ops.add_json_file(&manifest_path, &local_manifest);

            // Add WASM file
            self.mock_ops.add_file(
                &format!("extensions/{}/{}/extension.wasm", id, version),
                b"fake wasm".to_vec(),
            );

            // Add asset files
            for (_, path) in asset_files {
                self.mock_ops.add_file(
                    &format!("extensions/{}/{}/{}", id, version, path),
                    b"fake asset content".to_vec(),
                );
            }

            self
        }

        /// Add a URL pattern mapping for testing URL-based extension finding
        fn with_url_pattern(mut self, url_prefix: &str, extension_ids: &[&str]) -> Self {
            let mut url_pattern = UrlPattern {
                url_prefix: url_prefix.to_string(),
                extensions: BTreeSet::new(),
            };

            for id in extension_ids {
                url_pattern.extensions.insert(id.to_string());
            }

            self.local_manifest.index.url_patterns.push(url_pattern);

            // Also update base_urls in the extension summaries
            for ext_id in extension_ids {
                if let Some(versions) = self.local_manifest.extensions.get_mut(*ext_id) {
                    for (_, summary) in versions.all_versions.iter_mut() {
                        if !summary.base_urls.contains(&url_prefix.to_string()) {
                            summary.base_urls.push(url_prefix.to_string());
                        }
                    }
                }
            }

            self
        }

        /// Add a full extension manifest for testing manifest operations
        fn with_full_manifest(mut self, id: &str, version: &str, wasm_content: &[u8]) -> Self {
            let version_parsed = Version::parse(version).unwrap();
            let manifest_path = format!("extensions/{}/{}/manifest.json", id, version);
            let wasm_path = format!("extensions/{}/{}/extension.wasm", id, version);

            // Create a proper ExtensionManifest
            let wasm_file = FileReference::new("extension.wasm".to_string(), wasm_content);

            let extension_manifest = ExtensionManifest {
                id: id.to_string(),
                name: id.to_string(),
                version: version_parsed.clone(),
                author: "Test Author".to_string(),
                langs: vec!["en".to_string()],
                base_urls: vec!["https://example.com".to_string()],
                rds: vec![ReadingDirection::Ltr],
                attrs: vec![Attribute::Fanfiction],
                signature: None,
                wasm_file: wasm_file.clone(),
                assets: vec![],
            };

            // Create LocalExtensionManifest
            let local_manifest = LocalExtensionManifest {
                manifest: extension_manifest,
                path: PathBuf::from(format!("extensions/{}/{}", id, version)),
                metadata: None,
            };

            // Add manifest and wasm files
            self.mock_ops.add_json_file(&manifest_path, &local_manifest);
            self.mock_ops.add_file(&wasm_path, wasm_content.to_vec());

            self
        }

        /// Build the processor with all configured data
        fn build(mut self) -> FileBasedProcessor<MockFileOperations> {
            self.mock_ops
                .add_json_file("store.json", &self.local_manifest);
            FileBasedProcessor::new(self.mock_ops, "test-store".to_string())
        }
    }

    #[tokio::test]
    async fn test_file_based_processor_basic() {
        let processor = TestFixture::new().build();

        let manifest = processor.get_store_manifest().await.unwrap();
        assert_eq!(manifest.name, "test-store");
        assert_eq!(manifest.store_type, "local");
    }

    #[tokio::test]
    async fn test_list_extensions_empty() {
        let processor = TestFixture::new().build();

        let extensions = processor.list_extensions().await.unwrap();
        assert!(extensions.is_empty());
    }

    #[tokio::test]
    async fn test_list_extensions_with_data() {
        let processor = TestFixture::new()
            .with_extension(
                "test-ext",
                "Test Extension",
                "1.0.0",
                vec!["en".to_string()],
            )
            .build();

        let extensions = processor.list_extensions().await.unwrap();
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].id, "test-ext");
        assert_eq!(extensions[0].name, "Test Extension");
        assert_eq!(extensions[0].version, Version::parse("1.0.0").unwrap());
    }

    #[tokio::test]
    async fn test_list_multiple_extensions() {
        let processor = TestFixture::new()
            .with_extension("ext-1", "Extension One", "1.0.0", vec!["en".to_string()])
            .with_extension("ext-2", "Extension Two", "2.0.0", vec!["fr".to_string()])
            .with_extension("ext-3", "Extension Three", "3.0.0", vec!["es".to_string()])
            .build();

        let extensions = processor.list_extensions().await.unwrap();
        assert_eq!(extensions.len(), 3);

        let ids: Vec<&str> = extensions.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"ext-1"));
        assert!(ids.contains(&"ext-2"));
        assert!(ids.contains(&"ext-3"));
    }

    #[tokio::test]
    async fn test_find_extensions_for_url() {
        let processor = TestFixture::new()
            .with_extension(
                "test-ext",
                "Test Extension",
                "1.0.0",
                vec!["en".to_string()],
            )
            .with_url_pattern("https://example.com", &["test-ext"])
            .with_url_pattern("https://test.org", &["test-ext"])
            .build();

        // Test matching URLs
        let matches = processor
            .find_extensions_for_url("https://example.com/some/path")
            .await
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "test-ext");
        assert_eq!(matches[0].1, "Test Extension");

        let matches = processor
            .find_extensions_for_url("https://test.org/another/path")
            .await
            .unwrap();
        assert_eq!(matches.len(), 1);

        // Test non-matching URL
        let matches = processor
            .find_extensions_for_url("https://nomatch.com/path")
            .await
            .unwrap();
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_extensions_for_same_url() {
        let processor = TestFixture::new()
            .with_extension("ext-1", "Extension One", "1.0.0", vec![])
            .with_extension("ext-2", "Extension Two", "1.0.0", vec![])
            .with_url_pattern("https://shared.com", &["ext-1", "ext-2"])
            .build();

        let matches = processor
            .find_extensions_for_url("https://shared.com/path")
            .await
            .unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[tokio::test]
    async fn test_search_extensions_by_text() {
        let processor = TestFixture::new()
            .with_extension(
                "json-parser",
                "JSON Parser",
                "1.0.0",
                vec!["en".to_string()],
            )
            .with_extension(
                "xml-handler",
                "XML Handler",
                "2.0.0",
                vec!["fr".to_string()],
            )
            .build();

        let query = SearchQuery {
            text: Some("json".to_string()),
            tags: vec![],
            categories: vec![],
            author: None,
            min_version: None,
            max_version: None,
            sort_by: crate::models::SearchSortBy::default(),
            limit: None,
            offset: None,
            include_prerelease: false,
        };
        let results = processor.search_extensions(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "json-parser");

        // Case insensitive search
        let query = SearchQuery {
            text: Some("JSON".to_string()),
            tags: vec![],
            categories: vec![],
            author: None,
            min_version: None,
            max_version: None,
            sort_by: crate::models::SearchSortBy::default(),
            limit: None,
            offset: None,
            include_prerelease: false,
        };
        let results = processor.search_extensions(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "json-parser");
    }

    #[tokio::test]
    async fn test_search_extensions_by_language() {
        let processor = TestFixture::new()
            .with_extension(
                "json-parser",
                "JSON Parser",
                "1.0.0",
                vec!["en".to_string()],
            )
            .with_extension(
                "xml-handler",
                "XML Handler",
                "2.0.0",
                vec!["fr".to_string()],
            )
            .build();

        let query = SearchQuery {
            text: None,
            tags: vec!["fr".to_string()],
            categories: vec![],
            author: None,
            min_version: None,
            max_version: None,
            sort_by: crate::models::SearchSortBy::default(),
            limit: None,
            offset: None,
            include_prerelease: false,
        };
        let results = processor.search_extensions(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "xml-handler");
    }

    #[tokio::test]
    async fn test_search_extensions_combined_filters() {
        let processor = TestFixture::new()
            .with_extension("json-en", "JSON Parser EN", "1.0.0", vec!["en".to_string()])
            .with_extension("json-fr", "JSON Parser FR", "1.0.0", vec!["fr".to_string()])
            .with_extension(
                "xml-handler",
                "XML Handler",
                "2.0.0",
                vec!["fr".to_string()],
            )
            .build();

        let query = SearchQuery {
            text: Some("json".to_string()),
            tags: vec!["fr".to_string()],
            categories: vec![],
            author: None,
            min_version: None,
            max_version: None,
            sort_by: crate::models::SearchSortBy::default(),
            limit: None,
            offset: None,
            include_prerelease: false,
        };
        let results = processor.search_extensions(&query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "json-fr");
    }

    #[tokio::test]
    async fn test_search_no_matches() {
        let processor = TestFixture::new()
            .with_extension(
                "json-parser",
                "JSON Parser",
                "1.0.0",
                vec!["en".to_string()],
            )
            .build();

        let query = SearchQuery {
            text: Some("nonexistent".to_string()),
            tags: vec![],
            categories: vec![],
            author: None,
            min_version: None,
            max_version: None,
            sort_by: crate::models::SearchSortBy::default(),
            limit: None,
            offset: None,
            include_prerelease: false,
        };
        let results = processor.search_extensions(&query).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_get_extension_latest_version() {
        let processor = TestFixture::new()
            .with_extension("my-ext", "My Extension", "2.5.0", vec![])
            .build();

        let latest = processor
            .get_extension_latest_version("my-ext")
            .await
            .unwrap();
        assert_eq!(latest, Some(Version::parse("2.5.0").unwrap()));
    }

    #[tokio::test]
    async fn test_get_extension_latest_version_not_found() {
        let processor = TestFixture::new().build();

        let latest = processor
            .get_extension_latest_version("nonexistent")
            .await
            .unwrap();
        assert_eq!(latest, None);
    }

    #[tokio::test]
    async fn test_list_extension_versions() {
        let processor = TestFixture::new()
            .with_extension_versions(
                "multi-version",
                "Multi Version",
                &["1.0.0", "1.1.0", "2.0.0"],
                vec![],
            )
            .build();

        let versions = processor
            .list_extension_versions("multi-version")
            .await
            .unwrap();
        assert_eq!(versions.len(), 3);
        assert!(versions.contains(&Version::parse("1.0.0").unwrap()));
        assert!(versions.contains(&Version::parse("1.1.0").unwrap()));
        assert!(versions.contains(&Version::parse("2.0.0").unwrap()));
    }

    #[tokio::test]
    async fn test_list_extension_versions_not_found() {
        let processor = TestFixture::new().build();

        let result = processor.list_extension_versions("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StoreError::ExtensionNotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_check_extension_version_exists() {
        let processor = TestFixture::new()
            .with_extension_versions("my-ext", "My Ext", &["1.0.0", "2.0.0"], vec![])
            .build();

        let exists = processor
            .check_extension_version_exists("my-ext", &Version::parse("1.0.0").unwrap())
            .await
            .unwrap();
        assert!(exists);

        let exists = processor
            .check_extension_version_exists("my-ext", &Version::parse("3.0.0").unwrap())
            .await
            .unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_get_extension_wasm_with_checksum() {
        let wasm_content = b"fake wasm content";

        let fixture = TestFixture::new()
            .with_extension("test-ext", "Test Extension", "1.0.0", vec![])
            .with_full_manifest("test-ext", "1.0.0", wasm_content);

        let processor = fixture.build();

        let local_manifest = processor
            .get_local_extension_manifest("test-ext", Some(&Version::parse("1.0.0").unwrap()))
            .await
            .unwrap();

        let wasm = processor.get_extension_wasm(&local_manifest).await.unwrap();
        assert_eq!(wasm, wasm_content);
    }

    #[tokio::test]
    async fn test_check_extension_updates() {
        let processor = TestFixture::new()
            .with_extension_versions("ext-1", "Extension 1", &["1.0.0", "2.0.0"], vec![])
            .with_extension("ext-2", "Extension 2", "1.0.0", vec![])
            .build();

        let installed = vec![
            InstalledExtension {
                id: "ext-1".to_string(),
                name: "Extension 1".to_string(),
                version: Version::parse("1.0.0").unwrap(),
                manifest: ExtensionManifest {
                    id: "ext-1".to_string(),
                    name: "Extension 1".to_string(),
                    version: Version::parse("1.0.0").unwrap(),
                    author: "Test Author".to_string(),
                    langs: vec!["en".to_string()],
                    base_urls: vec!["https://example.com".to_string()],
                    rds: vec![ReadingDirection::Ltr],
                    attrs: vec![],
                    signature: None,
                    wasm_file: FileReference {
                        path: "extension.wasm".to_string(),
                        checksum: "abc123".to_string(),
                        size: 1024,
                    },
                    assets: vec![],
                },
                metadata: None,
                size: 1024,
                installed_at: chrono::Utc::now(),
                last_updated: None,
                source_store: "test-store".to_string(),
                auto_update: false,
                checksum: None,
            },
            InstalledExtension {
                id: "ext-2".to_string(),
                name: "Extension 2".to_string(),
                version: Version::parse("1.0.0").unwrap(),
                manifest: ExtensionManifest {
                    id: "ext-2".to_string(),
                    name: "Extension 2".to_string(),
                    version: Version::parse("1.0.0").unwrap(),
                    author: "Test Author".to_string(),
                    langs: vec!["en".to_string()],
                    base_urls: vec!["https://example.com".to_string()],
                    rds: vec![ReadingDirection::Ltr],
                    attrs: vec![],
                    signature: None,
                    wasm_file: FileReference {
                        path: "extension.wasm".to_string(),
                        checksum: "def456".to_string(),
                        size: 2048,
                    },
                    assets: vec![],
                },
                metadata: None,
                size: 2048,
                installed_at: chrono::Utc::now(),
                last_updated: None,
                source_store: "test-store".to_string(),
                auto_update: false,
                checksum: None,
            },
        ];

        let updates = processor
            .check_extension_updates(&installed, "test-store")
            .await
            .unwrap();

        assert_eq!(updates.len(), 2);

        // Check ext-1 has update available
        match &updates[0] {
            UpdateInfo::UpdateAvailable(info) => {
                assert_eq!(info.extension_id, "ext-1");
                assert_eq!(info.current_version, Version::parse("1.0.0").unwrap());
                assert_eq!(info.latest_version, Version::parse("2.0.0").unwrap());
            }
            _ => panic!("Expected UpdateAvailable for ext-1"),
        }

        // Check ext-2 has no update
        match &updates[1] {
            UpdateInfo::NoUpdateNeeded(info) => {
                assert_eq!(info.extension_id, "ext-2");
            }
            _ => panic!("Expected NoUpdateNeeded for ext-2"),
        }
    }

    #[tokio::test]
    async fn test_check_updates_for_missing_extension() {
        let processor = TestFixture::new().build();

        let installed = vec![InstalledExtension {
            id: "missing-ext".to_string(),
            name: "Missing Extension".to_string(),
            version: Version::parse("1.0.0").unwrap(),
            manifest: ExtensionManifest {
                id: "missing-ext".to_string(),
                name: "Missing Extension".to_string(),
                version: Version::parse("1.0.0").unwrap(),
                author: "Test Author".to_string(),
                langs: vec!["en".to_string()],
                base_urls: vec!["https://example.com".to_string()],
                rds: vec![ReadingDirection::Ltr],
                attrs: vec![],
                signature: None,
                wasm_file: FileReference {
                    path: "extension.wasm".to_string(),
                    checksum: "missing123".to_string(),
                    size: 512,
                },
                assets: vec![],
            },
            metadata: None,
            size: 512,
            installed_at: chrono::Utc::now(),
            last_updated: None,
            source_store: "test-store".to_string(),
            auto_update: false,
            checksum: None,
        }];

        let updates = processor
            .check_extension_updates(&installed, "test-store")
            .await
            .unwrap();

        assert_eq!(updates.len(), 1);
        match &updates[0] {
            UpdateInfo::CheckFailed(info) => {
                assert_eq!(info.extension_id, "missing-ext");
                assert!(info.error.contains("not found"));
            }
            _ => panic!("Expected CheckFailed for missing extension"),
        }
    }

    #[tokio::test]
    async fn test_get_extension_metadata() {
        let metadata = ExtensionMetadata {
            description: "Test extension description".to_string(),
            long_description: Some("Long description of the test extension".to_string()),
            keywords: vec!["parser".to_string(), "utility".to_string()],
            categories: vec!["tools".to_string()],
            homepage: Some("https://example.com".to_string()),
            repository: Some("https://github.com/test/repo".to_string()),
            documentation: None,
            changelog: None,
            license: Some("MIT".to_string()),
            compatibility: CompatibilityInfo {
                min_engine_version: None,
                max_engine_version: None,
                platforms: None,
                required_features: vec![],
            },
        };

        let processor = TestFixture::new()
            .with_extension_metadata(
                "metadata-ext",
                "Metadata Extension",
                "1.0.0",
                vec!["en".to_string()],
                metadata.clone(),
            )
            .build();

        let result = processor
            .get_extension_metadata("metadata-ext", Some(&Version::parse("1.0.0").unwrap()))
            .await
            .unwrap();

        assert!(result.is_some());
        let retrieved_metadata = result.unwrap();
        assert_eq!(
            retrieved_metadata.description,
            "Test extension description".to_string()
        );
        assert_eq!(
            retrieved_metadata.keywords,
            vec!["parser".to_string(), "utility".to_string()]
        );
        assert_eq!(
            retrieved_metadata.homepage,
            Some("https://example.com".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_extension_metadata_not_found() {
        let processor = TestFixture::new().build();

        let result = processor.get_extension_metadata("nonexistent", None).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StoreError::ExtensionNotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_get_extension_metadata_no_metadata() {
        let processor = TestFixture::new()
            .with_extension(
                "no-metadata",
                "No Metadata",
                "1.0.0",
                vec!["en".to_string()],
            )
            .build();

        let result = processor
            .get_extension_metadata("no-metadata", Some(&Version::parse("1.0.0").unwrap()))
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_extension_info() {
        let processor = TestFixture::new()
            .with_extension_versions(
                "multi-ext",
                "Multi Extension",
                &["1.0.0", "1.1.0", "2.0.0"],
                vec!["en".to_string()],
            )
            .build();

        let info = processor.get_extension_info("multi-ext").await.unwrap();

        assert_eq!(info.len(), 3);
        let versions: Vec<Version> = info.iter().map(|i| i.version.clone()).collect();
        assert!(versions.contains(&Version::parse("1.0.0").unwrap()));
        assert!(versions.contains(&Version::parse("1.1.0").unwrap()));
        assert!(versions.contains(&Version::parse("2.0.0").unwrap()));

        // Check that all infos have correct basic data
        for ext_info in &info {
            assert_eq!(ext_info.id, "multi-ext");
            assert_eq!(ext_info.name, "Multi Extension");
            assert_eq!(ext_info.author, "Test Author");
            assert_eq!(ext_info.store_source, "test-store");
        }
    }

    #[tokio::test]
    async fn test_get_extension_info_not_found() {
        let processor = TestFixture::new().build();

        let result = processor.get_extension_info("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            StoreError::ExtensionNotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_get_extension_version_info() {
        let processor = TestFixture::new()
            .with_extension_versions(
                "version-test",
                "Version Test",
                &["1.5.0"],
                vec!["en".to_string()],
            )
            .build();

        let info = processor
            .get_extension_version_info("version-test", Some(&Version::parse("1.5.0").unwrap()))
            .await
            .unwrap();

        assert_eq!(info.id, "version-test");
        assert_eq!(info.name, "Version Test");
        assert_eq!(info.version, Version::parse("1.5.0").unwrap());
        assert_eq!(info.author, "Test Author");
        assert_eq!(info.store_source, "test-store");
        assert!(info.description.is_none()); // ExtensionManifest doesn't have description
        assert!(info.tags.is_empty()); // No tags extracted from manifest
    }

    #[tokio::test]
    async fn test_get_extension_version_info_latest() {
        let processor = TestFixture::new()
            .with_extension_versions(
                "latest-test",
                "Latest Test",
                &["2.0.0"],
                vec!["en".to_string()],
            )
            .build();

        // Test getting info for latest version (None)
        let info = processor
            .get_extension_version_info("latest-test", None)
            .await
            .unwrap();

        assert_eq!(info.id, "latest-test");
        assert_eq!(info.version, Version::parse("2.0.0").unwrap());
    }

    #[tokio::test]
    async fn test_get_extension_package() {
        let _wasm_content = b"test wasm content for package";

        let processor = TestFixture::new()
            .with_extension_assets(
                "package-test",
                "Package Test",
                "1.0.0",
                vec!["en".to_string()],
                vec![("icon.png", "icon.png"), ("config.json", "config.json")],
            )
            .build();

        let package = processor
            .get_extension_package(
                "package-test",
                Some(&Version::parse("1.0.0").unwrap()),
                "test-store".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(package.manifest.id, "package-test");
        assert_eq!(package.manifest.name, "Package Test");
        assert_eq!(package.manifest.version, Version::parse("1.0.0").unwrap());
        assert_eq!(package.wasm_component, b"fake wasm");
        assert_eq!(package.source_store, "test-store");

        // Check assets are loaded
        assert_eq!(package.assets.len(), 2);
        assert!(package.assets.contains_key("icon.png"));
        assert!(package.assets.contains_key("config.json"));
        assert_eq!(package.assets["icon.png"], b"fake asset content");
    }

    #[tokio::test]
    async fn test_get_extension_package_no_assets() {
        let processor = TestFixture::new()
            .with_extension_versions("no-assets", "No Assets", &["1.0.0"], vec!["en".to_string()])
            .build();

        let package = processor
            .get_extension_package(
                "no-assets",
                Some(&Version::parse("1.0.0").unwrap()),
                "test-store".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(package.manifest.id, "no-assets");
        assert!(package.assets.is_empty());
    }

    #[tokio::test]
    async fn test_list_extension_assets() {
        let processor = TestFixture::new()
            .with_extension_assets(
                "asset-test",
                "Asset Test",
                "1.0.0",
                vec!["en".to_string()],
                vec![
                    ("icon", "icon.png"),
                    ("documentation", "README.md"),
                    ("config", "settings.json"),
                ],
            )
            .build();

        let assets = processor
            .list_extension_assets("asset-test", Some(&Version::parse("1.0.0").unwrap()))
            .await
            .unwrap();

        assert_eq!(assets.len(), 3);
        assert!(assets.contains(&"icon.png".to_string()));
        assert!(assets.contains(&"README.md".to_string()));
        assert!(assets.contains(&"settings.json".to_string()));
    }

    #[tokio::test]
    async fn test_list_extension_assets_no_assets() {
        let processor = TestFixture::new()
            .with_extension_versions("no-assets", "No Assets", &["1.0.0"], vec!["en".to_string()])
            .build();

        let assets = processor
            .list_extension_assets("no-assets", Some(&Version::parse("1.0.0").unwrap()))
            .await
            .unwrap();

        assert!(assets.is_empty());
    }

    #[tokio::test]
    async fn test_clear_cache_invalidates_local_store_manifest() {
        let mut fixture = TestFixture::new().build();

        // Initial fetch of store manifest to cache it
        let first_manifest = fixture.get_local_store_manifest().await.unwrap();

        // Overwrite the store manifest file to simulate a change
        fixture.file_ops.add_json_file(
            "store.json",
            &LocalStoreManifest {
                index: LocalStoreManifestIndex {
                    url_patterns: vec![],
                },
                extensions: BTreeMap::new(),
                base: StoreManifest::new(
                    "modified-store".to_string(),
                    "local".to_string(),
                    "1.0.0".to_string(),
                ),
            },
        );

        // Test that manifest is unchanged due to caching
        let cached_manifest = fixture.get_local_store_manifest().await.unwrap();
        assert_eq!(first_manifest.base.name, "test-store");
        assert_eq!(first_manifest.base.name, cached_manifest.base.name);

        // Clear cache
        fixture.clear_cache().await;

        // Test that manifest is updated after cache clear
        let updated_manifest = fixture.get_local_store_manifest().await.unwrap();
        assert_eq!(updated_manifest.base.name, "modified-store");
    }
}
