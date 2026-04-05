//! Timeouts for async operations with `tokio::time::timeout`.
//!
//! Networks fail in two ways:
//!   1. Hard errors — connection refused, DNS failure, 4xx/5xx → `Err`
//!   2. Soft hangs — server accepts the socket but never sends data → waits forever
//!
//! `tokio::time::timeout(duration, future)` wraps any future. If the future
//! does not complete within `duration`, it is cancelled and an `Err(Elapsed)`
//! is returned. The wrapped future is dropped — no thread is left blocked.
//!
//! Key insight: in async Rust, dropping a future cancels it. `timeout` works
//! by racing two futures: your operation and a timer. Whichever finishes first
//! wins; the loser is dropped.
//!
//! Patterns demonstrated:
//! - Single request with per-request timeout
//! - Helper that converts Elapsed into a human-readable `anyhow::Error`
//! - Batch with per-request timeouts (using `join_all`)
//! - Global deadline across an entire batch with nested timeouts

use anyhow::{anyhow, Context, Result};
use std::time::Duration;
use tokio::time::timeout;

/// Download text from `url`, failing if the response takes longer than `dur`.
///
/// `tokio::time::timeout` returns `Result<T, Elapsed>`. We flatten that into
/// a single `anyhow::Result<String>` so callers don't need to unwrap twice.
pub async fn download_with_timeout(url: &str, dur: Duration) -> Result<String> {
    // `timeout(dur, future)` polls `future` for up to `dur`. If the future
    // finishes in time, we get `Ok(inner_result)`. If not, `Err(Elapsed)`.
    timeout(dur, fetch_text(url))
        .await
        // Elapsed → "timed out" message, preserving the url for context.
        .map_err(|_| anyhow!("request timed out after {:.0?}: {url}", dur))?
        // Inner reqwest/anyhow error passes through unchanged.
        .with_context(|| format!("downloading {url}"))
}

/// Internal: bare fetch without timeout logic.
async fn fetch_text(url: &str) -> Result<String> {
    let response = reqwest::get(url).await?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!("HTTP {status}"));
    }
    Ok(response.text().await?)
}

/// Apply a per-request timeout to each URL in a list, collecting results.
///
/// Uses `futures::future::join_all` so all requests fly concurrently.
/// Each request gets its own independent timer — a slow request only
/// affects itself, not its neighbours.
pub async fn download_many_with_timeout<'a>(
    urls: &'a [&'a str],
    per_request: Duration,
) -> Vec<(&'a str, Result<String>)> {
    use futures::future::join_all;

    let futures: Vec<_> = urls
        .iter()
        .map(|url| {
            let url = *url;
            async move { (url, download_with_timeout(url, per_request).await) }
        })
        .collect();

    join_all(futures).await
}

/// Demonstrate timeout behaviour with a real server and an intentionally
/// tight deadline that will expire.
pub async fn run_demo() -> Result<()> {
    // ── 1. Successful request within timeout ────────────────────────
    println!("  [timeout] Normal request (3s limit)…");
    match download_with_timeout(
        "https://jsonplaceholder.typicode.com/todos/1",
        Duration::from_secs(3),
    )
    .await
    {
        Ok(body) => println!("    ✓ got {} bytes", body.len()),
        Err(e) => println!("    ✗ {e:#}"),
    }

    // ── 2. Intentional timeout via httpbin delay endpoint ────────────
    // httpbin /delay/N sleeps N seconds before responding. With a 500 ms
    // deadline, this will always time out.
    println!("  [timeout] Slow request (500ms limit, 2s server delay)…");
    match download_with_timeout("https://httpbin.org/delay/2", Duration::from_millis(500)).await {
        Ok(body) => println!("    ✓ surprisingly got {} bytes", body.len()),
        Err(e) => println!("    ✗ expected: {e:#}"),
    }

    // ── 3. Batch with per-request timeouts ──────────────────────────
    println!("  [timeout] Batch of 3 requests (2s per-request limit)…");
    let urls = [
        "https://jsonplaceholder.typicode.com/todos/1",
        "https://jsonplaceholder.typicode.com/todos/2",
        "https://jsonplaceholder.typicode.com/todos/3",
    ];
    let results = download_many_with_timeout(&urls, Duration::from_secs(2)).await;
    for (url, result) in &results {
        match result {
            Ok(body) => println!("    ✓ {url}: {} bytes", body.len()),
            Err(e) => println!("    ✗ {url}: {e:#}"),
        };
    }

    // ── 4. Global deadline over an entire batch ──────────────────────
    // Wrap the whole `download_many_with_timeout` call in another `timeout`.
    // The inner per-request timeouts still apply, but the outer deadline is
    // an absolute cap regardless of how many requests are in the batch.
    println!("  [timeout] Batch with 5s global deadline…");
    match timeout(
        Duration::from_secs(5),
        download_many_with_timeout(&urls, Duration::from_secs(3)),
    )
    .await
    {
        Ok(results) => {
            let ok = results.iter().filter(|(_, r): &&(_, Result<String>)| r.is_ok()).count();
            println!("    ✓ batch finished — {ok}/{} succeeded", results.len());
        }
        Err(_) => println!("    ✗ entire batch exceeded global 5s deadline"),
    }

    Ok(())
}
