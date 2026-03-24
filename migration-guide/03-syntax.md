# Chapter 3: Syntax Migration

The mechanical translation of Rust syntax to MechGen syntax. Most of this can be
automated with `mg migrate`, but understanding each rule helps you fix edge
cases and review the output.

---

## 3.1 Automated Migration

Run the automated translator on your Rust source:

```bash
# Migrate a single file
mg migrate src/lib.rs --output src/lib.mg

# Migrate an entire directory
mg migrate src/ --output-dir src/

# Preview without writing (recommended first)
mg migrate src/ --dry-run

# Migrate with verbose diff output
mg migrate src/lib.rs --diff
```

The tool handles ~80% of translations automatically. The remaining 20%
requires manual attention: effect annotations, lifetime removal, unsafe
replacement, and idiom adjustments.

## 3.2 Declaration Keywords

### Functions

```diff
- fn private_function() { }
+ f private_function() { }

- pub fn public_function() { }
+ +f public_function() { }

- pub(crate) fn crate_function() { }
+ ~f crate_function() { }

- async fn async_private() { }
+ af async_private() { }

- pub async fn async_public() { }
+ +af async_public() { }

- const fn const_func() -> i32 { 42 }
+ c f const_func() -> i32 { 42 }
```

### Bindings

```diff
- let x = 42;
+ v x = 42

- let mut counter = 0;
+ m counter = 0

- pub const MAX_SIZE: usize = 1024;
+ +v MAX_SIZE: usize = 1024
```

### Types

```diff
- struct Point { x: f64, y: f64 }
+ S Point { x: f64, y: f64 }

- pub struct Config { pub host: String, port: u16 }
+ +S Config { +host: s, port: u16 }

- enum Color { Red, Green, Blue }
+ E Color { Red, Green, Blue }

- pub enum Result<T, E> { Ok(T), Err(E) }
+ +E Result[T, E] { Ok(T), Err(E) }

- trait Display { fn fmt(&self, f: &mut Formatter) -> Result; }
+ T Display { f fmt(&self, fmtr: &!Formatter) -> Result }

- pub trait Iterator { type Item; fn next(&mut self) -> Option<Self::Item>; }
+ +T Iterator { type Item; f next(&!self) -> ?Self.Item }
```

### Implementations

```diff
- impl Point { }
+ I ~ Point { }

- impl Display for Point { }
+ I Display ~ Point { }

- impl<T: Clone> From<Vec<T>> for MyType<T> { }
+ I[T: Clone] From[[T]~] ~ MyType[T] { }
```

### Modules and Imports

```diff
- mod utils;
+ M utils

- pub mod handlers;
+ +M handlers

- use std::io::File;
+ u std.io.File

- use std::collections::{HashMap, HashSet};
+ u std.col.{HashMap, HashSet}

- pub use crate::models::User;
+ +u ~.models.User
```

## 3.3 Control Flow

### Conditionals

```diff
  // Simple if
- if x > 0 {
+ ? x > 0 {
      do_something()
  }

  // If-else
- if x > 0 {
+ ? x > 0 {
      positive()
- } else {
+ } : {
      non_positive()
  }

  // If-else chain
- if x > 0 {
+ ? x > 0 {
      positive()
- } else if x == 0 {
+ } : ? x == 0 {
      zero()
- } else {
+ } : {
      negative()
  }
```

### Match / Pattern Matching

```diff
- match color {
-     Color::Red => println!("red"),
-     Color::Green => println!("green"),
-     Color::Blue => println!("blue"),
- }
+ ? color {
+     Color.Red => p"red",
+     Color.Green => p"green",
+     Color.Blue => p"blue",
+ }
```

### Loops

```diff
  // For loop
- for item in collection {
+ @ item ~ collection {
      process(item)
  }

  // For with range
- for i in 0..10 {
+ @ i ~ 0..10 {
      process(i)
  }

  // Return
- return value;
+ ret value
```

## 3.4 Attributes

```diff
- #[derive(Debug, Clone, PartialEq)]
+ @d(Debug, Clone, PartialEq)

- #[inline]
+ @i

- #[inline(always)]
+ @i(always)

- #[test]
+ @test

- #[bench]
+ @bench

- #[cfg(test)]
+ @cfg(test)

- #[cfg(target_os = "linux")]
+ @cfg(target_os = "linux")

- #[allow(dead_code)]
+ @allow(dead_code)

- #[must_use]
+ @must_use
```

