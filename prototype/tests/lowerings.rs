//! Plan 3 (NEUROSYMBOLIC_PLAN.md, the `unaccounted_lowering` invariant): every
//! lowering the compiler performs either round-trips or says what it loses.
//! `kb/forms.mg` states those claims as `lowers_to`, `round_trips` and `loses`
//! facts. This file checks them against the real code, in both directions:
//!
//! - each `loses` fact has a witness here, and each witness is a stated loss.
//!   A witness is a small source that contains the lost thing, run through the
//!   real lowering. It returns true only when the thing is gone from the output
//!   and a control in the same source survives, so it cannot pass by breaking
//!   the lowering outright;
//! - each `round_trips` fact has an assertion here, on a real input;
//! - each `lowers_to` fact has one or the other.
//!
//! A claim with no check fails, and so does a check with no claim. The first
//! test compares the two lists against the knowledge base, the next two run
//! them. A witness shows a stated loss happens; the last test shows decompile
//! loses nothing else, over every net in the repository.
//!
//! The types half of `AstTree -> HirTables` is not observed: `TypeChecker`
//! exposes no per-node query. The witness reads the resolver and effect maps.

use mage_prototype::{abl, abl_bridge, ast, effects, fmt, kb, lexer, parser, resolve};
use rmi::lang::{Expr as RmilExpr, Op};
use serde_json::Value;
use std::collections::BTreeSet;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

// ── The tables ──────────────────────────────────────────────────────

/// Every `loses` fact of kb/forms.mg: (from, to, the loss verbatim, witness).
/// The witness returns true when the loss is observed.
const LOSSES: &[(&str, &str, &str, fn() -> bool)] = &[
    ("HumanText", "AstTree", "comments", human_comments_lost_from_ast),
    ("HumanText", "AstTree", "layout: blank lines, indentation, line breaks", human_layout_lost_from_ast),
    ("HumanText", "AstTree", "parentheses: no Expr node records them", human_parens_lost_from_ast),
    ("HumanText", "AstTree", "spans, except on contract clauses and spec items", human_spans_lost_from_ast),
    ("HumanText", "AstTree", "block definitions: a block reference is expanded in place and the name dropped", human_blocks_lost_from_ast),
    ("SigilText", "AstTree", "comments", sigil_comments_lost_from_ast),
    ("SigilText", "AstTree", "layout: blank lines, indentation, line breaks", sigil_layout_lost_from_ast),
    ("SigilText", "AstTree", "parentheses: no Expr node records them", sigil_parens_lost_from_ast),
    ("SigilText", "AstTree", "spans, except on contract clauses and spec items", sigil_spans_lost_from_ast),
    ("SigilText", "AstTree", "block definitions: a block reference is expanded in place and the name dropped", sigil_blocks_lost_from_ast),
    ("SigilText", "HumanText", "comments", sigil_comments_lost_from_print),
    ("SigilText", "HumanText", "layout: the formatter's own is printed", sigil_layout_lost_from_print),
    ("SigilText", "HumanText", "parentheses the precedence does not need", sigil_parens_lost_from_print),
    ("HumanText", "SigilText", "comments", human_comments_lost_from_print),
    ("HumanText", "SigilText", "layout: the formatter's own is printed", human_layout_lost_from_print),
    ("HumanText", "SigilText", "parentheses the precedence does not need", human_parens_lost_from_print),
    ("AstTree", "HirTables", "the tree: types and effects are kept per name, not per node", ast_hir_name_keyed),
    ("NetDecl", "RmilExpr", "layer names: each layer becomes its op", net_layer_names_lost),
    ("NetDecl", "RmilExpr", "generics", net_generics_lost),
    ("NetDecl", "RmilExpr", "the forward block: only the walked result is kept", net_forward_lost),
    ("NetDecl", "RmilExpr", "a composition, when the forward block reaches every declared layer: the walked result wins", net_composition_lost),
    ("NetDecl", "RmilExpr", "layer arguments that are not literals, silently", net_layer_args_lost),
    ("NetDecl", "RmilExpr", "an unknown layer type: it lowers to IDENTITY, with a diagnostic", net_unknown_layer_lost),
    ("KbDecl", "RmilExpr", "a fact with a non-constant argument: left out and named", kb_fact_lost),
    ("KbDecl", "RmilExpr", "a rule with a comparison, negation or constant: left out and named", kb_rule_lost),
    ("AgentDecl", "RmilExpr", "the agent body: only the name, capabilities and approvals are lowered", agent_body_lost),
    ("SwarmDecl", "RmilExpr", "a size that is not a literal: it becomes 1", swarm_size_lost),
    ("SwarmDecl", "RmilExpr", "an unknown topology: only RECV is emitted", swarm_topology_lost),
    ("SwarmDecl", "RmilExpr", "on_dispatch, on_aggregate and on_failure: never read", swarm_callbacks_lost),
    ("TrainDecl", "RmilExpr", "optimizer, loss and epochs: the lowering is fixed; main.rs reads them from the AST", train_hyperparams_lost),
    ("EvolveDecl", "RmilExpr", "genome, fitness, mutate, crossover and select", evolve_fields_lost),
    ("AblBytes", "NetDecl", "layer names: regenerated as l_<type>_<n>", decompile_names_lost),
    ("AblBytes", "NetDecl", "the forward block, generics and composition: left empty", decompile_forward_lost),
    ("AblBytes", "NetDecl", "RES_ADD: skipped without being recorded", decompile_res_add_lost),
    ("AblBytes", "NetDecl", "branches: a Par is flattened into one layer list", decompile_branch_lost),
    ("AblBytes", "NetDecl", "layer arguments that are not literals", decompile_args_lost),
    ("AblBytes", "NetDecl", "IDENTITY: skipped without being recorded, so an unknown layer vanishes", decompile_identity_lost),
    ("AblBytes", "NetDecl", "ops with no layer name: recorded in DecompileResult::skipped", decompile_skipped_lost),
];

