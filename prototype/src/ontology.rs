//! Complete ontology over the MAGE language, the Agentic Binary Language IR, and the
//! RAP protocol surface. Built so an autonomous agent can discover
//! "what exists" without prior training on this codebase.
//!
//! Sections:
//!
//! - **`sigils`** — MAGE sigil → meaning table (handwritten, lives
//!   next to `lexer.rs` comments which are authoritative)
//! - **`keywords`**   — Reserved words and what they introduce
//! - **`ast_kinds`**  — Top-level AST node families an agent will encounter
//! - **`ir_ops`**     — Every `rmi::lang::Op` (programmatically from
//!   `Op::ALL` + `OpMeta`)
//! - **`op_families`** — The 7 `OpFamily` buckets and their semantics
//! - **`layer_map`**  — Surface layer name → opcode (from
//!   `abl_bridge::layer_name_to_op`)
//! - **`rap_methods`** — Protocol method catalog with input / output keys
//! - **`heal_patterns`** — Mechanical heal patterns (from
//!   `heal::pattern_names`)
//! - **`recovery_stages`** — The 4-stage recovery pipeline + agent.refine
//! - **`abl`**       — Binary IR container format constants
//!
//! Single entry point: [`build`] returns a `serde_json::Value`. The RAP
//! method `ontology/full` calls it; `ontology/section` returns just one
//! key from the same object.

use rmi::lang::Op;

/// Static sigil table. Keep in sync with `lexer.rs` token-kind comments.
/// Format: `(sigil, ast_kind, summary)`.
const SIGILS: &[(&str, &str, &str)] = &[
    ("+f", "Function", "pub fn — public function declaration"),
    ("+af", "Function", "pub async fn"),
    ("+uf", "Function", "pub unsafe fn"),
    ("f", "Function", "fn — private function"),
    ("v", "Let", "let — immutable binding"),
    ("m", "Let", "let mut — mutable binding"),
    ("C", "Const", "const declaration"),
    ("Z", "Static", "static declaration"),
    ("S", "Struct", "struct declaration"),
    ("E", "Enum", "enum declaration"),
    ("T", "Trait", "trait declaration"),
    ("I", "Impl", "impl block"),
    ("M", "Mod", "module declaration"),
    ("u", "Use", "use import"),
    ("Y", "TypeAlias", "type alias"),
    ("?", "If/Option/Try", "if expression, or Option type prefix, or try operator"),
    ("?=", "Match", "match expression"),
    ("?:", "If", "human-mode if (sugar for ?)"),
    ("@", "For/Attr/Arc", "for loop, attribute, struct literal, or Arc type"),
    ("@@", "Loop", "infinite loop"),
    ("@w", "While", "while loop"),
    ("&T", "RefType", "shared reference type"),
    ("&!T", "MutRefType", "mutable reference type"),
    ("@T", "ArcType", "Arc<T> shared ownership"),
    ("?T", "OptionType", "Option<T> sugar"),
    ("R[T,E]", "ResultType", "Result<T, E>"),
    ("[T]", "Slice", "slice type"),
    ("[T]~", "Vec", "Vec<T>"),
    ("s", "Str", "string type"),
    ("1b", "Bool", "true"),
    ("0b", "Bool", "false"),
    ("_", "SelfVal", "self value"),
    ("_T", "SelfType", "Self type"),
    // `ret`, not `^`. This entry said `^` and it was wrong: `MAGE_SPEC.md`
    // names `ret` as the Return sigil in both of its sigil tables, and
    // mentions `^` exactly once — as bitwise XOR. `^` is also the `^T` Box
    // type prefix, so it was double-booked before anyone proposed a third
    // meaning. Nothing but this line ever claimed `^` returns.
    ("ret", "Return", "return expression"),
    ("!", "Break/Not", "break, or logical not, or assert"),
    (">>", "Continue", "continue"),
    ("??", "Todo", "todo!()"),
    ("???", "Unimplemented", "unimplemented!()"),
];

/// Curated summaries for notable keywords. NOT the keyword list itself — the
/// authoritative spelling set is `crate::lexer::KEYWORDS` (the same table the
/// lexer uses), which `keywords_section` enumerates in full so the ontology is
/// complete and can never drift. This map only adds prose where it helps; any
/// keyword without an entry still appears, with a generated summary.
///
/// The middle column is **not** published. It used to overwrite the emitted
/// `introduces` field, which made that field mean two different things: the
/// real token kind for the 83 keywords with no entry here, and a hand-written
/// name for the 19 with one — `agent` claimed `AgentDef`, `match` claimed
/// `Match` (it is `QuestionEq`), and `val`/`var` claimed `Let`, a token this
/// language removed and now rejects on sight. An agent could not tell which
/// kind of answer it was reading, and could not join the field against
/// anything. `introduces` is now always the token the spelling produces, and
/// `keywords_introduce_the_token_they_claim` holds it to that.
const KEYWORD_DOCS: &[(&str, &str, &str)] = &[
    ("net", "NetDef", "neural network definition; lowers to Agentic Binary Language"),
    ("kb", "KbDef", "symbolic knowledge base; lowers to Agentic Binary Language"),
    ("agent", "AgentDef", "agent role; lowers to Agentic Binary Language agent ops"),
    ("swarm", "SwarmDef", "agent swarm topology; lowers to Agentic Binary Language"),
    ("train", "TrainDef", "training pipeline; lowers to Agentic Binary Language compute"),
    ("evolve", "EvolveDef", "evolutionary search; lowers to Agentic Binary Language meta"),
    ("layer", "Layer", "neural-net layer inside a net block"),
    ("forward", "Forward", "forward pass inside a net block"),
    ("effect", "Effect", "effect declaration"),
    ("handle", "Handle", "effect handler"),
    ("spec", "Spec", "spec/contract block"),
    ("extern", "Extern", "FFI block"),
    ("val", "Let", "immutable binding (also `v`)"),
    ("var", "Let", "mutable binding (also `m`)"),
    ("data", "Data", "record or sum type (also `D`)"),
    ("extend", "Extend", "inherent/trait methods on a type (also `xd`)"),
    ("match", "Match", "pattern match (sigil `?`)"),
    ("guard", "Guard", "early-exit guard (also `gd`)"),
    ("defer", "Defer", "run expression on scope exit (also `df`)"),
];

/// Built-in type catalog. Covers scalar types, the composite type
/// constructors, and the standard sigil shorthand. An agent reading
/// this knows the full set of type names it can write without
/// importing anything.
///
/// Columns: `(name, category, summary)`.
const TYPES: &[(&str, &str, &str)] = &[
    // ── scalar integers ─────────────────────────────────────────────
    ("i8",   "scalar", "signed 8-bit integer"),
    ("i16",  "scalar", "signed 16-bit integer"),
    ("i32",  "scalar", "signed 32-bit integer (default int)"),
    ("i64",  "scalar", "signed 64-bit integer"),
    ("isize","scalar", "pointer-sized signed integer"),
    ("u8",   "scalar", "unsigned 8-bit (also: byte)"),
    ("u16",  "scalar", "unsigned 16-bit integer"),
    ("u32",  "scalar", "unsigned 32-bit integer"),
    ("u64",  "scalar", "unsigned 64-bit integer"),
    ("usize","scalar", "pointer-sized unsigned integer"),
    // ── scalar floats / boolean / char ─────────────────────────────
    ("f32",  "scalar", "32-bit IEEE-754 float"),
    ("f64",  "scalar", "64-bit IEEE-754 float (default float)"),
    ("bool", "scalar", "boolean; literals `1b` (true) / `0b` (false)"),
    ("char", "scalar", "Unicode scalar value"),
    // ── string / unit ──────────────────────────────────────────────
    ("s",    "string", "string slice (Rust `&str`); sigil shorthand"),
    ("String","string", "owned string (Rust `String`). `S` is *not* a spelling of it — `S` is the `struct` keyword."),
    ("()",   "unit",   "unit type; sole value `()`; implicit fn return"),
    // ── reference / pointer sigils ─────────────────────────────────
    ("&T",   "ref",    "shared reference (Rust `&T`)"),
    ("&!T",  "ref",    "mutable reference (Rust `&mut T`)"),
    ("@T",   "smart",  "Arc<T>; shared atomic ownership"),
    // ── composite type constructors ────────────────────────────────
    ("?T",      "option", "Option<T>; sigil shorthand"),
    ("R[T,E]",  "result", "Result<T, E>; sigil shorthand"),
    ("[T]",     "slice",  "slice type (fixed-view, Rust `&[T]`)"),
    ("[T; N]",  "array",  "fixed-size array (length N at compile time)"),
    ("[T]~",    "vec",    "Vec<T>; owned dynamic array; sigil shorthand"),
    ("(T1,T2,...)", "tuple", "tuple type; positional fields"),
    ("{K: V}",  "map",    "HashMap<K, V>; standard map type. Written with braces, not `Map[K,V]`."),
    ("{T}",     "set",    "HashSet<T>; standard set type. Written with braces, not `Set[T]`."),
    ("Box[T]",  "smart",  "Box<T>; heap-owned single value"),
    // ── function type ──────────────────────────────────────────────
    ("f(T)->R", "fn",     "function pointer type; (T) -> R signature"),
];

/// AST top-level item kinds an agent should know it can find in a module.
const AST_KINDS: &[(&str, &str)] = &[
    ("Function", "Function definition (incl. async / unsafe variants)"),
    ("Struct", "Struct definition"),
    ("Enum", "Enum definition"),
    ("Trait", "Trait definition"),
    ("Impl", "impl block"),
    ("Mod", "Module declaration"),
    ("Use", "use import"),
    ("Const", "const declaration"),
    ("Static", "static declaration"),
    ("TypeAlias", "type alias"),
    ("NetDef", "AI: neural network (Agentic Binary Language-routed)"),
    ("KbDef", "AI: symbolic knowledge base (Agentic Binary Language-routed)"),
    ("AgentDef", "AI: agent role (Agentic Binary Language-routed)"),
    ("SwarmDef", "AI: swarm topology (Agentic Binary Language-routed)"),
    ("TrainDef", "AI: training pipeline (Agentic Binary Language-routed)"),
    ("EvolveDef", "AI: evolutionary search (Agentic Binary Language-routed)"),
    ("EffectDef", "Effect declaration"),
    ("SpecBlock", "Spec / contract block"),
];

/// The 7 op-family buckets. Aligned with `rmi::lang::OpFamily`.
const OP_FAMILIES: &[(&str, u8, &str)] = &[
    ("Neural", 0x00, "Differentiable neural-network operations"),
    ("Symbolic", 0x01, "Symbolic reasoning: unification, inference, planning"),
    ("Control", 0x02, "Control flow: seq, par, cond, loop"),
    ("Memory", 0x03, "Memory management: alloc, reshape, transpose"),
    ("Agent", 0x04, "Agent communication: send, recv, spawn, delegate"),
    ("Meta", 0x05, "Meta / introspection: hash, typeof, mutate, decompose"),
    ("Math", 0x06, "Elementwise math: add, mul, exp, log, sin, ..."),
];

/// Canonical layer surface names that lower to an `Op`. Pulled from the
/// authoritative mapping in `abl_bridge::layer_name_to_op`.
const LAYER_SURFACE_NAMES: &[&str] = &[
    "Linear", "Conv2D", "Attention", "Embed", "Dropout", "Softmax",
    "ReLU", "GELU", "SiLU", "Sigmoid", "Tanh", "Mish", "Softplus",
    "LayerNorm", "BatchNorm", "RMSNorm", "GroupNorm", "InstanceNorm",
    "MaxPool", "AvgPool", "GlobalAvgPool",
    "Unify", "Resolve", "Infer", "Plan",
    "Send", "Recv", "Spawn", "Delegate",
    "Hash", "Typeof",
];

/// Catalog of RAP methods. Pairs each method name with a short summary
/// and the input / output JSON keys an agent should expect. The truth
/// for behavior is the `dispatch` match in `rap.rs`; this is the
/// agent-discoverable summary.
const RAP_METHODS: &[(&str, &str, &[&str], &[&str])] = &[
    ("language/parse", "Parse source to AST (JSON).",
        &["source"], &["ok", "ast", "error"]),
    ("language/tokens", "Tokenize source.",
        &["source"], &["tokens"]),
    ("build/check", "Lex + parse + report diagnostics.",
        &["source"], &["ok", "diagnostics"]),
    ("build/heal", "Generate fix candidates for diagnostics.",
        &["source"], &["ok", "diagnostics"]),
    ("build/recover", "Run the 5-stage recovery pipeline; return final source.",
        &["source"], &["ok", "stage", "candidates_tried", "source", "changed"]),
    ("abl/encode", "Source -> Agentic Binary Language bytes (hex).",
        &["source"], &["ok", "magic", "version", "container_bytes", "items", "abl_hex"]),
    ("abl/decode", "Agentic Binary Language bytes (hex) -> decompiled per-item view.",
        &["abl_hex"], &["ok", "container_bytes", "items"]),
    ("abl/run", "Source -> encode -> CpuBackend dispatch.",
        &["source"], &["ok", "container_bytes", "runs"]),
    ("pipeline/recover-and-encode", "Recover then encode Agentic Binary Language in one call.",
        &["source"], &["ok", "recover_stage", "recovered_source", "abl_hex", "items"]),
    ("cost/query", "Per-construct cost estimate.",
        &["construct", "target", "opt"], &["construct", "target", "opt", "estimate"]),
    ("cost/compare", "Compare costs of two constructs.",
        &["a", "b", "target", "opt"], &["a", "b", "winner"]),
    ("skb/query", "Query structured knowledge base.",
        &["query"], &["results"]),
    ("skb/spec", "Lookup spec block for a symbol.",
        &["fqn"], &["found", "spec"]),
    ("skb/rules", "List SKB rules.",
        &[], &["rules"]),
    ("verify/contracts", "Verify function contracts (req/ens/inv).",
        &["source"], &["ok", "results"]),
    ("verify/module", "Verify entire module.",
        &["source"], &["ok", "results"]),
    ("format/agent", "Format source in agent-canonical sigil mode.",
        &["source"], &["ok", "formatted"]),
    ("format/human", "Format source in human-readable keyword mode.",
        &["source"], &["ok", "formatted"]),
    ("lint/check", "Run lints on source.",
        &["source"], &["ok", "lints"]),
    ("token/report", "Per-construct token cost report for source.",
        &["source"], &["ok", "report"]),
    ("effects/infer", "Infer effects of each function.",
        &["source"], &["ok", "effects"]),
    ("effects/check", "Check declared effects against inferred.",
        &["source"], &["ok", "results"]),
    ("elision/apply", "Apply elision rules to compact source.",
        &["source"], &["ok", "elided"]),
    ("attribute/expand", "Expand attribute shorthand.",
        &["source"], &["ok", "expanded"]),
    ("attribute/compress", "Compress attributes back to shorthand.",
        &["source"], &["ok", "compressed"]),
    ("capability/check", "List capabilities required by source.",
        &["source"], &["ok", "capabilities"]),
    ("heal/graph", "Heal-pipeline diagnostic graph.",
        &["source"], &["ok", "graph"]),
    ("sandbox/policy", "Lookup sandbox policy by name.",
        &["name"], &["ok", "policy"]),
    ("doc/query", "Lookup documentation by FQN.",
        &["fqn"], &["found", "doc"]),
    ("grammar/list", "List grammar extensions.",
        &[], &["ok", "extensions"]),
    ("manifest/generate", "Generate capability manifest for a module.",
        &["source", "crate_name", "version"], &["ok", "manifest"]),
    ("nl/generate", "Generate MAGE from a natural-language prompt.",
        &["prompt"], &["ok", "code_human", "code_agent"]),
    ("nl/explain", "Explain source in natural language.",
        &["source"], &["ok", "explanation"]),
    ("nl/refactor", "Refactor source via natural-language request.",
        &["source"], &["ok", "code_human", "code_agent"]),
    ("nl/query", "General NL query against the SKB.",
        &["prompt"], &["ok", "explanation", "kb_results"]),
    ("ontology/full", "Return this complete ontology.",
        &[], &["ok", "version", "sections"]),
    ("ontology/section", "Return one named section of the ontology.",
        &["section"], &["ok", "section", "data"]),
];

