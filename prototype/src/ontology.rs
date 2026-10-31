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
//! - **`ir_ops`**     — Every `rmi::lang::Op`: opcode, family, arity, flags
//!   and summary (the `op` facts of `kb/meaning.mg`)
//! - **`op_families`** — The `OpFamily` buckets and their semantics
//!   (`family_row` facts)
//! - **`layer_map`**  — Surface layer name → opcode (the `canonical_layer`
//!   and `lowers_as` facts)
//! - **`types`** and **`effects`** — Built-in types and effect annotations
//!   (`type_row` and `effect_row` facts)
//! - **`rap_methods`** — Protocol method catalog with input / output keys
//! - **`heal_patterns`** — Mechanical heal patterns (from
//!   `heal::pattern_names`)
//! - **`recovery_stages`** — The 4-stage recovery pipeline + agent.refine
//! - **`abl`**       — Binary IR container format (`abl_field` facts; the
//!   magic and version are the constants in `abl`)
//!
//! Single entry point: [`build`] returns a `serde_json::Value`. The RAP
//! method `ontology/full` calls it; `ontology/section` returns just one
//! key from the same object.

/// The `sigils`, `keywords`, `ast_kinds`, `ir_ops`, `op_families`, `layer_map`,
/// `types`, `effects` and `abl` sections, as `kb/` states them. Read once per
/// process: loading the knowledge base checks every invariant.
struct Stated {
    /// `(notation, what it builds, summary)`: the `sigil_row` facts of
    /// `kb/lexicon.mg`, in notation order. The lexicon is where a sigil is
    /// stated; the table that was here is those facts now.
    sigils: Vec<(String, String, String)>,
    /// `(spelling, token kind, summary)`; see `keyword_rows`.
    keywords: Vec<(String, String, String)>,
    /// `(kind, summary)`; see `ast_kind_rows`.
    ast_kinds: Vec<(String, String)>,
    /// `(name, code, summary)`: the `family_row` facts, by code; see
    /// `op_family_rows`.
    op_families: Vec<(String, u8, String)>,
    /// One row per op, by opcode; see `ir_op_rows`.
    ir_ops: Vec<IrOpRow>,
    /// One row per `canonical_layer`, by surface name; see `layer_rows`.
    layers: Vec<LayerRow>,
    /// `(name, category, summary)`: the `type_row` facts, by name.
    types: Vec<(String, String, String)>,
    /// `(name, slot, purpose)`: the `effect_row` facts, by name.
    effects: Vec<(String, String, String)>,
    /// `(position, format line)`: the `abl_field` facts, by position.
    abl_fields: Vec<(String, String)>,
    /// The `abl_media_type` fact.
    abl_media_type: String,
}

/// One op's row of the `ir_ops` section.
struct IrOpRow {
    name: String,
    /// Zero-prefixed four-digit hex, as the section writes it.
    opcode: String,
    /// The high byte of the opcode: the code of the op's family.
    family: u8,
    arity: u64,
    differentiable: bool,
    has_params: bool,
    stateful: bool,
    summary: String,
}

/// One row of the `layer_map` section: a surface spelling, the op it lowers
/// to, and that op's opcode and family.
struct LayerRow {
    surface: String,
    op_name: String,
    opcode: String,
    family: u8,
}

/// A code written `0x…`, as the facts write opcodes and family codes.
fn hex_code(s: &str) -> u64 {
    let digits = s.strip_prefix("0x").unwrap_or_else(|| panic!("`{s}` is not a `0x` code"));
    u64::from_str_radix(digits, 16).unwrap_or_else(|_| panic!("`{s}` is not hex"))
}

fn stated() -> &'static Stated {
    static STATED: std::sync::OnceLock<Stated> = std::sync::OnceLock::new();
    STATED.get_or_init(|| {
        // Compiled in (`kb::FILES`) and held loadable by `kb::tests`, so
        // failing here is a build that broke `kb/`.
        let kb = crate::kb::load().unwrap_or_else(|e| panic!("kb/ does not load: {e:#?}"));
        let summary: std::collections::HashMap<String, String> =
            kb.facts("summary").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
        let word = |s: &str| {
            s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        };
        let mut keywords: Vec<(String, String, String)> = kb
            .facts("lexes_as")
            .into_iter()
            .filter(|f| word(&f[0]) && f[1] != "Ident")
            .map(|f| {
                let d = summary.get(&f[0]).cloned().unwrap_or_else(|| format!("reserved word → {} token", f[1]));
                (f[0].clone(), f[1].clone(), d)
            })
            .collect();
        keywords.sort();

        // The op families. The code is the high byte of each op's opcode.
        let mut op_families: Vec<(String, u8, String)> = kb
            .facts("family_row")
            .into_iter()
            .map(|f| (f[0].clone(), u8::try_from(hex_code(&f[1])).expect("a family code is a byte"), f[2].clone()))
            .collect();
        op_families.sort_by_key(|r| r.1);
        let family_code: std::collections::HashMap<String, u8> =
            op_families.iter().map(|(n, c, _)| (n.clone(), *c)).collect();

        // The ops: each name's facts, by name. A flag is stated only when true.
        let by_op = |predicate: &str| -> std::collections::HashMap<String, String> {
            kb.facts(predicate).into_iter().map(|f| (f[0].clone(), f[1].clone())).collect()
        };
        let flagged = |predicate: &str| -> std::collections::HashSet<String> {
            kb.facts(predicate).into_iter().map(|f| f[0].clone()).collect()
        };
        let need = |m: &std::collections::HashMap<String, String>, op: &str, what: &str| -> String {
            m.get(op).cloned().unwrap_or_else(|| panic!("kb/meaning.mg states no {what} for `{op}`"))
        };
        let (opcode_of, family_of, arity_of, summary_of) =
            (by_op("opcode"), by_op("op_family"), by_op("op_arity"), by_op("op_summary"));
        let (differentiable, has_params, stateful) =
            (flagged("differentiable"), flagged("has_params"), flagged("stateful"));
        let mut ir_ops: Vec<(u64, IrOpRow)> = kb
            .facts("instance_of")
            .into_iter()
            .filter(|f| f[1] == "Op")
            .map(|f| {
                let name = f[0].clone();
                let opcode = need(&opcode_of, &name, "opcode");
                let fam = need(&family_of, &name, "family");
                let family = *family_code
                    .get(&fam)
                    .unwrap_or_else(|| panic!("`{name}` is in family `{fam}`, which has no family_row"));
                let row = IrOpRow {
                    arity: need(&arity_of, &name, "arity").parse().expect("an arity is a number"),
                    differentiable: differentiable.contains(&name),
                    has_params: has_params.contains(&name),
                    stateful: stateful.contains(&name),
                    summary: need(&summary_of, &name, "summary"),
                    opcode: opcode.clone(),
                    family,
                    name,
                };
                (hex_code(&opcode), row)
            })
            .collect();
        ir_ops.sort_by_key(|(code, _)| *code);
        let ir_ops: Vec<IrOpRow> = ir_ops.into_iter().map(|(_, row)| row).collect();

        // The layer spellings `layer_map` publishes, each with the op it lowers to.
        let lowers: std::collections::HashMap<String, String> =
            kb.facts("lowers_as").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
        let mut layers: Vec<LayerRow> = kb
            .facts("canonical_layer")
            .into_iter()
            .map(|f| {
                let surface = f[0].clone();
                let op_name = lowers
                    .get(&surface)
                    .cloned()
                    .unwrap_or_else(|| panic!("canonical layer `{surface}` lowers as no op"));
                let op = ir_ops
                    .iter()
                    .find(|o| o.name == op_name)
                    .unwrap_or_else(|| panic!("`{surface}` lowers as `{op_name}`, which is no op"));
                LayerRow { surface, opcode: op.opcode.clone(), family: op.family, op_name }
            })
            .collect();
        layers.sort_by(|a, b| a.surface.cmp(&b.surface));

        let mut types: Vec<(String, String, String)> =
            kb.facts("type_row").into_iter().map(|f| (f[0].clone(), f[1].clone(), f[2].clone())).collect();
        types.sort();
        let mut effects: Vec<(String, String, String)> =
            kb.facts("effect_row").into_iter().map(|f| (f[0].clone(), f[1].clone(), f[2].clone())).collect();
        effects.sort();

        let mut abl_fields: Vec<(String, String)> =
            kb.facts("abl_field").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
        abl_fields.sort();
        let abl_media_type = kb
            .facts("abl_media_type")
            .into_iter()
            .next()
            .map(|f| f[0].clone())
            .expect("kb/meaning.mg states the container's media type");

        Stated {
            sigils: kb.facts("sigil_row").into_iter().map(|f| (f[0].clone(), f[1].clone(), f[2].clone())).collect(),
            keywords,
            ast_kinds: kb
                .facts("instance_of")
                .into_iter()
                .filter(|f| f[1] == "AstKind")
                .map(|f| (f[0].clone(), summary.get(&f[0]).cloned().unwrap_or_default()))
                .collect(),
            op_families,
            ir_ops,
            layers,
            types,
            effects,
            abl_fields,
            abl_media_type,
        }
    })
}

