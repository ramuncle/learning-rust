//! Bounded concurrency: limit simultaneous in-flight requests.
//!
//! When you have many URLs, firing them all at once can overwhelm the target
//! server or exhaust local file descriptors. A `Semaphore` acts as a
//! concurrency gate: each task acquires a permit before starting, and
//! releases it when done. Only `N` tasks run at any moment.
//!
//! Pattern:
//!   Arc<Semaphore> ──► multiple tokio::spawn tasks
//!   Each task: acquire permit → do work → permit auto-released on drop
//!   Collect JoinHandles → await all handles

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Download `urls` concurrently, but with at most `concurrency` requests
/// in flight at any given time.
///
/// Returns a Vec of (url, Result<String>) in the same order as the input.
pub async fn download_bounded(
    urls: Vec<&'static str>,
    concurrency: usize,
) -> Vec<(&'static str, Result<String>)> {
    // Arc allows multiple tasks to share ownership of the semaphore.
    let sem = Arc::new(Semaphore::new(concurrency));

    // Spawn one task per URL. tokio::spawn runs the task on the runtime's
    // thread pool; it may run on a different thread than the caller.
    let handles: Vec<_> = urls
        .into_iter()
        .map(|url| {
            let sem = Arc::clone(&sem);
            tokio::spawn(async move {
                // `.acquire_owned()` gives a permit that is released when
                // `_permit` is dropped — i.e., when the async block ends.
                // If all permits are taken, this `.await` suspends the task
                // until one becomes available.
                let _permit = sem.acquire_owned().await.expect("semaphore closed");

                let result = crate::downloader::download_text(url).await;
                (url, result)
            })
        })
        .collect();

    // Collect results in spawning order.
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(pair) => results.push(pair),
            Err(e) => {
                // JoinError: the spawned task panicked or was cancelled.
                eprintln!("task panicked: {e}");
            }
        }
    }
    results
}

/// Demonstrate the semaphore pattern with timing output.
pub async fn run_demo() -> Result<()> {
    let urls = vec![
        "https://jsonplaceholder.typicode.com/todos/1",
        "https://jsonplaceholder.typicode.com/todos/2",
        "https://jsonplaceholder.typicode.com/todos/3",
        "https://jsonplaceholder.typicode.com/todos/4",
        "https://jsonplaceholder.typicode.com/todos/5",
        "https://jsonplaceholder.typicode.com/todos/6",
    ];

    let start = std::time::Instant::now();
    // Allow at most 2 concurrent requests.
    let results = download_bounded(urls, 2).await;
    let elapsed = start.elapsed();

    for (url, result) in &results {
        match result {
            Ok(body) => println!("  ✓ {url}: {} bytes", body.len()),
            Err(e) => println!("  ✗ {url}: {e:#}"),
        }
    }
    println!("  6 requests, concurrency=2, elapsed: {elapsed:.2?}");
    println!("  (compare: without semaphore all 6 fire instantly)");
    Ok(())
}
