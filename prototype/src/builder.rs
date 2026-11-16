//! Tool-mediated construction layer.
//!
//! The agentic-first frontier (see IDEAL_AGENTIC_LANGUAGE.md): instead of an
//! agent *emitting source text* (token cost floored by identifiers/literals) or
//! *base64 bytes* (which erases the binary win on emission), the agent emits a
//! compact, schema-validated **structured spec**, and this layer constructs +
//! validates + lowers it to the deterministic, no-exec Agentic Binary Language artifact.
//!
//! Why this beats the text-token floor:
//! - **token**: the spec is positional/minimal JSON — no keywords, no syntax,
//!   only the irreducible payload (names, ops, dims).
//! - **reliability**: invalid specs are rejected *by construction* with
//!   machine-readable errors (unknown op, bad dims, shape mismatch) BEFORE any
//!   artifact exists — the typed-API reliability the design calls for.
//! - **determinism / safety**: inherited from the Agentic Binary Language artifact (byte-stable,
//!   loads as pure data, never executes).
//!
//! Spec format (compact, positional):
//! ```json
//! {"net":"M","layers":[["fc1","Linear",[3,8]],["a1","ReLU",[]],["fc2","Linear",[8,1]]]}
//! ```

use serde::Deserialize;

/// One layer: (name, op, dims). Positional to minimize tokens.
#[derive(Debug, Clone, Deserialize)]
pub struct LayerSpec(pub String, pub String, pub Vec<i64>);

/// A net construction spec — what the agent emits instead of source text.
#[derive(Debug, Clone, Deserialize)]
pub struct NetSpec {
    pub net: String,
    pub layers: Vec<LayerSpec>,
}

/// One fact: (predicate, args). Positional. Arity = `args.len()`.
#[derive(Debug, Clone, Deserialize)]
pub struct FactSpec(pub String, pub Vec<String>);

/// One rule: (name, params, body). A Horn clause — the head predicate `name`
/// (arity = `params.len()`) holds when every body literal holds. Each body
/// literal reuses [`FactSpec`] shape `(predicate, [args])`; all args are logic
/// variables. Example: `["gp", ["x","z"], [["parent",["x","y"]],["parent",["y","z"]]]]`.
#[derive(Debug, Clone, Deserialize)]
pub struct RuleSpec(pub String, pub Vec<String>, pub Vec<FactSpec>);

/// A knowledge-base construction spec — the *symbolic* half of the
/// neurosymbolic IR. The agent emits this instead of `kb { ... }` source.
///
/// NOTE: the lowered Agentic Binary Language artifact stores predicate **arities** and
/// the unify→infer rule structure — not ground argument terms or predicate
/// names (the symbol table is not serialized). Validation operates on the spec,
/// where names/arities/refs are still present, so reject-by-construction is
/// fully enforced before the names are elided.
#[derive(Debug, Clone, Deserialize)]
pub struct KbSpec {
    pub kb: String,
    #[serde(default)]
    pub facts: Vec<FactSpec>,
    #[serde(default)]
    pub rules: Vec<RuleSpec>,
}

/// An agent construction spec — an agentic role with named capabilities.
/// Lowers to `SPAWN(agent, caps…)`; the artifact carries the agent name and
/// every capability name.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentSpec {
    pub agent: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub requires_approval: Vec<String>,
}

/// A swarm construction spec — a multi-agent topology. Lowers to
/// `SPAWN(agent_type, size) >> comm >> REDUCE`. The artifact carries the agent
/// type, size, comm pattern, and (for `rmi-*`) the transport; the exact
/// topology label is not recoverable (it only selects the comm pattern).
#[derive(Debug, Clone, Deserialize)]
pub struct SwarmSpec {
    pub swarm: String,
    pub agent: String,
    #[serde(default)]
    pub size: Option<i64>,
    #[serde(default)]
    pub topology: Option<String>,
    #[serde(default)]
    pub consensus: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
}

/// A unified construction spec: one Agentic Binary Language container holding MANY
/// items of mixed kinds (nets + knowledge bases) — i.e. build a whole
/// *neurosymbolic* application (a model AND its knowledge base) in one artifact.
/// Each item is a `net` or `kb` spec; the kind is detected by its key.
#[derive(Debug, Clone, Deserialize)]
pub struct UnifiedSpec {
    pub items: Vec<serde_json::Value>,
}

/// A classified item within a [`UnifiedSpec`].
#[derive(Debug, Clone)]
pub enum ItemSpec {
    /// A neural net item.
    Net(NetSpec),
    /// A symbolic knowledge-base item.
    Kb(KbSpec),
    /// An agent item.
    Agent(AgentSpec),
    /// A swarm item.
    Swarm(SwarmSpec),
}

impl ItemSpec {
    /// The item's declared name (its container key).
    pub fn name(&self) -> &str {
        match self {
            ItemSpec::Net(n) => &n.net,
            ItemSpec::Kb(k) => &k.kb,
            ItemSpec::Agent(a) => &a.agent,
            ItemSpec::Swarm(s) => &s.swarm,
        }
    }
    /// Validate this item with its kind-specific rules.
    pub fn validate(&self) -> Vec<BuildError> {
        match self {
            ItemSpec::Net(n) => validate(n),
            ItemSpec::Kb(k) => validate_kb(k),
            ItemSpec::Agent(a) => validate_agent(a),
            ItemSpec::Swarm(s) => validate_swarm(s),
        }
    }
    /// Render this item as canonical MAGE source.
    pub fn source(&self) -> String {
        match self {
            ItemSpec::Net(n) => to_mg_source(n),
            ItemSpec::Kb(k) => to_mg_source_kb(k),
            ItemSpec::Agent(a) => to_mg_source_agent(a),
            ItemSpec::Swarm(s) => to_mg_source_swarm(s),
        }
    }
}

/// Detect and deserialize one unified item by its discriminating key
/// (`net`/`kb`/`agent`/`swarm`). `U0002` if none match or it fails to
/// deserialize. `swarm` is checked before `agent` because a swarm spec also
/// carries an `agent` field.
pub fn classify_item(v: &serde_json::Value) -> Result<ItemSpec, BuildError> {
    let de = |kind: &str| -> Result<ItemSpec, BuildError> {
        match kind {
            "net" => serde_json::from_value(v.clone()).map(ItemSpec::Net),
            "kb" => serde_json::from_value(v.clone()).map(ItemSpec::Kb),
            "swarm" => serde_json::from_value(v.clone()).map(ItemSpec::Swarm),
            _ => serde_json::from_value(v.clone()).map(ItemSpec::Agent),
        }
        .map_err(|e| BuildError::new("U0002", format!("bad {kind} item: {e}"), "each item must be a net, kb, agent, or swarm spec; see `--build=schema`"))
    };
    if v.get("net").is_some() {
        de("net")
    } else if v.get("kb").is_some() {
        de("kb")
    } else if v.get("swarm").is_some() {
        de("swarm")
    } else if v.get("agent").is_some() {
        de("agent")
    } else {
        Err(BuildError::new(
            "U0002",
            "item has no \"net\"/\"kb\"/\"agent\"/\"swarm\" key".into(),
            "each item must be a net, kb, agent, or swarm spec; see `--build=schema`",
        ))
    }
}

/// A machine-readable construction error (the reliability surface).
#[derive(Debug, Clone)]
pub struct BuildError {
    pub code: &'static str,
    pub message: String,
    pub fix: &'static str,
}

impl BuildError {
    fn new(code: &'static str, message: String, fix: &'static str) -> Self {
        BuildError { code, message, fix }
    }
    /// A malformed-spec (`B0000`) error — for JSON that doesn't match a spec shape.
    pub fn malformed(message: String) -> Self {
        BuildError::new("B0000", message, "emit valid JSON matching a spec format; see `--build=schema`")
    }
    /// Machine-parseable one-line form (matches the --check --json spirit).
    pub fn as_json(&self) -> serde_json::Value {
        serde_json::json!({ "code": self.code, "message": self.message, "fix": self.fix })
    }
}

/// One constructible op: its dimension arity, whether it transforms the
/// running feature dim (its last dim becomes the new running dim), and a
/// one-line doc. This table is the SINGLE SOURCE OF TRUTH — `op_arity`,
/// validation, and the machine-readable `build_schema()` all derive from it,
/// so the agent-facing schema can never drift from what `validate` enforces.
pub struct OpInfo {
    /// Op name as written in a layer spec.
    pub name: &'static str,
    /// Number of trailing dims the op takes.
    pub arity: usize,
    /// Whether the op transforms the running feature dim (last dim → new dim).
    pub transforms: bool,
    /// One-line human/agent doc.
    pub doc: &'static str,
}

/// The constructible op catalog (see [`OpInfo`]). Deterministic order.
pub const OPS: &[OpInfo] = &[
    OpInfo { name: "Linear",    arity: 2, transforms: true,  doc: "fully-connected (in, out) -> out" },
    OpInfo { name: "Embedding", arity: 2, transforms: true,  doc: "lookup table (vocab, dim) -> dim" },
    OpInfo { name: "Conv2d",    arity: 2, transforms: true,  doc: "2D convolution (in_ch, out_ch) -> out_ch" },
    OpInfo { name: "LayerNorm", arity: 1, transforms: false, doc: "layer normalization (dim)" },
    OpInfo { name: "ReLU",      arity: 0, transforms: false, doc: "rectified linear activation" },
    OpInfo { name: "GELU",      arity: 0, transforms: false, doc: "Gaussian error linear activation" },
    OpInfo { name: "Tanh",      arity: 0, transforms: false, doc: "hyperbolic tangent activation" },
    OpInfo { name: "Sigmoid",   arity: 0, transforms: false, doc: "logistic activation" },
    OpInfo { name: "Softmax",   arity: 0, transforms: false, doc: "softmax over the feature dim" },
    OpInfo { name: "Dropout",   arity: 0, transforms: false, doc: "stochastic regularization (identity at inference)" },
];

