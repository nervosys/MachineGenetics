# Appendix: Redox ↔ Rust Cheatsheet

A comprehensive mapping between Rust and Redox syntax.

## Declarations

| Rust                 | Redox             | Notes                 |
| -------------------- | ----------------- | --------------------- |
| `fn foo()`           | `f foo()`         | Private function      |
| `pub fn foo()`       | `+f foo()`        | Public function       |
| `async fn foo()`     | `af foo()`        | Async function        |
| `pub async fn foo()` | `+af foo()`       | Public async function |
| `let x = 5;`         | `v x = 5`         | Immutable binding     |
| `let mut x = 5;`     | `m x = 5`         | Mutable binding       |
| `const X: i32 = 5;`  | `+v X: i32 = 5`   | Public constant       |
| `struct Foo { ... }` | `S Foo { ... }`   | Struct                |
| `pub struct Foo`     | `+S Foo`          | Public struct         |
| `enum Foo { ... }`   | `E Foo { ... }`   | Enum                  |
| `pub enum Foo`       | `+E Foo`          | Public enum           |
| `trait Foo { ... }`  | `T Foo { ... }`   | Trait                 |
| `pub trait Foo`      | `+T Foo`          | Public trait          |
| `impl Foo { ... }`   | `I ~ Foo { ... }` | Inherent impl         |
| `impl Bar for Foo`   | `I Bar ~ Foo`     | Trait impl            |
| `mod foo;`           | `M foo;`          | Module                |
| `pub mod foo`        | `+M foo`          | Public module         |
| `use foo::bar;`      | `u foo.bar`       | Import                |
| `pub use foo::bar;`  | `+u foo.bar`      | Re-export             |
| `pub(crate) fn`      | `~f`              | Crate-visible         |

## Types

| Rust                 | Redox       | Saves     |
| -------------------- | ----------- | --------- |
| `String`             | `s`         | 5 chars   |
| `&str`               | `&s`        | 2 chars   |
| `Vec<T>`             | `[T]~`      | 2 chars   |
| `Option<T>`          | `?T`        | 7 chars   |
| `Result<T, E>`       | `R[T, E]`   | 5 chars   |
| `Box<T>`             | `^T`        | 3 chars   |
| `Rc<T>`              | `$T`        | 2 chars   |
| `Arc<T>`             | `@T`        | 3 chars   |
| `HashMap<K, V>`      | `{K: V}`    | 7 chars   |
| `HashSet<K>`         | `{K}`       | 7 chars   |
| `&mut T`             | `&!T`       | 2 chars   |
| `bool`               | `bool`      | same      |
| `true` / `false`     | `1b` / `0b` | 2-3 chars |
| `i8..i128, u8..u128` | same        | same      |
| `f32, f64`           | same        | same      |
| `usize, isize`       | same        | same      |
| `char`               | `char`      | same      |
| `()`                 | `()`        | same      |

## Control flow

| Rust              | Redox                        | Notes                |
| ----------------- | ---------------------------- | -------------------- |
| `if cond { ... }` | `? cond { ... }`             | `?` replaces `if`    |
| `else { ... }`    | `: { ... }`                  | `:` replaces `else`  |
| `else if cond`    | `: ? cond`                   | Chained              |
| `match x { ... }` | `? x { ... }`                | Unified `if`/`match` |
| `for x in iter`   | `@ x : iter`                 | `@` replaces `for`   |
| `loop { ... }`    | `loop { ... }`               | Same                 |
| `while cond`      | `loop { ? !cond { break } }` | Explicit loop        |
| `return x`        | `ret x`                      | Early return         |
| `x?`              | `x?`                         | Same (postfix `?`)   |
| `break`           | `break`                      | Same                 |
| `continue`        | `continue`                   | Same                 |

## Attributes

| Rust             | Redox       | Notes            |
| ---------------- | ----------- | ---------------- |
| `#[derive(...)]` | `@d(...)`   | Derive macro     |
| `#[inline]`      | `@i`        | Inline hint      |
| `#[test]`        | `@test`     | Test attribute   |
| `#[cfg(...)]`    | `@cfg(...)` | Config attribute |

## Generics & Bounds

| Rust                       | Redox              | Notes                 |
| -------------------------- | ------------------ | --------------------- |
| `fn foo<T>(x: T)`          | `f foo[T](x: T)`   | Square brackets       |
| `T: Clone + Debug`         | `T: Clone + Debug` | Same                  |
| `where T: Clone`           | `~> T: Clone`      | `~>` replaces `where` |
| `foo::<i32>()` (turbofish) | `foo[i32]()`       | No turbofish          |

## Paths

| Rust                        | Redox           |
| --------------------------- | --------------- |
| `std::collections::HashMap` | `std.col.Map`   |
| `crate::module::Type`       | `~.module.Type` |
| `self::sub`                 | `self.sub`      |
| `super::parent`             | `super.parent`  |
| `foo::bar::baz()`           | `foo.bar.baz()` |

## Effects (no Rust equivalent)

```rdx
// Effect annotation — declares side effects
+f read_file(path: &s) -> R[s, IoError] / io { ... }

// Multiple effects
+f fetch(url: &s) -> R[s, Error] / io, net { ... }

// Pure function — no effects
+f add(a: i32, b: i32) -> i32 { a + b }
```

## Safety (replaces `unsafe`)

| Rust              | Redox                             |
| ----------------- | --------------------------------- |
| `unsafe { ... }`  | `Capability.request(ffi) { ... }` |
| Lifetime `'a`     | SKB rule-based (no syntax)        |
| `PhantomData<T>`  | Not needed                        |
| `Pin<T>`          | Not needed                        |
| `ManuallyDrop<T>` | Not needed                        |

## Struct literals

| Rust                      | Redox                      |
| ------------------------- | -------------------------- |
| `Foo { x: 1, y: 2 }`      | `Foo @{ x: 1, y: 2 }`      |
| `Foo { x, y }`            | `Foo @{ x, y }`            |
| `Foo { x: 1, ..default }` | `Foo @{ x: 1, ..default }` |

## Closures

| Rust                          | Redox                         |
| ----------------------------- | ----------------------------- |
| `\|x\| x + 1`                 | `\|x\| x + 1`                 |
| `\|x: i32\| -> i32 { x + 1 }` | `\|x: i32\| -> i32 { x + 1 }` |
| `move \|\| { ... }`           | `move \|\| { ... }`           |

## Format strings

| Rust                 | Redox        | Notes           |
| -------------------- | ------------ | --------------- |
| `format!("x = {x}")` | `f"x = {x}"` | Format string   |
| `println!("hello")`  | `p"hello"`   | Print + newline |
| `eprintln!("err")`   | `ep"err"`    | Stderr print    |
