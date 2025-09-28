use url::Url;

use crate::store_commands::{ExtensionCommands, StoreCommands};

#[derive(clap::Parser, Debug)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Fetch content from novels and websites
    Fetch {
        #[clap(subcommand)]
        command: FetchCommands,
    },
    /// Search for novels (automatically uses simple or complex search)
    Search {
        /// Search query
        query: String,
        /// Filter by author
        #[clap(long)]
        author: Option<String>,
        /// Filter by tags (switches to complex search)
        #[clap(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Filter by categories (switches to complex search)
        #[clap(long, value_delimiter = ',')]
        categories: Vec<String>,
        /// Maximum number of results
        #[clap(long)]
        limit: Option<usize>,
    },
    /// List available extensions in the registry
    List,
    /// Show registry health status
    Status,
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

#[derive(clap::Subcommand, Debug)]
pub enum FetchCommands {
    /// Fetch novel information
    Novel { url: Url },
    /// Fetch chapter content
    Chapter { url: Url },
}