## 3.5 String and Output Macros

```diff
- println!("Hello, {}", name);
+ p"Hello, {name}"

- println!("x = {}, y = {}", point.x, point.y);
+ p"x = {point.x}, y = {point.y}"

- eprintln!("Error: {}", err);
+ ep"Error: {err}"

- format!("Hello, {}", name)
+ f"Hello, {name}"

- String::from("hello")
+ s.from("hello")

- "hello".to_string()
+ "hello".to_string()    // same — or s.from("hello")
```

## 3.6 Paths and Namespaces

```diff
  // Module paths
- std::io::File
+ std.io.File

- std::collections::HashMap
+ std.col.HashMap

  // Crate root
- crate::models::User
+ ~.models.User

  // Enum variants
- Option::Some(x)
+ Some(x)              // same (prelude)

- MyEnum::Variant
+ MyEnum.Variant

  // Associated functions
- String::new()
+ s.new()

- Vec::new()
+ [T]~.new()           // Or just [T]~.new() with the concrete type

- HashMap::new()
+ {K: V}.new()
```

## 3.7 Generics

```diff
  // Function generics
- fn identity<T>(x: T) -> T { x }
+ f identity[T](x: T) -> T { x }

  // Multiple generics
- fn pair<A, B>(a: A, b: B) -> (A, B) { (a, b) }
+ f pair[A, B](a: A, b: B) -> (A, B) { (a, b) }

  // Bounds
- fn clone_it<T: Clone>(x: &T) -> T { x.clone() }
+ f clone_it[T: Clone](x: &T) -> T { x.clone() }

  // Where clause
- fn process<T>(x: T) where T: Clone + Debug { }
+ f process[T](x: T) ~> T: Clone + Debug { }

  // Turbofish
- let x = parse::<i32>("42");
+ v x = parse[i32]("42")

  // Struct generics
- struct Wrapper<T> { inner: T }
+ S Wrapper[T] { inner: T }
```

## 3.8 Boolean Literals

```diff
- let active = true;
+ v active = 1b

- let deleted = false;
+ v deleted = 0b

- if enabled == true { }
+ ? enabled == 1b { }

  // In struct literals
- Config { debug: false, verbose: true }
+ Config @{ debug: 0b, verbose: 1b }
```

## 3.9 Struct Literals

```diff
  // Named fields
- let point = Point { x: 1.0, y: 2.0 };
+ v point = Point @{ x: 1.0, y: 2.0 }

  // Shorthand
- let point = Point { x, y };
+ v point = Point @{ x, y }

  // Update syntax
- let point2 = Point { x: 3.0, ..point };
+ v point2 = Point @{ x: 3.0, ..point }
```

## 3.10 Semicolons

MechGen uses the same semicolon rules as Rust: statements end with semicolons
(often optional in practice), and the last expression in a block is the return
value without a semicolon.

```diff
  // These are equivalent in MechGen:
  v x = 42;
  v x = 42    // semicolon optional for bindings

  // Block return value — no semicolon on final expression
  f double(x: i32) -> i32 {
      x * 2    // implicit return
  }
```

## 3.11 Quick Regex Reference

For scripted migrations, these regexes cover the most common patterns:

| Pattern      | Find (Rust)   | Replace (MechGen) |
| ------------ | ------------- | --------------- |
| `pub fn`     | `pub fn `     | `+f `           |
| `fn`         | `fn `         | `f `            |
| `let mut`    | `let mut `    | `m `            |
| `let`        | `let `        | `v `            |
| `pub struct` | `pub struct ` | `+S `           |
| `struct`     | `struct `     | `S `            |
| `pub enum`   | `pub enum `   | `+E `           |
| `enum`       | `enum `       | `E `            |
| `pub trait`  | `pub trait `  | `+T `           |
| `trait`      | `trait `      | `T `            |
| `pub mod`    | `pub mod `    | `+M `           |
| `mod`        | `mod `        | `M `            |
| `return`     | `return `     | `ret `          |
| `::`         | `::`          | `.`             |
| `true`       | `\btrue\b`    | `1b`            |
| `false`      | `\bfalse\b`   | `0b`            |

> **Warning:** Naive regex substitution can break strings and comments. Always
> use `mg migrate` for production migrations — it parses the Rust AST properly.
