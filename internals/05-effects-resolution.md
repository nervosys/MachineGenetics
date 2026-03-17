# Chapter 5: Effects & Resolution

The effect system tracks and validates what side effects a function may
produce. Effects are Redox's alternative to Rust's `unsafe` keyword — they
express capabilities explicitly and hierarchically.

---

## 5.1 Effect Model

### Built-In Effects

```rust
pub enum Effect {
    Io,       // file system, stdio, process spawning
    Net,      // network access (TCP, UDP, HTTP, DNS)
    Async,    // async operations, runtime interaction
    Unsafe,   // raw pointers, FFI, transmute
    Db,       // database access
    Agent,    // agent spawning, Swarm operations
    Log,      // logging, tracing, metrics
    Env,      // environment variables, system info
    Custom(String),  // user-defined effects
}
```

### Effect Sets

Every function and expression has an `EffectSet` — the set of effects it
may produce:

```rust
pub type EffectSet = BTreeSet<Effect>;
```

`BTreeSet` gives deterministic ordering for serialization and diagnostics.

### Declaring Effects

Functions annotate their effects after the return type:

```redox
f read_file(path: &s) -> R[s, Error] / io { ... }
af fetch_url(url: &s) -> R[s, Error] / net, async { ... }
f compute(x: i32) -> i32 { ... }  // pure — no effects
```

The `/ effect1, effect2` syntax declares the function's effect signature.

## 5.2 Effect Inference

The effect checker (`rdx_effects`, prototype: `prototype/src/effects.rs`)
performs bottom-up inference.

### Algorithm

```
For each function f:
  1. Initialize effect_set = {} (empty)
  2. Walk the HIR body:
     a. For each function call g(args):
        - Look up g's declared effect set
        - Union it into f's effect_set
     b. For each effect-producing primitive:
        - fs.read(...) → add Effect::Io
        - net.connect(...) → add Effect::Net
        - unsafe { } → add Effect::Unsafe
  3. Compare inferred effect_set with f's declared effects
  4. If declared effects are a superset of inferred → OK
  5. If inferred has effects not in declared → ERROR
  6. If declared has effects not in inferred → WARNING (over-declaration)
```

### Implementation

```rust
pub struct EffectChecker<'a> {
    db: &'a dyn HirDatabase,
    current_fn: DefId,
    inferred: EffectSet,
    errors: Vec<EffectError>,
}

impl<'a> EffectChecker<'a> {
    pub fn check_fn(&mut self, def_id: DefId) -> EffectResult {
        self.current_fn = def_id;
        self.inferred.clear();

        let body = self.db.fn_body(def_id);
        self.walk_body(&body);

        let declared = self.db.fn_effects(def_id);
        self.compare(&declared, &self.inferred)
    }

    fn walk_expr(&mut self, expr: &HirExpr) {
        match &expr.kind {
            HirExprKind::Call { callee, .. } => {
                let callee_effects = self.db.fn_effects(callee.def_id);
                self.inferred.extend(callee_effects);
            }
            HirExprKind::Handle { effects, body, handlers } => {
                // handle block intercepts effects — they don't propagate
                let mut inner = EffectChecker::new(self.db);
                inner.walk_body(body);
                // Remove handled effects from what propagates up
                let unhandled = inner.inferred.difference(effects);
                self.inferred.extend(unhandled);
            }
            // ... other cases
            _ => {}
        }
    }
}
```

### Handle Blocks

The `handle` block is the effect system's key feature — it intercepts
effects and provides alternative implementations:

```redox
v result = handle / io {
    read_file("config.toml")
} with {
    fs.read_to_string(path) => Ok(s.from("mock data")),
}
```

In the effect checker:
- The body of `handle / io { ... }` may produce `io` effects
- But the `handle` intercepts them — `io` does NOT propagate to the caller
- Other effects (if any) still propagate normally

This is how effect mocking works in tests.

## 5.3 Effect Hierarchy

Effects form a hierarchy for capability scoping:

```
io
├── fs     (file system subset)
├── stdio  (stdin/stdout/stderr)
└── proc   (process spawning)

net
├── tcp
├── udp
├── http
└── dns

unsafe
├── ptr    (raw pointers)
├── ffi    (foreign function interface)
└── asm    (inline assembly)
```

