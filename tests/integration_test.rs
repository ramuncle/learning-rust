//! Integration tests for async download patterns.
//!
//! These tests make real HTTP requests to public test APIs.
//! Run with: cargo test -- --test-threads=1
//!
//! Note: requires an internet connection.

use futures::future::join_all;
use learning_rust::downloader;
use learning_rust::bounded;

/// Single download returns non-empty body.
#[tokio::test]
async fn test_single_download() {
    let body = downloader::download_text("https://jsonplaceholder.typicode.com/todos/1")
        .await
        .expect("single download should succeed");

    assert!(!body.is_empty(), "body should not be empty");
    assert!(body.contains("userId"), "should look like a todo JSON");
}

/// Concurrent downloads with join_all all succeed.
#[tokio::test]
async fn test_join_all_concurrent() {
    let urls = vec![
        "https://jsonplaceholder.typicode.com/todos/1",
        "https://jsonplaceholder.typicode.com/todos/2",
        "https://jsonplaceholder.typicode.com/todos/3",
    ];

    let futures: Vec<_> = urls.iter().map(|u| downloader::download_text(u)).collect();
    let results = join_all(futures).await;

    assert_eq!(results.len(), 3);
    for (i, result) in results.into_iter().enumerate() {
        let body = result.expect(&format!("request {} should succeed", i + 1));
        assert!(!body.is_empty());
    }
}

/// Byte download returns the expected number of bytes.
#[tokio::test]
async fn test_byte_download() {
    let bytes = downloader::download_bytes("https://httpbin.org/bytes/64")
        .await
        .expect("byte download should succeed");

    assert_eq!(bytes.len(), 64, "should receive exactly 64 bytes");
}

/// Bounded concurrency: 4 requests with concurrency=2 all complete.
#[tokio::test]
async fn test_bounded_all_complete() {
    let urls = vec![
        "https://jsonplaceholder.typicode.com/todos/1",
        "https://jsonplaceholder.typicode.com/todos/2",
        "https://jsonplaceholder.typicode.com/todos/3",
        "https://jsonplaceholder.typicode.com/todos/4",
    ];

    let results = bounded::download_bounded(urls.clone(), 2).await;

    assert_eq!(results.len(), 4, "should get one result per URL");
    for (_url, result) in results {
        assert!(result.is_ok(), "all requests should succeed");
    }
}

/// Error case: requesting a non-existent resource returns an error.
#[tokio::test]
async fn test_404_returns_error() {
    let result =
        downloader::download_text("https://jsonplaceholder.typicode.com/nonexistent/99999999")
            .await;

    assert!(result.is_err(), "404 response should yield an error");
}
