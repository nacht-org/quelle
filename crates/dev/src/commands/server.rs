//! Development server command handler

use eyre::Result;

use crate::server;

/// Handle development server command
pub async fn handle(extension_name: String, watch: bool, use_chrome: bool) -> Result<()> {
    server::start(extension_name, watch, use_chrome).await
}
