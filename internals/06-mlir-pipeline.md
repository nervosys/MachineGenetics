# Chapter 6: MLIR Pipeline

The backend lowers typed, effect-checked HIR into MLIR (Multi-Level
Intermediate Representation), then progressively lowers through MLIR
dialects to LLVM IR and finally machine code.

---

## 6.1 Why MLIR?

Traditional compilers go directly from IR to LLVM IR. Redox interposes MLIR
because:

1. **Multi-target**: MLIR has dialects for CPU (LLVM), GPU (GPU/SPIR-V),
   WASM, and custom accelerators. One HIR → many backends.
2. **Effect representation**: The Redox MLIR dialect encodes effects as
   first-class attributes, enabling effect-aware optimizations.
3. **Cost oracle**: MLIR's analysis framework enables the cost oracle to
   estimate performance characteristics before codegen.
4. **Autotuning**: MLIR passes can generate variant code for performance
   autotuning (`@pa` annotation).

## 6.2 The Redox MLIR Dialect

The Redox dialect defines operations that map directly to Redox language
constructs:

### Operations

| Op                  | Description         | Operands                           |
| ------------------- | ------------------- | ---------------------------------- |
| `redox.func`        | Function definition | name, params, return type, effects |
| `redox.call`        | Function call       | callee, args                       |
| `redox.struct_def`  | Struct definition   | name, fields                       |
| `redox.enum_def`    | Enum definition     | name, variants                     |
| `redox.trait_def`   | Trait definition    | name, methods                      |
| `redox.impl`        | Impl block          | type, trait, methods               |
| `redox.const`       | Constant definition | name, type, value                  |
| `redox.let`         | Variable binding    | name, type, value                  |
| `redox.match`       | Pattern match       | scrutinee, arms                    |
| `redox.for`         | For loop            | pattern, iterator, body            |
| `redox.effect`      | Effect marker       | effect set                         |
| `redox.handle`      | Handle block        | effects, body, handlers            |
| `redox.agent_spawn` | Agent spawn         | agent value                        |
| `redox.swarm_join`  | Swarm join          | swarm value                        |

### Type System

```mlir
// Primitive types
!redox.int<32>         // i32
!redox.uint<64>        // u64
!redox.float<64>       // f64
!redox.bool
!redox.str             // String
!redox.unit            // ()

// Compound types
!redox.vec<!redox.int<32>>            // [i32]~
!redox.option<!redox.str>             // ?s
!redox.result<!redox.str, !redox.err> // R[s, Error]
!redox.map<!redox.str, !redox.int<32>> // {s: i32}
!redox.box<!redox.str>                // ^s
!redox.rc<!redox.str>                 // $s
!redox.arc<!redox.str>                // @s
!redox.ref<mutable=false, !redox.str> // &s
!redox.ref<mutable=true, !redox.str>  // &!s
```

### Effect Attributes

```mlir
redox.func @read_file(%path: !redox.ref<mutable=false, !redox.str>)
    -> !redox.result<!redox.str, !redox.err>
    attributes { effects = ["io"] }
{
    // ...
}
```

## 6.3 Emitter: HIR → Redox MLIR

The emitter is in `rdx_mlir` (prototype: `prototype/src/mlir.rs`).

### Emitter Context

```rust
struct EmitCtx<'a> {
    buf: String,        // output MLIR text buffer
    indent: usize,      // current indentation level
    ssa: usize,         // next SSA value number
    effects: &'a HashMap<String, EffectSet>,  // per-function effect sets
}
```

### SSA Value Generation

MLIR uses SSA (Static Single Assignment) form. Each computed value gets a
unique name:

```rust
fn fresh(&mut self) -> String {
    let n = self.ssa;
    self.ssa += 1;
    format!("%{n}")
}
```

### Function Emission

