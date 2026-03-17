# Chapter 6: Async & Concurrency Migration

Migrate from tokio/async-std to Redox's built-in async runtime, adopt the
Swarm model for structured concurrency, and convert threaded code to agents.

---

## 6.1 Async Runtime Removal

Redox has a built-in async runtime. Remove external runtime dependencies.

### Entry Point

```diff
  // Rust with tokio
- #[tokio::main]
- async fn main() -> Result<(), Box<dyn std::error::Error>> {
-     let result = fetch_data().await?;
-     println!("{}", result);
-     Ok(())
- }

  // Redox — built-in async
+ +af main() -> R[(), ^dyn Error] / net {
+     v result = fetch_data().await?
+     p"{result}"
+     Ok(())
+ }
```

### Forge.toml — Remove Runtime Dependencies

```diff
  [rust-dependencies]
- tokio = { version = "1", features = ["full"] }
- async-std = "1"

  # These are no longer needed — Redox provides:
  # - async/await natively
  # - Task spawning via Swarm
  # - Timer via std.time
  # - I/O via std.io / std.fs / std.net
```

## 6.2 Task Spawning → Swarm

The biggest paradigm shift: replace `tokio::spawn` with the Swarm model.

### Simple Spawn

```diff
  // Rust — spawn a detached task
- let handle = tokio::spawn(async {
-     expensive_work().await
- });
- let result = handle.await?;

  // Redox — create an agent and use Swarm
+ u std.agent.{Agent, Swarm}
+
+ S Worker { input: s }
+
+ I Agent ~ Worker {
+     +af execute(&!self) -> R[s, Error] / agent {
+         expensive_work(&self.input).await
+     }
+ }
+
+ v swarm = Swarm.new()
+ swarm.spawn(Worker @{ input: s.from("data") })
+ v results = swarm.join_all().await?
+ v result = &results[0]
```

### Multiple Concurrent Tasks

```diff
  // Rust — spawn multiple tasks
- let mut handles = vec![];
- for url in urls {
-     handles.push(tokio::spawn(async move {
-         reqwest::get(&url).await?.text().await
-     }));
- }
- let mut results = vec![];
- for handle in handles {
-     results.push(handle.await??);
- }

  // Redox — fan-out with Swarm
+ S Fetcher { url: s }
+
+ I Agent ~ Fetcher {
+     +af execute(&!self) -> R[s, Error] / net, agent {
+         v resp = http.get(&self.url).await?
+         resp.text().await
+     }
+ }
+
+ v swarm = Swarm.new()
+ @ url ~ urls {
+     swarm.spawn(Fetcher @{ url })
+ }
+ v results = swarm.join_all().await?
```

### Why Swarm Over Raw Spawn?

| Feature           | `tokio::spawn`        | `Swarm.spawn`                       |
| ----------------- | --------------------- | ----------------------------------- |
| Structured        | No — fire and forget  | Yes — join_all waits for all        |
| Cancellation      | Manual via JoinHandle | Automatic when Swarm drops          |
| Error handling    | Per-handle await      | Collected in join_all result        |
| Capability checks | None                  | Agent-level capability grants       |
| Monitoring        | Manual                | Built-in via Swarm.status()         |
| Backpressure      | Manual                | Configurable via Swarm.with_limit() |

## 6.3 Select / Race

```diff
  // Rust — tokio::select!
- tokio::select! {
-     result = async_task_1() => handle_1(result),
-     result = async_task_2() => handle_2(result),
-     _ = tokio::time::sleep(Duration::from_secs(5)) => {
-         println!("timeout");
-     }
- }

  // Redox — async.select
+ u std.async.select
+ u std.time.Duration
+
+ v winner = select {
+     result = async_task_1() => handle_1(result),
+     result = async_task_2() => handle_2(result),
+     _ = time.sleep(Duration.from_secs(5)) => {
+         p"timeout"
+     },
+ }
```

## 6.4 Channels

```diff
  // Rust — tokio::sync::mpsc
- use tokio::sync::mpsc;
-
- let (tx, mut rx) = mpsc::channel(32);
-
- tokio::spawn(async move {
-     tx.send("hello".to_string()).await.unwrap();
- });
-
- while let Some(msg) = rx.recv().await {
-     println!("got: {}", msg);
- }

  // Redox — std.sync.channel
+ u std.sync.{channel, Sender, Receiver}
+ u std.agent.{Agent, Swarm}
+
+ S Sender_Agent {
+     tx: Sender[s],
+ }
+
+ I Agent ~ Sender_Agent {
+     +af execute(&!self) -> R[(), Error] / agent {
+         self.tx.send(s.from("hello")).await?
+         Ok(())
+     }
+ }
+
+ v (tx, rx) = channel[s](32)
+ v swarm = Swarm.new()
+ swarm.spawn(Sender_Agent @{ tx })
+
+ @ msg ~ rx {
+     p"got: {msg}"
+ }
```

