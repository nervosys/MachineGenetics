# Step 3: Syntax in 5 Minutes

MAGE uses token-minimal syntax — every construct is as short as possible, so an
agent can read and write more code per token. This is the agent-mode tour; the
human-mode spellings (`fn`, `val`, `struct`, `match`) are in
[the syntax quick reference](../agent-guide/syntax-quick-ref.md).

**Every block below was verified with `mage-parse --check`.** The previous
version of this page was not: it taught `I Area ~ Shape`, `Point @{ x, y }`,
`@ item ~ items`, `[1, 2, 3]~`, `{1, 2, 3}`, `+v PI`, `c f`, `+M math` with
imports, and — worst — "the compiler tracks effects automatically, you don't
need to annotate them". The opposite is true, and it is the one rule the
language is built around.

---

## Variables

```MAGE
+f main() -> i32 / io {
    v x = 42          // immutable binding (`val` in human mode)
    m y = 0           // mutable binding (`var`)
    y = 10            // reassign
    p"{x} {y}"
    x + y
}
```

## Functions

```MAGE
f add(a: i32, b: i32) -> i32 {          // private function
    a + b
}

+f multiply(a: f64, b: f64) -> f64 {    // public function
    a * b
}

// `af` is an async function. `/ net` is the effect it performs — a public
// function must declare every one.
+af fetch(url: s) -> R[s, s] / net {
    Ok(net.connect(url))
}
```

## Types at a Glance

```MAGE
+f main() -> i32 {
    // Primitives
    v a: i32 = 42
    v b: f64 = 3.14
    v c: bool = 1b        // `true` is not a name; `1b` / `0b` are the literals
    v d: s = "hello"      // the string type — there is one

    // Collections
    v nums: [i32]~ = [1, 2, 3]
    v pair: (i32, s) = (42, "hello")
    v lookup: {s: i32} = {"a": 1, "b": 2}

    // Optional and result
    v maybe: ?i32 = Some(42)
    v result: R[i32, s] = Ok(42)

    a + (len(nums) as i32)
}
```

### Type Cheat Sheet

| MAGE | Rust | Notes |
|---|---|---|
| `s` | `String` / `&str` | One string type. `&s` is a *reference to* it, and a `s` parameter will not take one. |
| `[T]~` | `Vec<T>` | The literal is `[1, 2, 3]` — no trailing `~`. |
| `?T` | `Option<T>` | Built with `Some(x)` / `None`. |
| `R[T,E]` | `Result<T,E>` | Built with `Ok(v)` / `Err(e)`; `T or E` is the same type. |
| `^T` | `Box<T>` | |
| `$T` | `Rc<T>` | |
| `@T` | `Arc<T>` | |
| `{K:V}` | `HashMap<K,V>` | Literal: `{"a": 1}`. |
| `{K}` | `HashSet<K>` | Type only — **there is no set literal**; `{1, 2, 3}` is a parse error. |
| `&T` | `&T` | |
| `&!T` | `&mut T` | |
| `1b` / `0b` | `true` / `false` | The words `true` and `false` are **not** in scope. |

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
    f area(self) -> f64;
}

// Implement a trait for a type — `for`, not `~`.
I Area for Shape {
    +f area(self) -> f64 {
        ?= self {
            Circle(r) => 3.14159 * r * r,
            Rect(w, h) => w * h,
        }
    }
}

// Inherent methods — `I Type`, or `xd Type` (`extend`).
I Point {
    +f norm2(self) -> f64 { self.x * self.x + self.y * self.y }
}

