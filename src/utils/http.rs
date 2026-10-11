use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{header, Client, Response};
use std::time::Duration;

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

pub fn build_client() -> Result<Client> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(DEFAULT_USER_AGENT),
    );
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"),
    );
    headers.insert(
        header::ACCEPT_LANGUAGE,
        header::HeaderValue::from_static("en-US,en;q=0.9"),
    );

    let client = Client::builder()
        .default_headers(headers)
        .cookie_store(true)
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(15))
        .build()
        .context("Failed to build HTTP client")?;

    Ok(client)
}

pub fn create_spinner(message: &'static str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
            .template("{spinner:.green} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.set_message(message);
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

pub fn create_progress_bar(total_bytes: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total_bytes);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("#>-"),
    );
    pb.set_message(message.to_string());
    pb
}

pub async fn download_bytes_with_progress(
    client: &Client,
    url: &str,
    progress_msg: &str,
) -> Result<Vec<u8>> {
    use futures_util::StreamExt;

    let res: Response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to GET {}", url))?;

    if !res.status().is_success() {
        anyhow::bail!(
            "HTTP request to {} failed with status: {}",
            url,
            res.status()
        );
    }

    let total_size = res.content_length().unwrap_or(0);
    let pb = if total_size > 0 {
        create_progress_bar(total_size, progress_msg)
    } else {
        create_spinner("Downloading...")
    };

    let mut stream = res.bytes_stream();
    let mut data = Vec::with_capacity(total_size as usize);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("Error reading stream from {}", url))?;
        data.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }

    pb.finish_and_clear();
    Ok(data)
}
