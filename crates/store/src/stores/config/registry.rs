//! Source Configuration
//!
//! External configuration for extension sources as managed by the store manager.
//! This is separate from a store's internal manifest — it represents how the manager
//! configures and tracks a source, not what the source itself contains.
//!
//! - [`SourceConfig`]: How the manager configures one source (priority, trusted, enabled)
//! - [`SourceConfigs`]: A persisted collection of [`SourceConfig`] entries

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// External configuration for an extension source as managed by the store manager.
///
/// Distinct from `StoreManifest`, which is the store's own self-description:
/// - `SourceConfig`  — registry-side settings (priority, trusted, enabled)
/// - `StoreManifest` — store-side contents   (extensions, URL patterns, domains)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SourceConfig {
    /// Store identifier (must match the store's internal name)
    pub store_name: String,

    /// Registry-assigned store type identifier
    pub store_type: String,

    /// Connection URL or path to the store
    pub url: Option<String>,

    /// Human-readable description
    pub description: Option<String>,

    /// Priority for store ordering (higher = more preferred)
    pub priority: u32,

    /// Whether this source is trusted by the registry
    pub trusted: bool,

    /// Whether this source is enabled for operations
    pub enabled: bool,

    /// When this source configuration was created in the registry
    pub created_at: DateTime<Utc>,

    /// Last time this source was successfully accessed
    pub last_accessed: Option<DateTime<Utc>>,

    /// Additional configuration specific to the store type
    pub config: HashMap<String, serde_json::Value>,
}

impl SourceConfig {
    /// Create a new source configuration.
    pub fn new(store_name: String, store_type: String) -> Self {
        Self {
            store_name,
            store_type,
            url: None,
            description: None,
            priority: 100, // default middle priority
            trusted: false,
            enabled: true,
            created_at: Utc::now(),
            last_accessed: None,
            config: HashMap::new(),
        }
    }

    /// Set the store URL.
    pub fn with_url(mut self, url: String) -> Self {
        self.url = Some(url);
        self
    }

    /// Set the human-readable description.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set the priority (higher value = higher preference).
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Mark the source as trusted.
    pub fn trusted(mut self) -> Self {
        self.trusted = true;
        self
    }

    /// Mark the source as untrusted.
    pub fn untrusted(mut self) -> Self {
        self.trusted = false;
        self
    }

    /// Enable the source.
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Disable the source.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Attach an arbitrary configuration value.
    pub fn with_config_value(mut self, key: String, value: serde_json::Value) -> Self {
        self.config.insert(key, value);
        self
    }

    /// Retrieve an arbitrary configuration value.
    pub fn get_config_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.config.get(key)
    }

    /// Record the current time as the last-accessed timestamp.
    pub fn mark_accessed(&mut self) {
        self.last_accessed = Some(Utc::now());
    }

    /// Return `true` if the source was accessed within `within` of now.
    pub fn accessed_recently(&self, within: chrono::Duration) -> bool {
        if let Some(last_accessed) = self.last_accessed {
            Utc::now().signed_duration_since(last_accessed) <= within
        } else {
            false
        }
    }

    /// Return `true` if the source has not been accessed for longer than `threshold`.
    /// A source that has never been accessed is considered stale once `threshold` has
    /// elapsed since its creation time.
    pub fn is_stale(&self, threshold: chrono::Duration) -> bool {
        let reference = self.last_accessed.unwrap_or(self.created_at);
        Utc::now().signed_duration_since(reference) > threshold
    }
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self::new("unnamed".to_string(), "unknown".to_string())
    }
}

// ---------------------------------------------------------------------------
// SourceConfigs — persisted collection
// ---------------------------------------------------------------------------

/// A persisted collection of [`SourceConfig`] entries.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SourceConfigs {
    /// Map of store name → configuration.
    pub stores: HashMap<String, SourceConfig>,

    /// Last time this collection was modified.
    pub last_updated: DateTime<Utc>,

    /// Configuration format version.
    pub version: String,
}

impl SourceConfigs {
    /// Create a new, empty collection.
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
            last_updated: Utc::now(),
            version: "1.0".to_string(),
        }
    }

    /// Insert or replace a source configuration.
    pub fn add_store(&mut self, config: SourceConfig) {
        self.stores.insert(config.store_name.clone(), config);
        self.last_updated = Utc::now();
    }

    /// Remove a source configuration, returning it if it existed.
    pub fn remove_store(&mut self, store_name: &str) -> Option<SourceConfig> {
        let result = self.stores.remove(store_name);
        if result.is_some() {
            self.last_updated = Utc::now();
        }
        result
    }

    /// Look up a source configuration by name.
    pub fn get_store(&self, store_name: &str) -> Option<&SourceConfig> {
        self.stores.get(store_name)
    }

    /// Look up a source configuration mutably.
    pub fn get_store_mut(&mut self, store_name: &str) -> Option<&mut SourceConfig> {
        self.stores.get_mut(store_name)
    }

    /// Return all store names.
    pub fn store_names(&self) -> Vec<String> {
        self.stores.keys().cloned().collect()
    }

    /// Return enabled stores sorted by priority (highest first).
    pub fn enabled_stores_by_priority(&self) -> Vec<&SourceConfig> {
        let mut stores: Vec<&SourceConfig> = self.stores.values().filter(|c| c.enabled).collect();

        stores.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.store_name.cmp(&b.store_name))
        });

        stores
    }

    /// Return only trusted stores.
    pub fn trusted_stores(&self) -> Vec<&SourceConfig> {
        self.stores.values().filter(|c| c.trusted).collect()
    }

    /// Compute counts for the current collection.
    pub fn store_counts(&self) -> SourceCounts {
        let total = self.stores.len();
        let enabled = self.stores.values().filter(|c| c.enabled).count();
        let trusted = self.stores.values().filter(|c| c.trusted).count();
        let stale = self
            .stores
            .values()
            .filter(|c| c.is_stale(chrono::Duration::days(30)))
            .count();

        SourceCounts {
            total,
            enabled,
            trusted,
            stale,
        }
    }

    /// Touch the `last_updated` timestamp without making any other change.
    pub fn touch(&mut self) {
        self.last_updated = Utc::now();
    }
}

