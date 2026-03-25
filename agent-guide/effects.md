# MechGen Effect Annotation Guide

> Complete reference for the MechGen effect system, optimized for AI agents.
> All examples use **human syntax** (default).

---

## What Are Effects?

Effects declare the **side effects** a function may produce. Pure functions have
no annotation. Impure functions list their effects after `/`.

```MechGen
// Pure — no annotation
fn add(a: i32, b: i32) -> i32 { a + b }

// Impure — annotated
fn read() -> Result<String, Error> / io { fs::read_to_string("data.txt") }
```

## Built-In Effects

| Effect    | Meaning            | Typical Operations                                       |
| --------- | ------------------ | -------------------------------------------------------- |
| `io`      | File / console I/O | `fs::read`, `fs::write`, `println!("...")`, stdin/stdout |
| `net`     | Network access     | `http::get`, `TcpStream::connect`, DNS lookup            |
| `rng`     | Randomness         | `rng::gen()`, `rng::shuffle()`                           |
| `async`   | Async execution    | `.await`, `spawn`, `select`                              |
| `agent`   | Agent operations   | `Agent::execute`, `Swarm::spawn`, `Capability::request`  |
| `time`    | Clock access       | `Instant::now()`, `sleep`, `SystemTime`                  |
| `env`     | Environment access | `env::var()`, `env::args()`, `env::current_dir()`        |
| `process` | Process control    | `Command::new()`, `exit()`, `spawn_process()`            |

## Effect Hierarchy

Some effects imply others. You only need to declare the **most specific** effect.

```
net    ⊃  io       →  / net  (already includes io)
agent  ⊃  async    →  / agent (already includes async)
```

**Examples:**
```MechGen
// WRONG — redundant io with net
pub async fn fetch(url: &str) -> Result<String, Error> / io, net { ... }

// CORRECT — net implies io
pub async fn fetch(url: &str) -> Result<String, Error> / net { ... }

// WRONG — redundant async with agent
pub async fn run(a: &mut Agent) -> Result<(), Error> / async, agent { ... }

// CORRECT — agent implies async
pub async fn run(a: &mut Agent) -> Result<(), Error> / agent { ... }
```

## Effect Propagation

**Rule:** If function A calls function B which has effect E, then A must also
declare effect E (or a superset of E).

```MechGen
// B has / io
fn read_data() -> Result<String, Error> / io {
    fs::read_to_string("data.txt")
}

// A calls B, so A must also have / io
fn process() -> Result<(), Error> / io {
    let data = read_data()?;
    println!("Got: {data}");
    return ();
}
```

**Violation** — compiler error:
```MechGen
// WRONG — missing / io, but calls read_data() which requires / io
fn process() -> Result<(), Error> {
    let data = read_data()?;   // ERROR: effect `io` not declared
}
```

## Decision Tree: Which Effects to Annotate

```
Does the function...

├── Read/write files or console?           → / io
├── Make network requests?                 → / net
├── Generate random numbers?               → / rng
├── Use .await or spawn tasks?             → / async
├── Create/run agents or swarms?           → / agent
├── Read clock or sleep?                   → / time
├── Access env vars or CLI args?           → / env
├── Spawn/manage OS processes?             → / process
├── Call another function with effects?    → Propagate its effects
└── None of the above?                     → Pure (no annotation)
```

## Multiple Effects

Comma-separate multiple effects:

```MechGen
pub async fn download_and_save(url: &str, path: &str) -> Result<(), Error> / net, io {
    let data = http::get(url).await?;
    fs::write(path, data.bytes())?;
    return ();
}
```

But apply the hierarchy rule — don't list implied effects:

```MechGen
// net already implies io, so this is redundant:
pub async fn fetch() -> Result<String, Error> / io, net { ... }

// Just use:
pub async fn fetch() -> Result<String, Error> / net { ... }
```

## Effect Handling (Mocking)

Use `handle` blocks to intercept effects. Essential for testing:

```MechGen
#[test]
fn test_read_config() {
    handle io {
        read_to_string(_) => "key = value",
    } {
        let config = read_config("config.toml");
        assert_eq!(config.unwrap().key, "value");
    }
}
```

Multiple handlers:

```MechGen
#[test]
fn test_fetch_and_parse() {
    handle net {
        get(_) => Response::mock(200, "{}"),
    }
    handle io {
        write(_, _) => (),
    } {
        let result = fetch_and_save("http://example.com", "out.json");
        assert!(result.is_ok());
    }
}
```

## Effect Annotations on Trait Methods

```MechGen
pub trait DataSource {
    fn fetch(&self, query: &str) -> Result<String, Error> / io;
    fn count(&self) -> usize;    // pure
}

impl DataSource for FileSource {
    fn fetch(&self, query: &str) -> Result<String, Error> / io {
        fs::read_to_string(&format!("data/{query}.txt"))
    }

    fn count(&self) -> usize {
        self.entries.len()
    }
}
```

**Rule:** Implementors must declare the **same or fewer** effects as the trait method.

## Effect Annotations on Closures / Function Parameters

```MechGen
// Accept a closure that performs io
pub fn with_file<T>(path: &str, work: fn(&str) -> T / io) -> Result<T, Error> / io {
    let content = fs::read_to_string(path)?;
    return work(&content);
}

// Accept a pure closure
pub fn transform<T>(data: T, func: fn(T) -> T) -> T {
    func(data)
}
```

## Common Effect Combinations

| Scenario               | Effect Annotation                   |
| ---------------------- | ----------------------------------- |
| CLI tool reading files | `/ io, env`                         |
| HTTP API handler       | `/ net, io` (simplified to `/ net`) |
| Agent executing a task | `/ agent`                           |
| Swarm with network I/O | `/ net, agent`                      |
| Random data generator  | `/ rng`                             |
| Timed benchmark        | `/ time, io`                        |
| Process launcher       | `/ process, io`                     |
| Pure computation       | *(none)*                            |
| Async-only (no I/O)    | `/ async`                           |
| Full-stack agent       | `/ net, agent, time, env`           |

## Summary Rules

1. **Pure by default** — no annotation means no side effects
2. **Declare all effects** — every impure operation must be annotated
3. **Effects propagate** — callers inherit callee effects
4. **Hierarchy simplifies** — `net ⊃ io`, `agent ⊃ async`
5. **Handle for testing** — use `handle` blocks to mock effects
6. **Trait bounds match** — impl effects ≤ trait method effects
7. **Closures carry effects** — annotate function parameters with their effects
