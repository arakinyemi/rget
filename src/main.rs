// use clap::{error::ErrorKind, CommandFactory};
use clap::Parser;

mod config;
// mod downloader;
mod multi;

use config::Cli;
// use crate::downloader::download;
use crate::multi::multi_download;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    // if cli.single_thread && cli.num_connections != 1 {
    //     let mut cmd = Cli::command();
    //     cmd.error(ErrorKind::ArgumentConflict, "Can't have single thread and multiple connections");
    // }
    match multi_download(&cli).await {
        Ok(path) => println!("Downloaded to {}", path.display()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
