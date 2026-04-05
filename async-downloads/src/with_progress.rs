/// with_progress.rs - Concurrent downloads with live progress bars + streaming body
///
/// Learning goals:
/// - Streaming response body with `bytes_stream()`
/// - indicatif progress bars updated per-chunk
/// - tokio::sync::Semaphore for bounded concurrency
/// - Saving to disk with tokio::fs

use anyhow::{Context, Result};
use futures::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

const URLS: &[(&str, &str)] = &[
    ("https://httpbin.org/bytes/102400", "file1.bin"),
    ("https://httpbin.org/bytes/204800", "file2.bin"),
    ("https://httpbin.org/bytes/512000", "file3.bin"),
    ("https://httpbin.org/bytes/102400", "file4.bin"),
];

// Limit to N concurrent downloads at a time
const MAX_CONCURRENT: usize = 3;

async fn download_with_progress(
    client: reqwest::Client,
    url: &'static str,
    filename: &'static str,
    mp: Arc<MultiProgress>,
    permit: tokio::sync::OwnedSemaphorePermit,
) -> Result<usize> {
    // Permit is held for the duration of the download, released on drop

    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} for {url}", resp.status());
    }

    let total = resp.content_length().unwrap_or(0);

    let pb = mp.add(ProgressBar::new(total));
    pb.set_style(
        ProgressStyle::with_template(
            "{msg:20} [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("=>-"),
    );
    pb.set_message(filename);

    // Write to /tmp so we don't litter the workspace
    let path = format!("/tmp/{filename}");
    let mut file = tokio::fs::File::create(&path)
        .await
        .with_context(|| format!("Create {path}"))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded = 0usize;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("Stream error for {url}"))?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("Write error for {path}"))?;
        downloaded += chunk.len();
        pb.set_position(downloaded as u64);
    }

    pb.finish_with_message(format!("✓ {filename}"));

    // Explicitly drop permit to release the semaphore slot
    drop(permit);

    Ok(downloaded)
}

#[tokio::main]
async fn main() -> Result<()> {
    let client = reqwest::Client::new();
    let mp = Arc::new(MultiProgress::new());
    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));

    let start = Instant::now();

    println!("=== Concurrent Downloads with Progress + Bounded Concurrency ===\n");
    println!("Max concurrent: {MAX_CONCURRENT}\n");

    let handles: Vec<_> = URLS
        .iter()
        .map(|&(url, filename)| {
            let c = client.clone();
            let mp = Arc::clone(&mp);
            let sem = Arc::clone(&sem);

            tokio::spawn(async move {
                // acquire_owned: permit moves into the closure
                let permit = sem.acquire_owned().await.expect("semaphore closed");
                download_with_progress(c, url, filename, mp, permit).await
            })
        })
        .collect();

    let mut total_bytes = 0usize;
    for handle in handles {
        match handle.await {
            Ok(Ok(n)) => total_bytes += n,
            Ok(Err(e)) => eprintln!("Error: {e:#}"),
            Err(e) => eprintln!("Task panic: {e}"),
        }
    }

    let elapsed = start.elapsed();
    println!(
        "\n✅ Done. {total_bytes} bytes in {elapsed:.2?} ({:.1} KB/s)",
        total_bytes as f64 / elapsed.as_secs_f64() / 1024.0
    );

    Ok(())
}