/// Hardware accelerator backends RecursiveMachineIntelligence defines. Each entry
/// describes a backend variant + what's needed to enable it. CPU is
/// always available; the others require feature flags on the
/// dependency and platform SDKs.
///
/// The list is sourced from `prototype/src/backends.rs::HARDWARE_ACCELERATORS`.
/// Agents wanting to dispatch on a specific accelerator can query
/// `ontology/section { "section": "hardware_accelerators" }` to see
/// which are buildable and which need a flag flip.
fn hardware_accelerators_section() -> serde_json::Value {
    // Pulls from the extensible runtime registry (P93), not a
    // hardcoded table. Includes built-in entries + anything loaded
    // from RDX_BACKENDS_PATH, ~/.mage/backends.json, or
    // --backends-file. The `source` field tells agents where each
    // entry came from.
    let items: Vec<_> = crate::backends::all_descriptors()
        .into_iter()
        .map(|d| {
            serde_json::json!({
                "name": d.name,
                "family": d.family,
                "vendor": d.vendor,
                "requires": d.requires,
                "summary": d.summary,
                "available_at_runtime": d.available_at_runtime,
                "tags": d.tags,
                "source": d.source,
            })
        })
        .collect();
    serde_json::json!(items)
}

/// Every CLI flag the `mage-parse` binary accepts, with what it
/// does and where the entry-point is. Agents wanting to drive the
/// system from a shell can discover the full flag set in one call.
///
/// Columns: `(flag, purpose, takes_path)`.
/// Every flag the binary accepts, as published to agents.
///
/// It listed 17 of 29. The twelve missing ones included **`--eval`** — the
/// second of the two oracles, and the only way to *run* a program — plus
/// `--version`, `--json`, `--fix`, `--manifest`, `--token-report` and the
/// whole `--build=` / `--describe=` / `--spine=` family. An agent grounding
/// in the ontology could not learn that MAGE programs can be executed.
///
/// The test below now compares this table against the flag literals in
/// `main.rs` in both directions. It used to assert that eight named flags
/// were present, under a doc comment claiming it checked every flag the
/// binary accepts — a fixed list wearing a cross-boundary claim.
const CLI_FLAGS: &[(&str, &str, bool)] = &[
    ("--rap", "Start the RAP JSON-RPC server on the given addr (default 127.0.0.1:9876)", false),
    ("--emit-ontology", "Dump the complete ontology to disk as static JSON", true),
    ("--check", "Lex + parse + resolve; report diagnostics", true),
    ("--fmt-compact", "Reformat source in agent-canonical sigil mode", true),
    ("--fmt-expand", "Reformat source in human-readable keyword mode", true),
    ("--target=abl", "Print per-item Agentic Binary Language stats (nodes/depth/hash/bytes)", true),
    ("--target=abl-bytes", "Encode Agentic Binary Language-routed items to a binary Agentic Binary Language container", true),
    ("--from=abl-bytes", "Decode a Agentic Binary Language container back to MAGE view", true),
    ("--run=abl-bytes", "Decode Agentic Binary Language and dispatch each item on CpuBackend", true),
    ("--target=abl-generate", "Autoregressive generation from a trained checkpoint", true),
    ("--target=abl-infer", "Inference over a `train` block's saved weights", true),
    ("--target=abl-train", "Train every `train` block in the module", true),
    ("--target=abl-compute", "Forward pass without training over Agentic Binary Language items", true),
    ("--target=abl-run", "End-to-end run of Agentic Binary Language-routed items", true),
    ("--pipeline", "Run the full lex+parse+resolve+effects+verify pipeline", true),
    ("--backend=<name>",
        "Select the hardware accelerator for dispatch (default: cpu). Honoured by --run=abl-bytes and by every --target=abl-* path; a subprocess backend has no in-process Backend, so those paths report the fallback rather than using it silently. See ontology.hardware_accelerators for the catalog.",
        false),
    ("--backends-file=<path>",
        "Register additional backend descriptors at runtime from a JSON file. Stacks with RDX_BACKENDS_PATH env var and ~/.mage/backends.json. Schema: [{ name, family, vendor, requires, summary, available_at_runtime, tags }].",
        true),
    // ── Running and inspecting ────────────────────────────────────────
    ("--eval", "Evaluate the module and print the value of `main` (or of the named function). The second oracle: a program can typecheck and fail to run, and the reverse.", true),
    ("--version", "Print the compiler version. Ribosome keys its registry on this string.", false),
    ("--manifest", "Print the tool manifest: version, capabilities, and the flag surface.", false),
    ("--rain", "Render the source as MAGE digital rain (a display, not an analysis).", true),
    // ── Building from a JSON spec ─────────────────────────────────────
    ("--build=schema", "Print the JSON schema a build spec must satisfy.", false),
    ("--build=abl", "Build a `net` / `swarm` spec from JSON into MAGE source; `--fix` attempts deterministic repair first.", true),
    ("--describe", "Describe a module in JSON. Takes a mode; run it with none to see the list.", true),
    ("--describe=abl", "Describe an Agentic Binary Language container in JSON.", true),
    ("--run=abl", "Run an Agentic Binary Language container and print a JSON result.", true),
    ("--spine=frame", "Emit a spine frame (JSON) for the given module.", true),
    ("--spine=profile", "Emit an agent profile from a JSON agent spec.", true),
    ("--spine=swarm", "Emit a swarm profile from a JSON swarm spec.", true),
    // ── Modifiers, which stack with the commands above ────────────────
    ("--json", "Report diagnostics as structured JSON (code/span/category/fix) rather than text.", false),
    ("--fix", "With `--build=abl`: attempt deterministic auto-repair before rejecting a spec.", false),
    ("--input", "With `--run=abl`: the JSON input value for the run, as the next argument.", false),
    ("--no-elision", "Skip the elision pass, so the AST is checked exactly as written.", false),
    ("--syntax=legacy", "Translate legacy syntax before lexing.", false),
    ("--syntax=canonical", "Read the source as canonical syntax (the default).", false),
    ("--token-report", "Print a token-count report alongside the check.", false),
];

/// `reliability-bench` backends. Each backend implements
/// `CandidateAgent` and drives the bench differently.
///
/// Columns: `(name, mode, purpose)`.
const BENCH_BACKENDS: &[(&str, &str, &str)] = &[
    ("file-oracle", "default",
        "Echo the corpus reference solution. Upper bound on parse rate."),
    ("perturbed", "deterministic_noise",
        "Reference + 1 of 8 mutations (drop-`;`, drop-`}`, drop-`)`, swap let/mut, stray `,`, truncate-75%, dup-`;`, swap-words). Simulates near-correct LLM output."),
    ("subprocess:<cmd>", "external",
        "Spawn external process per task. stdin=task description, stdout=MAGE source. Plug in a real LLM here."),
    ("perturbed+refine:<cmd>", "hybrid",
        "perturbed for propose, subprocess for refine. Measures Stage-3 contribution layered on the perturbed baseline."),
];

/// Effect annotations the language supports. Used in `spec` blocks
/// and the `verify/contracts` RAP method.
///
/// Columns: `(annotation, slot, purpose)`.
const EFFECTS: &[(&str, &str, &str)] = &[
    ("@fx",   "spec/fn", "Effect list: enumerate side effects the body may produce. `@fx()` = pure."),
    ("@req",  "spec/fn", "Precondition: predicate over parameters; checked at entry"),
    ("@ens",  "spec/fn", "Postcondition: predicate over result; checked at exit"),
    ("@inv",  "spec/struct", "Invariant: predicate that must hold across all methods"),
    ("@perf", "spec/fn", "Performance contract: latency / throughput target"),
    // The built-in effect kinds — every name writable in a `/ …` annotation
    // with no declaration. This is the set `hir::Effect::from_name` folds and
    // `MAGE_SPEC.md` §11.2 documents; `builtin_effect_names_are_the_writable_ones`
    // holds the three in agreement.
    //
    // This list used to be "canonical effect names used in the corpus and
    // stdlib", which is a different question from the one an agent reading
    // `effect_name` is asking. Four of its ten — `db`, `log`, `tools`, `rand` —
    // were rejected by the compiler as unknown effects. Three of those are
    // capability *namespaces* (`resolve.rs`, `db.query(…)`), which is a
    // separate list that had been folded into this one; `rand` was `rng`
    // under a name nothing has ever accepted. `rng`, `gpu`,
    // `npu`, `evolve`, `learn`, `alloc`, `panic`, `ffi`, `env` and `agent` were
    // missing. Anything not here needs an `effect Name { … }` block (§11.5).
    ("io",     "effect_name", "File and stream I/O"),
    ("net",    "effect_name", "Network I/O"),
    ("fs",     "effect_name", "Filesystem operations"),
    ("async",  "effect_name", "Asynchronous task management"),
    ("alloc",  "effect_name", "Heap memory allocation"),
    ("panic",  "effect_name", "Unwinding / structured panics"),
    ("ffi",    "effect_name", "Foreign function invocation"),
    ("env",    "effect_name", "Environment variable access"),
    ("time",   "effect_name", "Clock and timer access"),
    ("gpu",    "effect_name", "GPU computation"),
    ("npu",    "effect_name", "Neural processing unit"),
    ("llm",    "effect_name", "Language model invocation"),
    ("evolve", "effect_name", "Evolutionary computation"),
    ("learn",  "effect_name", "Training / gradient descent"),
    ("rng",    "effect_name", "Random number generation"),
    ("agent",  "effect_name", "Agent coordination"),
    ("proc",   "effect_name", "Process and system access"),
];

/// Subprocess agent protocol contract. Environment variables and
/// stdin/stdout semantics for wrappers under `scripts/agent_wrappers/`.
///
/// Columns: `(name, kind, purpose)`.
const WRAPPER_PROTOCOL: &[(&str, &str, &str)] = &[
    ("RDX_BENCH_MODE", "env",
        "`propose` (first call) or `refine` (Stage-3 re-prompt)"),
    ("RDX_TASK_ID", "env",
        "Stable task identifier; set on every call"),
    ("RDX_TASK_DESCRIPTION", "env",
        "Original task description; set on refine only (stdin carries broken source)"),
    ("RDX_PARSE_ERROR", "env",
        "Parse error that triggered refine; set on refine only"),
    ("stdin (propose)", "stream",
        "Natural-language task description"),
    ("stdin (refine)",  "stream",
        "Broken MAGE source the mechanical pipeline could not save"),
    ("stdout", "stream",
        "MAGE source to evaluate. Empty / unchanged input = no contribution."),
    ("stderr", "stream",
        "Human-readable diagnostics; surfaces as `agent refused: <text>` on non-zero exit"),
    ("exit code", "stream",
        "0 = success; non-zero = agent refused / no candidate"),
];

/// Top-level project directories. Tells an agent where to find what.
///
/// Columns: `(path, purpose)`.
const PROJECT_LAYOUT: &[(&str, &str)] = &[
    ("prototype/", "Rust compiler + RAP server + benches"),
    ("prototype/src/", "Lexer / parser / heal / recover / abl_bridge / rap / ontology"),
    ("prototype/src/bin/", "reliability-bench, token-bench"),
    ("prototype/examples/", "Inline `.mg` examples used by parser tests"),
    ("RecursiveMachineIntelligence/", "RMI Rust crate: binary IR, opcodes, codec, CpuBackend"),
    ("RecursiveMachineIntelligence/src/lang/", "Op enum, OpFamily, codec"),
    ("RecursiveMachineIntelligence/src/compute/", "CpuBackend + dispatch"),
    ("framework/framewerx/", "RecursiveMachineIntelligence-MG (this framework: FLAX-equivalent in MAGE)"),
    ("framework/framewerx/src/", "Module / layers / optim / loss / train / specs / neurosymbolic"),
    ("framework/framewerx/src/neural/", "Modern neural: attention variants, MoE, adapters, quantization, dynamical, energy, memory, multimodal, world models, advanced diffusion"),
    ("framework/framewerx/src/symbolic/", "Classical symbolic: logic, solvers, planning, probabilistic, knowledge"),
    ("framework/framewerx/src/neurosymbolic/", "Bridging: differentiable_logic, reasoning, verification"),
    ("framework/framewerx/src/agentic/", "Orchestration: reasoning_patterns, multi_agent, decoding"),
    ("framework/framewerx/examples/", "Parse-verified, end-to-end-compile-tested examples"),
    ("benchmarks/", "Reliability + token benches; corpus + reports"),
    ("benchmarks/tasks/", "100-task reference corpus"),
    ("scripts/agent_wrappers/", "Subprocess wrappers for the agent protocol"),
    ("scripts/demo_agent_workflow.sh", "One-command end-to-end demo of the agent flow"),
    ("examples/", "Standalone `.mg` examples (agent-swarm, http-client, cli-tool, etc.)"),
    ("UNIFICATION.md", "Architecture doc with per-phase history"),
    ("MAGE_ONTOLOGY.json", "Static dump of this ontology (refresh via --emit-ontology)"),
    (".github/workflows/ci.yml", "CI with floors on parse / heal / refine / token-ratio"),
];

