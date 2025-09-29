use url::Url;

#[derive(clap::Parser, Debug)]
#[command(name = "quelle")]
#[command(about = "A novel scraper and library manager")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Override storage location
    #[arg(long, global = true)]
    pub storage_path: Option<String>,

    /// Use custom config file
    #[arg(long, global = true)]
    pub config: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Quiet output
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Show what would be done without executing
    #[arg(long, global = true)]
    pub dry_run: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Fetch content from novels and websites
    Fetch {
        #[command(subcommand)]
        command: FetchCommands,
    },
    /// Manage local library of stored novels and chapters
    Library {
        #[command(subcommand)]
        command: LibraryCommands,
    },
    /// Export content to various formats
    Export {
        #[command(subcommand)]
        command: ExportCommands,
    },
    /// Search for novels across extensions
    Search {
        /// Search query
        query: String,
        /// Filter by author
        #[arg(long)]
        author: Option<String>,
        /// Filter by tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Filter by source
        #[arg(long)]
        source: Option<String>,
        /// Maximum number of results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Manage extensions
    Extension {
        #[command(subcommand)]
        command: ExtensionCommands,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum FetchCommands {
    /// Fetch novel metadata and cover
    Novel { url: Url },
    /// Fetch chapter content and images
    Chapter { url: Url },
    /// Fetch all chapters for a novel
    Chapters {
        /// Novel ID
        novel_id: String,
    },
    /// Fetch everything (novel + all chapters + assets)
    All { url: Url },
}

#[derive(clap::Subcommand, Debug)]
pub enum LibraryCommands {
    /// List all stored novels
    List {
        /// Filter by source
        #[arg(long)]
        source: Option<String>,
    },
    /// Show details for a stored novel
    Show {
        /// Novel ID
        novel_id: String,
    },
    /// List chapters for a stored novel
    Chapters {
        /// Novel ID
        novel_id: String,
        /// Show only chapters with downloaded content
        #[arg(long)]
        downloaded_only: bool,
    },
    /// Read a specific chapter
    Read {
        /// Novel ID
        novel_id: String,
        /// Chapter number or URL
        chapter: String,
    },
    /// Check for new chapters
    Sync {
        /// Novel ID (or 'all' for all novels)
        novel_id: String,
    },
    /// Fetch new chapters
    Update {
        /// Novel ID (or 'all' for all novels)
        novel_id: String,
    },
    /// Remove a stored novel and all its data
    Remove {
        /// Novel ID
        novel_id: String,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Clean up orphaned data and fix inconsistencies
    Cleanup,
    /// Show library statistics
    Stats,
}

#[derive(clap::Subcommand, Debug)]
pub enum ExportCommands {
    /// Export to EPUB format
    Epub {
        /// Novel ID (or 'all' for all novels)
        novel_id: String,
        /// Chapter range (e.g., "1-10", "5", "1,3,5-10")
        #[arg(long)]
        chapters: Option<String>,
        /// Output directory
        #[arg(long)]
        output: Option<String>,
        /// Custom template
        #[arg(long)]
        template: Option<String>,
        /// Combine volumes into single file
        #[arg(long)]
        combine_volumes: bool,
        /// Export only novels updated since last export
        #[arg(long)]
        updated: bool,
    },
    /// Export to PDF format (future)
    Pdf {
        /// Novel ID
        novel_id: String,
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Export to HTML format (future)
    Html {
        /// Novel ID
        novel_id: String,
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Export to plain text (future)
    Txt {
        /// Novel ID
        novel_id: String,
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ExtensionCommands {
    /// Install an extension
    Install {
        /// Extension ID
        id: String,
        /// Specific version to install
        #[arg(long)]
        version: Option<String>,
        /// Force reinstallation
        #[arg(long)]
        force: bool,
    },
    /// List installed extensions
    List {
        /// Show detailed information
        #[arg(long)]
        detailed: bool,
    },
    /// Update an extension
    Update {
        /// Extension ID (or 'all' for all extensions)
        id: String,
        /// Include pre-release versions
        #[arg(long)]
        prerelease: bool,
        /// Force update even if no new version
        #[arg(long)]
        force: bool,
    },
    /// Remove an extension
    Remove {
        /// Extension ID
        id: String,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// Search for available extensions
    Search {
        /// Search query
        query: String,
        /// Filter by author
        #[arg(long)]
        author: Option<String>,
        /// Maximum number of results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Show extension information
    Info {
        /// Extension ID
        id: String,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ConfigCommands {
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., "storage.path")
        key: String,
        /// Configuration value
        value: String,
    },
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },
    /// Show all configuration
    Show,
    /// Reset configuration to defaults
    Reset {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}
