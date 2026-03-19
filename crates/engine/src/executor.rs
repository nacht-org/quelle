//! HTTP executor selection.

use crate::ExtensionEngine;
use crate::http::{GhostwireExecutor, HeadlessChromeExecutor, ReqwestExecutor};
use eyre::Result;
use std::sync::Arc;

/// The HTTP executor backend to use for extension requests.
#[derive(Debug, Clone, Default, clap::ValueEnum)]
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
    match executor {
        Executor::Ghostwire => {
            tracing::info!("Using Ghostwire executor");
            let e = Arc::new(GhostwireExecutor::new().map_err(eyre::Report::from)?);
            ExtensionEngine::new(e).map_err(eyre::Report::from)
        }
        Executor::Chrome => create_chrome_engine().or_else(|err| {
            tracing::warn!("Chrome executor failed, falling back to Ghostwire: {err}");
            let e = Arc::new(GhostwireExecutor::new().map_err(eyre::Report::from)?);
            ExtensionEngine::new(e).map_err(eyre::Report::from)
        }),
        Executor::Reqwest => {
            tracing::info!("Using Reqwest executor");
            let e = Arc::new(ReqwestExecutor::new());
            ExtensionEngine::new(e).map_err(eyre::Report::from)
        }
    }
}

fn create_chrome_engine() -> Result<ExtensionEngine> {
    tracing::info!("Using HeadlessChrome executor");
    let e = Arc::new(HeadlessChromeExecutor::new());
    ExtensionEngine::new(e).map_err(eyre::Report::from)
}
