use anyhow::{Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{
    Client,
    header::{CONTENT_LENGTH, RANGE},
};
use std::path::{PathBuf};
use std::sync::Arc;
use std::io::SeekFrom::Start;
use tokio::{fs::{File, OpenOptions}, io::{AsyncSeekExt, AsyncWriteExt}, task};

use crate::config::Cli;

pub async fn multi_download(config: &Cli) -> Result<PathBuf> {
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

    let multi = Arc::new(MultiProgress::new());
    let file = OpenOptions::new().write(true).create(true).open(&output_path).await?;
    let client = Client::new();
    
    let total_size = match client.head(config.url.clone()).send().await {
        Ok(resp) if resp.status().is_success() => {
            resp.headers()
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap()
        }
        Ok(_) | Err(_) => {
            println!("HEAD request failed or missing Content-Length. Trying GET...");
            let resp = client.get(config.url.clone()).send().await?;
            resp
                .headers()
                .get(CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap()
        }
    };

    file.set_len(total_size).await?;

    let num_connections = 8;
    let mut ranges = Vec::new();
    let chunk_size = total_size / num_connections as u64;

    for i in 0..num_connections {
        let start = i * chunk_size;
        let end = if i == num_connections - 1 {
            total_size - 1
        } else {
            start + chunk_size - 1
        };
        ranges.push((start, end));
    }

    let mut handles = Vec::new();
    for (i, range) in ranges.iter().enumerate() {
        let file = file.try_clone().await?;
        let url = config.url.to_string();
        let range = *range;

        let pb = multi.add(ProgressBar::new(range.1 - range.0 + 1));
        pb.set_style(
            ProgressStyle::with_template(&format!(
                "Chunk {}: [{{bar:40.cyan/blue}}] {{bytes}}/{{total_bytes}} ({{eta}})",
                i + 1
            ))?
            .progress_chars("=>-"),
        );
        handles.push(task::spawn(
            async move { download_chunk(file ,&url, range, pb).await },

        ))
    }

    for handle in handles {
        handle.await??;
    }

    multi.clear()?;

    Ok(output_path)
}

async fn download_chunk (mut file: File, url: &str, range: (u64, u64), pb: ProgressBar) -> anyhow::Result<()> {
    let client = Client::new();
    let range_header = format!("bytes={}-{}", range.0, range.1);
    let mut cursor = range.0;
    
    println!("Downloading range: {}", range_header);

    let mut resp = client
        .get(url)
        .header(RANGE, range_header)
        .send()
        .await?
        .error_for_status()?;


    while let Some(chunk) = resp.chunk().await? {
        file.seek(Start(cursor)).await?;
        file.write(&chunk).await?;
        pb.inc(chunk.len() as u64);
        cursor += chunk.len() as u64;
    }

    Ok(())
}
