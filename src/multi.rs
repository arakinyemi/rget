use anyhow::{Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{
    Client,
    header::{CONTENT_LENGTH, RANGE},
};
use std::{fs::OpenOptions, io::Read, io::Write, path::{Path, PathBuf}};
use std::sync::Arc;
use tokio::{io::AsyncWriteExt, task};

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

    let client = Client::new();

    // let resp = client.head(config.url.clone()).send().await?;
    // println!("{:?}", resp);
    // let total_size = resp
    //     .headers()
    //     .get(CONTENT_LENGTH)
    //     .and_then(|v| v.to_str().ok())
    //     .and_then(|v| v.parse::<u64>().ok());

    let total_size = 
    // if let Some(size) = total_size {
    //     size
    // }
    //  else
      {
        println!("HEAD response missing Content-Length. Trying GET...");
        let resp = client.get(config.url.clone()).send().await?;
        resp
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
    };


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
            async move { download_chunk(&url, range, i, pb).await },
        ))
    }

    for handle in handles {
        handle.await??;
    }

    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&output_path)?;

    for i in 0..num_connections{
        let chunk_name = format!("chunk_{}", i);
        let path = Path::new(&chunk_name);
        
        let mut chunk = std::fs::File::open(path)?;

        let mut buffer = Vec::new();
        chunk.read_to_end(&mut buffer)?;

        output.write_all(&buffer)?;
        std::fs::remove_file(path)?;

    }
    Ok(output_path)
}

async fn download_chunk (url: &str, range: (u64, u64), id: usize, pb: ProgressBar) -> anyhow::Result<()> {
    let client = Client::new();
    let range_header = format!("bytes={}-{}", range.0, range.1);

    println!("Downloading range: {}", range_header);

    let mut resp = client
        .get(url)
        .header(RANGE, range_header)
        .send()
        .await?
        .error_for_status()?;

    let mut file = tokio::fs::File::create(format!("chunk_{}", id)).await?;

    while let Some(chunk) = resp.chunk().await? {
        pb.inc(chunk.len() as u64);
        file.write_all(&chunk).await?;
        pb.finish_with_message("Done");
    }

    Ok(())
}
