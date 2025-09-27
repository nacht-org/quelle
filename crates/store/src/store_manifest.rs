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

    /// URL Routing & Domain Support
    pub url_patterns: Vec<UrlPattern>,
    pub supported_domains: Vec<String>,

    /// Extension Index for Fast Lookups
    pub extension_count: u32,
    pub extensions: Vec<ExtensionSummary>,

    /// Metadata
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UrlPattern {
    /// URL prefix that this pattern matches (e.g., "https://example.com")
    pub url_prefix: String,
    /// Extensions that can handle URLs matching this prefix
    pub extensions: Vec<String>,
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
            url_patterns: Vec::new(),
            supported_domains: Vec::new(),
            extension_count: 0,
            extensions: Vec::new(),
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

    /// Add a URL pattern for extension matching
    pub fn add_url_pattern(&mut self, url_prefix: String, extensions: Vec<String>, priority: u8) {
        self.url_patterns.push(UrlPattern {
            url_prefix,
            extensions,
            priority,
        });

        // Sort patterns by priority (highest first)
        self.url_patterns
            .sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Add an extension summary to the manifest
    pub fn add_extension(&mut self, extension: ExtensionSummary) {
        // Update supported domains from extension base URLs
        for base_url in &extension.base_urls {
            if let Ok(parsed) = url::Url::parse(base_url) {
                if let Some(domain) = parsed.domain() {
                    let domain = domain.to_string();
                    if !self.supported_domains.contains(&domain) {
                        self.supported_domains.push(domain);
                    }
                }
            }
        }

        self.extensions.push(extension);
        self.extension_count = self.extensions.len() as u32;
        self.supported_domains.sort();
        self.last_updated = chrono::Utc::now();
    }

    /// Find extensions that can handle the given URL
    pub fn find_extensions_for_url(&self, url: &str) -> Vec<String> {
        let mut matches = Vec::new();

        // Check URL patterns first (sorted by priority)
        for pattern in &self.url_patterns {
            if url.starts_with(&pattern.url_prefix) {
                matches.extend(pattern.extensions.clone());
            }
        }

        // If no pattern matches, check individual extension base URLs
        if matches.is_empty() {
            for ext in &self.extensions {
                for base_url in &ext.base_urls {
                    if url.starts_with(base_url) {
                        matches.push(ext.name.clone());
                        break; // Don't add the same extension multiple times
                    }
                }
            }
        }

        // Remove duplicates while preserving order
        let mut unique_matches = Vec::new();
        for m in matches {
            if !unique_matches.contains(&m) {
                unique_matches.push(m);
            }
        }

        unique_matches
    }

    /// Get extensions that support a specific domain
    pub fn extensions_for_domain(&self, domain: &str) -> Vec<String> {
        let mut matches = Vec::new();

        for ext in &self.extensions {
            for base_url in &ext.base_urls {
                if let Ok(parsed) = url::Url::parse(base_url) {
                    if let Some(url_domain) = parsed.domain() {
                        if url_domain == domain {
                            matches.push(ext.name.clone());
                            break;
                        }
                    }
                }
            }
        }

        matches
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
        assert_eq!(manifest.extension_count, 0);
    }

    #[test]
    fn test_url_pattern_matching() {
        let mut manifest =
            StoreManifest::new("test".to_string(), "local".to_string(), "1.0".to_string());

        manifest.add_url_pattern(
            "https://example.com".to_string(),
            vec!["example-ext".to_string()],
            10,
        );

        let matches = manifest.find_extensions_for_url("https://example.com/novel/123");
        assert_eq!(matches, vec!["example-ext"]);

        let no_matches = manifest.find_extensions_for_url("https://other.com/novel/123");
        assert!(no_matches.is_empty());
    }

    #[test]
    fn test_extension_base_url_matching() {
        let mut manifest =
            StoreManifest::new("test".to_string(), "local".to_string(), "1.0".to_string());

        let ext = ExtensionSummary {
            name: "novel-ext".to_string(),
            version: "1.0.0".to_string(),
            base_urls: vec!["https://novels.com".to_string()],
            langs: vec!["en".to_string()],
            last_updated: chrono::Utc::now(),
        };

        manifest.add_extension(ext);

        let matches = manifest.find_extensions_for_url("https://novels.com/book/456");
        assert_eq!(matches, vec!["novel-ext"]);

        // Should also update supported domains
        assert!(manifest
            .supported_domains
            .contains(&"novels.com".to_string()));
    }

    #[test]
    fn test_priority_ordering() {
        let mut manifest =
            StoreManifest::new("test".to_string(), "local".to_string(), "1.0".to_string());

        // Add patterns in reverse priority order
        manifest.add_url_pattern(
            "https://example.com".to_string(),
            vec!["low".to_string()],
            1,
        );
        manifest.add_url_pattern(
            "https://example.com".to_string(),
            vec!["high".to_string()],
            10,
        );
        manifest.add_url_pattern(
            "https://example.com".to_string(),
            vec!["med".to_string()],
            5,
        );

        // Should be sorted by priority (highest first)
        assert_eq!(manifest.url_patterns[0].priority, 10);
        assert_eq!(manifest.url_patterns[1].priority, 5);
        assert_eq!(manifest.url_patterns[2].priority, 1);
    }

    #[test]
    fn test_comprehensive_url_matching() {
        let mut manifest = StoreManifest::new(
            "test-store".to_string(),
            "local".to_string(),
            "1.0".to_string(),
        );

        // Add URL patterns with different priorities
        manifest.add_url_pattern(
            "https://novels.com".to_string(),
            vec!["novel-scraper".to_string()],
            10,
        );
        manifest.add_url_pattern(
            "https://manga.site".to_string(),
            vec!["manga-reader".to_string()],
            8,
        );
        manifest.add_url_pattern(
            "https://webtoon.platform".to_string(),
            vec!["webtoon-ext".to_string()],
            6,
        );

        // Add extensions with base URLs for fallback matching
        let novel_ext = ExtensionSummary {
            name: "fallback-novel".to_string(),
            version: "1.0.0".to_string(),
            base_urls: vec!["https://backup-novels.com".to_string()],
            langs: vec!["en".to_string()],
            last_updated: chrono::Utc::now(),
        };
        manifest.add_extension(novel_ext);

        let multi_url_ext = ExtensionSummary {
            name: "multi-domain".to_string(),
            version: "2.0.0".to_string(),
            base_urls: vec![
                "https://domain1.com".to_string(),
                "https://domain2.com".to_string(),
            ],
            langs: vec!["en", "es"].iter().map(|s| s.to_string()).collect(),
            last_updated: chrono::Utc::now(),
        };
        manifest.add_extension(multi_url_ext);

        // Test URL pattern matching (should have priority)
        assert_eq!(
            manifest.find_extensions_for_url("https://novels.com/book/123"),
            vec!["novel-scraper"]
        );
        assert_eq!(
            manifest.find_extensions_for_url("https://manga.site/chapter/456"),
            vec!["manga-reader"]
        );
        assert_eq!(
            manifest.find_extensions_for_url("https://webtoon.platform/series/789"),
            vec!["webtoon-ext"]
        );

        // Test fallback to extension base URLs
        assert_eq!(
            manifest.find_extensions_for_url("https://backup-novels.com/story/abc"),
            vec!["fallback-novel"]
        );
        assert_eq!(
            manifest.find_extensions_for_url("https://domain1.com/content"),
            vec!["multi-domain"]
        );
        assert_eq!(
            manifest.find_extensions_for_url("https://domain2.com/page"),
            vec!["multi-domain"]
        );

        // Test no matches
        assert!(manifest
            .find_extensions_for_url("https://unknown-site.com/page")
            .is_empty());

        // Test supported domains are populated correctly
        let domains = &manifest.supported_domains;
        assert!(domains.contains(&"backup-novels.com".to_string()));
        assert!(domains.contains(&"domain1.com".to_string()));
        assert!(domains.contains(&"domain2.com".to_string()));
        assert_eq!(domains.len(), 3); // Should be sorted and unique
    }

    #[test]
    fn test_extensions_for_domain() {
        let mut manifest =
            StoreManifest::new("test".to_string(), "local".to_string(), "1.0".to_string());

        let ext1 = ExtensionSummary {
            name: "ext-1".to_string(),
            version: "1.0.0".to_string(),
            base_urls: vec!["https://example.com".to_string()],
            langs: vec!["en".to_string()],
            last_updated: chrono::Utc::now(),
        };

        let ext2 = ExtensionSummary {
            name: "ext-2".to_string(),
            version: "1.0.0".to_string(),
            base_urls: vec![
                "https://example.com".to_string(),
                "https://other.com".to_string(),
            ],
            langs: vec!["en".to_string()],
            last_updated: chrono::Utc::now(),
        };

        manifest.add_extension(ext1);
        manifest.add_extension(ext2);

        // Both extensions support example.com
        let example_extensions = manifest.extensions_for_domain("example.com");
        assert_eq!(example_extensions.len(), 2);
        assert!(example_extensions.contains(&"ext-1".to_string()));
        assert!(example_extensions.contains(&"ext-2".to_string()));

        // Only ext-2 supports other.com
        let other_extensions = manifest.extensions_for_domain("other.com");
        assert_eq!(other_extensions, vec!["ext-2"]);

        // No extensions support unknown.com
        let unknown_extensions = manifest.extensions_for_domain("unknown.com");
        assert!(unknown_extensions.is_empty());
    }

    #[test]
    fn test_manifest_touch_updates_timestamp() {
        let mut manifest =
            StoreManifest::new("test".to_string(), "local".to_string(), "1.0".to_string());
        let original_time = manifest.last_updated;

        // Sleep a tiny bit to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        manifest.touch();
        assert!(manifest.last_updated > original_time);
    }
}
