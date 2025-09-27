use std::{fs::File, io::copy, path::PathBuf};
use reqwest::blocking::Client;
use std::error::Error;
use crate::config::Cli;

pub fn download(config: &Cli) -> Result<PathBuf, Box<dyn Error>> {
    let client = Client::builder()
        .timeout(config.timeout_duration())
        .build()?;
    
    let resp = client.get(config.url.clone()).send()?;

    if !resp.status().is_success() {
        return Err(format!("Request failed with status {}", resp.status()).into());
    }

    let output_path = if let Some(ref out) = config.output {
        PathBuf::from(out)
    } else {
        let url = &config.url;
        let filename = url
            .path_segments()
            .and_then(|segments| segments.last())
            .filter(|name| !name.is_empty())
            .unwrap_or("downloaded_file");
        PathBuf::from(filename)
    };

    let mut file = File::create(&output_path)?;
    let mut content = resp;

    copy(&mut content, &mut file)?;

    Ok(output_path)
}