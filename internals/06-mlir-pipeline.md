# Chapter 6: MLIR Pipeline

The backend lowers typed, effect-checked HIR into MLIR (Multi-Level
Intermediate Representation), then progressively lowers through MLIR
dialects to LLVM IR and finally machine code.

---

## 6.1 Why MLIR?

Traditional compilers go directly from IR to LLVM IR. MAGE interposes MLIR
because:

1. **Multi-target**: MLIR has dialects for CPU (LLVM), GPU (GPU/SPIR-V),
   WASM, and custom accelerators. One HIR → many backends.
2. **Effect representation**: The MAGE MLIR dialect encodes effects as
   first-class attributes, enabling effect-aware optimizations.
3. **Cost oracle**: MLIR's analysis framework enables the cost oracle to
   estimate performance characteristics before codegen.
4. **Autotuning**: MLIR passes can generate variant code for performance
   autotuning (`@pa` annotation).

## 6.2 The MAGE MLIR Dialect

The MAGE dialect defines operations that map directly to MAGE language
constructs:

### Operations

| Op                  | Description         | Operands                           |
| ------------------- | ------------------- | ---------------------------------- |
| `MAGE.func`        | Function definition | name, params, return type, effects |
| `MAGE.call`        | Function call       | callee, args                       |
| `MAGE.struct_def`  | Struct definition   | name, fields                       |
| `MAGE.enum_def`    | Enum definition     | name, variants                     |
| `MAGE.trait_def`   | Trait definition    | name, methods                      |
| `MAGE.impl`        | Impl block          | type, trait, methods               |
| `MAGE.const`       | Constant definition | name, type, value                  |
| `MAGE.let`         | Variable binding    | name, type, value                  |
| `MAGE.match`       | Pattern match       | scrutinee, arms                    |
| `MAGE.for`         | For loop            | pattern, iterator, body            |
| `MAGE.effect`      | Effect marker       | effect set                         |
| `MAGE.handle`      | Handle block        | effects, body, handlers            |
| `MAGE.agent_spawn` | Agent spawn         | agent value                        |
| `MAGE.swarm_join`  | Swarm join          | swarm value                        |

### Type System

```mlir
// Primitive types
!MAGE.int<32>         // i32
!MAGE.uint<64>        // u64
!MAGE.float<64>       // f64
!MAGE.bool
!MAGE.str             // String
!MAGE.unit            // ()

// Compound types
!MAGE.vec<!MAGE.int<32>>            // [i32]~
!MAGE.option<!MAGE.str>             // ?s
!MAGE.result<!MAGE.str, !MAGE.err> // R[s, Error]
!MAGE.map<!MAGE.str, !MAGE.int<32>> // {s: i32}
!MAGE.box<!MAGE.str>                // ^s
!MAGE.rc<!MAGE.str>                 // $s
!MAGE.arc<!MAGE.str>                // @s
!MAGE.ref<mutable=false, !MAGE.str> // &s
!MAGE.ref<mutable=true, !MAGE.str>  // &!s
```

### Effect Attributes

```mlir
MAGE.func @read_file(%path: !MAGE.ref<mutable=false, !MAGE.str>)
    -> !MAGE.result<!MAGE.str, !MAGE.err>
    attributes { effects = ["io"] }
{
    // ...
}
```

## 6.3 Emitter: HIR → MAGE MLIR

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
        "MAGE.func {} @{}({}){} attributes {{ sym_visibility = \"{}\"{} }} {{",
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
                "i8"    => "!MAGE.int<8>",
                "i16"   => "!MAGE.int<16>",
                "i32"   => "!MAGE.int<32>",
                "i64"   => "!MAGE.int<64>",
                "u8"    => "!MAGE.uint<8>",
                "u32"   => "!MAGE.uint<32>",
                "u64"   => "!MAGE.uint<64>",
                "f32"   => "!MAGE.float<32>",
                "f64"   => "!MAGE.float<64>",
                "bool"  => "!MAGE.bool",
                "usize" => "!MAGE.uint<64>",
                _ => {
                    if type_args.is_empty() {
                        format!("!MAGE.named<\"{}\">", name)
                    } else {
                        let args: Vec<String> = type_args.iter()
                            .map(|a| self.mlir_type(a)).collect();
                        format!("!MAGE.named<\"{}\", {}>", name, args.join(", "))
                    }
                }
            }
        }
        Type::StringType => "!MAGE.str",
        Type::Vec { inner } => format!("!MAGE.vec<{}>", self.mlir_type(inner)),
        Type::Option { inner } => format!("!MAGE.option<{}>", self.mlir_type(inner)),
        Type::Result { ok, err } => format!(
            "!MAGE.result<{}, {}>", self.mlir_type(ok), self.mlir_type(err)
        ),
        Type::OwnedPtr { inner } => format!("!MAGE.box<{}>", self.mlir_type(inner)),
        Type::Arc { inner } => format!("!MAGE.arc<{}>", self.mlir_type(inner)),
        Type::Rc { inner } => format!("!MAGE.rc<{}>", self.mlir_type(inner)),
        Type::Reference { mutable, inner } => format!(
            "!MAGE.ref<mutable={}, {}>", mutable, self.mlir_type(inner)
        ),
        // ... other types
    }
}
```

## 6.4 Lowering Passes

After emitting the MAGE dialect, a series of MLIR passes lower it toward
LLVM:

```
MAGE MLIR
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

Converts MAGE type sugar to underlying representations:

| MAGE Type        | MLIR Lowering                      |
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

| MAGE Construct      | Lowered To                              |
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
| x86-64       | MAGE → LLVM dialect → LLVM x86     | Native binary  |
| AArch64      | MAGE → LLVM dialect → LLVM AArch64 | Native binary  |
| WASM         | MAGE → LLVM dialect → LLVM WASM    | `.wasm` module |
| GPU (NVIDIA) | MAGE → GPU dialect → NVVM          | PTX kernel     |
| GPU (AMD)    | MAGE → GPU dialect → ROCDL         | AMDGPU ISA     |
| SPIR-V       | MAGE → SPIR-V dialect              | Vulkan compute |

The `@pt(target)` annotation controls target selection:

```MAGE
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

    assert!(mlir.contains("MAGE.func public @add"));
    assert!(mlir.contains("!MAGE.int<32>"));
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
// CHECK: MAGE.func public @main
// CHECK-SAME: -> !MAGE.result<!MAGE.unit, !MAGE.box<!MAGE.named<"Error">>>
// CHECK-SAME: effects = ["io"]
+f main() -> R[(), ^dyn Error] / io {
    p"hello"
}
```
