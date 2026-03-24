# Rust → MechGen Migration Patterns

> Side-by-side translation rules for AI agents converting Rust code to MechGen.
> All examples use **standard syntax** (default). Most Rust syntax carries over directly.

---

## What Changes?

MechGen standard syntax is intentionally close to Rust. Migration is primarily about:

1. **Adding effect annotations** (`/ io`, `/ net`, etc.) to impure functions
2. **Removing lifetime annotations** (SKB infers them)
3. **Removing `unsafe`** (use `Capability` system instead)
4. **Adding contracts** (`@req`, `@ens`) to public APIs
5. **Using MechGen stdlib modules** (`std::agent`, `std::skb`, `std::effect`)

## What Stays the Same?

| Feature           | Rust                   | MechGen Standard         |
| ----------------- | ---------------------- | ---------------------- |
| Functions         | `fn name()`            | `fn name()`            |
| Public functions  | `pub fn name()`        | `pub fn name()`        |
| Variables         | `let x = expr`         | `let x = expr`         |
| Mutable variables | `let mut x = expr`     | `let mut x = expr`     |
| Structs           | `struct Foo { }`       | `struct Foo { }`       |
| Enums             | `enum Bar { }`         | `enum Bar { }`         |
| Traits            | `trait Tr { }`         | `trait Tr { }`         |
| Impl blocks       | `impl Trait for Type`  | `impl Trait for Type`  |
| Modules           | `mod name`             | `mod name`             |
| Imports           | `use std::io::File`    | `use std::io::File`    |
| If/else           | `if cond { } else { }` | `if cond { } else { }` |
| Match             | `match expr { }`       | `match expr { }`       |
| For loops         | `for x in iter { }`    | `for x in iter { }`    |
| Return            | `return expr`          | `return expr`          |
| Generics          | `fn foo<T>(x: T)`      | `fn foo<T>(x: T)`      |
| Where clauses     | `where T: Clone`       | `where T: Clone`       |
| Paths             | `std::io::File`        | `std::io::File`        |
| Derive            | `#[derive(Debug)]`     | `#[derive(Debug)]`     |
| Print macros      | `println!("hi {x}")`   | `println!("hi {x}")`   |
| Format macros     | `format!("hi {x}")`    | `format!("hi {x}")`    |
| Struct literals   | `Foo { x: 1 }`         | `Foo { x: 1 }`         |
| Bool literals     | `true` / `false`       | `true` / `false`       |
| String types      | `String` / `&str`      | `String` / `&str`      |
| Collections       | `Vec<T>`, `HashMap`    | `Vec<T>`, `HashMap`    |
| Option/Result     | `Option<T>`, `Result`  | `Option<T>`, `Result`  |
| Closures          | `\|x\| x + 1`          | `\|x\| x + 1`          |
| Async/await       | `async fn` / `.await`  | `async fn` / `.await`  |

## What's New in MechGen?

| Feature             | Syntax                                          |
| ------------------- | ----------------------------------------------- |
| Effect annotations  | `fn read() -> Result<String, Error> / io`       |
| Effect handling     | `handle io { read(_) => "mock" } { ... }`       |
| Preconditions       | `@req x > 0`                                    |
| Postconditions      | `@ens result > 0`                               |
| Invariants          | `@inv self.len() <= self.capacity()`            |
| Performance budgets | `@perf latency < 10ms`                          |
| Capability system   | `Capability::request("fs.read", path)?`         |
| Agent trait         | `impl Agent for MyAgent { async fn execute() }` |
| Swarm               | `Swarm::new()`, `swarm.spawn(agent)`            |
| Knowledge base      | `std::skb::{Rule, Query, Proof}`                |
| Compact mode pragma | `#![syntax(compact)]` (opt-in sigil syntax)     |

---

## Worked Migration: Simple Function

### Rust
```rust
pub fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}
```

### MechGen
```MechGen
pub fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}
```

**Steps applied:**
1. Pure function — no changes needed! Rust syntax is valid MechGen standard syntax.

---

## Worked Migration: Struct with Methods

### Rust
```rust
use std::fmt;

#[derive(Debug, Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}
```

### MechGen
```MechGen
use std::fmt;

#[derive(Debug, Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}
```

**Steps applied:**
1. Pure struct with no I/O — no changes needed. Identical Rust and MechGen.

---

## Worked Migration: Error Handling with I/O

### Rust
```rust
use std::fs;
use std::io;

pub fn read_config(path: &str) -> Result<String, io::Error> {
    let content = fs::read_to_string(path)?;
    if content.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty config"));
    }
    Ok(content)
}
```

### MechGen
```MechGen
use std::fs;
use std::io;

pub fn read_config(path: &str) -> Result<String, io::Error> / io {
    let content = fs::read_to_string(path)?;
    if content.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "empty config"));
    }
    Ok(content)
}
```

