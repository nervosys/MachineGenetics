# Concurrency

Redox provides both synchronous primitives (`std.sync`) and async utilities
(`std.async`).

## Mutex

```rdx
u std.sync.Mutex

+f main() / async {
    v counter = @.new(Mutex.new(0))    // Arc<Mutex<i32>>

    m handles = [JoinHandle[()]]~.new()
    @ _ : 0..10 {
        v counter = counter.clone()
        v h = spawn(|| {
            m guard = counter.lock()?
            *guard += 1
        })
        handles.push(h)
    }

    @ h : handles { h.join()? }
    p"Count: {*counter.lock()?}"    // 10
}
```

## Channels

```rdx
u std.sync.{channel, Sender, Receiver}

+f main() / async, io {
    v (tx, rx) = channel[s]()

    // Spawn a producer
    spawn(|| / io {
        @ i : 0..5 {
            tx.send(f"message {i}")?
        }
    })

    // Receive messages
    @ msg : rx {
        p"Got: {msg}"
    }
}
```

## Async / Await

```rdx
u std.async.{spawn, join, sleep}
u std.time.Duration

+af main() / async, io {
    // Spawn concurrent tasks
    v h1 = spawn(|| fetch("https://api.example.com/a"))
    v h2 = spawn(|| fetch("https://api.example.com/b"))

    // Await both
    v (a, b) = join(h1, h2).await?
    p"Results: {a}, {b}"
}

+af fetch(url: &s) -> R[s, Error] / net {
    v resp = Request.get(url).send().await?
    resp.text().await
}
```

## Select (first completion)

```rdx
u std.async.{spawn, select}

+af main() / async {
    v fast = spawn(|| fast_operation())
    v slow = spawn(|| slow_operation())

    // Return whichever finishes first
    v result = select(fast, slow).await
}
```

## RwLock

```rdx
u std.sync.RwLock

+f main() / async {
    v data = @.new(RwLock.new([1, 2, 3]~))

    // Multiple readers
    {
        v guard = data.read()?
        p"data: {guard}"
    }

    // One writer
    {
        m guard = data.write()?
        guard.push(4)
    }
}
```

## Atomics

For lock-free counters and flags:

```rdx
u std.sync.{AtomicUsize, Ordering}

v counter = AtomicUsize.new(0)

// From multiple threads
counter.fetch_add(1, Ordering.SeqCst)

v val = counter.load(Ordering.SeqCst)
```