fn sigil_rows() -> &'static [(String, String, String)] {
    &stated().sigils
}

/// The `op_families` section's rows, `(name, code, summary)`, by code: the
/// `family_row` facts of `kb/meaning.mg`. `every_op_family_is_stated` holds
/// them to the ops.
fn op_family_rows() -> &'static [(String, u8, String)] {
    &stated().op_families
}

/// The `ir_ops` section's rows, by opcode: every op, from the `opcode`,
/// `op_family`, `op_arity`, `op_summary` and flag facts of `kb/meaning.mg`.
/// `every_op_has_a_fact_and_every_fact_an_op` holds them to `Op`.
fn ir_op_rows() -> &'static [IrOpRow] {
    &stated().ir_ops
}

/// The `layer_map` section's rows, by surface name: each `canonical_layer`,
/// with the op its `lowers_as` fact names.
fn layer_rows() -> &'static [LayerRow] {
    &stated().layers
}

/// The `types` section's rows, by name: the `type_row` facts.
fn type_rows() -> &'static [(String, String, String)] {
    &stated().types
}

/// The `effects` section's rows, by name: the `effect_row` facts.
fn effect_rows() -> &'static [(String, String, String)] {
    &stated().effects
}

/// The `keywords` section's rows, `(spelling, token kind, summary)`: every word the
/// lexer reserves, read from `kb/lexicon.mg` as the `lexes_as` facts whose
/// spelling is a word and whose kind is not `Ident` (`cons` and `topology`
/// are words that lex as identifiers). `lexer::KEYWORDS` is the same set,
/// and `the_keywords_section_is_the_lexer_table` holds it to that. Sorted by
/// spelling.
///
/// `introduces` is always the token the spelling produces. It once took a
/// hand-written name instead for the 19 keywords with curated prose (`agent`
/// claimed `AgentDef`, `match` claimed `Match`; it is `QuestionEq`), so the
/// field meant two things and could be joined against nothing.
/// `keywords_introduce_the_token_they_claim` holds it. The prose is
/// `summary` facts; a keyword without one gets a generated summary.
fn keyword_rows() -> &'static [(String, String, String)] {
    &stated().keywords
}

/// AST top-level item kinds an agent should know it can find in a module,
/// `(kind, summary)`: the `AstKind` instances of `kb/grammar.mg`, each built
/// by the item production its `parses_as` fact names. Named exactly as
/// `ast::ItemKind` names them.
///
/// The published names used to be a parallel vocabulary — `Mod` for `Module`,
/// `EffectDef` for `Effect`, `SpecBlock` for `Spec` — which made the list
/// impossible to check against the AST without a mapping that could itself
/// rot. Worse, **`Data` and `Extend` were missing entirely**: an agent
/// enumerating item kinds from the ontology would not learn that `data
/// Point(…)` (records and sums) or `extend Type { … }` (methods) are item
/// kinds at all, and those are the two the human-mode guides lead with.
///
/// `every_item_kind_is_published_and_vice_versa` compares these against
/// `ItemKind`'s variants by scraping `ast.rs`, in both directions.
fn ast_kind_rows() -> &'static [(String, String)] {
    &stated().ast_kinds
}

