//! Extension engine construction helpers.

use eyre::Result;
use quelle_engine::{
    ExtensionEngine,
    http::{GhostwireExecutor, HeadlessChromeExecutor, ReqwestExecutor},
};

#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum Executor {
    #[default]
    Ghostwire,
    Chrome,
    Reqwest,
}

pub fn create_extension_engine() -> Result<ExtensionEngine> {
    create_extension_engine_with_executor(Executor::default())
}

pub fn create_extension_engine_with_executor(executor: Executor) -> Result<ExtensionEngine> {
    match executor {
        Executor::Ghostwire => create_ghostwire_engine(),
        Executor::Chrome => try_create_chrome_engine().or_else(|e| {
            tracing::warn!("Failed to create Chrome executor, falling back to Ghostwire: {e}");
            create_ghostwire_engine()
        }),
        Executor::Reqwest => create_reqwest_engine(),
    }
}

fn create_ghostwire_engine() -> Result<ExtensionEngine> {
    tracing::info!("Using Ghostwire executor for extensions");
    let executor = std::sync::Arc::new(GhostwireExecutor::new().map_err(eyre::Report::from)?);
    ExtensionEngine::new(executor).map_err(eyre::Report::from)
}

fn try_create_chrome_engine() -> Result<ExtensionEngine> {
    tracing::info!("Using HeadlessChrome executor for extensions");
    let executor = std::sync::Arc::new(HeadlessChromeExecutor::new());
    ExtensionEngine::new(executor).map_err(eyre::Report::from)
}

fn create_reqwest_engine() -> Result<ExtensionEngine> {
    tracing::info!("Using Reqwest executor for extensions");
    let executor = std::sync::Arc::new(ReqwestExecutor::new());
    ExtensionEngine::new(executor).map_err(eyre::Report::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let result = create_extension_engine();
        assert!(result.is_ok(), "Engine creation should succeed");
    }
}
