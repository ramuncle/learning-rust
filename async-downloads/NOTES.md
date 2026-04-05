# Async Downloads in Rust — Notes

Learning async HTTP in Rust with `tokio` + `reqwest`. Three programs, increasing complexity.

---

## Programs

| Binary | What it teaches |
|---|---|
| `sequential` | Baseline: basic `async/await`, `reqwest::Client`, `anyhow` errors |
| `concurrent` | `tokio::spawn` + `future::join_all` for fan-out parallelism |
| `with_progress` | Streaming body, `indicatif` progress bars, `Semaphore` for bounded concurrency |

---

## Benchmark Results

Downloading 5 files (112 KB total) from httpbin.org:

| Approach | Time | Throughput |
|---|---|---|
| Sequential | 2.78s | 39.6 KB/s |
| Concurrent (join_all) | 1.88s | 58.5 KB/s |
| **Speedup** | **1.48x** | I/O-bound, so gain is limited by httpbin latency per request |

The speedup is modest here because each individual request is fast and httpbin adds ~500ms latency per call. In real-world use (large files, many hosts), concurrent wins by a much larger margin.

---

## Key Learnings

### `reqwest::Client` is cheap to clone
```rust
let c = client.clone(); // Arc internally, O(1)
```
Always clone the client into each task rather than wrapping in `Arc<Mutex<>>`.

### `tokio::spawn` vs `future::join_all`
- `future::join_all` — poll all futures on the same task, simpler, no `Send` requirement
- `tokio::spawn` — true OS-thread parallelism, futures **must** be `Send + 'static`
- For HTTP (I/O-bound), `spawn` is usually better since it doesn't block the executor

### Error handling with `JoinHandle`
```rust
// tokio::spawn returns JoinHandle<Result<T, YourError>>
// so you get two levels of Result:
match handle.await {
    Ok(Ok(data)) => { /* success */ }
    Ok(Err(app_err)) => { /* your error */ }
    Err(join_err) => { /* task panicked or was cancelled */ }
}
```

### Streaming body with `bytes_stream()`
```rust
let mut stream = resp.bytes_stream();
while let Some(chunk) = stream.next().await {
    let chunk = chunk?; // each chunk is Result<Bytes>
    file.write_all(&chunk).await?;
    pb.set_position(downloaded as u64);
}
```
Required `futures::StreamExt` in scope for `.next()`.

### `Semaphore` for bounded concurrency
```rust
let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));
// Inside task:
let permit = sem.acquire_owned().await?;
// permit dropped at end of task → slot released
```
`acquire_owned()` returns a permit that can be sent across threads (moves into the async block).

---

## Run

```bash
cargo run --bin sequential
cargo run --bin concurrent
cargo run --bin with_progress
```
