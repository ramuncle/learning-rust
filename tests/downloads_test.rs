//! Integration tests for async download patterns.
//!
//! These tests hit real network endpoints (httpbin.org / jsonplaceholder).
//! They are intentionally end-to-end to validate the full async chain —
//! runtime, client, error handling — not just unit logic.
//!
//! Run with: `cargo test`

use learning_rust::bounded::download_bounded;
use learning_rust::downloader::{download_bytes, download_text};
use learning_rust::retry::{download_text_with_retry, RetryConfig};
use learning_rust::streaming::stream_to_memory_with_progress;
use learning_rust::timeout::{download_many_with_timeout, download_with_timeout};
use std::time::Duration;

// ── single download ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_single_text_download_returns_non_empty_body() {
    let body = download_text("https://jsonplaceholder.typicode.com/todos/1")
        .await
        .expect("download should succeed");
    assert!(!body.is_empty(), "body should be non-empty");
    assert!(body.contains("userId"), "JSON should contain 'userId'");
}

#[tokio::test]
async fn test_download_bytes_returns_correct_length() {
    // httpbin /bytes/64 returns exactly 64 random bytes
    let bytes = download_bytes("https://httpbin.org/bytes/64")
        .await
        .expect("byte download should succeed");
    assert_eq!(bytes.len(), 64, "should get exactly 64 bytes");
}

#[tokio::test]
async fn test_download_text_fails_on_404() {
    let result = download_text("https://httpbin.org/status/404").await;
    assert!(result.is_err(), "404 should return an error");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(msg.contains("404"), "error message should mention 404 status");
}

// ── bounded concurrency ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_bounded_downloads_all_succeed() {
    let urls = vec![
        "https://jsonplaceholder.typicode.com/todos/1",
        "https://jsonplaceholder.typicode.com/todos/2",
        "https://jsonplaceholder.typicode.com/todos/3",
    ];

    let results = download_bounded(urls, 2).await;
    assert_eq!(results.len(), 3, "should get one result per URL");
    for (url, result) in &results {
        assert!(result.is_ok(), "download of {url} should succeed");
    }
}

#[tokio::test]
async fn test_bounded_concurrency_of_one_behaves_like_sequential() {
    // concurrency=1 means requests run one at a time — results should still
    // all succeed; the only difference is timing.
    let urls = vec![
        "https://jsonplaceholder.typicode.com/todos/1",
        "https://jsonplaceholder.typicode.com/todos/2",
    ];

    let results = download_bounded(urls, 1).await;
    assert_eq!(results.len(), 2);
    for (_, result) in &results {
        assert!(result.is_ok());
    }
}

// ── retry logic ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_retry_succeeds_on_good_url() {
    let cfg = RetryConfig {
        max_attempts: 3,
        base_delay: Duration::from_millis(50),
        backoff_factor: 2.0,
        max_delay: Duration::from_secs(2),
    };
    let body = download_text_with_retry("https://jsonplaceholder.typicode.com/todos/1", cfg)
        .await
        .expect("retry download should succeed");
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_retry_exhausts_attempts_on_bad_url() {
    // httpbin /status/503 always returns 503 — simulates a persistently
    // failing server. After max_attempts the function should give up.
    let cfg = RetryConfig {
        max_attempts: 2,
        base_delay: Duration::from_millis(50),
        backoff_factor: 1.0, // constant delay — faster for tests
        max_delay: Duration::from_millis(100),
    };
    let result = download_text_with_retry("https://httpbin.org/status/503", cfg).await;
    assert!(result.is_err(), "should fail after exhausting retries");
}

// ── timeout ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_timeout_succeeds_within_deadline() {
    let result = download_with_timeout(
        "https://jsonplaceholder.typicode.com/todos/1",
        Duration::from_secs(5),
    )
    .await;
    assert!(result.is_ok(), "request should complete within 5s");
    let body = result.unwrap();
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_timeout_fires_on_slow_response() {
    // httpbin /delay/3 sleeps 3 seconds. 500ms deadline should always expire.
    let result =
        download_with_timeout("https://httpbin.org/delay/3", Duration::from_millis(500)).await;
    assert!(result.is_err(), "should time out");
    let msg = format!("{:#}", result.unwrap_err());
    assert!(
        msg.contains("timed out"),
        "error should mention timeout, got: {msg}"
    );
}

#[tokio::test]
async fn test_timeout_batch_all_complete() {
    let urls = [
        "https://jsonplaceholder.typicode.com/todos/1",
        "https://jsonplaceholder.typicode.com/todos/2",
    ];
    let results = download_many_with_timeout(&urls, Duration::from_secs(5)).await;
    assert_eq!(results.len(), 2);
    for (url, result) in &results {
        assert!(result.is_ok(), "download of {url} should succeed");
    }
}

// ── streaming ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_stream_to_memory_returns_correct_byte_count() {
    // httpbin /bytes/256 returns exactly 256 random bytes.
    let body = stream_to_memory_with_progress("https://httpbin.org/bytes/256")
        .await
        .expect("streaming download should succeed");
    assert_eq!(body.len(), 256, "should receive exactly 256 bytes");
}

#[tokio::test]
async fn test_stream_to_memory_json_content() {
    let body = stream_to_memory_with_progress(
        "https://jsonplaceholder.typicode.com/posts/1",
    )
    .await
    .expect("streaming download should succeed");

    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("userId"), "response should be a JSON post");
}
