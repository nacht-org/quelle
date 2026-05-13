//! Command definitions and handlers for development tools

use clap::Subcommand;
use eyre::Result;
use url::Url;

pub mod generate;
pub mod test;
pub mod validate;

/// Development commands available in the CLI
#[derive(Subcommand, Debug, Clone)]
pub enum DevCommands {
    /// Interactive testing shell for extensions
    Test {
        /// Extension name to test
        extension: String,
        /// Test URL for novel info testing
        #[arg(long)]
        url: Option<Url>,
        /// Test search query
        #[arg(long)]
        query: Option<String>,
    },
    /// Generate a new extension from template
    Generate {
        /// Extension name (lowercase, no spaces)
        name: String,
        /// Display name for the extension
        #[arg(long)]
        display_name: String,
        /// Base URL of the target website
        #[arg(long)]
        base_url: String,
        /// Primary language code (default: en)
        #[arg(long, default_value = "en")]
        language: String,
        /// Reading direction (ltr or rtl)
        #[arg(long, default_value = "ltr")]
        reading_direction: String,
        /// Force overwrite if extension already exists
        #[arg(long)]
        force: bool,
    },
    /// Validate extension without publishing
    Validate {
        /// Extension name to validate
        extension: String,
    },
}

/// Handle development commands
pub async fn handle_command(cmd: DevCommands) -> Result<()> {
    match cmd {
        DevCommands::Test {
            extension,
            url,
            query,
        } => test::run(extension, url, query).await,
        DevCommands::Generate {
            name,
            display_name,
            base_url,
            language,
            reading_direction,
            force,
        } => {
            generate::handle(
                name,
                display_name,
                base_url,
                language,
                reading_direction,
                force,
            )
            .await
        }
        DevCommands::Validate { extension } => validate::handle(extension).await,
    }
}
