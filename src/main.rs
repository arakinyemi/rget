use clap::Parser;

mod config;
mod multi;

use crate::multi::multi_download;
use config::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match multi_download(&cli).await {
        Ok(path) => println!("Downloaded to {}", path.display()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
