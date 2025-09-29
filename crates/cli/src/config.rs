use directories::ProjectDirs;
use eyre::Result;
use quelle_store::{RegistryConfig, StoreManager};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[derive(Default)]
pub struct Config {
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub export: ExportConfig,
    #[serde(default)]
    pub fetch: FetchConfig,
    #[serde(default)]
    pub registry: RegistryConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageConfig {
    pub path: String,
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

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            path: get_default_data_dir()
                .join("library")
                .to_string_lossy()
                .to_string(),
        }
    }
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

    pub fn get_data_dir() -> PathBuf {
        get_default_data_dir()
    }

    pub fn get_registry_dir() -> PathBuf {
        get_default_data_dir().join("registry")
    }

    pub async fn load() -> Result<Self> {
        let config_path = Self::get_config_path();

        if !config_path.exists() {
            let default_config = Self::default();
            default_config.save().await?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(&config_path).await?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    pub async fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&config_path, content).await?;
        Ok(())
    }

    /// Apply registry configuration to the store manager
    pub async fn apply(&self, store_manager: &mut StoreManager) -> Result<()> {
        self.registry
            .apply(store_manager)
            .await
            .map_err(|e| eyre::eyre!("Failed to apply registry config: {}", e))
    }

    pub fn set_value(&mut self, key: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = key.split('.').collect();

        match parts.as_slice() {
            ["storage", "path"] => {
                self.storage.path = value.to_string();
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
            _ => {
                return Err(eyre::eyre!("Unknown configuration key: {}", key));
            }
        }

        Ok(())
    }

    pub fn get_value(&self, key: &str) -> Result<String> {
        let parts: Vec<&str> = key.split('.').collect();

        let value = match parts.as_slice() {
            ["storage", "path"] => self.storage.path.clone(),
            ["export", "format"] => self.export.format.clone(),
            ["export", "include_covers"] => self.export.include_covers.to_string(),
            ["export", "output_dir"] => self
                .export
                .output_dir
                .clone()
                .unwrap_or_default(),
            ["fetch", "auto_fetch_covers"] => self.fetch.auto_fetch_covers.to_string(),
            ["fetch", "auto_fetch_assets"] => self.fetch.auto_fetch_assets.to_string(),
            _ => {
                return Err(eyre::eyre!("Unknown configuration key: {}", key));
            }
        };

        Ok(value)
    }

    pub fn show_all(&self) -> String {
        let registry_sources = if self.registry.extension_sources.is_empty() {
            "(none configured)".to_string()
        } else {
            self.registry
                .extension_sources
                .iter()
                .map(|s| format!("{} (priority: {})", s.name, s.priority))
                .collect::<Vec<_>>()
                .join(", ")
        };

        format!(
            "Configuration:\n\
             Storage:\n\
             ├─ path: {}\n\
             Export:\n\
             ├─ format: {}\n\
             ├─ include_covers: {}\n\
             └─ output_dir: {}\n\
             Fetch:\n\
             ├─ auto_fetch_covers: {}\n\
             └─ auto_fetch_assets: {}\n\
             Registry:\n\
             └─ extension_sources: {}",
            self.storage.path,
            self.export.format,
            self.export.include_covers,
            self.export
                .output_dir
                .as_ref()
                .unwrap_or(&"(not set)".to_string()),
            self.fetch.auto_fetch_covers,
            self.fetch.auto_fetch_assets,
            registry_sources
        )
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
        // Fallback to current directory if we can't determine project dirs
        PathBuf::from(".quelle").join("config")
    }
}

/// Get the default data directory
fn get_default_data_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("org", "quelle", "quelle") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        // Fallback to current directory if we can't determine project dirs
        PathBuf::from(".quelle").join("data")
    }
}
