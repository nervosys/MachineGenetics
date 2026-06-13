# MAGE Language Specification — Draft v0.1

> Status: Working draft generated from the prototype compiler.

---

## 1. Overview

**MAGE** is a systems programming language designed for agentic compilation. It
combines Rust's performance and safety model with contract-first design, an
algebraic effect system, token-minimal syntax, and first-class support for
multi-agent development workflows.

### 1.1 Design Principles

1. **Agent-first** — every construct is queryable, costable, and
   machine-readable.
2. **Contract-driven** — preconditions (`@req`), postconditions (`@ens`),
   invariants (`@inv`) are part of the type surface.
3. **Effect-tracked** — side effects are declared (`@fx`) and propagated through
   the type system.
4. **Token-minimal** — canonical syntax minimises token count for efficient LLM
   processing.
5. **Legacy-compatible** — a bidirectional transpiler maps MAGE ↔ Rust.

---

## 2. Lexical Structure

### 2.1 Character Set

Source files are UTF-8 encoded. Identifiers use `[a-zA-Z_][a-zA-Z0-9_]*`.

### 2.2 Keywords

#### Core Item Keywords

| Sigil | Meaning                | Rust Equivalent     |
| ----- | ---------------------- | ------------------- |
| `f`   | Function               | `fn`                |
| `+f`  | Public function        | `pub fn`            |
| `af`  | Async function         | `async fn`          |
| `+af` | Public async function  | `pub async fn`      |
| `uf`  | Unsafe function        | `unsafe fn`         |
| `+uf` | Public unsafe function | `pub unsafe fn`     |
| `S`   | Struct                 | `struct`            |
| `+S`  | Public struct          | `pub struct`        |
| `E`   | Enum                   | `enum`              |
| `+E`  | Public enum            | `pub enum`          |
| `T`   | Trait                  | `trait`             |
| `+T`  | Public trait           | `pub trait`         |
| `I`   | Impl block             | `impl`              |
| `M`   | Module                 | `mod`               |
| `+M`  | Public module          | `pub mod`           |
| `~f`  | Crate-public function  | `pub(crate) fn`     |
| `~S`  | Crate-public struct    | `pub(crate) struct` |

#### Visibility Prefixes

| Prefix | Meaning      | Rust Equivalent |
| ------ | ------------ | --------------- |
| `+`    | Public       | `pub`           |
| `~`    | Crate-public | `pub(crate)`    |
| (none) | Private      | (default)       |

#### Control Flow

| Sigil | Meaning       | Rust Equivalent  |
| ----- | ------------- | ---------------- |
| `?`   | Match         | `match`          |
| `?:`  | If / if-else  | `if` / `if-else` |
| `??`  | If-let        | `if let`         |
| `@`   | For-in loop   | `for`            |
| `@@`  | Infinite loop | `loop`           |
| `@w`  | While loop    | `while`          |
| `@wl` | While-let     | `while let`      |
| `!`   | Break         | `break`          |
| `>>`  | Continue      | `continue`       |
| `ret` | Return        | `return`         |

#### Contract Annotations

| Sigil   | Meaning            | Description                      |
| ------- | ------------------ | -------------------------------- |
| `@req`  | Precondition       | Must hold on function entry      |
| `@ens`  | Postcondition      | Must hold on function exit       |
| `@inv`  | Invariant          | Must hold across struct lifetime |
| `@fx`   | Effect declaration | Side effects of a function       |
| `@perf` | Performance hint   | Performance annotation           |

### 2.3 Operators and Punctuation

| Token       | Meaning                  |
| ----------- | ------------------------ |
| `->`        | Return type              |
| `=>`        | Match arm                |
| `::`        | Path separator           |
| `.`         | Field access             |
| `:`         | Type annotation          |
| `=`         | Assignment / binding     |
| `==`        | Equality                 |
| `!=`        | Inequality               |
| `<` `>`     | Comparison / type args   |
| `<=` `>=`   | Comparison               |
| `+` `-`     | Arithmetic               |
| `*` `/` `%` | Arithmetic               |
| `&&`        | Logical AND              |
| `\|\|`      | Logical OR               |
| `&`         | Borrow / reference       |
| `^`         | Box (smart pointer)      |
| `$`         | Rc (reference counted)   |
| `@T`        | Arc (atomic ref counted) |
| `#`         | Attribute / derive       |

