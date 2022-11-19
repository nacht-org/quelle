mod build;
mod lock;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use fenster_engine::Runner;
use simplelog::{Config, LevelFilter, TermLogger};
use url::Url;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Provide additional information (default only shows errors).
    #[clap(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a given wasm extension
    Run {
        /// The path to the wasm file to be ran
        path: PathBuf,

        /// Print the meta information of the source
        #[arg(short, long)]
        meta: bool,

        /// Fetch and print the novel information
        #[arg(short, long)]
        novel: Option<Url>,
    },

    /// Build the extensions
    Build {
        /// The output directory for the built extensions
        #[arg(short, long, default_value = "dist")]
        out: PathBuf,
    },

    Lock,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let level = match cli.verbose {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    TermLogger::init(
        level,
        Config::default(),
        simplelog::TerminalMode::Mixed,
        simplelog::ColorChoice::Auto,
    )
    .unwrap();

    match cli.command {
        Commands::Run { path, meta, novel } => {
            let mut runner = Runner::new(&path)?;

            if meta {
                let meta = runner.meta()?;
                println!("{meta:#?}");
            }

            if let Some(url) = novel {
                let novel = runner.fetch_novel(url.as_str())?;
                println!("{novel:#?}");
            }
        }
        Commands::Build { out } => {
            build::build(out)?;
        }
        Commands::Lock => {
            lock::lock()?;
        }
    }

    Ok(())
}