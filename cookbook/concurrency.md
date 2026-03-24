# Concurrency

---

### Spawn parallel tasks

**Problem**: Run several independent tasks concurrently and collect results.

**Solution**:

```mg
u std.async.{spawn, join_all}

+af compute_all(inputs: &[i32]~) -> [i64]~ / async {
    m handles = [_]~.new()
    @ &n : inputs {
        handles.push(spawn(move || async {
            heavy_compute(n)
        }))
    }
    v results = join_all(handles).await
    results.into_iter().map(|r| r.unwrap()).collect()
}

f heavy_compute(n: i32) -> i64 {
    // Simulate expensive work
    m acc: i64 = 1
    @ i : 1..=(n as i64) { acc *= i }
    acc
}
```

---

### Producer-consumer with channels

**Problem**: One thread produces work, another consumes it.

**Solution**:

```mg
u std.sync.{channel, Sender, Receiver}
u std.async.spawn

+f main() / io, async {
    v (tx, rx) = channel[s]()

    // Producer
    v producer = spawn(move || / io {
        @ i : 0..10 {
            tx.send(f"task-{i}")?
        }
        drop(tx) // signal completion
    })

    // Consumer
    v consumer = spawn(move || / io {
        @ msg : rx {
            p"Processing: {msg}"
        }
    })

    producer.join()?
    consumer.join()?
}
```

---

### Shared counter with Mutex

**Problem**: Increment a counter from multiple threads safely.

**Solution**:

```mg
u std.sync.Mutex

+f main() / async {
    v counter = @.new(Mutex.new(0u64))
    m handles = [_]~.new()

    @ _ : 0..100 {
        v c = counter.clone()
        handles.push(spawn(move || {
            m guard = c.lock().unwrap()
            *guard += 1
        }))
    }

    @ h : handles { h.join().unwrap() }
    p"Final count: {*counter.lock().unwrap()}"  // 100
}
```

---

### Rate limiter

**Problem**: Limit operations to at most N per second.

**Solution**:

```mg
u std.sync.Mutex
u std.time.{Instant, Duration}

S RateLimiter {
    max_per_sec: u32,
    window_start: Mutex[Instant],
    count: Mutex[u32],
}

I ~ RateLimiter {
    +f new(max_per_sec: u32) -> Self {
        RateLimiter @{
            max_per_sec,
            window_start: Mutex.new(Instant.now()),
            count: Mutex.new(0),
        }
    }

    +f acquire(&self) / async {
        loop {
            {
                m start = self.window_start.lock().unwrap()
                m count = self.count.lock().unwrap()

                ? start.elapsed() >= Duration.from_secs(1) {
                    *start = Instant.now()
                    *count = 0
                }

                ? *count < self.max_per_sec {
                    *count += 1
                    ret
                }
            }
            sleep(Duration.from_millis(10)).await
        }
    }
}
```

**Discussion**: This is a simple sliding-window rate limiter. For production
use, consider a token-bucket algorithm.

---

### Fan-out / fan-in

**Problem**: Distribute work across multiple workers and merge results.

**Solution**:

```mg
u std.sync.channel
u std.async.spawn

+f fan_out[T: Send, R: Send](
    items: [T]~,
    workers: usize,
    work_fn: f(T) -> R,
) -> [R]~ / async {
    v (tx, rx) = channel[R]()
    v (work_tx, work_rx) = channel[T]()

    // Spawn workers
    @ _ : 0..workers {
        v work_rx = work_rx.clone()
        v tx = tx.clone()
        spawn(move || {
            @ item : work_rx {
                v result = work_fn(item)
                tx.send(result).unwrap()
            }
        })
    }
    drop(tx)

    // Feed work
    @ item : items {
        work_tx.send(item).unwrap()
    }
    drop(work_tx)

    // Collect results
    rx.into_iter().collect()
}
```

---

### Timeout wrapper

**Problem**: Run an operation with a deadline — abort if it takes too long.

**Solution**:

```mg
u std.async.{spawn, select}
u std.time.Duration

+af with_timeout[T](
    duration: Duration,
    work: af() -> T,
) -> R[T, Error] / async {
    v work_handle = spawn(work)
    v timer_handle = spawn(|| async { sleep(duration).await })

    ? select(work_handle, timer_handle).await {
        First(result) => Ok(result?),
        Second(_) => Err(Error.new("operation timed out")),
    }
}

// Usage
+af main() / async, io {
    v result = with_timeout(Duration.from_secs(5), || async {
        long_running_operation().await
    }).await

    ? result {
        Ok(v) => p"Success: {v}",
        Err(e) => p"Timed out: {e}",
    }
}
```

---

### Parallel map

**Problem**: Apply a function to every element in a collection, in parallel.

**Solution**:

```mg
u std.async.{spawn, join_all}

+af par_map[T: Send + Clone, R: Send](
    items: &[T]~,
    map_fn: f(T) -> R,
) -> [R]~ / async {
    v handles: [_]~ = items.iter()
        .map(|item| {
            v item = item.clone()
            spawn(move || map_fn(item))
        })
        .collect()

    v results = join_all(handles).await
    results.into_iter().map(|r| r.unwrap()).collect()
}

// Usage
+af main() / async, io {
    v urls = ["https://a.com", "https://b.com", "https://c.com"]~
    v pages = par_map(&urls, |url| fetch(&url)).await
    p"Fetched {pages.len()} pages"
}
```