/// Every `round_trips` fact of kb/forms.mg: (from, to, the check).
const ROUND_TRIPS: &[(&str, &str, fn())] = &[
    ("AstTree", "HumanText", rt_ast_human),
    ("AstTree", "SigilText", rt_ast_sigil),
    ("AstTree", "NetDecl", rt_ast_netdecl),
    ("RmilExpr", "AblBytes", rt_rmil_ablbytes),
];

// ── Helpers ─────────────────────────────────────────────────────────

fn parse(src: &str) -> ast::Module {
    parser::parse(&lexer::lex(src)).unwrap_or_else(|e| panic!("does not parse: {e:?}\n{src}"))
}

/// The one item of a source that holds exactly one.
fn kind_of(src: &str) -> ast::ItemKind {
    let mut m = parse(src);
    assert_eq!(m.items.len(), 1, "one item expected from:\n{src}");
    m.items.remove(0).kind
}

fn net_of(src: &str) -> ast::NetDef {
    match kind_of(src) {
        ast::ItemKind::Net(n) => n,
        _ => panic!("not a net:\n{src}"),
    }
}

fn raw(m: &ast::Module) -> Value {
    serde_json::to_value(m).unwrap()
}

/// Replace every source span with `null`. A span is where a node sits in the
/// text, so a reprint that moves it changes the JSON without changing the AST.
fn without_spans(v: &mut Value) {
    match v {
        Value::Object(map) => {
            let is_span = map.len() == 4
                && ["col", "len", "line", "offset"].iter().all(|k| map.contains_key(*k));
            if is_span {
                *v = Value::Null;
            } else {
                map.values_mut().for_each(without_spans);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(without_spans),
        _ => {}
    }
}

fn spanless(m: &ast::Module) -> Value {
    let mut v = raw(m);
    without_spans(&mut v);
    v
}

fn mentions(v: &Value, s: &str) -> bool {
    v.to_string().contains(s)
}

/// Two sources parse to the same AST (spans aside), and the control name
/// survives in it.
fn same_ast_with_control(with: &str, without: &str, control: &str) -> bool {
    let a = spanless(&parse(with));
    a == spanless(&parse(without)) && mentions(&a, control)
}

/// Two sources print the same, and the control text is in the print.
fn same_print_with_control(
    mode: fn(&ast::Module) -> String,
    with: &str,
    without: &str,
    control: &str,
) -> bool {
    let a = mode(&parse(with));
    a == mode(&parse(without)) && a.contains(control)
}

/// Spans are absent from a plain item (shifting its text leaves the raw JSON
/// unchanged, and no `offset` key appears), and present on a contract clause
/// (shifting its text changes the raw JSON).
fn spans_only_on_contracts(plain: &str, contract: &str) -> bool {
    let shift = |s: &str| format!("\n\n\n   {s}");
    let p = raw(&parse(plain)).to_string();
    let c = raw(&parse(contract)).to_string();
    p == raw(&parse(&shift(plain))).to_string()
        && !p.contains("\"offset\"")
        && c != raw(&parse(&shift(contract))).to_string()
        && c.contains("\"offset\"")
}

/// A block definition is expanded in place: the module holds the net and the
/// item, the expansion's layer names and substituted literal are present, and
/// the block's own name is not.
fn block_is_expanded(item: &str) -> bool {
    let src = r#"block BlkZeta(d, ff) {
    layer attn: Attention(d, 4);
    layer ff1: Linear(d, ff);
}
net GPT {
    stack 3 { BlkZeta(64, 256) }
    forward { attn_0 }
}
ITEM
"#
    .replace("ITEM", item);
    let m = parse(&src);
    let j = raw(&m).to_string();
    m.items.len() == 2
        && j.contains("attn_0")
        && j.contains("ff1_0")
        && j.contains("\"64\"")
        && j.contains("keep_me")
        && !j.contains("BlkZeta")
}

/// Lower a source and return its single item's expression.
fn lowered(src: &str) -> RmilExpr {
    let m = abl_bridge::lower_module(&parse(src));
    assert_eq!(m.items.len(), 1, "one item expected from:\n{src}");
    m.items.into_iter().next().unwrap().1
}

fn lower(src: &str) -> abl_bridge::AblModule {
    abl_bridge::lower_module(&parse(src))
}

fn has_par(e: &RmilExpr) -> bool {
    match e {
        RmilExpr::Par(..) => true,
        RmilExpr::Seq(a, b) => has_par(a) || has_par(b),
        RmilExpr::App(_, args) => args.iter().any(has_par),
        _ => false,
    }
}

/// Encode a source to the binary container and decode it again.
fn container(src: &str) -> Vec<abl::AblItem> {
    let (blob, _) = abl::encode_module(&parse(src));
    abl::decode_container(&blob).unwrap_or_else(|e| panic!("does not decode: {e}"))
}

/// The net a source's container decompiles to, and the ops it skipped.
fn decompiled(src: &str) -> abl_bridge::DecompileResult {
    let items = container(src);
    assert_eq!(items.len(), 1, "one item expected from:\n{src}");
    abl_bridge::decompile(&items[0].expr, &items[0].name)
}

fn layer_names(net: &ast::NetDef) -> Vec<String> {
    net.layers.iter().map(|l| l.name.clone()).collect()
}

fn catch<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    panic::catch_unwind(AssertUnwindSafe(f)).map_err(|p| {
        p.downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "panicked".to_string())
    })
}

