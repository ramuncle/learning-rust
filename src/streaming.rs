//! Streaming downloads with progress tracking.
//!
//! When you `.await` `response.text()` or `response.bytes()`, reqwest
//! buffers the entire response in memory before returning. For large files
//! this is wasteful — you can't show progress, and you might OOM.
//!
//! `response.bytes_stream()` returns a `Stream<Item = Result<Bytes>>`.
//! A `Stream` is the async analogue of an `Iterator`: it yields chunks
//! asynchronously as they arrive from the network. We consume chunks with
//! `StreamExt::next()`, which returns `None` when the stream is exhausted.
//!
//! Key concepts:
//! - `Stream` / `StreamExt::next()` — async iteration
//! - `Content-Length` header — pre-known file size for percentage progress
//! - Writing chunks incrementally — constant memory footprint regardless of size
//! - Combining with `tokio::fs::File` — fully async disk I/O
//!
//! Dependencies used:
//! - `reqwest` (bytes_stream is built-in, no extra feature flag)
//! - `futures::StreamExt` (the `next()` combinator for streams)
//! - `tokio::fs` (async file I/O)
//! - `tokio::io::AsyncWriteExt` (the `write_all` method on async files)

use anyhow::{Context, Result};
use futures::StreamExt;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Stream a URL to memory, printing progress as chunks arrive.
///
/// Returns the full body as `Vec<u8>` — in a real downloader you'd write
/// each chunk to disk instead of accumulating them, but this keeps the demo
/// self-contained.
pub async fn stream_to_memory_with_progress(url: &str) -> Result<Vec<u8>> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("GET {url}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!("HTTP {status}"));
    }

    // `Content-Length` may be absent (chunked transfer, compressed response).
    // We use it for percentage display when available.
    let total_bytes: Option<u64> = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    // `bytes_stream()` converts the response into a `Stream<Item = Result<Bytes>>`.
    // No data is buffered yet — each `.next().await` drives one TCP read.
    let mut stream = response.bytes_stream();
    let mut body: Vec<u8> = Vec::new();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        // `chunk` is `Result<Bytes, reqwest::Error>`.
        let chunk: bytes::Bytes = chunk.with_context(|| format!("reading stream from {url}"))?;

        downloaded += chunk.len() as u64;
        body.extend_from_slice(&chunk);

        // Progress line.
        match total_bytes {
            Some(total) if total > 0 => {
                let pct = downloaded * 100 / total;
                eprint!(
                    "\r    {downloaded} / {total} bytes ({pct}%)    "
                );
            }
            _ => eprint!("\r    {downloaded} bytes received…    "),
        }
    }
    eprintln!(); // newline after progress

    Ok(body)
}

/// Stream a URL directly to a file on disk.
///
/// Memory usage is bounded by the chunk size (~16 KB default for reqwest).
/// The function returns the number of bytes written.
pub async fn stream_to_file(url: &str, dest: &Path) -> Result<u64> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("GET {url}"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::anyhow!("HTTP {status}"));
    }

    let total_bytes: Option<u64> = response
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    // Open the file for async writing.
    let mut file = File::create(dest)
        .await
        .with_context(|| format!("creating {:?}", dest))?;

    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk: bytes::Bytes = chunk.with_context(|| "reading stream chunk")?;

        // `write_all` loops internally until every byte is flushed.
        file.write_all(&chunk)
            .await
            .with_context(|| format!("writing to {:?}", dest))?;

        written += chunk.len() as u64;

        match total_bytes {
            Some(total) if total > 0 => {
                let pct = written * 100 / total;
                eprint!("\r    {written} / {total} bytes ({pct}%)    ");
            }
            _ => eprint!("\r    {written} bytes written…    "),
        }
    }
    eprintln!();

    // Flush OS buffers.
    file.flush()
        .await
        .with_context(|| format!("flushing {:?}", dest))?;

    Ok(written)
}

/// Demonstrate streaming downloads.
pub async fn run_demo() -> Result<()> {
    // ── 1. Stream to memory with progress ──────────────────────────
    // Using httpbin /bytes/N which returns exactly N random bytes with a
    // Content-Length header — perfect for demonstrating percentage progress.
    println!("  [streaming] Stream 4096 bytes to memory…");
    let body = stream_to_memory_with_progress("https://httpbin.org/bytes/4096").await?;
    println!("    ✓ received {} bytes total", body.len());

    // ── 2. Stream to a temp file ────────────────────────────────────
    // A small JSON payload from JSONPlaceholder — stable public API,
    // always responds quickly.
    let dest = std::env::temp_dir().join("learning-rust-stream-demo.json");
    println!("  [streaming] Stream JSON to {:?}…", dest);
    let written =
        stream_to_file("https://jsonplaceholder.typicode.com/posts/1", &dest).await?;
    println!("    ✓ wrote {written} bytes");

    // Show first 120 chars of what we wrote.
    let contents = tokio::fs::read_to_string(&dest).await?;
    let preview = contents.chars().take(120).collect::<String>();
    println!("    preview: {preview}…");

    // Tidy up.
    tokio::fs::remove_file(&dest).await.ok();

    Ok(())
}