/// Catalog of RAP methods. Pairs each method name with a short summary
/// and the input / output JSON keys an agent should expect. The truth
/// for behavior is the `dispatch` match in `rap.rs`; this is the
/// agent-discoverable summary.
const RAP_METHODS: &[(&str, &str, &[&str], &[&str])] = &[
    ("language/parse", "Parse source to AST (JSON).",
        &["source"], &["ok", "ast", "error"]),
    ("language/tokens", "Tokenize source.",
        &["source"], &["ok", "lex_errors", "tokens"]),
    ("build/check", "Lex + parse + report diagnostics.",
        &["source"], &["ok", "errors"]),
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
        &["source"], &["ok", "recover_stage", "recovered_source", "candidates_tried", "changed", "abl_hex", "container_bytes", "items"]),
    ("cost/query", "Per-construct cost estimate.",
        &["construct", "target", "opt"], &["ok", "estimate", "error"]),
    ("cost/compare", "Compare costs of two constructs.",
        &["a", "b", "target"], &["ok", "comparison", "error"]),
    ("skb/query", "Query structured knowledge base.",
        &["by", "value"], &["ok", "count", "query", "matches"]),
    ("skb/spec", "Lookup spec block for a symbol.",
        &["fqn"], &["ok", "spec", "error"]),
    ("skb/rules", "List SKB rules.",
        &["domain"], &["ok", "matches"]),
    ("verify/contracts", "Verify function contracts (req/ens/inv).",
        &["fqn", "requires", "ensures", "declared_effects", "used_effects"], &["ok", "verdict", "detail", "result"]),
    ("verify/module", "Verify entire module.",
        &["source"], &["ok", "verdict", "detail", "results"]),
    ("format/agent", "Format source in agent-canonical sigil mode.",
        &["source"], &["ok", "formatted", "ast", "error"]),
    ("format/human", "Format source in human-readable keyword mode.",
        &["source"], &["ok", "formatted", "ast", "error"]),
    ("lint/check", "Run lints on source.",
        &["source"], &["ok", "verify", "effect_diagnostics", "error"]),
    ("token/report", "Per-construct token cost report for source.",
        &["source"], &["ok", "report"]),
    // The method that lists the methods, missing from the list of methods.
    //
    // `rap.rs` serves it and `METHODS` advertises it; this table — a fourth,
    // hand-maintained copy of the same list — omitted it, so an agent doing
    // discovery through the ontology was never told that discovery has an
    // endpoint. Found 2026-09-02 by comparing all four statements of the list
    // after `check-rap-methods.sh` compared only three.
    ("rap/methods", "List every method this server serves, with its count.",
        &[], &["ok", "count", "methods"]),
    ("kb/schema", "The knowledge base's schema: classes, relations, invariants.",
        &[], &["ok", "schema_version", "classes", "relations", "invariants", "error"]),
    ("kb/query", "Facts of a predicate in the knowledge base, stated or derived; \"?\" matches any.",
        &["predicate", "args"], &["ok", "count", "matches", "error"]),
    ("effects/infer", "Infer effects of each function.",
        &["source"], &["ok", "effects", "diagnostics"]),
    ("effects/check", "Check declared effects against inferred.",
        &["source"], &["ok", "diagnostics", "error"]),
    ("elision/apply", "Apply elision rules to compact source.",
        &["source"], &["ok", "ast", "error"]),
    ("attribute/expand", "Expand attribute shorthand.",
        &["name"], &["ok", "expanded", "error"]),
    ("attribute/compress", "Compress attributes back to shorthand.",
        &["name"], &["ok", "compressed", "error"]),
    ("capability/check", "List capabilities required by source.",
        &["source"], &["ok", "verdict", "detail", "agents_checked", "results"]),
    ("heal/graph", "Heal-pipeline diagnostic graph.",
        &["source"], &["ok", "graphs"]),
    ("sandbox/policy", "Lookup sandbox policy by name.",
        &["source", "agent"], &["ok", "agent", "capabilities", "requires_approval", "error"]),
    ("doc/query", "Lookup documentation by FQN.",
        &["fqn"], &["ok", "matches"]),
    ("grammar/list", "List grammar extensions.",
        &[], &["ok", "extensions"]),
    ("manifest/generate", "Generate capability manifest for a module.",
        &["source", "crate_name", "version"], &["ok", "manifest"]),
    ("nl/generate", "Generate MAGE from a natural-language prompt.",
        &["prompt"], &["ok", "code_human", "code_agent", "explanation", "diagnostics", "fixes", "verification"]),
    ("nl/explain", "Explain source in natural language.",
        &["source"], &["ok", "explanation"]),
    ("nl/refactor", "Refactor source via natural-language request.",
        &["source"], &["ok", "code_human", "code_agent", "explanation"]),
    ("nl/query", "General NL query against the SKB.",
        &["prompt"], &["ok", "explanation", "kb_results"]),
    ("ontology/full", "Return this complete ontology.",
        &[], &["ok", "schema_version", "version", "project", "short_name", "organization", "builtin_framework", "counts", "sections"]),
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
    ("--emit-skb", "Regenerate skb/ from the rules compiled into this binary", true),
    ("--check", "Lex + parse + resolve; report diagnostics", true),
    ("--differentiable", "Report differentiability for every function, net and train block (smooth / almost everywhere / unknown / not, with reasons)", true),
    ("--gradcheck", "Check computed derivatives against central differences; reports deductive and inductive verdicts separately", true),
    ("--fmt-compact", "Reformat source in agent-canonical sigil mode", true),
    ("--fmt-expand", "Reformat source in human-readable keyword mode", true),
    ("--count-tokens", "Print the MAGE lexer token count of a file (the arena's description-length unit)", true),
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
    ("--solve", "Minimise (or with --max, maximise) a function over the box given by its @param consts; prints the Solution JSON naming the solver (ProjectedGradient when certified convex or concave, else Es) and why. Takes [--fuel N] [--seed S].", true),
    ("--max", "With `--solve`: maximise the objective instead of minimising it.", false),
    ("--fuel", "With `--solve`: the fuel budget for the search (default 1000000). Takes a whole number after it.", false),
    ("--seed", "With `--solve`: the seed for the evolution strategy (default 0). Takes a whole number after it.", false),
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
    ("--core", "With `--check`: also require MAGE-core (MAGE_SPEC.md §4.12), the subset the RSI kernel runs.", false),
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

/// The reliability floors, and where each is enforced.
///
/// This section used to say "Read from `.github/workflows/ci.yml`". **That
/// file contained none of them** — no reliability-bench job, no heal
/// threshold, no token-ratio gate — so an agent reading the ontology believed
/// six regressions were gated and none were. `UNIFICATION.md` described a CI
/// step parsing `benchmarks/TOKEN_REPORT.md`; there was no such step either.
///
/// Two of the six were not true on a default run: the file-oracle
/// structural-heal contribution is **1**, not `>= 2`, and the stage-3 refine
/// smoke is 0 unless a wrapper command is supplied. Those two are now stated
/// as observations rather than floors.
///
/// The three that are real live in `scripts/check-ci-floors.sh`, which
/// measures and enforces them; CI runs it. Columns:
/// `(name, threshold, what_it_protects)`.
const CI_FLOORS: &[(&str, &str, &str)] = &[
    ("MIN_PARSE", ">= 98",
        "File-oracle parse rate (100-task corpus) must not regress. Enforced by scripts/check-ci-floors.sh"),
    ("MIN_HEAL", ">= 40",
        "Perturbed-oracle pattern-heal recovery count must not regress. Enforced by scripts/check-ci-floors.sh"),
    ("native-lexer ratio", "<= 1.100",
        "MAGE text size must stay within 10% of equivalent Rust, read from the native-lexer Total row of benchmarks/TOKEN_REPORT.md. Enforced by scripts/check-ci-floors.sh"),
    ("file-oracle structural-heal", "= 1 (observed)",
        "Trim-bad-token recovers the single corpus task that does not parse cleanly. An observation, not a floor: the count is 1, and the >= 2 this section used to publish was never met"),
    ("refine smoke", "0 without a wrapper (observed)",
        "Stage-3 refine contributes only when `--agent perturbed+refine:<cmd>` supplies a wrapper; a default run reports 0"),
    ("subprocess echo smoke", "no-op",
        "Subprocess agent backend must be invocable; exercised by the wrapper protocol tests"),
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
            "sigils": sigil_rows().len(),
            "keywords": keyword_rows().len(),
            "types": type_rows().len(),
            "ast_kinds": ast_kind_rows().len(),
            "ir_ops": ir_op_rows().len(),
            "op_families": op_family_rows().len(),
            "rap_methods": RAP_METHODS.len(),
            "recovery_stages": RECOVERY_STAGES.len(),
            "examples": EXAMPLES.len(),
            "framewerx_modules": FRAMEWERX_MODULES.len(),
            "cli_flags": CLI_FLAGS.len(),
            "bench_backends": BENCH_BACKENDS.len(),
            "effects": effect_rows().len(),
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
    let items: Vec<_> = sigil_rows()
        .iter()
        .map(|(s, k, d)| serde_json::json!({ "sigil": s, "ast_kind": k, "summary": d }))
        .collect();
    serde_json::json!(items)
}