fn triples(rows: Vec<Vec<String>>, what: &str) -> BTreeSet<(String, String, String)> {
    rows.into_iter()
        .map(|r| {
            assert_eq!(r.len(), 3, "{what} fact with {} arguments", r.len());
            (r[0].clone(), r[1].clone(), r[2].clone())
        })
        .collect()
}

fn pairs(rows: Vec<Vec<String>>, what: &str) -> BTreeSet<(String, String)> {
    rows.into_iter()
        .map(|r| {
            assert_eq!(r.len(), 2, "{what} fact with {} arguments", r.len());
            (r[0].clone(), r[1].clone())
        })
        .collect()
}

fn describe<T: std::fmt::Debug>(set: &BTreeSet<T>) -> String {
    set.iter().map(|x| format!("    {x:?}")).collect::<Vec<_>>().join("\n")
}

// ── Witnesses: AST (source and source-to-source) ────────────────────

fn human_comments_lost_from_ast() -> bool {
    same_ast_with_control(
        "fn keep_me(x: i32) -> i32 {\n    // MARKER_ZQ\n    x\n}\n",
        "fn keep_me(x: i32) -> i32 {\n    x\n}\n",
        "keep_me",
    ) && !raw(&parse("fn keep_me(x: i32) -> i32 {\n    // MARKER_ZQ\n    x\n}\n"))
        .to_string()
        .contains("MARKER_ZQ")
}

fn human_layout_lost_from_ast() -> bool {
    same_ast_with_control(
        "pub fn keep_me(x: i32) -> i32 {\n\n\n        val y = x;\n\n\n    x\n}\n",
        "pub fn keep_me(x: i32) -> i32 { val y = x; x }",
        "keep_me",
    )
}

fn human_parens_lost_from_ast() -> bool {
    same_ast_with_control(
        "fn keep_me(x: i32) -> i32 { (x) }",
        "fn keep_me(x: i32) -> i32 { x }",
        "keep_me",
    )
}

fn human_spans_lost_from_ast() -> bool {
    spans_only_on_contracts(
        "fn keep_me(x: i32) -> i32 { x }",
        "@req(x > 0)\nfn keep_me(x: i32) -> i32 { x }",
    )
}

fn human_blocks_lost_from_ast() -> bool {
    block_is_expanded("fn keep_me(x: i32) -> i32 { x }")
}

fn sigil_comments_lost_from_ast() -> bool {
    same_ast_with_control(
        "f keep_me(x: i32) -> i32 {\n    // MARKER_ZQ\n    x\n}\n",
        "f keep_me(x: i32) -> i32 {\n    x\n}\n",
        "keep_me",
    ) && !raw(&parse("f keep_me(x: i32) -> i32 {\n    // MARKER_ZQ\n    x\n}\n"))
        .to_string()
        .contains("MARKER_ZQ")
}

fn sigil_layout_lost_from_ast() -> bool {
    same_ast_with_control(
        "f keep_me(x: i32) -> i32 {\n\n\n      v y = x;\n\n    x\n}\n",
        "f keep_me(x: i32) -> i32 { v y = x; x }",
        "keep_me",
    )
}

fn sigil_parens_lost_from_ast() -> bool {
    same_ast_with_control(
        "f keep_me(x: i32) -> i32 { (x) }",
        "f keep_me(x: i32) -> i32 { x }",
        "keep_me",
    )
}

fn sigil_spans_lost_from_ast() -> bool {
    spans_only_on_contracts(
        "f keep_me(x: i32) -> i32 { x }",
        "@req(x > 0)\nf keep_me(x: i32) -> i32 { x }",
    )
}