```rust
fn emit_function(&mut self, f: &FunctionDef, vis: &Visibility) {
    let vis_attr = match vis {
        Visibility::Public => "public",
        Visibility::Private => "private",
    };

    // Build parameter list
    let params: Vec<String> = f.params.iter()
        .map(|p| format!("%{}: {}", p.name, self.mlir_type(&p.ty)))
        .collect();

    // Build return type
    let ret = match &f.return_type {
        Some(ty) => format!(" -> {}", self.mlir_type(ty)),
        None => String::new(),
    };

    // Build effect attributes
    let effects = self.effects.get(&f.name)
        .map(|es| format_effect_attr(es))
        .unwrap_or_default();

    self.line(&format!(
        "redox.func {} @{}({}){} attributes {{ sym_visibility = \"{}\"{} }} {{",
        vis_attr, f.name, params.join(", "), ret, vis_attr, effects
    ));

    self.indent += 1;
    self.emit_block(&f.body);
    self.indent -= 1;
    self.line("}");
}
```

### Type Mapping

```rust
fn mlir_type(&self, ty: &Type) -> String {
    match ty {
        Type::Path { segments, type_args } => {
            let name = segments.join(".");
            match name.as_str() {
                "i8"    => "!redox.int<8>",
                "i16"   => "!redox.int<16>",
                "i32"   => "!redox.int<32>",
                "i64"   => "!redox.int<64>",
                "u8"    => "!redox.uint<8>",
                "u32"   => "!redox.uint<32>",
                "u64"   => "!redox.uint<64>",
                "f32"   => "!redox.float<32>",
                "f64"   => "!redox.float<64>",
                "bool"  => "!redox.bool",
                "usize" => "!redox.uint<64>",
                _ => {
                    if type_args.is_empty() {
                        format!("!redox.named<\"{}\">", name)
                    } else {
                        let args: Vec<String> = type_args.iter()
                            .map(|a| self.mlir_type(a)).collect();
                        format!("!redox.named<\"{}\", {}>", name, args.join(", "))
                    }
                }
            }
        }
        Type::StringType => "!redox.str",
        Type::Vec { inner } => format!("!redox.vec<{}>", self.mlir_type(inner)),
        Type::Option { inner } => format!("!redox.option<{}>", self.mlir_type(inner)),
        Type::Result { ok, err } => format!(
            "!redox.result<{}, {}>", self.mlir_type(ok), self.mlir_type(err)
        ),
        Type::OwnedPtr { inner } => format!("!redox.box<{}>", self.mlir_type(inner)),
        Type::Arc { inner } => format!("!redox.arc<{}>", self.mlir_type(inner)),
        Type::Rc { inner } => format!("!redox.rc<{}>", self.mlir_type(inner)),
        Type::Reference { mutable, inner } => format!(
            "!redox.ref<mutable={}, {}>", mutable, self.mlir_type(inner)
        ),
        // ... other types
    }
}
```

## 6.4 Lowering Passes

After emitting the Redox dialect, a series of MLIR passes lower it toward
LLVM:

```
Redox MLIR
    │
    ▼
┌──────────────────────────┐
│ Pass: Effect Elimination │  Remove effect markers, insert runtime checks
└──────────┬───────────────┘
           ▼
┌──────────────────────────┐
│ Pass: Sugar Lowering     │  Vec → alloc+ptr, Option → tag+union, etc.
└──────────┬───────────────┘
           ▼
┌──────────────────────────┐
│ Pass: Agent Lowering     │  Spawn → thread pool dispatch, Swarm → channels
└──────────┬───────────────┘
           ▼
┌──────────────────────────┐
│ Pass: Memory Lowering    │  Box → malloc, Rc → refcount, Arc → atomic refcount
└──────────┬───────────────┘
           ▼
┌──────────────────────────┐
│ Pass: Standard MLIR      │  SCF (structured control flow), Arith, MemRef
└──────────┬───────────────┘
           ▼
┌──────────────────────────┐
│ Pass: LLVM Lowering      │  Lower MLIR std ops to LLVM dialect
└──────────┬───────────────┘
           ▼
LLVM IR → LLVM Backend → Machine Code
```

### Effect Elimination Pass

Converts effect markers into runtime validation:

- `/ io` → no-op in release, capability check in debug
- `handle / io { body } with { handlers }` → generates dispatch table that
  intercepts calls

### Sugar Lowering Pass

Converts Redox type sugar to underlying representations:

| Redox Type        | MLIR Lowering                      |
| ----------------- | ---------------------------------- |
| `[T]~` (Vec)      | Pointer + length + capacity struct |
| `?T` (Option)     | Discriminant + union               |
| `R[T,E]` (Result) | Discriminant + union               |
| `{K:V}` (Map)     | Hash table struct                  |
| `^T` (Box)        | Heap allocation + unique pointer   |
| `$T` (Rc)         | Refcount + data pointer            |
| `@T` (Arc)        | Atomic refcount + data pointer     |

### Agent Lowering Pass

Converts agent primitives to runtime calls:

| Redox Construct      | Lowered To                              |
| -------------------- | --------------------------------------- |
| `Swarm.new()`        | Thread pool allocation                  |
| `swarm.spawn(agent)` | Task submission to pool + channel setup |
| `swarm.join_all()`   | Channel recv loop + result collection   |
| `Agent.execute`      | Closure wrapping for dispatch           |

## 6.5 Cost Oracle

The cost oracle uses MLIR's analysis framework to estimate performance:

```rust
pub struct CostEstimate {
    pub instructions: u64,       // estimated instruction count
    pub memory_bytes: u64,       // estimated memory usage
    pub latency_ns: u64,         // estimated latency
    pub cache_pressure: f64,     // 0.0 (cold) to 1.0 (thrashing)
    pub branch_predictability: f64, // 0.0 to 1.0
}
```

The cost oracle is queried by:
- The performance advisor (ACI)
- The autotuning system (`@pa` annotation)
- The `rdx bench --estimate` command
- Agents via `rap.query("cost", func_id)`

## 6.6 Multi-Target Code Generation

MLIR enables targeting multiple backends from the same HIR:

| Target       | MLIR Path                           | Output         |
| ------------ | ----------------------------------- | -------------- |
| x86-64       | Redox → LLVM dialect → LLVM x86     | Native binary  |
| AArch64      | Redox → LLVM dialect → LLVM AArch64 | Native binary  |
| WASM         | Redox → LLVM dialect → LLVM WASM    | `.wasm` module |
| GPU (NVIDIA) | Redox → GPU dialect → NVVM          | PTX kernel     |
| GPU (AMD)    | Redox → GPU dialect → ROCDL         | AMDGPU ISA     |
| SPIR-V       | Redox → SPIR-V dialect              | Vulkan compute |

The `@pt(target)` annotation controls target selection:

```redox
@pt(auto)  // let the compiler choose (default)
@pt(cpu)   // force CPU
@pt(gpu)   // force GPU
```

## 6.7 Testing the Backend

### MLIR Output Tests

Compare expected MLIR text with emitter output:

```rust
#[test]
fn test_emit_function() {
    let source = "+f add(a: i32, b: i32) -> i32 { a + b }";
    let ast = parse(source);
    let effects = EffectInfer::new();
    let mlir = emit(&ast, &effects);

    assert!(mlir.contains("redox.func public @add"));
    assert!(mlir.contains("!redox.int<32>"));
}
```

### Round-Trip Tests

For lowering passes, verify that lowering then re-raising preserves
semantics:

```bash
rdx build --emit=mlir src/main.rdx > before.mlir
rdx mlir-pass --lower-sugar before.mlir > lowered.mlir
rdx mlir-pass --roundtrip lowered.mlir > after.mlir
diff before.mlir after.mlir  # structural equivalence
```

### FileCheck Tests

MLIR provides FileCheck for pattern-matching test output:

```
// RUN: rdx build --emit=mlir %s | FileCheck %s
// CHECK: redox.func public @main
// CHECK-SAME: -> !redox.result<!redox.unit, !redox.box<!redox.named<"Error">>>
// CHECK-SAME: effects = ["io"]
+f main() -> R[(), ^dyn Error] / io {
    p"hello"
}
```
