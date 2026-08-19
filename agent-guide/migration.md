# Rust to MAGE Migration Patterns

> Side-by-side translation for agents converting Rust to MAGE. Human syntax
> throughout. Every MAGE block below was verified with `mage-parse --check`.

**Read this first: most Rust syntax does *not* carry over.** The previous
version of this document opened with "Migration is primarily about adding
effect annotations", printed a 25-row table asserting that `let`, `mod`,
`use path::to::Item`, `#[derive(Debug)]`, `println!`, `Foo { x: 1 }`,
`async fn` and `where` clauses were identical in both languages, and answered
six of its eight worked migrations with "no changes needed — identical Rust
and MAGE". Every row was re-checked against the compiler for this rewrite.

## What does not carry over

| Rust | MAGE | Why |
|---|---|---|
| `let x` / `let mut x` | `val x` / `var x` | The parser rejects `let` **by name**, with that advice. |
| `true` / `false` | `1b` / `0b` | The words are not in scope — `true` is an unresolved name. |
| `Foo { x: 1 }` | `@Foo { x: 1 }` | Bare braces are a **map literal**. The Rust spelling silently means something else. |
| `#[derive(…)]`, `#[test]`, `#[cfg(…)]` | `@test` | `#[…]` does not parse. There are no derive macros. |
| `println!`, `format!`, `vec!`, `assert_eq!` | `println(…)`, `f"…"`, `[…]` | **MAGE has no macros.** These are parse errors that name themselves. |
| `std::io::File`, `foo::<i32>()` | — | `::` does not parse, in paths or in a turbofish. |
| `mod name`, `use std::io` | — | There is no module system. `use` parses and imports nothing; the checker warns. |
| `async fn f()` | `async f()` | `async` *is* the declaration keyword. |
| `unsafe`, `static`, `extern`, `const fn`, `pub(crate)` | — | None of these parse. |
| `String`, `&str` | `str` | One string type. `&mut T` does not parse either. |
| `fn f<T>()` | `fn f[T]()` | `<T>` also parses; `[T]` is canonical. |
| `impl Type { }` | `extend Type { }` | `impl Trait for Type` stays, for trait implementations. |
| `&self` / `&mut self` | `self` | Methods take the value and return the updated one. |

## What genuinely stays the same

`fn` / `pub fn`, `struct`, `enum`, `trait`, `impl Trait for Type`, `if` /
`else` / `match` / `for` / `while` / `loop` / `return` / `break` / `continue`,
`|x| x + 1` closures, tuples, indexing, arithmetic, and the shape of a
signature: `fn name(param: Type) -> Type`.

## What is new

| Feature | Syntax |
|---|---|
| Effect annotation | `pub fn read(p: str) -> str / fs` |
| Effect declaration | `effect Vault { fn read(key: str) -> str; }` |
| Effect handling | `handle { … } with Vault { read(k) => "x" }` |
| Contracts | `sp name { @req(c) @ens(c) @fx() }` |
| Capability namespaces | `fs.read_to_string(p)`, `net.connect(u)`, `mem.alloc(n)` |
| Agents | `agent Name { capabilities: [net] requires_approval: [publish] }` |
| Swarms | `swarm Name { agent: A size: 3 topology: mesh consensus: majority }` |
| Knowledge base | `kb Name { fact f(x); rule r(x: i32) { f(x) } }` |
| Neural nets | `net Name { layer h: Linear(64, 4); forward { h } }` |
| Agent mode | the sigil syntax: `+f`, `v`, `m`, `S`, `E`, `I`, `T` |

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

### MAGE
```MAGE
pub fn fibonacci(n: i32) -> i32 {
    if n <= 1 {
        return n
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}
```

**What changed:**

1. `u64` to `i32` (or `i64`): the integer types are a smaller set.
2. Drop the semicolon after `return n` — statements end at the newline.
3. Nothing else. A pure function over scalars is the one case that really does carry over.

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

    pub fn distance_sq(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}
```

### MAGE
```MAGE
pub struct Point { x: f64, y: f64 }