fn op_info(op: &str) -> Option<&'static OpInfo> {
    OPS.iter().find(|o| o.name == op)
}

/// How many trailing dims a known op takes, and whether it transforms the
/// running feature dim. `None` ⇒ unknown op.
fn op_arity(op: &str) -> Option<(usize, bool)> {
    op_info(op).map(|o| (o.arity, o.transforms))
}

/// The machine-readable construction schema — the **typed self-describing
/// interface** of the tool-mediated paradigm. An agent fetches this ONCE
/// (`--build=schema`, prompt-cacheable standing context) and then emits specs
/// that validate first-try: it enumerates every op (from [`OPS`]), the spec
/// format, the shape rule, and the full error-code catalog with fixes. Pure +
/// deterministic, so it is content-hashable and never drifts from `validate`.
pub fn build_schema() -> serde_json::Value {
    let ops: Vec<serde_json::Value> = OPS
        .iter()
        .map(|o| {
            serde_json::json!({
                "op": o.name,
                "dims": o.arity,
                "transforms_feature_dim": o.transforms,
                "doc": o.doc,
            })
        })
        .collect();
    let knowledge = crate::kb::load().expect("the knowledge base loads");
    serde_json::json!({
        "schema": "mage.abl.net-spec",
        "version": 1,
        "spec_format": {
            "net": "string, non-empty — the net's name",
            "layers": "array of [name, op, [dims]] tuples, in forward order"
        },
        "example": {
            "net": "M",
            "layers": [["fc1", "Linear", [3, 8]], ["a1", "ReLU", []], ["fc2", "Linear", [8, 1]]]
        },
        "ops": ops,
        "shape_rule":
            "a Linear layer's first (input) dim must equal the running feature dim — \
             the last dim of the most recent dim-transforming layer",
        "errors": error_table(&knowledge, "net"),
        "kb": {
            "spec_format": {
                "kb": "string, non-empty — the knowledge base's name",
                "facts": "array of [predicate, [args]] tuples (arity = number of args)",
                "rules": "array of [name, [params], [body]] Horn clauses; body is a list of [predicate, [args]] literals (all args are logic variables); the head holds when all body literals hold; every head param must appear in the body (range-safe)"
            },
            "example": {
                "kb": "Family",
                "facts": [["parent", ["alice", "bob"]], ["parent", ["bob", "carol"]]],
                "rules": [["grandparent", ["x", "z"], [["parent", ["x", "y"]], ["parent", ["y", "z"]]]]]
            },
            "artifact_note":
                "the lowered Agentic Binary Language kb artifact stores predicate names, ground term \
                 names, and rule parameter names (the symbol table is serialized) — the full \
                 facts and rule signatures round-trip; --describe=abl reports them.",
            "errors": error_table(&knowledge, "kb")
        },
        "agent": {
            "spec_format": {
                "agent": "string identifier — the agent's name",
                "capabilities": "array of capability identifier strings (recoverable from the artifact)",
                "requires_approval": "array of operation identifier strings requiring approval (optional, recoverable)"
            },
            "example": {"agent": "Worker", "capabilities": ["read_source", "query_types"], "requires_approval": ["write_files"]},
            "errors": error_table(&knowledge, "agent")
        },
        "swarm": {
            "spec_format": {
                "swarm": "string identifier — the swarm's name",
                "agent": "string identifier — the agent type populating the swarm",
                "size": "positive integer (optional, default 1)",
                "topology": "one of: star, ring, mesh, broadcast, tree (optional)",
                "consensus": "one of: majority, unanimous, weighted, quorum (optional)",
                "transport": "an rmi_* identifier, e.g. rmi_quic / rmi_tcp / rmi_grpc (optional)"
            },
            "example": {"swarm": "Workers", "agent": "Worker", "size": 4, "topology": "ring", "consensus": "majority", "transport": "rmi_quic"},
            "artifact_note":
                "the artifact carries the agent type, size, exact topology, consensus, and \
                 transport — all round-trip via the serialized symbol table.",
            "errors": error_table(&knowledge, "swarm")
        },
        "unified": {
            "spec_format": {
                "items": "array of net and/or kb specs — build a whole neurosymbolic application (model + knowledge base) into ONE Agentic Binary Language container"
            },
            "example": {
                "items": [
                    {"net": "Encoder", "layers": [["fc1", "Linear", [8, 4]]]},
                    {"kb": "Rules", "facts": [["valid", ["x"]]], "rules": []}
                ]
            },
            "errors": error_table(&knowledge, "unified")
        }
    })
}

/// The `errors` table of one schema family: its `build_rule_row` facts as
/// `{code, when, fix}` objects, in code order. The codes, their text and their
/// fixes are stated once, in kb/rules/builder.mg, and served from there.
fn error_table(knowledge: &crate::kb::Kb, family: &str) -> serde_json::Value {
    let mut rows: Vec<(String, String, String)> = knowledge
        .facts("build_rule_row")
        .into_iter()
        .filter(|f| f[1] == family)
        .map(|f| (f[0].clone(), f[2].clone(), f[3].clone()))
        .collect();
    rows.sort();
    rows.into_iter()
        .map(|(code, when, fix)| serde_json::json!({"code": code, "when": when, "fix": fix}))
        .collect()
}

/// Validate a spec fully (collect ALL errors, not just the first — an agent
/// fixes a batch in one round). Empty result ⇒ valid.
pub fn validate(spec: &NetSpec) -> Vec<BuildError> {
    let mut errs = Vec::new();
    if spec.net.trim().is_empty() {
        errs.push(BuildError::new("B0001", "net has no name".into(), "set a non-empty \"net\" field"));
    }
    if spec.layers.is_empty() {
        errs.push(BuildError::new("B0002", "net has no layers".into(), "add at least one layer"));
        return errs;
    }
    // running feature dim (output of the last dim-transforming layer)
    let mut running: Option<i64> = None;
    for (i, LayerSpec(name, op, dims)) in spec.layers.iter().enumerate() {
        let Some((arity, transforms)) = op_arity(op) else {
            errs.push(BuildError::new(
                "B0003",
                format!("layer `{name}` (#{i}): unknown op `{op}`"),
                "use an op listed in `ops` (see `mage-parse --build=schema`)",
            ));
            continue;
        };
        if dims.len() != arity {
            errs.push(BuildError::new(
                "B0004",
                format!("layer `{name}` (#{i}): op `{op}` takes {arity} dim(s), got {}", dims.len()),
                "match the op's `dims` arity",
            ));
            continue;
        }
        if dims.iter().any(|&d| d <= 0) {
            errs.push(BuildError::new(
                "B0005",
                format!("layer `{name}` (#{i}): dims must be positive, got {dims:?}"),
                "use positive integers for dims",
            ));
            continue;
        }
        // Shape-chain check: a Linear's input dim must match the running dim.
        if op == "Linear" {
            let in_dim = dims[0];
            if let Some(prev) = running
                && prev != in_dim {
                    errs.push(BuildError::new(
                        "B0006",
                        format!(
                            "shape mismatch at `{name}` (#{i}): previous output is {prev}, `Linear` expects input {in_dim}"
                        ),
                        "make each Linear's input dim equal the previous layer's output dim",
                    ));
                }
        }
        if transforms {
            running = Some(*dims.last().unwrap());
        }
    }
    errs
}

/// Render the validated spec as canonical MAGE `net` source. (Internal — the
/// agent never writes or sees this; it's the bridge to the existing, tested
/// lexer→parser→Agentic Binary Language pipeline. Deterministic: fixed field order.)
pub fn to_mg_source(spec: &NetSpec) -> String {
    let mut s = format!("net {} {{\n", spec.net);
    for LayerSpec(name, op, dims) in &spec.layers {
        let args: Vec<String> = dims.iter().map(|d| d.to_string()).collect();
        if args.is_empty() {
            s.push_str(&format!("    layer {name}: {op};\n"));
        } else {
            s.push_str(&format!("    layer {name}: {op}({});\n", args.join(", ")));
        }
    }
    // single layer name ⇒ "run all declared layers in order" (lowering convention)
    let first = &spec.layers[0].0;
    s.push_str(&format!("    forward {{ {first} }}\n}}\n"));
    s
}

