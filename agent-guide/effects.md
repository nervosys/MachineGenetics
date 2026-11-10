# MAGE Effect Annotation Guide

> Complete reference for the MAGE effect system, optimized for AI agents.
> All examples use **human syntax** (default).

---

## What Are Effects?

Effects declare the **side effects** a function may produce. Pure functions have
no annotation. Impure functions list their effects after `/`.

```MAGE
// Pure — no annotation
+f add(a: i32, b: i32) -> i32 { a + b }

// Performs an effect — annotated
+f read(path: str) -> str / fs { fs.read_to_string(path) }
```

## Built-In Effects

There are nineteen built-in effects. A function acquires one by annotation, by reaching its capability namespace, or by calling a bare builtin (`println`, `panic`). An effect marked *annotation* has no namespace and is granted only by `/ effect`.

| Effect    | Domain                       | How a function acquires it                              |
| --------- | ---------------------------- | ------------------------------------------------------- |
| `io`      | read, write, seek, close     | `println(…)`, or the `io` namespace: `io.read_line(…)`  |
| `fs`      | open, stat, mkdir, remove    | the `fs` namespace: `fs.read(…)`                        |
| `net`     | connect, listen, send        | the `net` namespace: `net.connect(…)`                   |
| `env`     | get_var, set_var             | the `env` namespace: `env.get_env(…)`                   |
| `time`    | now, sleep, timeout          | the `time` namespace: `time.now()`                      |
| `rng`     | random, seed, sample         | the `rng` namespace: `rng.random()`                     |
| `agent`   | lifecycle, message, lease    | the `agent` or `swarm` namespace                        |
| `proc`    | spawn, exec, tool invocation | the `process`, `os`, `sys` or `tools` namespace         |
| `alloc`   | alloc, dealloc, realloc      | the `mem` namespace                                     |
| `llm`     | generate, embed, analyze     | the `llm` namespace: `llm.generate(…)`                  |
| `gpu`     | dispatch, synchronize        | the `gpu` namespace                                     |
| `panic`   | panic, catch_panic           | `panic(…)`, or annotation                               |
| `async`   | spawn, join, select          | annotation                                              |
| `ffi`     | call_foreign                 | annotation                                              |
| `npu`     | dispatch, synchronize        | annotation                                              |
| `evolve`  | evaluate, select, mutate     | annotation                                              |
| `learn`   | forward, backward, step      | annotation                                              |
| `heldout` | read                         | annotation                                              |
| `promote` | promote, journal             | annotation                                              |

## No Effect Hierarchy

Effects do not imply one another. `/ net` does not cover `io`, and `/ agent` does not cover `async`. Each effect a function performs must be declared, or the checker reports E0550.

**Examples:**
```MAGE
// There is no effect hierarchy. A function performing both `io` and `net`
// declares both — `/ net` does not cover an inferred `io`, and `/ agent`
// does not cover an inferred `async`.
+f fetch(url: str) -> str / io, net {
    net.connect(url)
    println(url)
    url
}
```

## Effect Propagation

**Rule:** If function A calls function B which has effect E, then A must also
declare effect E (or a superset of E).

```MAGE
f read_data(path: str) -> str / fs {
    fs.read_to_string(path)
}

// `process` reaches `fs` through `read_data`, so it declares `fs` too —
// and `io` for the `println`.
+f process(path: str) -> i32 / fs, io {
    v data = read_data(path)
    println(data)
    0
}
```

**Violation** — this is wrong, and the compiler says so:
```MAGE
f read_data(path: str) -> str / fs {
    fs.read_to_string(path)
}

// Calls `read_data`, which performs `fs`, and declares nothing.
+f process(path: str) -> i32 {
    v data = read_data(path)
    0
}
// error: function `process` performs undeclared effects: [FS] —
//        add them to its `/ effect` annotation
```

## Decision Tree: Which Effects to Annotate

