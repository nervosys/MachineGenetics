# Redox Ecosystem Design

_Redox Language — Package Registry, Migration Tooling, IDE Integration, Agent Training, and Standard Library_

---

## Table of Contents

1. [Package Registry Architecture](#1-package-registry-architecture)
2. [Migration Tooling](#2-migration-tooling)
3. [IDE & Editor Integration](#3-ide--editor-integration)
4. [Agent Training & Data Formats](#4-agent-training--data-formats)
5. [Standard Library Design](#5-standard-library-design)
6. [Build System & Dependency Management](#6-build-system--dependency-management)
7. [Documentation & Learning Resources](#7-documentation--learning-resources)
8. [Community Infrastructure](#8-community-infrastructure)

---

## 1. Package Registry Architecture

### 1.1 Overview

The Redox package registry (**Forge**) serves as the central repository for Redox
packages (called **crates** for Rust compatibility, **modules** in Redox terminology).
Forge supports dual-format packages: native Redox (`.mg`) and transpiled Rust (`.rs`).

### 1.2 Registry Data Model

```
Module
├── name: String                     # e.g., "http-client"
├── version: SemVer                  # e.g., 1.3.0
├── source: ModuleSource
│   ├── rdx_files: [File]            # Native .mg source
│   ├── rs_files: [File]             # Optional Rust compatibility source
│   └── mlir_cache: ?[MlirArtifact]  # Pre-lowered MLIR artifacts
├── metadata: ModuleMetadata
│   ├── description: String
│   ├── license: String
│   ├── authors: [String]
│   ├── repository: ?Url
│   ├── keywords: [String]
│   ├── categories: [String]
│   ├── edition: RedoxEdition        # e.g., "2025"
│   └── rust_compatibility: ?RustVersion  # If transpilable to Rust
├── dependencies: [{name, version_req, features}]
├── skb_rules: [SkbRule]             # Package-specific safety rules
├── specs: [SpecBlock]               # Published API contracts
├── effects: [EffectDecl]            # Declared effect signatures
└── benchmarks: ?BenchmarkReport     # Performance envelope
```

### 1.3 Registry API

| Endpoint                                    | Method  | Description               |
| ------------------------------------------- | :-----: | ------------------------- |
| `/api/v1/modules`                           |   GET   | Search/list modules       |
| `/api/v1/modules/{name}`                    |   GET   | Module metadata           |
| `/api/v1/modules/{name}/{version}`          |   GET   | Specific version          |
| `/api/v1/modules/{name}/{version}/download` |   GET   | Download tarball          |
| `/api/v1/modules/{name}/{version}/mlir`     |   GET   | Pre-cached MLIR artifacts |
| `/api/v1/modules/{name}/{version}/skb`      |   GET   | SKB rules for this module |
| `/api/v1/modules/{name}/{version}/specs`    |   GET   | Published API contracts   |
| `/api/v1/modules/new`                       |   PUT   | Publish new module        |
| `/api/v1/modules/{name}/owners`             | GET/PUT | Ownership management      |
| `/api/v1/audit/{name}/{version}`            |   GET   | Security audit report     |

### 1.4 Compatibility with crates.io

Forge maintains a compatibility layer with crates.io:

| Feature                   | Mechanism                                                             |
| ------------------------- | --------------------------------------------------------------------- |
| **Import Rust crates**    | Auto-transpile on first use via `rust2mg`                            |
| **Publish to both**       | `forge publish --also-crates-io` generates `.rs` wrapper              |
| **Dependency resolution** | Unified resolver handles Rust + Redox deps                            |
| **Version mapping**       | Redox `u http.Client` resolves to crates.io `reqwest` via alias table |
| **FFI bridge**            | Rust crates used as-is through FFI binding generation (P45)           |

### 1.5 MLIR Artifact Caching

Forge stores pre-lowered MLIR artifacts to accelerate builds:

```
Artifact Cache Structure:
  module-name/1.3.0/
    ├── redox-dialect.mlir         # Redox dialect (highest level)
    ├── linalg-dialect.mlir        # After Linalg lowering
    ├── affine-dialect.mlir        # After Affine lowering
    ├── llvm-dialect.mlir          # After LLVM lowering
    ├── x86_64.o                   # Pre-compiled for x86-64
    ├── aarch64.o                  # Pre-compiled for AArch64
    └── wasm32.wasm                # Pre-compiled for WASM
```

Cache hit rate target: **> 95%** for published modules on common targets.
Cache invalidation: Triggered by dependency updates, compiler version bumps, or
SKB rule changes.

---

## 2. Migration Tooling

### 2.1 `rust2mg` — Rust-to-Redox Transpiler

Automated source-level translation from Rust to Redox.

#### 2.1.1 Translation Pipeline

```
Rust source (.rs)
    │
    ├─ (1) Parse with rustc → HIR
    ├─ (2) Extract ownership/lifetime info from borrow checker
    ├─ (3) Generate SKB rules from explicit annotations
    ├─ (4) Syntax transform: Rust → Redox canonical syntax
    ├─ (5) Verify: parse output with Redox parser
    └─ (6) Emit .mg files + module definition
```

#### 2.1.2 Translation Rules

| Rust Construct            | Redox Output                   | Confidence |
| ------------------------- | :----------------------------- | :--------: |
| `pub fn name(...)`        | `+f name(...)`                 |    100%    |
| `fn name(...)`            | `f name(...)`                  |    100%    |
| `let x = ...`             | `v x = ...`                    |    100%    |
| `let mut x = ...`         | `m x = ...`                    |    100%    |
| `struct Name { ... }`     | `S Name { ... }`               |    100%    |
| `enum Name { ... }`       | `E Name { ... }`               |    100%    |
| `trait Name { ... }`      | `T Name { ... }`               |    100%    |
| `impl Name { ... }`       | `I Name { ... }`               |    100%    |
| `use path::to::Item`      | `u path.to.Item`               |    100%    |
| `Vec<T>`                  | `[T]~`                         |    100%    |
| `Option<T>`               | `?T`                           |    100%    |
| `Result<T, E>`            | `R[T, E]`                      |    100%    |
| `Box<T>`                  | `^T`                           |    100%    |
| `Rc<T>`                   | `$T`                           |    100%    |
| `Arc<T>`                  | `@T`                           |    100%    |
| `HashMap<K, V>`           | `{K: V}`                       |    100%    |
| `&str`, `String`          | `s`                            |    98%     |
| `'a` lifetime annotations | (removed, SKB rule generated)  |    95%     |
| `unsafe { ... }`          | (removed, capability attached) |    90%     |
| `where T: Bound`          | (removed or simplified)        |    92%     |
| `#[derive(...)]`          | `@d(...)`                      |    100%    |
| `#[inline]`               | `@i`                           |    100%    |
| `println!()`              | `p"..."`                       |    100%    |
| `format!()`               | `f"..."`                       |    100%    |
| `async fn`                | `af`                           |    100%    |
| `if let Some(x) = val`    | `?val => x`                    |    95%     |
| Complex pattern matching  | Context-dependent              |    85%     |

#### 2.1.3 CLI Interface

```
rust2mg [OPTIONS] <INPUT>

Arguments:
  <INPUT>            Rust source file or directory

Options:
  --output, -o       Output directory (default: ./rdx/)
  --verify           Parse output with Redox parser (default: on)
  --preserve-unsafe  Keep unsafe blocks as @unsafe annotations
  --generate-skb     Emit SKB rules from lifetime annotations
  --diff             Show side-by-side diff instead of writing files
  --crate            Process entire Cargo.toml workspace
  --dry-run          Show what would change without writing
  --stats            Print token count comparison
```

### 2.2 `mg2rs` — Redox-to-Rust Back-Transpiler

For interoperability, Redox code can be transpiled back to Rust:

| Redox Construct        | Rust Output               | Notes                     |
| ---------------------- | :------------------------ | ------------------------- |
| `+f name(...)`         | `pub fn name(...)`        | Direct                    |
| `v x = ...`            | `let x = ...`             | Direct                    |
| `[T]~`                 | `Vec<T>`                  | Direct                    |
| `?T`                   | `Option<T>`               | Direct                    |
| `R[T, E]`              | `Result<T, E>`            | Direct                    |
| SKB-inferred lifetimes | Explicit `'a` annotations | Reconstructed from SKB    |
| Effect declarations    | `// effect: io` comment   | No Rust equivalent        |
| Spec blocks            | `// spec: ...` comment    | Optional debug assertions |

### 2.3 Migration Workflow

```
Phase 1: Analyze
  $ rust2mg --stats my_crate/
  → Report: 5,000 LOC Rust → ~2,350 LOC Redox (53% reduction)
  → 142 lifetime annotations → 0
  → 23 unsafe blocks → 0 (12 become capabilities, 11 become SKB rules)

Phase 2: Translate
  $ rust2mg --crate my_crate/ -o my_crate_rdx/
  → Generated 45 .mg files
  → Generated 23 SKB rules
  → 100% parse validation passed

Phase 3: Verify
  $ redox check my_crate_rdx/
  → All specs satisfied
  → SKB coverage: 98.2%
  → 3 items need manual review (complex unsafe patterns)

Phase 4: Test
  $ redox test my_crate_rdx/
  → 234/234 tests pass (transpiled from Rust test suite)
```

---

## 3. IDE & Editor Integration

### 3.1 RAP (Redox Agent Protocol) — Language Server

The RAP server provides IDE features over JSON-RPC (see prototype/src/rap.rs):

| Feature          | RAP Method             | Response                       |
| ---------------- | ---------------------- | ------------------------------ |
| Parse            | `language/parse`       | AST (JSON)                     |
| Tokenize         | `language/tokens`      | Token stream with spans        |
| Check            | `build/check`          | Diagnostic list                |
| Hover            | `language/hover`       | Type info, doc, cost estimate  |
| Completion       | `language/completion`  | Context-aware completions      |
| Go to Definition | `language/definition`  | Source location                |
| Find References  | `language/references`  | Location list                  |
| Rename           | `language/rename`      | Workspace edit                 |
| Format           | `language/format`      | Formatted source               |
| Inlay Hints      | `language/inlayHints`  | Inferred types, costs, effects |
| Code Actions     | `language/codeActions` | Quick fixes, refactorings      |
| SKB Query        | `skb/query`            | Applicable safety rules        |
| Effect View      | `language/effects`     | Effect tree for function       |
| Cost View        | `language/cost`        | Cost oracle data for scope     |
| Swarm Status     | `swarm/status`         | Agent activity view            |

### 3.2 VS Code Extension

```
redox-vscode/
├── package.json              # Extension manifest
├── syntaxes/
│   └── redox.tmLanguage.json # TextMate grammar for syntax highlighting
├── language-configuration.json
├── src/
│   ├── extension.ts          # Extension entry point
│   ├── rap-client.ts         # RAP protocol client
│   ├── cost-view.ts          # Inline cost annotations
│   ├── effect-tree.ts        # Effect visualization panel
│   ├── skb-explorer.ts       # SKB rule browser
│   └── swarm-panel.ts        # Swarm orchestration dashboard
└── media/
    └── icons/                # Redox-themed icons
```

#### Key features:

- **Inline cost annotations**: Shows `// ⏱ 12 ns (amort.)` next to operations.
- **Effect gutter icons**: Color-coded indicators for function effect sets.
- **SKB rule hover**: Hover over any construct to see which SKB rules apply.
- **Swarm dashboard**: Live view of agent activity when running multi-agent builds.
- **One-click migration**: Right-click `.rs` file → "Convert to Redox".

### 3.3 Other Editor Support

| Editor   | Mechanism                     | Priority |
| -------- | ----------------------------- | :------: |
| VS Code  | Native extension (RAP client) |    P0    |
| Neovim   | RAP via nvim-lspconfig        |    P0    |
| Helix    | RAP via native LSP            |    P1    |
| Zed      | RAP via extension API         |    P1    |
| IntelliJ | RAP plugin                    |    P2    |
| Emacs    | RAP via lsp-mode / eglot      |    P2    |

### 3.4 Syntax Highlighting

TextMate grammar scopes for Redox:

| Scope                        | Constructs                                        |
| ---------------------------- | ------------------------------------------------- |
| `keyword.declaration.redox`  | `f`, `m`, `v`, `c`, `S`, `E`, `T`, `I`, `M`, `u`  |
| `keyword.control.redox`      | `loop`, `break`, `continue`, `ret`, `yield`       |
| `keyword.operator.redox`     | `?` (if/match), `@` (for/attr/struct), `:` (else) |
| `keyword.other.redox`        | `effect`, `handle`, `spec`, `type`, `static`      |
| `entity.name.function.redox` | Function names after `f`/`+f`/`~f`                |
| `entity.name.type.redox`     | Type names after `S`/`E`/`T`                      |
| `storage.modifier.redox`     | `+` (pub), `~` (crate), `-` (private)             |
| `string.quoted.double.redox` | `"..."`                                           |
| `string.interpolated.redox`  | `f"...{expr}..."`                                 |
| `string.print.redox`         | `p"...{expr}..."`                                 |
| `comment.line.redox`         | `// ...`                                          |
| `comment.block.redox`        | `/* ... */`                                       |
| `variable.parameter.redox`   | Function parameter names                          |
| `support.type.redox`         | `s`, `i32`, `f64`, `u8`, `bool`                   |
| `punctuation.sigil.redox`    | `^`, `$`, `@`, `?`, `~`, `&!`                     |

---

## 4. Agent Training & Data Formats

### 4.1 Training Data Format

Redox defines a standard format for AI agent training data:

```json
{
  "format": "redox-training-v1",
  "samples": [
    {
      "id": "sample-001",
      "task": "Implement a concurrent web scraper",
      "context": {
        "imports": ["u http.Client", "u async.spawn"],
        "existing_code": "S Config { urls: [s]~, max_concurrent: u32 }",
        "constraints": ["max memory: 256 MB", "timeout: 30s per URL"]
      },
      "solution": {
        "rdx_source": "+f scrape(config: &Config) -> R[[Response]~, Error] { ... }",
        "token_count": 45,
        "effects": ["io", "net", "async"],
        "skb_rules_used": ["borrow:shared-iter", "async:spawn-join"],
        "cost_profile": { "latency_p99": "2.3s", "memory_peak": "128 MB" }
      },
      "rust_equivalent": {
        "rs_source": "pub async fn scrape(config: &Config) -> Result<Vec<Response>, Box<dyn Error>> { ... }",
        "token_count": 89,
        "compilation_errors_on_first_try": 3,
        "iterations_to_correct": 4
      }
    }
  ]
}
```

### 4.2 Agent Instruction Format

Standard prompts for AI agents working with Redox:

```yaml
# .redox/agent-instructions.yaml
language: redox
edition: "2025"

syntax_rules:
  - "Use f/+f for function definitions (not fn/pub fn)"
  - "Use v for immutable bindings, m for mutable (not let/let mut)"
  - "Use S/E/T/I for struct/enum/trait/impl (not full keywords)"
  - "Use ? for if/match, @ for for loops (not if/match/for keywords)"
  - "Use [T]~ for Vec, ?T for Option, R[T,E] for Result"
  - "Use ^T for Box, $T for Rc, @T for Arc"
  - "Use {K:V} for HashMap, [T] for slices"
  - "Use & for shared ref, &! for exclusive ref (not &mut)"
  - "Omit lifetime annotations (SKB handles them)"
  - "Omit unsafe blocks (use capability system)"
  - "Use u for imports with dot-separated paths"
  - "Use f\"...\" for format strings, p\"...\" for print"

effect_rules:
  - "Declare effects in function signatures when non-pure"
  - "Pure functions need no annotation"
  - "Use handle blocks for effect handling"

contract_rules:
  - "Use spec blocks for pre/post conditions"
  - "Use @req for preconditions, @ens for postconditions"
  - "Use @inv for invariants"

style:
  indent: 4
  max_line_length: 100
  trailing_comma: true
  brace_style: same_line
```

### 4.3 Benchmark Corpus

A standardized set of 100 programming tasks for evaluating Redox agent performance:

| Category                |  Tasks  | Token Range | Complexity |
| ----------------------- | :-----: | :---------: | :--------: |
| Basic I/O               |   10    |    20–50    |   Simple   |
| Data structures         |   15    |   50–150    |   Medium   |
| Algorithms              |   15    |   80–200    |   Medium   |
| Concurrency             |   10    |   100–300   |    Hard    |
| Web/Network             |   10    |   150–400   |    Hard    |
| Systems programming     |   10    |   200–500   |    Hard    |
| Agent orchestration     |   10    |   300–800   | Very Hard  |
| Full applications       |   10    |  500–2000   | Very Hard  |
| Error handling patterns |    5    |   50–150    |   Medium   |
| Generic/trait design    |    5    |   100–300   |    Hard    |
| **Total**               | **100** |      —      |     —      |

Each task includes:
- Natural language description
- Expected Redox solution (reference)
- Equivalent Rust solution (baseline)
- Token count for both
- Expected effects and SKB rules
- Test suite (input/output pairs)

### 4.4 Agent Evaluation Metrics

| Metric                      | Definition                              | Target  |
| --------------------------- | --------------------------------------- | :-----: |
| **First-Pass Success Rate** | % of tasks correct on first generation  |  > 95%  |
| **Token Efficiency**        | Agent tokens / reference tokens         |  < 1.1  |
| **Effect Correctness**      | % of correctly declared effects         |  > 99%  |
| **Spec Compliance**         | % of generated code satisfying specs    |  > 98%  |
| **Migration Accuracy**      | % of Rust→Redox translations that parse |  > 99%  |
| **Iteration Count**         | Average roundtrips to correct code      |  < 1.5  |
| **Cost/Task**               | API tokens consumed per benchmark task  | < $0.05 |

---

## 5. Standard Library Design

### 5.1 Module Hierarchy

```
std
├── io              # File I/O, streams, buffering
│   ├── Read, Write, Seek    # Core traits
│   ├── BufReader, BufWriter # Buffered wrappers
│   ├── stdin, stdout, stderr
│   └── File
├── net             # Networking
│   ├── TcpStream, TcpListener
│   ├── UdpSocket
│   ├── http        # HTTP client/server
│   └── dns         # DNS resolution
├── fs              # File system
│   ├── read, write, create, remove
│   ├── metadata, permissions
│   └── walk        # Directory traversal
├── col             # Collections
│   ├── Map ({K:V}) # HashMap
│   ├── Set ({K})   # HashSet
│   ├── BTree       # Ordered map
│   ├── VecDeque    # Double-ended queue
│   └── LinkedList  # Doubly-linked list
├── sync            # Synchronization
│   ├── Mutex, RwLock
│   ├── Channel     # MPSC/MPMC channels
│   ├── Barrier, Semaphore
│   └── Atomic      # Atomic primitives
├── async           # Async runtime
│   ├── spawn, join
│   ├── select, race
│   ├── sleep, timeout
│   └── Stream      # Async iterator
├── fmt             # Formatting
│   ├── Display, Debug
│   ├── format      # Format string engine
│   └── print, println, eprint
├── str             # String utilities
│   ├── split, join, trim
│   ├── regex       # Regular expressions
│   └── encode      # UTF-8/16/32 conversion
├── math            # Mathematics
│   ├── trig, exp, log
│   ├── random      # RNG
│   └── simd        # SIMD operations
├── time            # Date/time
│   ├── Instant, Duration
│   ├── SystemTime
│   └── format      # strftime-style
├── json            # JSON (first-class)
│   ├── parse, stringify
│   ├── Value       # Dynamic JSON
│   └── Serialize, Deserialize  # Traits
├── env             # Environment
│   ├── args, vars
│   ├── current_dir
│   └── home_dir
├── process         # Process management
│   ├── Command
│   ├── exit
│   └── signal
├── agent           # Agent primitives (Redox-unique)
│   ├── Agent       # Agent trait
│   ├── Swarm       # Swarm orchestration
│   ├── Message     # Inter-agent message
│   ├── Lease       # Capability lease
│   ├── Region      # Code region
│   └── Bus         # Swarm bus client
├── skb             # Safety Knowledge Base
│   ├── Rule        # Rule definition
│   ├── query       # SKB-QL interface
│   └── validate    # Inline validation
├── effect          # Effect system
│   ├── Effect      # Effect trait
│   ├── handle      # Handler construction
│   └── perform     # Effect invocation
├── spec            # Contract system
│   ├── require, ensure
│   ├── invariant
│   └── verify      # Runtime verification
└── test            # Testing
    ├── assert, assert_eq
    ├── bench        # Benchmarking harness
    └── prop         # Property-based testing
```

### 5.2 Key Design Decisions

| Decision                               | Rationale                                                       |
| -------------------------------------- | --------------------------------------------------------------- |
| **JSON as first-class** (`std.json`)   | AI agents communicate via JSON; eliminate external dependency   |
| **HTTP in stdlib** (`std.net.http`)    | ~80% of agent tasks involve HTTP; reduce boilerplate            |
| **Regex in stdlib** (`std.str.regex`)  | Common enough to include; avoid version fragmentation           |
| **Agent primitives** (`std.agent`)     | Core language feature; must be in stdlib                        |
| **Property testing** (`std.test.prop`) | Encourages high-quality testing by default                      |
| **No `std::marker`**                   | Send/Sync are auto-derived via SKB; no manual markers needed    |
| **No `std::borrow`**                   | Borrowing is compiler-managed; no `Cow` or `ToOwned` needed     |
| **No `std::mem`**                      | Memory management is compiler-managed; no `std::mem::swap` etc. |

### 5.3 Naming Conventions

| Rust                        | Redox          | Rationale                  |
| --------------------------- | -------------- | -------------------------- |
| `std::collections::HashMap` | `std.col.Map`  | Shorter path, simpler name |
| `std::sync::Arc`            | `@T` (sigil)   | Built into type syntax     |
| `std::option::Option`       | `?T` (sigil)   | Built into type syntax     |
| `std::result::Result`       | `R[T, E]`      | Built into type syntax     |
| `std::vec::Vec`             | `[T]~` (sigil) | Built into type syntax     |
| `std::string::String`       | `s` (keyword)  | First-class type           |
| `std::io::Read`             | `std.io.Read`  | Dot-separated paths        |

---

## 6. Build System & Dependency Management

### 6.1 `Forge.toml` — Project Configuration

```toml
[module]
name = "my-project"
version = "0.1.0"
edition = "2025"
description = "A fast web scraper"
license = "MIT"

[dependencies]
http = "1.0"
json = "1.0"
async-rt = { version = "0.5", features = ["multi-thread"] }

[dev-dependencies]
test-utils = "0.3"

[build]
target = ["x86_64", "aarch64", "wasm32"]    # Multi-target by default
mlir-cache = true                            # Cache MLIR artifacts
parallel = true                              # Parallel compilation

[safety]
mode = "skb-only"                 # none | skb-only | warnings | full
profile = "agent-dev"             # agent-dev | human-dev | ci-pipeline | production

[agent]
swarm-size = 4                    # Default swarm size for builds
consensus = "majority"            # Consensus strategy
lease-timeout = "5m"              # Default lease timeout
```

### 6.2 CLI — `rdx` Command

```
rdx <COMMAND>

Commands:
  new <name>          Create a new Redox project
  init                Initialize Redox in existing directory
  build               Compile the project
  check               Type-check without codegen
  test                Run tests
  bench               Run benchmarks
  run                 Build and run
  fmt                 Format source code
  lint                Run linter
  doc                 Generate documentation
  publish             Publish to Forge registry
  install             Install a binary from Forge
  update              Update dependencies
  migrate             Run rust2mg on a Rust project
  rap                 Start RAP language server
  swarm               Manage agent swarms
  skb                 Query/manage Safety Knowledge Base
  cost                Show cost oracle data for a function
  spec                Verify spec/contract compliance

Global Options:
  --edition <YEAR>    Override edition
  --target <TRIPLE>   Override build target
  --release           Build with optimizations
  --verbose, -v       Verbose output
  --quiet, -q         Suppress output
  --jobs, -j <N>      Parallel jobs
```

### 6.3 Dependency Resolution

Forge uses a SAT-based dependency resolver (like Cargo) with extensions:

| Feature                    | Description                                       |
| -------------------------- | ------------------------------------------------- |
| **Dual-source resolution** | Resolve Redox modules and Rust crates together    |
| **MLIR artifact matching** | Prefer cached MLIR artifacts over source builds   |
| **Effect compatibility**   | Reject dependencies with incompatible effect sets |
| **SKB rule merging**       | Merge package-level SKB rules into project SKB    |
| **Spec verification**      | Verify dependency API contracts at resolve time   |

---

## 7. Documentation & Learning Resources

### 7.1 Documentation Tiers

| Tier                | Content                 | Format                | Audience         |
| ------------------- | ----------------------- | --------------------- | ---------------- |
| **Quick Start**     | 10-min tutorial         | Interactive web       | New users        |
| **Book**            | Comprehensive guide     | mdBook site           | All users        |
| **Reference**       | Formal language spec    | REDOX_SPEC.md         | Language lawyers |
| **API Docs**        | Standard library docs   | Generated from source | Developers       |
| **Cookbook**        | Recipe-style examples   | Searchable web        | Practitioners    |
| **Agent Guide**     | Agent-specific patterns | Structured prompts    | AI agents        |
| **Migration Guide** | Rust → Redox            | Step-by-step          | Rust developers  |
| **Internals**       | Compiler architecture   | Technical docs        | Contributors     |

### 7.2 Auto-Generated Documentation

The `rdx doc` command generates documentation from:

1. **Source comments** (`///` and `//!`)
2. **Spec blocks** — formatted as API contracts
3. **Effect signatures** — shown as side-effect documentation
4. **Cost oracle data** — performance characteristics
5. **SKB rules** — safety guarantees

Example generated documentation:

```markdown
## `+f sort[T: Ord](data: &![T]~)`

Sorts a vector in place using an adaptive merge sort.

### Effects
- None (pure function)

### Contract
- **Requires**: `data.len() > 0`
- **Ensures**: `data` is sorted in ascending order
- **Ensures**: `data.len()` is unchanged (no elements added/removed)

### Performance
- **Time**: O(n log n) average, O(n) best case (pre-sorted)
- **Space**: O(n) auxiliary
- **Cost**: ~45 ns per element (amortized, x86-64)

### Safety
- SKB rule `borrow:exclusive-sort`: Exclusive borrow prevents data races
- SKB rule `lifetime:in-place-mutation`: No dangling references
```

---

## 8. Community Infrastructure

### 8.1 Governance

| Role                 | Responsibility                        |
| -------------------- | ------------------------------------- |
| **Core Team**        | Language design, compiler development |
| **SKB Curators**     | Safety Knowledge Base rule curation   |
| **Forge Moderators** | Package registry quality control      |
| **RFC Authors**      | Design proposals (Redox RFC process)  |
| **ACI Trainers**     | AI Coding Intelligence model training |

### 8.2 Contribution Workflow

```
1. Discuss    → GitHub Discussions or Discord
2. Propose    → RFC (rdx-rfcs repository)
3. Prototype  → Branch on rdx-compiler
4. Review     → Pull request with CI (build + test + benchmark)
5. Merge      → After core team approval
6. Release    → Included in next edition
```

### 8.3 CI/CD Pipeline

| Stage              | Action                    | Duration Target |
| ------------------ | ------------------------- | :-------------: |
| Lint               | `rdx lint`                |      < 5s       |
| Format Check       | `rdx fmt --check`         |      < 2s       |
| Type Check         | `rdx check`               |      < 30s      |
| Test               | `rdx test`                |      < 60s      |
| Spec Verify        | `rdx spec --verify`       |      < 30s      |
| SKB Validate       | `rdx skb --validate`      |      < 10s      |
| Benchmark          | `rdx bench --compare`     |     < 120s      |
| Multi-target Build | `rdx build --all-targets` |     < 180s      |
| Publish            | `rdx publish` (on tag)    |      < 30s      |

### 8.4 Interoperability Ecosystem

```
                    ┌─────────────┐
                    │   Forge     │  (Package Registry)
                    │  Registry   │
                    └──────┬──────┘
                           │
    ┌──────────┬───────────┼───────────┬──────────┐
    │          │           │           │          │
┌───┴───┐ ┌───┴───┐ ┌─────┴─────┐ ┌──┴───┐ ┌───┴───┐
│ VS    │ │ Neovim│ │ rust2mg  │ │ RAP  │ │ Swarm │
│ Code  │ │ Helix │ │ mg2rs    │ │Server│ │ CLI   │
│Plugin │ │ Conf  │ │ Migration │ │      │ │       │
└───────┘ └───────┘ └───────────┘ └──────┘ └───────┘
    │          │           │           │          │
    └──────────┴───────────┴───────────┴──────────┘
                           │
                    ┌──────┴──────┐
                    │  rdx CLI    │
                    │ (build/test │
                    │  /run/doc)  │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
        ┌─────┴────┐ ┌────┴────┐ ┌─────┴────┐
        │  MLIR    │ │  SKB    │ │  ACI     │
        │ Pipeline │ │ Engine  │ │ Engine   │
        └──────────┘ └─────────┘ └──────────┘
```

### 8.5 Versioning & Editions

| Edition | Year  | Key Features                                                   |
| ------- | :---: | -------------------------------------------------------------- |
| 2025    | 2025  | Core language, basic SKB, LL(1) parser                         |
| 2026    | 2026  | Full MLIR pipeline, effect system, agent primitives            |
| 2027    | 2027  | Full ACI, Cost Oracle, hot-reload, swarm orchestration         |
| 2028+   | 2028+ | Self-hosting compiler, advanced synthesis, formal verification |

Edition migration is automated via `rdx migrate --edition 2026`, which applies
syntax and semantic changes with full backward compatibility guarantees.