+f main() -> f64 {
    v p = @Point { x: 3.0, y: 4.0 }   // struct literal: `@Name { … }`
    p.norm2() + Rect(2.0, 3.0).area()
}
```

| Token | Rust Equivalent | |
|---|---|---|
| `+S` `+E` `+T` | `pub struct` / `pub enum` / `pub trait` | |
| `I Trait for Type` | `impl Trait for Type` | `I Trait ~ Type` does **not** parse. |
| `I Type` | `impl Type` | `I ~ Type` does **not** parse. |
| `@Name { … }` | `Name { … }` | Struct literal. `Name @{ … }` does **not** parse, and bare `Name { … }` is a *map*. |
| `?= x { … }` | `match x { … }` | |

## Control Flow

```MAGE
+f describe(x: i32, colour: s) -> s / io {
    // If / else
    ? x > 0 {
        p"positive"
    } : ? x == 0 {
        p"zero"
    } : {
        p"negative"
    }

    // Match
    v mood = ?= colour {
        "red" => "hot",
        "blue" => "cool",
        _ => "other",
    }

    // For loop — `in`, not `~` or `:`
    @ item in [1, 2, 3] {
        p"{item}"
    }

    // While
    m i = 0
    @w i < 3 {
        i += 1
    }

    // Infinite loop, and `!` is break
    @@ {
        ? i > 0 { ! }
    }

    mood
}
```

| Token | Rust Equivalent | |
|---|---|---|
| `?` … `:` | `if` … `else` | |
| `?= x { arms }` | `match x { arms }` | |
| `@ item in iter` | `for item in iter` | The separator is `in`. `~` and `:` do not parse. |
| `@w cond` | `while cond` | |
| `@@` | `loop` | |
| `!` | `break` | |

## Modules and Imports

```MAGE
// There is no module system and nothing to import: every declaration in the
// file shares one namespace, and the standard vocabulary and capability
// namespaces are in scope everywhere.
//
// `u std.io` parses and brings in nothing. The checker warns; do not write it.

f sqrt_approx(x: f64) -> f64 { x / 2.0 }

+f main() -> f64 {
    sqrt_approx(4.0)
}
```

There is no module system. `+M name { … }` parses as an item and resolves
nothing: a function declared inside is not reachable by any path, so the block
buys nothing. `u std.io.{Read, Write}` parses and imports nothing — the checker
warns. `::` is a parse error everywhere.

## Error Handling

```MAGE
// `?` propagates an error, as in Rust.
f read_file(path: s) -> R[s, s] / fs {
    v content = fs.read_to_string(path)
    guard len(content) > 0 else { ret Err("empty") }
    Ok(content)
}

// The last expression is the value; `ret` is an early return.
f find_at(xs: [i32]~, target: i32) -> ?i32 {
    @ x in xs {
        ? x == target { ret Some(x) }
    }
    None
}
```

## Effects

**The compiler infers what a function performs and requires a public function
to declare it.** That is the capability gate, and it is checked before anything
runs — `inferred ⊆ declared`, so over-declaring passes and under-declaring
fails. There is no hierarchy: `/ net` does not cover an inferred `io`, and
`/ agent` does not cover `async`.

```MAGE
+S Config { name: s, port: i32 }

// No annotation = pure.
f pure_add(a: i32, b: i32) -> i32 { a + b }

// Reading a file is `fs`; printing is `io`. Neither implies the other, and a
// public function must declare **every** effect it performs — the checker
// infers what the body reaches and requires the annotation to cover it.
+f load(path: s) -> Config / fs, io {
    v raw = fs.read_to_string(path)
    p"loaded {len(raw)} bytes"
    @Config { name: path, port: 8080 }
}

+af fetch(url: s) -> R[s, s] / net {
    Ok(net.connect(url))
}
```

The 17 built-in kinds:

```
io  fs  net  env  time  rng  proc  alloc  panic  ffi
async  agent  llm  gpu  npu  evolve  learn
```

Declare your own with `fx Name { … }` (`effect` in human mode), and discharge
one for a block with `handle { … } with Name { op(x) => value }`.

## Generics

```MAGE
// Generic parameters go in `[]`, not `<>`.
f first_or[T](xs: [T]~, fallback: T) -> T {
    ?= first(xs) {
        Some(x) => x,
        None => fallback,
    }
}

+S Pair[A, B] {
    first: A,
    second: B,
}

// `~>` is `where`.
f count_all[T](xs: [T]~) -> i32 ~> T: Display {
    len(xs) as i32
}
```

| Token | Rust Equivalent |
|---|---|
| `[T]` | `<T>` (generic parameter) |
| `~>` | `where` |

## Attributes

```MAGE
@d(Debug, Clone)            // #[derive(Debug, Clone)]
+S Config {
    name: s,
    port: i32,
}

f add(a: i32, b: i32) -> i32 { a + b }

// A test is a function whose value the runner checks. There is no `assert!`.
@test
f test_add() -> bool {
    add(2, 3) == 5
}

@i                          // inline hint
f fast_op(x: i32) -> i32 { x * 2 }
```

`@d(…)`, `@test`, `@i` and `@cfg(…)` are the attribute forms. Rust's `#[…]`
does not parse, and there are no macros: `assert!`, `println!` and `vec!` are
parse errors that name themselves.

---

**That is the syntax.** Two things it does not have that a Rust reader will
reach for: macros, and imports.

**[Next: Build, Run, Test →](04-build-run-test.md)**