**Steps applied:**
1. **Added `/ io`** — this function does file I/O. This is the only change.

---

## Worked Migration: Async with Generics

### Rust
```rust
use std::collections::HashMap;

pub async fn fetch_all<T>(urls: Vec<String>) -> Result<HashMap<String, T>, Error>
where
    T: serde::de::DeserializeOwned,
{
    let mut results = HashMap::new();
    for url in urls {
        let resp = reqwest::get(&url).await?;
        let data: T = resp.json().await?;
        results.insert(url, data);
    }
    Ok(results)
}
```

### MechGen
```MechGen
use std::collections::HashMap;

pub async fn fetch_all<T>(urls: Vec<String>) -> Result<HashMap<String, T>, Error> / net
where
    T: serde::de::DeserializeOwned,
{
    let mut results = HashMap::new();
    for url in urls {
        let resp = http::get(&url).await?;
        let data: T = resp.json().await?;
        results.insert(url, data);
    }
    Ok(results)
}
```

**Steps applied:**
1. **Added `/ net`** — network requests (net implies io)
2. `reqwest::get` → `http::get` — use MechGen stdlib

---

## Worked Migration: Trait with Persistence

### Rust
```rust
pub trait Repository<T> {
    fn find(&self, id: u64) -> Option<T>;
    fn save(&mut self, item: T) -> Result<(), Error>;
    fn delete(&mut self, id: u64) -> Result<(), Error>;
}
```

### MechGen
```MechGen
pub trait Repository<T> {
    fn find(&self, id: u64) -> Option<T>;
    fn save(&mut self, item: T) -> Result<(), Error> / io;
    fn delete(&mut self, id: u64) -> Result<(), Error> / io;
}
```

**Steps applied:**
1. **Added `/ io`** — save/delete are persistence operations
2. Everything else is identical

---

## Worked Migration: Adding Contracts

### Rust
```rust
pub fn divide(a: f64, b: f64) -> f64 {
    a / b
}
```

### MechGen
```MechGen
@req b != 0.0
@ens result == a / b
pub fn divide(a: f64, b: f64) -> f64 {
    a / b
}
```

**Steps applied:**
1. Added `@req` precondition (b must not be zero)
2. Added `@ens` postcondition (result correctness)

---

## Worked Migration: Replacing unsafe with Capabilities

### Rust
```rust
pub fn read_raw_memory(ptr: *const u8, len: usize) -> Vec<u8> {
    unsafe {
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}
```

### MechGen
```MechGen
pub fn read_raw_memory(ptr: *const u8, len: usize, cap: &Capability) -> Result<Vec<u8>, Error> {
    cap.check("mem.read", (ptr, len))?;
    std::mem::safe_read(ptr, len)
}
```

**Steps applied:**
1. Removed `unsafe` block
2. Added `Capability` parameter and check
3. Used safe stdlib abstraction

---

## Worked Migration: Threading → Agent/Swarm

### Rust
```rust
use std::thread;

pub fn parallel_process(items: Vec<String>) -> Vec<Result<String, Error>> {
    let handles: Vec<_> = items.into_iter().map(|item| {
        thread::spawn(move || process_item(item))
    }).collect();

    handles.into_iter().map(|h| h.join().unwrap()).collect()
}
```

### MechGen
```MechGen
use std::agent::{Agent, Swarm};

pub struct ItemProcessor {
    item: String,
}

impl Agent for ItemProcessor {
    pub async fn execute(&mut self) -> Result<String, Error> / agent {
        process_item(&self.item)
    }
}

pub async fn parallel_process(items: Vec<String>) -> Vec<Result<String, Error>> / agent {
    let mut swarm = Swarm::new();
    for item in items {
        swarm.spawn(ItemProcessor { item });
    }
    swarm.join_all().await
}
```

**Steps applied:**
1. Replaced `thread::spawn` with `Agent` trait + `Swarm`
2. Added `/ agent` effect annotation
3. Structured concurrency with lifecycle management

---

## Migration Checklist

For each Rust file being migrated:

1. [ ] Change file extension from `.rs` to `.mg`
2. [ ] Add effect annotations (`/ io`, `/ net`, etc.) to all impure functions
3. [ ] Remove lifetime annotations (SKB infers them)
4. [ ] Remove `unsafe` blocks → use `Capability` system
5. [ ] Add `@req` / `@ens` contracts to public APIs
6. [ ] Replace raw threading with `Agent` / `Swarm` where appropriate
7. [ ] Use MechGen stdlib modules (`std::agent`, `std::skb`, `std::effect`)
8. [ ] Optionally add `#![syntax(compact)]` for token-optimized files