### 2.4 Type Sigils

| Sigil    | Rust Equivalent | Description            |
| -------- | --------------- | ---------------------- |
| `[T]~`   | `Vec<T>`        | Dynamic array          |
| `[T; N]` | `[T; N]`        | Fixed array            |
| `{K: V}` | `HashMap<K, V>` | Hash map               |
| `{K}`    | `HashSet<K>`    | Hash set               |
| `^T`     | `Box<T>`        | Heap pointer           |
| `$T`     | `Rc<T>`         | Reference count        |
| `@T`     | `Arc<T>`        | Atomic reference count |
| `&T`     | `&T`            | Shared reference       |
| `&m T`   | `&mut T`        | Mutable reference      |
| `T?`     | `Option<T>`     | Optional               |
| `T!E`    | `Result<T, E>`  | Result                 |
| `s`      | `String`        | Owned string           |
| `&s`     | `&str`          | String slice           |

### 2.5 Literal Formats

| Syntax         | Meaning           |
| -------------- | ----------------- |
| `42`           | Integer literal   |
| `3.14`         | Float literal     |
| `"hello"`      | String literal    |
| `f"x = {x}"`   | Format string     |
| `b"bytes"`     | Byte string       |
| `true`/`false` | Boolean           |
| `'c'`          | Character literal |

### 2.6 Comments

```
// Line comment
/// Doc comment
/* Block comment */
```

---

## 3. Items

### 3.1 Functions

```
f name(param: Type) -> RetType {
    body
}

+af async_func(x: i32) -> i32 {
    x + 1
}
```

### 3.2 Functions with Contracts

```
f divide(x: f64, y: f64) -> f64
    @req y != 0.0 "divisor must be non-zero"
    @ens result == x / y
    @fx pure
{
    x / y
}
```

### 3.3 Structs

```
+S Point {
    x: f64,
    y: f64,
}
    @inv x.is_finite() && y.is_finite()
```

### 3.4 Enums

```
+E Shape {
    Circle { radius: f64 },
    Rect { w: f64, h: f64 },
}
```

### 3.5 Traits

```
+T Drawable {
    f draw(&self);
    f bounds(&self) -> Rect;
}
```

### 3.6 Impl Blocks

```
I Point {
    +f new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    f distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}
```

### 3.7 Modules

```
+M geometry;

M internal {
    f helper() -> i32 { 42 }
}
```

---

## 4. Type System

### 4.1 Primitive Types

`i8`, `i16`, `i32`, `i64`, `i128`, `isize`,
`u8`, `u16`, `u32`, `u64`, `u128`, `usize`,
`f32`, `f64`, `bool`, `char`, `()`.

### 4.2 Composite Types

- **Tuples**: `(A, B, C)`
- **Arrays**: `[T; N]`
- **Slices**: `[T]`
- **References**: `&T`, `&m T`
- **Pointers**: `^T` (Box), `$T` (Rc), `@T` (Arc)

### 4.3 Generic Types

```
f identity<T>(x: T) -> T { x }

S Pair<A, B> { first: A, second: B }
```

### 4.4 Trait Bounds

```
f stringify<T: Display>(x: T) -> s {
    f"{x}"
}

f process<T>(x: T) -> T
    where T: Clone + Debug
{
    x.clone()
}
```

### 4.5 Lifetime Annotations

```
f longest<'a>(x: &'a s, y: &'a s) -> &'a s {
    ?: x.len() >= y.len() { x } _ { y }
}
```

---

## 5. Control Flow

### 5.1 Conditionals

```
// If-else
?: condition {
    a
} _ {
    b
}

// If-let
?? Some(x) = opt {
    use(x)
}
```

### 5.2 Pattern Matching

