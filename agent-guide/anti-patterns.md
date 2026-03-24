# MechGen Anti-Patterns

> Common mistakes AI agents make when generating MechGen code.
> Each entry shows the **wrong** code and the **correct** fix.
> All examples use **standard syntax** (default). For compact mode, add `#![syntax(compact)]`.

---

## Anti-Pattern 1: Lifetime Annotations

**WRONG:**
```
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
```

**CORRECT:**
```MechGen
fn longest(a: &str, b: &str) -> &str {
```

**Rule:** The SKB infers and proves lifetimes. Never write lifetime parameters.

---

## Anti-Pattern 2: Missing Effect Annotations

**WRONG:**
```MechGen
pub fn save(data: &str) -> Result<(), Error> {
    fs::write("out.txt", data)?
}
```

**CORRECT:**
```MechGen
pub fn save(data: &str) -> Result<(), Error> / io {
    fs::write("out.txt", data)?
}
```

**Rule:** Any function performing I/O, network, or other side effects MUST declare them with `/ effect`.

---

## Anti-Pattern 3: Using `unsafe` Blocks

**WRONG:**
```
unsafe {
    let ptr = alloc(layout);
    // ...
}
```

**CORRECT:**
```MechGen
let cap = Capability::request("mem.alloc", layout)?;
// Use capability-gated safe abstractions
```

**Rule:** MechGen has no `unsafe`. Use the `Capability` system for privileged operations.

---

## Anti-Pattern 4: Raw Concurrency Instead of Swarm

**WRONG:**
```MechGen
use std::sync::{Arc, Mutex};
use std::thread;

let handle = thread::spawn(|| {
    expensive_work()
});
let result = handle.join()?;
```

**CORRECT:**
```MechGen
use std::agent::{Agent, Swarm};

pub struct Worker { input: String }

impl Agent for Worker {
    pub async fn execute(&mut self) -> Result<String, Error> / agent {
        expensive_work(&self.input)
    }
}

let swarm = Swarm::new();
swarm.spawn(Worker { input: String::from("data") });
let results = swarm.join_all().await?;
```

**Rule:** Prefer `Swarm` for parallel work. It provides structured concurrency with capability checks.

---

## Anti-Pattern 5: Omitting Visibility on Public APIs

**WRONG:**
```MechGen
struct Config {
    host: String,
    port: u16,
}

fn new_config() -> Config {
    Config { host: String::from("localhost"), port: 8080 }
}
```

**CORRECT:**
```MechGen
pub struct Config {
    pub host: String,
    pub port: u16,
}

pub fn new_config() -> Config {
    Config { host: String::from("localhost"), port: 8080 }
}
```

**Rule:** Use `pub` for public items. Fields are private by default — use `pub field_name` for public fields.

---

## Anti-Pattern 6: Forgetting Effect Propagation

**WRONG:**
```MechGen
fn process(url: &str) -> Result<String, Error> / net {
    let data = fetch(url)?;        // fetch is / net
    let parsed = parse(&data);     // parse is pure — OK
    save_to_disk(&parsed)?         // save_to_disk is / io — MISSING!
}
```

**CORRECT:**
```MechGen
fn process(url: &str) -> Result<String, Error> / io, net {
    let data = fetch(url)?;        // fetch is / net
    let parsed = parse(&data);     // parse is pure — OK
    save_to_disk(&parsed)?         // save_to_disk is / io
}
```

**Rule:** A function's effect set must be the union of all effects from its callees.

---

## Anti-Pattern 7: Not Using the Agent Trait

**WRONG** — ad-hoc async task:
```MechGen
pub async fn do_work(input: String) -> Result<String, Error> / agent {
    // logic here
}
```

**CORRECT** — structured agent:
```MechGen
pub struct Worker {
    input: String,
}

impl Agent for Worker {
    pub async fn execute(&mut self) -> Result<String, Error> / agent {
        // logic here
    }
}
```

**Rule:** For async work units, prefer implementing `Agent` over bare async functions. Agents get lifecycle management, observability, and swarm composition.

---

## Anti-Pattern 8: Missing Contract Annotations on APIs

**WRONG:**
```MechGen
pub fn divide(a: f64, b: f64) -> f64 {
    a / b
}
```

**CORRECT:**
```MechGen
@req b != 0.0
@ens result == a / b
pub fn divide(a: f64, b: f64) -> f64 {
    a / b
}
```

**Rule:** Public functions should use `@req` (precondition) and `@ens` (postcondition) to document and verify contracts.

---

## Anti-Pattern 9: Ignoring Capability Checks

**WRONG:**
```MechGen
pub fn read_secret(path: &str) -> Result<String, Error> / io {
    fs::read_to_string(path)
}
```

**CORRECT:**
```MechGen
pub fn read_secret(path: &str, cap: &Capability) -> Result<String, Error> / io {
    cap.check("fs.read", path)?;
    fs::read_to_string(path)
}
```

**Rule:** Sensitive operations should require capability tokens, not just effect annotations.

---

## Anti-Pattern 10: Mixing Rust Crate Paths with MechGen Stdlib

**WRONG:**
```MechGen
use tokio::fs;
use serde_json::Value;
```

**CORRECT:**
```MechGen
use std::fs;
use std::json::Value;
```

**Rule:** Use MechGen's `std::` modules. External Rust crates may not be compatible with the effect system.

---

## Quick Self-Check

Before submitting generated MechGen code, verify:

- [ ] No lifetime annotations (`'a`, `'static`)
- [ ] No `unsafe` blocks (use `Capability` system)
- [ ] All impure functions have `/ effect` annotations
- [ ] Effect sets are the union of all callee effects
- [ ] Async work uses `Agent` trait, not bare functions
- [ ] Public APIs have `@req` / `@ens` contracts
- [ ] Sensitive ops use `Capability` tokens
- [ ] Using `std::` MechGen modules, not external Rust crates
