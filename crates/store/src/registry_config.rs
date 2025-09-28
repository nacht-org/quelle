//! Registry Store Configuration
//!
//! This module contains the external configuration for stores as managed by the registry.
//! This is separate from the store's internal manifest - it represents how the registry
//! configures and manages the store, not what the store contains.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// External configuration for a store as managed by the registry
///
/// This is different from StoreManifest which contains internal store metadata:
/// - RegistryStoreConfig: How the registry configures the store (priority, trusted, enabled)
/// - StoreManifest: What the store actually contains (extensions, URL patterns, domains)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RegistryStoreConfig {
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

    /// Whether this store is trusted by the registry
    pub trusted: bool,

    /// Whether this store is enabled for operations
    pub enabled: bool,

    /// When this store configuration was created in the registry
    pub created_at: DateTime<Utc>,

    /// Last time this store was successfully accessed
    pub last_accessed: Option<DateTime<Utc>>,

    /// Additional configuration specific to the store type
    pub config: HashMap<String, serde_json::Value>,
}

impl RegistryStoreConfig {
    /// Create a new registry store configuration
    pub fn new(store_name: String, store_type: String) -> Self {
        Self {
            store_name,
            store_type,
            url: None,
            description: None,
            priority: 100, // Default middle priority
            trusted: false,
            enabled: true,
            created_at: Utc::now(),
            last_accessed: None,
            config: HashMap::new(),
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

    /// Set the store priority
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Mark the store as trusted
    pub fn trusted(mut self) -> Self {
        self.trusted = true;
        self
    }

    /// Mark the store as untrusted
    pub fn untrusted(mut self) -> Self {
        self.trusted = false;
        self
    }

    /// Enable the store
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Disable the store
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add a configuration value
    pub fn with_config_value(mut self, key: String, value: serde_json::Value) -> Self {
        self.config.insert(key, value);
        self
    }

    /// Get a configuration value
    pub fn get_config_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.config.get(key)
    }

    /// Update the last accessed timestamp
    pub fn mark_accessed(&mut self) {
        self.last_accessed = Some(Utc::now());
    }

    /// Check if the store has been accessed recently (within the given duration)
    pub fn accessed_recently(&self, within: chrono::Duration) -> bool {
        if let Some(last_accessed) = self.last_accessed {
            Utc::now().signed_duration_since(last_accessed) <= within
        } else {
            false
        }
    }

    /// Check if the store is considered stale (not accessed for a long time)
    pub fn is_stale(&self, threshold: chrono::Duration) -> bool {
        if let Some(last_accessed) = self.last_accessed {
            Utc::now().signed_duration_since(last_accessed) > threshold
        } else {
            // Never accessed - consider stale after creation threshold
            Utc::now().signed_duration_since(self.created_at) > threshold
        }
    }
}

impl Default for RegistryStoreConfig {
    fn default() -> Self {
        Self::new("unnamed".to_string(), "unknown".to_string())
    }
}

/// Collection of registry store configurations
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct RegistryStoreConfigs {
    /// Map of store name to configuration
    pub stores: HashMap<String, RegistryStoreConfig>,

    /// Last time the configuration was updated
    pub last_updated: DateTime<Utc>,

    /// Configuration format version
    pub version: String,
}

impl RegistryStoreConfigs {
    /// Create a new empty store configurations collection
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
            last_updated: Utc::now(),
            version: "1.0".to_string(),
        }
    }

    /// Add or update a store configuration
    pub fn add_store(&mut self, config: RegistryStoreConfig) {
        self.stores.insert(config.store_name.clone(), config);
        self.last_updated = Utc::now();
    }

    /// Remove a store configuration
    pub fn remove_store(&mut self, store_name: &str) -> Option<RegistryStoreConfig> {
        let result = self.stores.remove(store_name);
        if result.is_some() {
            self.last_updated = Utc::now();
        }
        result
    }

    /// Get a store configuration
    pub fn get_store(&self, store_name: &str) -> Option<&RegistryStoreConfig> {
        self.stores.get(store_name)
    }

    /// Get a mutable store configuration
    pub fn get_store_mut(&mut self, store_name: &str) -> Option<&mut RegistryStoreConfig> {
        self.stores.get_mut(store_name)
    }

    /// List all store names
    pub fn store_names(&self) -> Vec<String> {
        self.stores.keys().cloned().collect()
    }

    /// List enabled stores sorted by priority
    pub fn enabled_stores_by_priority(&self) -> Vec<&RegistryStoreConfig> {
        let mut stores: Vec<&RegistryStoreConfig> = self
            .stores
            .values()
            .filter(|config| config.enabled)
            .collect();

        stores.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.store_name.cmp(&b.store_name))
        });

        stores
    }

    /// List trusted stores
    pub fn trusted_stores(&self) -> Vec<&RegistryStoreConfig> {
        self.stores
            .values()
            .filter(|config| config.trusted)
            .collect()
    }

    /// Count stores by status
    pub fn store_counts(&self) -> StoreConfigCounts {
        let total = self.stores.len();
        let enabled = self.stores.values().filter(|c| c.enabled).count();
        let trusted = self.stores.values().filter(|c| c.trusted).count();
        let stale = self
            .stores
            .values()
            .filter(|c| c.is_stale(chrono::Duration::days(30)))
            .count();

        StoreConfigCounts {
            total,
            enabled,
            disabled: total - enabled,
            trusted,
            untrusted: total - trusted,
            stale,
        }
    }

    /// Update the last_updated timestamp
    pub fn touch(&mut self) {
        self.last_updated = Utc::now();
    }
}

