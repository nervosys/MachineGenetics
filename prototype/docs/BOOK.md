# The MechGen Book

> A comprehensive guide to the MechGen programming language.

---

## Part I — Getting Started

### Chapter 1: Introduction

MechGen is a systems programming language designed for agentic compilation. It
inherits Rust's safety and performance model while adding:

- **Contract-first design** — preconditions, postconditions, and invariants are
  part of the type surface.
- **Algebraic effect system** — side effects are tracked and propagated.
- **Token-minimal syntax** — optimised for LLM consumption.
- **Cost oracle** — every construct exposes its cost before compilation.
- **Swarm primitives** — first-class support for multi-agent workflows.

### Chapter 2: Installation

```bash
# Clone the repository
git clone https://github.com/nervosys/MechGen.git
cd MechGen/prototype

# Build
cargo build

# Run a MechGen file
cargo run -- input.rx

# Run tests
cargo test
```

### Chapter 3: Hello, MechGen

```
+f main() {
    p"Hello, MechGen!"
}
```

This defines a public function `main` that prints "Hello, MechGen!" to stdout.

- `+f` — public function (`pub fn` in Rust)
- `p"..."` — print macro (`println!` in Rust)

---

## Part II — Language Tour

### Chapter 4: Variables and Types

```
let x: i32 = 42;
let mut name: s = "MechGen".to_string();
let pi: f64 = 3.14159;
let active: bool = true;
```

Type sigils for collections:

```
let nums: [i32]~ = vec![1, 2, 3];       // Vec<i32>
let fixed: [i32; 3] = [1, 2, 3];        // [i32; 3]
let map: {s: i32} = HashMap.new();       // HashMap<String, i32>
let set: {i32} = HashSet.new();          // HashSet<i32>
```

Smart pointers:

```
let boxed: ^i32 = ^42;                  // Box<i32>
let shared: $Node = $.new(node);         // Rc<Node>
let atomic: @Data = @.new(data);         // Arc<Data>
```

### Chapter 5: Functions

```
f add(x: i32, y: i32) -> i32 {
    x + y
}

+f greet(name: &s) {
    p"Hello, {name}!"
}

af fetch_data(url: &s) -> s!Error {
    http.get(url).await?.text().await?
}
```

### Chapter 6: Control Flow

```
// Conditionals
?: x > 0 {
    p"positive"
} _ {
    p"non-positive"
}

// Pattern matching
? shape {
    Circle { r } => pi * r * r,
    Rect { w, h } => w * h,
    _ => 0.0,
}

// Loops
@ item in items {
    process(item)
}

@@ {
    ?: done { ! }
    work()
}
```

### Chapter 7: Structs and Enums

```
+S Point {
    x: f64,
    y: f64,
}

I Point {
    +f new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

+E Color {
    Red,
    Green,
    Blue,
    Custom { r: u8, g: u8, b: u8 },
}
```

### Chapter 8: Traits

```
+T Summary {
    f summarize(&self) -> s;

    f preview(&self) -> s {
        let full = self.summarize();
        f"{full[..20]}..."
    }
}

I Summary for Point {
    f summarize(&self) -> s {
        f"({self.x}, {self.y})"
    }
}
```

### Chapter 9: Error Handling

```
f parse_number(input: &s) -> i32!ParseError {
    input.parse()?
}

f find_item(items: &[i32], target: i32) -> i32? {
    @ item in items {
        ?: *item == target { ret Some(*item) }
    }
    None
}
```

---

## Part III — Advanced Features

### Chapter 10: Contracts

Contracts are first-class in MechGen:

```
f binary_search(arr: &[i32], target: i32) -> usize?
    @req arr.windows(2).all(|w| w[0] <= w[1]) "array must be sorted"
    @ens result.map_or(true, |i| arr[i] == target)
{
    // implementation
}
```

Struct invariants:

```
S BoundedQueue<T> {
    items: [T]~,
    capacity: usize,
}
    @inv self.items.len() <= self.capacity
```

### Chapter 11: Effect System

```
f pure_add(x: i32, y: i32) -> i32
    @fx pure
{
    x + y
}

f read_config(path: &s) -> Config!IoError
    @fx io, fs
{
    let data = std.fs.read_to_string(path)?;
    parse_config(&data)
}
```

Effects propagate through the call graph. The compiler tracks which functions
have which effects and reports violations.

### Chapter 12: Cost Oracle

Every construct has a queryable cost:

```
// Query the cost of Vec::push on x86_64
let cost = cost.query("Vec::push", "x86_64", Release);
// cost.cycles = 5, cost.memory_bytes = 0, cost.latency_ns = 3
```

Agents use cost queries to make informed decisions about code generation.

### Chapter 13: Performance Annotations

```
f hot_loop(data: &[f64]) -> f64
    @perf force_inline
    @perf vectorize(256)
    @perf pure
{
    let mut sum = 0.0;
    @ x in data { sum += x; }
    sum
}
```

### Chapter 14: Swarm Communication

```
// Publish/subscribe bus
let bus = SwarmBus.new();
bus.publish("build.complete", result);

// Agent leases
let lease = Lease.acquire(resource, Duration.secs(30));

// Capability sandbox
let sandbox = SandboxManager.create("agent-1", limits);
sandbox.grant(CapabilityToken.restricted("fs.read"));
```

---

## Part IV — Tooling

### Chapter 15: The MechGen Compiler

The MechGen compiler pipeline:

1. **Lexer** — tokenises source into a stream of tokens
2. **Parser** — builds an AST from the token stream
3. **Name Resolution** — resolves identifiers to definitions
4. **Type Checking** — verifies types, contracts, and effects
5. **HIR Lowering** — lowers AST to High-level IR
6. **MLIR Emission** — generates MLIR for target-specific optimisation
7. **Code Generation** — produces machine code via LLVM/Cranelift

### Chapter 16: Legacy Compatibility

```bash
# Convert Rust to MechGen
MechGen transpile --from-rust src/main.rs -o src/main.rx

# Convert MechGen to Rust
MechGen transpile --to-rust src/main.rx -o src/main.rs
```

### Chapter 17: Project Configuration

```toml
# MechGen.toml
[package]
name = "my-project"
version = "0.1.0"

[dependencies]
serde = "1.0"

[agent]
swarm_size = 4
token_budget = 8192
```

---

## Appendix: Syntax Quick Reference

| MechGen  | Rust       | Meaning           |
| ------ | ---------- | ----------------- |
| `f`    | `fn`       | Function          |
| `+f`   | `pub fn`   | Public function   |
| `af`   | `async fn` | Async function    |
| `S`    | `struct`   | Struct            |
| `E`    | `enum`     | Enum              |
| `T`    | `trait`    | Trait             |
| `I`    | `impl`     | Impl block        |
| `?:`   | `if`       | Conditional       |
| `?`    | `match`    | Pattern match     |
| `@`    | `for`      | For loop          |
| `@@`   | `loop`     | Infinite loop     |
| `!`    | `break`    | Break             |
| `>>`   | `continue` | Continue          |
| `[T]~` | `Vec<T>`   | Dynamic array     |
| `^T`   | `Box<T>`   | Box pointer       |
| `$T`   | `Rc<T>`    | Reference count   |
| `@T`   | `Arc<T>`   | Atomic ref count  |
| `@req` | —          | Precondition      |
| `@ens` | —          | Postcondition     |
| `@inv` | —          | Invariant         |
| `@fx`  | —          | Effect annotation |
