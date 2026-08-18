# MAGE Syntax Quick Reference

> Optimized for AI agent context windows. Minimal prose, maximum density.
> Human syntax on the left, agent-mode sigil on the right where one exists.

**Every row below was checked against the compiler.** The previous version of
this file was a Rust cheat sheet: it listed `let`, `async fn`, `mod name`,
`use path::to::Item`, `foo::<i32>()`, `std::io::File`, `Foo { x: 1 }` as a
struct literal, `println!`, `#[derive(Debug)]`, four `std::` module tables and
an effect hierarchy. None of those parse or exist. A quick reference is the
densest thing an agent reads, so a false row here costs more than anywhere
else in the documentation.

## Declarations

```
fn name()                       function (private)            f
pub fn name()                   public function               +f
async name()                    async function — NOT `async fn`   af
struct Name { fields }          struct                        S / +S
enum Name { variants }          enum                          E / +E
trait Name { methods }          trait                         T / +T
impl Trait for Type { }         trait implementation          I
extend Type { }                 methods on a type             xd
data Name(field: T)             record                        D
data Name = A(T) | B            sum                           D
effect Name { ops }             effect declaration            fx
agent Name { capabilities: [] } agent role
swarm Name { agent: A }         swarm
kb Name { fact …; rule … }      knowledge base
net Name { layer …; forward }   neural net
val x = expr                    immutable binding             v
var x = expr                    mutable binding               m
pub const NAME: T = expr;       constant (`;` required)       C
Z NAME: T = expr;               static — agent mode only      Z
type Id = i32;                  type alias
```

**Not in the language:** `let` (the parser rejects it by name), `mut x`,
`pub(crate)`, `const fn`, `extern`, `unsafe`, `mod`/`use` as anything that
imports — see *One namespace* below. The human spelling `static N: T = v;`
does not parse either, but the agent-mode `Z` above does — the only
declaration in this table with no human form.

## Control Flow

```
if cond { } else { }            conditional                   ? c { } : { }
match expr { pat => body, }     pattern match                 ?= e { }
for item in iter { }            iteration                     @ x in xs { }
while cond { }                  conditional loop              @w
loop { }                        infinite loop                 @@
return expr                     early return                  ret
break                           break loop                    !
continue                        skip iteration
guard cond else { return … }    early exit, else must diverge  gd
defer expr                      run at scope exit             df
??                              todo / unimplemented
```

## Types

```
str             string                (NOT `String`/`&str`)
i32 i64 u32 usize f64 bool
[T]~            slice                 (`Vec<T>` also parses)
{K: V}          map                   (`HashMap<K, V>` also parses)
?T              optional              (`Option<T>` also parses)
R[T, E]         result                (`Result<T, E>` also parses)
T or E          result, as a union
^T              box                   (`Box<T>` also parses)
(A, B)          tuple
&T              reference             (`&mut T` does NOT parse)
fn(A) -> B      function type — no effect annotation is allowed here
```

## Generics

```
fn name[T](x: T) -> T           generic function   (`<T>` also parses)
fn name[T](x: T) -> T where …   bound
```

No turbofish: `id::<i32>(1)` is a parse error. Each call site instantiates its
own copy of the type variables, so `id(1)` and `id("ab")` coexist without one.

## Literals

```
1b / 0b                         true / false — `true` and `false` are NOT names
"text"                          string
f"hi {x}"                       interpolated string
@Name { field: value }          struct literal
{"k": v}                        map literal      — bare `Name { … }` is a MAP
[1, 2, 3]                       list literal     — `vec![…]` is a parse error
Some(x) None Ok(v) Err(e)       sum constructors
```

## Attributes and contracts

```
@test                           test function      (`#[test]` does NOT parse)
sp name { @req(c) @ens(c) @fx() }   contracts, in a block sharing the name
```

There are no derive macros, no `#[cfg(test)]`, and **no macros at all**:
`println!(…)`, `format!(…)`, `vec![…]` and `assert_eq!(…)` are parse errors
that name themselves.

## Effects

```
fn pure_fn() -> i32                    no annotation = pure
fn read(p: str) -> str / fs            single effect
pub fn fetch(u: str) -> str / io, net  multiple, comma-separated
handle { … } with E { op(x) => v }     discharge an effect for one block
```

The 17 built-in kinds:

```
io  fs  net  env  time  rng  proc  alloc  panic  ffi
async  agent  llm  gpu  npu  evolve  learn
```

**There is no effect hierarchy.** `/ net` does not cover an inferred `io`, and
`/ agent` does not cover `async`. Declare every effect the function performs.
The rule the checker enforces is `inferred ⊆ declared`: a declared set is an
upper bound, so over-declaring passes and under-declaring fails.

A `pub` function must declare what it performs; a private one infers silently
and its effects still surface in every public caller that reaches it.

## One namespace

There is no module system. Every function, type, effect and agent in the
compilation unit shares one flat namespace, and two things are in scope
everywhere:

```
map filter fold reduce sum len count sort reverse zip freq first last any all
find take range keys values flatten group scan contains split join chars words
lines upper lower
```

The 20 capability namespaces:

```
io  fs  net  env  time  rng  llm  gpu  agent   ← same name as the effect
http  mem  log  swarm                          ← net, alloc, io, agent
os  sys  process  tools                        ← all four perform `proc`
json  kb  db                                   ← perform nothing (deliberate)
```

`use` parses, for source compatibility, and brings nothing into scope — the
checker warns. `::` paths are a parse error. There is no `std::io`, no
`std::agent`, no `std::skb`.

## Names that are keywords

Single letters are the agent-mode declaration keywords, so they cannot be
identifiers: `f v m u C S E T I M U D Y Z`. `|u| …` and `f(x)` as a *variable*
are parse errors — spell closure parameters and function-typed parameters out.
`pipeline`, `select`, `query`, `net`, `layer`, `param`, `train`, `grad`,
`policy` and `reward` are keywords too.

## Canonical Examples

### Hello World
```MAGE
pub fn main() -> i32 / io {
    println("Hello, world!")
    0
}
```

### Fibonacci
```MAGE
fn fib(n: i32) -> i32 {
    if n <= 1 { return n }
    fib(n - 1) + fib(n - 2)
}
```

### Read File
```MAGE
+f read_config(path: str) -> R[str, str] / fs {
    guard len(path) > 0 else { ret Err("empty path") }
    Ok(fs.read_to_string(path))
}
```

### Struct with Methods
```MAGE
+S Point { x: f64, y: f64 }

I Point {
    +f new(x: f64, y: f64) -> Point { @Point { x: x, y: y } }

    +f dist_sq(self, other: Point) -> f64 {
        v dx = self.x - other.x
        v dy = self.y - other.y
        dx * dx + dy * dy
    }
}
```

### Error Handling
```MAGE
+S Config { port: i32 }

+f load_config(raw: i32) -> R[Config, str] / fs {
    guard raw > 0 else { ret Err("port must be positive") }
    Ok(@Config { port: raw })
}
```

### Agent
```MAGE
agent Analyzer {
    capabilities: [agent]
}

+f analyze(data: [f64]~) -> f64 / agent {
    agent.spawn("analyze")
    fold(data, 0.0, |acc, x| acc + x)
}
```
