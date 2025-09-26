use url::Url;

use crate::store_commands::{ExtensionCommands, StoreCommands};

#[derive(clap::Parser, Debug)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Fetch novel information
    Novel { url: Url },
    /// Fetch chapter content
    Chapter { url: Url },
    /// Search for novels
    Search { query: String },
    /// Manage extension stores
    Store {
        #[clap(subcommand)]
        command: StoreCommands,
    },
    /// Manage extensions
    Extension {
        #[clap(subcommand)]
        command: ExtensionCommands,
    },
}