fn sigil_blocks_lost_from_ast() -> bool {
    block_is_expanded("f keep_me(x: i32) -> i32 { x }")
}

// ── Witnesses: the formatter (parse, then print in the other mode) ──

fn sigil_comments_lost_from_print() -> bool {
    let with = "f keep_me(x: i32) -> i32 {\n    // MARKER_ZQ\n    x\n}\n";
    let without = "f keep_me(x: i32) -> i32 {\n    x\n}\n";
    same_print_with_control(fmt::format_human, with, without, "fn keep_me(")
        && !fmt::format_human(&parse(with)).contains("MARKER_ZQ")
}

fn sigil_layout_lost_from_print() -> bool {
    same_print_with_control(
        fmt::format_human,
        "f keep_me(x: i32) -> i32 {\n\n\n      v y = x;\n\n    x\n}\n",
        "f keep_me(x: i32) -> i32 { v y = x; x }",
        "fn keep_me(",
    )
}

fn sigil_parens_lost_from_print() -> bool {
    same_print_with_control(
        fmt::format_human,
        "f keep_me(x: i32) -> i32 { (x * 2) + 1 }",
        "f keep_me(x: i32) -> i32 { x * 2 + 1 }",
        "keep_me(",
    )
}

fn human_comments_lost_from_print() -> bool {
    let with = "pub fn keep_me(x: i32) -> i32 {\n    // MARKER_ZQ\n    x\n}\n";
    let without = "pub fn keep_me(x: i32) -> i32 {\n    x\n}\n";
    same_print_with_control(fmt::format_agent, with, without, "keep_me(")
        && !fmt::format_agent(&parse(with)).contains("MARKER_ZQ")
}

fn human_layout_lost_from_print() -> bool {
    same_print_with_control(
        fmt::format_agent,
        "pub fn keep_me(x: i32) -> i32 {\n\n\n        val y = x;\n\n\n    x\n}\n",
        "pub fn keep_me(x: i32) -> i32 { val y = x; x }",
        "keep_me(",
    )
}

fn human_parens_lost_from_print() -> bool {
    same_print_with_control(
        fmt::format_agent,
        "pub fn keep_me(x: i32) -> i32 { (x * 2) + 1 }",
        "pub fn keep_me(x: i32) -> i32 { x * 2 + 1 }",
        "keep_me(",
    )
}

// ── Witnesses: AST to HIR ───────────────────────────────────────────

/// The resolver and the effect pass are keyed by name: two `x` occurrences in
/// two functions (one a local, one a parameter) leave one `x` entry and no
/// per-occurrence record, and the effect map is keyed by function name only.
/// The types are checked into `TypeChecker`, which exposes no per-node query,
/// so only the resolver and effect maps are observed here.
fn ast_hir_name_keyed() -> bool {
    let src = "f keep_me(x: i32) -> i32 {\n    v x = 1;\n    x\n}\nf other(x: i32) -> i32 {\n    x\n}\n";
    let m = parse(src);
    let occurrences = raw(&m).to_string().matches("\"name\":\"x\"").count();
    let resolved = resolve::resolve(&m);
    let mut fns: Vec<String> = effects::infer_effects(&m).inferred.keys().cloned().collect();
    fns.sort();
    occurrences >= 3 && resolved.resolved.contains_key("x") && fns == ["keep_me", "other"]
}

// ── Witnesses: net declarations to RMIL ─────────────────────────────

fn net_layer_names_lost() -> bool {
    let a = "net N {\n    layer alpha: Linear(4, 4);\n    layer beta: ReLU;\n    forward { alpha }\n}\n";
    let b = "net N {\n    layer gamma: Linear(4, 4);\n    layer delta: ReLU;\n    forward { gamma }\n}\n";
    let c = "net N {\n    layer alpha: Linear(4, 4);\n    layer beta: Sigmoid;\n    forward { alpha }\n}\n";
    let ea = lowered(a);
    ea == lowered(b) && ea != lowered(c)
}

fn net_generics_lost() -> bool {
    let a = "net N<T> {\n    layer a1: Linear(4, 4);\n    forward { a1 }\n}\n";
    let b = "net N {\n    layer a1: Linear(4, 4);\n    forward { a1 }\n}\n";
    let c = "net N {\n    layer a1: Linear(4, 8);\n    forward { a1 }\n}\n";
    !net_of(a).generics.is_empty()
        && net_of(b).generics.is_empty()
        && lowered(a) == lowered(b)
        && lowered(a) != lowered(c)
}

fn net_forward_lost() -> bool {
    let a = "net N {\n    layer a1: Linear(4, 4);\n    layer a2: ReLU;\n    forward { a1 }\n}\n";
    let b = "net N {\n    layer a1: Linear(4, 4);\n    layer a2: ReLU;\n    forward { val k = 3; a1 }\n}\n";
    let c = "net N {\n    layer a1: Linear(4, 4);\n    layer a2: Sigmoid;\n    forward { a1 }\n}\n";
    net_of(b).forward.stmts.len() == 1
        && lowered(a) == lowered(b)
        && lowered(a) != lowered(c)
}

