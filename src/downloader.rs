use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{blocking::Client, header::USER_AGENT};
use std::fs;
use std::io::{self, Write};
use std::{fs::File, fs::OpenOptions, path::PathBuf, time::Duration};
// use std::error::Error;
use crate::config::Cli;
// use std::time::Instant;

pub struct ProgressWriter<W: Write> {
    inner: W,
    pb: ProgressBar,
}

impl<W: Write> ProgressWriter<W> {
    pub fn new(inner: W, pb: ProgressBar) -> Self {
        Self { inner, pb }
    }
}

impl<W: Write> Write for ProgressWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.pb.inc(n as u64);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
pub fn download(config: &Cli) -> Result<PathBuf> {
    let mut existing_size = 0;
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


    let client = Client::builder()
        .timeout(config.timeout_duration())
        .build()
        .context("Failed to build HTTP Client")?;

    let mut req = client.get(config.url.clone());
    if let Some(ref ua) = config.user_agent {
        req = req.header(USER_AGENT, ua.as_str());
    }

    if fs::exists(&output_path).unwrap() {
        existing_size = fs::metadata(&output_path)
            .context("Couldn't read the files content")?
            .len();
        req = req.header("Range", format!("bytes={}-", existing_size));
    }

    let mut resp = req
        .send()
        .with_context(|| format!("Failed to send GET request to {}", &config.url))?;

    if !resp.status().is_success() {
        anyhow::bail!("Request failed: HTTP {}", &resp.status());
    }
    if existing_size > 0 && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        println!("Server doesn't support resuming. Restarting download...");
        existing_size = 0;
        std::fs::remove_file(&output_path).ok();
    } else {
        println!("Download resumed...");
    }

    
    let total_size = resp.content_length().map(|len| len + existing_size);

    
    let pb = match total_size {
        Some(total) => {
            let pb = ProgressBar::new(total);
            pb.set_style(
                ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({decimal_bytes_per_sec}, {eta})")
                    .context("Progress Bar style shouldn't fail")?
                    .progress_chars("=>-"),
            );
            pb.set_position(existing_size);
            pb
        }
        None => {
            let pb = ProgressBar::new_spinner();
            pb.set_message("Downloading");
            pb.enable_steady_tick(Duration::from_millis(100));
            pb.set_style(
                ProgressStyle::with_template("{spinner:.green} {bytes} bytes")
                    .context("Spinner style shouldn't fail")?,
            );
            pb
        }
    };

    let file = if existing_size > 0 {
        OpenOptions::new().append(true).open(&output_path)?
    } else {
        File::create(&output_path)
            .with_context(|| format!("Failed to create file at {}", output_path.display()))?
    };

    let mut writer = ProgressWriter::new(file, pb.clone());

    let bytes_copied = resp
        .copy_to(&mut writer)
        .context("Failed while streaming response to file")?;

    pb.finish_with_message(format!("Downloaded {} bytes", bytes_copied));

    Ok(output_path)
}