/// A name is a valid predicate/rule identifier: `[A-Za-z_][A-Za-z0-9_]*`.
fn is_ident(s: &str) -> bool {
    let mut cs = s.chars();
    matches!(cs.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && cs.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// Validate a knowledge-base spec fully (collect ALL errors). Empty ⇒ valid.
/// Reject-by-construction for the symbolic IR:
/// - K0001 empty kb name, K0002 empty kb (no facts and no rules)
/// - K0003 invalid predicate/rule/arg identifier
/// - K0004 arity conflict (a predicate used at two different arities)
/// - K0006 dangling reference (a rule ref that resolves to no declared predicate)
/// - K0007 unsafe rule (a head param no body literal binds)
/// - K0008 anything else the logic engine refuses in the rendered source: the
///   same check `--check` runs, so a built kb is one `--check` accepts
pub fn validate_kb(spec: &KbSpec) -> Vec<BuildError> {
    use std::collections::HashMap;
    let mut errs = Vec::new();
    if spec.kb.trim().is_empty() {
        errs.push(BuildError::new("K0001", "kb has no name".into(), "set a non-empty \"kb\" field"));
    }
    if spec.facts.is_empty() && spec.rules.is_empty() {
        errs.push(BuildError::new("K0002", "kb has no facts or rules".into(), "add at least one fact or rule"));
        return errs;
    }
    // Collect predicate declarations (facts and rules both define predicates),
    // checking identifiers as we go.
    let mut decls: Vec<(&str, usize, &str)> = Vec::new(); // (name, arity, kind)
    for FactSpec(name, args) in &spec.facts {
        if !is_ident(name) {
            errs.push(BuildError::new("K0003", format!("fact predicate `{name}` is not a valid identifier"), "use [A-Za-z_][A-Za-z0-9_]* for predicates, rules, params, terms"));
            continue;
        }
        for arg in args {
            if !is_ident(arg) {
                errs.push(BuildError::new("K0003", format!("fact `{name}` arg `{arg}` is not a valid term identifier"), "use [A-Za-z_][A-Za-z0-9_]* for predicates, rules, params, terms"));
            }
        }
        decls.push((name, args.len(), "fact"));
    }
    for RuleSpec(name, params, body) in &spec.rules {
        if !is_ident(name) {
            errs.push(BuildError::new("K0003", format!("rule `{name}` is not a valid identifier"), "use [A-Za-z_][A-Za-z0-9_]* for predicates, rules, params, terms"));
            continue;
        }
        for p in params {
            if !is_ident(p) {
                errs.push(BuildError::new("K0003", format!("rule `{name}` param `{p}` is not a valid identifier"), "use [A-Za-z_][A-Za-z0-9_]* for predicates, rules, params, terms"));
            }
        }
        for FactSpec(bp, bargs) in body {
            if !is_ident(bp) {
                errs.push(BuildError::new("K0003", format!("rule `{name}` body predicate `{bp}` is not a valid identifier"), "use [A-Za-z_][A-Za-z0-9_]* for predicates, rules, params, terms"));
            }
            for a in bargs {
                if !is_ident(a) {
                    errs.push(BuildError::new("K0003", format!("rule `{name}` body arg `{a}` is not a valid identifier"), "use [A-Za-z_][A-Za-z0-9_]* for predicates, rules, params, terms"));
                }
            }
        }
        decls.push((name, params.len(), "rule"));
    }
    // Arity-consistency check: a predicate may not appear at two arities.
    let mut arity: HashMap<&str, usize> = HashMap::new();
    for &(name, a, what) in &decls {
        match arity.get(name) {
            Some(&prev) if prev != a => errs.push(BuildError::new(
                "K0004",
                format!("predicate `{name}` ({what}) has arity {a} but was already declared with arity {prev}"),
                "use a consistent arity for each predicate",
            )),
            Some(_) => {}
            None => {
                arity.insert(name, a);
            }
        }
    }
    // K0006 (dangling reference): every body predicate must be declared.
    // K0007 (range restriction / safety): every head param must appear in the
    // body — an unbound head variable makes the rule unsafe (non-evaluable).
    for RuleSpec(name, params, body) in &spec.rules {
        for FactSpec(bp, _) in body {
            if !arity.contains_key(bp.as_str()) {
                errs.push(BuildError::new(
                    "K0006",
                    format!("rule `{name}` body references unknown predicate `{bp}`"),
                    "reference a declared fact-predicate or rule, or add it",
                ));
            }
        }
        let body_vars: std::collections::HashSet<&str> = body
            .iter()
            .flat_map(|FactSpec(_, a)| a.iter().map(|s| s.as_str()))
            .collect();
        for p in params {
            if !body_vars.contains(p.as_str()) {
                errs.push(BuildError::new(
                    "K0007",
                    format!("rule `{name}` head param `{p}` is not bound by the body (unsafe rule)"),
                    "use each head param as an argument in some body literal",
                ));
            }
        }
    }
    if errs.is_empty() {
        let src = to_mg_source_kb(spec);
        match crate::parser::parse(&crate::lexer::lex(&src)) {
            Ok(module) => {
                for d in crate::logic::module_diagnostics(&module) {
                    errs.push(BuildError::new("K0008", d.message, "give every rule at least one body literal; the message says what else"));
                }
            }
            Err(e) => errs.push(BuildError::new("K0008", format!("rendered kb does not parse: {e:?}"), "give every rule at least one body literal; the message says what else")),
        }
    }
    errs
}

/// Render a validated kb spec as canonical MAGE `kb` source. Internal bridge
/// to the tested lexer→parser→Agentic Binary Language pipeline. Deterministic.
///
/// A rule's body literals become its block, joined by `&&`: the form
/// MAGE_SPEC §7.1 gives and `logic::read_rule` reads. Until 2026-10-29 they
/// went in `where` with a placeholder block `{ x }`, which `--check` refuses.
pub fn to_mg_source_kb(spec: &KbSpec) -> String {
    let mut s = format!("kb {} {{\n", spec.kb);
    for FactSpec(name, args) in &spec.facts {
        s.push_str(&format!("    fact {name}({});\n", args.join(", ")));
    }
    for RuleSpec(name, params, body) in &spec.rules {
        let typed: Vec<String> = params.iter().map(|p| format!("{p}: i32")).collect();
        let lits: Vec<String> = body
            .iter()
            .map(|FactSpec(p, a)| format!("{p}({})", a.join(", ")))
            .collect();
        s.push_str(&format!("    rule {name}({}) {{ {} }}\n", typed.join(", "), lits.join(" && ")));
    }
    s.push_str("}\n");
    s
}

/// Valid swarm communication topologies (each selects a comm pattern).
pub(crate) const TOPOLOGIES: &[&str] = &["star", "ring", "mesh", "broadcast", "tree"];
/// Valid swarm consensus strategies.
pub(crate) const CONSENSUS: &[&str] = &["majority", "unanimous", "weighted", "quorum"];

/// Validate an agent spec. A0001 invalid name; A0002 invalid capability;
/// A0003 invalid `requires_approval` entry.
pub fn validate_agent(spec: &AgentSpec) -> Vec<BuildError> {
    let mut errs = Vec::new();
    if !is_ident(&spec.agent) {
        errs.push(BuildError::new("A0001", format!("agent name `{}` is not a valid identifier", spec.agent), "use [A-Za-z_][A-Za-z0-9_]*"));
    }
    for (i, c) in spec.capabilities.iter().enumerate() {
        if !is_ident(c) {
            errs.push(BuildError::new("A0002", format!("capability #{i} (`{c}`) is not a valid identifier"), "use [A-Za-z_][A-Za-z0-9_]* capability names"));
        }
    }
    for (i, r) in spec.requires_approval.iter().enumerate() {
        if !is_ident(r) {
            errs.push(BuildError::new("A0003", format!("requires_approval #{i} (`{r}`) is not a valid identifier"), "use [A-Za-z_][A-Za-z0-9_]* operation names"));
        }
    }
    errs
}

/// Validate a swarm spec. S0001 bad swarm name; S0002 bad agent type;
/// S0003 non-positive size; S0004 unknown topology; S0005 unknown consensus;
/// S0006 transport that won't be encoded (must be an `rmi_*` identifier).
pub fn validate_swarm(spec: &SwarmSpec) -> Vec<BuildError> {
    let mut errs = Vec::new();
    if !is_ident(&spec.swarm) {
        errs.push(BuildError::new("S0001", format!("swarm name `{}` is not a valid identifier", spec.swarm), "use [A-Za-z_][A-Za-z0-9_]*"));
    }
    if !is_ident(&spec.agent) {
        errs.push(BuildError::new("S0002", format!("agent type `{}` is not a valid identifier", spec.agent), "use [A-Za-z_][A-Za-z0-9_]*"));
    }
    if let Some(n) = spec.size
        && n <= 0 {
            errs.push(BuildError::new("S0003", format!("size must be positive, got {n}"), "use a positive integer size"));
        }
    if let Some(t) = &spec.topology
        && !TOPOLOGIES.contains(&t.as_str()) {
            errs.push(BuildError::new("S0004", format!("unknown topology `{t}`"), "use star, ring, mesh, broadcast, or tree"));
        }
    if let Some(c) = &spec.consensus
        && !CONSENSUS.contains(&c.as_str()) {
            errs.push(BuildError::new("S0005", format!("unknown consensus `{c}`"), "use majority, unanimous, weighted, or quorum"));
        }
    if let Some(t) = &spec.transport {
        // Only `rmi_*` transports are encoded into the artifact; reject anything
        // else (and non-identifiers) so the spec can't claim a silent no-op.
        if !is_ident(t) || !t.starts_with("rmi_") {
            errs.push(BuildError::new("S0006", format!("transport `{t}` is not encodable"), "use an rmi_* identifier (e.g. rmi_quic, rmi_tcp, rmi_grpc)"));
        }
    }
    errs
}

/// Render an agent spec as canonical MAGE source.
pub fn to_mg_source_agent(spec: &AgentSpec) -> String {
    let mut s = format!("agent {} {{\n", spec.agent);
    if !spec.capabilities.is_empty() {
        // Capabilities are bare identifiers in the grammar (parse_bracket_string_list).
        s.push_str(&format!("    capabilities: [{}]\n", spec.capabilities.join(", ")));
    }
    if !spec.requires_approval.is_empty() {
        s.push_str(&format!("    requires_approval: [{}]\n", spec.requires_approval.join(", ")));
    }
    s.push_str("}\n");
    s
}

/// Render a swarm spec as canonical MAGE source.
pub fn to_mg_source_swarm(spec: &SwarmSpec) -> String {
    let mut s = format!("swarm {} {{\n    agent: {};\n", spec.swarm, spec.agent);
    if let Some(n) = spec.size {
        s.push_str(&format!("    size: {n};\n"));
    }
    if let Some(t) = &spec.topology {
        s.push_str(&format!("    topology: {t};\n"));
    }
    if let Some(c) = &spec.consensus {
        s.push_str(&format!("    consensus: {c};\n"));
    }
    if let Some(t) = &spec.transport {
        s.push_str(&format!("    transport: {t};\n"));
    }
    s.push_str("}\n");
    s
}

/// Validate a unified multi-item spec: every item is validated with its own
/// kind-specific rules (errors prefixed with the item index), plus:
/// - U0001 the spec has no items
/// - U0002 an item is neither a net nor a kb (via [`classify_item`])
/// - U0003 two items share a name (ambiguous in the container, which is keyed
///   by name)
pub fn validate_unified(spec: &UnifiedSpec) -> Vec<BuildError> {
    use std::collections::HashMap;
    let mut errs = Vec::new();
    if spec.items.is_empty() {
        errs.push(BuildError::new("U0001", "unified spec has no items".into(), "add at least one net or kb item"));
        return errs;
    }
    let mut names: HashMap<String, usize> = HashMap::new();
    for (i, v) in spec.items.iter().enumerate() {
        match classify_item(v) {
            Ok(item) => {
                for mut e in item.validate() {
                    e.message = format!("item #{i}: {}", e.message);
                    errs.push(e);
                }
                let nm = item.name().to_string();
                if let Some(&j) = names.get(&nm) {
                    errs.push(BuildError::new(
                        "U0003",
                        format!("item #{i}: duplicate name `{nm}` (already used by item #{j})"),
                        "give each item a unique name",
                    ));
                } else {
                    names.insert(nm, i);
                }
            }
            Err(mut e) => {
                e.message = format!("item #{i}: {}", e.message);
                errs.push(e);
            }
        }
    }
    errs
}

// ── Auto-fix / ranked repair ─────────────────────────────────────────
//
// Reject-by-construction tells the agent *what* is wrong with a stable code +
// fix hint. Repair goes one step further: it proposes (and, with `--fix`,
// applies) the concrete edit, closing the self-correction loop without a round
// trip. Repairs are deterministic and conservative — only unambiguous fixes are
// auto-applied; everything else is surfaced as a ranked suggestion.

/// Levenshtein edit distance (small strings; iterative two-row).
fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The closest candidate to `target` within `max` edits, if any.
pub(crate) fn nearest<'a>(target: &str, candidates: &[&'a str], max: usize) -> Option<&'a str> {
    candidates
        .iter()
        .map(|c| (*c, levenshtein(target, c)))
        .filter(|(_, d)| *d <= max)
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| c)
}

