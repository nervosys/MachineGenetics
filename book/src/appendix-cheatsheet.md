# Appendix: MechGen Syntax Cheatsheet

MechGen has two syntax modes: **standard** (default, C-like) and **compact**
(`#![syntax(compact)]`). Standard syntax is nearly identical to Rust.

## Standard Syntax (Default)

### Declarations

| Feature         | MechGen Standard          | Notes        |
| --------------- | ----------------------- | ------------ |
| Function        | `fn foo()`              | Same as Rust |
| Public function | `pub fn foo()`          | Same as Rust |
| Async function  | `async fn foo()`        | Same as Rust |
| Pub async fn    | `pub async fn foo()`    | Same as Rust |
| Variable        | `let x = 5;`            | Same as Rust |
| Mutable binding | `let mut x = 5;`        | Same as Rust |
| Constant        | `pub const X: i32 = 5;` | Same as Rust |
| Struct          | `struct Foo { ... }`    | Same as Rust |
| Pub struct      | `pub struct Foo`        | Same as Rust |
| Enum            | `enum Foo { ... }`      | Same as Rust |
| Pub enum        | `pub enum Foo`          | Same as Rust |
| Trait           | `trait Foo { ... }`     | Same as Rust |
| Pub trait       | `pub trait Foo`         | Same as Rust |
| Inherent impl   | `impl Foo { ... }`      | Same as Rust |
| Trait impl      | `impl Bar for Foo`      | Same as Rust |
| Module          | `mod foo;`              | Same as Rust |
| Pub module      | `pub mod foo`           | Same as Rust |
| Import          | `use foo::bar;`         | Same as Rust |
| Re-export       | `pub use foo::bar;`     | Same as Rust |
| Crate-visible   | `pub(crate) fn`         | Same as Rust |

### Types

| Type               | Standard           | Notes |
| ------------------ | ------------------ | ----- |
| Owned string       | `String`           | Same  |
| String slice       | `&str`             | Same  |
| Vector             | `Vec<T>`           | Same  |
| Option             | `Option<T>`        | Same  |
| Result             | `Result<T, E>`     | Same  |
| Boxed              | `Box<T>`           | Same  |
| Ref-counted        | `Rc<T>`            | Same  |
| Atomic ref-counted | `Arc<T>`           | Same  |
| Hash map           | `HashMap<K, V>`    | Same  |
| Hash set           | `HashSet<K>`       | Same  |
| Mutable reference  | `&mut T`           | Same  |
| Bool literals      | `true` / `false`   | Same  |
| Primitives         | `i32`, `f64`, etc. | Same  |

### Control Flow

| Feature           | Standard                | Notes |
| ----------------- | ----------------------- | ----- |
| Conditional       | `if cond { ... }`       | Same  |
| Else              | `else { ... }`          | Same  |
| Else if           | `else if cond { ... }`  | Same  |
| Pattern match     | `match x { ... }`       | Same  |
| For loop          | `for x in iter { ... }` | Same  |
| Loop              | `loop { ... }`          | Same  |
| While             | `while cond { ... }`    | Same  |
| Return            | `return x`              | Same  |
| Error propagation | `x?`                    | Same  |
| Break / continue  | `break` / `continue`    | Same  |

### Attributes

| Feature | Standard         | Notes |
| ------- | ---------------- | ----- |
| Derive  | `#[derive(...)]` | Same  |
| Inline  | `#[inline]`      | Same  |
| Test    | `#[test]`        | Same  |
| Config  | `#[cfg(...)]`    | Same  |

### Generics

| Feature      | Standard           | Notes |
| ------------ | ------------------ | ----- |
| Generic fn   | `fn foo<T>(x: T)`  | Same  |
| Bounds       | `T: Clone + Debug` | Same  |
| Where clause | `where T: Clone`   | Same  |
| Turbofish    | `foo::<i32>()`     | Same  |

### Paths

| Feature    | Standard                    | Notes |
| ---------- | --------------------------- | ----- |
| Std path   | `std::collections::HashMap` | Same  |
| Crate path | `crate::module::Type`       | Same  |
| Self path  | `self::sub`                 | Same  |
| Super path | `super::parent`             | Same  |

## MechGen-Unique Features (Both Modes)

