use clap::{Arg, Command};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;

fn main() {
    let matches = Command::new("Rget")
        .version("0.1.0")
        .author("Araoluwa Akinyemi <arakinyemi@gmail.com>")
        .about("wget clone written in Rust")
        .arg(
            Arg::new("URL")
            .required(true)
            .index(1)
            .help("url to download"),
        )
        .get_matches();
    let url = matches.get_one::<String>("URL").unwrap();
    println!("{}", url);
}

fn create_progress_bar(quiet_mode: bool, msg: &'static str, length: Option<u64>) -> ProgressBar {
    let bar = match quiet_mode {
        true => ProgressBar::hidden(),
        false => {
            match length {
                Some(len) => ProgressBar::new(len),
                None => ProgressBar::new_spinner()
            }
        }
    };

    bar.set_message(msg);
    match length.is_some(){
        true => bar
            .set_style(ProgressStyle::default_bar()
                .template("{msg} {spinner:.green} [{elapsed_precise}] [{wide_bar: .cyan}] {bytes}/{total_bytes} eta: {eta}")
                .unwrap()
                .progress_chars("=> ")
            ),
        false => bar.set_style(ProgressStyle::default_spinner()),
    };

    bar
}