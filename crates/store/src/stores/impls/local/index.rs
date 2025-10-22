use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::ExtensionVersion;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct LocalStoreManifestIndex {
    pub url_patterns: Vec<UrlPattern>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UrlPattern {
    /// URL prefix that this pattern matches (e.g., "https://example.com")
    pub url_prefix: String,
    /// Extensions that can handle URLs matching this prefix
    pub extensions: BTreeSet<String>,
}

impl LocalStoreManifestIndex {
    /// Add a URL pattern for extension matching
    pub(crate) fn add_url_pattern(&mut self, url_prefix: String, extension: String) {
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
            });
        }
    }

    pub(crate) fn regenerate<'a>(
        &mut self,
        extension_versions: impl Iterator<Item = &'a ExtensionVersion>,
    ) {
        self.url_patterns.clear();

        for extension in extension_versions {
            for base_url in &extension.base_urls {
                self.add_url_pattern(base_url.clone(), extension.id.clone());
            }
        }
    }

    pub(crate) fn reset(&mut self) {
        self.url_patterns.clear();
    }
}
