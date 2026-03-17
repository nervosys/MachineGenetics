# Rust → Redox Migration Patterns

> Side-by-side translation rules for AI agents converting Rust code to Redox.

---

## Rule Table: Keyword Substitutions

| Rust | Redox | Example |
|------|-------|---------|
| `fn name()` | `f name()` | `f greet()` |
| `pub fn name()` | `+f name()` | `+f greet()` |
| `pub(crate) fn name()` | `~f name()` | `~f helper()` |
| `async fn name()` | `af name()` | `af fetch()` |
| `pub async fn name()` | `+af name()` | `+af fetch()` |
| `const fn name()` | `c f name()` | `c f max()` |
| `let x = ...` | `v x = ...` | `v count = 0` |
| `let mut x = ...` | `m x = ...` | `m total = 0` |
| `pub const X` | `+v X` | `+v MAX: i32 = 100` |
| `struct Foo` | `S Foo` | `S Point { x: f64 }` |
| `pub struct Foo` | `+S Foo` | `+S Point { x: f64 }` |
| `enum Bar` | `E Bar` | `E Color { Red, Blue }` |
| `pub enum Bar` | `+E Bar` | `+E Color { Red, Blue }` |
| `trait Tr` | `T Tr` | `T Display { }` |
| `pub trait Tr` | `+T Tr` | `+T Display { }` |
| `impl Trait for Type` | `I Trait ~ Type` | `I Display ~ Foo` |
| `impl Type` | `I ~ Type` | `I ~ Foo` |
| `mod name` | `M name` | `M utils` |
| `pub mod name` | `+M name` | `+M utils` |
| `use path::Item` | `u path.Item` | `u std.io.File` |
| `pub use path::Item` | `+u path.Item` | `+u ~.models.User` |
| `return expr` | `ret expr` | `ret 42` |
| `if cond { }` | `? cond { }` | `? x > 0 { }` |
| `else { }` | `: { }` | `: { default() }` |
| `else if cond { }` | `: ? cond { }` | `: ? x == 0 { }` |
| `match expr { }` | `? expr { }` | `? color { Red => 1 }` |
| `for x in iter { }` | `@ x ~ iter { }` | `@ n ~ 0..10 { }` |
| `true` | `1b` | `v flag = 1b` |
| `false` | `0b` | `v done = 0b` |

## Rule Table: Type Substitutions

| Rust | Redox |
|------|-------|
| `String` | `s` |
| `&str` | `&s` |
| `Vec<T>` | `[T]~` |
| `Option<T>` | `?T` |
| `Result<T, E>` | `R[T, E]` |
| `Box<T>` | `^T` |
| `Rc<T>` | `$T` |
| `Arc<T>` | `@T` |
| `HashMap<K, V>` | `{K: V}` |
| `HashSet<K>` | `{K}` |
| `&mut T` | `&!T` |

## Rule Table: Syntax Substitutions

| Rust | Redox |
|------|-------|
| `std::io::File` | `std.io.File` |
| `crate::module::Item` | `~.module.Item` |
| `foo::<i32>()` | `foo[i32]()` |
| `fn foo<T>(x: T)` | `f foo[T](x: T)` |
| `where T: Clone` | `~> T: Clone` |
| `Foo { x: 1, y: 2 }` | `Foo @{ x: 1, y: 2 }` |
| `#[derive(Debug)]` | `@d(Debug)` |
| `#[inline]` | `@i` |
| `#[test]` | `@test` |
| `#[cfg(test)]` | `@cfg(test)` |
| `println!("hi {x}")` | `p"hi {x}"` |
| `format!("hi {x}")` | `f"hi {x}"` |
| `eprintln!("err {e}")` | `ep"err {e}"` |

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

### Redox
```redox
+f fibonacci(n: u64) -> u64 {
    ? n <= 1 {
        ret n
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}
```

**Steps applied:**
1. `pub fn` → `+f`
2. `if` → `?`
3. `return` → `ret`
4. Remove semicolons where implicit

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

    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}
```

### Redox
```redox
u std.fmt

@d(Debug, Clone)
+S Point {
    +x: f64,
    +y: f64,
}

I ~ Point {
    +f new(x: f64, y: f64) -> Self {
        Point @{ x, y }
    }

    +f distance(&self, other: &Point) -> f64 {
        v dx = self.x - other.x
        v dy = self.y - other.y
        (dx * dx + dy * dy).sqrt()
    }
}