/// Sanitize a string into a valid identifier (for suggestions).
///
/// Part of the agent-facing surface: the `fix` strings for B0001/K0003 tell an
/// agent to use `[A-Za-z_][A-Za-z0-9_]*`, and this is how it turns a rejected
/// name into an acceptable one without guessing at the rule.
pub fn to_ident(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()) {
            out.push(c);
        } else if c.is_ascii_digit() && i == 0 {
            out.push('_');
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { out.push('_'); }
    out
}

/// Auto-repair a net spec in place; returns a description of each applied fix.
/// Fixes: unknown op → nearest known op; non-positive dim → 1; Linear input dim
/// → previous output (shape chain). Conservative: an unknown op with no close
/// match is left for the agent.
pub fn repair_net(spec: &mut NetSpec) -> Vec<String> {
    let mut fixes = Vec::new();
    let op_names: Vec<&str> = OPS.iter().map(|o| o.name).collect();
    let mut running: Option<i64> = None;
    for LayerSpec(name, op, dims) in spec.layers.iter_mut() {
        if op_info(op).is_none()
            && let Some(best) = nearest(op, &op_names, 3) {
                fixes.push(format!("layer `{name}`: op `{op}` → `{best}`"));
                *op = best.to_string();
            }
        for d in dims.iter_mut() {
            if *d <= 0 {
                fixes.push(format!("layer `{name}`: dim {d} → 1"));
                *d = 1;
            }
        }
        if op == "Linear" && dims.len() == 2
            && let Some(prev) = running
                && dims[0] != prev {
                    fixes.push(format!("layer `{name}`: input dim {} → {prev} (shape chain)", dims[0]));
                    dims[0] = prev;
                }
        if let Some((_, true)) = op_arity(op)
            && let Some(last) = dims.last() {
                running = Some(*last);
            }
    }
    fixes
}

/// Auto-repair a swarm spec: snap unknown topology/consensus to the nearest
/// valid value; replace an unencodable transport with `rmi_quic`.
pub fn repair_swarm(spec: &mut SwarmSpec) -> Vec<String> {
    let mut fixes = Vec::new();
    if let Some(t) = &spec.topology
        && !TOPOLOGIES.contains(&t.as_str())
            && let Some(best) = nearest(t, TOPOLOGIES, 4) {
                fixes.push(format!("topology `{t}` → `{best}`"));
                spec.topology = Some(best.to_string());
            }
    if let Some(c) = &spec.consensus
        && !CONSENSUS.contains(&c.as_str())
            && let Some(best) = nearest(c, CONSENSUS, 4) {
                fixes.push(format!("consensus `{c}` → `{best}`"));
                spec.consensus = Some(best.to_string());
            }
    if let Some(t) = &spec.transport
        && (!is_ident(t) || !t.starts_with("rmi_")) {
            fixes.push(format!("transport `{t}` → `rmi_quic`"));
            spec.transport = Some("rmi_quic".to_string());
        }
    fixes
}

