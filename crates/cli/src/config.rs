use directories::ProjectDirs;
use eyre::Result;
use quelle_store::{RegistryConfig, StoreManager};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::{self};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
    #[serde(default)]
    pub export: ExportConfig,
    #[serde(default)]
    pub fetch: FetchConfig,
    #[serde(default)]
    pub registry: RegistryConfig,
    #[serde(default)]
    pub official: OfficialConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExportConfig {
    pub format: String,
    pub include_covers: bool,
    pub output_dir: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FetchConfig {
    pub auto_fetch_covers: bool,
    pub auto_fetch_assets: bool,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            format: "epub".to_string(),
            include_covers: true,
            output_dir: None,
        }
    }
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            auto_fetch_covers: true,
            auto_fetch_assets: true,
        }
    }
}

impl Config {
    pub fn get_config_path() -> PathBuf {
        get_default_config_dir().join("config.json")
    }

    pub fn get_data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(get_default_data_dir)
    }

    pub fn get_registry_dir(&self) -> PathBuf {
        self.get_data_dir().join("registry")
    }

    pub fn get_storage_path(&self) -> PathBuf {
        self.get_data_dir().join("library")
    }

    pub fn get_stores_dir(&self) -> PathBuf {
        self.get_data_dir().join("stores")
    }

    pub async fn load() -> Result<Self> {
        let config_path = Self::get_config_path();
        Self::load_from(&config_path).await
    }

    /// Load configuration from a specific path instead of the default location.
    pub async fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(path).await?;
        let mut config: Config = serde_json::from_str(&content)?;

        #[cfg(feature = "git")]
        config.add_official_source().await?;

        Ok(config)
    }

    pub async fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Filter out the official store before saving
        let mut config = self.clone();
        config
            .registry
            .extension_sources
            .retain(|s| s.name != "official");

        let content = serde_json::to_string_pretty(&config)?;
        fs::write(&config_path, content).await?;

        Ok(())
    }

    pub async fn add_official_source(&mut self) -> Result<()> {
        if !self.official.enabled {
            tracing::debug!("Official extensions are disabled in the configuration");
            return Ok(());
        }

        #[cfg(feature = "github")]
        let official_store = quelle_store::ExtensionSource::official_github(&self.get_stores_dir());
        #[cfg(not(feature = "github"))]
        let official_store = quelle_store::ExtensionSource::official(&self.get_stores_dir());

        if self
            .registry
            .extension_sources
            .iter()
            .any(|s| s.name == official_store.name)
        {
            tracing::warn!("An 'official' extension source is already configured");
            return Ok(());
        }

        self.registry.add_source(official_store);

        Ok(())
    }

    /// Apply registry configuration to the store manager
    pub async fn apply(&self, store_manager: &mut StoreManager) -> Result<()> {
        self.registry
            .apply(store_manager)
            .await
            .map_err(|e| eyre::eyre!("Failed to apply registry config: {}", e))
    }

    pub async fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = key.split('.').collect();

        match parts.as_slice() {
            ["data_dir"] => {
                self.data_dir = if value.is_empty() {
                    None
                } else {
                    // Ensure the directory exists
                    let path = PathBuf::from(value);
                    if !path.exists() {
                        fs::create_dir_all(&path).await?;
                    }

                    // Convert to absolute path
                    let canonical_path = fs::canonicalize(path).await?;

                    Some(canonical_path)
                };
            }
            ["export", "format"] => {
                self.export.format = value.to_string();
            }
            ["export", "include_covers"] => {
                self.export.include_covers = value
                    .parse::<bool>()
                    .map_err(|_| eyre::eyre!("Invalid boolean value: {}", value))?;
            }
            ["export", "output_dir"] => {
                self.export.output_dir = if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            ["fetch", "auto_fetch_covers"] => {
                self.fetch.auto_fetch_covers = value
                    .parse::<bool>()
                    .map_err(|_| eyre::eyre!("Invalid boolean value: {}", value))?;
            }
            ["fetch", "auto_fetch_assets"] => {
                self.fetch.auto_fetch_assets = value
                    .parse::<bool>()
                    .map_err(|_| eyre::eyre!("Invalid boolean value: {}", value))?;
            }
            ["official", "enabled"] => {
                self.official.enabled = value
                    .parse::<bool>()
                    .map_err(|_| eyre::eyre!("Invalid boolean value: {}", value))?;
            }
            _ => {
                return Err(eyre::eyre!("Unknown configuration key: {}", key));
            }
        }

        Ok(())
    }

    pub fn get_value(&self, key: &str) -> Result<String> {
        let parts: Vec<&str> = key.split('.').collect();

        let value = match parts.as_slice() {
            ["data_dir"] => self
                .data_dir
                .clone()
                .unwrap_or_else(|| "(default)".into())
                .to_string_lossy()
                .to_string(),
            ["export", "format"] => self.export.format.clone(),
            ["export", "include_covers"] => self.export.include_covers.to_string(),
            ["export", "output_dir"] => self.export.output_dir.clone().unwrap_or_default(),
            ["fetch", "auto_fetch_covers"] => self.fetch.auto_fetch_covers.to_string(),
            ["fetch", "auto_fetch_assets"] => self.fetch.auto_fetch_assets.to_string(),
            ["official", "enabled"] => self.official.enabled.to_string(),
            _ => {
                return Err(eyre::eyre!("Unknown configuration key: {}", key));
            }
        };

        Ok(value)
    }

    /// Return a flat `key: value` representation of all configuration fields.
    pub fn show_all(&self) -> String {
        let data_dir = self
            .data_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| get_default_data_dir().display().to_string());

        let storage_path = self.get_storage_path().display().to_string();

        let output_dir = self
            .export
            .output_dir
            .as_deref()
            .unwrap_or("(not set)")
            .to_string();

        let extension_sources = if self.registry.extension_sources.is_empty() {
            "(none)".to_string()
        } else {
            self.registry
                .extension_sources
                .iter()
                .map(|s| format!("  {} (priority: {})", s.name, s.priority))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let mut lines = vec![
            format!("data_dir: {}", data_dir),
            format!("storage_path: {}", storage_path),
            format!("export.format: {}", self.export.format),
            format!("export.include_covers: {}", self.export.include_covers),
            format!("export.output_dir: {}", output_dir),
            format!("fetch.auto_fetch_covers: {}", self.fetch.auto_fetch_covers),
            format!("fetch.auto_fetch_assets: {}", self.fetch.auto_fetch_assets),
            format!("official.enabled: {}", self.official.enabled),
        ];

        if self.registry.extension_sources.is_empty() {
            lines.push("extension_sources: (none)".to_string());
        } else {
            lines.push("extension_sources:".to_string());
            lines.push(extension_sources);
        }

        lines.join("\n")
    }

    pub async fn reset() -> Result<Self> {
        let config = Self::default();
        config.save().await?;
        Ok(config)
    }
}

/// Get the default configuration directory
fn get_default_config_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("org", "quelle", "quelle") {
        proj_dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from(".quelle").join("config")
    }
}

/// Get the default data directory
pub fn get_default_data_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("org", "quelle", "quelle") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        PathBuf::from(".quelle").join("data")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OfficialConfig {
    pub enabled: bool,
}

impl Default for OfficialConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_storage_path() {
        let config = Config::default();
        let storage_path = config.get_storage_path();
        // Storage path should end with "library"
        assert_eq!(storage_path.file_name().unwrap(), "library");
    }

    #[test]
    fn test_show_all_flat_format() {
        let config = Config::default();
        let output = config.show_all();
        // Should not contain box-drawing characters
        assert!(!output.contains('├'));
        assert!(!output.contains('└'));
        assert!(!output.contains('─'));
        // Should contain flat key: value lines
        assert!(output.contains("data_dir: "));
        assert!(output.contains("storage_path: "));
        assert!(output.contains("export.format: epub"));
        assert!(output.contains("export.include_covers: true"));
        assert!(output.contains("fetch.auto_fetch_covers: true"));
        assert!(output.contains("official.enabled: true"));
        assert!(output.contains("extension_sources: (none)"));
    }
}