/// With `residual` around the only layer, `forward` reaches every layer, so
/// the walked result is kept and no RES_ADD is emitted. With two layers it
/// reaches fewer, and the composition drives the lowering.
fn net_composition_lost() -> bool {
    let one = "net N {\n    residual {\n        layer fc1: Linear(4, 4);\n    }\n    forward { fc1 }\n}\n";
    let flat = "net N {\n    layer fc1: Linear(4, 4);\n    forward { fc1 }\n}\n";
    let two = "net N {\n    residual {\n        layer fc1: Linear(4, 4);\n        layer fc2: ReLU;\n    }\n    forward { fc1 }\n}\n";
    net_of(one).composition.is_some()
        && lowered(one) == lowered(flat)
        && lowered(two).opcodes().contains(&Op::RES_ADD)
}

fn net_layer_args_lost() -> bool {
    let a = "net N {\n    layer a1: Linear(n, 4);\n    forward { a1 }\n}\n";
    let b = "net N {\n    layer a1: Linear(4);\n    forward { a1 }\n}\n";
    let c = "net N {\n    layer a1: Linear(4, 4);\n    forward { a1 }\n}\n";
    lower(a).diagnostics.is_empty()
        && lower(b).diagnostics.is_empty()
        && lowered(a) == lowered(b)
        && lowered(a) != lowered(c)
}

fn net_unknown_layer_lost() -> bool {
    let a = "net N {\n    layer a1: Zorp(4);\n    forward { a1 }\n}\n";
    let b = "net N {\n    layer a1: Yorp(4);\n    forward { a1 }\n}\n";
    let c = "net N {\n    layer a1: Linear(4);\n    forward { a1 }\n}\n";
    let diag = lower(a).diagnostics;
    diag.iter().any(|d| d.contains("unknown layer type `Zorp`"))
        && lowered(a).opcodes().contains(&Op::IDENTITY)
        && lowered(a) == lowered(b)
        && lowered(a) != lowered(c)
}

// ── Witnesses: kb, agent, swarm, train, evolve ──────────────────────

fn kb_fact_lost() -> bool {
    let a = "kb K {\n    fact p(1);\n    fact q(1 + 2);\n}\n";
    let b = "kb K {\n    fact p(1);\n}\n";
    let c = "kb K {\n    fact p(1);\n    fact r(2);\n}\n";
    let ma = lower(a);
    ma.diagnostics.iter().any(|d| d.contains("fact `q` left out"))
        && ma.items[0].1 == lowered(b)
        && lowered(b) != lowered(c)
}

fn kb_rule_lost() -> bool {
    let base = "kb K {\n    fact p(1);\n}\n";
    let refused = [
        "kb K {\n    fact p(1);\n    rule r(x: i32) { p(x) && x == 1 }\n}\n",
        "kb K {\n    fact p(1);\n    rule r(x: i32) { p(x) && !p(x) }\n}\n",
        "kb K {\n    fact p(1);\n    rule r(x: i32) { p(x, 3) }\n}\n",
    ];
    let kept = "kb K {\n    fact p(1);\n    rule r(x: i32) { p(x) }\n}\n";
    refused.iter().all(|src| {
        let m = lower(src);
        m.diagnostics.iter().any(|d| d.contains("rule `r` left out")) && m.items[0].1 == lowered(base)
    }) && lowered(kept) != lowered(base)
}

fn agent_body_lost() -> bool {
    // The AST has no body: an AgentDef is a name, capabilities and approvals.
    // The parser rejects any other field, so the lowering can only see these.
    let src = "agent A {\n    capabilities: [read_source, query_types]\n    requires_approval: [write_source]\n}\n";
    let def = match kind_of(src) {
        ast::ItemKind::Agent(a) => a,
        _ => panic!("not an agent"),
    };
    let mut keys: Vec<String> = serde_json::to_value(&def)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    keys.sort();
    let ops = lowered(src).opcodes();
    let other = "agent A {\n    capabilities: [read_source]\n}\n";
    keys == ["capabilities", "name", "requires_approval"]
        && parser::parse(&lexer::lex("agent A { brain: x }")).is_err()
        && ops.iter().all(|o| *o == Op::SPAWN || *o == Op::DELEGATE)
        && ops.contains(&Op::SPAWN)
        && ops.contains(&Op::DELEGATE)
        && lowered(src) != lowered(other)
}

fn swarm_size_lost() -> bool {
    let a = "swarm S {\n    agent: Critic;\n    size: n;\n}\n";
    let b = "swarm S {\n    agent: Critic;\n    size: 1;\n}\n";
    let c = "swarm S {\n    agent: Critic;\n    size: 3;\n}\n";
    lowered(a) == lowered(b) && lowered(a) != lowered(c)
}