Declaring `/ io` grants all of `fs`, `stdio`, `proc`. You can be more
specific: `/ fs` grants only file system access.

### Implementation

```rust
fn is_sub_effect(parent: &Effect, child: &Effect) -> bool {
    match (parent, child) {
        (Effect::Io, Effect::Custom(s)) if s == "fs" || s == "stdio" || s == "proc" => true,
        (Effect::Net, Effect::Custom(s)) if s == "tcp" || s == "udp" || s == "http" || s == "dns" => true,
        (Effect::Unsafe, Effect::Custom(s)) if s == "ptr" || s == "ffi" || s == "asm" => true,
        _ => parent == child,
    }
}
```

## 5.4 Capability Validation

Effects are checked against the capability grants in `Forge.toml`:

```toml
[capabilities]
allow-io = ["src/config.rdx", "src/server.rdx"]
allow-net = ["src/server.rdx"]
allow-unsafe = ["src/simd.rdx"]
```

### Validation Flow

```
1. Parse Forge.toml capability grants
2. For each file in the crate:
   a. Collect all functions in the file
   b. For each function, get its effect set
   c. Check: is the file listed in the capability grant for each effect?
   d. If not → ERROR: "src/foo.rdx uses `io` effect but is not listed in
      [capabilities] allow-io"
```

### Default Capabilities

Some effects are always available:

- `/ log` — logging is always permitted
- `/ async` — async is always permitted (it's a control flow mechanism)
- `/ agent` — agent spawning requires explicit grant

## 5.5 Effect Polymorphism

Functions can be generic over effects using effect bounds:

```redox
f with_retry[F, R](op: F, retries: u32) -> R[R, Error]
~> F: Fn() -> R[R, Error] / * {
    // The `/ *` means "whatever effects F has"
    // This function inherits F's effects
    @ _ ~ 0..retries {
        ? op() {
            Ok(val) => ret Ok(val),
            Err(_) => continue,
        }
    }
    op()
}
```

The `/ *` annotation means "this function's effects include whatever
effects the closure argument has."

## 5.6 User-Defined Effects

Users can define custom effects:

```redox
effect Rng {
    f random_u32() -> u32
    f random_range(min: u32, max: u32) -> u32
}
```

This defines a new effect `Rng` with associated operations. Functions using
randomness declare `/ Rng`:

```redox
f shuffle[T](data: &![T]~) / Rng {
    @ i ~ (1..data.len()).rev() {
        v j = Rng.random_range(0, i as u32) as usize
        data.swap(i, j)
    }
}
```

Tests can intercept the effect:

```redox
@test
f test_shuffle() {
    v result = handle / Rng {
        m data = [1, 2, 3]~
        shuffle(&!data)
        data
    } with {
        Rng.random_range(_, max) => max - 1,  // deterministic
    }
    // result is deterministic despite shuffle using randomness
}
```

## 5.7 Diagnostics

### Missing Effect Declaration

```
error[E0401]: function `save_data` performs `io` effect but does not declare it
  --> src/data.rdx:15:1
   |
15 | f save_data(path: &s, data: &[u8]) -> R[(), Error] {
   |            ^^^^^^^^^ missing `/ io` effect annotation
   |
   = help: add `/ io` to the function signature:
           f save_data(path: &s, data: &[u8]) -> R[(), Error] / io {
```

### Capability Violation

```
error[E0402]: file `src/utils.rdx` uses `net` effect but is not authorized
  --> src/utils.rdx:8:5
   |
 8 |     v resp = http.get(url).await?
   |              ^^^^^^^^^^^^^ `net` effect not permitted in this file
   |
   = note: add to Forge.toml:
           [capabilities]
           allow-net = ["src/utils.rdx"]
```

### Over-Declaration Warning

```
warning[W0410]: function `compute` declares `/ io` effect but never uses it
  --> src/math.rdx:3:1
   |
 3 | f compute(x: f64) -> f64 / io {
   |                           ^^^^ unnecessary effect
   |
   = help: remove the `/ io` annotation — this function is pure
```
