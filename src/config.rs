use clap::Parser;
use std::path::PathBuf;
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "rget")]
#[command(version, long_about = None)]
pub struct Cli {
    #[arg(value_name = "URL")]
    pub url: Url,

    #[arg(short = 'O', long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    #[arg(short = 'c', long, help = "Continue partially downloaded file")]
    pub continue_download: bool,

    #[arg(short = 'q', long, help = "Quiet mode - no progress output")]
    pub quiet: bool,
}
