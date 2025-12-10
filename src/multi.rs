use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{
    Client,
    header::{CONTENT_LENGTH, RANGE},
};
use std::io::SeekFrom::Start;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncSeekExt, AsyncWriteExt},
    task,
};

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

    let client = Client::new();

    // Check if file exists and get existing size
    let existing_size = if config.continue_download && output_path.exists() {
        tokio::fs::metadata(&output_path).await?.len()
    } else {
        0
    };

    // Try to get content length from HEAD request
    let total_size = match client.head(config.url.clone()).send().await {
        Ok(resp) if resp.status().is_success() => resp
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok()),
        _ => {
            // If HEAD fails, try GET
            match client.get(config.url.clone()).send().await {
                Ok(resp) => resp
                    .headers()
                    .get(CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok()),
                Err(_) => None,
            }
        }
    };

    // If no content length, fall back to single download
    if total_size.is_none() {
        if !config.quiet {
            println!("Content-Length not available. Falling back to single-threaded download...");
        }
        return single_download(config, &output_path, existing_size).await;
    }

    let total_size = total_size.unwrap();

    // If file is already complete
    if existing_size >= total_size {
        if !config.quiet {
            println!("File already fully downloaded.");
        }
        return Ok(output_path);
    }

    // Check if server supports range requests
    let supports_ranges = match client
        .head(config.url.clone())
        .header(RANGE, "bytes=0-0")
        .send()
        .await
    {
        Ok(resp) => resp.status() == 206 || resp.headers().get("accept-ranges").is_some(),
        Err(_) => false,
    };

    if !supports_ranges {
        if !config.quiet {
            println!(
                "Server doesn't support range requests. Falling back to single-threaded download..."
            );
        }
        if existing_size > 0 && !config.continue_download {
            tokio::fs::remove_file(&output_path).await.ok();
        }
        return single_download(config, &output_path, existing_size).await;
    }

    if !config.quiet {
        println!("Starting multi-threaded download...");
    }

    let multi = if !config.quiet {
        Arc::new(MultiProgress::new())
    } else {
        Arc::new(MultiProgress::new())
    };

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&output_path)
        .await?;

    file.set_len(total_size).await?;

    let num_connections = 8;
    let remaining_size = total_size - existing_size;
    let chunk_size = remaining_size / num_connections as u64;

    let mut ranges = Vec::new();
    for i in 0..num_connections {
        let start = existing_size + (i * chunk_size);
        let end = if i == num_connections - 1 {
            total_size - 1
        } else {
            existing_size + (i + 1) * chunk_size - 1
        };
        if start <= end {
            ranges.push((start, end));
        }
    }

    let mut handles = Vec::new();
    for (i, range) in ranges.iter().enumerate() {
        let file = file.try_clone().await?;
        let url = config.url.to_string();
        let range = *range;
        let quiet = config.quiet;

        let pb = if !quiet {
            let pb = multi.add(ProgressBar::new(range.1 - range.0 + 1));
            pb.set_style(
                ProgressStyle::with_template(&format!(
                    "Chunk {}: [{{bar:40.cyan/blue}}] {{bytes}}/{{total_bytes}} ({{eta}})",
                    i + 1
                ))?
                .progress_chars("=>-"),
            );
            Some(pb)
        } else {
            None
        };

        handles.push(task::spawn(async move {
            download_chunk(file, &url, range, pb).await
        }));
    }

    for handle in handles {
        handle.await??;
    }

    if !config.quiet {
        multi.clear()?;
        println!("Download complete!");
    }

    Ok(output_path)
}

async fn download_chunk(
    mut file: File,
    url: &str,
    range: (u64, u64),
    pb: Option<ProgressBar>,
) -> Result<()> {
    let client = Client::new();
    let range_header = format!("bytes={}-{}", range.0, range.1);

    let mut cursor = range.0;

    let mut resp = client
        .get(url)
        .header(RANGE, range_header)
        .send()
        .await?
        .error_for_status()?;

    while let Some(chunk) = resp.chunk().await? {
        file.seek(Start(cursor)).await?;
        file.write_all(&chunk).await?;
        if let Some(ref pb) = pb {
            pb.inc(chunk.len() as u64);
        }
        cursor += chunk.len() as u64;
    }

    if let Some(pb) = pb {
        pb.finish();
    }

    Ok(())
}

async fn single_download(
    config: &Cli,
    output_path: &PathBuf,
    existing_size: u64,
) -> Result<PathBuf> {
    let client = Client::new();

    let mut request = client.get(config.url.clone());

    // If continuing and file exists, add Range header
    if existing_size > 0 && config.continue_download {
        if !config.quiet {
            println!("Resuming download from {} bytes...", existing_size);
        }
        request = request.header(RANGE, format!("bytes={}-", existing_size));
    }

    let mut response = request.send().await?.error_for_status()?;

    // Check if server supports resume
    if existing_size > 0
        && config.continue_download
        && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
    {
        if !config.quiet {
            println!("Server doesn't support resuming. Restarting download...");
        }
        tokio::fs::remove_file(output_path).await.ok();
        response = client.get(config.url.clone()).send().await?;
    }

    let total_size = response.content_length().map(|len| len + existing_size);

    let pb = if !config.quiet {
        if let Some(total) = total_size {
            let pb = ProgressBar::new(total);
            pb.set_style(
                ProgressStyle::with_template(
                    "[{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
                )?
                .progress_chars("=>-"),
            );
            pb.set_position(existing_size);
            Some(pb)
        } else {
            let pb = ProgressBar::new_spinner();
            pb.set_message("Downloading...");
            pb.set_style(ProgressStyle::with_template(
                "{spinner:.green} {bytes} downloaded",
            )?);
            Some(pb)
        }
    } else {
        None
    };

    let mut file = if existing_size > 0 && config.continue_download {
        OpenOptions::new().append(true).open(output_path).await?
    } else {
        File::create(output_path).await?
    };

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
        if let Some(ref pb) = pb {
            pb.inc(chunk.len() as u64);
        }
    }

    if let Some(pb) = pb {
        pb.finish_with_message("Download complete!");
    }

    Ok(output_path.clone())
}