fn swarm_topology_lost() -> bool {
    let a = "swarm S {\n    agent: Critic;\n    topology: zigzag;\n}\n";
    let ring = "swarm S {\n    agent: Critic;\n    topology: ring;\n}\n";
    let comm = |e: &RmilExpr| -> Vec<Op> {
        e.opcodes().into_iter().filter(|o| *o == Op::SEND || *o == Op::RECV).collect()
    };
    comm(&lowered(a)) == vec![Op::RECV] && comm(&lowered(ring)).contains(&Op::SEND)
}

fn swarm_callbacks_lost() -> bool {
    let src = "swarm S {\n    agent: Critic;\n    size: 2;\n}\n";
    let base = lowered(src);
    let mut m = parse(src);
    let blk = |name: &str| ast::Block {
        stmts: Vec::new(),
        tail_expr: Some(Box::new(ast::Expr::Ident { name: name.to_string() })),
    };
    if let ast::ItemKind::Swarm(s) = &mut m.items[0].kind {
        s.on_dispatch = Some(blk("dispatch_zz"));
        s.on_aggregate = Some(blk("aggregate_zz"));
        s.on_failure = Some(blk("failure_zz"));
    } else {
        return false;
    }
    let set = match &m.items[0].kind {
        ast::ItemKind::Swarm(s) => s.on_dispatch.is_some() && s.on_aggregate.is_some() && s.on_failure.is_some(),
        _ => false,
    };
    let with_blocks = abl_bridge::lower_module(&m).items[0].1.clone();
    set && with_blocks == base
}

fn train_hyperparams_lost() -> bool {
    let a = "train T {\n    net: N;\n    optimizer: SGD(0.05);\n    loss: MSE;\n    epochs: 10;\n}\n";
    let b = "train T {\n    net: N;\n    optimizer: Adam(0.1);\n    loss: CrossEntropy;\n    epochs: 20;\n}\n";
    let other = "evolve E {\n    genome: i32;\n    fitness { 1 }\n}\n";
    lowered(a) == lowered(b) && lowered(a) != lowered(other)
}

fn evolve_fields_lost() -> bool {
    let a = "evolve E {\n    genome: i32;\n    population: 10;\n    generations: 5;\n    fitness { 1 }\n    mutate { 2 }\n    crossover { 3 }\n    select { 4 }\n}\n";
    let b = "evolve E {\n    genome: f64;\n    population: 10;\n    generations: 5;\n    fitness { 7 }\n    mutate { 8 }\n    crossover { 9 }\n    select { 10 }\n}\n";
    let ops = lowered(a).opcodes();
    lowered(a) == lowered(b) && ops.contains(&Op::MAP) && ops.contains(&Op::REDUCE)
}

// ── Witnesses: the binary container back to a net ───────────────────

fn decompile_names_lost() -> bool {
    let src = "net N {\n    layer fc1: Linear(4, 4);\n    layer fc2: ReLU;\n    forward { fc1 }\n}\n";
    let net = decompiled(src).net;
    layer_names(&net) == ["l_linear_1", "l_relu_2"]
        && !layer_names(&net).iter().any(|n| n.contains("fc"))
}

fn decompile_forward_lost() -> bool {
    let generic = "net N<T> {\n    layer fc1: Linear(4, 4);\n    forward { fc1 }\n}\n";
    let composed = "net N {\n    residual {\n        layer fc1: Linear(4, 4);\n    }\n    forward { fc1 }\n}\n";
    let g = net_of(generic);
    let c = net_of(composed);
    let back = decompiled(generic).net;
    let back_c = decompiled(composed).net;
    !g.generics.is_empty()
        && g.forward.tail_expr.is_some()
        && c.composition.is_some()
        && back.generics.is_empty()
        && back.forward.stmts.is_empty()
        && back.forward.tail_expr.is_none()
        && back_c.composition.is_none()
}

fn decompile_res_add_lost() -> bool {
    let src = "net R {\n    residual {\n        layer fc1: Linear(4, 4);\n        layer fc2: ReLU;\n    }\n    forward { fc1 }\n}\n";
    let expr = lowered(src);
    let items = container(src);
    let skipped = decompiled(src).skipped;
    expr.opcodes().contains(&Op::RES_ADD)
        && items[0].expr.opcodes().contains(&Op::RES_ADD)
        && !skipped.contains(&Op::RES_ADD)
}

fn decompile_branch_lost() -> bool {
    let branch = "net B {\n    branch {\n        layer fc1: Linear(4, 4);\n    } {\n        layer fc2: ReLU;\n    }\n}\n";
    let seq = "net B {\n    layer fc1: Linear(4, 4);\n    layer fc2: ReLU;\n}\n";
    let b = decompiled(branch).net;
    let s = decompiled(seq).net;
    has_par(&lowered(branch))
        && !has_par(&lowered(seq))
        && net_of(branch).composition.is_some()
        && b.composition.is_none()
        && layer_names(&b) == layer_names(&s)
        && b.layers.len() == 2
}