/// Documentation pointers. Where to read what.
///
/// Columns: `(path, purpose, audience)`.
const DOCS: &[(&str, &str, &str)] = &[
    ("UNIFICATION.md", "Per-phase architecture history and final-state grid", "human"),
    ("benchmarks/STATUS.md", "One-page current-state snapshot", "human/agent"),
    ("MAGE_ONTOLOGY.json", "This ontology as static JSON (agent bootstrap)", "agent"),
    ("framework/framewerx/README.md", "RecursiveMachineIntelligence-MG architecture (JAX:FLAX :: RMI:RecursiveMachineIntelligence-MG)", "agent"),
    ("scripts/agent_wrappers/README.md", "Subprocess wrapper protocol specification", "agent"),
    ("MAGE_SPEC.md", "Language specification: sigils, keywords, grammar", "human/agent"),
    ("../../utilities/IronAccelerator/",
        "External reference: production HW-agnostic driver substrate + per-model accelerator ontology (NVIDIA / AMD / Apple / Qualcomm / Intel / Google / AWS / open APIs). Drill in for model-specific guidance; MAGE's hardware_accelerators section stays at the backend-family level.",
        "agent/reference"),
];

/// Current CI floors. An agent proposing a change can check whether
/// its measured numbers stay above each. Read from
/// `.github/workflows/ci.yml`.
///
/// Columns: `(name, threshold, what_it_protects)`.
const CI_FLOORS: &[(&str, &str, &str)] = &[
    ("MIN_PARSE", ">= 98",
        "File-oracle parse rate (100-task corpus) must not regress"),
    ("file-oracle structural-heal", ">= 2",
        "Trim-bad-token mechanism (P51) must keep contributing"),
    ("MIN_HEAL", ">= 40",
        "Perturbed-8 pattern-heal recovery count must not regress"),
    ("refine smoke", "> 0",
        "Stage-3 refine wrapper protocol must fire end-to-end"),
    ("subprocess echo smoke", "no-op",
        "Subprocess agent backend must be invocable"),
    ("native-lexer ratio", "<= 1.100",
        "MAGE text size must stay within 10% of equivalent Rust"),
];

/// Curated golden examples. Each one is a minimal, parseable program
/// that demonstrates a specific surface form. Listed here so an agent
/// reading the ontology has copy-paste-ready patterns covering the
/// load-bearing constructs. The `exercises` field cross-references
/// which sigils/keywords the snippet uses.
///
/// **Invariant**: every entry in this table MUST parse cleanly. The
/// `examples_all_parse` test in this module enforces it — if a sigil
/// or keyword changes shape, the broken example surfaces here first.
const EXAMPLES: &[(&str, &str, &str, &[&str])] = &[
    (
        "hello-world",
        "Minimal public function returning a string literal.",
        "+f hello() -> s { \"Hello, World!\" }",
        &["+f", "->", "s"],
    ),
    (
        "let-bindings",
        "Immutable and mutable bindings; arithmetic.",
        "+f compute() -> i32 { v x = 1; m y = 2; y = x + y; y }",
        &["+f", "v", "m"],
    ),
    (
        "if-else",
        "Sigil-mode if/else returning a value.",
        "+f sign(x: i32) -> i32 { ? x > 0 { 1 } : { ? x < 0 { -1 } : { 0 } } }",
        &["?", ":"],
    ),
    (
        "match-option",
        "Match on an Option<T> using ?T sugar.",
        "+f opt_or_zero(o: ?i32) -> i32 { ?= o { Some(n) => n, None => 0 } }",
        &["?=", "?T", "Some", "None"],
    ),
    (
        "struct-impl",
        "Struct definition with an impl block. Uses @T sigil for struct literal.",
        "S Point { x: i32, y: i32 } I Point { +f origin() -> _T { @Point { x: 0, y: 0 } } }",
        &["S", "I", "+f", "_T", "@T"],
    ),
    (
        "for-loop",
        "Iterate a range using `for` keyword (Phase 56: range in iter position).",
        "+f sum_to(n: i32) -> i32 { m s = 0; for i in 0..n { s = s + i; } s }",
        &["+f", "m", "for", ".."],
    ),
    (
        "net-linear",
        "Minimal neural net: one Linear layer (lowers to Agentic Binary Language).",
        "net tiny { layer fc: Linear(8, 4); forward { fc } }",
        &["net", "layer", "forward"],
    ),
    (
        "net-activation-chain",
        "Linear + ReLU chain inside a forward pass.",
        "net mlp { layer fc1: Linear(16, 8); layer act: ReLU; forward { act(fc1) } }",
        &["net", "layer", "forward"],
    ),
    (
        "kb-rule",
        "Symbolic knowledge base with facts and a rule (lowers to Agentic Binary Language).",
        "kb FamilyKb { fact parent(a, b); fact parent(b, c); rule grandparent(x: i32, y: i32) { x } }",
        &["kb", "fact", "rule"],
    ),
    (
        "agent-role",
        "Agent role declaration with required capabilities.",
        "agent Bot { capabilities: [net, read_source] }",
        &["agent"],
    ),
];

