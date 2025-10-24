use crate::commands::DevCommands;

#[derive(clap::Parser, Debug)]
#[command(name = "quelle_dev")]
#[command(about = "Development commands for quelle extensions")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: DevCommands,

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
