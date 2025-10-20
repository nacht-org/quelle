use serde::{Deserialize, Serialize};

use crate::{ExtensionVersion, UrlPattern};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LocalStoreManifestIndex {
    pub url_patterns: Vec<UrlPattern>,
    pub supported_domains: Vec<String>,
}

impl LocalStoreManifestIndex {
    /// Add a URL pattern for extension matching
    pub(crate) fn add_url_pattern(&mut self, url_prefix: String, extension: String, priority: u8) {
        // Check if pattern already exists
        if let Some(pattern) = self
            .url_patterns
            .iter_mut()
            .find(|p| p.url_prefix == url_prefix)
        {
            // Add extension if not already present
            if !pattern.extensions.contains(&extension) {
                pattern.extensions.insert(extension);
            }
        } else {
            // Create new pattern
            self.url_patterns.push(UrlPattern {
                url_prefix,
                extensions: [extension].into_iter().collect(),
                priority,
            });
        }

        // Sort patterns by priority (higher first)
        self.url_patterns
            .sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub(crate) fn regenerate<'a>(
        &mut self,
        extension_versions: impl Iterator<Item = &'a ExtensionVersion>,
    ) {
        self.url_patterns.clear();
        self.supported_domains.clear();

        for extension in extension_versions {
            for base_url in &extension.base_urls {
                self.add_url_pattern(base_url.clone(), extension.id.clone(), 1);
            }

            // Add supported domains
            for base_url in &extension.base_urls {
                if let Ok(url) = url::Url::parse(base_url) {
                    if let Some(host) = url.host_str() {
                        self.supported_domains.push(host.to_string());
                    }
                }
            }
        }

        self.supported_domains.sort();
    }

    pub(crate) fn reset(&mut self) {
        self.url_patterns.clear();
        self.supported_domains.clear();
    }
}
