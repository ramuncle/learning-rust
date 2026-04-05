/// sequential.rs - Baseline: download files one at a time
///
/// Learning goals:
/// - Basic async/await with tokio
/// - reqwest client usage
/// - Error handling with anyhow

use anyhow::{Context, Result};
use std::time::Instant;

const URLS: &[&str] = &[
    "https://httpbin.org/bytes/10240",  // 10KB
    "https://httpbin.org/bytes/20480",  // 20KB
    "https://httpbin.org/bytes/51200",  // 50KB
    "https://httpbin.org/bytes/10240",  // 10KB
    "https://httpbin.org/bytes/20480",  // 20KB
];

/// Download a single URL and return (url, bytes_downloaded)
async fn download(client: &reqwest::Client, url: &str) -> Result<(String, usize)> {
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to GET {url}"))?;

    // Check status — treat 4xx/5xx as errors
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {status} for {url}");
    }

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("Failed to read body from {url}"))?;

    Ok((url.to_string(), bytes.len()))
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::new();
    let start = Instant::now();

    println!("=== Sequential Downloads ===\n");

    let mut total_bytes = 0usize;
    for url in URLS {
        match download(&client, url).await {
            Ok((url, size)) => {
                println!("  ✓ {url} — {size} bytes");
                total_bytes += size;
            }
            Err(e) => {
                eprintln!("  ✗ Error: {e:#}");
            }
        }
    }

    let elapsed = start.elapsed();
    println!("\nTotal: {total_bytes} bytes in {elapsed:.2?}");
    println!(
        "Throughput: {:.1} KB/s",
        total_bytes as f64 / elapsed.as_secs_f64() / 1024.0
    );

    Ok(())
}
