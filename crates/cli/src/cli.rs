use semver::Version;
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
    /// Add a novel to your library (fetches novel + all chapters)
    Add {
        /// Novel URL to add
        url: Url,
        /// Only fetch novel metadata, skip downloading chapters
        #[arg(long)]
        no_chapters: bool,
        /// Maximum number of chapters to fetch (useful for testing)
        #[arg(long)]
        max_chapters: Option<usize>,
    },
    /// Update novels with new chapters (default: update all novels)
    Update {
        /// Novel ID, URL, title, or 'all' for all novels
        #[arg(default_value = "all")]
        novel: String,
        /// Only check for new chapters, don't download them
        #[arg(long)]
        check_only: bool,
    },
    /// Read a chapter from your library
    Read {
        /// Novel ID, URL, or title
        novel: String,
        /// Chapter number, title, or URL (shows available chapters if not specified)
        chapter: Option<String>,
        /// Show chapter list instead of reading
        #[arg(long)]
        list: bool,
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
        /// Page number for pagination
        #[arg(long)]
        page: Option<u32>,
        /// Use complex search with interactive filter selection
        #[arg(long)]
        advanced: bool,
        /// Force simple search even when tags/categories are provided
        #[arg(long)]
        simple: bool,
    },
    /// Remove a novel and all its data from your library
    Remove {
        /// Novel ID, URL, or title
        novel: String,
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },

    /// Manage local library (list, stats, cleanup)
    Library {
        #[command(subcommand)]
        command: LibraryCommands,
    },
    /// Manage extensions (install, list, update)
    Extensions {
        #[command(subcommand)]
        command: ExtensionCommands,
    },
    /// Export novels to various formats (epub, pdf)
    Export {
        /// Novel ID, URL, title, or 'all' for all novels
        novel: String,
        /// Output format
        #[arg(long, default_value = "epub")]
        format: String,
        /// Output directory
        #[arg(long)]
        output: Option<String>,
        /// Include images in export
        #[arg(long)]
        include_images: bool,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Manage extension stores
    Store {
        #[command(subcommand)]
        command: StoreCommands,
    },
    /// Publish and manage extensions
    Publish {
        #[command(subcommand)]
        command: PublishCommands,
    },
    /// Show system status and health
    Status,
    /// Advanced fetch operations (for developers/debugging)
    Fetch {
        #[command(subcommand)]
        command: FetchCommands,
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
        /// Novel ID, URL, or title
        novel: String,
    },
    /// List chapters for a stored novel
    Chapters {
        /// Novel ID, URL, or title
        novel: String,
        /// Show only chapters with downloaded content
        #[arg(long)]
        downloaded_only: bool,
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
        #[command(subcommand)]
        store_type: AddStoreCommands,
    },
    /// Initialize a local extension store directory (creates store.json) for an already added store
    Init {
        /// Store name (must already be added and be a local store)
        name: String,
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
        version: Option<Version>,
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
        /// Force update even if no new version
        #[arg(long)]
        force: bool,
        /// Only check for updates, don't install them
        #[arg(long)]
        check_only: bool,
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
        /// Extension visibility
        #[arg(long, default_value = "public")]
        visibility: VisibilityOption,
        /// Overwrite existing version
        #[arg(long)]
        overwrite: bool,
        /// Skip validation checks
        #[arg(long)]
        skip_validation: bool,
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
}

#[derive(clap::Subcommand, Debug)]
pub enum AddStoreCommands {
    /// Add a local file system store
    Local {
        /// Store name
        name: String,
        /// Local directory path (defaults to data_dir/stores/name if not provided)
        path: Option<String>,
        /// Priority (lower = higher priority)
        #[arg(long, default_value = "100")]
        priority: u32,
    },
    /// Add a git repository store
    Git {
        /// Store name
        name: String,
        /// Git repository URL
        url: String,
        /// Priority (lower = higher priority)
        #[arg(long, default_value = "100")]
        priority: u32,
        /// Git branch to track
        #[arg(long)]
        branch: Option<String>,
        /// Git tag to track
        #[arg(long)]
        tag: Option<String>,
        /// Git commit hash to pin to
        #[arg(long)]
        commit: Option<String>,
        /// Git authentication token (GitHub/GitLab personal access token)
        #[arg(long)]
        token: Option<String>,
        /// SSH private key path for authentication
        #[arg(long)]
        ssh_key: Option<String>,
        /// SSH public key path (optional, will be inferred if not provided)
        #[arg(long)]
        ssh_pub_key: Option<String>,
        /// SSH key passphrase
        #[arg(long)]
        ssh_passphrase: Option<String>,
        /// Username for basic authentication
        #[arg(long)]
        username: Option<String>,
        /// Password for basic authentication
        #[arg(long)]
        password: Option<String>,
        /// Custom cache directory (defaults to data_dir/stores/name)
        #[arg(long)]
        cache_dir: Option<String>,
    },
    /// Add a GitHub repository store (API-based reads, git-based writes)
    #[cfg(feature = "github")]
    #[command(name = "github")]
    GitHub {
        /// Store name
        name: String,
        /// GitHub repository owner (username or organization)
        owner: String,
        /// GitHub repository name
        repo: String,
        /// Priority (lower = higher priority)
        #[arg(long, default_value = "100")]
        priority: u32,
        /// Git branch to track
        #[arg(long)]
        branch: Option<String>,
        /// Git tag to track
        #[arg(long)]
        tag: Option<String>,
        /// Git commit hash to pin to
        #[arg(long)]
        commit: Option<String>,
        /// GitHub personal access token for API and git operations
        #[arg(long)]
        token: Option<String>,
        /// SSH private key path for git authentication
        #[arg(long)]
        ssh_key: Option<String>,
        /// SSH public key path (optional, will be inferred if not provided)
        #[arg(long)]
        ssh_pub_key: Option<String>,
        /// SSH key passphrase
        #[arg(long)]
        ssh_passphrase: Option<String>,
        /// Username for basic authentication
        #[arg(long)]
        username: Option<String>,
        /// Password for basic authentication
        #[arg(long)]
        password: Option<String>,
        /// Custom cache directory (defaults to data_dir/stores/name)
        #[arg(long)]
        cache_dir: Option<String>,
    },
}
