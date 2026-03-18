//! Development server command handler

use eyre::Result;

use crate::server::{self, Executor};

/// Handle development server command
pub async fn handle(extension_name: String, watch: bool, executor: Executor) -> Result<()> {
    server::start(extension_name, watch, executor).await
}
