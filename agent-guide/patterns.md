# MAGE Idiomatic Patterns

> Common patterns for generating correct, idiomatic MAGE code.
> All examples use **human syntax** (`pub fn`, `struct`, `match`, `if`), which
> is the readable half of MAGE's dual surface. Every block below was verified
> with `mage-parse --check`; the previous version of this file was Rust in a
> MAGE fence, and ten of its twelve patterns did not parse.

**Three things human syntax does *not* inherit from Rust**, each of which cost
a pattern below:

- **`let` does not exist.** `val` binds immutably, `var` mutably. The parser
  rejects `let` by name, with that advice.
- **Single-letter keywords are not identifiers.** `f`, `v`, `m`, `u`, `S`, `E`,
  `T`, `I`, `C`, `D`, `M`, `U`, `Y`, `Z` are the agent-mode spellings of
  declarations, so `|u| …` and `f(x)` as a *variable* are parse errors. Name
  closure parameters and function-typed parameters in full.
- **There is no `::`, no `impl Trait`, no `where`, no `mod`.** Paths are flat
  (Pattern 11), generics go in `[…]` (Pattern 8), and effects are annotated
  with `/ effect` rather than bounded (Pattern 8 again).

---

## Pattern 1: Configuration with Default Arguments

MAGE has default arguments, so the builder chain has nothing to build.

```MAGE
pub struct ServerConfig {
    host: str,
    port: i32,
    max_connections: i32,
}

// A default argument replaces the builder: the caller names only what it
// changes, and the signature documents the defaults.
pub fn server_config(
    host: str = "localhost",
    port: i32 = 8080,
    max_connections: i32 = 100,
) -> ServerConfig {
    @ServerConfig { host: host, port: port, max_connections: max_connections }
}

pub fn main() -> i32 {
    val defaults = server_config()
    val tuned = server_config("0.0.0.0", 3000, 500)
    defaults.port + tuned.port
}
```

## Pattern 2: Errors as a Sum Type

```MAGE
// An error type is an ordinary sum. There is no `Error` type in scope, and no
// `From`/`Display` traits to implement.
pub enum AppError {
    NotFound(str),
    Parse(str),
    Denied(str),
}

pub fn describe(err: AppError) -> str {
    match err {
        NotFound(key) => join(["not found:", key], " "),
        Parse(msg) => join(["parse error:", msg], " "),
        Denied(who) => join(["denied:", who], " "),
    }
}

// `R[T, E]` is the result type; `T or E` is the same thing spelled as a union.
pub fn load(path: str) -> R[str, AppError] / fs {
    guard len(path) > 0 else { return Err(Parse("empty path")) }
    Ok(fs.read_to_string(path))
}

pub fn load_or_default(path: str) -> str / fs {
    match load(path) {
        Ok(text) => text,
        Err(err) => describe(err),
    }
}
```

## Pattern 3: Vocabulary Pipelines

```MAGE
// `map`, `filter` and `fold` are part of the standard vocabulary — global
// functions, not methods. There is no `.iter()`, no `.collect()`.
pub fn active_names(names: [str]~) -> [str]~ {
    map(filter(names, |name| len(name) > 0), |name| upper(name))
}

pub fn total_cost(prices: [f64]~) -> f64 {
    fold(prices, 0.0, |acc, price| acc + price)
}

pub fn first_admin(names: [str]~) -> ?str {
    first(filter(names, |name| name == "admin"))
}
```

## Pattern 4: Optional Values

```MAGE
// `?T` is the optional type. `guard … else` must diverge — `return`, `break`,
// or a panic — which makes the early exit visible at the top of the body.
pub fn head(xs: [i32]~) -> ?i32 {
    guard len(xs) > 0 else { return None }
    Some(xs[0])
}

pub fn head_or(xs: [i32]~, fallback: i32 = 0) -> i32 {
    match head(xs) {
        Some(x) => x,
        None => fallback,
    }
}
```

## Pattern 5: Traits and Implementations

```MAGE
pub trait Renderer {
    fn render(self, data: str) -> str;
}

pub struct Html { prefix: str }
pub struct Json { indent: i32 }

impl Renderer for Html {
    pub fn render(self, data: str) -> str {
        join([self.prefix, data], "")
    }
}

impl Renderer for Json {
    pub fn render(self, data: str) -> str {
        join(["{\"content\": \"", data, "\"}"], "")
    }
}

pub fn main() -> str {
    val page = @Html { prefix: "<p>" }
    page.render("hello")
}
```

## Pattern 6: Wrapping a Primitive

