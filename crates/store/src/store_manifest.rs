use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Store Manifest - Internal metadata about the store's contents and capabilities
///
/// This is different from StoreInfo which contains external configuration:
/// - StoreInfo: How the store is configured in the registry (priority, trusted, enabled)
/// - StoreManifest: What the store actually contains (extensions, URL patterns, domains)
///
/// The manifest is stored within the store itself (e.g., store.json for LocalStore)
/// and is used for fast URL routing and extension discovery.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoreManifest {
    /// Store Identity (intrinsic properties of the store itself)
    pub store_name: String,
    pub store_type: String,
    pub store_version: String,
    pub manifest_version: String,
    pub url: Option<String>,
    pub description: Option<String>,

    /// Metadata
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UrlPattern {
    /// URL prefix that this pattern matches (e.g., "https://example.com")
    pub url_prefix: String,
    /// Extensions that can handle URLs matching this prefix
    pub extensions: BTreeSet<String>,
    /// Priority for this pattern (higher = more preferred)
    pub priority: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExtensionSummary {
    pub name: String,
    pub version: String,
    pub base_urls: Vec<String>,
    pub langs: Vec<String>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl StoreManifest {
    /// Create a new store manifest with basic information
    pub fn new(store_name: String, store_type: String, store_version: String) -> Self {
        Self {
            store_name,
            store_type,
            store_version,
            manifest_version: "1.0".to_string(),
            url: None,
            description: None,
            last_updated: chrono::Utc::now(),
        }
    }

    /// Set the store URL
    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    /// Set the store description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Update the last_updated timestamp
    pub fn touch(&mut self) {
        self.last_updated = chrono::Utc::now();
    }
}

impl Default for StoreManifest {
    fn default() -> Self {
        Self::new(
            "unnamed".to_string(),
            "unknown".to_string(),
            "0.1.0".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_creation() {
        let manifest = StoreManifest::new(
            "test-store".to_string(),
            "local".to_string(),
            "1.0.0".to_string(),
        );
        assert_eq!(manifest.store_name, "test-store");
        assert_eq!(manifest.store_type, "local");
        assert_eq!(manifest.store_version, "1.0.0");
        assert_eq!(manifest.manifest_version, "1.0");
    }

    #[test]
    fn test_with_url_and_description() {
        let manifest =
            StoreManifest::new("test".to_string(), "local".to_string(), "1.0".to_string())
                .with_url("https://example.com".to_string())
                .with_description("Test store".to_string());

        assert_eq!(manifest.url, Some("https://example.com".to_string()));
        assert_eq!(manifest.description, Some("Test store".to_string()));
    }

    #[test]
    fn test_touch_updates_timestamp() {
        let mut manifest =
            StoreManifest::new("test".to_string(), "local".to_string(), "1.0".to_string());
        let original_time = manifest.last_updated;

        // Sleep a tiny bit to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        manifest.touch();
        assert!(manifest.last_updated > original_time);
    }
}
