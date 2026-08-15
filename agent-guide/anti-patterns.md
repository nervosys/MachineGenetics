# MAGE Anti-Patterns

> Common mistakes AI agents make when generating MAGE code.
> Each entry shows the **wrong** code and the **correct** fix.
> All examples use **human syntax** (default). For agent mode, add `#![syntax(agent)]`.

---

## Anti-Pattern 1: Lifetime Annotations

**WRONG:**
```
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
```

**CORRECT:**
```MAGE
+f longest(a: str, b: str) -> str {
    ? len(a) > len(b) { a } : { b }
}
```

**Rule:** The SKB infers and proves lifetimes. Never write lifetime parameters.

---

## Anti-Pattern 2: Missing Effect Annotations

**WRONG:**
```MAGE
pub fn save(data: &str) -> Result<(), Error> {
    fs::write("out.txt", data)?
}
```

**CORRECT:**
```MAGE
+f save(data: str) -> R[i32, str] / fs {
    fs.write("out.txt", data)
    Ok(1)
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
```MAGE
// No `unsafe`. Reach the resource through its capability handle, and
// declare the effect that handle performs.
+f alloc_buffer(n: i32) -> i32 / alloc {
    mem.alloc(n)
    n
}
```

**Rule:** MAGE has no `unsafe`. Use the `Capability` system for privileged operations.

---

## Anti-Pattern 4: Raw Concurrency Instead of Swarm

**WRONG:**
```MAGE
use std::sync::{Arc, Mutex};
use std::thread;

let handle = thread::spawn(|| {
    expensive_work()
});
let result = handle.join()?;
```

**CORRECT:**
```MAGE
agent Worker { capabilities: [agent] }
swarm Pool { agent: Worker }

+f run_all(inputs: [str]~) -> i32 / agent {
    agent.spawn("pool")
    len(inputs) as i32
}
```

**Rule:** Prefer `Swarm` for parallel work. It provides structured concurrency with capability checks.

---

## Anti-Pattern 5: Omitting Visibility on Public APIs

**WRONG:**
```MAGE
struct Config {
    host: String,
    port: u16,
}

fn new_config() -> Config {
    Config { host: String::from("localhost"), port: 8080 }
}
```

**CORRECT:**
```MAGE
+S Config { host: str, port: i32 }

+f new_config() -> Config {
    @Config { host: "localhost", port: 8080 }
}
```

**Rule:** Use `pub` for public items. Fields are private by default — use `pub field_name` for public fields.

---

## Anti-Pattern 6: Forgetting Effect Propagation

**WRONG:**
```MAGE
fn process(url: &str) -> Result<String, Error> / net {
    let data = fetch(url)?;        // fetch is / net
    let parsed = parse(&data);     // parse is pure — OK
    save_to_disk(&parsed)?         // save_to_disk is / io — MISSING!
}
```

**CORRECT:**
```MAGE
f inner(path: str) -> i32 / fs {
    fs.open(path)
    0
}

// The caller inherits what it reaches, and must say so.
+f outer(path: str) -> i32 / fs {
    inner(path)
}
```

**Rule:** A function's effect set must be the union of all effects from its callees.

---

## Anti-Pattern 7: Not Using the Agent Trait

**WRONG** — ad-hoc async task:
```MAGE
pub async fn do_work(input: String) -> Result<String, Error> / agent {
    // logic here
}
```

**CORRECT** — structured agent:
```MAGE
agent Worker { capabilities: [agent] }

+f run(input: str) -> str / agent {
    agent.spawn(input)
    input
}
```

**Rule:** For async work units, prefer implementing `Agent` over bare async functions. Agents get lifecycle management, observability, and swarm composition.

---

## Anti-Pattern 8: Missing Contract Annotations on APIs

**WRONG:**
```MAGE
pub fn divide(a: f64, b: f64) -> f64 {
    a / b
}
```

**CORRECT:**
```MAGE
sp transfer {
    @req(1b)
    @ens(1b)
    @fx()
}

+f transfer(amount: i32) -> R[i32, str] {
    guard amount > 0 else { ret Err("amount must be positive") }
    Ok(amount)
}
```

**Rule:** Public functions should use `@req` (precondition) and `@ens` (postcondition) to document and verify contracts.

---

## Anti-Pattern 9: Ignoring Capability Checks

**WRONG:**
```MAGE
pub fn read_secret(path: &str) -> Result<String, Error> / io {
    fs::read_to_string(path)
}
```

**CORRECT:**
```MAGE
+f read_secret(path: str) -> R[str, str] / fs {
    guard len(path) > 0 else { ret Err("no path") }
    Ok(fs.read_to_string(path))
}
```

**Rule:** Sensitive operations should require capability tokens, not just effect annotations.

---

## Anti-Pattern 10: Mixing Rust Crate Paths with MAGE Stdlib

**WRONG:**
```MAGE
use tokio::fs;
use serde_json::Value;
```

**CORRECT:**
```MAGE
// There is nothing to import. The standard vocabulary (`join`, `split`,
// `len`, …) and the capability namespaces (`io`, `fs`, `net`, …) are in
// scope everywhere — `use` parses but brings nothing in.
+f main() -> i32 / io {
    println(join(["a", "b"], ","))
    0
}
```

**Rule:** Use MAGE's `std::` modules. External Rust crates may not be compatible with the effect system.

---

## Quick Self-Check

Before submitting generated MAGE code, verify:

- [ ] No lifetime annotations (`'a`, `'static`)
- [ ] No `unsafe` blocks (use `Capability` system)
- [ ] All impure functions have `/ effect` annotations
- [ ] Effect sets are the union of all callee effects
- [ ] Async work uses `Agent` trait, not bare functions
- [ ] Public APIs have `@req` / `@ens` contracts
- [ ] Sensitive ops use `Capability` tokens
- [ ] Using `std::` MAGE modules, not external Rust crates
