//! Tool-mediated construction layer.
//!
//! The agentic-first frontier (see IDEAL_AGENTIC_LANGUAGE.md): instead of an
//! agent *emitting source text* (token cost floored by identifiers/literals) or
//! *base64 bytes* (which erases the binary win on emission), the agent emits a
//! compact, schema-validated **structured spec**, and this layer constructs +
//! validates + lowers it to the deterministic, no-exec Machine Language artifact.
//!
//! Why this beats the text-token floor:
//! - **token**: the spec is positional/minimal JSON — no keywords, no syntax,
//!   only the irreducible payload (names, ops, dims).
//! - **reliability**: invalid specs are rejected *by construction* with
//!   machine-readable errors (unknown op, bad dims, shape mismatch) BEFORE any
//!   artifact exists — the typed-API reliability the design calls for.
//! - **determinism / safety**: inherited from the Machine Language artifact (byte-stable,
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
    serde_json::json!({
        "schema": "mechgen.ml.net-spec",
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
        "errors": [
            {"code": "B0000", "when": "spec JSON is malformed",                "fix": "emit valid JSON matching spec_format"},
            {"code": "B0001", "when": "net name is empty",                     "fix": "set a non-empty \"net\" field"},
            {"code": "B0002", "when": "net has no layers",                     "fix": "add at least one layer"},
            {"code": "B0003", "when": "unknown op",                            "fix": "use an op listed in `ops`"},
            {"code": "B0004", "when": "wrong dim count for the op",            "fix": "match the op's `dims` arity"},
            {"code": "B0005", "when": "a dim is not a positive integer",       "fix": "use positive integers for dims"},
            {"code": "B0006", "when": "shape mismatch in the layer chain",     "fix": "make each Linear's input dim equal the previous output dim"}
        ]
    })
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
                "use a known op (see `MechGen-parse --build=schema` for the catalog)",
            ));
            continue;
        };
        if dims.len() != arity {
            errs.push(BuildError::new(
                "B0004",
                format!("layer `{name}` (#{i}): op `{op}` takes {arity} dim(s), got {}", dims.len()),
                "match the op's dimension count",
            ));
            continue;
        }
        if dims.iter().any(|&d| d <= 0) {
            errs.push(BuildError::new(
                "B0005",
                format!("layer `{name}` (#{i}): dims must be positive, got {dims:?}"),
                "use positive integers for dimensions",
            ));
            continue;
        }
        // Shape-chain check: a Linear's input dim must match the running dim.
        if op == "Linear" {
            let in_dim = dims[0];
            if let Some(prev) = running {
                if prev != in_dim {
                    errs.push(BuildError::new(
                        "B0006",
                        format!(
                            "shape mismatch at `{name}` (#{i}): previous output is {prev}, `Linear` expects input {in_dim}"
                        ),
                        "make this layer's input dim equal the previous layer's output dim",
                    ));
                }
            }
        }
        if transforms {
            running = Some(*dims.last().unwrap());
        }
    }
    errs
}

/// Render the validated spec as canonical MechGen `net` source. (Internal — the
/// agent never writes or sees this; it's the bridge to the existing, tested
/// lexer→parser→Machine Language pipeline. Deterministic: fixed field order.)
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
        // The paradigm's end-to-end invariant: a valid spec lowers to Machine Language
        // bytes, those bytes are byte-stable, and decoding them as PURE DATA
        // (no execution) recovers the SAME op/dim structure the spec declared.
        // This is the "deterministic, introspectable, no-exec artifact" claim.
        let s = spec(
            r#"{"net":"M","layers":[["fc1","Linear",[4,16]],["a","ReLU",[]],["fc2","Linear",[16,3]]]}"#,
        );
        assert!(validate(&s).is_empty());
        let src = to_mg_source(&s);
        let module = crate::parser::parse(&crate::lexer::lex(&src)).expect("parses");

        let (blob, summary) = crate::machine::encode_module(&module);
        assert_eq!(summary.len(), 1, "one net → one item");
        // Byte-stable artifact (content-hashable cache key).
        let (blob2, _) = crate::machine::encode_module(&module);
        assert_eq!(blob, blob2, "Machine Language artifact is not byte-stable");

        // No-exec decode recovers the structure faithfully.
        let items = crate::machine::decode_container(&blob).expect("decode pure data");
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].expr.content_hash(),
            summary[0].2,
            "describe hash must equal encode hash"
        );
        let result = crate::machine_bridge::decompile(&items[0].expr, &items[0].name);
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
