# learning-rust — Async HTTP Downloads

Exploring async/await in Rust with **tokio** and **reqwest**.

## What This Demonstrates

- **`async`/`.await`** — Rust's zero-cost abstraction for asynchronous I/O. An `async fn` returns a `Future` that does nothing until polled. `.await` suspends the current task, yielding control to the runtime so other work can proceed on the same thread.
- **`#[tokio::main]`** — Boots a multi-threaded tokio runtime that drives our async main function to completion.
- **Single download** — The simplest pattern: call an async function and `.await` its result.
- **`tokio::join!`** — Runs a fixed set of futures concurrently within the same task. All futures are polled in parallel; the macro returns when every one completes.
- **`futures::future::join_all`** — The dynamic version: takes a `Vec<Future>` and drives them all concurrently. Useful when the number of tasks is determined at runtime.
- **Error handling with `anyhow`** — Provides `Result<T>` with rich context and the `?` operator for ergonomic propagation. `with_context` attaches human-readable messages to low-level errors.
- **Bounded concurrency with `Arc<Semaphore>` + `tokio::spawn`** — Caps the number of simultaneous in-flight requests. Each spawned task acquires a permit before making a request; the permit is released automatically on drop. Essential for polite crawling and avoiding fd exhaustion.

## Patterns at a Glance

| Pattern | API | Best for |
|---|---|---|
| Single download | `async fn` + `.await` | One request |
| Fixed concurrency | `tokio::join!` | Known-at-compile-time set |
| Dynamic concurrency | `futures::join_all` | Runtime-determined list |
| **Bounded concurrency** | `Arc<Semaphore>` + `tokio::spawn` | **Rate-limiting large lists** |

## How async/await Works in Rust

1. **Futures are lazy** — calling an `async fn` returns a `Future` but does not start execution. Work only happens when something polls the future.
2. **The runtime polls futures** — tokio's scheduler calls `poll()` on each future. If the future can make progress, it does. If it would block (e.g., waiting for network data), it returns `Pending` and registers a waker.
3. **Wakers re-schedule** — when the I/O completes, the OS notifies the runtime via epoll/kqueue/IOCP, and the waker places the future back on the run queue.
4. **No OS threads are blocked** — unlike `std::thread::spawn`, `.await` yields the runtime thread. A single thread can drive thousands of concurrent connections.

## Project Structure

```
src/
  main.rs        — Entry point demonstrating all four async download patterns
  downloader.rs  — Reusable download_text() and download_bytes() functions
  bounded.rs     — Bounded concurrency via Arc<Semaphore> + tokio::spawn
```

## Running

```bash
cargo run
```

## Dependencies

| Crate    | Purpose                                  |
|----------|------------------------------------------|
| tokio    | Async runtime (scheduler, I/O, timers)   |
| reqwest  | Ergonomic async HTTP client              |
| futures  | Utilities like `join_all`                |
| anyhow   | Flexible error handling with context     |
