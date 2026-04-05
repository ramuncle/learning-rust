//! Async HTTP downloads with tokio + reqwest.
//!
//! This program demonstrates three core async patterns in Rust:
//! 1. A single async download using `.await`
//! 2. Concurrent downloads with `tokio::join!` (fixed number of futures)
//! 3. Concurrent downloads with `futures::future::join_all` (dynamic list)

mod downloader;

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

    println!("\nAll downloads complete!");
    Ok(())
}