/// RecursiveMachineIntelligence-MG framework registry. Lists every Module / Layer /
/// Optimizer / Loss available in the `framework/framewerx/` directory,
/// with the file path agents can read for full source. This is the
/// "FLAX-equivalent" surface, layered over the RMI low-level ops.
///
/// Columns: `(name, category, path, summary)`.
const FRAMEWERX_MODULES: &[(&str, &str, &str, &str)] = &[
    // ── Composition primitives ──────────────────────────────────────
    ("Module", "trait", "framework/framewerx/src/module.mg",
        "Base trait: forward(self, Tensor) -> Tensor"),
    ("Sequential", "composition", "framework/framewerx/src/module.mg",
        "Apply a list of modules left-to-right"),
    ("Residual", "composition", "framework/framewerx/src/module.mg",
        "y = x + f(x); standard transformer block pattern"),
    ("Branch", "composition", "framework/framewerx/src/module.mg",
        "Parallel: apply N modules to same input, return tuple"),

    // ── Linear / Conv / Pool ─────────────────────────────────────────
    ("Linear", "layer", "framework/framewerx/src/layers/linear.mg",
        "Affine transform y = xW^T + b; lowers to opcode 0x0002"),
    ("Conv2D", "layer", "framework/framewerx/src/layers/conv.mg",
        "2-D convolution; lowers to opcode 0x0003"),
    ("MaxPool", "layer", "framework/framewerx/src/layers/conv.mg",
        "Spatial max pooling"),
    ("AvgPool", "layer", "framework/framewerx/src/layers/conv.mg",
        "Spatial average pooling"),
    ("GlobalAvgPool", "layer", "framework/framewerx/src/layers/conv.mg",
        "Global average pooling over spatial dims"),

    // ── Embeddings ──────────────────────────────────────────────────
    ("Embedding", "embedding", "framework/framewerx/src/layers/embedding.mg",
        "Token embedding table; opcode 0x0005"),
    ("PositionalEmbedding", "embedding", "framework/framewerx/src/layers/embedding.mg",
        "Sinusoidal positional embedding (frozen at init)"),
    ("LearnedPositionEmbedding", "embedding", "framework/framewerx/src/layers/embedding.mg",
        "Trainable positional embedding table"),
    ("RotaryEmbedding", "embedding", "framework/framewerx/src/layers/embedding.mg",
        "Rotary position embedding (RoPE) for attention"),

    // ── Regularisation ──────────────────────────────────────────────
    ("Dropout", "regularisation", "framework/framewerx/src/layers/dropout.mg",
        "Stochastic activation dropout; opcode 0x0006"),
    ("Dropout2D", "regularisation", "framework/framewerx/src/layers/dropout.mg",
        "Spatial dropout: drops entire feature maps"),
    ("DropPath", "regularisation", "framework/framewerx/src/layers/dropout.mg",
        "Stochastic depth: drops residual branches"),

    // ── Activations ─────────────────────────────────────────────────
    ("ReLU", "activation", "framework/framewerx/src/layers/activation.mg",
        "max(0, x); opcode 0x0010"),
    ("GELU", "activation", "framework/framewerx/src/layers/activation.mg",
        "Gaussian Error Linear Unit; opcode 0x0011"),
    ("SiLU", "activation", "framework/framewerx/src/layers/activation.mg",
        "x * sigmoid(x); opcode 0x0012"),
    ("Sigmoid", "activation", "framework/framewerx/src/layers/activation.mg",
        "1 / (1 + exp(-x)); opcode 0x0013"),
    ("Tanh", "activation", "framework/framewerx/src/layers/activation.mg",
        "Hyperbolic tangent; opcode 0x0014"),
    ("Softmax", "activation", "framework/framewerx/src/layers/activation.mg",
        "Softmax over a chosen axis; opcode 0x0007"),
    ("SELU", "activation", "framework/framewerx/src/layers/activation.mg",
        "Self-normalising scaled exponential linear unit"),
    ("ELU", "activation", "framework/framewerx/src/layers/activation.mg",
        "Exponential linear unit"),
    ("LeakyReLU", "activation", "framework/framewerx/src/layers/activation.mg",
        "ReLU with non-zero negative-slope leak"),
    ("Mish", "activation", "framework/framewerx/src/layers/activation.mg",
        "x * tanh(softplus(x)); opcode 0x0015"),
    ("Softplus", "activation", "framework/framewerx/src/layers/activation.mg",
        "ln(1 + exp(x)); opcode 0x0016"),
    ("HardSwish", "activation", "framework/framewerx/src/layers/activation.mg",
        "Edge-friendly Swish approximation"),
    ("HardSigmoid", "activation", "framework/framewerx/src/layers/activation.mg",
        "Edge-friendly Sigmoid approximation"),
    ("SwiGLU", "activation", "framework/framewerx/src/layers/activation.mg",
        "Gated linear unit with Swish; used in modern LLMs"),
    ("GeGLU", "activation", "framework/framewerx/src/layers/activation.mg",
        "Gated linear unit with GELU"),

    // ── Normalisation ───────────────────────────────────────────────
    ("LayerNorm", "norm", "framework/framewerx/src/layers/norm.mg",
        "Layer normalisation; opcode 0x0020"),
    ("RMSNorm", "norm", "framework/framewerx/src/layers/norm.mg",
        "RMS normalisation; opcode 0x0022"),
    ("BatchNorm", "norm", "framework/framewerx/src/layers/norm.mg",
        "Batch normalisation; opcode 0x0021"),

    // ── Attention ───────────────────────────────────────────────────
    ("Attention", "attention", "framework/framewerx/src/layers/attention.mg",
        "Multi-head attention; opcode 0x0004"),

    // ── Recurrent ───────────────────────────────────────────────────
    ("RNNCell", "recurrent", "framework/framewerx/src/layers/recurrent.mg",
        "Basic Elman RNN cell"),
    ("LSTMCell", "recurrent", "framework/framewerx/src/layers/recurrent.mg",
        "Long Short-Term Memory cell"),
    ("GRUCell", "recurrent", "framework/framewerx/src/layers/recurrent.mg",
        "Gated Recurrent Unit cell"),
    ("RNN", "recurrent", "framework/framewerx/src/layers/recurrent.mg",
        "Multi-layer RNN with optional bidirectional / dropout"),
    ("LSTM", "recurrent", "framework/framewerx/src/layers/recurrent.mg",
        "Multi-layer LSTM with optional bidirectional / dropout"),
    ("GRU", "recurrent", "framework/framewerx/src/layers/recurrent.mg",
        "Multi-layer GRU with optional bidirectional / dropout"),

    // ── Graph ───────────────────────────────────────────────────────
    ("GCNLayer", "graph", "framework/framewerx/src/layers/graph.mg",
        "Graph convolution layer (Kipf & Welling)"),
    ("GATLayer", "graph", "framework/framewerx/src/layers/graph.mg",
        "Graph attention layer (Velickovic et al.)"),
    ("GraphSAGELayer", "graph", "framework/framewerx/src/layers/graph.mg",
        "Sampled neighbour aggregation: mean / max"),
    ("EdgeConv", "graph", "framework/framewerx/src/layers/graph.mg",
        "k-NN edge convolution (PointNet, DGCNN)"),
    ("GlobalMeanPool", "graph", "framework/framewerx/src/layers/graph.mg",
        "Graph-level mean readout"),

    // ── State-space ─────────────────────────────────────────────────
    ("S4Layer", "state_space", "framework/framewerx/src/layers/state_space.mg",
        "Structured state-space (HiPPO init)"),
    ("S5Layer", "state_space", "framework/framewerx/src/layers/state_space.mg",
        "Parallel scan-based MIMO SSM"),
    ("MambaBlock", "state_space", "framework/framewerx/src/layers/state_space.mg",
        "Selective SSM with input-dependent dt/B/C"),
    ("H3Layer", "state_space", "framework/framewerx/src/layers/state_space.mg",
        "Hybrid SSM/attention layer"),

    // ── Architectures: CNN family ───────────────────────────────────
    ("CNN", "architecture", "framework/framewerx/src/architectures/cnn.mg",
        "Plain CNN: Nx (Conv -> ReLU -> Pool) + head"),
    ("ResNetBlock", "architecture", "framework/framewerx/src/architectures/cnn.mg",
        "Two-conv residual block"),
    ("DepthwiseSeparable", "architecture", "framework/framewerx/src/architectures/cnn.mg",
        "MobileNet-style depthwise+pointwise conv"),
    ("UNet", "architecture", "framework/framewerx/src/architectures/cnn.mg",
        "Encoder-decoder with skip connections"),

    // ── Architectures: Transformer family ───────────────────────────
    ("TransformerEncoder", "architecture", "framework/framewerx/src/architectures/transformer.mg",
        "Stack of pre-norm encoder blocks"),
    ("TransformerDecoder", "architecture", "framework/framewerx/src/architectures/transformer.mg",
        "GPT-style causal decoder"),
    ("EncoderDecoder", "architecture", "framework/framewerx/src/architectures/transformer.mg",
        "T5/BART-style encoder-decoder with cross-attention"),
    ("ViT", "architecture", "framework/framewerx/src/architectures/transformer.mg",
        "Vision Transformer: patches + position + encoder"),

    // ── Architectures: Generative ───────────────────────────────────
    ("VAE", "architecture", "framework/framewerx/src/architectures/generative.mg",
        "Variational Autoencoder"),
    ("CVAE", "architecture", "framework/framewerx/src/architectures/generative.mg",
        "Conditional VAE"),
    ("GAN", "architecture", "framework/framewerx/src/architectures/generative.mg",
        "Generator + Discriminator pair"),
    ("WGAN_GP", "architecture", "framework/framewerx/src/architectures/generative.mg",
        "Wasserstein GAN with gradient penalty"),
    ("DDPM", "architecture", "framework/framewerx/src/architectures/generative.mg",
        "Denoising Diffusion Probabilistic Model"),
    ("LatentDiffusion", "architecture", "framework/framewerx/src/architectures/generative.mg",
        "Diffusion in VAE latent space (Stable Diffusion family)"),

    // ── Architectures: Graph ────────────────────────────────────────
    ("GCN", "architecture", "framework/framewerx/src/architectures/graph_nets.mg",
        "Graph Convolutional Network"),
    ("GAT", "architecture", "framework/framewerx/src/architectures/graph_nets.mg",
        "Graph Attention Network"),
    ("GraphSAGE", "architecture", "framework/framewerx/src/architectures/graph_nets.mg",
        "GraphSAGE: sampled aggregation"),
    ("MPNN", "architecture", "framework/framewerx/src/architectures/graph_nets.mg",
        "Message Passing Neural Network base"),

    // ── Architectures: Sequence (RNN / SSM) ─────────────────────────
    ("LSTMLanguageModel", "architecture", "framework/framewerx/src/architectures/sequence.mg",
        "Plain LSTM language model"),
    ("Mamba", "architecture", "framework/framewerx/src/architectures/sequence.mg",
        "Selective SSM language model"),
    ("Hyena", "architecture", "framework/framewerx/src/architectures/sequence.mg",
        "Long-convolution sequence model"),
    ("RWKV", "architecture", "framework/framewerx/src/architectures/sequence.mg",
        "Receptance-weighted key-value: hybrid attention+RNN"),

    // ── Optimizers ──────────────────────────────────────────────────
    ("SGD", "optimizer", "framework/framewerx/src/optim/sgd.mg",
        "Stochastic gradient descent with optional momentum"),
    ("Adam", "optimizer", "framework/framewerx/src/optim/adam.mg",
        "Adam: adaptive moment estimation"),

    // ── Losses ──────────────────────────────────────────────────────
    ("MSE", "loss", "framework/framewerx/src/loss.mg",
        "Mean squared error"),
    ("CrossEntropy", "loss", "framework/framewerx/src/loss.mg",
        "Categorical cross-entropy"),
    ("BCE", "loss", "framework/framewerx/src/loss.mg",
        "Binary cross-entropy"),

    // ── Training infrastructure ─────────────────────────────────────
    ("TrainState", "training", "framework/framewerx/src/train.mg",
        "Bundles params + optimizer state + step counter"),

    // ── Neural: attention variants ──────────────────────────────────
    ("FlashAttention", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "Tiled exact attention with O(N) memory"),
    ("SlidingWindowAttention", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "Each token sees only window_size tokens behind"),
    ("LongformerAttention", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "Sliding window + global tokens"),
    ("LinearAttention", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "O(N) attention via kernel feature maps"),
    ("PerformerAttention", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "FAVOR+ random-feature attention"),
    ("GroupedQueryAttention", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "Query heads share KV heads (Llama-2/3)"),
    ("MultiQueryAttention", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "One KV head shared across all queries"),
    ("CrossAttention", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "Query from one sequence, KV from another"),
    ("KVCache", "neural_attention", "framework/framewerx/src/neural/attention_variants.mg",
        "Stateful per-layer KV buffer for decoding"),

    // ── Neural: mixture-of-experts ──────────────────────────────────
    ("Expert", "moe", "framework/framewerx/src/neural/moe.mg",
        "Single FF sub-network the router can dispatch to"),
    ("TopKRouter", "moe", "framework/framewerx/src/neural/moe.mg",
        "Picks top-K highest-scoring experts per token"),
    ("SwitchRouter", "moe", "framework/framewerx/src/neural/moe.mg",
        "Switch-Transformer always-top-1 capacity-limited"),
    ("ExpertChoiceRouter", "moe", "framework/framewerx/src/neural/moe.mg",
        "Each expert picks K tokens"),
    ("SparseMoE", "moe", "framework/framewerx/src/neural/moe.mg",
        "Router + N experts + load-balancing loss"),
    ("MixtureOfDepths", "moe", "framework/framewerx/src/neural/moe.mg",
        "Token-level skip via per-block router"),

    // ── Neural: PEFT / adapters ─────────────────────────────────────
    ("LoRA", "adapter", "framework/framewerx/src/neural/adapters.mg",
        "Low-rank adapter W' = W + B*A"),
    ("QLoRA", "adapter", "framework/framewerx/src/neural/adapters.mg",
        "LoRA over 4-bit-quantized base weights"),
    ("DoRA", "adapter", "framework/framewerx/src/neural/adapters.mg",
        "Weight-decomposed LoRA"),
    ("IA3", "adapter", "framework/framewerx/src/neural/adapters.mg",
        "Scale activations rather than add weights"),
    ("PrefixTuning", "adapter", "framework/framewerx/src/neural/adapters.mg",
        "Learnable KV prefix per attention layer"),
    ("PromptTuning", "adapter", "framework/framewerx/src/neural/adapters.mg",
        "Learnable virtual tokens prepended to input"),
    ("Adapter", "adapter", "framework/framewerx/src/neural/adapters.mg",
        "Houlsby bottleneck FF block after each sublayer"),

    // ── Neural: quantization ────────────────────────────────────────
    ("Int8Linear", "quantization", "framework/framewerx/src/neural/quantization.mg",
        "8-bit integer Linear layer"),
    ("Int4Linear", "quantization", "framework/framewerx/src/neural/quantization.mg",
        "4-bit Linear with grouped quantization"),
    ("BitNetLinear", "quantization", "framework/framewerx/src/neural/quantization.mg",
        "Ternary {-1, 0, 1} weights"),
    ("GPTQConfig", "quantization", "framework/framewerx/src/neural/quantization.mg",
        "GPTQ quantization recipe"),
    ("AWQConfig", "quantization", "framework/framewerx/src/neural/quantization.mg",
        "Activation-aware weight quantization"),
    ("MixedPrecision", "quantization", "framework/framewerx/src/neural/quantization.mg",
        "FP8 / BF16 mixed-precision training config"),

    // ── Neural: dynamical / biological ──────────────────────────────
    ("NeuralODE", "dynamical", "framework/framewerx/src/neural/dynamical.mg",
        "Continuous-time hidden-state network"),
    ("NeuralSDE", "dynamical", "framework/framewerx/src/neural/dynamical.mg",
        "Stochastic continuous-time model"),
    ("LiquidCell", "dynamical", "framework/framewerx/src/neural/dynamical.mg",
        "Closed-form continuous-time cell"),
    ("LIF", "dynamical", "framework/framewerx/src/neural/dynamical.mg",
        "Leaky Integrate-and-Fire spiking neuron"),
    ("ALIF", "dynamical", "framework/framewerx/src/neural/dynamical.mg",
        "Adaptive LIF with spike-frequency adaptation"),
    ("ModernHopfield", "dynamical", "framework/framewerx/src/neural/dynamical.mg",
        "Modern dense associative memory"),
    ("SpikingTransformer", "dynamical", "framework/framewerx/src/neural/dynamical.mg",
        "Surrogate-gradient spiking transformer"),

    // ── Neural: energy-based / flows ────────────────────────────────
    ("EBM", "energy", "framework/framewerx/src/neural/energy_based.mg",
        "Energy-based model with MCMC sampling"),
    ("RBM", "energy", "framework/framewerx/src/neural/energy_based.mg",
        "Restricted Boltzmann Machine"),
    ("ScoreModel", "energy", "framework/framewerx/src/neural/energy_based.mg",
        "Score-based generative (NCSN/VESDE)"),
    ("NormalizingFlow", "energy", "framework/framewerx/src/neural/energy_based.mg",
        "Invertible NN density estimator"),
    ("RealNVPCoupling", "energy", "framework/framewerx/src/neural/energy_based.mg",
        "Real-NVP affine coupling layer"),
    ("CNF", "energy", "framework/framewerx/src/neural/energy_based.mg",
        "Continuous Normalising Flow"),

    // ── Neural: memory-augmented ────────────────────────────────────
    ("NeuralTuringMachine", "memory_aug", "framework/framewerx/src/neural/memory.mg",
        "Controller + R/W heads over external memory matrix"),
    ("DNC", "memory_aug", "framework/framewerx/src/neural/memory.mg",
        "Differentiable Neural Computer (DeepMind)"),
    ("MemoryNetwork", "memory_aug", "framework/framewerx/src/neural/memory.mg",
        "Multi-hop attention over a memory bank"),
    ("PointerNetwork", "memory_aug", "framework/framewerx/src/neural/memory.mg",
        "Attention as a pointer to input positions"),
    ("VSABinder", "memory_aug", "framework/framewerx/src/neural/memory.mg",
        "Vector Symbolic Architecture binding (HRR/MAP/FHRR/BSC)"),

    // ── Neural: multimodal ──────────────────────────────────────────
    ("CLIP", "multimodal", "framework/framewerx/src/neural/multimodal.mg",
        "Image-text contrastive pretraining"),
    ("BLIP2", "multimodal", "framework/framewerx/src/neural/multimodal.mg",
        "Q-Former bridging vision and LLM"),
    ("LLaVAProjector", "multimodal", "framework/framewerx/src/neural/multimodal.mg",
        "Linear projection from vision to LLM tokens"),
    ("FlamingoBlock", "multimodal", "framework/framewerx/src/neural/multimodal.mg",
        "Gated cross-attention vision<->language"),
    ("PerceiverIO", "multimodal", "framework/framewerx/src/neural/multimodal.mg",
        "Cross-attention bottleneck on fixed latent array"),
    ("WaveNet", "multimodal", "framework/framewerx/src/neural/multimodal.mg",
        "Dilated causal conv stack for audio"),
    ("Conformer", "multimodal", "framework/framewerx/src/neural/multimodal.mg",
        "Conv-augmented transformer for speech"),

    // ── Neural: world models / self-supervised ──────────────────────
    ("JEPA", "world_model", "framework/framewerx/src/neural/world_models.mg",
        "Joint-Embedding Predictive Architecture (LeCun)"),
    ("IJEPA", "world_model", "framework/framewerx/src/neural/world_models.mg",
        "Image JEPA over patch embeddings"),
    ("VJEPA", "world_model", "framework/framewerx/src/neural/world_models.mg",
        "Video JEPA"),
    ("DreamerV3", "world_model", "framework/framewerx/src/neural/world_models.mg",
        "Dreamer-V3 world-model agent"),
    ("SimSiam", "world_model", "framework/framewerx/src/neural/world_models.mg",
        "Self-supervised siamese without negatives"),
    ("BYOL", "world_model", "framework/framewerx/src/neural/world_models.mg",
        "Bootstrap Your Own Latent"),
    ("DINO", "world_model", "framework/framewerx/src/neural/world_models.mg",
        "Self-distillation with no labels"),
    ("MAE", "world_model", "framework/framewerx/src/neural/world_models.mg",
        "Masked Autoencoder (75% patch mask)"),

    // ── Neural: advanced diffusion ──────────────────────────────────
    ("DiT", "diffusion", "framework/framewerx/src/neural/diffusion_advanced.mg",
        "Diffusion Transformer (Peebles & Xie)"),
    ("EDMSchedule", "diffusion", "framework/framewerx/src/neural/diffusion_advanced.mg",
        "Karras et al. noise schedule"),
    ("ConsistencyModel", "diffusion", "framework/framewerx/src/neural/diffusion_advanced.mg",
        "Direct noise->data mapping"),
    ("RectifiedFlow", "diffusion", "framework/framewerx/src/neural/diffusion_advanced.mg",
        "Straight-line probability paths"),
    ("FlowMatching", "diffusion", "framework/framewerx/src/neural/diffusion_advanced.mg",
        "Continuous flow matching with OT"),
    ("ClassifierFreeGuidance", "diffusion", "framework/framewerx/src/neural/diffusion_advanced.mg",
        "Conditional/unconditional guidance scaling"),

    // ── Symbolic: logic ─────────────────────────────────────────────
    ("Term", "symbolic_logic", "framework/framewerx/src/symbolic/logic.mg",
        "Variable / Constant / Function term"),
    ("HornClause", "symbolic_logic", "framework/framewerx/src/symbolic/logic.mg",
        "head :- body Horn clause"),
    ("Unifier", "symbolic_logic", "framework/framewerx/src/symbolic/logic.mg",
        "Most-general unifier engine"),
    ("SLDResolver", "symbolic_logic", "framework/framewerx/src/symbolic/logic.mg",
        "Prolog-style backward chaining"),
    ("ForwardChainer", "symbolic_logic", "framework/framewerx/src/symbolic/logic.mg",
        "Forward production rule system"),
    ("BackwardChainer", "symbolic_logic", "framework/framewerx/src/symbolic/logic.mg",
        "Goal-driven proof-tree builder"),
    ("DLOntology", "symbolic_logic", "framework/framewerx/src/symbolic/logic.mg",
        "Description-logic ontology (TBox/ABox/RBox)"),
    ("TableauReasoner", "symbolic_logic", "framework/framewerx/src/symbolic/logic.mg",
        "Tableau-based DL reasoner (ALC/SHOIQ)"),

    // ── Symbolic: solvers ───────────────────────────────────────────
    ("SATSolver", "symbolic_solver", "framework/framewerx/src/symbolic/solvers.mg",
        "CDCL SAT solver"),
    ("SMTSolver", "symbolic_solver", "framework/framewerx/src/symbolic/solvers.mg",
        "SAT + theory combination"),
    ("CSPSolver", "symbolic_solver", "framework/framewerx/src/symbolic/solvers.mg",
        "Constraint satisfaction with AC-3 / backtracking"),
    ("MILPSolver", "symbolic_solver", "framework/framewerx/src/symbolic/solvers.mg",
        "Mixed-integer linear programming"),
    ("TheoremProver", "symbolic_solver", "framework/framewerx/src/symbolic/solvers.mg",
        "Resolution / superposition theorem prover"),
    ("TermRewriteSystem", "symbolic_solver", "framework/framewerx/src/symbolic/solvers.mg",
        "Equational term rewriting"),

    // ── Symbolic: planning ──────────────────────────────────────────
    ("PDDLAction", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "Preconditions + effects action schema"),
    ("STRIPSPlanner", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "STRIPS forward-search planner"),
    ("HTNPlanner", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "Hierarchical Task Network planner"),
    ("POPPlanner", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "Partial-order causal-link planner"),
    ("MCTS", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "Monte Carlo Tree Search (UCT)"),
    ("MDP", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "Markov Decision Process spec"),
    ("POMDP", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "Partially-observable MDP"),
    ("ValueIteration", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "Value iteration solver"),
    ("PolicyIteration", "planning", "framework/framewerx/src/symbolic/planning.mg",
        "Policy iteration solver"),

    // ── Symbolic: probabilistic ─────────────────────────────────────
    ("BayesianNetwork", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Directed graphical model"),
    ("VariableElimination", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Exact inference by variable elimination"),
    ("BeliefPropagation", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Sum-product message passing"),
    ("JunctionTree", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Clique-tree exact inference"),
    ("MarkovRandomField", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Undirected graphical model"),
    ("HMM", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Hidden Markov Model"),
    ("ParticleFilter", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Sequential Monte Carlo"),
    ("MCMCSampler", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "NUTS / HMC / Metropolis sampler"),
    ("VariationalInference", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Mean-field / structured variational"),
    ("StructuralCausalModel", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Pearl's structural causal model"),
    ("DoCalculus", "probabilistic", "framework/framewerx/src/symbolic/probabilistic.mg",
        "Interventional reasoning"),

    // ── Symbolic: knowledge representation ──────────────────────────
    ("KnowledgeGraph", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "Subject-predicate-object triple store"),
    ("TripleStore", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "Triple store with multi-index"),
    ("SPARQLEngine", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "SPARQL-like query engine"),
    ("TransE", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "Translation-based KG embedding"),
    ("DistMult", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "Bilinear-diagonal KG embedding"),
    ("RotatE", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "Complex-rotation KG embedding"),
    ("ComplEx", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "Complex-bilinear KG embedding"),
    ("OWLReasoner", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "OWL EL/RL/QL reasoner"),
    ("SemanticNetwork", "knowledge", "framework/framewerx/src/symbolic/knowledge.mg",
        "Node-edge semantic network"),

    // ── Neurosymbolic: differentiable logic ─────────────────────────
    ("LogicTensorNetwork", "neurosymbolic", "framework/framewerx/src/neurosymbolic/differentiable_logic.mg",
        "LTN: real-logic FOL on tensors"),
    ("DeepProbLog", "neurosymbolic", "framework/framewerx/src/neurosymbolic/differentiable_logic.mg",
        "ProbLog atoms parameterised by NNs"),
    ("SemanticLoss", "neurosymbolic", "framework/framewerx/src/neurosymbolic/differentiable_logic.mg",
        "Constraint-violation penalty"),
    ("DifferentiableSAT", "neurosymbolic", "framework/framewerx/src/neurosymbolic/differentiable_logic.mg",
        "NeuroSAT / SATNet differentiable SAT"),
    ("NeuralTheoremProver", "neurosymbolic", "framework/framewerx/src/neurosymbolic/differentiable_logic.mg",
        "Differentiable backward-chaining (NTP)"),
    ("TNorm", "neurosymbolic", "framework/framewerx/src/neurosymbolic/differentiable_logic.mg",
        "Product/Godel/Lukasiewicz/nilpotent t-norms"),
    ("MarkovLogicNetwork", "neurosymbolic", "framework/framewerx/src/neurosymbolic/differentiable_logic.mg",
        "Weighted first-order formulae"),

    // ── Neurosymbolic: reasoning / retrieval ────────────────────────
    ("ConceptBottleneckModel", "neurosymbolic", "framework/framewerx/src/neurosymbolic/reasoning.mg",
        "Features -> concepts -> task"),
    ("NeuralAlgorithmicReasoner", "neurosymbolic", "framework/framewerx/src/neurosymbolic/reasoning.mg",
        "GNN that mimics a classical algorithm"),
    ("DifferentiableIndex", "neurosymbolic", "framework/framewerx/src/neurosymbolic/reasoning.mg",
        "Soft attention into a key-value store"),
    ("RAG", "neurosymbolic", "framework/framewerx/src/neurosymbolic/reasoning.mg",
        "Retrieval-augmented generation"),
    ("Atlas", "neurosymbolic", "framework/framewerx/src/neurosymbolic/reasoning.mg",
        "End-to-end-trained REALM-style RAG"),
    ("VectorIndex", "neurosymbolic", "framework/framewerx/src/neurosymbolic/reasoning.mg",
        "FAISS-style HNSW / IVF-PQ index"),
    ("ToolCall", "neurosymbolic", "framework/framewerx/src/neurosymbolic/reasoning.mg",
        "Function-calling tool spec"),
    ("ToolUsingAgent", "neurosymbolic", "framework/framewerx/src/neurosymbolic/reasoning.mg",
        "LLM that dispatches to external tools"),

    // ── Neurosymbolic: verification / safety ────────────────────────
    ("NeuralVerifier", "verification", "framework/framewerx/src/neurosymbolic/verification.mg",
        "Sound NN verification (CROWN/IBP/Marabou)"),
    ("RandomizedSmoothing", "verification", "framework/framewerx/src/neurosymbolic/verification.mg",
        "Certified-robust via randomized smoothing"),
    ("ConstraintProjection", "verification", "framework/framewerx/src/neurosymbolic/verification.mg",
        "Project output into feasible polytope"),
    ("ConformalPredictor", "verification", "framework/framewerx/src/neurosymbolic/verification.mg",
        "Distribution-free coverage guarantee"),
    ("AbstentionGate", "verification", "framework/framewerx/src/neurosymbolic/verification.mg",
        "Confidence-threshold refusal gate"),
    ("ConstitutionalGuard", "verification", "framework/framewerx/src/neurosymbolic/verification.mg",
        "Rule-set veto on outputs"),

    // ── Agentic: reasoning patterns ─────────────────────────────────
    ("ChainOfThought", "agentic_pattern", "framework/framewerx/src/agentic/reasoning_patterns.mg",
        "Step-by-step reasoning wrapper"),
    ("TreeOfThoughts", "agentic_pattern", "framework/framewerx/src/agentic/reasoning_patterns.mg",
        "Branching exploration over reasoning paths"),
    ("GraphOfThoughts", "agentic_pattern", "framework/framewerx/src/agentic/reasoning_patterns.mg",
        "DAG over partial-result nodes"),
    ("ReAct", "agentic_pattern", "framework/framewerx/src/agentic/reasoning_patterns.mg",
        "Interleaved Reason / Act / Observe loop"),
    ("Reflexion", "agentic_pattern", "framework/framewerx/src/agentic/reasoning_patterns.mg",
        "Self-critique then retry"),
    ("SelfConsistency", "agentic_pattern", "framework/framewerx/src/agentic/reasoning_patterns.mg",
        "Majority-vote over sampled CoT paths"),
    ("PlanAndSolve", "agentic_pattern", "framework/framewerx/src/agentic/reasoning_patterns.mg",
        "Explicit plan before execution"),
    ("SkeletonOfThought", "agentic_pattern", "framework/framewerx/src/agentic/reasoning_patterns.mg",
        "Parallel completion of independent points"),

    // ── Agentic: multi-agent ────────────────────────────────────────
    ("MultiAgentDebate", "agentic_multi", "framework/framewerx/src/agentic/multi_agent.mg",
        "Agents argue, judge picks winner"),
    ("ConstitutionalAI", "agentic_multi", "framework/framewerx/src/agentic/multi_agent.mg",
        "Critic-revisor loop guided by principles"),
    ("RLHF", "agentic_multi", "framework/framewerx/src/agentic/multi_agent.mg",
        "Reward model + PPO on policy"),
    ("DPO", "agentic_multi", "framework/framewerx/src/agentic/multi_agent.mg",
        "Direct Preference Optimization"),
    ("HierarchicalAgent", "agentic_multi", "framework/framewerx/src/agentic/multi_agent.mg",
        "Planner delegates to worker agents"),
    ("SwarmOrchestrator", "agentic_multi", "framework/framewerx/src/agentic/multi_agent.mg",
        "Role-based dispatch + shared workspace"),
    ("SkillLibrary", "agentic_multi", "framework/framewerx/src/agentic/multi_agent.mg",
        "Retrievable named-skill registry"),
    ("WorldModelAgent", "agentic_multi", "framework/framewerx/src/agentic/multi_agent.mg",
        "Plans via rollouts in learned world model"),

    // ── Agentic: decoding ───────────────────────────────────────────
    ("GreedyDecode", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Argmax at each step"),
    ("BeamSearch", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Width-K beam with length penalty"),
    ("TopKSampling", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Sample from top-K logits"),
    ("TopPSampling", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Nucleus (top-p) sampling"),
    ("MinPSampling", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Min-p sampling threshold"),
    ("MirostatSampling", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Target-perplexity sampling"),
    ("SpeculativeDecoding", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Draft model proposes, target verifies"),
    ("Medusa", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Multi-head self-speculative decoding"),
    ("ConstrainedDecode", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Grammar / regex / JSON-schema constrained"),
    ("StructuredGenerator", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Schema-driven structured generation"),
    ("SelfSpeculative", "decoding", "framework/framewerx/src/agentic/decoding.mg",
        "Self-speculative drafting"),

    // ── Spec contracts ──────────────────────────────────────────────
    ("ModuleForward", "spec", "framework/framewerx/src/specs.mg",
        "Pure forward pass; output shape must be non-empty"),
    ("OptimStep", "spec", "framework/framewerx/src/specs.mg",
        "Optimizer update; step counter strictly increases"),
    ("LossEvaluation", "spec", "framework/framewerx/src/specs.mg",
        "Loss function: pure, shape-matched, non-negative output"),
    ("HybridVerification", "spec", "framework/framewerx/src/specs.mg",
        "Hybrid output: verified flag or non-empty rationale"),
    ("TrainStep", "spec", "framework/framewerx/src/specs.mg",
        "Training step: io-effect-only, increments step by 1"),

    // ── Neurosymbolic ───────────────────────────────────────────────
    ("Hybrid", "neurosymbolic", "framework/framewerx/src/neurosymbolic.mg",
        "Compose net (neural) with kb (symbolic) for verified output"),
    ("HybridOutput", "neurosymbolic", "framework/framewerx/src/neurosymbolic.mg",
        "Output of a Hybrid: { value, verified, rationale }"),

    // ── Worked examples (parse-verified by ontology test) ───────────
    ("mlp_classifier", "example", "framework/framewerx/examples/mlp_classifier.mg",
        "784->128->64->10 MLP for digit classification"),
    ("transformer_block", "example", "framework/framewerx/examples/transformer_block.mg",
        "Pre-norm transformer block: norm/attn/norm/ffn"),
    ("neurosymbolic_qa", "example", "framework/framewerx/examples/neurosymbolic_qa.mg",
        "Neural Retriever validated against symbolic FactBase"),
    ("resnet_classifier", "example", "framework/framewerx/examples/resnet_classifier.mg",
        "ResNet-style image classifier (conv stem + residual blocks)"),
    ("vit_classifier", "example", "framework/framewerx/examples/vit_classifier.mg",
        "Vision Transformer for image classification"),
    ("lstm_lm", "example", "framework/framewerx/examples/lstm_lm.mg",
        "LSTM-based language model over 50k vocab"),
    ("mamba_lm", "example", "framework/framewerx/examples/mamba_lm.mg",
        "Mamba-style state-space language model"),
    ("vae_mnist", "example", "framework/framewerx/examples/vae_mnist.mg",
        "Variational Autoencoder for MNIST (encoder + decoder pair)"),
    ("gan_simple", "example", "framework/framewerx/examples/gan_simple.mg",
        "Generator + Discriminator pair for image GAN training"),
    ("flash_attention_block", "example", "framework/framewerx/examples/flash_attention_block.mg",
        "Transformer block with FlashAttention variant (exercises P78 mapping)"),
    ("gqa_llama_style", "example", "framework/framewerx/examples/gqa_llama_style.mg",
        "Llama-style decoder: GQA + RMSNorm + SwiGLU"),
    ("lora_finetune", "example", "framework/framewerx/examples/lora_finetune.mg",
        "LoRA fine-tuning template over a frozen Linear base"),
    ("mixture_of_experts", "example", "framework/framewerx/examples/mixture_of_experts.mg",
        "Sparse MoE block with TopKRouter and N experts"),
];

/// The 5-stage recovery pipeline (4 mechanical + agent.refine).
const RECOVERY_STAGES: &[(&str, &str)] = &[
    ("already-valid", "Source parsed on first try; no recovery applied."),
    ("pattern-heal", "Multi-pass over ranked heal::heal_one candidates."),
    ("structural-balance", "Append matching closers at EOF for unbalanced (/[/{."),
    ("structural-completion", "Splice ()/0/_ placeholder after trailing operator, then re-balance."),
    ("trim-bad-token", "Delete the bad token at the parse error position, or its neighbor."),
    ("agent.refine", "Stage 3: re-prompt the agent backend with broken source + parse error."),
    ("failed", "All stages exhausted; source unchanged."),
];

/// Build the complete ontology as a single JSON value.
pub fn build() -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "project": "Machine Genetics",
        "short_name": "MAGE",
        "organization": "NERVOSYS",
        "builtin_framework": "RecursiveMachineIntelligence (rmi crate) — the built-in agentic-first AI framework",
        "version": "1.0",
        "schema_version": 1,
        "sections": {
            "sigils": sigils_section(),
            "keywords": keywords_section(),
            "types": types_section(),
            "ast_kinds": ast_kinds_section(),
            "ir_ops": ir_ops_section(),
            "op_families": op_families_section(),
            "layer_map": layer_map_section(),
            "rap_methods": rap_methods_section(),
            "heal_patterns": heal_patterns_section(),
            "recovery_stages": recovery_stages_section(),
            "abl": abl_section(),
            "examples": examples_section(),
            "framewerx_modules": framewerx_modules_section(),
            "cli_flags": cli_flags_section(),
            "bench_backends": bench_backends_section(),
            "effects": effects_section(),
            "vocabulary": vocabulary_section(),
            "wrapper_protocol": wrapper_protocol_section(),
            "project_layout": project_layout_section(),
            "docs": docs_section(),
            "ci_floors": ci_floors_section(),
            "hardware_accelerators": hardware_accelerators_section(),
        },
        "counts": {
            "sigils": SIGILS.len(),
            "keywords": crate::lexer::KEYWORDS.len(),
            "types": TYPES.len(),
            "ast_kinds": AST_KINDS.len(),
            "ir_ops": Op::ALL.len(),
            "op_families": OP_FAMILIES.len(),
            "rap_methods": RAP_METHODS.len(),
            "recovery_stages": RECOVERY_STAGES.len(),
            "examples": EXAMPLES.len(),
            "framewerx_modules": FRAMEWERX_MODULES.len(),
            "cli_flags": CLI_FLAGS.len(),
            "bench_backends": BENCH_BACKENDS.len(),
            "effects": EFFECTS.len(),
            "vocabulary": crate::resolve::VOCABULARY.len(),
            "wrapper_protocol": WRAPPER_PROTOCOL.len(),
            "project_layout": PROJECT_LAYOUT.len(),
            "docs": DOCS.len(),
            "ci_floors": CI_FLOORS.len(),
            "hardware_accelerators": crate::backends::all_descriptors().len(),
        },
    })
}

/// Return one named section by string key, or `None` if unknown.
pub fn section(name: &str) -> Option<serde_json::Value> {
    Some(match name {
        "sigils" => sigils_section(),
        "keywords" => keywords_section(),
        "types" => types_section(),
        "ast_kinds" => ast_kinds_section(),
        "ir_ops" => ir_ops_section(),
        "op_families" => op_families_section(),
        "layer_map" => layer_map_section(),
        "rap_methods" => rap_methods_section(),
        "heal_patterns" => heal_patterns_section(),
        "recovery_stages" => recovery_stages_section(),
        "abl" => abl_section(),
        "examples" => examples_section(),
        "framewerx_modules" => framewerx_modules_section(),
        "cli_flags" => cli_flags_section(),
        "bench_backends" => bench_backends_section(),
        "effects" => effects_section(),
        "vocabulary" => vocabulary_section(),
        "wrapper_protocol" => wrapper_protocol_section(),
        "project_layout" => project_layout_section(),
        "docs" => docs_section(),
        "ci_floors" => ci_floors_section(),
        "hardware_accelerators" => hardware_accelerators_section(),
        _ => return None,
    })
}

fn sigils_section() -> serde_json::Value {
    let items: Vec<_> = SIGILS
        .iter()
        .map(|(s, k, d)| serde_json::json!({ "sigil": s, "ast_kind": k, "summary": d }))
        .collect();
    serde_json::json!(items)
}

fn keywords_section() -> serde_json::Value {
    // Enumerate the AUTHORITATIVE keyword table the lexer uses, so the ontology
    // covers every reserved word the compiler accepts (no drift, no gaps).
    // Sorted by spelling for deterministic output. Curated prose is merged from
    // KEYWORD_DOCS where available; otherwise a summary is generated from the
    // token the spelling produces.
    let mut rows: Vec<(&str, String, String)> = crate::lexer::KEYWORDS
        .iter()
        .map(|(spelling, kind)| {
            let summary = KEYWORD_DOCS
                .iter()
                .find(|(w, _, _)| w == spelling)
                .map(|(_, _, summary)| summary.to_string())
                .unwrap_or_else(|| format!("reserved word → {kind:?} token"));
            (*spelling, format!("{kind:?}"), summary)
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(b.0));
    let items: Vec<_> = rows
        .iter()
        .map(|(w, k, d)| serde_json::json!({ "keyword": w, "introduces": k, "summary": d }))
        .collect();
    serde_json::json!(items)
}

fn types_section() -> serde_json::Value {
    let items: Vec<_> = TYPES
        .iter()
        .map(|(n, c, d)| serde_json::json!({ "name": n, "category": c, "summary": d }))
        .collect();
    serde_json::json!(items)
}

/// The standard SWE vocabulary (§8) — enumerated from the authoritative
/// `resolve::VOCABULARY` table so an agent discovers every combinator (name,
/// signature, summary) from the cached ontology without spending tokens to
/// rediscover it. No drift: the same table drives name resolution + typing.
fn vocabulary_section() -> serde_json::Value {
    let items: Vec<_> = crate::resolve::VOCABULARY
        .iter()
        .map(|(n, sig, d)| serde_json::json!({ "name": n, "signature": sig, "summary": d }))
        .collect();
    serde_json::json!(items)
}

fn ast_kinds_section() -> serde_json::Value {
    let items: Vec<_> = AST_KINDS
        .iter()
        .map(|(k, d)| serde_json::json!({ "kind": k, "summary": d }))
        .collect();
    serde_json::json!(items)
}

fn ir_ops_section() -> serde_json::Value {
    let items: Vec<_> = Op::ALL
        .iter()
        .map(|op| {
            let meta = op.meta();
            serde_json::json!({
                "name": meta.name,
                "opcode": format!("0x{:04x}", op.0),
                "family": op.family(),
                "arity": meta.arity,
                "differentiable": meta.differentiable,
                "has_params": meta.has_params,
                "stateful": meta.stateful,
                "summary": meta.desc,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn op_families_section() -> serde_json::Value {
    let items: Vec<_> = OP_FAMILIES
        .iter()
        .map(|(name, code, desc)| {
            serde_json::json!({ "name": name, "code": code, "summary": desc })
        })
        .collect();
    serde_json::json!(items)
}

fn layer_map_section() -> serde_json::Value {
    let items: Vec<_> = LAYER_SURFACE_NAMES
        .iter()
        .filter_map(|name| {
            crate::abl_bridge::layer_name_to_op(name).map(|op| {
                serde_json::json!({
                    "surface_name": name,
                    "opcode": format!("0x{:04x}", op.0),
                    "op_name": op.name(),
                    "family": op.family(),
                })
            })
        })
        .collect();
    serde_json::json!(items)
}

fn rap_methods_section() -> serde_json::Value {
    let items: Vec<_> = RAP_METHODS
        .iter()
        .map(|(name, summary, inputs, outputs)| {
            serde_json::json!({
                "method": name,
                "summary": summary,
                "params": inputs,
                "returns": outputs,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn heal_patterns_section() -> serde_json::Value {
    let names = crate::heal::pattern_names();
    let items: Vec<_> = names
        .iter()
        .map(|n| serde_json::json!({ "name": n }))
        .collect();
    serde_json::json!(items)
}

fn recovery_stages_section() -> serde_json::Value {
    let items: Vec<_> = RECOVERY_STAGES
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| {
            serde_json::json!({ "order": i, "stage": name, "summary": desc })
        })
        .collect();
    serde_json::json!(items)
}

fn cli_flags_section() -> serde_json::Value {
    let items: Vec<_> = CLI_FLAGS
        .iter()
        .map(|(flag, purpose, takes_path)| {
            serde_json::json!({
                "flag": flag,
                "purpose": purpose,
                "takes_path": takes_path,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn bench_backends_section() -> serde_json::Value {
    let items: Vec<_> = BENCH_BACKENDS
        .iter()
        .map(|(name, mode, purpose)| {
            serde_json::json!({
                "name": name,
                "mode": mode,
                "purpose": purpose,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn effects_section() -> serde_json::Value {
    let items: Vec<_> = EFFECTS
        .iter()
        .map(|(name, slot, purpose)| {
            serde_json::json!({
                "name": name,
                "slot": slot,
                "purpose": purpose,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn wrapper_protocol_section() -> serde_json::Value {
    let items: Vec<_> = WRAPPER_PROTOCOL
        .iter()
        .map(|(name, kind, purpose)| {
            serde_json::json!({
                "name": name,
                "kind": kind,
                "purpose": purpose,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn project_layout_section() -> serde_json::Value {
    let items: Vec<_> = PROJECT_LAYOUT
        .iter()
        .map(|(path, purpose)| {
            serde_json::json!({
                "path": path,
                "purpose": purpose,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn docs_section() -> serde_json::Value {
    let items: Vec<_> = DOCS
        .iter()
        .map(|(path, purpose, audience)| {
            serde_json::json!({
                "path": path,
                "purpose": purpose,
                "audience": audience,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn ci_floors_section() -> serde_json::Value {
    let items: Vec<_> = CI_FLOORS
        .iter()
        .map(|(name, threshold, protects)| {
            serde_json::json!({
                "name": name,
                "threshold": threshold,
                "protects": protects,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn framewerx_modules_section() -> serde_json::Value {
    let items: Vec<_> = FRAMEWERX_MODULES
        .iter()
        .map(|(name, category, path, summary)| {
            serde_json::json!({
                "name": name,
                "category": category,
                "path": path,
                "summary": summary,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn examples_section() -> serde_json::Value {
    let items: Vec<_> = EXAMPLES
        .iter()
        .map(|(name, desc, src, exercises)| {
            serde_json::json!({
                "name": name,
                "description": desc,
                "source": src,
                "exercises": exercises,
                "bytes": src.len(),
            })
        })
        .collect();
    serde_json::json!(items)
}

fn abl_section() -> serde_json::Value {
    serde_json::json!({
        "magic": std::str::from_utf8(crate::abl::ABL_MAGIC).unwrap_or("Agentic Binary Language"),
        "version": crate::abl::ABL_VERSION,
        "format": [
            "magic    : 4 bytes (\"Agentic Binary Language\")",
            "version  : u16 LE",
            "count    : u32 LE",
            "per item : { name_len:u32, name:utf8, expr_len:u32, expr:bytes }",
        ],
        "media_type": "application/abl",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ontology_vocabulary_covers_every_registered_combinator() {
        // §8c drift guard: the ontology's vocabulary section must list every
        // combinator in the authoritative resolve::VOCABULARY table, so an agent
        // discovers the full standard vocabulary (name, signature) from the
        // cached ontology — the discoverability the token saving depends on.
        let section = vocabulary_section();
        let listed: std::collections::HashSet<String> = section
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect();
        for (name, _sig, _doc) in crate::resolve::VOCABULARY {
            assert!(
                listed.contains(*name),
                "ontology vocabulary section is missing `{name}`"
            );
        }
        assert_eq!(listed.len(), crate::resolve::VOCABULARY.len());
    }

    #[test]
    fn ontology_keywords_cover_every_lexer_keyword() {
        // Drift guard: the ontology's keyword section must list EVERY spelling
        // the lexer recognises — so an agent grounding in the ontology sees the
        // complete, ground-truth keyword surface (no gaps, can't drift).
        let section = keywords_section();
        let listed: std::collections::HashSet<String> = section
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["keyword"].as_str().unwrap().to_string())
            .collect();
        for (spelling, _) in crate::lexer::KEYWORDS {
            assert!(
                listed.contains(*spelling),
                "ontology keyword section is missing `{spelling}` (lexer recognises it)"
            );
        }
        assert_eq!(
            listed.len(),
            crate::lexer::KEYWORDS.len(),
            "ontology keyword count must equal the lexer keyword table"
        );
    }

    #[test]
    fn build_returns_all_sections() {
        let o = build();
        assert_eq!(o["ok"], true);
        assert_eq!(o["schema_version"], 1);
        let sections = o["sections"].as_object().expect("sections obj");
        for required in [
            "sigils", "keywords", "types", "ast_kinds", "ir_ops", "op_families",
            "layer_map", "rap_methods", "heal_patterns", "recovery_stages", "abl",
            "examples", "framewerx_modules",
            // P82 - operational discoverability
            "cli_flags", "bench_backends", "effects", "wrapper_protocol",
            "project_layout", "docs", "ci_floors",
            // P91 - hardware accelerators
            "hardware_accelerators",
        ] {
            assert!(sections.contains_key(required), "missing section: {required}");
            assert!(!sections[required].is_null(), "null section: {required}");
            // `abl` is an object describing the container layout;
            // every other section is an array.
            if required != "abl" {
                let arr = sections[required].as_array()
                    .unwrap_or_else(|| panic!("section {required} not an array"));
                assert!(!arr.is_empty(), "section {required} is empty");
            }
        }
    }

    /// Every flag `main.rs` recognises is published, and every published flag
    /// exists in `main.rs`.
    ///
    /// The previous version asserted that eight named flags were present,
    /// under a doc comment claiming it checked "every CLI flag the binary
    /// actually accepts". Twelve flags were missing from the ontology,
    /// including `--eval` — the only way to run a program, and half of the
    /// two-oracle discipline this repository depends on.
    ///
    /// Source-scraping is the point: the two sides are a `match` in a binary
    /// and a table in a library, and nothing else connects them.
    #[test]
    fn every_flag_in_main_is_published_and_vice_versa() {
        let main_rs = include_str!("main.rs");

        // Flag literals in `main.rs`: `"--name"`, possibly with `=value`.
        let mut in_main: Vec<String> = Vec::new();
        let bytes = main_rs.as_bytes();
        let mut i = 0;
        while let Some(off) = main_rs[i..].find("\"--") {
            let start = i + off + 1;
            let end = match main_rs[start..].find('"') {
                Some(e) => start + e,
                None => break,
            };
            let lit = &main_rs[start..end];
            i = end + 1;
            let _ = bytes;
            if lit.len() > 2
                && lit[2..].chars().all(|c| c.is_ascii_lowercase() || "=-0123456789".contains(c))
            {
                in_main.push(lit.to_string());
            }
        }

        // `--backend=cpu` is a value comparison, not a flag; the flag itself
        // is published as `--backend=<name>`. Same for the two prefixes that
        // are matched with `strip_prefix`.
        let published: Vec<&str> = CLI_FLAGS.iter().map(|(f, _, _)| *f).collect();
        let normalise = |f: &str| -> String {
            match f {
                "--backend=cpu" | "--backend=" => "--backend=<name>".to_string(),
                "--backends-file=" => "--backends-file=<path>".to_string(),
                other => other.to_string(),
            }
        };

        for lit in &in_main {
            let want = normalise(lit);
            assert!(
                published.contains(&want.as_str()),
                "`{lit}` is accepted by main.rs and not published in CLI_FLAGS"
            );
        }
        for flag in &published {
            let stem = flag.split('=').next().unwrap();
            assert!(
                main_rs.contains(&format!("\"{stem}")),
                "`{flag}` is published and does not appear in main.rs"
            );
        }
    }

    #[test]
    fn bench_backends_section_lists_all_four() {
        let arr = bench_backends_section();
        let v = arr.as_array().unwrap();
        let names: Vec<&str> = v.iter().filter_map(|e| e["name"].as_str()).collect();
        for required in ["file-oracle", "perturbed",
                         "subprocess:<cmd>", "perturbed+refine:<cmd>"] {
            assert!(names.contains(&required), "missing backend: {required}");
        }
    }

    #[test]
    fn effects_section_covers_annotations_and_names() {
        let arr = effects_section();
        let v = arr.as_array().unwrap();
        let names: Vec<&str> = v.iter().filter_map(|e| e["name"].as_str()).collect();
        // `db` used to be in this list. It is not a built-in effect kind, so
        // `/ db` was an unknown-effect error — the test asserted the ontology
        // published the name and never asked whether the name worked. Which
        // names belong here is now `builtin_effect_names_are_the_writable_ones`.
        for required in ["@fx", "@req", "@ens", "@inv",
                         "io", "fs", "net", "async", "llm", "rng", "agent"] {
            assert!(names.contains(&required), "missing effect entry: {required}");
        }
    }

    #[test]
    fn wrapper_protocol_section_lists_env_and_streams() {
        let arr = wrapper_protocol_section();
        let v = arr.as_array().unwrap();
        let names: Vec<&str> = v.iter().filter_map(|e| e["name"].as_str()).collect();
        for required in ["RDX_BENCH_MODE", "RDX_TASK_ID",
                         "RDX_PARSE_ERROR", "stdout", "stderr"] {
            assert!(names.contains(&required), "missing protocol entry: {required}");
        }
    }

    #[test]
    fn project_layout_section_lists_top_dirs() {
        let arr = project_layout_section();
        let v = arr.as_array().unwrap();
        let paths: Vec<&str> = v.iter().filter_map(|e| e["path"].as_str()).collect();
        for required in ["prototype/", "RecursiveMachineIntelligence/",
                         "framework/framewerx/", "benchmarks/",
                         "scripts/agent_wrappers/", "UNIFICATION.md"] {
            assert!(paths.contains(&required), "missing layout entry: {required}");
        }
    }

    #[test]
    fn docs_section_lists_canonical_docs() {
        let arr = docs_section();
        let v = arr.as_array().unwrap();
        let paths: Vec<&str> = v.iter().filter_map(|e| e["path"].as_str()).collect();
        for required in ["UNIFICATION.md", "benchmarks/STATUS.md",
                         "MAGE_ONTOLOGY.json"] {
            assert!(paths.contains(&required), "missing doc entry: {required}");
        }
    }

    #[test]
    fn hardware_accelerators_section_lists_backends() {
        let arr = hardware_accelerators_section();
        let v = arr.as_array().unwrap();
        let names: Vec<&str> = v.iter().filter_map(|e| e["name"].as_str()).collect();
        for required in ["cpu", "cuda", "metal", "apple_ane", "vulkan", "webgpu", "qualcomm"] {
            assert!(names.contains(&required), "missing backend: {required}");
        }
        // CPU must always be marked available_at_runtime.
        let cpu = v.iter().find(|e| e["name"] == "cpu").expect("cpu entry");
        assert_eq!(cpu["available_at_runtime"], true,
            "cpu must be available at runtime");
        // Every entry must have all five descriptive fields.
        for entry in v {
            for f in ["name", "family", "vendor", "requires", "summary", "available_at_runtime"] {
                assert!(!entry[f].is_null(), "entry missing field {f}: {entry:?}");
            }
        }
    }

    #[test]
    fn ci_floors_section_lists_floors() {
        let arr = ci_floors_section();
        let v = arr.as_array().unwrap();
        let names: Vec<&str> = v.iter().filter_map(|e| e["name"].as_str()).collect();
        for required in ["MIN_PARSE", "MIN_HEAL", "native-lexer ratio"] {
            assert!(names.contains(&required), "missing floor entry: {required}");
        }
    }

    #[test]
    fn counts_match_data() {
        let o = build();
        let counts = &o["counts"];
        assert_eq!(counts["sigils"].as_u64().unwrap(), SIGILS.len() as u64);
        assert!(counts["ir_ops"].as_u64().unwrap() > 50, "expected many ops");
        assert_eq!(counts["op_families"], 7);
    }

    #[test]
    fn ir_ops_have_meta() {
        let s = ir_ops_section();
        let arr = s.as_array().expect("array");
        assert!(!arr.is_empty());
        // Every entry has required keys.
        for entry in arr {
            assert!(entry["name"].is_string());
            assert!(entry["opcode"].as_str().unwrap().starts_with("0x"));
            assert!(entry["arity"].is_number());
        }
    }

    #[test]
    fn layer_map_resolves_to_real_ops() {
        let s = layer_map_section();
        let arr = s.as_array().expect("array");
        // Linear and ReLU are load-bearing — they must always be present.
        let names: Vec<&str> = arr
            .iter()
            .filter_map(|e| e["surface_name"].as_str())
            .collect();
        assert!(names.contains(&"Linear"), "missing Linear; got {names:?}");
        assert!(names.contains(&"ReLU"), "missing ReLU; got {names:?}");
    }

    #[test]
    fn rap_methods_includes_self() {
        // Sanity: the ontology methods themselves are registered.
        let s = rap_methods_section();
        let arr = s.as_array().unwrap();
        let names: Vec<&str> = arr
            .iter()
            .filter_map(|e| e["method"].as_str())
            .collect();
        assert!(names.contains(&"ontology/full"));
        assert!(names.contains(&"ontology/section"));
        assert!(names.contains(&"build/recover"));
        assert!(names.contains(&"abl/encode"));
    }

    #[test]
    fn section_lookup_works() {
        assert!(section("sigils").is_some());
        assert!(section("ir_ops").is_some());
        assert!(section("nonexistent").is_none());
    }

    #[test]
    fn heal_patterns_section_nonempty() {
        let arr = heal_patterns_section();
        assert!(arr.as_array().unwrap().len() >= 10);
    }

    #[test]
    fn recovery_stages_in_order() {
        let arr = recovery_stages_section();
        let v = arr.as_array().unwrap();
        assert_eq!(v[0]["stage"], "already-valid");
        assert_eq!(v[1]["stage"], "pattern-heal");
        assert_eq!(v.last().unwrap()["stage"], "failed");
    }

    #[test]
    fn types_section_covers_scalars_and_constructors() {
        let arr = types_section();
        let v = arr.as_array().unwrap();
        let names: Vec<&str> = v.iter().filter_map(|e| e["name"].as_str()).collect();
        for required in ["i32", "f64", "bool", "s", "?T", "[T]~", "&T"] {
            assert!(names.contains(&required), "missing type: {required}");
        }
        // Category column must be set on every row.
        for entry in v {
            assert!(
                entry["category"].as_str().filter(|c| !c.is_empty()).is_some(),
                "category missing on {:?}",
                entry["name"]
            );
        }
    }

    #[test]
    fn examples_section_populated() {
        let arr = examples_section();
        let v = arr.as_array().unwrap();
        assert!(v.len() >= 8, "expected at least 8 examples; got {}", v.len());
        // Spot-check the load-bearing entries are present and shaped right.
        let names: Vec<&str> = v.iter().filter_map(|e| e["name"].as_str()).collect();
        for required in ["hello-world", "net-linear", "kb-rule"] {
            assert!(names.contains(&required), "missing example: {required}");
        }
        for entry in v {
            assert!(!entry["source"].as_str().unwrap().is_empty());
            assert!(!entry["exercises"].as_array().unwrap().is_empty());
        }
    }

    /// **Load-bearing invariant**: every example in the ontology must
    /// parse cleanly. If a sigil/keyword changes shape, this test
    /// surfaces the breakage at the example layer first — before any
    /// agent reads a stale pattern. Re-running the bench would catch
    /// it too, but the ontology examples are the agent-facing
    /// recommendations and need a tighter feedback loop.
    #[test]
    fn framewerx_modules_section_populated() {
        let arr = framewerx_modules_section();
        let v = arr.as_array().unwrap();
        assert!(v.len() >= 180, "expected at least 180 framewerx entries");
        let names: Vec<&str> = v.iter().filter_map(|e| e["name"].as_str()).collect();
        for required in [
            // base
            "Module", "Linear", "Attention", "Adam", "Hybrid", "mlp_classifier",
            "Embedding", "Dropout", "SELU", "LSTM", "GCNLayer", "MambaBlock",
            "VAE", "GAN", "ViT", "Mamba",
            // neural breadth
            "FlashAttention", "GroupedQueryAttention", "KVCache",
            "SparseMoE", "LoRA", "QLoRA", "BitNetLinear",
            "NeuralODE", "LIF", "ModernHopfield",
            "EBM", "NormalizingFlow", "NeuralTuringMachine",
            "CLIP", "MAE", "PerceiverIO",
            "JEPA", "DreamerV3", "DiT", "ConsistencyModel",
            // symbolic
            "Unifier", "SLDResolver", "DLOntology", "TableauReasoner",
            "SATSolver", "SMTSolver", "TheoremProver",
            "STRIPSPlanner", "MCTS", "POMDP",
            "BayesianNetwork", "MCMCSampler", "StructuralCausalModel", "DoCalculus",
            "KnowledgeGraph", "TransE", "OWLReasoner",
            // neurosymbolic
            "LogicTensorNetwork", "DeepProbLog", "SemanticLoss",
            "NeuralTheoremProver", "MarkovLogicNetwork",
            "ConceptBottleneckModel", "RAG", "ToolUsingAgent",
            "NeuralVerifier", "ConformalPredictor", "AbstentionGate",
            // agentic
            "ChainOfThought", "TreeOfThoughts", "ReAct", "Reflexion",
            "SelfConsistency", "MultiAgentDebate", "ConstitutionalAI",
            "RLHF", "DPO", "SpeculativeDecoding", "Medusa", "ConstrainedDecode",
        ] {
            assert!(names.contains(&required), "missing entry: {required}");
        }
        // Category column must be set on every row.
        for entry in v {
            assert!(entry["category"].as_str().filter(|c| !c.is_empty()).is_some());
            assert!(entry["path"].as_str().filter(|p| p.starts_with("framework/")).is_some());
        }
    }

    /// **Load-bearing invariant**: every `.mg` source file referenced
    /// by the `framewerx_modules` ontology section must EXIST and parse
    /// cleanly. If the framework directory isn't a sibling of the
    /// prototype/ build (e.g. publishing prototype on its own), the
    /// test resolves both `../framework/...` and `<workspace>/framework/...`
    /// paths and only skips if neither exists. **A registered path that
    /// resolves nowhere fails the test** - the ontology must not point
    /// agents at 404s.
    #[test]
    fn framewerx_source_files_all_parse() {
        use std::collections::BTreeSet;
        let arr = framewerx_modules_section();
        let v = arr.as_array().unwrap();
        let paths: BTreeSet<&str> = v
            .iter()
            .filter_map(|e| e["path"].as_str())
            .collect();

        // Try a small set of resolution roots so the test works both
        // from `cargo test` in prototype/ and from the workspace root.
        let cwd = std::env::current_dir().unwrap();
        let roots: Vec<std::path::PathBuf> = vec![
            cwd.clone(),
            cwd.join(".."),
            cwd.join("../.."),
        ];

        let mut any_resolved = false;
        for path in paths {
            let mut resolved: Option<std::path::PathBuf> = None;
            for root in &roots {
                let candidate = root.join(path);
                if candidate.exists() {
                    resolved = Some(candidate);
                    break;
                }
            }
            let Some(file) = resolved else {
                // If at least one path resolved already, the framework
                // directory IS present and this specific entry is a
                // 404 - fail. If NOTHING resolves, the whole framework/
                // tree is absent (e.g. installing prototype alone);
                // skip the entire test in that case.
                if any_resolved {
                    panic!("ontology entry path does not exist: {path}");
                }
                continue;
            };
            any_resolved = true;
            let source = std::fs::read_to_string(&file)
                .unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
            let tokens = crate::lexer::lex(&source);
            let lex_errors: Vec<_> = tokens
                .iter()
                .filter(|t| t.kind == crate::lexer::TokenKind::Error)
                .collect();
            assert!(
                lex_errors.is_empty(),
                "{} has lex errors: {lex_errors:?}",
                file.display()
            );
            let result = crate::parser::parse(&tokens);
            assert!(
                result.is_ok(),
                "{} failed to parse: {:?}",
                file.display(),
                result.err()
            );
        }
    }

    /// End-to-end integration test of the JAX:FLAX :: RMI:RecursiveMachineIntelligence-MG
    /// architecture. Walks every RecursiveMachineIntelligence-MG example through the full
    /// stack: source -> parse -> abl_bridge::lower_module -> Agentic Binary Language
    /// encode -> decode -> assert per-item structural invariants.
    ///
    /// If the bridge stops routing `net` blocks to Agentic Binary Language, or the codec
    /// loses fidelity, this test fires - end-to-end at the layer where
    /// agents actually use the framework.
    #[test]
    fn framewerx_examples_compile_to_ml() {
        let cwd = std::env::current_dir().unwrap();
        let roots: Vec<std::path::PathBuf> = vec![
            cwd.clone(),
            cwd.join(".."),
            cwd.join("../.."),
        ];
        let examples = [
            "framework/framewerx/examples/mlp_classifier.mg",
            "framework/framewerx/examples/transformer_block.mg",
            "framework/framewerx/examples/resnet_classifier.mg",
            "framework/framewerx/examples/vit_classifier.mg",
            "framework/framewerx/examples/lstm_lm.mg",
            "framework/framewerx/examples/mamba_lm.mg",
            "framework/framewerx/examples/vae_mnist.mg",
            "framework/framewerx/examples/gan_simple.mg",
            "framework/framewerx/examples/flash_attention_block.mg",
            "framework/framewerx/examples/gqa_llama_style.mg",
            "framework/framewerx/examples/lora_finetune.mg",
            "framework/framewerx/examples/mixture_of_experts.mg",
        ];

        let mut any_resolved = false;
        for rel in examples {
            let mut resolved: Option<std::path::PathBuf> = None;
            for root in &roots {
                let p = root.join(rel);
                if p.exists() {
                    resolved = Some(p);
                    break;
                }
            }
            let Some(file) = resolved else {
                if any_resolved {
                    panic!("missing example: {rel}");
                }
                continue;
            };
            any_resolved = true;

            // 1. parse
            let source = std::fs::read_to_string(&file).unwrap();
            let tokens = crate::lexer::lex(&source);
            let module = crate::parser::parse(&tokens)
                .unwrap_or_else(|e| panic!("{}: parse: {e:?}", file.display()));

            // 2. lower via the bridge - should produce Agentic Binary Language items
            let lowered = crate::abl_bridge::lower_module(&module);
            assert!(
                !lowered.items.is_empty(),
                "{}: bridge produced no Agentic Binary Language items (net block not recognized?)",
                file.display()
            );

            // 3. round-trip through the Agentic Binary Language codec
            let (blob, summary) = crate::abl::encode_module(&module);
            assert!(blob.len() > 8, "{}: Agentic Binary Language blob too small", file.display());
            assert_eq!(
                summary.len(),
                lowered.items.len(),
                "{}: summary count mismatch",
                file.display()
            );
            let decoded = crate::abl::decode_container(&blob)
                .unwrap_or_else(|e| panic!("{}: decode: {e}", file.display()));
            assert_eq!(
                decoded.len(),
                lowered.items.len(),
                "{}: decode count mismatch",
                file.display()
            );

            // 4. content-hash stability: encode and decode again, hashes
            // must match. Catches any nondeterminism in the codec.
            for (a, b) in lowered.items.iter().zip(decoded.iter()) {
                assert_eq!(
                    a.0,
                    b.name,
                    "{}: item name mismatch",
                    file.display()
                );
                assert_eq!(
                    a.1.content_hash(),
                    b.expr.content_hash(),
                    "{}: content-hash drift between source-lowered and decoded",
                    file.display()
                );
            }
        }
    }

    /// **P88 CI-floor**: every framework example listed below must
    /// not only compile to Agentic Binary Language but DISPATCH cleanly on the CpuBackend
    /// with the auto-inferred input shape, producing a non-degenerate
    /// output shape. CI-locks the P86 sweep so future bridge / op /
    /// inference changes can't silently regress dispatch coverage.
    ///
    /// Output shape sanity: must be rank >= 2 (batch + at least one
    /// content dim) and the leading "batch" dim must be > 0.
    #[test]
    fn framewerx_examples_dispatch_end_to_end() {
        let cwd = std::env::current_dir().unwrap();
        let roots: Vec<std::path::PathBuf> = vec![
            cwd.clone(),
            cwd.join(".."),
            cwd.join("../.."),
        ];
        let examples = [
            "framework/framewerx/examples/mlp_classifier.mg",
            "framework/framewerx/examples/transformer_block.mg",
            "framework/framewerx/examples/resnet_classifier.mg",
            "framework/framewerx/examples/vit_classifier.mg",
            "framework/framewerx/examples/lstm_lm.mg",
            "framework/framewerx/examples/mamba_lm.mg",
            "framework/framewerx/examples/vae_mnist.mg",
            "framework/framewerx/examples/gan_simple.mg",
            "framework/framewerx/examples/flash_attention_block.mg",
            "framework/framewerx/examples/gqa_llama_style.mg",
            "framework/framewerx/examples/lora_finetune.mg",
            "framework/framewerx/examples/mixture_of_experts.mg",
            "framework/framewerx/examples/agent_test_classifier.mg",
            "framework/framewerx/examples/agent_test_flash_block.mg",
            "framework/framewerx/examples/agent_test_gnn.mg",
        ];

        let backend = rmi::compute::cpu::CpuBackend::new();
        let mut any_resolved = false;
        let mut dispatched = 0usize;
        let mut tested = 0usize;
        let mut failures: Vec<String> = Vec::new();

        for rel in examples {
            let mut resolved: Option<std::path::PathBuf> = None;
            for root in &roots {
                let p = root.join(rel);
                if p.exists() {
                    resolved = Some(p);
                    break;
                }
            }
            let Some(file) = resolved else {
                if any_resolved {
                    panic!("missing example: {rel}");
                }
                continue;
            };
            any_resolved = true;
            tested += 1;

            // Compile to Agentic Binary Language, decode each item, dispatch via CpuBackend.
            let source = std::fs::read_to_string(&file).unwrap();
            let tokens = crate::lexer::lex(&source);
            let module = crate::parser::parse(&tokens)
                .unwrap_or_else(|e| panic!("{}: parse: {e:?}", file.display()));
            let lowered = crate::abl_bridge::lower_module(&module);

            for (name, expr) in &lowered.items {
                let shape = crate::abl_compute::infer_input_shape(expr)
                    .unwrap_or_else(|| vec![8]);
                match crate::abl_compute::run_pipeline(&backend, expr, &shape, 1.0) {
                    Ok(r) => {
                        // Output shape sanity: rank >= 2, batch > 0,
                        // and at least one op was dispatched (no
                        // empty pipelines).
                        if r.output.shape.len() < 2 {
                            failures.push(format!(
                                "{}::{name}: output rank {} (expected >= 2): shape={:?}",
                                file.display(), r.output.shape.len(), r.output.shape
                            ));
                        } else if r.output.shape[0] == 0 {
                            failures.push(format!(
                                "{}::{name}: zero batch dim: shape={:?}",
                                file.display(), r.output.shape
                            ));
                        } else if r.dispatched == 0 {
                            failures.push(format!(
                                "{}::{name}: zero ops dispatched (pipeline empty?)",
                                file.display()
                            ));
                        } else {
                            dispatched += 1;
                        }
                    }
                    Err(e) => {
                        failures.push(format!(
                            "{}::{name}: dispatch error with input {:?}: {e}",
                            file.display(), shape
                        ));
                    }
                }
            }
        }

        if !any_resolved {
            // Running from a tree without the framework/ sibling -
            // skip silently (matches the existing -compile_to_ml
            // test's behaviour).
            return;
        }
        assert!(
            failures.is_empty(),
            "{}/{tested} framework examples dispatched; {} failed:\n  {}",
            dispatched,
            failures.len(),
            failures.join("\n  ")
        );
    }

    /// Every name the ontology publishes under `effect_name` must be writable
    /// as `/ name` with no declaration, and every built-in kind must be
    /// published.
    ///
    /// The ontology is what an agent grounds on, so a name here that the
    /// compiler rejects is worse than a missing one — it is an instruction to
    /// emit code that does not check. Four of the ten names formerly listed
    /// (`db`, `log`, `tools`, `rand`) were rejected as unknown effects, and ten
    /// built-in kinds were absent, including `rng` — for which `rand` was
    /// standing in under a name the compiler has never known.
    #[test]
    fn builtin_effect_names_are_the_writable_ones() {
        let published: Vec<&str> = EFFECTS
            .iter()
            .filter(|(_, slot, _)| *slot == "effect_name")
            .map(|(name, _, _)| *name)
            .collect();

        for name in &published {
            let src = format!("+f a() -> i32 / {name} {{ 1 }}");
            let tokens = crate::lexer::lex(&src);
            let module = crate::parser::parse(&tokens)
                .unwrap_or_else(|e| panic!("ontology publishes `/ {name}`, which fails to parse: {e:?}"));
            let diags = crate::effects::infer_effects(&module).diagnostics;
            assert!(
                diags.is_empty(),
                "ontology publishes `/ {name}`, which the checker rejects: {:?}",
                diags.iter().map(|d| &d.message).collect::<Vec<_>>()
            );
        }

        // The other direction: a built-in kind the ontology does not publish is
        // a capability an agent cannot discover.
        for name in [
            "io", "net", "fs", "async", "alloc", "panic", "ffi", "env", "time", "gpu", "npu",
            "llm", "evolve", "learn", "rng", "agent", "proc",
        ] {
            assert!(
                published.contains(&name),
                "`{name}` is a built-in effect kind but the ontology does not publish it"
            );
        }
    }

    /// Every keyword introduces the token the ontology says it does.
    ///
    /// `introduces` was two fields wearing one name — see `KEYWORD_DOCS`.
    /// This joins it back against `lexer::KEYWORDS`, which is the only thing
    /// that makes the field usable to a reader who is not this compiler.
    #[test]
    fn keywords_introduce_the_token_they_claim() {
        let published = keywords_section();
        for entry in published.as_array().unwrap() {
            let spelling = entry["keyword"].as_str().unwrap();
            let claimed = entry["introduces"].as_str().unwrap();
            let (_, actual) = crate::lexer::KEYWORDS
                .iter()
                .find(|(w, _)| *w == spelling)
                .unwrap_or_else(|| panic!("ontology publishes `{spelling}`, not a keyword"));
            assert_eq!(
                claimed,
                format!("{actual:?}"),
                "`{spelling}` is published as introducing {claimed}"
            );
        }
    }

    /// Every documented type can be written in a signature.
    ///
    /// Three could not: `S` (published as a shorthand for `String`, but `S` is
    /// the `struct` keyword and can never be a type), and `Map[K,V]` / `Set[T]`
    /// — the real spellings are `{K: V}` and `{T}`. An agent that believed the
    /// ontology emitted a program that did not compile, which is the failure
    /// mode this whole file exists to prevent.
    ///
    /// Metavariables are substituted because the entries are schemas: `&T`
    /// documents a shape, and only `&i32` can be compiled.
    #[test]
    fn every_documented_type_can_be_written() {
        for (name, _, _) in TYPES {
            let concrete = name
                .replace("(T1,T2,...)", "(i32, i64)")
                .replace("[T; N]", "[i32; 4]")
                .replace("K: V", "i32: i32")
                .replace("T1", "i32")
                .replace("T2", "i32");
            // `R` is overloaded in the ontology's own notation: the `Result`
            // constructor in `R[T,E]`, and the return-type metavariable in
            // `f(T)->R`. Only the second is substituted.
            let concrete: String = concrete
                .replace("->R", "->i32")
                .replace(['T', 'E', 'K', 'V'], "i32");
            let src = format!("f probe(x: {concrete}) -> i32 {{ 0 }}");
            let tokens = crate::lexer::lex(&src);
            let module = crate::parser::parse(&tokens).unwrap_or_else(|e| {
                panic!("documented type `{name}` (as `{concrete}`) does not parse: {e:?}")
            });
            let diags = crate::resolve::resolve(&module).diagnostics;
            let unresolved: Vec<_> = diags
                .iter()
                .filter(|d| d.message.contains("unresolved type"))
                .map(|d| &d.message)
                .collect();
            assert!(
                unresolved.is_empty(),
                "documented type `{name}` (as `{concrete}`) does not resolve: {unresolved:?}"
            );
        }
    }

    /// Every control-flow sigil the ontology publishes actually parses.
    ///
    /// Two did not. `!` is published as Break and the spec agrees — and it
    /// parsed nowhere, because prefix `!` always demanded an operand, so only
    /// the spelling the lexer calls *legacy* (`break`) worked. That is now
    /// implemented.
    ///
    /// `^` was published as Return and was never a thing: the spec names `ret`
    /// in both sigil tables and mentions `^` once, as bitwise XOR. The entry
    /// was the error, not the compiler — worth remembering, because the first
    /// attempt at this fix implemented `^` to make the ontology true, which is
    /// backwards. The ontology describes the language; it does not define it.
    #[test]
    fn every_published_control_sigil_parses() {
        let cases: &[(&str, &str)] = &[
            ("!", "f a() -> i32 {\n m i = 0\n @@ {\n i = i + 1\n ? i > 2 { ! } : { }\n }\n i\n}"),
            ("ret", "f a() -> i32 { ret 5 }"),
            (">>", "f a() -> i32 {\n m i = 0\n @w i < 3 {\n i = i + 1\n ? i > 9 { >> } : { }\n }\n i\n}"),
            ("@@", "f a() -> i32 {\n m i = 0\n @@ {\n i = i + 1\n ? i > 2 { break } : { }\n }\n i\n}"),
            ("@w", "f a() -> i32 {\n m i = 0\n @w i < 2 { i = i + 1 }\n i\n}"),
            ("?", "f a(c: bool) -> i32 { ? c { 1 } : { 2 } }"),
            ("?=", "f a(x: i32) -> i32 { ?= x { _ => 1 } }"),
            ("??", "f a() -> i32 { ?? }"),
            ("???", "f a() -> i32 { ??? }"),
            ("1b", "f a() -> bool { 1b }"),
            ("0b", "f a() -> bool { 0b }"),
        ];
        for (sigil, src) in cases {
            let published = SIGILS.iter().any(|(s, _, _)| s == sigil);
            assert!(published, "`{sigil}` is exercised here but no longer published");
            assert!(
                crate::parser::parse(&crate::lexer::lex(src)).is_ok(),
                "published sigil `{sigil}` does not parse: {src}"
            );
        }
        // The one that was wrong, held in both directions.
        assert!(
            !SIGILS.iter().any(|(s, _, _)| *s == "^"),
            "`^` is bitwise XOR and the `^T` Box prefix — it does not return"
        );
    }

    /// Every layer surface name in `layer_map` can actually be written in a
    /// `net` block.
    ///
    /// `layer_map_resolves_to_real_ops` already checks that each entry points
    /// at a real opcode — but that is the ontology agreeing with the IR, not
    /// with the *parser*. These 21 names are what an agent types when it writes
    /// a network, so the claim that matters is whether `layer x: GELU;`
    /// compiles.
    ///
    /// Scope, stated honestly: `layer_map` is *filtered* by
    /// `layer_name_to_op`, and the unknown-layer check added to
    /// `abl_shape::check_module_shapes` rejects exactly when that function
    /// returns `None` — so the unknown-layer half of this test cannot fail by
    /// construction. What it does still catch is a published name that
    /// resolves to an op but does not survive the *parser* in layer position
    /// (a name that becomes a keyword, say) or the typechecker. The
    /// non-circular half of the story is
    /// `abl_shape::tests::an_unknown_layer_type_is_rejected`.
    #[test]
    fn every_layer_surface_name_is_usable_in_a_net() {
        // Iterate what is *published*, not `LAYER_SURFACE_NAMES`. That list
        // holds 31 names, of which 10 — `Unify`, `Resolve`, `Infer`, `Plan`,
        // `Send`, `Recv`, `Spawn`, `Delegate`, `Hash`, `Typeof` — are symbolic
        // and agent ops rather than neural layers, and `layer_map_section`
        // filters them out because `layer_name_to_op` returns `None`. They are
        // correctly not layers; asserting they compile as one would be
        // asserting the wrong thing.
        let published = layer_map_section();
        let rows = published.as_array().unwrap();
        // A loop over an empty list passes silently, which is the failure mode
        // this whole file is about.
        assert!(rows.len() >= 21, "layer_map shrank to {} — was 21", rows.len());
        for surface in rows.iter().map(|r| r["surface_name"].as_str().unwrap()) {
            // Activations and norms take no constructor arguments; shaped
            // layers do. Try the forms a writer would try.
            let accepted = ["", "(128)", "(128, 64)", "(2)"].iter().any(|args| {
                let src = format!(
                    "net N {{\n    layer a: Linear(8, 128);\n    layer b: {surface}{args};\n    forward {{ b(a) }}\n}}"
                );
                let Ok(module) = crate::parser::parse(&crate::lexer::lex(&src)) else {
                    return false;
                };
                let typed = crate::types::check(&module)
                    .diagnostics
                    .iter()
                    .all(|d| !matches!(d.severity, crate::hir::Severity::Error));
                // `types::check` does not run the net passes — an unknown
                // layer type is caught by `abl_shape::check_module_shapes`,
                // which is what `--check` also runs. Omitting it made an
                // earlier version of this test pass with a deliberately bogus
                // layer name injected into the list it is supposed to police.
                //
                // Only the *unknown layer* diagnostic disqualifies a name. A
                // shape mismatch means the name resolved and the probe's
                // arbitrary dimensions did not line up, which is a different
                // claim and not this test's business.
                let known = !crate::abl_shape::check_module_shapes(&module)
                    .iter()
                    .any(|d| d.message.contains("unknown layer type"));
                typed && known
            });
            assert!(
                accepted,
                "`layer x: {surface}` is published in layer_map but does not compile in a net"
            );
        }
    }

    /// Every `framewerx_modules` path exists.
    ///
    /// 256 entries over 56 files, and the section is the map an agent uses to
    /// find the framework. Paths rot quietly when files move.
    #[test]
    fn every_framewerx_module_path_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root");
        let published = framewerx_modules_section();
        for entry in published.as_array().unwrap() {
            let path = entry["path"].as_str().unwrap();
            assert!(
                root.join(path).exists(),
                "framewerx_modules path is gone: {path}"
            );
        }
    }

    /// Every path the ontology publishes exists.
    ///
    /// Cheap, and the kind of claim that rots silently: a file moves and the
    /// map an agent navigates by keeps pointing at where it used to be.
    #[test]
    fn every_documented_path_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root");
        for (path, _) in PROJECT_LAYOUT.iter().map(|(p, d)| (*p, *d)) {
            assert!(root.join(path).exists(), "project_layout path is gone: {path}");
        }
        for (path, _, _) in DOCS {
            assert!(root.join(path).exists(), "docs path is gone: {path}");
        }
    }

    #[test]
    fn examples_all_parse() {
        for (name, _, src, _) in EXAMPLES {
            let tokens = crate::lexer::lex(src);
            let lex_errors: Vec<_> = tokens
                .iter()
                .filter(|t| t.kind == crate::lexer::TokenKind::Error)
                .collect();
            assert!(
                lex_errors.is_empty(),
                "example {name:?} has lex errors: {lex_errors:?}\nsource: {src}"
            );
            let result = crate::parser::parse(&tokens);
            assert!(
                result.is_ok(),
                "example {name:?} failed to parse: {:?}\nsource: {src}",
                result.err()
            );
        }
    }
}
