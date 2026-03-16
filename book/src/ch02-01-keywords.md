# Keywords & Declarations

Redox replaces Rust's verbose keywords with single-character (or two-character)
forms. This is not abbreviation for its own sake — it is a deliberate design
choice for **token efficiency** and **parsing simplicity**.

## Declaration Keywords

| Redox | Rust Equivalent | Meaning           |
| ----- | --------------- | ----------------- |
| `f`   | `fn`            | Function          |
| `v`   | `let`           | Immutable binding |
| `m`   | `let mut`       | Mutable binding   |
| `S`   | `struct`        | Struct            |
| `E`   | `enum`          | Enum              |
| `T`   | `trait`         | Trait             |
| `I`   | `impl`          | Implementation    |
| `M`   | `mod`           | Module            |
| `u`   | `use`           | Import            |
| `c`   | `const`         | Constant          |

## Visibility Modifiers

Prefix a declaration keyword with `+` for public, `~` for crate-visible, or
nothing for private:

```rdx
+f public_fn() { }      // pub fn
~f crate_fn() { }       // pub(crate) fn
f private_fn() { }      // fn (private)

+S PublicStruct { }     // pub struct
+T PublicTrait { }      // pub trait
+E PublicEnum { }       // pub enum
+v PUBLIC_CONST = 42    // pub const (using v for const binding)
```

## Struct declarations

```rdx
// A simple struct
S Point {
    x: f64,
    y: f64,
}

// A public struct with generic
+S Pair[A, B] {
    first: A,
    second: B,
}

// A unit struct
S Marker;

// A tuple struct
S Color(u8, u8, u8);
```

## Enum declarations

```rdx
+E Shape {
    Circle(f64),          // radius
    Rectangle(f64, f64),  // width, height
    Point,                // unit variant
}

// An enum with methods
+E Option[T] {
    Some(T),
    None,
}

I Option[T] {
    +f is_some(&self) -> bool {
        ? self {
            Some(_) => 1b,
            None => 0b,
        }
    }
}
```

## Trait declarations

```rdx
+T Display {
    +f fmt(&self, f: &!Formatter) -> R[(), FmtError];
}

// Trait with default method
+T Greet {
    +f name(&self) -> &s;

    +f greet(&self) -> s {
        f"Hello, {self.name()}!"
    }
}
```

## Impl blocks

```rdx
// Inherent impl
I Point {
    +f new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    +f distance(&self, other: &Point) -> f64 {
        v dx = self.x - other.x
        v dy = self.y - other.y
        (dx * dx + dy * dy).sqrt()
    }
}

// Trait impl
I Display ~ Point {
    +f fmt(&self, f: &!Formatter) -> R[(), FmtError] {
        write!(f, "({}, {})", self.x, self.y)
    }
}
```

The `~` in `I Display ~ Point` reads as "impl Display *for* Point".

## Attributes

```rdx
@d(Clone, Debug)          // #[derive(Clone, Debug)]
@i                        // #[inline]
@i(always)                // #[inline(always)]
@cfg(target_os = "linux") // #[cfg(...)]
@test                     // #[test]
```

## Boolean literals

```rdx
v yes = 1b    // true
v no = 0b     // false
```

`1b` and `0b` are single tokens — one bit, true or false.
