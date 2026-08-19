# Chapter 5: Effects & Resolution

The effect system tracks and validates what side effects a function may
produce. Effects are MAGE's alternative to Rust's `unsafe` keyword — they
express capabilities explicitly and hierarchically.

---

## 5.1 Effect Model

### Built-In Effects

```rust
pub enum Effect {
    IO,       // file and stream I/O
    Net,      // network access (TCP, UDP, HTTP, DNS)
    FS,       // filesystem operations
    Async,    // async operations, runtime interaction
    Alloc,    // heap allocation
    Panic,    // unwinding / structured panics
    FFI,      // foreign function invocation
    Env,      // environment variables, system info
    Time,     // clock and timer access
    Gpu,      // GPU computation
    Npu,      // neural processing unit
    Llm,      // language model invocation
    Evolve,   // evolutionary computation
    Learn,    // training / gradient descent
    Rng,      // random number generation
    Agent,    // agent coordination — lifecycle, message, lease
    Proc,     // process/system access, external tool invocation
    Custom(String),  // user-defined effects, from an `effect` block
}
```

This is `hir::Effect`, and the seventeen non-`Custom` variants are exactly the
built-in kinds of `MAGE_SPEC.md` §11.2. Anything else — `db`, `log`, `unsafe` —
is `Custom`, and must be declared by an `effect` block or it is an error.

### Effect Sets

Every function and expression has an `EffectSet` — the set of effects it
may produce:

```rust
pub type EffectSet = BTreeSet<Effect>;
```

`BTreeSet` gives deterministic ordering for serialization and diagnostics.

### Declaring Effects

Functions annotate their effects after the return type:

```MAGE
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
     b. For each recognized effectful builtin (by name — see §11.2):
        - read(...) / println(...) → add Effect::IO
        - connect(...) → add Effect::Net
        - mkdir(...) → add Effect::FS
     c. For each `E.op(...)` on a declared effect block → add Effect::Custom(e)
     d. For each `ns.op(...)` on a capability namespace → add its effect
        (io.println → IO, llm.generate → Llm, process.spawn → Proc;
         hir::CAPABILITY_NAMESPACES is the table, and a declared
         `effect E { … }` outranks a namespace of the same name)
     e. For each `handle { … } with E { … }` region:
        resolve the region's effects, subtract E, union the rest
  3. Compare inferred effect_set with f's declared effects
  4. If declared effects are a superset of inferred → OK
  5. If inferred has effects not in declared → ERROR
  6. If declared has effects not in inferred → OK, deliberately: an annotation
     is an upper bound (§11.4). There is no over-declaration warning.
```

Steps 4–6 are the *boundary* rule and apply to any function that annotates, and
to every `pub` function whether it annotates or not. A private function with no
annotation infers silently — its effects still reach its public callers through
step 2a.

### Implementation

> **Sketch.** There is no `EffectChecker`, no `HirDatabase` and no `rdx_effects`
> crate. The real entry point is `effects::infer_effects(&ast::Module) ->
> EffectInfer`, which walks the AST — not HIR — in passes over the whole module
> rather than per function, because the call graph has to be closed before any
> function's set is final. The shape below is still roughly what step 2 does.

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

```MAGE
effect Config {
    f read(path: str) -> str;
}

f load(path: str) -> str / config { Config.read(path) }

f mocked(path: str) -> str {
    v result = handle {
        load(path)
    } with Config {
        read(p) => "mock data"
    }
    result
}
```

The effect handled is named after `with`; `handle` itself takes only the block.
An arm names an operation bare (`read`, not `Config.read`), and the operation
must be one the `effect Config { … }` block declares.

In the effect checker:
- The body may produce `config` effects
- The `handle` intercepts them — `config` does NOT propagate to the caller
- Other effects (if any) still propagate normally
- The arm's *own* effects join the enclosing function's set

The subtraction is per region, not per function: `handled` records each
`handle` occurrence separately, and a call to `load` sitting outside this block
still reports `config`. Deleting the effect from the whole function would be
unsound.

This is how effect mocking works in tests. Only a declared effect can be
handled — a built-in kind like `io` has no `effect` block, so there is nothing
to name after `with`.

## 5.3 Effect Hierarchy

> **Design, not implementation.** `is_sub_effect` does not exist. Effects are a
> flat set: `fs` is a built-in kind in its own right, not `Custom("fs")` under
> `io`, so declaring `/ io` grants nothing else and `/ io` does not satisfy an
> inferred `fs`. There is no `Effect::Unsafe`. The Rust below is a sketch of a
> hierarchy nobody has built.

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