/// Statistics about store configurations
#[derive(Debug, Clone)]
pub struct StoreConfigCounts {
    pub total: usize,
    pub enabled: usize,
    pub disabled: usize,
    pub trusted: usize,
    pub untrusted: usize,
    pub stale: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_store_config_creation() {
        let config = RegistryStoreConfig::new("test-store".to_string(), "local".to_string());
        assert_eq!(config.store_name, "test-store");
        assert_eq!(config.store_type, "local");
        assert_eq!(config.priority, 100);
        assert!(!config.trusted);
        assert!(config.enabled);
        assert!(config.url.is_none());
    }

    #[test]
    fn test_registry_store_config_builder() {
        let config = RegistryStoreConfig::new("test".to_string(), "local".to_string())
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
        let mut config = RegistryStoreConfig::new("test".to_string(), "local".to_string());
        assert!(config.last_accessed.is_none());
        assert!(!config.accessed_recently(chrono::Duration::minutes(5)));

        config.mark_accessed();
        assert!(config.last_accessed.is_some());
        assert!(config.accessed_recently(chrono::Duration::minutes(5)));
    }

    #[test]
    fn test_store_configs_collection() {
        let mut configs = RegistryStoreConfigs::new();
        assert_eq!(configs.stores.len(), 0);

        let config1 = RegistryStoreConfig::new("store1".to_string(), "local".to_string())
            .with_priority(10)
            .enabled();
        let config2 = RegistryStoreConfig::new("store2".to_string(), "http".to_string())
            .with_priority(20)
            .trusted();

        configs.add_store(config1);
        configs.add_store(config2);

        assert_eq!(configs.stores.len(), 2);
        assert!(configs.get_store("store1").is_some());
        assert!(configs.get_store("store2").is_some());

        let enabled = configs.enabled_stores_by_priority();
        assert_eq!(enabled.len(), 2);
        assert_eq!(enabled[0].store_name, "store2"); // Higher priority first
        assert_eq!(enabled[1].store_name, "store1");

        let trusted = configs.trusted_stores();
        assert_eq!(trusted.len(), 1);
        assert_eq!(trusted[0].store_name, "store2");
    }

    #[test]
    fn test_store_counts() {
        let mut configs = RegistryStoreConfigs::new();

        configs.add_store(
            RegistryStoreConfig::new("enabled-trusted".to_string(), "local".to_string())
                .enabled()
                .trusted(),
        );
        configs.add_store(
            RegistryStoreConfig::new("disabled-untrusted".to_string(), "local".to_string())
                .disabled()
                .untrusted(),
        );
        configs.add_store(
            RegistryStoreConfig::new("enabled-untrusted".to_string(), "local".to_string())
                .enabled()
                .untrusted(),
        );

        let counts = configs.store_counts();
        assert_eq!(counts.total, 3);
        assert_eq!(counts.enabled, 2);
        assert_eq!(counts.disabled, 1);
        assert_eq!(counts.trusted, 1);
        assert_eq!(counts.untrusted, 2);
    }

    #[test]
    fn test_serialization() {
        let config = RegistryStoreConfig::new("test".to_string(), "local".to_string())
            .with_url("file:///test".to_string())
            .trusted();

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RegistryStoreConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.store_name, config.store_name);
        assert_eq!(deserialized.store_type, config.store_type);
        assert_eq!(deserialized.url, config.url);
        assert_eq!(deserialized.trusted, config.trusted);
    }
}
