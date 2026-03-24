# Redox Syntax Quick Reference

> Optimized for AI agent context windows. Minimal prose, maximum density.
> This shows **standard syntax** (default). For compact mode, add `#![syntax(compact)]`.

## Declarations

```
fn name()                       function (private)
pub fn name()                   public function
pub(crate) fn name()            crate-visible function
async fn name()                 async function (private)
pub async fn name()             public async function
const fn name()                 const function
let x = expr                    immutable binding
let mut x = expr                mutable binding
pub const NAME: T = expr        public constant
struct Name { fields }          struct (private)
pub struct Name { fields }      public struct
enum Name { variants }          enum (private)
pub enum Name { variants }      public enum
trait Name { methods }          trait (private)
pub trait Name { methods }      public trait
impl Trait for Type { }         trait implementation
impl Type { }                   inherent implementation
mod name                        module (private)
pub mod name                    public module
use path::to::Item              import
pub use path::to::Item          re-export
```

## Control Flow

```
if cond { body }                         conditional
if cond { body } else { body }           if-else
if cond { } else if cond2 { } else { }  chain
match expr { pat => body, }              pattern match
for item in iter { body }                iteration
loop { body }                            infinite loop
return expr                              early return
break                                    break loop
continue                                 skip iteration
```

## Types (same as Rust)

```
String          owned string
&str            string slice
Vec<T>          dynamic array
Option<T>       optional value
Result<T, E>    error handling
Box<T>          heap allocation
Rc<T>           reference counted
Arc<T>          atomic ref counted
HashMap<K, V>   hash map
HashSet<K>      hash set
&mut T          mutable reference
```

## Generics & Paths

```
fn foo<T>(x: T)                    generic function
fn foo<T>(x: T) where T: Clone    bounded generic
foo::<i32>()                       turbofish
std::io::File                      module path
crate::module::Item                crate-relative path
Foo { x: 1, y: 2 }                struct literal
```

## Macros & Attributes

```
println!("hello {name}")          print line
format!("hello {name}")           format string
eprintln!("error: {e}")           error print
#[derive(Debug, Clone)]           derive macro
#[inline]                         inline hint
#[test]                           test function
#[bench]                          benchmark
#[cfg(test)]                      conditional compilation
```

## Effects (Redox-unique)

```
fn pure_fn() -> i32                          // no effect = pure
fn read() -> Result<String, Error> / io      // single effect
pub async fn fetch() -> Result<String, Error> / io, net  // multiple
```

Built-in effects: `io` `net` `rng` `async` `agent` `time` `env` `process`

## Effect Hierarchy

```
net  ⊃  io        (net implies io)
agent ⊃ async     (agent implies async)
```

## Contract Annotations (Redox-unique)

```
@req condition        precondition
@ens condition        postcondition
@inv condition        invariant
@perf metric < bound  performance budget
@fx / effect          effect declaration
```

## Standard Library Modules

```
std::io         I/O operations (File, BufReader, stdin/stdout)
std::net        Networking (TcpStream, UdpSocket, http)
std::fs         Filesystem (read, write, create_dir)
std::col        Collections (vec, map, set, deque)
std::sync       Synchronization (Mutex, RwLock, channel)
std::async      Async runtime (spawn, select, timeout)
std::fmt        Formatting (Display, Debug, Formatter)
std::str        String utilities (split, trim, parse)
std::math       Math (sin, cos, sqrt, PI)
std::time       Time (Instant, Duration, SystemTime)
std::json       JSON (parse, stringify, Value)
std::env        Environment (var, args, current_dir)
std::process    Process (Command, exit, spawn)
std::agent      Agent primitives (Agent, Capability, Swarm)
std::skb        Knowledge base (Rule, Query, Proof)
std::effect     Effect types (Effect, Handler, handle)
std::spec       Specifications (pre, post, invariant)
std::test       Testing (assert, mock, bench)
```

## Canonical Examples

### Hello World
```redox
pub fn main() / io {
    println!("Hello, world!");
}
```

### Fibonacci
```redox
fn fib(n: u64) -> u64 {
    if n <= 1 { return n; }
    fib(n - 1) + fib(n - 2)
}
```

### Read File
```redox
use std::fs;

fn read_config(path: &str) -> Result<String, io::Error> / io {
    fs::read_to_string(path)
}
```

### Struct with Methods
```redox
#[derive(Debug, Clone)]
pub struct Point {
    x: f64,
    y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn distance(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}
```

### Error Handling
```redox
use std::io;
use std::json;

pub fn load_config(path: &str) -> Result<Config, Error> / io {
    let text = fs::read_to_string(path)?;
    let config = json::parse::<Config>(&text)?;
    return config;
}
```

### Agent
```redox
use std::agent::{Agent, Swarm};

pub struct Analyzer {
    data: Vec<f64>,
}

impl Agent for Analyzer {
    pub async fn execute(&mut self) -> Result<f64, Error> / agent {
        let sum: f64 = self.data.iter().sum();
        return sum / self.data.len() as f64;
    }
}
```
