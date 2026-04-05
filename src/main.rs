//! Async HTTP downloads with tokio + reqwest.
//!
//! This program demonstrates five core async patterns in Rust:
//! 1. A single async download using `.await`
//! 2. Concurrent downloads with `tokio::join!` (fixed number of futures)
//! 3. Concurrent downloads with `futures::future::join_all` (dynamic list)
//! 4. Bounded concurrency via `tokio::spawn` + `Semaphore` (rate-limiting)
//! 5. Retry with exponential backoff via `tokio::time::sleep`

mod bounded;
mod downloader;
mod retry;

use anyhow::Result;

// `#[tokio::main]` transforms `async fn main()` into a synchronous entry
// point that boots the tokio runtime and blocks on our future. Under the
// hood it creates a multi-threaded scheduler that polls futures to completion.
#[tokio::main]
async fn main() -> Result<()> {
    // ── 1. Single download ──────────────────────────────────────────
    // The simplest case: one request, one `.await`. The current task
    // suspends while the network I/O happens, freeing the thread for
    // other work (there isn't any yet, but the mechanism is the same).
    println!("=== Single download ===");
    let body = downloader::download_text("https://httpbin.org/get").await?;
    println!("Response (first 200 chars): {}\n", &body[..body.len().min(200)]);

    // ── 2. Fixed concurrency with tokio::join! ──────────────────────
    // `tokio::join!` polls multiple futures *concurrently* on the same
    // task. All three requests fly in parallel; we wait until every one
    // finishes. This is ideal when you know the exact set at compile time.
    println!("=== Concurrent downloads with tokio::join! ===");
    let (r1, r2, r3) = tokio::join!(
        downloader::download_text("https://jsonplaceholder.typicode.com/todos/1"),
        downloader::download_text("https://jsonplaceholder.typicode.com/todos/2"),
        downloader::download_text("https://jsonplaceholder.typicode.com/todos/3"),
    );

    for (i, result) in [r1, r2, r3].into_iter().enumerate() {
        match result {
            Ok(text) => println!("Todo {}: {}", i + 1, text.trim()),
            Err(e) => eprintln!("Todo {} failed: {e:#}", i + 1),
        }
    }

    // ── 3. Dynamic concurrency with join_all ────────────────────────
    // When the number of URLs isn't known at compile time, collect them
    // into a Vec of futures and use `join_all`. Each future is spawned
    // lazily; `join_all` drives them all to completion concurrently.
    println!("\n=== Dynamic concurrent downloads with join_all ===");
    let urls = vec![
        "https://jsonplaceholder.typicode.com/posts/1",
        "https://jsonplaceholder.typicode.com/posts/2",
        "https://jsonplaceholder.typicode.com/posts/3",
        "https://jsonplaceholder.typicode.com/posts/4",
        "https://jsonplaceholder.typicode.com/posts/5",
    ];

    // Build a Vec of futures (nothing runs yet — futures are lazy in Rust).
    let futures: Vec<_> = urls.iter().map(|url| downloader::download_text(url)).collect();

    // Drive all futures concurrently and collect results.
    let results = futures::future::join_all(futures).await;

    for (url, result) in urls.iter().zip(results) {
        match result {
            Ok(text) => println!("{url}: {} bytes", text.len()),
            Err(e) => eprintln!("{url}: error — {e:#}"),
        }
    }

    // ── 4. Byte download example ────────────────────────────────────
    println!("\n=== Byte download ===");
    let bytes = downloader::download_bytes("https://httpbin.org/bytes/256").await?;
    println!("Downloaded {} raw bytes", bytes.len());

    // ── 5. Bounded concurrency with Semaphore ──────────────────────
    // Real-world downloads often need a cap: don't blast 100 requests
    // simultaneously. Arc<Semaphore> + tokio::spawn gives fine-grained
    // control. Each spawned task acquires a permit; at most N run at once.
    println!("\n=== Bounded concurrency (Semaphore, max 2 in-flight) ===");
    bounded::run_demo().await?;

    // ── 6. Retry with exponential backoff ──────────────────────────
    // Real networks are flaky. A proper downloader retries on transient
    // errors using exponential backoff so it doesn't hammer a struggling
    // server. `tokio::time::sleep` keeps the wait fully async.
    println!("\n=== Retry with exponential backoff ===");
    let cfg = retry::RetryConfig {
        max_attempts: 3,
        base_delay: std::time::Duration::from_millis(100),
        backoff_factor: 2.0,
        max_delay: std::time::Duration::from_secs(5),
    };
    match retry::download_text_with_retry("https://httpbin.org/get", cfg).await {
        Ok(body) => println!("Retry download OK — {} bytes", body.len()),
        Err(e) => eprintln!("Retry download failed: {e:#}"),
    }

    println!("\nAll downloads complete!");
    Ok(())
}