fn decompile_args_lost() -> bool {
    let a = "net N {\n    layer a1: Linear(n, 4);\n    forward { a1 }\n}\n";
    let full = "net N {\n    layer a1: Linear(4, 4);\n    forward { a1 }\n}\n";
    let original = net_of(a).layers[0].args.len();
    let back_a = decompiled(a).net.layers[0].args.clone();
    let back_full = decompiled(full).net.layers[0].args.len();
    let literal_four = matches!(
        back_a.first(),
        Some(ast::Expr::Literal { value, .. }) if value == "4"
    );
    original == 2 && back_a.len() == 1 && literal_four && back_full == 2
}

/// A layer of an unknown type lowers to IDENTITY, and decompile drops it with
/// no record; a known layer beside it survives.
fn decompile_identity_lost() -> bool {
    let src = "net N {\n    layer a1: Zorp(4);\n    layer a2: ReLU;\n}\n";
    let d = decompiled(src);
    lowered(src).opcodes().contains(&Op::IDENTITY)
        && layer_names(&d.net) == ["l_relu_1"]
        && d.skipped.is_empty()
}

/// Every op with no layer name is named in `skipped` and makes no layer. No
/// lowering emits one today, so the expression is built by hand. A kb's
/// RESOLVE and an agent's SPAWN have layer names, so they become layers.
fn decompile_skipped_lost() -> bool {
    let nameless: Vec<Op> = Op::ALL
        .iter()
        .copied()
        .filter(|o| abl_bridge::op_to_layer_name(*o).is_none() && *o != Op::IDENTITY && *o != Op::RES_ADD)
        .collect();
    !nameless.is_empty()
        && nameless.iter().all(|o| {
            let d = abl_bridge::decompile(&RmilExpr::op1(*o), "N");
            d.skipped == vec![*o] && d.net.layers.is_empty()
        })
        && decompiled("kb K {\n    fact p(1);\n}\n").skipped.is_empty()
        && decompiled("agent A {\n    capabilities: [read_source]\n}\n").skipped.is_empty()
}

// ── Round trips ─────────────────────────────────────────────────────

const ROUND_TRIP_SRC: &str = "net N {
    layer fc1: Linear(4, 4);
    layer fc2: ReLU;
    forward { fc1 }
}
fn keep_me(x: i32) -> i32 { x }
S Point { x: i32, y: i32 }
";

fn rt_ast_human() {
    let m = parse(ROUND_TRIP_SRC);
    let printed = fmt::format_human(&m);
    assert!(printed.contains("fn keep_me("), "human print has no fn:\n{printed}");
    let again = parse(&printed);
    assert_eq!(
        spanless(&m),
        spanless(&again),
        "human print reparses to a different AST:\n{printed}"
    );
}

fn rt_ast_sigil() {
    let m = parse(ROUND_TRIP_SRC);
    let printed = fmt::format_agent(&m);
    assert!(printed.contains("keep_me("), "agent print has no fn:\n{printed}");
    let again = parse(&printed);
    assert_eq!(
        spanless(&m),
        spanless(&again),
        "agent print reparses to a different AST:\n{printed}"
    );
}

fn rt_ast_netdecl() {
    let src = "net Mlp {\n    layer fc1: Linear(784, 128);\n    layer fc2: ReLU;\n    layer fc3: Linear(128, 10);\n    forward { fc3 }\n}\n";
    let m = parse(src);
    assert_eq!(m.items.len(), 1);
    let nd = match &m.items[0].kind {
        ast::ItemKind::Net(nd) => nd,
        other => panic!("the item is not a net: {other:?}"),
    };
    assert_eq!(nd.name, "Mlp");
    assert_eq!(layer_names(nd), ["fc1", "fc2", "fc3"]);
    assert!(nd.forward.tail_expr.is_some(), "the forward block is empty");
    let reprinted = net_of(&fmt::format_human(&m));
    assert_eq!(
        without_spans_of_net(nd),
        without_spans_of_net(&reprinted),
        "the net does not come back from its own print"
    );
}

fn without_spans_of_net(nd: &ast::NetDef) -> Value {
    let mut v = serde_json::to_value(nd).unwrap();
    without_spans(&mut v);
    v
}

fn rt_rmil_ablbytes() {
    let src = "net Mlp {\n    layer fc1: Linear(784, 128);\n    layer fc2: ReLU;\n    forward { fc1 }\n}\nkb Animals {\n    fact dog(1);\n    fact cat(2);\n}\n";
    let m = parse(src);
    let lowered = abl_bridge::lower_module(&m);
    let (blob, _) = abl::encode_module(&m);
    let items = abl::decode_container(&blob).expect("decodes");
    assert_eq!(items.len(), lowered.items.len(), "item count");
    for (got, (name, expr)) in items.iter().zip(&lowered.items) {
        assert_eq!(&got.name, name, "decoded name");
        assert_eq!(&got.expr, expr, "decoded expr for `{name}`");
    }
}

// ── The tests ───────────────────────────────────────────────────────

fn sources(root: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(root).unwrap().flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_dir() {
            if !name.starts_with('.') && name != "target" && name != "node_modules" {
                sources(&p, out);
            }
        } else if name.ends_with(".mg") {
            out.push(p);
        }
    }
}

