# Redox Syntax Quick Reference

> Optimized for AI agent context windows. Minimal prose, maximum density.

## Declarations

```
f name()                  →  fn name()
+f name()                 →  pub fn name()
~f name()                 →  pub(crate) fn name()
af name()                 →  async fn name()
+af name()                →  pub async fn name()
c f name()                →  const fn name()
v x = expr                →  let x = expr
m x = expr                →  let mut x = expr
+v NAME: T = expr         →  pub const NAME: T = expr
S Name { fields }         →  struct Name { fields }
+S Name { fields }        →  pub struct Name { fields }
E Name { variants }       →  enum Name { variants }
+E Name { variants }      →  pub enum Name { variants }
T Name { methods }        →  trait Name { methods }
+T Name { methods }       →  pub trait Name { methods }
I Trait ~ Type { }        →  impl Trait for Type { }
I ~ Type { }              →  impl Type { }
M name                    →  mod name
+M name                   →  pub mod name
u path.to.Item            →  use path::to::Item
+u path.to.Item           →  pub use path::to::Item
```

## Control Flow

```
? cond { body }                    →  if cond { body }
? cond { body } : { body }        →  if cond { } else { }
? cond { } : ? cond2 { } : { }   →  if/else if/else chain
? expr { pat => body, }           →  match expr { pat => body, }
@ item ~ iter { body }            →  for item in iter { body }
loop { body }                     →  loop { body }
ret expr                          →  return expr
break                             →  break
continue                          →  continue
```

## Type Sugar

```
s          →  String
&s         →  &str
[T]~       →  Vec<T>
?T         →  Option<T>
R[T, E]    →  Result<T, E>
^T         →  Box<T>
$T         →  Rc<T>
@T         →  Arc<T>
{K: V}     →  HashMap<K, V>
{K}        →  HashSet<K>
&!T        →  &mut T
1b         →  true
0b         →  false
```

## Generics & Paths

```
f foo[T](x: T)         →  fn foo<T>(x: T)
f foo[T](x: T) ~> T: Clone   →  fn foo<T>(x: T) where T: Clone
foo[i32]()             →  foo::<i32>()
std.io.File            →  std::io::File
~.module.Item          →  crate::module::Item
Foo @{ x: 1, y: 2 }   →  Foo { x: 1, y: 2 }   (struct literal)
```

## String / Output Sugar

```
p"hello {name}"        →  println!("hello {name}")
f"hello {name}"        →  format!("hello {name}")
ep"error: {e}"         →  eprintln!("error: {e}")
```

## Attributes

```
@d(Debug, Clone)       →  #[derive(Debug, Clone)]
@i                     →  #[inline]
@test                  →  #[test]
@bench                 →  #[bench]
@cfg(test)             →  #[cfg(test)]
```

## Effects

```
f pure_fn() -> i32                      // no effect = pure
f read() -> R[s, Error] / io            // single effect
+af fetch() -> R[s, Error] / io, net    // multiple effects
```

Built-in effects: `io` `net` `rng` `async` `agent` `time` `env` `process`

## Effect Hierarchy

```
net  ⊃  io        (net implies io)
agent ⊃ async     (agent implies async)
```

## Standard Library Modules

```
std.io         I/O operations (File, BufReader, stdin/stdout)
std.net        Networking (TcpStream, UdpSocket, http)
std.fs         Filesystem (read, write, create_dir)
std.col        Collections (vec, map, set, deque)
std.sync       Synchronization (Mutex, RwLock, channel)
std.async      Async runtime (spawn, select, timeout)
std.fmt        Formatting (Display, Debug, Formatter)
std.str        String utilities (split, trim, parse)
std.math       Math (sin, cos, sqrt, PI)
std.time       Time (Instant, Duration, SystemTime)
std.json       JSON (parse, stringify, Value)
std.env        Environment (var, args, current_dir)
std.process    Process (Command, exit, spawn)
std.agent      Agent primitives (Agent, Capability, Swarm)
std.skb        Knowledge base (Rule, Query, Proof)
std.effect     Effect types (Effect, Handler, handle)
std.spec       Specifications (pre, post, invariant)
std.test       Testing (assert, mock, bench)
```

## Canonical Examples

### Hello World
```redox
+f main() / io {
    p"Hello, world!"
}
```

### Fibonacci
```redox
f fib(n: u64) -> u64 {
    ? n <= 1 { ret n }
    fib(n - 1) + fib(n - 2)
}
```

### Read File
```redox
u std.fs

f read_config(path: &s) -> R[s, io.Error] / io {
    fs.read_to_string(path)
}
```

### Struct with Methods
```redox
@d(Debug, Clone)
+S Point {
    x: f64,
    y: f64,
}

I ~ Point {
    +f new(x: f64, y: f64) -> Self {
        Self @{ x, y }
    }

    +f distance(&self, other: &Point) -> f64 {
        v dx = self.x - other.x
        v dy = self.y - other.y
        (dx * dx + dy * dy).sqrt()
    }
}
```

### Error Handling
```redox
u std.io
u std.json

+f load_config(path: &s) -> R[Config, Error] / io {
    v text = fs.read_to_string(path)?
    v config = json.parse[Config](&text)?
    ret config
}
```

### Agent
```redox
u std.agent.{Agent, Swarm}

+S Analyzer {
    data: [f64]~,
}

I Agent ~ Analyzer {
    +af execute(&!self) -> R[f64, Error] / agent {
        v sum: f64 = self.data.iter().sum()
        ret sum / self.data.len() as f64
    }
}
```
