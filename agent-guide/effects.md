# Redox Effect Annotation Guide

> Complete reference for the Redox effect system, optimized for AI agents.

---

## What Are Effects?

Effects declare the **side effects** a function may produce. Pure functions have
no annotation. Impure functions list their effects after `/`.

```redox
// Pure — no annotation
f add(a: i32, b: i32) -> i32 { a + b }

// Impure — annotated
f read() -> R[s, Error] / io { fs.read_to_string("data.txt") }
```

## Built-In Effects

| Effect | Meaning | Typical Operations |
|--------|---------|--------------------|
| `io` | File / console I/O | `fs.read`, `fs.write`, `p"..."`, stdin/stdout |
| `net` | Network access | `http.get`, `TcpStream.connect`, DNS lookup |
| `rng` | Randomness | `rng.gen()`, `rng.shuffle()` |
| `async` | Async execution | `.await`, `spawn`, `select` |
| `agent` | Agent operations | `Agent.execute`, `Swarm.spawn`, `Capability.request` |
| `time` | Clock access | `Instant.now()`, `sleep`, `SystemTime` |
| `env` | Environment access | `env.var()`, `env.args()`, `env.current_dir()` |
| `process` | Process control | `Command.new()`, `exit()`, `spawn_process()` |

## Effect Hierarchy

Some effects imply others. You only need to declare the **most specific** effect.

```
net    ⊃  io       →  / net  (already includes io)
agent  ⊃  async    →  / agent (already includes async)
```

**Examples:**
```redox
// WRONG — redundant io with net
+af fetch(url: &s) -> R[s, Error] / io, net { ... }

// CORRECT — net implies io
+af fetch(url: &s) -> R[s, Error] / net { ... }

// WRONG — redundant async with agent
+af run(a: &!Agent) -> R[(), Error] / async, agent { ... }

// CORRECT — agent implies async
+af run(a: &!Agent) -> R[(), Error] / agent { ... }
```

## Effect Propagation

**Rule:** If function A calls function B which has effect E, then A must also
declare effect E (or a superset of E).

```redox
// B has / io
f read_data() -> R[s, Error] / io {
    fs.read_to_string("data.txt")
}

// A calls B, so A must also have / io
f process() -> R[(), Error] / io {
    v data = read_data()?
    p"Got: {data}"
    ret ()
}
```

**Violation** — compiler error:
```redox
// WRONG — missing / io, but calls read_data() which requires / io
f process() -> R[(), Error] {
    v data = read_data()?   // ERROR: effect `io` not declared
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

```redox
+af download_and_save(url: &s, path: &s) -> R[(), Error] / net, io {
    v data = http.get(url).await?
    fs.write(path, data.bytes())?
    ret ()
}
```

But apply the hierarchy rule — don't list implied effects:

```redox
// net already implies io, so this is redundant:
+af fetch() -> R[s, Error] / io, net { ... }

// Just use:
+af fetch() -> R[s, Error] / net { ... }
```

## Effect Handling (Mocking)

Use `handle` blocks to intercept effects. Essential for testing:

```redox
@test
f test_read_config() {
    handle io {
        read_to_string(_) => "key = value",
    } {
        v config = read_config("config.toml")
        assert_eq!(config.unwrap().key, "value")
    }
}
```

Multiple handlers:

```redox
@test
f test_fetch_and_parse() {
    handle net {
        get(_) => Response.mock(200, "{}"),
    }
    handle io {
        write(_, _) => (),
    } {
        v result = fetch_and_save("http://example.com", "out.json")
        assert!(result.is_ok())
    }
}
```

## Effect Annotations on Trait Methods

```redox
+T DataSource {
    f fetch(&self, query: &s) -> R[s, Error] / io
    f count(&self) -> usize    // pure
}

I DataSource ~ FileSource {
    f fetch(&self, query: &s) -> R[s, Error] / io {
        fs.read_to_string(&f"data/{query}.txt")
    }

    f count(&self) -> usize {
        self.entries.len()
    }
}
```

**Rule:** Implementors must declare the **same or fewer** effects as the trait method.

## Effect Annotations on Closures / Function Parameters

```redox
// Accept a closure that performs io
+f with_file[T](path: &s, work: f(&s) -> T / io) -> R[T, Error] / io {
    v content = fs.read_to_string(path)?
    ret work(&content)
}

// Accept a pure closure
+f transform[T](data: T, func: f(T) -> T) -> T {
    func(data)
}
```

## Common Effect Combinations

| Scenario | Effect Annotation |
|----------|-------------------|
| CLI tool reading files | `/ io, env` |
| HTTP API handler | `/ net, io` (simplified to `/ net`) |
| Agent executing a task | `/ agent` |
| Swarm with network I/O | `/ net, agent` |
| Random data generator | `/ rng` |
| Timed benchmark | `/ time, io` |
| Process launcher | `/ process, io` |
| Pure computation | *(none)* |
| Async-only (no I/O) | `/ async` |
| Full-stack agent | `/ net, agent, time, env` |

## Summary Rules

1. **Pure by default** — no annotation means no side effects
2. **Declare all effects** — every impure operation must be annotated
3. **Effects propagate** — callers inherit callee effects
4. **Hierarchy simplifies** — `net ⊃ io`, `agent ⊃ async`
5. **Handle for testing** — use `handle` blocks to mock effects
6. **Trait bounds match** — impl effects ≤ trait method effects
7. **Closures carry effects** — annotate function parameters with their effects
