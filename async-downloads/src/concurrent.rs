/// concurrent.rs - Download files concurrently with futures::join_all
///
/// Learning goals:
/// - futures::join_all for fan-out
/// - tokio::spawn for true parallelism
/// - Collecting results from concurrent futures
/// - Error handling per-task without aborting others

use anyhow::{Context, Result};
use futures::future;
use std::time::Instant;

const URLS: &[&str] = &[
    "https://httpbin.org/bytes/10240",
    "https://httpbin.org/bytes/20480",
    "https://httpbin.org/bytes/51200",
    "https://httpbin.org/bytes/10240",
    "https://httpbin.org/bytes/20480",
];

/// Download one URL, returning Ok((url, size)) or Err with context.
async fn download(client: reqwest::Client, url: &'static str) -> Result<(&'static str, usize)> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url} failed"))?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} for {url}", resp.status());
    }

    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("Reading body from {url} failed"))?;

    Ok((url, bytes.len()))
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::new();
    let start = Instant::now();

    println!("=== Concurrent Downloads (join_all) ===\n");

    // Fan out: spawn all downloads simultaneously
    // Each future gets a clone of the Arc-backed client
    let handles: Vec<_> = URLS
        .iter()
        .map(|&url| {
            let c = client.clone(); // reqwest::Client is cheap to clone (Arc internally)
            tokio::spawn(async move { download(c, url).await })
        })
        .collect();

    // Wait for all; tokio::spawn returns JoinHandle<Result<...>>
    let results = future::join_all(handles).await;

    let mut total_bytes = 0usize;
    let mut errors = 0usize;

    for result in results {
        match result {
            Ok(Ok((url, size))) => {
                println!("  ✓ {url} — {size} bytes");
                total_bytes += size;
            }
            Ok(Err(e)) => {
                eprintln!("  ✗ Download error: {e:#}");
                errors += 1;
            }
            Err(e) => {
                // JoinError — task panicked or was cancelled
                eprintln!("  ✗ Task panicked: {e}");
                errors += 1;
            }
        }
    }

    let elapsed = start.elapsed();
    println!("\nTotal: {total_bytes} bytes in {elapsed:.2?} ({errors} errors)");
    println!(
        "Throughput: {:.1} KB/s",
        total_bytes as f64 / elapsed.as_secs_f64() / 1024.0
    );

    Ok(())
}
