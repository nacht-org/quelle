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
}
