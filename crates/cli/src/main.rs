mod build;
mod lock;

use std::{fs::File, io::BufReader, path::PathBuf};

use clap::{Parser, Subcommand};
use quelle_engine::Runner;
use lock::Lock;
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

        /// Fetch and print the chapter content
        #[arg(short, long)]
        content: Option<Url>,
    },

    /// Build the extensions
    Build {
        /// Build this extension only.
        #[arg(short, long)]
        extension: Option<PathBuf>,

        /// The output directory for the built extensions
        #[arg(short, long, default_value = "extensions")]
        out: PathBuf,
    },

    Lock {
        /// The directory to find wasm extensions
        #[arg(short, long, default_value = ".")]
        dir: PathBuf,
    },

    Detect {
        url: Url,

        /// The path to the lock file
        #[arg(short, long, default_value = "extension-lock.json")]
        lock: PathBuf,
    },
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
        Commands::Run {
            path,
            meta,
            novel,
            content,
        } => {
            let mut runner = Runner::new(&path)?;

            if meta {
                let meta = runner.meta()?;
                println!("{meta:#?}");
            }

            if let Some(url) = novel {
                let novel = runner.fetch_novel(url.as_str())?;
                println!("{novel:#?}");
            }

            if let Some(url) = content {
                let content = runner.fetch_chapter_content(url.as_str())?;
                println!("{content:#?}");
            }
        }
        Commands::Detect { url, lock } => {
            let file = File::open(lock)?;
            let lock: Lock = serde_json::from_reader(BufReader::new(file))?;

            let extension = lock
                .extensions
                .into_iter()
                .map(|(_, e)| e)
                .find(|e| e.base_urls.iter().any(|bu| url.as_str().starts_with(bu)));

            match extension {
                Some(extension) => println!("{extension:#?}"),
                None => println!("No source matching '{url}' found"),
            }
        }
        Commands::Build { extension, out } => {
            build::build(extension, out)?;
        }
        Commands::Lock { dir } => {
            lock::lock(dir)?;
        }
    }

    Ok(())
}