/// "Nothing else": for every net in the repository, decompiling its lowering
/// and lowering the result gives the same expression, unless a stated
/// decompile loss applies (RES_ADD, a `Par`, or an op with no layer name). A
/// loss nobody stated shows up here as a net that changes.
#[test]
fn decompile_loses_nothing_it_does_not_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let mut files = Vec::new();
    sources(root, &mut files);
    let (mut nets, mut exact) = (0, 0);
    let mut changed = Vec::new();
    for path in &files {
        let Ok(src) = std::fs::read_to_string(path) else { continue };
        let Ok(module) = parser::parse(&lexer::lex(&src)) else { continue };
        for item in &module.items {
            let ast::ItemKind::Net(net) = &item.kind else { continue };
            nets += 1;
            let expr = abl_bridge::NetTranslator::translate(net).expr;
            let back = abl_bridge::decompile(&expr, &net.name);
            if expr.opcodes().contains(&Op::RES_ADD) || has_par(&expr) || !back.skipped.is_empty() {
                continue;
            }
            exact += 1;
            let again = abl_bridge::NetTranslator::translate(&back.net).expr;
            if again.content_hash() != expr.content_hash() {
                changed.push(format!("{}: net {}", path.display(), net.name));
            }
        }
    }
    assert!(nets >= 30, "only {nets} nets: the corpus walk is broken");
    assert!(exact * 2 >= nets, "only {exact} of {nets} nets are free of stated losses");
    assert!(changed.is_empty(), "{} of {exact} nets changed:\n{}", changed.len(), changed.join("\n"));
}

#[test]
fn every_stated_loss_has_a_witness_and_every_witness_a_stated_loss() {
    let knowledge = kb::load().unwrap_or_else(|e| panic!("kb does not load: {e:#?}"));

    let stated_losses = triples(knowledge.facts("loses"), "loses");
    let witnessed_losses: BTreeSet<(String, String, String)> = LOSSES
        .iter()
        .map(|(f, t, x, _)| (f.to_string(), t.to_string(), x.to_string()))
        .collect();
    let stated_trips = pairs(knowledge.facts("round_trips"), "round_trips");
    let witnessed_trips: BTreeSet<(String, String)> =
        ROUND_TRIPS.iter().map(|(f, t, _)| (f.to_string(), t.to_string())).collect();
    let lowers = pairs(knowledge.facts("lowers_to"), "lowers_to");

    let mut problems = Vec::new();
    let missing: BTreeSet<_> = stated_losses.difference(&witnessed_losses).cloned().collect();
    if !missing.is_empty() {
        problems.push(format!("stated losses with no witness:\n{}", describe(&missing)));
    }
    let unstated: BTreeSet<_> = witnessed_losses.difference(&stated_losses).cloned().collect();
    if !unstated.is_empty() {
        problems.push(format!("witnesses for losses kb/forms.mg does not state:\n{}", describe(&unstated)));
    }
    let missing_trips: BTreeSet<_> = stated_trips.difference(&witnessed_trips).cloned().collect();
    if !missing_trips.is_empty() {
        problems.push(format!("stated round trips with no check:\n{}", describe(&missing_trips)));
    }
    let unstated_trips: BTreeSet<_> = witnessed_trips.difference(&stated_trips).cloned().collect();
    if !unstated_trips.is_empty() {
        problems.push(format!("checks for round trips kb/forms.mg does not state:\n{}", describe(&unstated_trips)));
    }
    let uncovered: BTreeSet<_> = lowers
        .iter()
        .filter(|(f, t)| {
            !witnessed_losses.iter().any(|(lf, lt, _)| lf == f && lt == t)
                && !witnessed_trips.contains(&(f.clone(), t.clone()))
        })
        .cloned()
        .collect();
    if !uncovered.is_empty() {
        problems.push(format!(
            "lowers_to facts with neither a loses nor a round_trips row:\n{}",
            describe(&uncovered)
        ));
    }

    assert!(problems.is_empty(), "\n{}\n", problems.join("\n"));
}

#[test]
fn every_stated_loss_happens() {
    let mut failures = Vec::new();
    for (from, to, text, witness) in LOSSES {
        match catch(*witness) {
            Ok(true) => {}
            Ok(false) => failures.push(format!(
                "{from} -> {to}: \"{text}\" was not observed (witness returned false)"
            )),
            Err(p) => failures.push(format!("{from} -> {to}: \"{text}\": witness panicked: {p}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} stated losses did not hold:\n{}",
        failures.len(),
        LOSSES.len(),
        failures.join("\n")
    );
}

#[test]
fn every_stated_round_trip_holds() {
    let mut failures = Vec::new();
    for (from, to, check) in ROUND_TRIPS {
        if let Err(p) = catch(*check) {
            failures.push(format!("{from} -> {to} does not round-trip: {p}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} stated round trips failed:\n{}",
        failures.len(),
        ROUND_TRIPS.len(),
        failures.join("\n")
    );
}