/// Render a validated unified spec as one MAGE module (each item's canonical
/// source, concatenated). Assumes [`validate_unified`] passed; unclassifiable
/// items are skipped.
pub fn to_mg_source_unified(spec: &UnifiedSpec) -> String {
    let mut s = String::new();
    for v in &spec.items {
        if let Ok(item) = classify_item(v) {
            s.push_str(&item.source());
            s.push('\n');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(json: &str) -> NetSpec {
        serde_json::from_str(json).expect("parse spec")
    }

    #[test]
    fn valid_spec_passes_and_lowers() {
        let s = spec(r#"{"net":"M","layers":[["fc1","Linear",[3,8]],["a","ReLU",[]],["fc2","Linear",[8,1]]]}"#);
        assert!(validate(&s).is_empty(), "{:?}", validate(&s));
        let src = to_mg_source(&s);
        // The generated source must parse + check clean through the real pipeline.
        let toks = crate::lexer::lex(&src);
        let m = crate::parser::parse(&toks).expect("generated net parses");
        let r = crate::resolve::resolve(&m);
        assert!(
            !r.diagnostics.iter().any(|d| d.severity == crate::hir::Severity::Error),
            "generated source should resolve clean: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn shape_mismatch_is_rejected_by_construction() {
        // fc1 outputs 8, fc2 expects 4 → must be caught BEFORE any artifact.
        let s = spec(r#"{"net":"M","layers":[["fc1","Linear",[3,8]],["fc2","Linear",[4,1]]]}"#);
        let e = validate(&s);
        assert!(e.iter().any(|x| x.code == "B0006"), "shape mismatch must be flagged: {e:?}");
    }

    #[test]
    fn unknown_op_and_bad_dims_rejected() {
        let s = spec(r#"{"net":"M","layers":[["x","Frobnicate",[3]],["y","Linear",[0,8]]]}"#);
        let e = validate(&s);
        assert!(e.iter().any(|x| x.code == "B0003"), "unknown op");
        assert!(e.iter().any(|x| x.code == "B0005"), "non-positive dim");
    }

    #[test]
    fn construction_is_deterministic() {
        let s = spec(r#"{"net":"M","layers":[["fc1","Linear",[3,8]],["fc2","Linear",[8,1]]]}"#);
        assert_eq!(to_mg_source(&s), to_mg_source(&s));
    }

    #[test]
    fn schema_is_deterministic_and_covers_every_op() {
        // Deterministic (content-hashable standing context).
        assert_eq!(build_schema(), build_schema());
        // Drift guard: the schema's op catalog must list EXACTLY the ops the
        // validator accepts — same names, same arity. If OPS changes and the
        // schema is regenerated from it (it is), this stays true; if anyone
        // hand-edits the schema out of sync, this fails.
        let schema = build_schema();
        let ops = schema["ops"].as_array().expect("ops array");
        assert_eq!(ops.len(), OPS.len(), "schema op count diverged from OPS");
        for o in OPS {
            let entry = ops
                .iter()
                .find(|e| e["op"] == o.name)
                .unwrap_or_else(|| panic!("schema missing op {}", o.name));
            assert_eq!(entry["dims"].as_u64().unwrap() as usize, o.arity, "{} arity", o.name);
            // The validator must agree with the schema for this op.
            assert_eq!(op_arity(o.name), Some((o.arity, o.transforms)), "{} op_arity", o.name);
        }
    }

    #[test]
    fn build_then_describe_roundtrips_structure_no_exec() {
        // The paradigm's end-to-end invariant: a valid spec lowers to Agentic Binary Language
        // bytes, those bytes are byte-stable, and decoding them as PURE DATA
        // (no execution) recovers the SAME op/dim structure the spec declared.
        // This is the "deterministic, introspectable, no-exec artifact" claim.
        let s = spec(
            r#"{"net":"M","layers":[["fc1","Linear",[4,16]],["a","ReLU",[]],["fc2","Linear",[16,3]]]}"#,
        );
        assert!(validate(&s).is_empty());
        let src = to_mg_source(&s);
        let module = crate::parser::parse(&crate::lexer::lex(&src)).expect("parses");

        let (blob, summary) = crate::abl::encode_module(&module);
        assert_eq!(summary.len(), 1, "one net → one item");
        // Byte-stable artifact (content-hashable cache key).
        let (blob2, _) = crate::abl::encode_module(&module);
        assert_eq!(blob, blob2, "Agentic Binary Language artifact is not byte-stable");

        // No-exec decode recovers the structure faithfully.
        let items = crate::abl::decode_container(&blob).expect("decode pure data");
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].expr.content_hash(),
            summary[0].2,
            "describe hash must equal encode hash"
        );
        let result = crate::abl_bridge::decompile(&items[0].expr, &items[0].name);
        let ops: Vec<String> = result
            .net
            .layers
            .iter()
            .map(|l| match &l.layer_type {
                crate::ast::Type::Path { segments, .. } => {
                    segments.last().cloned().unwrap_or_default()
                }
                _ => "?".into(),
            })
            .collect();
        assert_eq!(ops, vec!["Linear", "ReLU", "Linear"], "decoded ops must match the spec");
        // The dim-carrying layers round-trip their dims.
        let dims: Vec<Vec<i64>> = result
            .net
            .layers
            .iter()
            .map(|l| {
                l.args
                    .iter()
                    .filter_map(|a| match a {
                        crate::ast::Expr::Literal { value, .. } => value.parse::<i64>().ok(),
                        _ => None,
                    })
                    .collect()
            })
            .collect();
        assert_eq!(dims, vec![vec![4, 16], vec![], vec![16, 3]], "decoded dims must match the spec");
    }

    fn kbspec(json: &str) -> KbSpec {
        serde_json::from_str(json).expect("parse kb spec")
    }

    #[test]
    fn valid_kb_lowers_and_describes_structure_no_exec() {
        // Build → encode → decode (pure data) recovers the symbolic STRUCTURE:
        // one arity per fact, one param-count per rule. Names are not in the
        // artifact, by design — so we assert on arities/counts, deterministically.
        let s = kbspec(
            r#"{"kb":"Family","facts":[["parent",["a","b"]],["parent",["b","c"]],["male",["a"]]],"rules":[["grandparent",["x","z"],[["parent",["x","y"]],["parent",["y","z"]]]]]}"#,
        );
        assert!(validate_kb(&s).is_empty(), "{:?}", validate_kb(&s));
        let src = to_mg_source_kb(&s);
        let module = crate::parser::parse(&crate::lexer::lex(&src)).expect("generated kb parses");

        let (blob, summary) = crate::abl::encode_module(&module);
        assert_eq!(summary.len(), 1, "one kb → one item");
        let (blob2, _) = crate::abl::encode_module(&module);
        assert_eq!(blob, blob2, "kb artifact is not byte-stable");

        let items = crate::abl::decode_container(&blob).expect("decode pure data");
        let view = crate::abl_bridge::decompile_symbolic(&items[0].expr);
        assert_eq!(view.fact_arities, vec![2, 2, 1], "fact arities must round-trip");
        assert_eq!(view.rule_param_counts, vec![2], "rule param-count must round-trip");

        // Symbol table (v2): predicate NAMES round-trip too (no execution).
        let symbols = crate::abl::decode_symbols(&blob).expect("decode symbols");
        let names: Vec<&str> = view
            .fact_syms
            .iter()
            .map(|&id| symbols.get(id as usize).map(|s| s.as_str()).unwrap_or(""))
            .collect();
        assert_eq!(names, vec!["parent", "parent", "male"], "fact names must round-trip");
        let rule_name = symbols.get(view.rule_syms[0] as usize).map(|s| s.as_str());
        assert_eq!(rule_name, Some("grandparent"), "rule name must round-trip");
    }

    #[test]
    fn kb_rejects_arity_conflict_dangling_ref_and_bad_ident() {
        // parent/2 then parent/3 → K0004; rule refs unknown `ancestor` → K0006;
        // bad identifier `2bad` → K0003.
        let s = kbspec(
            r#"{"kb":"K","facts":[["parent",["a","b"]],["parent",["a","b","c"]],["2bad",["x"]]],"rules":[["r",["x"],[["ancestor",["x"]]]]]}"#,
        );
        let e = validate_kb(&s);
        assert!(e.iter().any(|x| x.code == "K0004"), "arity conflict: {e:?}");
        assert!(e.iter().any(|x| x.code == "K0006"), "dangling ref: {e:?}");
        assert!(e.iter().any(|x| x.code == "K0003"), "bad identifier: {e:?}");
    }

    #[test]
    fn kb_rejects_empty() {
        assert!(validate_kb(&kbspec(r#"{"kb":"","facts":[],"rules":[]}"#))
            .iter()
            .any(|x| x.code == "K0001"));
        assert!(validate_kb(&kbspec(r#"{"kb":"K","facts":[],"rules":[]}"#))
            .iter()
            .any(|x| x.code == "K0002"));
    }

    #[test]
    fn schema_documents_kb_kind() {
        let schema = build_schema();
        let kb = &schema["kb"];
        assert!(kb["spec_format"]["facts"].is_string(), "kb spec_format present");
        let codes: std::collections::HashSet<&str> = kb["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["code"].as_str().unwrap())
            .collect();
        for c in ["K0001", "K0002", "K0003", "K0004", "K0006"] {
            assert!(codes.contains(c), "kb schema missing {c}");
        }
    }

    fn unifiedspec(json: &str) -> UnifiedSpec {
        serde_json::from_str(json).expect("parse unified spec")
    }

    #[test]
    fn unified_builds_a_multi_item_neurosymbolic_container() {
        // A model AND its knowledge base in ONE artifact.
        let s = unifiedspec(
            r#"{"items":[
                {"net":"Encoder","layers":[["fc1","Linear",[8,4]],["a","ReLU",[]]]},
                {"kb":"Rules","facts":[["valid",["x"]]],"rules":[["ok",["y"],[["valid",["y"]]]]]}
            ]}"#,
        );
        assert!(validate_unified(&s).is_empty(), "{:?}", validate_unified(&s));
        let src = to_mg_source_unified(&s);
        let module = crate::parser::parse(&crate::lexer::lex(&src)).expect("unified source parses");
        let (blob, summary) = crate::abl::encode_module(&module);
        assert_eq!(summary.len(), 2, "two items → two container entries");
        // Byte-stable, and both items decode (net + kb).
        let (blob2, _) = crate::abl::encode_module(&module);
        assert_eq!(blob, blob2, "unified container not byte-stable");
        let items = crate::abl::decode_container(&blob).expect("decode");
        assert_eq!(items.len(), 2);
        // item 0 is the net (has layer ops), item 1 is the kb (has RESOLVE/UNIFY).
        assert!(!crate::abl_bridge::decompile(&items[0].expr, &items[0].name).net.layers.is_empty());
        let kb_view = crate::abl_bridge::decompile_symbolic(&items[1].expr);
        assert_eq!(kb_view.fact_arities, vec![1]);
        assert_eq!(kb_view.rule_param_counts, vec![1]);
    }

    #[test]
    fn unified_rejects_dupes_empty_unknown_and_prefixes_item_errors() {
        // duplicate names
        let dup = unifiedspec(r#"{"items":[{"net":"M","layers":[["a","ReLU",[]]]},{"net":"M","layers":[["b","ReLU",[]]]}]}"#);
        assert!(validate_unified(&dup).iter().any(|e| e.code == "U0003"), "dup: {:?}", validate_unified(&dup));
        // empty
        assert!(validate_unified(&unifiedspec(r#"{"items":[]}"#)).iter().any(|e| e.code == "U0001"));
        // unknown item kind
        assert!(validate_unified(&unifiedspec(r#"{"items":[{"foo":1}]}"#)).iter().any(|e| e.code == "U0002"));
        // item-level error is prefixed with the item index
        let bad = unifiedspec(r#"{"items":[{"net":"A","layers":[["a","ReLU",[]]]},{"net":"B","layers":[["x","Linear",[3,8]],["y","Linear",[4,1]]]}]}"#);
        let e = validate_unified(&bad);
        assert!(e.iter().any(|x| x.code == "B0006" && x.message.contains("item #1")), "prefixed shape error: {e:?}");
    }

    #[test]
    fn schema_documents_unified_kind() {
        let u = &build_schema()["unified"];
        assert!(u["spec_format"]["items"].is_string());
        let codes: std::collections::HashSet<&str> = u["errors"].as_array().unwrap()
            .iter().map(|e| e["code"].as_str().unwrap()).collect();
        for c in ["U0001", "U0002", "U0003"] {
            assert!(codes.contains(c), "unified schema missing {c}");
        }
    }

    #[test]
    fn agent_lowers_and_capabilities_round_trip() {
        let s: AgentSpec = serde_json::from_str(
            r#"{"agent":"Worker","capabilities":["read_source","query_types"]}"#,
        ).unwrap();
        assert!(validate_agent(&s).is_empty(), "{:?}", validate_agent(&s));
        let module = crate::parser::parse(&crate::lexer::lex(&to_mg_source_agent(&s))).expect("agent parses");
        let (blob, _) = crate::abl::encode_module(&module);
        let items = crate::abl::decode_container(&blob).expect("decode");
        let symbols = crate::abl::decode_symbols(&blob).expect("symbols");
        let ag = crate::abl_bridge::decompile_agentic(&items[0].expr).expect("agentic");
        assert!(!ag.is_swarm, "agent is not a swarm");
        let caps: Vec<&str> = ag.cap_syms.iter().map(|&id| symbols[id as usize].as_str()).collect();
        assert_eq!(caps, vec!["read_source", "query_types"], "capability names round-trip");
    }

    #[test]
    fn swarm_lowers_with_size_and_comm() {
        let s: SwarmSpec = serde_json::from_str(
            r#"{"swarm":"Workers","agent":"Worker","size":4,"topology":"ring"}"#,
        ).unwrap();
        assert!(validate_swarm(&s).is_empty(), "{:?}", validate_swarm(&s));
        let module = crate::parser::parse(&crate::lexer::lex(&to_mg_source_swarm(&s))).expect("swarm parses");
        let (blob, _) = crate::abl::encode_module(&module);
        let items = crate::abl::decode_container(&blob).expect("decode");
        let symbols = crate::abl::decode_symbols(&blob).expect("symbols");
        let ag = crate::abl_bridge::decompile_agentic(&items[0].expr).expect("agentic");
        assert!(ag.is_swarm, "swarm has REDUCE aggregate");
        assert_eq!(ag.size, Some(4), "size round-trips");
        assert_eq!(ag.spawn_sym.map(|id| symbols[id as usize].as_str()), Some("Worker"));
        assert!(ag.has_send && ag.has_recv, "ring topology lowers to send>>recv");
    }

    #[test]
    fn kb_facts_round_trip_ground_terms() {
        let s = kbspec(r#"{"kb":"F","facts":[["parent",["alice","bob"]]],"rules":[]}"#);
        let module = crate::parser::parse(&crate::lexer::lex(&to_mg_source_kb(&s))).expect("parses");
        let (blob, _) = crate::abl::encode_module(&module);
        let items = crate::abl::decode_container(&blob).unwrap();
        let syms = crate::abl::decode_symbols(&blob).unwrap();
        let view = crate::abl_bridge::decompile_symbolic(&items[0].expr);
        let terms: Vec<&str> = view.fact_arg_syms[0].iter().map(|&id| syms[id as usize].as_str()).collect();
        assert_eq!(terms, vec!["alice", "bob"], "ground terms must round-trip");
    }

    #[test]
    fn kb_forward_chaining_derives_grandparent() {
        // Full vertical: spec → source → encode → decode → reconstruct Horn
        // clause → evaluate to fixpoint → derive grandparent(alice, carol).
        let s = kbspec(r#"{"kb":"F","facts":[["parent",["alice","bob"]],["parent",["bob","carol"]]],"rules":[["grandparent",["x","z"],[["parent",["x","y"]],["parent",["y","z"]]]]]}"#);
        assert!(validate_kb(&s).is_empty(), "{:?}", validate_kb(&s));
        let module = crate::parser::parse(&crate::lexer::lex(&to_mg_source_kb(&s))).expect("parses");
        let (blob, _) = crate::abl::encode_module(&module);
        let items = crate::abl::decode_container(&blob).unwrap();
        let syms = crate::abl::decode_symbols(&blob).unwrap();
        let name = |id: u32| syms[id as usize].clone();
        let v = crate::abl_bridge::decompile_symbolic(&items[0].expr);
        let (facts, rules) = crate::abl_bridge::kb_program(&v, name);
        assert_eq!(rules[0].body.len(), 2, "rule body round-trips two literals");
        let derived = crate::abl_bridge::evaluate_kb(&facts, &rules);
        assert!(
            derived.iter().any(|(p, a)| p == "grandparent" && a == &vec!["alice".to_string(), "carol".to_string()]),
            "must derive grandparent(alice, carol): {derived:?}"
        );
    }

    /// A rule with no body literals rendered to `{  }`, which the engine
    /// refuses; the builder now asks the engine before building.
    #[test]
    fn kb_rejects_a_rule_the_logic_engine_refuses() {
        let s = kbspec(r#"{"kb":"K","facts":[["p",["a"]]],"rules":[["q",[],[]]]}"#);
        let e = validate_kb(&s);
        assert!(e.iter().any(|x| x.code == "K0008" && x.message.contains("rule `q`")), "{e:?}");
    }

    /// What the builder renders is what `--check` accepts.
    #[test]
    fn a_built_kb_passes_check() {
        let s = kbspec(r#"{"kb":"F","facts":[["parent",["alice","bob"]]],"rules":[["grandparent",["x","z"],[["parent",["x","y"]],["parent",["y","z"]]]]]}"#);
        let src = to_mg_source_kb(&s);
        assert!(src.contains("rule grandparent(x: i32, z: i32) { parent(x, y) && parent(y, z) }"), "{src}");
        let module = crate::parser::parse(&crate::lexer::lex(&src)).expect("parses");
        assert!(crate::logic::module_diagnostics(&module).is_empty());
    }

    #[test]
    fn kb_rejects_unsafe_rule_unbound_head_var() {
        // head param `y` never appears in the body → range-restriction failure.
        let s = kbspec(r#"{"kb":"K","facts":[["p",["a"]]],"rules":[["q",["x","y"],[["p",["x"]]]]]}"#);
        assert!(validate_kb(&s).iter().any(|e| e.code == "K0007"), "{:?}", validate_kb(&s));
    }

    #[test]
    fn agent_policy_execution_decides_each_op() {
        use crate::abl_bridge::{eval_agent_policy, OpDecision};
        let caps = vec!["read_source".to_string(), "write_files".to_string()];
        let appr = vec!["write_files".to_string()];
        let ops = vec!["read_source".to_string(), "write_files".to_string(), "exfiltrate".to_string()];
        let d = eval_agent_policy(&caps, &appr, &ops);
        assert_eq!(d[0].1, OpDecision::Allowed);
        assert_eq!(d[1].1, OpDecision::RequiresApproval);
        assert_eq!(d[2].1, OpDecision::Denied);
    }

    #[test]
    fn swarm_consensus_execution_decides() {
        use crate::abl_bridge::eval_swarm_consensus as run;
        assert_eq!(run(3, "ring", "majority", &[1, 1, 2]).decided, Some(1), "plurality");
        assert_eq!(run(3, "mesh", "unanimous", &[1, 1, 1]).decided, Some(1), "all agree");
        assert_eq!(run(3, "mesh", "unanimous", &[1, 1, 2]).decided, None, "not unanimous");
        assert_eq!(run(3, "star", "quorum", &[1, 1, 2]).decided, Some(1), "2/3 quorum");
        assert_eq!(run(3, "star", "quorum", &[1, 2, 3]).decided, None, "no quorum");
        assert_eq!(run(4, "mesh", "majority", &[2, 2, 1, 1]).decided, Some(1), "tie → smallest");
    }

    #[test]
    fn swarm_rounds_match_topology_diameter() {
        use crate::abl_bridge::rounds_to_converge as r;
        assert_eq!(r("mesh", 5), 1);
        assert_eq!(r("broadcast", 9), 1);
        assert_eq!(r("ring", 5), 4);
        assert_eq!(r("tree", 8), 3);
        assert_eq!(r("tree", 1), 0);
    }

    #[test]
    fn agent_requires_approval_round_trips() {
        let s: AgentSpec = serde_json::from_str(
            r#"{"agent":"W","capabilities":["read"],"requires_approval":["write_files","deploy"]}"#,
        ).unwrap();
        assert!(validate_agent(&s).is_empty());
        let module = crate::parser::parse(&crate::lexer::lex(&to_mg_source_agent(&s))).expect("parses");
        let (blob, _) = crate::abl::encode_module(&module);
        let items = crate::abl::decode_container(&blob).unwrap();
        let syms = crate::abl::decode_symbols(&blob).unwrap();
        let ag = crate::abl_bridge::decompile_agentic(&items[0].expr).unwrap();
        let appr: Vec<&str> = ag.approval_syms.iter().map(|&id| syms[id as usize].as_str()).collect();
        assert_eq!(appr, vec!["write_files", "deploy"], "requires_approval round-trips");
    }

    #[test]
    fn swarm_topology_consensus_transport_round_trip() {
        let s: SwarmSpec = serde_json::from_str(
            r#"{"swarm":"W","agent":"Worker","size":4,"topology":"ring","consensus":"majority","transport":"rmi_quic"}"#,
        ).unwrap();
        assert!(validate_swarm(&s).is_empty(), "{:?}", validate_swarm(&s));
        let module = crate::parser::parse(&crate::lexer::lex(&to_mg_source_swarm(&s))).expect("parses");
        let (blob, _) = crate::abl::encode_module(&module);
        let items = crate::abl::decode_container(&blob).unwrap();
        let syms = crate::abl::decode_symbols(&blob).unwrap();
        let ag = crate::abl_bridge::decompile_agentic(&items[0].expr).unwrap();
        let name = |o: Option<u32>| o.map(|id| syms[id as usize].clone());
        assert_eq!(name(ag.topology_sym).as_deref(), Some("ring"), "exact topology round-trips");
        assert_eq!(name(ag.consensus_sym).as_deref(), Some("majority"), "consensus round-trips");
        assert_eq!(name(ag.transport_sym).as_deref(), Some("rmi_quic"), "transport round-trips");
        assert!(ag.cap_syms.is_empty(), "swarm has no capabilities");
    }

    #[test]
    fn swarm_rejects_unknown_consensus_and_bad_transport() {
        let s: SwarmSpec = serde_json::from_str(
            r#"{"swarm":"W","agent":"Worker","consensus":"dictator","transport":"http"}"#,
        ).unwrap();
        let e = validate_swarm(&s);
        assert!(e.iter().any(|x| x.code == "S0005"), "unknown consensus: {e:?}");
        assert!(e.iter().any(|x| x.code == "S0006"), "non-rmi transport: {e:?}");
    }

    #[test]
    fn agent_swarm_reject_by_construction() {
        let bad_agent: AgentSpec = serde_json::from_str(r#"{"agent":"2bad","capabilities":[""]}"#).unwrap();
        let e = validate_agent(&bad_agent);
        assert!(e.iter().any(|x| x.code == "A0001"));
        assert!(e.iter().any(|x| x.code == "A0002"));
        let bad_swarm: SwarmSpec = serde_json::from_str(r#"{"swarm":"W","agent":"Worker","size":0,"topology":"clique"}"#).unwrap();
        let e = validate_swarm(&bad_swarm);
        assert!(e.iter().any(|x| x.code == "S0003"), "non-positive size");
        assert!(e.iter().any(|x| x.code == "S0004"), "unknown topology");
    }

    #[test]
    fn schema_documents_agent_and_swarm() {
        let schema = build_schema();
        assert!(schema["agent"]["spec_format"]["capabilities"].is_string());
        assert!(schema["swarm"]["spec_format"]["topology"].is_string());
    }

    #[test]
    fn unified_can_mix_all_four_kinds() {
        let s = unifiedspec(
            r#"{"items":[
                {"net":"N","layers":[["a","ReLU",[]]]},
                {"kb":"K","facts":[["f",["x"]]],"rules":[]},
                {"agent":"Ag","capabilities":["c1"]},
                {"swarm":"Sw","agent":"Ag2","size":2,"topology":"mesh"}
            ]}"#,
        );
        assert!(validate_unified(&s).is_empty(), "{:?}", validate_unified(&s));
        let module = crate::parser::parse(&crate::lexer::lex(&to_mg_source_unified(&s))).expect("parses");
        let (_blob, summary) = crate::abl::encode_module(&module);
        assert_eq!(summary.len(), 4, "four mixed items → four container entries");
    }

    #[test]
    fn repair_net_fixes_shape_op_and_dims_to_valid() {
        // ReLu (typo) → ReLU; -3 dim → 1; fc2 input 4 → 8 (shape chain).
        let mut s = spec(r#"{"net":"M","layers":[["fc1","Linear",[3,8]],["a","ReLu",[]],["fc2","Linear",[4,-3]]]}"#);
        assert!(!validate(&s).is_empty(), "starts invalid");
        let fixes = repair_net(&mut s);
        assert!(fixes.iter().any(|f| f.contains("ReLU")), "op typo fixed: {fixes:?}");
        assert!(fixes.iter().any(|f| f.contains("→ 1")), "bad dim fixed: {fixes:?}");
        assert!(fixes.iter().any(|f| f.contains("shape chain")), "shape fixed: {fixes:?}");
        assert!(validate(&s).is_empty(), "repaired net must be valid: {:?}", validate(&s));
    }

    #[test]
    fn repair_swarm_snaps_enums_and_transport() {
        let mut s: SwarmSpec = serde_json::from_str(
            r#"{"swarm":"W","agent":"Worker","topology":"rng","consensus":"majorty","transport":"grpc"}"#,
        ).unwrap();
        assert!(!validate_swarm(&s).is_empty());
        let fixes = repair_swarm(&mut s);
        assert_eq!(s.topology.as_deref(), Some("ring"), "{fixes:?}");
        assert_eq!(s.consensus.as_deref(), Some("majority"));
        assert_eq!(s.transport.as_deref(), Some("rmi_quic"));
        assert!(validate_swarm(&s).is_empty(), "repaired swarm valid");
    }

    #[test]
    fn levenshtein_and_nearest_basic() {
        assert_eq!(levenshtein("ring", "rng"), 1);
        assert_eq!(nearest("ReLu", &["ReLU", "GELU", "Tanh"], 3), Some("ReLU"));
        assert_eq!(nearest("xyzzy", &["ReLU"], 2), None, "too far → no suggestion");
    }

    #[test]
    fn schema_lists_every_error_code_validate_can_emit() {
        // Every B-code the validator can produce must be documented in the
        // schema, so an agent that grounds in the schema understands any
        // rejection it receives. (B0000 is emitted by the CLI JSON-parse step.)
        let schema = build_schema();
        let codes: std::collections::HashSet<&str> = schema["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["code"].as_str().unwrap())
            .collect();
        for code in ["B0000", "B0001", "B0002", "B0003", "B0004", "B0005", "B0006"] {
            assert!(codes.contains(code), "schema is missing error {code}");
        }
    }

    /// The build codes quoted in `src`: a letter of B, K, A, S or U and four
    /// digits, between double quotes. Comments are read too.
    fn quoted_build_codes(src: &str) -> std::collections::BTreeSet<String> {
        src.split('"')
            .skip(1)
            .step_by(2)
            .filter(|s| {
                let c: Vec<char> = s.chars().collect();
                c.len() == 5 && "BKASU".contains(c[0]) && c[1..].iter().all(|d| d.is_ascii_digit())
            })
            .map(str::to_string)
            .collect()
    }

    /// The codes the knowledge base states: each `BuildRule` instance, and each
    /// `build_rule_row` (its first argument).
    fn stated_build_codes() -> (std::collections::BTreeSet<String>, std::collections::BTreeSet<String>) {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let classed = kb
            .facts("instance_of")
            .into_iter()
            .filter(|f| f[1] == "BuildRule")
            .map(|f| f[0].clone())
            .collect();
        let rowed = kb.facts("build_rule_row").into_iter().map(|f| f[0].clone()).collect();
        (classed, rowed)
    }

    /// Every build code is stated both ways. The codes this file quotes outside
    /// its tests are the `BuildRule` instances of kb/rules/builder.mg, and each
    /// has a `build_rule_row`; the codes main.rs quotes are among them.
    #[test]
    fn every_build_code_is_stated() {
        let source = include_str!("builder.rs");
        let production = source.split_once("#[cfg(test)]").map_or(source, |(p, _)| p);
        let quoted = quoted_build_codes(production);
        let (classed, rowed) = stated_build_codes();
        assert_eq!(classed, quoted, "BuildRule instances vs codes quoted in builder.rs");
        assert_eq!(rowed, quoted, "build_rule_row codes vs codes quoted in builder.rs");
        let cli = quoted_build_codes(include_str!("main.rs"));
        assert!(cli.is_subset(&quoted), "main.rs quotes codes not stated: {:?}", cli.difference(&quoted));
    }

    /// The errors `--build=abl` gives a spec: the kind is read from its key, as
    /// in main.rs; a spec that is not JSON, or does not fit its kind, is `B0000`.
    fn build_errors(json: &str) -> Vec<BuildError> {
        let value: serde_json::Value = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(e) => return vec![BuildError::malformed(format!("malformed spec JSON: {e}"))],
        };
        let bad = |kind: &str, e: serde_json::Error| vec![BuildError::malformed(format!("bad {kind} spec: {e}"))];
        if value.get("items").is_some() {
            serde_json::from_value::<UnifiedSpec>(value).map_or_else(|e| bad("unified", e), |s| validate_unified(&s))
        } else if value.get("kb").is_some() {
            serde_json::from_value::<KbSpec>(value).map_or_else(|e| bad("kb", e), |s| validate_kb(&s))
        } else if value.get("swarm").is_some() {
            serde_json::from_value::<SwarmSpec>(value).map_or_else(|e| bad("swarm", e), |s| validate_swarm(&s))
        } else if value.get("agent").is_some() {
            serde_json::from_value::<AgentSpec>(value).map_or_else(|e| bad("agent", e), |s| validate_agent(&s))
        } else {
            serde_json::from_value::<NetSpec>(value).map_or_else(|e| bad("net", e), |s| validate(&s))
        }
    }

    /// One spec per build code, each of which must be refused with that code.
    const BUILD_WITNESSES: &[(&str, &str)] = &[
        ("B0000", "{"),
        ("B0001", r#"{"net":"","layers":[["a","Linear",[1,1]]]}"#),
        ("B0002", r#"{"net":"M","layers":[]}"#),
        ("B0003", r#"{"net":"M","layers":[["x","Frobnicate",[3]]]}"#),
        ("B0004", r#"{"net":"M","layers":[["x","Linear",[3]]]}"#),
        ("B0005", r#"{"net":"M","layers":[["x","Linear",[0,8]]]}"#),
        ("B0006", r#"{"net":"M","layers":[["fc1","Linear",[3,8]],["fc2","Linear",[4,1]]]}"#),
        ("K0001", r#"{"kb":"","facts":[["p",["a"]]]}"#),
        ("K0002", r#"{"kb":"K"}"#),
        ("K0003", r#"{"kb":"K","facts":[["bad name",["a"]]]}"#),
        ("K0004", r#"{"kb":"K","facts":[["p",["a"]],["p",["a","b"]]]}"#),
        ("K0006", r#"{"kb":"K","facts":[["p",["a"]]],"rules":[["q",["x"],[["nope",["x"]]]]]}"#),
        ("K0007", r#"{"kb":"K","facts":[["p",["a"]]],"rules":[["q",["x","y"],[["p",["x"]]]]]}"#),
        ("K0008", r#"{"kb":"K","facts":[["p",["a"]]],"rules":[["q",[],[]]]}"#),
        ("A0001", r#"{"agent":"bad name"}"#),
        ("A0002", r#"{"agent":"Worker","capabilities":["bad cap"]}"#),
        ("A0003", r#"{"agent":"Worker","requires_approval":["bad op"]}"#),
        ("S0001", r#"{"swarm":"bad name","agent":"Worker"}"#),
        ("S0002", r#"{"swarm":"Workers","agent":"bad type"}"#),
        ("S0003", r#"{"swarm":"Workers","agent":"Worker","size":0}"#),
        ("S0004", r#"{"swarm":"Workers","agent":"Worker","topology":"blob"}"#),
        ("S0005", r#"{"swarm":"Workers","agent":"Worker","consensus":"vote"}"#),
        ("S0006", r#"{"swarm":"Workers","agent":"Worker","transport":"tcp"}"#),
        ("U0001", r#"{"items":[]}"#),
        ("U0002", r#"{"items":[{"foo":1}]}"#),
        ("U0003", r#"{"items":[{"agent":"W"},{"agent":"W"}]}"#),
    ];

    /// Every stated build code happens: each has a witness spec that the
    /// `--build=abl` steps refuse with that code, and the witnesses are the
    /// stated codes, no more and no fewer. No code is left out.
    #[test]
    fn every_build_rule_happens() {
        let (classed, rowed) = stated_build_codes();
        assert_eq!(classed, rowed, "BuildRule instances vs build_rule_row");
        let witnessed: std::collections::BTreeSet<String> = BUILD_WITNESSES.iter().map(|(c, _)| c.to_string()).collect();
        assert_eq!(witnessed.len(), BUILD_WITNESSES.len(), "a code has two witnesses");
        assert_eq!(witnessed, rowed, "witness codes vs stated codes");
        for (code, json) in BUILD_WITNESSES {
            let errs = build_errors(json);
            assert!(errs.iter().any(|e| e.code == *code), "{code} from {json} gives {errs:?}");
        }
    }

    /// The `fix` the knowledge base states for each build code.
    fn stated_fixes() -> std::collections::BTreeMap<String, String> {
        let kb = crate::kb::load().expect("the knowledge base loads");
        kb.facts("build_rule_row").into_iter().map(|f| (f[0].clone(), f[3].clone())).collect()
    }

    /// Every fix an error emits is the stated one, so what an agent reads in
    /// `--build=schema` is what it is told when refused. Each witness's emitted
    /// error with that code is checked, and so is every `BuildError::new` call
    /// site in this file's production code (its code and its last argument, a
    /// string literal), which covers codes emitted from more than one place.
    #[test]
    fn every_emitted_fix_is_the_stated_one() {
        let stated = stated_fixes();
        for (code, json) in BUILD_WITNESSES {
            let want = stated.get(*code).unwrap_or_else(|| panic!("{code} has no stated fix"));
            for e in build_errors(json).iter().filter(|e| e.code == *code) {
                assert_eq!(e.fix, want, "{code} from {json}: emitted fix vs stated fix");
            }
        }
        let source = include_str!("builder.rs");
        let production = source.split_once("#[cfg(test)]").map_or(source, |(p, _)| p);
        let mut sites = 0;
        for (at, _) in production.match_indices("BuildError::new(") {
            // The call's text, to its matching paren, with its string literals.
            let (mut depth, mut lits, mut end_of_last) = (0usize, Vec::<String>::new(), false);
            let mut chars = production[at + "BuildError::new".len()..].chars();
            while let Some(c) = chars.next() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    '"' => {
                        let mut lit = String::new();
                        while let Some(d) = chars.next() {
                            match d {
                                '\\' => lit.push(chars.next().expect("an escape has a character")),
                                '"' => break,
                                _ => lit.push(d),
                            }
                        }
                        lits.push(lit);
                        end_of_last = true;
                        continue;
                    }
                    c if c.is_whitespace() || c == ',' => continue,
                    _ => {}
                }
                end_of_last = false;
            }
            assert!(end_of_last, "a BuildError::new call does not end in a fix literal: {}", &production[at..at + 80]);
            let (code, fix) = (&lits[0], lits.last().unwrap());
            let want = stated.get(code).unwrap_or_else(|| panic!("{code} has no stated fix"));
            assert_eq!(fix, want, "{code}: a BuildError::new call site's fix vs the stated fix");
            sites += 1;
        }
        assert!(sites >= stated.len(), "found only {sites} BuildError::new call sites");
    }

    /// Every op of `OPS` is stated, and every stated op is in `OPS`: the
    /// `(name, arity, transforms, doc)` of each side, as multisets. Arity and
    /// transforms are text in the fact, spelled as `OPS` writes them.
    #[test]
    fn every_op_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let mut stated: Vec<Vec<String>> = kb.facts("build_op");
        stated.sort();
        let mut table: Vec<Vec<String>> = OPS
            .iter()
            .map(|o| vec![o.name.to_string(), o.arity.to_string(), o.transforms.to_string(), o.doc.to_string()])
            .collect();
        table.sort();
        assert_eq!(stated, table, "build_op facts vs OPS");
    }

    /// `TOPOLOGIES` and `CONSENSUS` are each stated, and each stated name is
    /// in its table, both ways.
    #[test]
    fn every_topology_and_consensus_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let names = |predicate: &str| -> Vec<String> {
            let mut v: Vec<String> = kb.facts(predicate).into_iter().map(|f| f[0].clone()).collect();
            v.sort();
            v
        };
        let sorted = |table: &[&str]| -> Vec<String> {
            let mut v: Vec<String> = table.iter().map(|t| t.to_string()).collect();
            v.sort();
            v
        };
        assert_eq!(names("build_topology"), sorted(TOPOLOGIES), "build_topology facts vs TOPOLOGIES");
        assert_eq!(names("build_consensus"), sorted(CONSENSUS), "build_consensus facts vs CONSENSUS");
    }

    // ── Property tests: the reliability surface of tool-mediated construction ──
    //
    // These verify the two guarantees the framework profile rests its
    // reliability claim on, over thousands of generated specs:
    //   (1) every structurally-valid spec lowers to clean-resolving,
    //       deterministically-constructed source (reject NOTHING valid), and
    //   (2) every structurally-invalid spec is rejected BY CONSTRUCTION with a
    //       machine-readable error and yields NO artifact (reject ALL invalid).
    // Together they are "reject-by-construction": no invalid net ever reaches
    // an artifact, and no valid net is ever spuriously refused.

    /// Tiny deterministic xorshift PRNG (no Date/rand; reproducible).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
        fn dim(&mut self) -> i64 {
            (self.below(16) + 1) as i64 // 1..=16, always positive
        }
    }

    /// Build a structurally VALID spec: a shape-consistent stack of layers.
    fn gen_valid(rng: &mut Rng) -> NetSpec {
        let acts = ["ReLU", "GELU", "Tanh", "Sigmoid", "Softmax", "Dropout"];
        let n_linear = rng.below(4) + 1; // 1..=4 Linear layers
        let mut layers = Vec::new();
        let mut running = rng.dim();
        for i in 0..n_linear {
            let out = rng.dim();
            layers.push(LayerSpec(format!("fc{i}"), "Linear".into(), vec![running, out]));
            running = out;
            // optional activation (0 dims) — never breaks the shape chain
            if rng.below(2) == 0 {
                let a = acts[rng.below(acts.len())];
                layers.push(LayerSpec(format!("a{i}"), a.into(), vec![]));
            }
        }
        NetSpec { net: format!("N{}", rng.below(1000)), layers }
    }

    #[test]
    fn valid_specs_always_lower_and_are_deterministic() {
        let mut rng = Rng(0x9E3779B97F4A7C15);
        for _ in 0..3000 {
            let s = gen_valid(&mut rng);
            let errs = validate(&s);
            assert!(errs.is_empty(), "valid spec spuriously rejected: {s:?} -> {errs:?}");
            // Construction is deterministic (byte-identical source).
            let a = to_mg_source(&s);
            let b = to_mg_source(&s);
            assert_eq!(a, b, "construction not deterministic");
            // And the constructed source resolves clean through the real pipeline.
            let toks = crate::lexer::lex(&a);
            let m = crate::parser::parse(&toks).expect("generated net must parse");
            let r = crate::resolve::resolve(&m);
            assert!(
                !r.diagnostics.iter().any(|d| d.severity == crate::hir::Severity::Error),
                "constructed source should resolve clean: {:?}",
                r.diagnostics
            );
        }
    }

    #[test]
    fn invalid_specs_are_always_rejected_by_construction() {
        let mut rng = Rng(0xD1B54A32D192ED03);
        // Each iteration takes a valid spec and injects exactly one fault
        // class, then asserts the matching error code fires. Because injection
        // is the only mutation, the expected code is known.
        for _ in 0..3000 {
            let mut s = gen_valid(&mut rng);
            let fault = rng.below(4);
            let expect = match fault {
                0 => {
                    // unknown op
                    let i = rng.below(s.layers.len());
                    s.layers[i].1 = "Frobnicate".into();
                    s.layers[i].2 = vec![]; // arity for the unknown op is irrelevant
                    "B0003"
                }
                1 => {
                    // wrong arity on a Linear (needs 2 dims)
                    let li = s.layers.iter().position(|l| l.1 == "Linear").unwrap_or(0);
                    s.layers[li].2 = vec![rng.dim()]; // 1 dim, needs 2
                    "B0004"
                }
                2 => {
                    // non-positive dim on a Linear
                    let li = s.layers.iter().position(|l| l.1 == "Linear").unwrap_or(0);
                    s.layers[li].2 = vec![0, rng.dim()];
                    "B0005"
                }
                _ => {
                    // shape mismatch: need ≥2 Linear layers to break the chain
                    let lins: Vec<usize> = s
                        .layers
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.1 == "Linear")
                        .map(|(i, _)| i)
                        .collect();
                    if lins.len() < 2 {
                        continue; // can't form a mismatch; skip this draw
                    }
                    let second = lins[1];
                    // make its input dim disagree with the running output
                    s.layers[second].2[0] += 100;
                    "B0006"
                }
            };
            let errs = validate(&s);
            assert!(
                errs.iter().any(|e| e.code == expect),
                "fault {fault} not rejected with {expect}: spec {s:?} -> {errs:?}"
            );
            // Reject-by-construction: every reported error carries a
            // machine-readable code + fix, so the agent can self-correct.
            for e in &errs {
                assert!(!e.code.is_empty() && !e.fix.is_empty(), "error not machine-actionable: {e:?}");
            }
        }
    }
}