```
? value {
    1 => "one",
    2 | 3 => "two or three",
    n ?: n > 10 => "big",
    _ => "other",
}
```

### 5.3 Loops

```
// For-in
@ item in collection {
    process(item)
}

// Infinite loop
@@ {
    ?: done { ! }
}

// While
@w condition {
    work()
}

// While-let
@wl Some(x) = iter.next() {
    process(x)
}
```

---

## 6. Effect System

### 6.1 Effect Declaration

```
f read_file(path: &s) -> s!IoError
    @fx io, fs
{
    std.fs.read_to_string(path)?
}
```

### 6.2 Effect Propagation

Effects propagate through the call graph. A function that calls an effectful
function inherits its effects unless explicitly handled.

### 6.3 Effect Kinds

| Effect             | Description                |
| ------------------ | -------------------------- |
| `pure`             | No side effects            |
| `io`               | General I/O                |
| `fs`               | File system access         |
| `net`              | Network access             |
| `mem`              | Memory allocation          |
| `panic`            | May panic                  |
| `unsafe`           | Unsafe operations          |
| `async`            | Asynchronous execution     |
| `nondeterministic` | Non-deterministic behavior |

---

## 7. Contract System

### 7.1 Preconditions (`@req`)

Specify conditions that must hold at function entry:

```
f sqrt(x: f64) -> f64
    @req x >= 0.0 "input must be non-negative"
{
    x.sqrt()
}
```

### 7.2 Postconditions (`@ens`)

Specify conditions that must hold at function exit:

```
f abs(x: i32) -> i32
    @ens result >= 0
{
    ?: x < 0 { -x } _ { x }
}
```

### 7.3 Invariants (`@inv`)

Specify conditions that must hold across the lifetime of a struct:

```
S NonEmpty<T> {
    items: [T]~,
}
    @inv !self.items.is_empty()
```

---

## 8. Smart Pointers and Ownership

### 8.1 Box (`^`)

```
let x: ^i32 = ^42;
```

### 8.2 Rc (`$`)

```
let shared: $Node = $.new(node);
```

### 8.3 Arc (`@`)

```
let atomic: @Mutex<i32> = @.new(Mutex.new(0));
```

---

## 9. String Formatting

```
let name = "world";
let msg = f"Hello, {name}!";     // format string
p"Result: {value}";               // println!
```

---

## 10. Error Handling

### 10.1 Result Type

```
f parse(input: &s) -> i32!ParseError {
    input.parse()?
}
```

### 10.2 Option Type

```
f find(items: &[i32], target: i32) -> i32? {
    @ item in items {
        ?: *item == target { ret Some(*item) }
    }
    None
}
```

---

## 11. Concurrency

### 11.1 Async Functions

```
+af fetch(url: &s) -> s!NetError
    @fx net, async
{
    client.get(url).await?.text().await?
}
```

### 11.2 Spawn

```
let handle = spawn(af || {
    heavy_computation().await
});
```

---

## 12. Agentic Primitives

### 12.1 Swarm Bus

Agents communicate via a typed publish/subscribe bus:

```
let bus = SwarmBus.new();
bus.publish("task.complete", payload);
bus.subscribe("task.complete", |msg| { process(msg) });
```

### 12.2 Leases

Temporary, revocable ownership grants:

```
let lease = Lease.acquire(resource, Duration.secs(30));
// Use resource through lease
lease.release();
```

### 12.3 Cost Oracle

```
let cost = cost.query("Vec::push", "x86_64", Release);
// cost.cycles, cost.memory_bytes, cost.latency_ns
```

### 12.4 Capability Sandbox

```
let sandbox = SandboxManager.create("agent-1", limits);
sandbox.grant(CapabilityToken.restricted("fs.read"));
sandbox.check_access("agent-1", "fs.read"); // true
```

---

## 13. Module System

### 13.1 Module Declaration

```
M utils;          // file-based module (utils.rs)
+M public_api;    // public module
```

### 13.2 Use Declarations

```
use std.collections.HashMap;
use crate.utils.{helper, Config};
```

