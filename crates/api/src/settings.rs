use std::path::PathBuf;

use once_cell::sync::Lazy;
use serde::{Deserialize, de::DeserializeOwned};
use url::Url;

/// Loads configuration settings by merging base, environment-specific, and environment variable sources.
///
/// The environment is determined by the `APP_ENVIRONMENT` variable (defaults to "dev").
/// Panics if the current directory cannot be determined or if the environment variable is invalid.
///
/// This function is typically used at application startup to hydrate strongly-typed settings.
pub fn get_settings<T>(dir: &str) -> Result<T, config::ConfigError>
where
    T: DeserializeOwned,
{
    let base_path = std::env::current_dir().expect("Failed to determine current directory");
    let config_dir = base_path.join(dir);

    let env: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "dev".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let settings: T = config::Config::builder()
        .add_source(config::File::from(config_dir.join("base")))
        .add_source(config::File::from(config_dir.join(env.as_str())).required(false))
        .add_source(
            config::Environment::with_prefix("app")
                .prefix_separator("__")
                .separator("__"),
        )
        .build()?
        .try_deserialize()?;

    Ok(settings)
}

pub static ENVIRONMENT: Lazy<Environment> = once_cell::sync::Lazy::new(|| {
    std::env::var("APP_ENVIRONMENT")
        .ok()
        .and_then(|env| match Environment::try_from(env) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!("Failed to parse APP_ENVIRONMENT: {e}. Defaulting to `dev`.");
                None
            }
        })
        .unwrap_or(Environment::Dev)
});

/// Application environment, used to distinguish between development and production modes.
///
/// This is typically set via the `APP_ENVIRONMENT` environment variable and influences configuration loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Dev,
    Prod,
}

impl Environment {
    /// Returns the canonical string for this environment ("dev" or "prod").
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Prod => "prod",
        }
    }
}

/// Allows parsing an `Environment` from a string, accepting only "dev" or "prod" (case-insensitive).
///
/// Returns an error for unsupported values.
impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "dev" => Ok(Self::Dev),
            "prod" => Ok(Self::Prod),
            other => Err(format!(
                "{other} is not a supported environment. Use either `dev` or `prod`."
            )),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Settings {
    #[serde(default)]
    pub server: ServerSettings,
    #[serde(default)]
    pub data: DataSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    #[serde(default = "default_server_url")]
    pub url: Url,
    #[serde(default = "default_server_host")]
    pub host: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            url: default_server_url(),
            host: default_server_host(),
            port: default_server_port(),
        }
    }
}

fn default_server_url() -> Url {
    Url::parse("http://localhost:8080").expect("Failed to parse default server URL")
}

fn default_server_host() -> String {
    "localhost".to_string()
}

fn default_server_port() -> u16 {
    8080
}

#[derive(Debug, Clone, Deserialize)]
pub struct DataSettings {
    #[serde(default = "default_data_base_dir")]
    pub base_dir: PathBuf,
}

impl Default for DataSettings {
    fn default() -> Self {
        Self {
            base_dir: default_data_base_dir(),
        }
    }
}

#[cfg(debug_assertions)]
fn default_data_base_dir() -> PathBuf {
    std::env::current_dir()
        .expect("Failed to determine current directory")
        .join("data")
}

#[cfg(not(debug_assertions))]
fn default_data_base_dir() -> PathBuf {
    std::env::current_exe()
        .expect("Failed to determine current executable path")
        .parent()
        .expect("Executable must have a parent directory")
        .join("data")
}

impl DataSettings {
    pub fn get_registry_dir(&self) -> PathBuf {
        self.base_dir.join("registry")
    }

    pub fn get_stores_dir(&self) -> PathBuf {
        self.base_dir.join("stores")
    }
}