fn keywords_section() -> serde_json::Value {
    let items: Vec<_> = keyword_rows()
        .iter()
        .map(|(w, k, d)| serde_json::json!({ "keyword": w, "introduces": k, "summary": d }))
        .collect();
    serde_json::json!(items)
}

fn types_section() -> serde_json::Value {
    let items: Vec<_> = type_rows()
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
    let items: Vec<_> = ast_kind_rows()
        .iter()
        .map(|(k, d)| serde_json::json!({ "kind": k, "summary": d }))
        .collect();
    serde_json::json!(items)
}

fn ir_ops_section() -> serde_json::Value {
    let items: Vec<_> = ir_op_rows()
        .iter()
        .map(|o| {
            serde_json::json!({
                "name": o.name,
                "opcode": o.opcode,
                "family": o.family,
                "arity": o.arity,
                "differentiable": o.differentiable,
                "has_params": o.has_params,
                "stateful": o.stateful,
                "summary": o.summary,
            })
        })
        .collect();
    serde_json::json!(items)
}

fn op_families_section() -> serde_json::Value {
    let items: Vec<_> = op_family_rows()
        .iter()
        .map(|(name, code, desc)| {
            serde_json::json!({ "name": name, "code": code, "summary": desc })
        })
        .collect();
    serde_json::json!(items)
}