```
Does the function...

├── Print, or read the console?            → / io
├── Read or write a file?                  → / fs
├── Make network requests?                 → / net
├── Generate random numbers?               → / rng
├── Use .await or spawn tasks?             → / async
├── Create/run agents or swarms?           → / agent
├── Read the clock or sleep?                → / time
├── Access env vars or CLI args?           → / env
├── Spawn/manage OS processes?             → / proc
├── Allocate?                              → / alloc
├── Call a model?                          → / llm
├── Call another function with effects?    → declare every one it performs
└── None of the above?                     → pure (no annotation)
```

## Multiple Effects

Comma-separate multiple effects:

```MAGE
+f download_and_save(url: str, path: str) -> i32 / net, fs {
    v data = net.connect(url)
    fs.write(path, url)
    0
}
```

There is no hierarchy rule. List every effect the function performs:

```MAGE
// Declare exactly what is performed. Neither is implied by the other.
+f fetch(url: str) -> str / io, net {
    net.connect(url)
    println(url)
    url
}
```

## Effect Handling (Mocking)

Use `handle` blocks to intercept effects. Essential for testing:

```MAGE
effect Cfg {
    f read(path: str) -> str;
}

f read_config(path: str) -> str / cfg {
    Cfg.read(path)
}

// A handler substitutes the operation's value, so the test never touches a
// real file. The effect being handled is named after `with`.
@test
f test_read_config() -> str {
    handle {
        read_config("config.toml")
    } with Cfg {
        read(path) => "key = value"
    }
}
```

Multiple handlers:

```MAGE
effect Net { f get(url: str) -> str; }
effect Store { f put(path: str, data: str) -> i32; }

f fetch_and_save(url: str, path: str) -> i32 / net, store {
    v body = Net.get(url)
    Store.put(path, body)
}

// One `handle` discharges one effect, so nest them.
@test
f test_fetch_and_save() -> i32 {
    handle {
        handle {
            fetch_and_save("http://example.com", "out.json")
        } with Net {
            get(url) => "{}"
        }
    } with Store {
        put(path, data) => 1
    }
}
```

## Effect Annotations on Trait Methods

```MAGE
+T DataSource {
    f fetch(self, query: str) -> str / fs;
    f count(self) -> i32;
}

+S FileSource { root: str }

I FileSource {
    +f fetch(self, query: str) -> str / fs {
        fs.read_to_string(query)
    }

    +f count(self) -> i32 { 0 }
}
```

**Rule:** Implementors must declare the **same or fewer** effects as the trait method.

## Effect Annotations on Closures / Function Parameters

```MAGE
// A closure parameter carries no effect annotation — there is no effect
// polymorphism, so `f(str) -> T / io` does not parse. The *caller* declares
// what it performs, which here is the `fs` read.
+f with_file(path: str, work: f(str) -> i32) -> i32 / fs {
    v content = fs.read_to_string(path)
    work(content)
}

// A pure higher-order function needs no annotation at all.
+f transform(data: i32, func: f(i32) -> i32) -> i32 {
    func(data)
}
```

## Common Effect Combinations

| Scenario               | Effect Annotation                   |
| ---------------------- | ----------------------------------- |
| CLI tool reading files | `/ io, env`                         |
| HTTP API handler       | `/ net, io`                         |
| Agent executing a task | `/ agent`                           |
| Swarm with network I/O | `/ net, agent`                      |
| Random data generator  | `/ rng`                             |
| Timed benchmark        | `/ time, io`                        |
| Process launcher       | `/ proc, io`                        |
| Pure computation       | *(none)*                            |
| Async-only (no I/O)    | `/ async`                           |
| Full-stack agent       | `/ net, agent, time, env`           |

## Summary Rules

1. **Pure by default** — no annotation means no side effects
2. **Declare all effects** — every impure operation must be annotated
3. **Effects propagate** — callers inherit callee effects
4. **No hierarchy** — each effect performed must be declared; `net` does not imply `io`
5. **Handle for testing** — use `handle` blocks to mock effects
6. **Trait bounds match** — impl effects ≤ trait method effects
7. **Closures carry effects** — annotate function parameters with their effects
