# Step 3: Syntax in 5 Minutes

MAGE uses token-minimal syntax — every construct is as short as
possible so AI agents can read and write code faster. Here's the full
tour.

---

## Variables

```MAGE
v x = 42          // immutable binding (let)
m y = 0           // mutable binding (let mut)
y = 10            // reassign mutable
+v PI = 3.14159   // public constant (pub const)
```

## Functions

```MAGE
f add(a: i32, b: i32) -> i32 {       // private function
    a + b
}

+f multiply(a: f64, b: f64) -> f64 { // public function
    a * b
}

af fetch(url: &s) -> R[s, Error] / io {  // async function with effect
    // ...
}

c f max_size() -> usize { 1024 }     // const function
```

## Types at a Glance

```MAGE
// Primitives
v a: i32 = 42
v b: f64 = 3.14
v c: bool = 1b        // true
v d: char = 'x'
v e: s = "hello"      // String
v f: &s = "world"     // &str

// Collections
v nums: [i32]~ = [1, 2, 3]~           // Vec<i32>
v pair: (i32, s) = (42, "hello")       // tuple
v map: {s: i32} = {"a": 1, "b": 2}    // HashMap
v set: {i32} = {1, 2, 3}              // HashSet

// Smart pointers
v boxed: ^i32 = ^42                    // Box<i32>
v shared: $i32 = $42                   // Rc<i32>
v atomic: @i32 = @42                   // Arc<i32>

// Optional & Result
v maybe: ?i32 = 42                     // Option<i32> = Some(42)
v result: R[i32, s] = 42              // Result<i32, String> = Ok(42)
```

### Type Cheat Sheet

| MAGE       | Rust             | Description        |
| ----------- | ---------------- | ------------------ |
| `s`         | `String`         | Owned string       |
| `&s`        | `&str`           | String slice       |
| `[T]~`      | `Vec<T>`         | Growable array     |
| `?T`        | `Option<T>`      | Optional value     |
| `R[T,E]`    | `Result<T,E>`    | Result type        |
| `^T`        | `Box<T>`         | Heap pointer       |
| `$T`        | `Rc<T>`          | Reference counted  |
| `@T`        | `Arc<T>`         | Atomic ref counted |
| `{K:V}`     | `HashMap<K,V>`   | Hash map           |
| `{K}`       | `HashSet<K>`     | Hash set           |
| `&T`        | `&T`             | Shared reference   |
| `&!T`       | `&mut T`         | Mutable reference  |
| `1b` / `0b` | `true` / `false` | Booleans           |

## Structs, Enums, Traits

```MAGE
// Struct
+S Point {
    x: f64,
    y: f64,
}

// Enum
+E Shape {
    Circle(f64),
    Rect(f64, f64),
}

// Trait
+T Area {
    f area(&self) -> f64
}

// Implement trait for type
I Area ~ Shape {
    f area(&self) -> f64 {
        ? self {
            Shape.Circle(r) => 3.14159 * r * r,
            Shape.Rect(w, h) => w * h,
        }
    }
}

// Inherent impl
I ~ Point {
    +f new(x: f64, y: f64) -> Self {
        Point @{ x, y }
    }
}
```

| Token            | Rust Equivalent                |
| ---------------- | ------------------------------ |
| `+S`             | `pub struct`                   |
| `+E`             | `pub enum`                     |
| `+T`             | `pub trait`                    |
| `I Trait ~ Type` | `impl Trait for Type`          |
| `I ~ Type`       | `impl Type`                    |
| `Foo @{ ... }`   | `Foo { ... }` (struct literal) |
| `? self { ... }` | `match self { ... }`           |

## Control Flow

```MAGE
// If / else
? x > 0 {
    p"positive"
} : ? x == 0 {
    p"zero"
} : {
    p"negative"
}

// Match
? color {
    "red" => p"hot",
    "blue" => p"cool",
    _ => p"other",
}

// For loop
@ item ~ items {
    p"{item}"
}

// While loop
@ m i = 0; i < 10; i += 1 {
    p"{i}"
}

// Loop (infinite)
@ {
    ? done { break }
}
```

| Token           | Rust Equivalent          |
| --------------- | ------------------------ |
| `?`             | `if` or `match`          |
| `:`             | `else`                   |
| `? x { arms }`  | `match x { arms }`       |
| `@ item ~ iter` | `for item in iter`       |
| `@`             | `loop` / `while` / `for` |

## Modules and Imports

```MAGE
// Module declaration
+M math {
    +f sqrt(x: f64) -> f64 { /* ... */ }
}

// Import
u std.io.{Read, Write}     // use std::io::{Read, Write}
+u std.fmt.Display          // pub use std::fmt::Display

// Path separator is . not ::
v result = math.sqrt(4.0)
```

## Error Handling

```MAGE
// The ? operator works the same as Rust
f read_file(path: &s) -> R[s, io.Error] / io {
    v content = fs.read_to_string(path)?
    content
}

// No return keyword needed — last expression is the return value
// Use `ret` for early return:
f find(xs: &[i32]~, target: i32) -> ?usize {
    @ i, x ~ xs.iter().enumerate() {
        ? *x == target { ret i }
    }
    ()  // None
}
```

## Effects

Effects declare what side-effects a function performs:

```MAGE
f pure_add(a: i32, b: i32) -> i32 { a + b }        // no effects

f read_config() -> Config / io { /* ... */ }         // io effect

af fetch(url: &s) -> R[s, Error] / io, net {         // multiple effects
    // ...
}
```

The compiler tracks effects automatically — you don't need to annotate
them unless you want to document intent.

## Generics

```MAGE
// Generics use [] not <>
f first[T](xs: &[T]~) -> ?&T {
    xs.first()
}

+S Pair[A, B] {
    first: A,
    second: B,
}

// Where clauses
f print_all[T](xs: &[T]~) ~> T: Display {
    @ x ~ xs {
        p"{x}"
    }
}
```

| Token | Rust Equivalent       |
| ----- | --------------------- |
| `[T]` | `<T>` (generic param) |
| `~>`  | `where`               |

## Attributes

```MAGE
@d(Debug, Clone)           // #[derive(Debug, Clone)]
+S Config {
    name: s,
    port: u16,
}

@test                       // #[test]
f test_add() {
    assert(add(2, 3) == 5)
}

@i                          // #[inline]
f fast_op(x: i32) -> i32 { x * 2 }

@cfg(target_os = "linux")   // #[cfg(...)]
f linux_only() { /* ... */ }
```

---

**That's all the syntax you need to start writing MAGE.**

The compiler handles safety rules (ownership, borrowing, lifetimes)
through the SKB — you never write lifetime annotations or borrow
markers.

**[Next: Build, Run, Test →](04-build-run-test.md)**