extend Point {
    pub fn new(x: f64, y: f64) -> Point {
        @Point { x: x, y: y }
    }

    pub fn distance_sq(self, other: Point) -> f64 {
        val dx = self.x - other.x
        val dy = self.y - other.y
        dx * dx + dy * dy
    }

    // `Display` does not exist. Rendering is an ordinary method, and an
    // f-string replaces `write!`/`format!`.
    pub fn show(self) -> str {
        f"({self.x}, {self.y})"
    }
}
```

**What changed:**

1. `impl Point` becomes `extend Point`, and `&self` becomes `self`.
2. `Self` becomes the type name, and `Point { x, y }` becomes `@Point { x: x, y: y }` — there is no field shorthand, and **bare `Point { … }` is a map literal**, not a struct.
3. `#[derive(Debug, Clone)]` is deleted: there are no derive macros, and no attributes of that form.
4. `impl fmt::Display` becomes an ordinary method. `write!` and `format!` are macros, which do not exist — an f-string replaces them.
5. `let` becomes `val`. The parser rejects `let` by name.

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

### MAGE
```MAGE
// `fs.read_to_string` is a capability call, so the effect is `fs` — not `io`,
// which is the console. The error type is whatever you choose; there is no
// `io::Error`, and `?` does not convert error types.
pub fn read_config(path: str) -> R[str, str] / fs {
    val content = fs.read_to_string(path)
    guard len(content) > 0 else { return Err("empty config") }
    Ok(content)
}
```

**What changed:**

1. The effect is **`fs`, not `io`** — the annotation names the capability reached, and `io` is the console.
2. `?` does not convert error types, and there is no `From`. Pick the error type you want and return it.
3. `io::Error` does not exist, and neither does the `::` path that names it.
4. `if content.is_empty() { return Err(…) }` becomes `guard … else`, whose else branch must diverge.

---

## Worked Migration: Async and Collections

### Rust
```rust
use std::collections::HashMap;

pub async fn fetch_all(urls: Vec<String>) -> Result<HashMap<String, String>, Error> {
    let mut results = HashMap::new();
    for url in urls {
        let resp = reqwest::get(&url).await?;
        results.insert(url, resp.text().await?);
    }
    Ok(results)
}
```

### MAGE
```MAGE
// `async` is the declaration keyword — `async fn` is a parse error. Generic
// parameters go in `[…]`, there is no trait-bound machinery to carry over, and
// `net` does not imply `io`: declare each effect you perform.
pub async fetch_all(urls: [str]~) -> {str: str} / net {
    var results = {"": ""}
    for url in urls {
        results[url] = net.connect(url)
    }
    results
}
```

**What changed:**

1. `async fn` is a **parse error**: `async` is itself the declaration keyword, so it is `pub async fetch_all(…)`.
2. `/ net` does not cover an inferred `io`. There is no effect hierarchy — declare each effect performed.
3. `HashMap<String, String>` becomes `{str: str}`. The Rust spelling also parses and lowers to the same type.
4. `reqwest::get(&url).await?` becomes `net.connect(url)` — a capability namespace, in scope everywhere, with nothing to import.

---

## Worked Migration: Trait with Persistence

### Rust
```rust
pub trait Repository<T> {
    fn find(&self, id: u64) -> Option<T>;
    fn save(&mut self, item: T) -> Result<(), Error>;
}
```

### MAGE
```MAGE
// A trait method carries its own effect annotation, and each implementation is
// checked against what it actually performs — which is the whole point of
// putting `/ fs` on the declaration.
pub trait Repository {
    fn find(self, id: i32) -> ?str;
    fn save(self, id: i32, item: str) -> R[i32, str] / fs;
}

pub struct FileRepo { root: str }

impl Repository for FileRepo {
    pub fn find(self, id: i32) -> ?str { None }

    pub fn save(self, id: i32, item: str) -> R[i32, str] / fs {
        fs.write(self.root, item)
        Ok(id)
    }
}
```

**What changed:**

1. The effect annotation goes on the **trait method declaration**, and each implementation is checked against what its body performs.
2. Generic parameters go in `[…]`. With no `dyn` and no monomorphisation to gain, the concrete type is often the better spelling.
3. `&mut self` becomes `self`: a method returns the updated value rather than mutating in place.

---

## Worked Migration: Adding Contracts

