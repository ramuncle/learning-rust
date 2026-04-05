# Async Downloads in Rust

Learning async HTTP downloads with **tokio** + **reqwest**. Three programs, three levels:

| Program | What it demonstrates |
|---|---|
| `sequential` | Baseline: one download at a time |
| `concurrent` | Fan-out with `join_all` + `tokio::spawn` |
| `with_progress` | Streaming body, `indicatif` progress bars, `Semaphore` for bounded concurrency |

---

## Key Learnings

### 1. `async/await` is sugar over state machines

Rust compiles `async fn` into a state machine — no heap allocation per `await` unless you box the future. `tokio::spawn` *does* heap-allocate so it can be polled across threads. For simple fan-out you can avoid spawn entirely with `join_all`.

```rust
// No spawn needed — all futures run on same task
let results = futures::future::join_all(futures).await;

// Spawn when you want true parallel execution (different OS threads)
let handle = tokio::spawn(async move { expensive_work().await });
let result = handle.await?;  // JoinHandle<T>, ?-unwraps JoinError
```

### 2. `reqwest::Client` is cheap to clone

Internally it's an `Arc<ClientRef>`, so cloning is just bumping a ref count. Always reuse/clone one client rather than creating a new one per request — it shares connection pools.

```rust
let client = reqwest::Client::new(); // one allocation
let c = client.clone(); // Arc::clone, ~4 ns
```

### 3. Two-layer error unwrapping with `tokio::spawn`

`tokio::spawn` returns `JoinHandle<T>`. If the task panics, `.await` returns `Err(JoinError)`. If your task returns `Result`, you get `Ok(Ok(val))` / `Ok(Err(e))` / `Err(join_err)`.

```rust
match handle.await {
    Ok(Ok(val))  => { /* success */ }
    Ok(Err(e))   => { /* application error */ }
    Err(join_e)  => { /* task panicked / cancelled */ }
}
```

### 4. Streaming body with `bytes_stream()`

Default `.bytes().await` buffers the whole response. For large files, stream chunks:

```rust
use futures::StreamExt;

let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;           // reqwest::Error if network fails
    file.write_all(&chunk).await?; // tokio::io::Error if disk fails
}
```

### 5. Bounded concurrency with `Semaphore`

`futures::join_all` with 1000 URLs would open 1000 connections simultaneously. A semaphore limits this:

```rust
let sem = Arc::new(Semaphore::new(MAX_CONCURRENT));

// acquire_owned() → permit moves into the task, auto-released on drop
let permit = sem.acquire_owned().await?;
tokio::spawn(async move {
    do_work().await;
    drop(permit); // releases slot for next task
});
```

### 6. Error context with `anyhow`

`anyhow::Context` adds human-readable context to any `Result`:

```rust
client.get(url).send().await
    .with_context(|| format!("GET {url} failed"))?;
```

Stack traces show the full chain: `"GET https://... failed: connection refused"`.

### 7. `tokio::fs` vs `std::fs`

`std::fs::File` blocks the OS thread — in async code that stalls the tokio executor. Use `tokio::fs::File` which offloads to a blocking thread pool internally.

---

## Running

```bash
cargo run --bin sequential
cargo run --bin concurrent
cargo run --bin with_progress
```

Expected result: `concurrent` and `with_progress` are **3–5× faster** than `sequential` for multiple files over high-latency connections (most of the time is waiting for server ACK, not CPU work).

---

## Mental Model: When to use what

| Scenario | Tool |
|---|---|
| 2–10 small requests | `join_all` over bare futures (no spawn overhead) |
| Many requests, CPU-light | `tokio::spawn` + `join_all` on handles |
| Many requests, cap connections | `Semaphore` + `spawn` |
| Large files | `bytes_stream()` + streaming write |
| Retry logic | `tower` or manual loop with backoff |

---

## Dependencies

```toml
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["stream"] }
futures = "0.3"
anyhow = "1"
indicatif = "0.17"
```
