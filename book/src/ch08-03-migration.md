# Migration from Rust

The `rust2mg` tool translates Rust source files into MechGen. Because MechGen's
standard syntax closely mirrors Rust, migration is mostly straightforward — the
main additions are effect annotations and the capability system.

## Quick start

```sh
# Single file
mg migrate src/main.rs

# Entire directory
mg migrate lib/

# Preview without writing
mg migrate src/main.rs --dry-run
```

## What stays the same

Most Rust syntax is valid MechGen in standard mode:

| Feature               | Rust                 | MechGen (Standard)     |
| --------------------- | -------------------- | -------------------- |
| Functions             | `pub fn foo()`       | `pub fn foo()`       |
| Variables             | `let x = 5;`         | `let x = 5;`         |
| Mutable bindings      | `let mut x = 5;`     | `let mut x = 5;`     |
| Structs               | `struct Foo { ... }` | `struct Foo { ... }` |
| Enums                 | `enum Foo { ... }`   | `enum Foo { ... }`   |
| Traits                | `trait Foo { ... }`  | `trait Foo { ... }`  |
| Impl blocks           | `impl Foo for Bar`   | `impl Foo for Bar`   |
| Modules               | `pub mod foo`        | `pub mod foo`        |
| Imports               | `use std::io`        | `use std::io`        |
| Generics              | `fn foo<T>(x: T)`    | `fn foo<T>(x: T)`    |
| Where clauses         | `where T: Clone`     | `where T: Clone`     |
| Pattern matching      | `match x { ... }`    | `match x { ... }`    |
| `if` / `else`         | `if cond { ... }`    | `if cond { ... }`    |
| `for` loops           | `for x in iter`      | `for x in iter`      |
| Closures              | `\|x\| x + 1`        | `\|x\| x + 1`        |
| `Vec<T>`, `Option<T>` | same                 | same                 |
| `HashMap<K, V>`       | same                 | same                 |
| `Box<T>`, `Arc<T>`    | same                 | same                 |
| `&mut T`              | same                 | same                 |
| `println!()` etc.     | same                 | same                 |
| `#[derive(...)]`      | same                 | same                 |
| `#[test]`             | same                 | same                 |

## What changes

| Rust                 | MechGen Addition                | Notes                     |
| -------------------- | ----------------------------- | ------------------------- |
| (no equivalent)      | `/ io`, `/ net`, `/ rng` etc. | Effect annotations on fns |
| `unsafe { ... }`     | `Capability::request("ffi")?` | Capability system         |
| Lifetime annotations | Removed — SKB handles them    | No `'a` syntax            |
| `Cargo.toml`         | `Forge.toml`                  | Project manifest          |
| `.rs` extension      | `.mg` extension              | File extension            |

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

### MechGen output (standard syntax)

```mg
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

The syntax is nearly identical. The translator removes lifetime annotations
and renames the file from `.rs` to `.mg`.

## Workflow for large projects

1. **Migrate file by file** — start with leaf modules that have no dependencies
2. **Run `mg check`** — fix any type errors the translator missed
3. **Run `mg test`** — verify behavior is preserved
4. **Add effect annotations** — the translator cannot infer effects; add `/ io`,
   `/ net`, etc. manually
5. **Replace `unsafe` blocks** — convert to capability-based safety

```sh
# Migrate in order
mg migrate src/utils.rs
mg migrate src/models.rs
mg migrate src/handlers.rs
mg migrate src/main.rs

# Verify
mg check
mg test
```

## Limitations

The translator handles **syntax** only. You will need to manually:

- Add effect annotations (`/ io`, `/ net`, etc.)
- Replace `unsafe` blocks with capability requests
- Remove lifetime annotations (MechGen uses SKB instead)
- Convert `Pin`, `PhantomData`, and other marker types
- Update `Cargo.toml` dependencies to `Forge.toml` format