```MAGE
// There are no tuple structs. Wrap a primitive in a one-field record and
// attach the conversions with `extend` — no `impl` block, no `Self`.
pub struct Temperature { celsius: f64 }

extend Temperature {
    pub fn to_fahrenheit(self) -> f64 {
        self.celsius * 9.0 / 5.0 + 32.0
    }
}

pub fn main() -> f64 {
    val boiling = @Temperature { celsius: 100.0 }
    boiling.to_fahrenheit()
}
```

## Pattern 7: Agent with a State Machine

```MAGE
// The state is a sum; the transition is a function returning the next state.
// `agent` declares the role and the capabilities it may use — it carries no
// code, so there is no `impl Agent for …` and no `execute` to override.
pub enum Stage {
    Idle,
    Fetching(str),
    Done(str),
    Failed(str),
}

agent DataPipeline {
    capabilities: [net]
}

// Declare exactly what the body performs — `net` here, and nothing more.
// A declared set is an upper bound, so over-declaring passes the check while
// handing the function a capability it never uses.
pub fn step(stage: Stage) -> Stage / net {
    match stage {
        Idle => Fetching("https://example.com"),
        Fetching(url) => Done(net.connect(url)),
        Done(body) => Done(body),
        Failed(msg) => Failed(msg),
    }
}
```

## Pattern 8: Higher-Order Functions and Generics

```MAGE
// Generic parameters go in square brackets. A function-typed parameter carries
// no effect annotation: there is no effect polymorphism, so `fn(str) -> T / io`
// does not parse. The *caller* declares what it performs.
pub fn apply_twice[T](x: T, func: fn(T) -> T) -> T {
    func(func(x))
}

pub fn with_file(path: str, work: fn(str) -> i32) -> i32 / fs {
    val content = fs.read_to_string(path)
    work(content)
}

pub fn main() -> i32 {
    apply_twice(3, |n| n * 2)
}
```

## Pattern 9: Capability-Gated Operations

```MAGE
// A capability namespace *is* the gate: reaching `fs` puts `fs` in the
// inferred set, and a `pub` function must declare it. The declaration is the
// permission, checked at compile time — there is no runtime `cap.request`.
pub fn read_key(root: str, key: str) -> R[str, str] / fs {
    guard len(key) > 0 else { return Err("empty key") }
    Ok(fs.read_to_string(join([root, key], "/")))
}

// Nothing forces a caller to keep the capability: `handle … with` removes the
// effect for the block it wraps, so this function is pure.
effect Store {
    fn read(key: str) -> str;
}

fn cached(key: str) -> str / store { Store.read(key) }

pub fn read_cached(key: str) -> str {
    handle {
        cached(key)
    } with Store {
        read(k) => k,
    }
}
```

## Pattern 10: Swarm Fan-Out / Fan-In

```MAGE
agent UrlChecker {
    capabilities: [net, agent]
}

swarm Fleet {
    agent: UrlChecker
}

// Fan out with `agent.spawn`, fan in by folding the results. `agent` is a
// capability namespace, so the effect must be declared like any other.
pub fn check_all(urls: [str]~) -> i32 / net, agent {
    var checked = 0
    for url in urls {
        agent.spawn(url)
        net.connect(url)
        checked += 1
    }
    checked
}
```

## Pattern 11: One Flat Namespace

```MAGE
// There is no module system. Every function, type, effect and agent in the
// compilation unit shares one flat namespace, and the standard vocabulary
// (`map`, `join`, `len`, …) plus the capability namespaces (`io`, `fs`, `net`,
// …) are in scope everywhere.
//
// `use` parses, for source compatibility, but brings nothing into scope — the
// checker warns and the name stays unresolved. Do not write it.
pub struct User { name: str }

pub fn handle_request(user: User) -> str / io {
    println(user.name)
    user.name
}
```

## Pattern 12: Tests and Effect Mocking

```MAGE
// `@test` marks a test function. There is no `mod tests`, no `#[cfg(test)]`,
// and no `assert_eq!` — a test is a function whose value the runner compares.
@test
pub fn test_addition() -> i32 {
    2 + 3
}

effect Cfg {
    fn read(path: str) -> str;
}

fn read_config(path: str) -> str / cfg {
    Cfg.read(path)
}

// A handler substitutes the operation's value, so the test never touches a
// real file. The effect being handled is named after `with`.
@test
pub fn test_with_mocked_io() -> str {
    handle {
        read_config("test.toml")
    } with Cfg {
        read(path) => "mock data",
    }
}
```
