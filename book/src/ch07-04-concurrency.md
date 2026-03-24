# Concurrency

Redox provides both synchronous primitives (`std::sync`) and async utilities
(`std::async`).

## Mutex

```rdx
use std::sync::Mutex;

pub fn main() / async {
    let counter = Arc::new(Mutex::new(0));    // Arc<Mutex<i32>>

    let mut handles: Vec<JoinHandle<()>> = Vec::new();
    for _ in 0..10 {
        let counter = counter.clone();
        let h = spawn(|| {
            let mut guard = counter.lock()?;
            *guard += 1;
        });
        handles.push(h);
    }

    for h in handles { h.join()?; }
    println!("Count: {}", *counter.lock()?);    // 10
}
```

## Channels

```rdx
use std::sync::{channel, Sender, Receiver};

pub fn main() / async, io {
    let (tx, rx) = channel::<String>();

    // Spawn a producer
    spawn(|| / io {
        for i in 0..5 {
            tx.send(format!("message {i}"))?;
        }
    });

    // Receive messages
    for msg in rx {
        println!("Got: {msg}");
    }
}
```

## Async / Await

```rdx
use std::async::{spawn, join, sleep};
use std::time::Duration;

pub async fn main() / async, io {
    // Spawn concurrent tasks
    let h1 = spawn(|| fetch("https://api.example.com/a"));
    let h2 = spawn(|| fetch("https://api.example.com/b"));

    // Await both
    let (a, b) = join(h1, h2).await?;
    println!("Results: {a}, {b}");
}

pub async fn fetch(url: &str) -> Result<String, Error> / net {
    let resp = Request::get(url).send().await?;
    resp.text().await
}
```

## Select (first completion)

```rdx
use std::async::{spawn, select};

pub async fn main() / async {
    let fast = spawn(|| fast_operation());
    let slow = spawn(|| slow_operation());

    // Return whichever finishes first
    let result = select(fast, slow).await;
}
```

## RwLock

```rdx
use std::sync::RwLock;

pub fn main() / async {
    let data = Arc::new(RwLock::new(vec![1, 2, 3]));

    // Multiple readers
    {
        let guard = data.read()?;
        println!("data: {guard}");
    }

    // One writer
    {
        let mut guard = data.write()?;
        guard.push(4);
    }
}
```

## Atomics

For lock-free counters and flags:

```rdx
use std::sync::{AtomicUsize, Ordering};

let counter = AtomicUsize::new(0);

// From multiple threads
counter.fetch_add(1, Ordering::SeqCst);

let val = counter.load(Ordering::SeqCst);
```
