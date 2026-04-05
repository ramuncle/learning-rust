//! Reusable async download utilities.
//!
//! This module wraps reqwest to provide a simple interface for downloading
//! content from URLs. Each function returns `anyhow::Result` so callers
//! get rich error context without manual error-type wiring.

use anyhow::{Context, Result};

/// Download the body of `url` as a String.
///
/// Uses reqwest's async client under the hood. The `.await` points are:
/// 1. Sending the HTTP request (network I/O)
/// 2. Reading the full response body (streaming I/O)
///
/// Both suspend the current task without blocking the OS thread, which is
/// what makes tokio's cooperative scheduling efficient.
pub async fn download_text(url: &str) -> Result<String> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("failed to send request to {url}"))?;

    // Check for HTTP-level errors (4xx, 5xx) before reading the body.
    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("request to {url} returned status {status}");
    }

    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read response body from {url}"))?;

    Ok(body)
}

/// Download the body of `url` as raw bytes.
///
/// Useful when you need binary content (images, archives, etc.)
/// rather than UTF-8 text.
pub async fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let response = reqwest::get(url)
        .await
        .with_context(|| format!("failed to send request to {url}"))?;

    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("request to {url} returned status {status}");
    }

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("failed to read bytes from {url}"))?;

    Ok(bytes.to_vec())
}