| Feature            | Syntax                                    |
| ------------------ | ----------------------------------------- |
| Effect annotation  | `fn foo() / io { ... }`                   |
| Multiple effects   | `fn foo() / io, net { ... }`              |
| Pure function      | `fn foo() -> i32 { ... }` (no `/` clause) |
| Capability request | `Capability::request("ffi")?`             |
| Capability region  | `Region::enter(cap, \|\| { ... })`        |
| SKB query          | `skb::query().category("borrow")`         |
| Agent trait        | `impl Agent for MyAgent { ... }`          |
| Swarm operations   | `swarm.broadcast("task")?`                |
| Effect handlers    | `handle::<IoEffect, T>(f, handler)`       |

## Compact Syntax Reference

Activate with `#![syntax(compact)]` at the top of a `.mg` file.

### Compact Declarations

| Standard             | Compact       |
| -------------------- | ------------- |
| `pub fn foo()`       | `+f foo()`    |
| `fn foo()`           | `f foo()`     |
| `pub async fn foo()` | `+af foo()`   |
| `async fn foo()`     | `af foo()`    |
| `let x = 5`          | `v x = 5`     |
| `let mut x = 5`      | `m x = 5`     |
| `pub const X = 5`    | `+v X = 5`    |
| `pub struct Foo`     | `+S Foo`      |
| `struct Foo`         | `S Foo`       |
| `pub enum Foo`       | `+E Foo`      |
| `enum Foo`           | `E Foo`       |
| `pub trait Foo`      | `+T Foo`      |
| `impl Foo for Bar`   | `I Foo ~ Bar` |
| `impl Foo`           | `I ~ Foo`     |
| `pub mod foo`        | `+M foo`      |
| `use foo::bar`       | `u foo.bar`   |
| `pub use foo::bar`   | `+u foo.bar`  |
| `pub(crate) fn`      | `~f`          |

### Compact Types

| Standard         | Compact     |
| ---------------- | ----------- |
| `String`         | `s`         |
| `&str`           | `&s`        |
| `Vec<T>`         | `[T]~`      |
| `Option<T>`      | `?T`        |
| `Result<T, E>`   | `R[T, E]`   |
| `Box<T>`         | `^T`        |
| `Rc<T>`          | `$T`        |
| `Arc<T>`         | `@T`        |
| `HashMap<K, V>`  | `{K: V}`    |
| `HashSet<K>`     | `{K}`       |
| `&mut T`         | `&!T`       |
| `true` / `false` | `1b` / `0b` |

### Compact Control Flow

| Standard          | Compact          |
| ----------------- | ---------------- |
| `if cond { ... }` | `? cond { ... }` |
| `else { ... }`    | `: { ... }`      |
| `else if cond`    | `: ? cond`       |
| `match x { ... }` | `? x { ... }`    |
| `for x in iter`   | `@ x : iter`     |
| `while cond`      | `loop ? cond`    |
| `return x`        | `ret x`          |

### Compact Attributes

| Standard         | Compact     |
| ---------------- | ----------- |
| `#[derive(...)]` | `@d(...)`   |
| `#[inline]`      | `@i`        |
| `#[test]`        | `@test`     |
| `#[cfg(...)]`    | `@cfg(...)` |

### Compact Generics & Paths

| Standard              | Compact          |
| --------------------- | ---------------- |
| `fn foo<T>(x: T)`     | `f foo[T](x: T)` |
| `where T: Clone`      | `~> T: Clone`    |
| `foo::<i32>()`        | `foo[i32]()`     |
| `std::io`             | `std.io`         |
| `crate::module::Type` | `~.module.Type`  |
| `Foo { x: 1, y: 2 }`  | `Foo @{ x: 1 }`  |

### Compact Format Strings

| Standard             | Compact      |
| -------------------- | ------------ |
| `println!("hello")`  | `p"hello"`   |
| `format!("x = {x}")` | `f"x = {x}"` |
| `eprintln!("err")`   | `ep"err"`    |

## Safety (replaces `unsafe`)

| Rust              | MechGen                                 |
| ----------------- | ------------------------------------- |
| `unsafe { ... }`  | `Capability::request("ffi")? { ... }` |
| Lifetime `'a`     | SKB rule-based (no syntax)            |
| `PhantomData<T>`  | Not needed                            |
| `Pin<T>`          | Not needed                            |
| `ManuallyDrop<T>` | Not needed                            |