I fmt.Display ~ Point {
    f fmt(&self, fmtr: &!fmt.Formatter) -> fmt.Result {
        fmtr.write_str(&f"({self.x}, {self.y})")
    }
}
```

**Steps applied:**
1. `use std::fmt` → `u std.fmt`
2. `#[derive(...)]` → `@d(...)`
3. `pub struct` → `+S`, `pub` fields → `+field`
4. `impl Point` → `I ~ Point`
5. `pub fn` → `+f`, `fn` → `f`
6. `let` → `v`
7. `Point { x, y }` → `Point @{ x, y }`
8. `impl fmt::Display for Point` → `I fmt.Display ~ Point`
9. `&mut fmt::Formatter` → `&!fmt.Formatter`
10. `::` → `.`

---

## Worked Migration: Error Handling

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

### Redox
```redox
u std.fs
u std.io

+f read_config(path: &s) -> R[s, io.Error] / io {
    v content = fs.read_to_string(path)?
    ? content.is_empty() {
        ret Err(io.Error.new(io.ErrorKind.InvalidData, "empty config"))
    }
    Ok(content)
}
```

**Steps applied:**
1. `use` → `u`, `::` → `.`
2. `pub fn` → `+f`
3. `&str` → `&s`, `String` → `s`, `Result<String, io::Error>` → `R[s, io.Error]`
4. **Added `/ io`** — this function does file I/O
5. `let` → `v`, `if` → `?`, `return` → `ret`

---

## Worked Migration: Async with Generics

### Rust
```rust
use std::collections::HashMap;

pub async fn fetch_all<T>(urls: Vec<String>) -> Result<HashMap<String, T>, Error>
where
    T: serde::de::DeserializeOwned,
{
    let mut results = HashMap::new();
    for url in urls {
        let resp = reqwest::get(&url).await?;
        let data: T = resp.json().await?;
        results.insert(url, data);
    }
    Ok(results)
}
```

### Redox
```redox
+af fetch_all[T](urls: [s]~) -> R[{s: T}, Error] / net
    ~> T: serde.de.DeserializeOwned
{
    m results = {s: T}.new()
    @ url ~ urls {
        v resp = http.get(&url).await?
        v data: T = resp.json().await?
        results.insert(url, data)
    }
    Ok(results)
}
```

**Steps applied:**
1. `pub async fn` → `+af`
2. `<T>` → `[T]`
3. `Vec<String>` → `[s]~`, `HashMap<String, T>` → `{s: T}`, `Result<..>` → `R[..]`
4. `where T: ...` → `~> T: ...`
5. `::` → `.`
6. `let mut` → `m`, `for x in y` → `@ x ~ y`
7. **Added `/ net`** — network requests (net implies io)

---

## Worked Migration: Trait with Generic Bound

### Rust
```rust
pub trait Repository<T> {
    fn find(&self, id: u64) -> Option<T>;
    fn save(&mut self, item: T) -> Result<(), Error>;
    fn delete(&mut self, id: u64) -> Result<(), Error>;
}
```

### Redox
```redox
+T Repository[T] {
    f find(&self, id: u64) -> ?T
    f save(&!self, item: T) -> R[(), Error] / io
    f delete(&!self, id: u64) -> R[(), Error] / io
}
```

**Steps applied:**
1. `pub trait` → `+T`
2. `<T>` → `[T]`
3. `fn` → `f`
4. `Option<T>` → `?T`, `Result<(), Error>` → `R[(), Error]`
5. `&mut self` → `&!self`
6. **Added `/ io`** — save/delete are persistence operations

---

## Migration Checklist

For each Rust file being migrated:

1. [ ] Change file extension from `.rs` to `.rdx`
2. [ ] Replace all keywords (fn → f, let → v, struct → S, etc.)
3. [ ] Replace all type sugar (Vec → [T]~, Option → ?T, etc.)
4. [ ] Replace `::` paths with `.` paths
5. [ ] Replace `<T>` generics with `[T]`
6. [ ] Replace `#[...]` attributes with `@...` shortcuts
7. [ ] Replace `println!`/`format!` with `p"..."`/`f"..."`
8. [ ] Replace `true`/`false` with `1b`/`0b`
9. [ ] Replace struct literals `Foo { }` with `Foo @{ }`
10. [ ] Add effect annotations to all impure functions
11. [ ] Remove lifetime annotations
12. [ ] Remove `unsafe` blocks → use `Capability`
13. [ ] Replace raw threading with `Agent`/`Swarm` where appropriate
