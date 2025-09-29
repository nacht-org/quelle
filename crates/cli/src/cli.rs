use std::path::PathBuf;
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
    /// Search for novels (automatically uses simple or complex search)
    Search {
        /// Search query
        query: String,
        /// Filter by author
        #[arg(long)]
        author: Option<String>,
        /// Filter by tags (switches to complex search)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Filter by categories (switches to complex search)
        #[arg(long, value_delimiter = ',')]
        categories: Vec<String>,
        /// Maximum number of results
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Manage local library of stored novels and chapters
    Library {
        #[command(subcommand)]
        command: LibraryCommands,
    },
    /// List available extensions in the registry
    List,
    /// Show registry health status
    Status,
    /// Manage extension stores
    Store {
        #[command(subcommand)]
        command: StoreCommands,
    },
    /// Manage extensions
    Extension {
        #[command(subcommand)]
        command: ExtensionCommands,
    },
    /// Export content to various formats
    Export {
        #[command(subcommand)]
        command: ExportCommands,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Publish and manage extensions
    Publish {
        #[command(subcommand)]
        command: PublishCommands,
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
        /// Novel ID or URL
        novel_id: String,
    },
    /// Fetch everything (novel + all chapters + assets)
    All { url: Url },
}

#[derive(clap::Subcommand, Debug)]
pub enum LibraryCommands {
    /// List all stored novels
    List {
        /// Filter by source (e.g., "webnovel", "royalroad")
        #[arg(long)]
        source: Option<String>,
    },
    /// Show details for a stored novel
    Show {
        /// Novel ID or URL
        novel_id: String,
    },
    /// List chapters for a stored novel
    Chapters {
        /// Novel ID or URL
        novel_id: String,
        /// Show only chapters with downloaded content
        #[arg(long)]
        downloaded_only: bool,
    },
    /// Read a specific chapter
    Read {
        /// Novel ID or URL
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
        /// Novel ID or URL
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
pub enum StoreCommands {
    /// Add a new extension store
    Add {
        /// Store name
        name: String,
        /// Store path
        path: String,
        /// Priority (lower = higher priority)
        #[arg(long, default_value = "100")]
        priority: u32,
    },
    /// Remove an extension store
    Remove {
        /// Store name
        name: String,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
    /// List configured extension stores
    List,
    /// Update extension store data
    Update {
        /// Store name (or 'all' for all stores)
        name: String,
    },
    /// Show store information
    Info {
        /// Store name
        name: String,
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
pub enum ExportCommands {
    /// Export to EPUB format
    Epub {
        /// Novel ID (or 'all' for all novels)
        novel_id: String,
        /// Output directory
        #[arg(long)]
        output: Option<String>,
        /// Include images in export
        #[arg(long)]
        include_images: bool,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum ConfigCommands {
    /// Set a configuration value
    Set {
        /// Configuration key (e.g., "data_dir", "export.format")
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

#[derive(clap::Subcommand, Debug)]
pub enum PublishCommands {
    /// Publish an extension (new or updated version)
    Extension {
        /// Path to extension package or directory
        package_path: PathBuf,
        /// Target store name
        #[arg(long)]
        store: String,
        /// Mark as pre-release
        #[arg(long)]
        pre_release: bool,
        /// Extension visibility
        #[arg(long, default_value = "public")]
        visibility: VisibilityOption,
        /// Overwrite existing version
        #[arg(long)]
        overwrite: bool,
        /// Skip validation checks
        #[arg(long)]
        skip_validation: bool,
        /// Release notes
        #[arg(long)]
        notes: Option<String>,
        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
        /// Access token for authentication
        #[arg(long)]
        token: Option<String>,
        /// Timeout in seconds
        #[arg(long, default_value = "300")]
        timeout: u64,
        /// Use development defaults (overwrite, skip validation, etc.)
        #[arg(long)]
        dev: bool,
    },
    /// Remove a published extension version
    Unpublish {
        /// Extension ID
        id: String,
        /// Version to unpublish
        version: String,
        /// Target store name
        #[arg(long)]
        store: String,
        /// Reason for unpublishing
        #[arg(long)]
        reason: Option<String>,
        /// Keep tombstone record
        #[arg(long)]
        keep_record: bool,
        /// Notify users who installed this version
        #[arg(long)]
        notify_users: bool,
        /// Access token for authentication
        #[arg(long)]
        token: Option<String>,
    },
    /// Validate an extension package (dry-run)
    Validate {
        /// Path to extension package or directory
        package_path: PathBuf,
        /// Target store name (optional)
        #[arg(long)]
        store: Option<String>,
        /// Use strict validation rules
        #[arg(long)]
        strict: bool,
        /// Show detailed validation results
        #[arg(long)]
        verbose: bool,
    },
    /// Show publishing requirements for a store
    Requirements {
        /// Store name (optional, shows all if not specified)
        #[arg(long)]
        store: Option<String>,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum VisibilityOption {
    Public,
    Private,
    Unlisted,
    Organization,
}