### 13.3 Path Separator

MAGE uses `.` as the path separator (instead of `::`):

```
std.fs.read_to_string(path)
```

---

## 14. Attributes and Derives

```
#[derive(Debug, Clone)]
+S Config {
    name: s,
    value: i32,
}

#[cfg(test)]
M tests {
    #[test]
    f it_works() {
        assert_eq!(2 + 2, 4);
    }
}
```

---

## 15. Project Manifest (`MAGE.toml`)

```toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = "1.0"

[grammar_extensions]
namespace = "custom"

[agent]
swarm_size = 4
token_budget = 8192
```

---

## 16. Legacy Compatibility

MAGE provides bidirectional translation with Rust:

- **MAGE → Rust**: `MAGE transpile --to-rust src/main.mg`
- **Rust → MAGE**: `MAGE transpile --from-rust src/main.rs`
- **Legacy mode**: Supports `fn`, `struct`, `enum`, `impl` as aliases.

All valid Rust programs have an equivalent MAGE representation, and vice
versa. The transpiler preserves contracts, effects, and performance annotations
as attributes in the Rust output.

---

## Appendix A: Grammar (EBNF Sketch)

```ebnf
program        = { item } ;
item           = [ visibility ] item_kind ;
visibility     = '+' | '~' ;
item_kind      = function | struct_def | enum_def | trait_def
               | impl_block | module | use_decl ;

function       = 'f' | 'af' | 'uf' , IDENT , '(' , params , ')' ,
                 [ '->' , type ] , { contract } , block ;
params         = [ param , { ',' , param } ] ;
param          = IDENT , ':' , type ;
contract       = '@req' , expr , [ STRING ]
               | '@ens' , expr
               | '@inv' , expr
               | '@fx' , ident_list
               | '@perf' , perf_hint ;

struct_def     = 'S' , IDENT , [ generics ] , '{' , fields , '}' ,
                 { '@inv' , expr } ;
enum_def       = 'E' , IDENT , [ generics ] ,
                 '{' , variant , { ',' , variant } , '}' ;
trait_def      = 'T' , IDENT , [ generics ] ,
                 '{' , { trait_item } , '}' ;
impl_block     = 'I' , [ type , 'for' ] , type ,
                 '{' , { function } , '}' ;

type           = primitive | path_type | ref_type | ptr_type
               | array_type | tuple_type | fn_type ;
ref_type       = '&' , [ 'm' ] , type ;
ptr_type       = '^' , type | '$' , type | '@' , type ;
array_type     = '[' , type , ']~'         (* Vec *)
               | '[' , type , ';' , expr , ']' ;  (* fixed *)

block          = '{' , { statement } , [ expr ] , '}' ;
statement      = let_stmt | expr_stmt | item ;
let_stmt       = 'let' , [ 'mut' ] , pattern , [ ':' , type ] ,
                 '=' , expr , ';' ;

expr           = literal | path | call | binary | unary | block
               | if_expr | match_expr | loop_expr | for_expr
               | closure | await_expr | return_expr ;

if_expr        = '?:' , expr , block , [ '_' , block ] ;
match_expr     = '?' , expr , '{' , { match_arm } , '}' ;
for_expr       = '@' , pattern , 'in' , expr , block ;
loop_expr      = '@@' , block ;
```

---

## Appendix B: Reserved Words

`f`, `af`, `uf`, `S`, `E`, `T`, `I`, `M`, `let`, `mut`, `ret`, `self`,
`Self`, `super`, `crate`, `true`, `false`, `as`, `in`, `where`, `for`,
`loop`, `break`, `continue`, `if`, `else`, `match`, `while`, `async`,
`await`, `unsafe`, `use`, `type`, `const`, `static`, `extern`, `dyn`,
`move`, `ref`, `pub`, `mod`, `fn`, `struct`, `enum`, `trait`, `impl`.

---

*This specification is derived from the MAGE prototype compiler — lexer,
parser, AST, HIR, MLIR, type system, effect system, and contract system.
Semantic details may evolve as the compiler matures.*