### Rust
```rust
pub fn divide(a: f64, b: f64) -> f64 {
    a / b
}
```

### MAGE
```MAGE
// Contracts live in a `sp` block that shares the function's name — they are
// not attributes above the signature, and `result` is not in scope.
sp divide {
    @req(1b)
    @ens(1b)
    @fx()
}

pub fn divide(a: f64, b: f64) -> R[f64, str] {
    guard b != 0.0 else { return Err("divide by zero") }
    Ok(a / b)
}
```

**What changed:**

1. Contracts live in a **`sp` block that shares the function name** — not as attributes above the signature.
2. `result` is not in scope inside `@ens`. Express the postcondition over the arguments, or check it in the body.
3. A partial function is better expressed with `R[T, E]` and a `guard`: the contract records the intent, the guard enforces it.

---

## Worked Migration: Replacing `unsafe` with Capabilities

### Rust
```rust
pub fn read_raw_memory(ptr: *const u8, len: usize) -> Vec<u8> {
    unsafe {
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}
```

### MAGE
```MAGE
// There is no `unsafe`, no raw pointer, and no runtime `cap.check(…)`.
// Reaching a capability namespace *is* the request; the annotation is the
// grant, and the checker enforces it before anything runs.
pub fn read_buffer(size: i32) -> R[i32, str] / alloc {
    guard size > 0 else { return Err("size must be positive") }
    mem.alloc(size)
    Ok(size)
}
```

**What changed:**

1. There is no `unsafe`, no raw pointer type, and no runtime `cap.check(…)`.
2. **Reaching the capability is the request**, the `/ alloc` annotation is the grant, and the checker enforces it before anything runs.
3. The capability namespace for memory is `mem`; `alloc` is the effect kind it performs.

---

## Worked Migration: Threading to Agent / Swarm

### Rust
```rust
use std::thread;

pub fn parallel_process(items: Vec<String>) -> Vec<String> {
    let handles: Vec<_> = items.into_iter().map(|item| {
        thread::spawn(move || process_item(item))
    }).collect();

    handles.into_iter().map(|h| h.join().unwrap()).collect()
}
```

### MAGE
```MAGE
// `thread::spawn` becomes an `agent` declaration plus an ordinary function.
// The agent block says what the role may do; the annotation says what this
// code does; `map` is the fan-out and `fold` the fan-in.
agent ItemProcessor {
    capabilities: [agent]
}

swarm Pool {
    agent: ItemProcessor
    size: 4
    topology: mesh
    consensus: majority
}

fn process_item(item: str) -> str / agent {
    agent.spawn(item)
    upper(item)
}

pub fn parallel_process(items: [str]~) -> [str]~ / agent {
    map(items, |item| process_item(item))
}
```

**What changed:**

1. `thread::spawn` becomes an `agent` declaration (what the role may do) plus an ordinary function (what it does).
2. A `swarm` block declares members, topology and consensus. Fan-out is `map`; fan-in is `fold`.
3. `/ agent` is required on everything that reaches `agent.spawn`, including the caller.
4. There is no handle to join: the effect system, not a handle type, is what makes the concurrency visible.

---

## Migration Checklist

1. [ ] Rename `.rs` to `.mg`.
2. [ ] `let` to `val` / `var`; `true`/`false` to `1b`/`0b`.
3. [ ] Struct literals to `@Name { … }`, every field named.
4. [ ] Delete every `#[…]` attribute; `#[test]` becomes `@test`.
5. [ ] Replace every macro call: `println!` with `println`, `format!` with an
   f-string, `vec!` with a list literal.
6. [ ] Delete every `use` and every `::` path. The standard vocabulary and the
   capability namespaces are already in scope.
7. [ ] `impl Type` to `extend Type`; `&self`/`&mut self` to `self`, returning
   the updated value.
8. [ ] Add an effect annotation to every `pub` function that reaches a
   capability — **every** effect it performs, since none implies another.
9. [ ] Replace `unsafe` with the capability that does the job, and declare its
   effect.
10. [ ] Replace threading with `agent` / `swarm` declarations plus ordinary
    functions.
11. [ ] Run `mage-parse --check` **and** `--eval`. They are independent
    oracles; agreeing with one says nothing about the other.
