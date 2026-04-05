//! Retry logic for flaky async downloads.
//!
//! Real-world networks fail transiently. A robust downloader should:
//! 1. Retry on transient errors (network timeout, 5xx, connection reset)
//! 2. Use exponential backoff to avoid hammering a struggling server
//! 3. Give up after a configurable max attempts
//!
//! This module demonstrates:
//! - `tokio::time::sleep` for async sleep (non-blocking — no thread is tied up)
//! - `std::time::Duration` arithmetic
//! - Custom retry predicate (caller decides what counts as retryable)

use anyhow::{anyhow, Result};
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for retry behaviour.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Total number of attempts (including the first one).
    pub max_attempts: u32,
    /// Initial backoff delay before the second attempt.
    pub base_delay: Duration,
    /// Multiplicative factor applied to the delay on each failure.
    /// 2.0 → classic exponential backoff; 1.0 → constant delay.
    pub backoff_factor: f64,
    /// Cap the delay to avoid absurdly long waits.
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(200),
            backoff_factor: 2.0,
            max_delay: Duration::from_secs(10),
        }
    }
}

/// Retry an async operation up to `config.max_attempts` times.
///
/// `operation` is a closure returning a `Future<Output = Result<T>>`.
/// Because closures that return futures require a bit of ceremony in Rust,
/// we accept an `F: Fn() -> Fut` — a factory that produces a fresh future
/// on each attempt. (Futures in Rust are consumed when awaited, so you
/// can't `.await` the same future twice.)
///
/// # Example
///
/// ```no_run
/// # use learning_rust::retry::{retry, RetryConfig};
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let result = retry(RetryConfig::default(), || async {
///     learning_rust::downloader::download_text("https://httpbin.org/get").await
/// }).await?;
/// # Ok(())
/// # }
/// ```
pub async fn retry<F, Fut, T>(config: RetryConfig, operation: F) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut delay = config.base_delay;
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(e) => {
                last_error = Some(e);

                if attempt < config.max_attempts {
                    eprintln!(
                        "[retry] attempt {}/{} failed — sleeping {:.0?} before next try",
                        attempt, config.max_attempts, delay
                    );
                    // `tokio::time::sleep` is async: the current task suspends
                    // but the thread is freed to do other work.
                    sleep(delay).await;

                    // Compute next backoff, clamped to max_delay.
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * config.backoff_factor)
                            .min(config.max_delay.as_secs_f64()),
                    );
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("retry: no attempts made")))
}

/// Convenience wrapper: download text with automatic retry.
pub async fn download_text_with_retry(url: &'static str, config: RetryConfig) -> Result<String> {
    retry(config, || async move { crate::downloader::download_text(url).await }).await
}
