//! HTTP executor selection.

use crate::ExtensionEngine;
use crate::http::{GhostwireExecutor, HeadlessChromeExecutor, HttpExecutor, ReqwestExecutor};
use eyre::Result;
use std::sync::Arc;

/// The HTTP executor backend to use for extension requests.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Executor {
    /// Ghostwire cloud-scraper — bypasses Cloudflare and other bot protections (default)
    #[default]
    Ghostwire,
    /// Headless Chrome — handles JavaScript-heavy sites; falls back to Ghostwire on failure
    Chrome,
    /// Plain reqwest — fast and lightweight, no JS support
    Reqwest,
}

/// Create an `ExtensionEngine` using the given executor choice.
pub fn create_engine(executor: Executor) -> Result<ExtensionEngine> {
    let http_executor = create_http_executor(executor)?;
    ExtensionEngine::new(http_executor).map_err(|e| eyre::eyre!(e))
}

pub fn create_http_executor(executor: Executor) -> Result<Arc<dyn HttpExecutor>> {
    match executor {
        Executor::Ghostwire => {
            tracing::info!("Using Ghostwire executor");
            let e = Arc::new(GhostwireExecutor::new().map_err(eyre::Report::from)?);
            Ok(e)
        }
        Executor::Chrome => create_chrome_engine().or_else(|err| {
            tracing::warn!("Chrome executor failed, falling back to Ghostwire: {err}");
            let e = Arc::new(GhostwireExecutor::new().map_err(eyre::Report::from)?);
            Ok(e)
        }),
        Executor::Reqwest => {
            tracing::info!("Using Reqwest executor");
            let e = Arc::new(ReqwestExecutor::new());
            Ok(e)
        }
    }
}

fn create_chrome_engine() -> Result<Arc<dyn HttpExecutor>> {
    tracing::info!("Using HeadlessChrome executor");
    let e = Arc::new(HeadlessChromeExecutor::new());
    Ok(e)
}
