# Migration from Rust

The `rust2rdx` tool translates Rust source files into Redox. It applies
28 syntactic transformation rules to convert Rust idioms to their Redox
equivalents.

## Quick start

```sh
# Single file
rdx migrate src/main.rs

# Entire directory
rdx migrate lib/

# Preview without writing
rdx migrate src/main.rs --dry-run
```

## What gets translated

| Rust                  | Redox            | Rule                    |
| --------------------- | ---------------- | ----------------------- |
| `pub fn`              | `+f`             | Function visibility     |
| `fn`                  | `f`              | Private function        |
| `let`                 | `v`              | Immutable binding       |
| `let mut`             | `m`              | Mutable binding         |
| `struct`              | `S`              | Struct declaration      |
| `pub struct`          | `+S`             | Public struct           |
| `enum`                | `E`              | Enum declaration        |
| `trait`               | `T`              | Trait declaration       |
| `impl Trait for Type` | `I Trait ~ Type` | Trait implementation    |
| `impl Type`           | `I ~ Type`       | Inherent implementation |
| `mod`                 | `M`              | Module declaration      |
| `pub mod`             | `+M`             | Public module           |
| `use`                 | `u`              | Import                  |
| `pub use`             | `+u`             | Re-export               |
| `pub(crate)`          | `~`              | Crate visibility        |
| `const`               | `+v`             | Public constant         |
| `true` / `false`      | `1b` / `0b`      | Boolean literals        |
| `String`              | `s`              | String type             |
| `Vec<T>`              | `[T]~`           | Vector type             |
| `Option<T>`           | `?T`             | Option type             |
| `Result<T, E>`        | `R[T, E]`        | Result type             |
| `Box<T>`              | `^T`             | Box (owned pointer)     |
| `Rc<T>`               | `$T`             | Reference-counted       |
| `Arc<T>`              | `@T`             | Atomic ref-counted      |
| `HashMap<K, V>`       | `{K: V}`         | Hash map                |
| `HashSet<K>`          | `{K}`            | Hash set                |
| `&mut`                | `&!`             | Exclusive reference     |
| `if` / `match`        | `?`              | Conditional / match     |
| `else`                | `:`              | Else branch             |
| `for ... in`          | `@ ... :`        | For loop                |
| `return`              | `ret`            | Early return            |
| `async fn`            | `af`             | Async function          |
| `pub async fn`        | `+af`            | Public async function   |
| `where`               | `~>`             | Where clause            |
| `#[derive(...)]`      | `@d(...)`        | Derive attribute        |
| `#[inline]`           | `@i`             | Inline attribute        |
| `#[test]`             | `@test`          | Test attribute          |
| `::` (path sep)       | `.`              | Path separator          |

## Example

### Rust input

```rust
use std::collections::HashMap;

pub fn count_words(text: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        let entry = counts.entry(word.to_string()).or_insert(0);
        *entry += 1;
    }
    counts
}
```

### Redox output

```rdx
u std.col.Map

+f count_words(text: &s) -> {s: usize} {
    m counts = {s: usize}.new()
    @ word : text.split_whitespace() {
        v entry = counts.entry(word.to_string()).or_insert(0)
        *entry += 1
    }
    counts
}
```

## Workflow for large projects

1. **Migrate file by file** — start with leaf modules that have no dependencies
2. **Run `rdx check`** — fix any type errors the translator missed
3. **Run `rdx test`** — verify behavior is preserved
4. **Add effect annotations** — the translator cannot infer effects; add `/ io`,
   `/ net`, etc. manually
5. **Replace `unsafe` blocks** — convert to capability-based safety

```sh
# Migrate in order
rdx migrate src/utils.rs
rdx migrate src/models.rs
rdx migrate src/handlers.rs
rdx migrate src/main.rs

# Verify
rdx check
rdx test
```

## Limitations

The translator handles **syntax** only. You will need to manually:

- Add effect annotations (`/ io`, `/ net`, etc.)
- Replace `unsafe` blocks with capability requests
- Remove lifetime annotations (Redox uses SKB instead)
- Convert `Pin`, `PhantomData`, and other marker types
- Update `Cargo.toml` dependencies to `Forge.toml` format
