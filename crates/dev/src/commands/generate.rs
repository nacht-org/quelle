//! Extension generation command handler

use eyre::Result;

use crate::generator;

/// Handle extension generation command
pub async fn handle(
    name: Option<String>,
    display_name: Option<String>,
    base_url: Option<String>,
    language: Option<String>,
    reading_direction: Option<String>,
    force: bool,
) -> Result<()> {
    generator::handle(
        name,
        display_name,
        base_url,
        language,
        reading_direction,
        force,
    )
    .await
}
