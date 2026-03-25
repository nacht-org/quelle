use once_cell::sync::Lazy;
use serde::{Deserialize, de::DeserializeOwned};

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

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
}
