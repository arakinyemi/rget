use clap::{Parser};
use std::{path::PathBuf, time::Duration};
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "rget")]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[arg(value_name="URL")]
    pub url: Url,

    #[arg(short = 'O', long, value_name="FILE")]
    pub output: Option<PathBuf>,

    #[arg(short = 'n', long = "num-connections", default_value_t = 8)]
    pub num_connections: usize,

    #[arg(short = 'T', long, default_value_t = 30)]
    pub timeout: u64,

    #[arg(short, long = "continue")]
    pub continue_download: bool,

    #[arg(short, long = "singlethread")]
    pub single_thread: bool,

    #[arg(short, long="useragent")]
    pub user_agent: Option<String>,

    #[arg(short, long)]
    pub quiet: bool,

    #[arg(short = 'H', long = "headers")]
    pub print_headers: bool

}

impl Cli {
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_secs(self.timeout)
    } 
}