// ---------------------------------------------------------------------------
// SourceCounts — summary statistics
// ---------------------------------------------------------------------------

/// Summary statistics derived from a [`SourceConfigs`] collection.
#[derive(Debug, Clone)]
pub struct SourceCounts {
    pub total: usize,
    pub enabled: usize,
    pub trusted: usize,
    pub stale: usize,
}

impl SourceCounts {
    pub fn disabled(&self) -> usize {
        self.total - self.enabled
    }

    pub fn untrusted(&self) -> usize {
        self.total - self.trusted
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_config_creation() {
        let config = SourceConfig::new("test-store".to_string(), "local".to_string());
        assert_eq!(config.store_name, "test-store");
        assert_eq!(config.store_type, "local");
        assert_eq!(config.priority, 100);
        assert!(!config.trusted);
        assert!(config.enabled);
        assert!(config.url.is_none());
    }

    #[test]
    fn test_source_config_builder() {
        let config = SourceConfig::new("test".to_string(), "local".to_string())
            .with_url("file:///test".to_string())
            .with_description("Test store".to_string())
            .with_priority(50)
            .trusted()
            .with_config_value("custom".to_string(), serde_json::Value::Bool(true));

        assert_eq!(config.url, Some("file:///test".to_string()));
        assert_eq!(config.description, Some("Test store".to_string()));
        assert_eq!(config.priority, 50);
        assert!(config.trusted);
        assert_eq!(
            config.get_config_value("custom"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    #[test]
    fn test_access_tracking() {
        let mut config = SourceConfig::new("test".to_string(), "local".to_string());
        assert!(config.last_accessed.is_none());
        assert!(!config.accessed_recently(chrono::Duration::minutes(5)));

        config.mark_accessed();
        assert!(config.last_accessed.is_some());
        assert!(config.accessed_recently(chrono::Duration::minutes(5)));
    }

    #[test]
    fn test_source_configs_collection() {
        let mut configs = SourceConfigs::new();
        assert_eq!(configs.stores.len(), 0);

        let config1 = SourceConfig::new("store1".to_string(), "local".to_string())
            .with_priority(10)
            .enabled();
        let config2 = SourceConfig::new("store2".to_string(), "http".to_string())
            .with_priority(20)
            .trusted();

        configs.add_store(config1);
        configs.add_store(config2);

        assert_eq!(configs.stores.len(), 2);
        assert!(configs.get_store("store1").is_some());
        assert!(configs.get_store("store2").is_some());

        let enabled = configs.enabled_stores_by_priority();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].store_name, "store2"); // higher priority first
        assert_eq!(enabled[1].store_name, "store1");

        let trusted = configs.trusted_stores();
        assert_eq!(trusted.len(), 1);
        assert_eq!(trusted[0].store_name, "store2");
    }

    #[test]
    fn test_source_counts() {
        let mut configs = SourceConfigs::new();

        configs.add_store(
            SourceConfig::new("enabled-trusted".to_string(), "local".to_string())
                .enabled()
                .trusted(),
        );
        configs.add_store(
            SourceConfig::new("disabled-untrusted".to_string(), "local".to_string())
                .disabled()
                .untrusted(),
        );
        configs.add_store(
            SourceConfig::new("enabled-untrusted".to_string(), "local".to_string())
                .enabled()
                .untrusted(),
        );

        let counts = configs.store_counts();
        assert_eq!(counts.total, 3);
        assert_eq!(counts.enabled, 2);
        assert_eq!(counts.disabled(), 1);
        assert_eq!(counts.trusted, 1);
        assert_eq!(counts.untrusted(), 2);
    }

    #[test]
    fn test_serialization() {
        let config = SourceConfig::new("test".to_string(), "local".to_string())
            .with_url("file:///test".to_string())
            .trusted();

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: SourceConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.store_name, config.store_name);
        assert_eq!(deserialized.store_type, config.store_type);
        assert_eq!(deserialized.url, config.url);
        assert_eq!(deserialized.trusted, config.trusted);
    }
}
