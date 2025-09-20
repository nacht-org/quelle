use url::Url;

#[derive(clap::Parser, Debug)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    Novel { url: Url },
    Chapter { url: Url },
    Search { query: String },
}