## 6.5 Mutex and Shared State

```diff
  // Rust — Arc<Mutex<T>> with tokio
- use std::sync::Arc;
- use tokio::sync::Mutex;
-
- let counter = Arc::new(Mutex::new(0u32));
- let counter_clone = counter.clone();
-
- tokio::spawn(async move {
-     let mut guard = counter_clone.lock().await;
-     *guard += 1;
- });

  // Redox — same concepts, different syntax
+ u std.sync.Mutex
+
+ v counter = @Mutex[u32].new(0)
+ v counter_ref = counter.clone()
+
+ // Using agent pattern instead of raw spawn
+ S Incrementer {
+     counter: @Mutex[u32],
+ }
+
+ I Agent ~ Incrementer {
+     +af execute(&!self) -> R[(), Error] / agent {
+         m guard = self.counter.lock().await
+         *guard += 1
+         Ok(())
+     }
+ }
```

## 6.6 Timeouts and Delays

```diff
  // Rust
- use tokio::time::{sleep, Duration, timeout};
-
- // Simple sleep
- sleep(Duration::from_secs(1)).await;
-
- // Timeout wrapper
- match timeout(Duration::from_secs(5), slow_operation()).await {
-     Ok(result) => handle(result),
-     Err(_) => println!("timed out"),
- }

  // Redox
+ u std.time.{sleep, Duration, timeout}
+
+ // Simple sleep
+ time.sleep(Duration.from_secs(1)).await
+
+ // Timeout wrapper
+ ? time.timeout(Duration.from_secs(5), slow_operation()).await {
+     Ok(result) => handle(result),
+     Err(_) => p"timed out",
+ }
```

## 6.7 Thread-Based Code → Agent Pattern

For code using `std::thread` instead of async:

```diff
  // Rust — threads
- use std::thread;
- use std::sync::{Arc, Mutex};
-
- let data = Arc::new(Mutex::new(Vec::new()));
- let mut handles = vec![];
-
- for i in 0..4 {
-     let data = data.clone();
-     handles.push(thread::spawn(move || {
-         let result = compute(i);
-         data.lock().unwrap().push(result);
-     }));
- }
-
- for handle in handles {
-     handle.join().unwrap();
- }

  // Redox — agents with Swarm
+ u std.agent.{Agent, Swarm}
+
+ S ComputeWorker {
+     index: usize,
+ }
+
+ I Agent ~ ComputeWorker {
+     +af execute(&!self) -> R[i32, Error] / agent {
+         ret compute(self.index)
+     }
+ }
+
+ v swarm = Swarm.new()
+ @ i ~ 0..4 {
+     swarm.spawn(ComputeWorker @{ index: i })
+ }
+ v results = swarm.join_all().await?
```

## 6.8 Migration Pattern Summary

| Rust Pattern                     | Redox Pattern                     |
| -------------------------------- | --------------------------------- |
| `#[tokio::main] async fn main()` | `+af main() / async`              |
| `tokio::spawn(async { })`        | `Swarm.new(); swarm.spawn(agent)` |
| `tokio::select! { }`             | `select { }`                      |
| `tokio::time::sleep(d)`          | `time.sleep(d)`                   |
| `tokio::time::timeout(d, f)`     | `time.timeout(d, f)`              |
| `tokio::sync::mpsc::channel(n)`  | `channel[T](n)`                   |
| `tokio::sync::Mutex`             | `std.sync.Mutex`                  |
| `tokio::sync::RwLock`            | `std.sync.RwLock`                 |
| `tokio::sync::broadcast`         | `std.sync.broadcast`              |
| `tokio::fs::read_to_string`      | `fs.read_to_string` + `/ io`      |
| `tokio::net::TcpListener`        | `net.TcpListener` + `/ net`       |
| `std::thread::spawn`             | Agent + `Swarm.spawn`             |
| `thread::JoinHandle`             | `Swarm.join_all()`                |
| `Arc::new(Mutex::new(data))`     | `@Mutex[T].new(data)`             |

## 6.9 Concurrency Migration Checklist

- [ ] Remove `tokio`/`async-std` from `[rust-dependencies]`
- [ ] Replace `#[tokio::main]` with `+af main() / async`
- [ ] Convert each `tokio::spawn` call to an Agent + Swarm pattern
- [ ] Replace `tokio::select!` with `select { }`
- [ ] Replace `tokio::time::*` with `std.time.*`
- [ ] Update channel imports from tokio to `std.sync`
- [ ] Add `/ async` or `/ agent` effects to all async functions
- [ ] Replace `thread::spawn` with Agent + Swarm where appropriate
- [ ] Test with `rdx test` to verify behavior
