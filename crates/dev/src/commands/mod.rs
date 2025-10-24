//! Command definitions and handlers for development tools

use clap::Subcommand;
use eyre::Result;
use url::Url;

pub mod generate;
pub mod server;
pub mod test;
pub mod validate;

/// Development commands available in the CLI
#[derive(Subcommand, Debug, Clone)]
pub enum DevCommands {
    /// Start development server with hot reload
    Server {
        /// Extension name to develop
        extension: String,
        /// Auto-rebuild on file changes
        #[arg(long, default_value = "true")]
        watch: bool,
        /// Use Chrome HTTP executor instead of Reqwest (better for JS-heavy sites)
        #[arg(long, default_value = "true")]
        chrome: bool,
    },
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
        /// Enable verbose logging
        #[arg(long, short)]
        verbose: bool,
    },
    /// Generate a new extension from template
    Generate {
        /// Extension name (lowercase, no spaces)
        name: Option<String>,
        /// Display name for the extension
        #[arg(long)]
        display_name: Option<String>,
        /// Base URL of the target website
        #[arg(long)]
        base_url: Option<String>,
        /// Primary language code (default: en)
        #[arg(long)]
        language: Option<String>,
        /// Reading direction (ltr or rtl)
        #[arg(long)]
        reading_direction: Option<String>,
        /// Force overwrite if extension already exists
        #[arg(long)]
        force: bool,
    },
    /// Validate extension without publishing
    Validate {
        /// Extension name to validate
        extension: String,
        /// Run extended validation tests
        #[arg(long)]
        extended: bool,
    },
}

/// Handle development commands
pub async fn handle_command(cmd: DevCommands) -> Result<()> {
    match cmd {
        DevCommands::Server {
            extension,
            watch,
            chrome,
        } => server::handle(extension, watch, chrome).await,
        DevCommands::Test {
            extension,
            url,
            query,
            verbose: _,
        } => test::start_interactive(extension, url, query).await,
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
        DevCommands::Validate {
            extension,
            extended,
        } => validate::handle(extension, extended).await,
    }
}
