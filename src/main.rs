// use clap::{error::ErrorKind, CommandFactory};
use clap::Parser;

mod config;
mod downloader;
use config::Cli;
use downloader::download;

fn main() {
    let cli = Cli::parse();
    // if cli.single_thread && cli.num_connections != 1 {
    //     let mut cmd = Cli::command();
    //     cmd.error(ErrorKind::ArgumentConflict, "Can't have single thread and multiple connections");
    // }
    match download(&cli) {
        Ok(path) => println!("Downloaded to {}", path.display()),
        Err(e) => eprintln!("Error: {}", e),
    }
}