> **Design, not implementation.** `forge` parses no `[capabilities]` table and
> no `allow-*` key, and nothing checks an effect against a per-file grant. The
> boundary the compiler *does* enforce is §11.4's: a `pub` function must declare
> what it performs, and a function that annotates at all is held to
> `inferred ⊆ declared`. That gate is per function, not per file.

Effects are checked against the capability grants in `Forge.toml`:

```toml
[capabilities]
allow-io = ["src/config.mg", "src/server.mg"]
allow-net = ["src/server.mg"]
allow-unsafe = ["src/simd.mg"]
```

### Validation Flow

```
1. Parse Forge.toml capability grants
2. For each file in the crate:
   a. Collect all functions in the file
   b. For each function, get its effect set
   c. Check: is the file listed in the capability grant for each effect?
   d. If not → ERROR: "src/foo.mg uses `io` effect but is not listed in
      [capabilities] allow-io"
```

### Default Capabilities

Part of the same unbuilt design. There is no grant mechanism, so no effect is
"always available" relative to one — every built-in kind is equally writable,
and every one propagates to callers the same way. `log` is not an effect at all:
it is a capability namespace (`log.info(…)`), and `/ log` is an unknown-effect
error.

## 5.5 Effect Polymorphism

> **Design, not implementation.** `/ *` is a parse error — `expected effect
> name, found '*'`. There are no effect variables and no effect bounds; a
> function that takes a closure gets the closure's effects only if the checker
> can see through to the call, which for a generic parameter it cannot.

The design, written as it would look. This block is **invalid MAGE** today:

```MAGE
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

An `effect` block defines an effect and the operations that belong to it.
Operations are terminated with `;`, as in a trait:

```MAGE
effect Dice {
    f roll(max: i32) -> i32;
}
```

Calling an operation performs the effect — this is the introduction rule, and
what puts `dice` in the function's inferred set. The annotation is the effect
*name*, lowercased, not the block name:

```MAGE
effect Dice {
    f roll(max: i32) -> i32;
}

f shuffled(n: i32) -> i32 / dice {
    Dice.roll(n)
}
```

`handle … with` discharges it. The effect being handled is named after `with`,
not after `handle`, and the arms name operations bare:

```MAGE
effect Dice {
    f roll(max: i32) -> i32;
}

f shuffled(n: i32) -> i32 / dice {
    Dice.roll(n)
}

+f main() -> i32 {
    handle {
        shuffled(6)
    } with Dice {
        roll(max) => max - 1
    }
}
```

`main` is pure: the handler removes `dice` from the block it wraps. This
program evaluates to `5`. Two things the compiler checks that are easy to
assume it does not: an operation the effect does not declare is an error
(a misspelled `Dice.rol` does not silently count as performing `dice`), and
the subtraction is per *block*, not per function — an unhandled call beside a
handled one still reports.

An arm's own effects are attributed honestly, so handling `dice` by writing a
file makes the handling function `/ fs`. A handler exchanges one effect for the
effects of handling it.

Handlers do **not** resume: an arm returns a value to the `handle` expression
rather than continuing the suspended computation. See §11.5 of the spec and the
`resume` open item.

## 5.7 Diagnostics

> **Illustrative renderings, not real output.** The compiler emits no `E0401` /
> `E0402` / `W0410` codes and no source-span carets — a diagnostic is one line
> of prose. `E0402` is for §5.4, which does not exist. And `W0410` contradicts
> §11.4: over-declaration is deliberately allowed as an upper bound, so
> declaring `/ io` and not using it is not warned about.

### Missing Effect Declaration

```
error[E0401]: function `save_data` performs `io` effect but does not declare it
  --> src/data.mg:15:1
   |
15 | f save_data(path: &s, data: &[u8]) -> R[(), Error] {
   |            ^^^^^^^^^ missing `/ io` effect annotation
   |
   = help: add `/ io` to the function signature:
           f save_data(path: &s, data: &[u8]) -> R[(), Error] / io {
```

### Capability Violation

```
error[E0402]: file `src/utils.mg` uses `net` effect but is not authorized
  --> src/utils.mg:8:5
   |
 8 |     v resp = http.get(url).await?
   |              ^^^^^^^^^^^^^ `net` effect not permitted in this file
   |
   = note: add to Forge.toml:
           [capabilities]
           allow-net = ["src/utils.mg"]
```

### Over-Declaration Warning

```
warning[W0410]: function `compute` declares `/ io` effect but never uses it
  --> src/math.mg:3:1
   |
 3 | f compute(x: f64) -> f64 / io {
   |                           ^^^^ unnecessary effect
   |
   = help: remove the `/ io` annotation — this function is pure
```
