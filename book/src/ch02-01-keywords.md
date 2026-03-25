# Keywords & Declarations

MechGen uses familiar C-family keywords in its default **Human Syntax** mode.
If you know Rust, you already know MechGen's keywords.

> **agent mode:** MechGen also supports a token-minimal agent syntax activated
> with `#![syntax(agent)]`. See the [appendix](appendix-cheatsheet.md) for the
> full mapping.

## Declaration Keywords

| MechGen (Human) | Rust Equivalent | Meaning           |
| ---------------- | --------------- | ----------------- |
| `fn`             | `fn`            | Function          |
| `let`            | `let`           | Immutable binding |
| `let mut`        | `let mut`       | Mutable binding   |
| `struct`         | `struct`        | Struct            |
| `enum`           | `enum`          | Enum              |
| `trait`          | `trait`         | Trait             |
| `impl`           | `impl`          | Implementation    |
| `mod`            | `mod`           | Module            |
| `use`            | `use`           | Import            |
| `const`          | `const`         | Constant          |

## Visibility Modifiers

Visibility uses the same keywords as Rust:

```mg
pub fn public_fn() { }         // pub fn
pub(crate) fn crate_fn() { }   // pub(crate) fn
fn private_fn() { }            // fn (private)

pub struct PublicStruct { }
pub trait PublicTrait { }
pub enum PublicEnum { }
pub const PUBLIC_CONST: i32 = 42;
```

## Struct declarations

```mg
// A simple struct
struct Point {
    x: f64,
    y: f64,
}

// A public struct with generic
pub struct Pair<A, B> {
    first: A,
    second: B,
}

// A unit struct
struct Marker;

// A tuple struct
struct Color(u8, u8, u8);
```

## Enum declarations

```mg
pub enum Shape {
    Circle(f64),          // radius
    Rectangle(f64, f64),  // width, height
    Point,                // unit variant
}

// An enum with methods
pub enum Option<T> {
    Some(T),
    None,
}

impl<T> Option<T> {
    pub fn is_some(&self) -> bool {
        match self {
            Some(_) => true,
            None => false,
        }
    }
}
```

## Trait declarations

```mg
pub trait Display {
    pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError>;
}

// Trait with default method
pub trait Greet {
    pub fn name(&self) -> &str;

    pub fn greet(&self) -> String {
        format!("Hello, {}!", self.name())
    }
}
```

## Impl blocks

```mg
// Inherent impl
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

// Trait impl
impl Display for Point {
    pub fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        write!(f, "({}, {})", self.x, self.y)
    }
}
```

The `impl Display for Point` syntax is identical to Rust.

## Attributes

```mg
#[derive(Clone, Debug)]          // #[derive(Clone, Debug)]
#[inline]                        // #[inline]
#[inline(always)]                // #[inline(always)]
#[cfg(target_os = "linux")]      // #[cfg(...)]
#[test]                          // #[test]
```

## Boolean literals

```mg
let yes = true;
let no = false;
```
