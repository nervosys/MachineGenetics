# Chapter 6: MLIR Pipeline

The backend lowers typed, effect-checked HIR into MLIR (Multi-Level
Intermediate Representation), then progressively lowers through MLIR
dialects to LLVM IR and finally machine code.

---

## 6.1 Why MLIR?

Traditional compilers go directly from IR to LLVM IR. MechGen interposes MLIR
because:

1. **Multi-target**: MLIR has dialects for CPU (LLVM), GPU (GPU/SPIR-V),
   WASM, and custom accelerators. One HIR → many backends.
2. **Effect representation**: The MechGen MLIR dialect encodes effects as
   first-class attributes, enabling effect-aware optimizations.
3. **Cost oracle**: MLIR's analysis framework enables the cost oracle to
   estimate performance characteristics before codegen.
4. **Autotuning**: MLIR passes can generate variant code for performance
   autotuning (`@pa` annotation).

## 6.2 The MechGen MLIR Dialect

The MechGen dialect defines operations that map directly to MechGen language
constructs:

### Operations

| Op                  | Description         | Operands                           |
| ------------------- | ------------------- | ---------------------------------- |
| `MechGen.func`        | Function definition | name, params, return type, effects |
| `MechGen.call`        | Function call       | callee, args                       |
| `MechGen.struct_def`  | Struct definition   | name, fields                       |
| `MechGen.enum_def`    | Enum definition     | name, variants                     |
| `MechGen.trait_def`   | Trait definition    | name, methods                      |
| `MechGen.impl`        | Impl block          | type, trait, methods               |
| `MechGen.const`       | Constant definition | name, type, value                  |
| `MechGen.let`         | Variable binding    | name, type, value                  |
| `MechGen.match`       | Pattern match       | scrutinee, arms                    |
| `MechGen.for`         | For loop            | pattern, iterator, body            |
| `MechGen.effect`      | Effect marker       | effect set                         |
| `MechGen.handle`      | Handle block        | effects, body, handlers            |
| `MechGen.agent_spawn` | Agent spawn         | agent value                        |
| `MechGen.swarm_join`  | Swarm join          | swarm value                        |

### Type System

```mlir
// Primitive types
!MechGen.int<32>         // i32
!MechGen.uint<64>        // u64
!MechGen.float<64>       // f64
!MechGen.bool
!MechGen.str             // String
!MechGen.unit            // ()

// Compound types
!MechGen.vec<!MechGen.int<32>>            // [i32]~
!MechGen.option<!MechGen.str>             // ?s
!MechGen.result<!MechGen.str, !MechGen.err> // R[s, Error]
!MechGen.map<!MechGen.str, !MechGen.int<32>> // {s: i32}
!MechGen.box<!MechGen.str>                // ^s
!MechGen.rc<!MechGen.str>                 // $s
!MechGen.arc<!MechGen.str>                // @s
!MechGen.ref<mutable=false, !MechGen.str> // &s
!MechGen.ref<mutable=true, !MechGen.str>  // &!s
```

### Effect Attributes

```mlir
MechGen.func @read_file(%path: !MechGen.ref<mutable=false, !MechGen.str>)
    -> !MechGen.result<!MechGen.str, !MechGen.err>
    attributes { effects = ["io"] }
{
    // ...
}
```

## 6.3 Emitter: HIR → MechGen MLIR

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
        "MechGen.func {} @{}({}){} attributes {{ sym_visibility = \"{}\"{} }} {{",
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
                "i8"    => "!MechGen.int<8>",
                "i16"   => "!MechGen.int<16>",
                "i32"   => "!MechGen.int<32>",
                "i64"   => "!MechGen.int<64>",
                "u8"    => "!MechGen.uint<8>",
                "u32"   => "!MechGen.uint<32>",
                "u64"   => "!MechGen.uint<64>",
                "f32"   => "!MechGen.float<32>",
                "f64"   => "!MechGen.float<64>",
                "bool"  => "!MechGen.bool",
                "usize" => "!MechGen.uint<64>",
                _ => {
                    if type_args.is_empty() {
                        format!("!MechGen.named<\"{}\">", name)
                    } else {
                        let args: Vec<String> = type_args.iter()
                            .map(|a| self.mlir_type(a)).collect();
                        format!("!MechGen.named<\"{}\", {}>", name, args.join(", "))
                    }
                }
            }
        }
        Type::StringType => "!MechGen.str",
        Type::Vec { inner } => format!("!MechGen.vec<{}>", self.mlir_type(inner)),
        Type::Option { inner } => format!("!MechGen.option<{}>", self.mlir_type(inner)),
        Type::Result { ok, err } => format!(
            "!MechGen.result<{}, {}>", self.mlir_type(ok), self.mlir_type(err)
        ),
        Type::OwnedPtr { inner } => format!("!MechGen.box<{}>", self.mlir_type(inner)),
        Type::Arc { inner } => format!("!MechGen.arc<{}>", self.mlir_type(inner)),
        Type::Rc { inner } => format!("!MechGen.rc<{}>", self.mlir_type(inner)),
        Type::Reference { mutable, inner } => format!(
            "!MechGen.ref<mutable={}, {}>", mutable, self.mlir_type(inner)
        ),
        // ... other types
    }
}
```

## 6.4 Lowering Passes

After emitting the MechGen dialect, a series of MLIR passes lower it toward
LLVM:

```
MechGen MLIR
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

Converts MechGen type sugar to underlying representations:

| MechGen Type        | MLIR Lowering                      |
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

| MechGen Construct      | Lowered To                              |
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
- The `mg bench --estimate` command
- Agents via `rap.query("cost", func_id)`

## 6.6 Multi-Target Code Generation

MLIR enables targeting multiple backends from the same HIR:

| Target       | MLIR Path                           | Output         |
| ------------ | ----------------------------------- | -------------- |
| x86-64       | MechGen → LLVM dialect → LLVM x86     | Native binary  |
| AArch64      | MechGen → LLVM dialect → LLVM AArch64 | Native binary  |
| WASM         | MechGen → LLVM dialect → LLVM WASM    | `.wasm` module |
| GPU (NVIDIA) | MechGen → GPU dialect → NVVM          | PTX kernel     |
| GPU (AMD)    | MechGen → GPU dialect → ROCDL         | AMDGPU ISA     |
| SPIR-V       | MechGen → SPIR-V dialect              | Vulkan compute |

The `@pt(target)` annotation controls target selection:

```MechGen
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

    assert!(mlir.contains("MechGen.func public @add"));
    assert!(mlir.contains("!MechGen.int<32>"));
}
```

### Round-Trip Tests

For lowering passes, verify that lowering then re-raising preserves
semantics:

```bash
mg build --emit=mlir src/main.mg > before.mlir
mg mlir-pass --lower-sugar before.mlir > lowered.mlir
mg mlir-pass --roundtrip lowered.mlir > after.mlir
diff before.mlir after.mlir  # structural equivalence
```

### FileCheck Tests

MLIR provides FileCheck for pattern-matching test output:

```
// RUN: mg build --emit=mlir %s | FileCheck %s
// CHECK: MechGen.func public @main
// CHECK-SAME: -> !MechGen.result<!MechGen.unit, !MechGen.box<!MechGen.named<"Error">>>
// CHECK-SAME: effects = ["io"]
+f main() -> R[(), ^dyn Error] / io {
    p"hello"
}
```