fn layer_map_section() -> serde_json::Value {
    let items: Vec<_> = layer_rows()
        .iter()
        .map(|l| {
            serde_json::json!({
                "surface_name": l.surface,
                "opcode": l.opcode,
                "op_name": l.op_name,
                "family": l.family,
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
    // With the example, not just the name: `parse-stray-comma-in-name-position`
    // tells an agent that a pattern exists and nothing about when it fires.
    let items: Vec<_> = crate::heal::patterns_with_examples()
        .iter()
        .map(|(name, example)| serde_json::json!({ "name": name, "example": example }))
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
    let items: Vec<_> = effect_rows()
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
    let s = stated();
    serde_json::json!({
        "magic": std::str::from_utf8(crate::abl::ABL_MAGIC).unwrap_or("ABL1"),
        "version": crate::abl::ABL_VERSION,
        "format": s.abl_fields.iter().map(|(_, line)| line.as_str()).collect::<Vec<&str>>(),
        "media_type": s.abl_media_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmi::lang::Op;

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

    /// The quick reference and the ontology must name the same sigils.
    ///
    /// `agent-guide/syntax-quick-ref.md` is what an agent reads to *learn* the
    /// compressed surface; `MAGE_ONTOLOGY.json` is what it reads to *discover*
    /// the language mechanically. They are two renderings of one fact and
    /// nothing compared them, so they disagreed about what MAGE contains:
    /// `D`, `xd`, `fx`, `sp`, `sw`, `af`, `gd`, `df` and `yl` all parse, the
    /// guide taught all nine, and the ontology published none of them. Four of
    /// those are whole features — records, inherent methods, effect
    /// declarations and contracts — invisible to anything reading the JSON.
    ///
    /// The guide is a fixed-width text table inside code fences, so the sigil
    /// is the last whitespace-separated column. Rows whose last column is
    /// prose (a parenthesised note, an arrow) are skipped by requiring the
    /// token to be short and free of spaces.
    #[test]
    fn the_quick_reference_and_the_ontology_name_the_same_sigils() {
        let guide = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../agent-guide/syntax-quick-ref.md"),
        )
        .expect("the quick reference is beside the prototype")
        .replace("\r\n", "\n");

        let mut in_fence = false;
        let mut fence_no = 0usize;
        let mut taught: Vec<String> = Vec::new();
        // The first fence is the "Declarations" table, where every right-hand
        // entry is a declaration sigil and nothing else. Collected separately
        // because the union check below is weak for these: `xd`, `af` and `D`
        // are also lexer keywords, so removing one from `SIGILS` still passes
        // the general test. The strict list is what makes that a failure.
        let mut declarations: Vec<String> = Vec::new();
        for line in guide.lines() {
            if line.starts_with("```") {
                in_fence = !in_fence;
                if in_fence {
                    fence_no += 1;
                }
                continue;
            }
            if !in_fence || line.trim().is_empty() {
                continue;
            }
            // Three or more columns separated by runs of spaces; the sigil is
            // the last. A two-column row is a form with no agent spelling.
            let cols: Vec<&str> = line.trim().split("  ").filter(|c| !c.trim().is_empty()).collect();
            if cols.len() < 3 {
                continue;
            }
            let last = cols[cols.len() - 1].trim();
            // `S / +S` teaches two; `?= e { }` is a form, not a sigil.
            for tok in last.split('/') {
                let tok = tok.trim();
                if tok.is_empty() || tok.len() > 6 || tok.contains(' ') || tok.starts_with('(') {
                    continue;
                }
                if !taught.iter().any(|t| t == tok) {
                    taught.push(tok.to_string());
                }
                if fence_no == 1 && !declarations.iter().any(|t| t == tok) {
                    declarations.push(tok.to_string());
                }
            }
        }
        assert!(taught.len() > 20, "the scrape found only {} sigils — it is broken", taught.len());

        // The guide's right-hand column is not always a sigil: its effect
        // table puts effect *names* there and its type table puts type names.
        // Those are published in other sections, and an agent that finds `ffi`
        // under `effects` has discovered it. So the claim is the general one —
        // nothing the guide teaches is invisible to a reader of the JSON —
        // checked against every section that names a spelling.
        let rows = sigil_rows();
        let mut published: Vec<String> = rows.iter().map(|(s, _, _)| s.clone()).collect();
        published.extend(effect_rows().iter().map(|(n, _, _)| n.clone()));
        published.extend(type_rows().iter().map(|(n, _, _)| n.clone()));
        published.extend(crate::lexer::KEYWORDS.iter().map(|(n, _)| (*n).to_string()));
        published.extend(crate::resolve::VOCABULARY.iter().map(|(n, _, _)| (*n).to_string()));
        published.extend(
            crate::hir::CAPABILITY_NAMESPACES.iter().map(|(n, _)| (*n).to_string()),
        );

        for t in &taught {
            assert!(
                published.iter().any(|p| p == t),
                "`{t}` is taught in syntax-quick-ref.md and appears in no ontology                  section — an agent reading the JSON cannot discover it"
            );
        }

        let sigils: Vec<&str> = rows.iter().map(|(s, _, _)| s.as_str()).collect();
        assert!(
            declarations.len() >= 10,
            "the Declarations fence yielded {} sigils — the scrape is broken",
            declarations.len()
        );
        for d in &declarations {
            assert!(
                sigils.contains(&d.as_str()),
                "`{d}` is a declaration sigil in syntax-quick-ref.md and is not in                  the ontology's `sigils` section"
            );
        }
    }
    /// Every `ItemKind` variant is published, and every published kind is a
    /// variant.
    ///
    /// `Data` and `Extend` were missing — records, sums and methods, three of
    /// the constructs the human-mode documentation leads with. Nothing
    /// compared the two lists, and the published names were a parallel
    /// vocabulary (`Mod`, `EffectDef`, `SpecBlock`) that made comparing them
    /// awkward enough that nobody had.
    #[test]
    fn every_item_kind_is_published_and_vice_versa() {
        let ast_rs = include_str!("ast.rs");
        let start = ast_rs.find("pub enum ItemKind {").expect("ItemKind");
        let body = &ast_rs[start..];
        let end = body.find("\n}").expect("end of enum");
        let variants: Vec<&str> = body[..end]
            .lines()
            .skip(1)
            .filter_map(|l| {
                let t = l.trim();
                if t.is_empty() || t.starts_with("//") || t.starts_with("/*") {
                    return None;
                }
                let name = t.split(['(', ',', ' ']).next()?;
                name.chars().next().filter(|c| c.is_ascii_uppercase()).map(|_| name)
            })
            .collect();
        assert!(variants.len() >= 18, "scrape looks wrong: {variants:?}");

        let kinds = ast_kind_rows();
        let published: Vec<&str> = kinds.iter().map(|(k, _)| k.as_str()).collect();
        for v in &variants {
            assert!(
                published.contains(v),
                "`ItemKind::{v}` is not published in ast_kinds"
            );
        }
        for k in &published {
            assert!(
                variants.contains(k),
                "ast_kinds publishes `{k}`, which is not an ItemKind variant"
            );
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
            // A literal ending in `=` is a `starts_with` prefix, not a flag —
            // `--target=` and friends appear that way in argument handling and
            // in tests. The two that *are* flags normalise to their published
            // `<value>` spelling below.
            if lit.ends_with('=')
                && !matches!(*lit, ref l if l == "--backend=" || l == "--backends-file=")
            {
                continue;
            }
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
        assert_eq!(counts["sigils"].as_u64().unwrap(), sigil_rows().len() as u64);
        assert_eq!(counts["keywords"].as_u64().unwrap(), crate::lexer::KEYWORDS.len() as u64);
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

    /// The published pattern list is exactly the registry's.
    ///
    /// This asserted `len() >= 10` over a list of 34 — true of any ten
    /// strings, and silent about every question worth asking. Whether each
    /// pattern *works* is `heal::every_pattern_matches_its_example_and_
    /// produces_a_fix`, which is where the matchers are; what belongs here is
    /// the boundary this file owns: that what the ontology publishes is what
    /// the registry contains, in both directions and in order.
    #[test]
    fn published_heal_patterns_are_the_registry() {
        let arr = heal_patterns_section();
        let published: Vec<&str> =
            arr.as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
        let registry = crate::heal::pattern_names();
        assert_eq!(
            published, registry,
            "the ontology's heal_patterns and heal.rs's registry disagree"
        );
        assert!(
            published.len() >= 30,
            "the registry shrank to {} patterns — if that is deliberate, the \
             count in benchmarks/STATUS.md moves with it",
            published.len()
        );
    }

    /// The container description names the bytes a decoder must look for.
    ///
    /// `"magic"` is read from `abl::ABL_MAGIC` and was right. Everything
    /// around it was prose, and a global rename of `ABL` to the language's
    /// full name rewrote all of it: the format line told an agent the four
    /// magic bytes were the 23-character string "Agentic Binary Language",
    /// `--manifest` said the same, and the decoder's own error message told a
    /// caller whose file failed to open that it had expected that string. The
    /// section passed every existing check because the one field derived from
    /// the constant was untouched.
    ///
    /// So the description is checked against the constant, not eyeballed: the
    /// format text must contain the real magic, and must not contain the
    /// language's name where a literal belongs.
    #[test]
    fn the_abl_description_names_the_real_magic_and_version() {
        let s = abl_section();
        let magic = std::str::from_utf8(crate::abl::ABL_MAGIC).expect("magic is utf-8");
        assert_eq!(s["magic"].as_str(), Some(magic));
        assert_eq!(s["version"].as_u64(), Some(u64::from(crate::abl::ABL_VERSION)));

        let format = s["format"].as_array().expect("format").iter().map(|l| l.as_str().unwrap()).collect::<Vec<_>>().join("\n");
        assert!(
            format.contains(magic),
            "the format description never names the magic bytes `{magic}`: {format}"
        );
        assert!(
            !format.contains("Agentic Binary Language"),
            "the format description puts the language's name where the four \
             magic bytes belong: {format}"
        );

        // And the same rename hit `--manifest`, which is the other place an
        // agent reads the container layout.
        let entry = crate::cli_manifest::MODES
            .iter()
            .find(|m| m.flag == "--target=abl-bytes")
            .expect("--target=abl-bytes is a mode");
        assert!(
            entry.detail.contains(magic) && !entry.detail.contains("magic \"Agentic"),
            "`--target=abl-bytes` describes the magic as something other than \
             `{magic}`: {}",
            entry.detail
        );
    }

    #[test]
    fn recovery_stages_in_order() {
        let arr = recovery_stages_section();
        let v = arr.as_array().unwrap();
        assert_eq!(v[0]["stage"], "already-valid");
        assert_eq!(v[1]["stage"], "pattern-heal");
        assert_eq!(v.last().unwrap()["stage"], "failed");

        // Every name `RecoveryStage` can report must be published, and every
        // published name must be one it can report — except `agent.refine`,
        // which belongs to the reliability bench rather than to `recover.rs`
        // and is the one deliberate difference between the two lists. Naming
        // it here is what keeps it deliberate: without this, a stage added to
        // the enum would simply be missing from the ontology, and nothing
        // would say so.
        use crate::recover::RecoveryStage as R;
        let from_enum = [
            R::AlreadyValid,
            R::PatternHeal,
            R::StructuralBalance,
            R::StructuralCompletion,
            R::TrimBadToken,
            R::Failed,
        ];
        let published: Vec<&str> = v.iter().map(|e| e["stage"].as_str().unwrap()).collect();
        for st in from_enum {
            assert!(
                published.contains(&st.as_str()),
                "`recover.rs` can report `{}` and the ontology does not publish it",
                st.as_str()
            );
        }
        for name in &published {
            assert!(
                *name == "agent.refine" || from_enum.iter().any(|s| s.as_str() == *name),
                "the ontology publishes recovery stage `{name}`, which `recover.rs`                  never reports and which is not the known bench-only stage"
            );
        }
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

        // And every published type must be one the checker resolves. The
        // seven names above are a spot-check; this is the claim. `S` was once
        // published as shorthand for `String` — it is the `struct` keyword and
        // can never be a type — and nothing here would have caught it, because
        // `S` was not one of the seven.
        //
        // The constructors are published with metavariables, so each is
        // instantiated with a concrete element type. A published name with no
        // instantiation here is a failure, not a skip.
        let concrete = |n: &str| -> String {
            match n {
                "&T" => "&i32".into(),
                "&!T" => "&!i32".into(),
                "@T" => "@i32".into(),
                "?T" => "?i32".into(),
                "R[T,E]" => "R[i32, str]".into(),
                "[T]" => "[i32]".into(),
                "[T; N]" => "[i32; 4]".into(),
                "[T]~" => "[i32]~".into(),
                "(T1,T2,...)" => "(i32, str)".into(),
                "{K: V}" => "{str: i32}".into(),
                "{T}" => "{i32}".into(),
                "Box[T]" => "Box[i32]".into(),
                "f(T)->R" => "f(i32)->i32".into(),
                other => other.into(),
            }
        };
        for name in &names {
            let src = format!("f m(x: {}) -> i32 {{ 1 }}", concrete(name));
            let module = crate::parser::parse(&crate::lexer::lex(&src))
                .unwrap_or_else(|e| panic!("`{name}` does not even parse: {e:?}"));
            let diags = crate::types::check(&module).diagnostics;
            assert!(
                diags.is_empty(),
                "the ontology publishes type `{name}`, which the checker rejects \
                 as `{}`: {:?}",
                concrete(name),
                diags.iter().map(|d| &d.message).collect::<Vec<_>>()
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
        let published: Vec<&str> = effect_rows()
            .iter()
            .filter(|(_, slot, _)| slot.as_str() == "effect_name")
            .map(|(name, _, _)| name.as_str())
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
        for (name, _) in crate::hir::BUILTIN_EFFECTS {
            assert!(
                published.contains(&name),
                "`{name}` is a built-in effect kind but the ontology does not publish it"
            );
        }
    }

    /// The `abl` section's format is the `abl_field` facts, and they are
    /// numbered: the positions run `01`, `02`, … with no gap, and the section
    /// has one format line for each.
    #[test]
    fn the_abl_section_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let mut positions: Vec<String> = kb.facts("abl_field").into_iter().map(|f| f[0].clone()).collect();
        positions.sort();
        for (i, p) in positions.iter().enumerate() {
            assert_eq!(p.as_str(), format!("{:02}", i + 1), "the abl_field positions are not 01, 02, … in order");
        }
        let lines = abl_section()["format"].as_array().expect("format").len();
        assert_eq!(
            lines,
            positions.len(),
            "the abl section has {lines} format lines for {} abl_field facts",
            positions.len()
        );
    }

    /// Every keyword introduces the token the ontology says it does.
    ///
    /// `introduces` was two fields wearing one name — see `keyword_rows`.
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

    /// The `keywords` section, read from `kb/lexicon.mg`, is exactly
    /// `lexer::KEYWORDS`: the same spellings, each with the same kind.
    #[test]
    fn the_keywords_section_is_the_lexer_table() {
        let published: std::collections::BTreeSet<(String, String)> =
            keyword_rows().iter().map(|(w, k, _)| (w.clone(), k.clone())).collect();
        let lexer: std::collections::BTreeSet<(String, String)> = crate::lexer::KEYWORDS
            .iter()
            .map(|(w, k)| (w.to_string(), format!("{k:?}")))
            .collect();
        assert_eq!(published, lexer);
    }

    /// The problems with a stated set against the code's, both ways: an entry
    /// the code has and the knowledge base does not state, and one it states
    /// that the code does not have. Empty when they agree.
    fn both_ways<T: Ord + std::fmt::Debug>(
        what: &str,
        code: &std::collections::BTreeSet<T>,
        stated: &std::collections::BTreeSet<T>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        for x in code.difference(stated) {
            out.push(format!("{what}: the code has {x:?}, and kb/meaning.mg does not state it"));
        }
        for x in stated.difference(code) {
            out.push(format!("{what}: kb/meaning.mg states {x:?}, and the code does not have it"));
        }
        out
    }

    /// The names `kb/meaning.mg` states as instances of `class`.
    fn instances_of(kb: &crate::kb::Kb, class: &str) -> std::collections::BTreeSet<String> {
        kb.facts("instance_of").into_iter().filter(|f| f[1] == class).map(|f| f[0].clone()).collect()
    }

    /// Every op of `Op::ALL` has a fact of every field, and every fact is an op.
    ///
    /// `kb/meaning.mg` states each op as seven facts: its `Op` class, its
    /// opcode, its family, its arity, its summary, and the three flags, which
    /// are stated only when true. The code side is `Op::meta()` and
    /// `Op::family()`, and the comparison is over sets, both ways, field by
    /// field. An op with no fact, a fact for a name that is not an op, and a
    /// flag set on one side only are each named. The opcode is held to
    /// `format!("0x{:04x}", op.0)`, the spelling `ir_ops_section` writes. The
    /// family is the name of the `family_row` whose code is `op.family()`,
    /// because `ir_ops_section` writes only that code and the JSON names the
    /// family through `op_families`.
    #[test]
    fn every_op_has_a_fact_and_every_fact_an_op() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let pairs = |p: &str| -> std::collections::BTreeSet<(String, String)> {
            kb.facts(p).into_iter().map(|f| (f[0].clone(), f[1].clone())).collect()
        };
        let flag = |p: &str| -> std::collections::BTreeSet<String> {
            kb.facts(p).into_iter().map(|f| f[0].clone()).collect()
        };
        let family_rows = kb.facts("family_row");
        let (mut class, mut opcode, mut family, mut arity, mut summary) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let (mut differentiable, mut has_params, mut stateful) = (Vec::new(), Vec::new(), Vec::new());
        for &op in Op::ALL {
            let meta = op.meta();
            let name = meta.name.to_string();
            class.push(name.clone());
            opcode.push((name.clone(), format!("0x{:04x}", op.0)));
            let fam = family_rows
                .iter()
                .find(|f| f[1] == format!("0x{:02x}", op.family()))
                .map(|f| f[0].clone())
                .unwrap_or_else(|| format!("<no family with code {:#04x}>", op.family()));
            family.push((name.clone(), fam));
            arity.push((name.clone(), meta.arity.to_string()));
            summary.push((name.clone(), meta.desc.to_string()));
            if meta.differentiable {
                differentiable.push(name.clone());
            }
            if meta.has_params {
                has_params.push(name.clone());
            }
            if meta.stateful {
                stateful.push(name);
            }
        }
        let mut problems = Vec::new();
        problems.extend(both_ways("instance_of(_, Op)", &class.into_iter().collect(), &instances_of(&kb, "Op")));
        problems.extend(both_ways("opcode", &opcode.into_iter().collect(), &pairs("opcode")));
        problems.extend(both_ways("op_family", &family.into_iter().collect(), &pairs("op_family")));
        problems.extend(both_ways("op_arity", &arity.into_iter().collect(), &pairs("op_arity")));
        problems.extend(both_ways("op_summary", &summary.into_iter().collect(), &pairs("op_summary")));
        problems.extend(both_ways("differentiable", &differentiable.into_iter().collect(), &flag("differentiable")));
        problems.extend(both_ways("has_params", &has_params.into_iter().collect(), &flag("has_params")));
        problems.extend(both_ways("stateful", &stateful.into_iter().collect(), &flag("stateful")));
        assert!(problems.is_empty(), "kb/meaning.mg and Op disagree:\n{}", problems.join("\n"));
    }

    /// Every op family is stated, and every stated family is one, and the rows
    /// agree with the ops.
    ///
    /// The `op_families` section is the `family_row` facts of `kb/meaning.mg`:
    /// each family's name, its code as two-digit hex, and its summary. Nothing
    /// on the code side is a table to compare with, so the rows are held to the
    /// ops. Each family's code is the high byte of some op, and each op's code
    /// has a row. Each op's `op_family` fact names a row whose code is
    /// `op.family()`, and whose name is the `OpFamily` that `Op::family_enum`
    /// decodes that code to. The `OpFamily` instances are compared with the
    /// family names, both ways.
    #[test]
    fn every_op_family_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let rows = kb.facts("family_row");
        let row_codes: std::collections::BTreeSet<String> = rows.iter().map(|f| f[1].clone()).collect();
        let op_codes: std::collections::BTreeSet<String> =
            Op::ALL.iter().map(|op| format!("0x{:02x}", op.family())).collect();
        let mut problems = both_ways("family_row codes", &op_codes, &row_codes);
        let names: std::collections::BTreeSet<String> = rows.iter().map(|f| f[0].clone()).collect();
        problems.extend(both_ways("instance_of(_, OpFamily)", &names, &instances_of(&kb, "OpFamily")));
        let op_family: std::collections::HashMap<String, String> =
            kb.facts("op_family").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
        for &op in Op::ALL {
            let name = op.meta().name;
            let code = format!("0x{:02x}", op.family());
            let stated = op_family.get(name).cloned().unwrap_or_default();
            match rows.iter().find(|f| f[0] == stated) {
                Some(row) if row[1] == code => {}
                Some(row) => problems.push(format!(
                    "`{name}` is in family {code}, and its family_row `{}` says {}",
                    row[0], row[1]
                )),
                None => problems.push(format!("`{name}` is in family `{stated}`, which has no family_row")),
            }
            match op.family_enum() {
                Some(fam) if format!("{fam:?}") == stated => {}
                Some(fam) => problems.push(format!("`{name}` decodes to {fam:?}, and kb/meaning.mg says `{stated}`")),
                None => problems.push(format!("`{name}` has family code {code}, which OpFamily does not decode")),
            }
        }
        assert!(problems.is_empty(), "kb/meaning.mg and the op families disagree:\n{}", problems.join("\n"));
    }

    /// Every layer spelling is stated, lowers to the op it names, and every op
    /// that has a canonical layer name is stated under it.
    ///
    /// The spellings are scraped from the match arms of
    /// `abl_bridge::layer_name_to_op`, the way `every_item_kind_is_published_and_vice_versa`
    /// scrapes `ast.rs`: every quoted string between `match name {` and the
    /// fallback arm, comments removed. Four things must hold. Each scraped
    /// spelling is a `Layer`, and each `Layer` is one of them. Each stated
    /// `lowers_as(layer, op)` is what `layer_name_to_op(layer)` returns. Each op
    /// with `op_to_layer_name(op) == Some(n)` has `lowers_as(n, op)`, unless `n`
    /// is one of the twelve `DECOMPILE_ONLY` names: `op_to_layer_name` names some
    /// ops by names `layer_name_to_op` does not accept (`Unify`, `Match`, `Map`
    /// and others), and those are reported, not excluded. Each `canonical_layer`,
    /// a spelling `layer_map` publishes, has a `lowers_as`.
    #[test]
    fn every_layer_spelling_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let bridge = include_str!("abl_bridge.rs");
        let start = bridge.find("pub fn layer_name_to_op(").expect("layer_name_to_op");
        let arms = &bridge[start..];
        let arms = &arms[..arms.find("_ => None").expect("the fallback arm")];
        let code_only: String = arms
            .lines()
            .map(|l| l.find("//").map_or(l, |i| &l[..i]))
            .collect::<Vec<_>>()
            .join("\n");
        let scraped: std::collections::BTreeSet<String> =
            code_only.split('"').skip(1).step_by(2).map(String::from).collect();
        assert!(scraped.len() >= 60, "scrape looks wrong: {} spellings", scraped.len());

        let stated_layers = instances_of(&kb, "Layer");
        let lowers: std::collections::BTreeSet<(String, String)> =
            kb.facts("lowers_as").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
        let accepted = |n: &str| crate::abl_bridge::layer_name_to_op(n).map_or("None".to_string(), |o| o.name().to_string());

        let mut problems = Vec::new();
        for (layer, op) in &lowers {
            if accepted(layer.as_str()) != *op {
                problems.push(format!(
                    "lowers_as({layer:?}, {op:?}), but layer_name_to_op({layer:?}) is {}",
                    accepted(layer.as_str())
                ));
            }
        }
        problems.extend(both_ways("instance_of(_, Layer)", &scraped, &stated_layers));
        // `op_to_layer_name` names these for decompile, and `layer_name_to_op`
        // refuses them: they are not layers (kb/forms.mg states the loss). The
        // list is exact, so a name that becomes a layer has to come off it.
        const DECOMPILE_ONLY: &[&str] = &[
            "Unify", "Resolve", "Infer", "Match", "Rewrite", "Plan",
            "Map", "Reduce", "Send", "Recv", "Spawn", "Delegate",
        ];
        let mut refused = std::collections::BTreeSet::new();
        for &op in Op::ALL {
            if let Some(n) = crate::abl_bridge::op_to_layer_name(op) {
                if !lowers.contains(&(n.to_string(), op.name().to_string())) {
                    refused.insert(n.to_string());
                }
            }
        }
        let listed: std::collections::BTreeSet<String> =
            DECOMPILE_ONLY.iter().map(|n| n.to_string()).collect();
        problems.extend(both_ways("named for decompile only", &listed, &refused));
        let canonical: Vec<String> = kb.facts("canonical_layer").into_iter().map(|f| f[0].clone()).collect();
        for n in &canonical {
            if !lowers.iter().any(|(layer, _)| layer == n) {
                problems.push(format!("canonical_layer({n:?}) has no lowers_as fact"));
            }
        }
        assert!(canonical.len() >= 21, "kb/meaning.mg states {} canonical layers; there are 21", canonical.len());
        assert!(problems.is_empty(), "layer names disagree:\n{}", problems.join("\n"));
    }

    /// Every type and every effect is stated, once, and every row is one.
    ///
    /// The `types` and `effects` sections are the `type_row` and `effect_row`
    /// facts of `kb/meaning.mg`. Each `Type` (or `Effect`) instance has exactly
    /// one row, and each row names an instance; both directions are checked by
    /// name. The counts are held to the floors the sections were stated at: 30
    /// types and 24 effects.
    #[test]
    fn every_type_and_effect_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let mut problems = Vec::new();
        for (row, class) in [("type_row", "Type"), ("effect_row", "Effect")] {
            let instances = instances_of(&kb, class);
            let rows: Vec<String> = kb.facts(row).into_iter().map(|f| f[0].clone()).collect();
            for name in &instances {
                let n = rows.iter().filter(|r| *r == name).count();
                if n != 1 {
                    problems.push(format!("{class} `{name}` has {n} {row} facts, not one"));
                }
            }
            for name in &rows {
                if !instances.contains(name) {
                    problems.push(format!("{row}({name:?}) names no {class}"));
                }
            }
        }
        assert!(problems.is_empty(), "kb/meaning.mg disagrees with itself:\n{}", problems.join("\n"));
        let (types, effects) = (kb.facts("type_row").len(), kb.facts("effect_row").len());
        assert!(types >= 30, "kb/meaning.mg states {types} types; at least 30 were stated");
        assert!(effects >= 24, "kb/meaning.mg states {effects} effects; at least 24 were stated");
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
        for (name, _, _) in type_rows() {
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

    /// Every `abbreviates(sigil, keyword)` pair in `kb/lexicon.mg` has a
    /// `sigil_row` fact for its sigil whose summary names the keyword as a word,
    /// so an agent reading the ontology learns the same pairs the formatter
    /// prints.
    #[test]
    fn every_abbreviation_has_a_sigil_row() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        for f in kb.facts("abbreviates") {
            let (sigil, keyword) = (&f[0], &f[1]);
            let named = sigil_rows().iter().any(|(s, _, summary)| {
                s == sigil
                    && summary
                        .split(|c: char| !c.is_ascii_alphanumeric())
                        .any(|w| w == keyword)
            });
            assert!(named, "no sigil_row for `{sigil}` names `{keyword}`");
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
        let rows = sigil_rows();
        let published = |sigil: &str| rows.iter().any(|(s, _, _)| s == sigil);
        for (sigil, src) in cases {
            let published = published(sigil);
            assert!(published, "`{sigil}` is exercised here but no longer published");
            assert!(
                crate::parser::parse(&crate::lexer::lex(src)).is_ok(),
                "published sigil `{sigil}` does not parse: {src}"
            );
        }
        // The two that were wrong, held in both directions.
        assert!(
            !published("^"),
            "`^` is bitwise XOR and the `^T` Box prefix — it does not return"
        );
        assert!(
            !published("?:"),
            "`?:` is not a token: the lexer has no such sigil, `else if` is              `: ?` with a space, and MAGE_SPEC.md B.2 published `?:` for a KB              query — two documents disagreeing with each other and with the              compiler"
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
        // Iterate what is *published*: `layer_map`, whose names are the
        // `canonical_layer` facts. `every_layer_spelling_is_stated` requires
        // each of them to be a layer.
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
            let full = root.join(path);
            assert!(full.exists(), "framewerx_modules path is gone: {path}");

            // And the published *name* has to be in that file. The path
            // existing says only that some file is there; 243 of these
            // entries name a symbol, and a rename would leave the map
            // pointing at the right file for the wrong reason.
            let name = entry["name"].as_str().unwrap();
            let category = entry["category"].as_str().unwrap_or("");
            if category == "example" {
                // An example entry is named for its file, not for a symbol.
                continue;
            }
            let src = std::fs::read_to_string(&full).unwrap_or_default();
            assert!(
                src.contains(name),
                "framewerx_modules publishes `{name}` in {path}, which does \
                 not mention it"
            );
        }
    }

    /// Every path the ontology publishes exists — or is declared to be
    /// somewhere this repository cannot see.
    ///
    /// Cheap, and the kind of claim that rots silently: a file moves and the
    /// map an agent navigates by keeps pointing at where it used to be.
    ///
    /// It also rotted in the other direction, and that is the interesting
    /// half. `DOCS` carries one entry that deliberately points *outside* the
    /// repository — `../../utilities/IronAccelerator/`, an external reference —
    /// and this test asserted it existed like any other. It passed for months
    /// on a machine where that sibling checkout happens to sit next to this
    /// one, and failed the first time it ran in CI, which is the only place
    /// that had ever told the truth about it. **A test green because of a
    /// directory outside the repository is not testing the repository.**
    ///
    /// So an escaping path is skipped, and the skip is itself asserted: it must
    /// be labelled a reference in its audience column. Otherwise `../` is
    /// indistinguishable from a typo in an in-repo path, and this test would
    /// grant an exemption to exactly the case it exists to catch.
    #[test]
    fn every_documented_path_exists() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root");
        for (path, _) in PROJECT_LAYOUT.iter().map(|(p, d)| (*p, *d)) {
            assert!(
                !path.starts_with("../"),
                "project_layout describes this repository's own layout; \
                 `{path}` escapes it and cannot be one of its directories"
            );
            assert!(root.join(path).exists(), "project_layout path is gone: {path}");
        }

        let mut external = 0;
        for (path, _, audience) in DOCS {
            if path.starts_with("../") {
                assert!(
                    audience.contains("reference"),
                    "`{path}` escapes the repository but is not labelled a \
                     reference in its audience column (`{audience}`). An \
                     in-repo path that begins with `../` is a typo, and \
                     skipping it silently is how this test came to pass on \
                     one machine only"
                );
                external += 1;
                continue;
            }
            assert!(root.join(path).exists(), "docs path is gone: {path}");
        }

        // Pin the count, so that adding a second out-of-tree reference is a
        // decision someone makes rather than one that rides along.
        assert_eq!(
            external, 1,
            "exactly one DOCS entry is an out-of-tree reference; found \
             {external}. Adding another means agents are being pointed at \
             more things a fresh checkout does not contain"
        );
    }

    /// The published examples must **typecheck**, not merely parse.
    ///
    /// Parsing was the whole bar here, and this repository has now found
    /// forty-odd bugs behind exactly that gap — a block that parses and then
    /// fails the checker is still a program an agent cannot run. These ten
    /// snippets are what an agent grounds in when it asks the ontology what
    /// MAGE looks like.
    #[test]
    fn examples_all_typecheck() {
        for (name, _, src, _) in EXAMPLES {
            let tokens = crate::lexer::lex(src);
            let module = crate::parser::parse(&tokens)
                .unwrap_or_else(|e| panic!("example {name:?} failed to parse: {e:?}"));
            let mut r = crate::resolve::Resolver::new();
            r.resolve_module(&module);
            let resolve_errors: Vec<_> = r
                .diagnostics
                .iter()
                .filter(|d| d.severity == crate::hir::Severity::Error)
                .collect();
            assert!(
                resolve_errors.is_empty(),
                "example {name:?} does not resolve: {resolve_errors:?}
source: {src}"
            );
            let tc = crate::types::check(&module);
            assert!(
                tc.diagnostics.is_empty(),
                "example {name:?} does not typecheck: {:?}
source: {src}",
                tc.diagnostics
            );
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
