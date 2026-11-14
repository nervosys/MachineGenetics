//! MAGE on the query engine: the compiler's stages as queries.
//!
//! ```text
//!  Source(f) ── Tokens(f) ── Parsed(f) ── Elided(f) ──┬── Items(f) ── ItemHash(f,i)
//!  Settings ───────────────────────────────┘          │
//!                                                     ├── Item(f,i)
//!                                                     ├── Names(f) ── ResolveItem(f,i) ┐
//!                                                     │         └──── ResolveContracts(f)├─ Resolve(f)
//!                                                     ├── ItemPass(f,p,i)   (verify, logic, contract,
//!                                                     │                      core, shape, ai, and any
//!                                                     │                      pass you register)
//!                                                     ├── ModulePass(f,p)   (passes you register)
//!                                                     ├── BorrowSummary(f)  whole module (cut off on its fingerprint)
//!                                                     ├── BorrowItem(f,i)   per item, reads BorrowSummary(f) ── Borrowck(f)
//!                                                     ├── Types(f)          whole module
//!                                                     ├── Effects(f)        whole module
//!                                                     ├── Diff(f)           whole module (reads Effects)
//!                                                     └── Mlir(f)           whole module (reads Effects)
//!   Diagnostics(f) = Resolve + Types + Effects + Diff + every pass      Checked(f) = `check_source`
//!   Heal(f), Report(f) = what `run_pipeline` computes
//! ```
//!
//! # What is per item and what is not
//!
//! **Per item** (the answer depends on that item and nothing else, so an edit
//! to one function re-runs only that function's queries):
//! `ResolveItem` (an item's names against the module's *list of names*, which
//! is its own query, `Names`), and the item passes: `verify`, `logic`,
//! `contract`, `core`, `shape`, `ai`. Each of those passes is a loop over
//! `module.items` with no state carried between iterations, so running it on
//! a one-item module is the same computation.
//!
//! **Whole module** (wrapped, not split):
//!
//! * `Types` — `types::check` threads one substitution and one type-variable
//!   supply through every function. An unannotated parameter or return type is
//!   a variable shared by the function's body and by every call site, so
//!   checking `f` can constrain what `g` is, and diagnostics print variable
//!   numbers that depend on how many were minted earlier. Splitting it would
//!   mean rewriting the checker.
//! * `BorrowSummary` — the borrow checker's module facts (`borrowck::ModuleInfo`):
//!   which user types are `Copy`, every signature, and which parameters each
//!   function's body consumes (a fixpoint through callee bodies). Building it
//!   reads every body, so it re-runs on any AST change, but its *answer* is
//!   small and comparable: an edit that moves no function's consumed-parameter
//!   set and no type fact leaves the fingerprint alone, and the per-item check
//!   (`BorrowItem`, the `pass/borrowck` row) is cut off for every other item.
//!   The check of one function reads its own body plus that summary, so it is
//!   a pure function of the two.
//! * `Effects` — a call graph with a transitive fixpoint: a function's
//!   inferred effects are those of everything it reaches.
//! * `Diff` — the same call-graph shape, plus the module's net names.
//! * `Mlir` — one SSA counter runs across the whole module, so item *n*'s text
//!   depends on how many values items before it produced.
//! * `ResolveContracts` — a `spec` block's clauses attach to a function found
//!   by name anywhere in the module.
//!
//! These re-run whenever the module's AST changes at all. What the engine does
//! for them is cut off *above* them: an edit that leaves the AST unchanged
//! (whitespace, comments, in a file with no span-bearing nodes) stops before
//! they are asked.
//!
//! # Fingerprints
//!
//! An answer's fingerprint is the SHA-256 of its `Debug` rendering, not of its
//! JSON. The two differ exactly where serde skips a field, and the AST skips
//! `line` on a kb fact and rule — a field `logic` reads. Hashing the JSON would
//! let a line shift pass early cutoff and serve a stale diagnostic.

use super::disk::{DiskCache, build_id};
use super::engine::{Db, Domain, Hash32, QueryError, QueryStats, Stats, digest, hex};
use crate::ast::{self, ItemKind};
use crate::borrowck::{self, ModuleInfo};
use crate::check::{Checked, Finding};
use crate::effects::{self, EffectInfer};
use crate::hir::{Diagnostic, DiagnosticCategory, EffectSet, Severity};
use crate::lexer::{self, Token};
use crate::resolve::{self, NameDecl};
use crate::{abl_shape, autograd, canon, contract, core_subset, differentiable, elision, heal, logic, mlir, parser, shape, types, verdict, verify};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

// ── Identities ───────────────────────────────────────────────────────

/// A file known to the compiler.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct File(pub u32);

/// An item's identity: stable when items above it are added or removed.
#[derive(Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemId {
    pub kind: String,
    pub name: String,
    /// Among items with the same kind and name, which one (in source order).
    pub nth: u32,
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.nth == 0 {
            write!(f, "{} {}", self.kind, self.name)
        } else {
            write!(f, "{} {}#{}", self.kind, self.name, self.nth)
        }
    }
}

fn kind_name(k: &ItemKind) -> &'static str {
    match k {
        ItemKind::Function(_) => "Function",
        ItemKind::Struct(_) => "Struct",
        ItemKind::Enum(_) => "Enum",
        ItemKind::Trait(_) => "Trait",
        ItemKind::Impl(_) => "Impl",
        ItemKind::Module(_) => "Module",
        ItemKind::Use(_) => "Use",
        ItemKind::TypeAlias(_) => "TypeAlias",
        ItemKind::Const(_) => "Const",
        ItemKind::Static(_) => "Static",
        ItemKind::Effect(_) => "Effect",
        ItemKind::Spec(_) => "Spec",
        ItemKind::Agent(_) => "Agent",
        ItemKind::Net(_) => "Net",
        ItemKind::Kb(_) => "Kb",
        ItemKind::Evolve(_) => "Evolve",
        ItemKind::Train(_) => "Train",
        ItemKind::Swarm(_) => "Swarm",
        ItemKind::Data(_) => "Data",
        ItemKind::Extend(_) => "Extend",
    }
}

fn item_name(k: &ItemKind) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow::{Borrowed, Owned};
    match k {
        ItemKind::Function(x) => Borrowed(&x.name),
        ItemKind::Struct(x) => Borrowed(&x.name),
        ItemKind::Enum(x) => Borrowed(&x.name),
        ItemKind::Trait(x) => Borrowed(&x.name),
        ItemKind::Module(x) => Borrowed(&x.name),
        ItemKind::Use(x) => Owned(x.path.join(".")),
        ItemKind::TypeAlias(x) => Borrowed(&x.name),
        ItemKind::Const(x) => Borrowed(&x.name),
        ItemKind::Static(x) => Borrowed(&x.name),
        ItemKind::Effect(x) => Borrowed(&x.name),
        ItemKind::Spec(x) => Borrowed(&x.name),
        ItemKind::Agent(x) => Borrowed(&x.name),
        ItemKind::Net(x) => Borrowed(&x.name),
        ItemKind::Kb(x) => Borrowed(&x.name),
        ItemKind::Evolve(x) => Borrowed(&x.name),
        ItemKind::Train(x) => Borrowed(&x.name),
        ItemKind::Swarm(x) => Borrowed(&x.name),
        ItemKind::Data(x) => Borrowed(&x.name),
        ItemKind::Impl(_) | ItemKind::Extend(_) => Borrowed(""),
    }
}

/// The identities of a module's top-level items, in source order.
pub fn item_ids(module: &ast::Module) -> Vec<ItemId> {
    let mut seen: BTreeMap<(&'static str, String), u32> = BTreeMap::new();
    module
        .items
        .iter()
        .map(|it| {
            let kind = kind_name(&it.kind);
            let name = item_name(&it.kind).into_owned();
            let n = seen.entry((kind, name.clone())).or_insert(0);
            let id = ItemId { kind: kind.to_string(), name, nth: *n };
            *n += 1;
            id
        })
        .collect()
}

fn sha_debug<T: std::fmt::Debug>(t: &T) -> Hash32 {
    digest(&[format!("{t:?}").as_bytes()])
}

// ── Answers ──────────────────────────────────────────────────────────

/// The settings that change what the stages produce.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    /// Safety elision (`--no-elision` turns it off).
    pub elision: bool,
    /// Also require MAGE-core (`--core`).
    pub core: bool,
}

/// A parse failure, with what `run_pipeline` and `check_source` print of it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ParseErrRec {
    pub line: usize,
    pub col: usize,
    pub message: String,
    pub described: String,
}

/// What the parser (and then elision) produced.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParseOutcome {
    pub module: Option<ast::Module>,
    pub error: Option<ParseErrRec>,
}

/// One top-level item, as listed.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct ItemInfo {
    pub id: ItemId,
    /// SHA-256 of the item's `Debug` form: exact, so a changed span counts.
    pub hash: String,
    /// `canon::definition_hash` for a function: equal for two functions that
    /// differ only in bound-variable names. Informational: no query is keyed
    /// on it, because diagnostics quote names.
    pub canon: Option<String>,
}

/// What one pass said about one item or one module.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PassOut {
    pub diags: Vec<Diagnostic>,
    /// Free-form lines a pass wants to report (the shape pass's messages, the
    /// ai pass's `net X: output shape [..]`, verify's JSON results).
    pub notes: Vec<String>,
    /// A pass-defined count (symbols allocated, errors found, results).
    pub count: usize,
}

impl PassOut {
    pub fn diags(diags: Vec<Diagnostic>) -> Self {
        PassOut { diags, notes: Vec::new(), count: 0 }
    }
}

/// The name list of a module and what defining it reported.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamesOut {
    pub decls: Vec<NameDecl>,
    pub base: resolve::Partial,
}

/// Name resolution, assembled.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolveOut {
    pub diagnostics: Vec<Diagnostic>,
    pub symbols: usize,
}

/// Effect inference, in the parts later stages read.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EffectsOut {
    pub inferred: BTreeMap<String, EffectSet>,
    pub diagnostics: Vec<Diagnostic>,
    pub surface: Vec<(String, Vec<String>, Vec<String>)>,
}

impl EffectsOut {
    /// An `EffectInfer` carrying what `mlir::emit` and `differentiable::infer`
    /// read of it (`inferred`), and its diagnostics.
    fn infer(&self) -> EffectInfer {
        let mut e = EffectInfer::new();
        e.inferred = self.inferred.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        e.diagnostics = self.diagnostics.clone();
        e
    }
}

/// Every diagnostic of every pass, in `check_source`'s order.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagOut {
    pub all: Vec<Diagnostic>,
    /// Messages of the typed-composition gate (`E0710`).
    pub shape: Vec<String>,
    pub surface: Vec<serde_json::Value>,
}

/// One record of `--check --json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingRec {
    pub severity: String,
    pub line: u32,
    pub col: u32,
    pub code: String,
    pub category: String,
    pub message: String,
    pub fix: String,
}

/// `check::Checked`, in a form that serialises.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckedRec {
    pub findings: Vec<FindingRec>,
    pub effects: Vec<serde_json::Value>,
}

impl CheckedRec {
    /// The `check::Checked` this record stands for.
    pub fn to_checked(&self) -> Checked {
        let sev = |s: &str| match s {
            "error" => "error",
            "warning" => "warning",
            _ => "note",
        };
        Checked {
            findings: self
                .findings
                .iter()
                .map(|f| Finding {
                    severity: sev(&f.severity),
                    line: f.line,
                    col: f.col,
                    code: f.code.clone(),
                    category: f.category.clone(),
                    message: f.message.clone(),
                    fix: f.fix.clone(),
                })
                .collect(),
            effects: self.effects.clone(),
        }
    }

    pub fn from_checked(c: &Checked) -> Self {
        CheckedRec {
            findings: c
                .findings
                .iter()
                .map(|f| FindingRec {
                    severity: f.severity.to_string(),
                    line: f.line,
                    col: f.col,
                    code: f.code.clone(),
                    category: f.category.clone(),
                    message: f.message.clone(),
                    fix: f.fix.clone(),
                })
                .collect(),
            effects: c.effects.clone(),
        }
    }
}

/// What `run_pipeline` computes, as data. The incremental path and the
/// reference path both produce this, and the corpus test compares them.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PipelineReport {
    pub parse_error: Option<ParseErrRec>,
    pub tokens: usize,
    pub lex_errors: usize,
    pub items: usize,
    /// `Some` only when elision ran: whether it changed the tree.
    pub elision_changed: Option<bool>,
    pub resolve_diags: Vec<String>,
    pub symbols: usize,
    pub type_diags: Vec<String>,
    /// Ownership and borrow diagnostics (E0382, E0502).
    #[serde(default)]
    pub borrow_diags: Vec<String>,
    pub ai_info: Vec<String>,
    pub ai_diags: Vec<String>,
    pub ai_errors: usize,
    pub effect_diags: Vec<String>,
    /// `(function, effects)` sorted by name.
    pub effects: Vec<(String, Vec<String>)>,
    pub verify_lines: Vec<String>,
    pub verify_diags: Vec<String>,
    pub contract_total: usize,
    pub verdict_summary: String,
    pub mlir_lines: usize,
    pub mlir_sha256: String,
    pub heal_diagnostics: usize,
    pub heal_fixes: usize,
    pub total_errors: usize,
}

// ── Passes ───────────────────────────────────────────────────────────

/// A per-item analysis: a pure function of one item.
pub type ItemFn = fn(&ast::Item) -> PassOut;
/// A per-module analysis: a pure function of the (elided) module.
pub type ModuleFn = fn(&ast::Module) -> PassOut;

/// How an item pass's output is used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Its diagnostics join the union that `Diagnostics`/`Checked` report.
    Diagnostics,
    /// As `Diagnostics`, but only under `--core`.
    CoreOnly,
    /// Its `notes` are typed-composition (`E0710`) messages.
    Shape,
    /// Its `notes` and `diags` belong to the pipeline report, not to `--check`.
    Report,
}

#[derive(Clone)]
pub struct ItemPassDef {
    pub name: &'static str,
    /// The row this pass has in the statistics (`pass/<name>`).
    pub stat: &'static str,
    /// Part of the cache address. Bump it when the pass's behaviour changes
    /// and the crate version does not: the build id covers the passes that ship
    /// in this crate, but cannot see a pass defined elsewhere.
    pub version: &'static str,
    pub role: Role,
    pub run: ItemFn,
}

#[derive(Clone)]
pub struct ModulePassDef {
    pub name: &'static str,
    pub stat: &'static str,
    pub version: &'static str,
    pub run: ModuleFn,
}

/// The analysis passes the engine runs after the fixed stages.
///
/// **Adding a pass is one line.** A pure per-module pass:
///
/// ```ignore
/// let registry = Registry::builtin().module_pass("borrowck", "1", |m| PassOut::diags(borrowck::check(m)));
/// ```
///
/// or a pure per-item one (re-run only for an edited item):
///
/// ```ignore
/// let registry = Registry::builtin().item_pass("lint", "1", |item| PassOut::diags(lint(item)));
/// ```
///
/// Its diagnostics are unioned into `Diagnostics`, `Checked` and the report,
/// after the built-in passes, in registration order. It runs on the *elided*
/// module, as the built-in passes do, and is cached under its name and version.
#[derive(Clone)]
pub struct Registry {
    pub items: Vec<ItemPassDef>,
    pub modules: Vec<ModulePassDef>,
}

impl Registry {
    /// The passes `check_source` runs per item, in its order.
    pub fn builtin() -> Self {
        let b = |name, stat, role, run| ItemPassDef { name, stat, version: "1", role, run };
        Registry {
            items: vec![
                b("verify", "pass/verify", Role::Diagnostics, pass_verify as ItemFn),
                b("logic", "pass/logic", Role::Diagnostics, pass_logic as ItemFn),
                b("contract", "pass/contract", Role::Diagnostics, pass_contract as ItemFn),
                b("core", "pass/core", Role::CoreOnly, pass_core as ItemFn),
                b("shape", "pass/shape", Role::Shape, pass_shape as ItemFn),
                b("ai", "pass/ai", Role::Report, pass_ai as ItemFn),
            ],
            modules: Vec::new(),
        }
    }

    /// Register a pure per-item pass.
    pub fn item_pass(mut self, name: &'static str, version: &'static str, run: ItemFn) -> Self {
        let stat = Box::leak(format!("pass/{name}").into_boxed_str());
        self.items.push(ItemPassDef { name, stat, version, role: Role::Diagnostics, run });
        self
    }

    /// Register a pure per-module pass.
    pub fn module_pass(mut self, name: &'static str, version: &'static str, run: ModuleFn) -> Self {
        let stat = Box::leak(format!("module-pass/{name}").into_boxed_str());
        self.modules.push(ModulePassDef { name, stat, version, run });
        self
    }

    fn item_index(&self, name: &str) -> usize {
        self.items.iter().position(|p| p.name == name).expect("built-in pass")
    }
}

fn one(item: &ast::Item) -> ast::Module {
    ast::Module { items: vec![item.clone()] }
}

fn pass_verify(item: &ast::Item) -> PassOut {
    let results = verify::verify_module(&one(item));
    PassOut {
        diags: verify::diagnostics(&results),
        notes: results.iter().map(|r| serde_json::to_string(r).expect("serialises")).collect(),
        count: results.len(),
    }
}

fn pass_logic(item: &ast::Item) -> PassOut {
    PassOut::diags(logic::module_diagnostics(&one(item)))
}

fn pass_contract(item: &ast::Item) -> PassOut {
    PassOut::diags(contract::unchecked_contract_warnings(&one(item)))
}

fn pass_core(item: &ast::Item) -> PassOut {
    PassOut::diags(core_subset::check(&one(item)))
}

fn pass_shape(item: &ast::Item) -> PassOut {
    PassOut {
        diags: Vec::new(),
        notes: abl_shape::check_module_shapes(&one(item)).into_iter().map(|d| d.message).collect(),
        count: 0,
    }
}

/// Phase 4.5 of `run_pipeline`: shape inference on a net, the autograd tape of
/// a train block, materialising a kb. The lines are `run_pipeline`'s `ai_info`
/// entries verbatim (a leading `!` marks one that is not a success).
fn pass_ai(item: &ast::Item) -> PassOut {
    let mut out = PassOut::default();
    let errors = |ds: &[Diagnostic], out: &mut PassOut| {
        for d in ds {
            if d.severity == Severity::Error {
                out.count += 1;
            }
            out.diags.push(d.clone());
        }
    };
    match &item.kind {
        ItemKind::Net(net) => {
            let mut infer = shape::ShapeInfer::new();
            let input_shape = vec![infer.fresh_dim(), infer.fresh_dim()];
            let out_shape = infer.infer_net(net, &input_shape);
            errors(&infer.diagnostics, &mut out);
            let resolved = infer.resolve_shape(&out_shape);
            let dims: Vec<String> = resolved.iter().map(|d| format!("{d:?}")).collect();
            out.notes.push(format!("net {}: output shape [{}]", net.name, dims.join(", ")));
        }
        ItemKind::Train(train) => {
            let tape = autograd::build_tape_from_train(train);
            let loss_id = if tape.nodes.is_empty() { 0 } else { tape.nodes.len() - 1 };
            let param_names: Vec<String> = tape
                .nodes
                .iter()
                .filter_map(|n| {
                    if let autograd::GradOp::Param(name) = &n.op { Some(name.clone()) } else { None }
                })
                .collect();
            let grad_result = autograd::backward(&tape, loss_id, &param_names);
            if tape.nodes.len() <= 1 {
                out.notes.push(format!(
                    "!train {}: autograd tape covers nothing ({} node) — it reads `body`, and this block declares its net and loss as fields. No gradient is computed here; training itself uses rmi's backend",
                    train.name,
                    tape.nodes.len()
                ));
            } else {
                out.notes.push(format!(
                    "train {}: {} forward ops, {} backward ops",
                    train.name,
                    tape.nodes.len(),
                    grad_result.mlir_ops.len()
                ));
            }
            errors(&grad_result.diagnostics, &mut out);
        }
        ItemKind::Kb(kb_def) => {
            let mut kb = logic::build_kb(kb_def);
            kb.materialize();
            errors(&kb.diagnostics, &mut out);
            out.notes.push(format!("kb {}: {} facts materialized", kb.name, kb.fact_count()));
        }
        _ => {}
    }
    out
}

// ── Keys and values ──────────────────────────────────────────────────

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    // Inputs.
    Source(File),
    Settings,
    // The front end.
    Tokens(File),
    Parsed(File),
    Elided(File),
    // Items.
    Items(File),
    Item(File, ItemId),
    ItemHash(File, ItemId),
    // Resolution.
    Names(File),
    ResolveItem(File, ItemId),
    ResolveContracts(File),
    Resolve(File),
    // Analyses.
    ItemPass(File, usize, ItemId),
    ModulePass(File, usize),
    /// The borrow checker's module summary.
    BorrowSummary(File),
    /// The borrow check of one item, against the summary.
    BorrowItem(File, ItemId),
    /// Every item's borrow diagnostics, in item order.
    Borrowck(File),
    Types(File),
    Effects(File),
    Diff(File),
    Mlir(File),
    // Assembled outputs.
    Diagnostics(File),
    Checked(File),
    Heal(File),
    Report(File),
}

#[derive(Clone, Debug)]
pub enum Val {
    Source(Arc<String>),
    Settings(Settings),
    Tokens(Arc<Vec<Token>>),
    Outcome(Arc<ParseOutcome>),
    Items(Arc<Vec<ItemInfo>>),
    Item(Arc<ast::Item>),
    /// An item's hash, or `None` if the module has no such item.
    ItemHash(Arc<Option<String>>),
    Names(Arc<NamesOut>),
    Pass(Arc<PassOut>),
    Resolve(Arc<ResolveOut>),
    Summary(Arc<ModuleInfo>),
    Effects(Arc<EffectsOut>),
    Text(Arc<String>),
    Diag(Arc<DiagOut>),
    Checked(Arc<CheckedRec>),
    Heal(Arc<Vec<heal::HealedDiagnostic>>),
    Report(Arc<PipelineReport>),
}

/// The domain.
pub struct Mage;

impl Domain for Mage {
    type Key = Key;
    type Val = Val;
    type Ctx = Registry;

    fn query_name(db: &Db<Mage>, key: &Key) -> &'static str {
        match key {
            Key::Source(_) => "source",
            Key::Settings => "settings",
            Key::Tokens(_) => "tokens",
            Key::Parsed(_) => "parsed",
            Key::Elided(_) => "elided",
            Key::Items(_) => "items",
            Key::Item(..) => "item",
            Key::ItemHash(..) => "item_hash",
            Key::Names(_) => "names",
            Key::ResolveItem(..) => "resolve_item",
            Key::ResolveContracts(_) => "resolve_contracts",
            Key::Resolve(_) => "resolve",
            // Item and module passes are named by the pass, so a registered
            // pass has its own row in the statistics.
            Key::ItemPass(_, p, _) => db.ctx.items[*p].stat,
            Key::ModulePass(_, p) => db.ctx.modules[*p].stat,
            Key::BorrowSummary(_) => "borrowck/summary",
            Key::BorrowItem(..) => "pass/borrowck",
            Key::Borrowck(_) => "borrowck",
            Key::Types(_) => "types",
            Key::Effects(_) => "effects",
            Key::Diff(_) => "diff",
            Key::Mlir(_) => "mlir",
            Key::Diagnostics(_) => "diagnostics",
            Key::Checked(_) => "checked",
            Key::Heal(_) => "heal",
            Key::Report(_) => "report",
        }
    }

    fn is_input(key: &Key) -> bool {
        matches!(key, Key::Source(_) | Key::Settings)
    }

    fn execute(db: &Db<Mage>, key: &Key) -> Result<Val, QueryError> {
        exec(db, key)
    }

    fn value_hash(_: &Key, val: &Val) -> Hash32 {
        match val {
            // `ModuleInfo` keeps its facts in hash maps, whose `Debug` order
            // differs from one map to the next: fingerprint a canonical form.
            Val::Summary(info) => summary_fingerprint(info),
            _ => sha_debug(val),
        }
    }

    fn address(db: &Db<Mage>, key: &Key) -> Result<Option<Hash32>, QueryError> {
        addr(db, key)
    }

    fn encode(key: &Key, val: &Val) -> Option<String> {
        encode(key, val)
    }

    fn decode(key: &Key, payload: &str) -> Option<Val> {
        decode(key, payload)
    }
}

type M = Db<Mage>;

/// SHA-256 of `info` as JSON with every object's keys sorted.
fn summary_fingerprint(info: &ModuleInfo) -> Hash32 {
    fn canon(v: &serde_json::Value, out: &mut String) {
        use serde_json::Value;
        match v {
            Value::Object(m) => {
                let mut keys: Vec<&String> = m.keys().collect();
                keys.sort();
                out.push('{');
                for k in keys {
                    out.push_str(&serde_json::to_string(k).unwrap_or_default());
                    out.push(':');
                    canon(&m[k], out);
                    out.push(',');
                }
                out.push('}');
            }
            Value::Array(a) => {
                out.push('[');
                for x in a {
                    canon(x, out);
                    out.push(',');
                }
                out.push(']');
            }
            other => out.push_str(&other.to_string()),
        }
    }
    let mut text = String::new();
    canon(&serde_json::to_value(info).unwrap_or(serde_json::Value::Null), &mut text);
    digest(&[text.as_bytes()])
}

/// Part of the cache address of the borrow check: bump it when `borrowck`'s
/// behaviour changes and the crate version does not.
const BORROWCK_VERSION: &str = "1";

fn mismatch(key: &Key, got: &Val) -> QueryError {
    let name = format!("{got:?}");
    let name = name.split(['(', ' ']).next().unwrap_or("?");
    QueryError::Failed(format!("{key:?} answered a {name}"))
}

macro_rules! fetch {
    ($db:expr, $key:expr, $var:ident) => {{
        let key = $key;
        match $db.get(&key)? {
            Val::$var(x) => x,
            other => return Err(mismatch(&key, &other)),
        }
    }};
}

fn settings(db: &M) -> Result<Settings, QueryError> {
    Ok(match db.get(&Key::Settings)? {
        Val::Settings(s) => s,
        other => return Err(mismatch(&Key::Settings, &other)),
    })
}

fn outcome(db: &M, f: File) -> Result<Arc<ParseOutcome>, QueryError> {
    Ok(fetch!(db, Key::Elided(f), Outcome))
}

fn find<'a>(module: &'a ast::Module, id: &ItemId) -> Option<&'a ast::Item> {
    let mut seen = 0;
    for it in &module.items {
        if kind_name(&it.kind) == id.kind && item_name(&it.kind) == id.name {
            if seen == id.nth {
                return Some(it);
            }
            seen += 1;
        }
    }
    None
}

fn parse_outcome(tokens: &[Token]) -> ParseOutcome {
    match parser::parse(tokens) {
        Ok(m) => ParseOutcome { module: Some(m), error: None },
        Err(e) => ParseOutcome {
            module: None,
            error: Some(ParseErrRec { line: e.line, col: e.col, message: e.message.clone(), described: e.describe() }),
        },
    }
}

fn empty_pass() -> Val {
    Val::Pass(Arc::new(PassOut::default()))
}

fn exec(db: &M, key: &Key) -> Result<Val, QueryError> {
    Ok(match key {
        Key::Source(_) | Key::Settings => unreachable!("inputs are never executed"),
        Key::Tokens(f) => {
            let src = fetch!(db, Key::Source(*f), Source);
            Val::Tokens(Arc::new(lexer::lex(&src)))
        }
        Key::Parsed(f) => {
            let toks = fetch!(db, Key::Tokens(*f), Tokens);
            Val::Outcome(Arc::new(parse_outcome(&toks)))
        }
        Key::Elided(f) => {
            let st = settings(db)?;
            let parsed = fetch!(db, Key::Parsed(*f), Outcome);
            match (&parsed.module, st.elision) {
                (Some(m), true) => {
                    Val::Outcome(Arc::new(ParseOutcome { module: Some(elision::elide(m)), error: None }))
                }
                _ => Val::Outcome(parsed),
            }
        }
        Key::Items(f) => {
            let o = outcome(db, *f)?;
            let infos = match &o.module {
                None => Vec::new(),
                Some(m) => item_ids(m)
                    .into_iter()
                    .zip(&m.items)
                    .map(|(id, it)| ItemInfo {
                        id,
                        hash: hex(&sha_debug(it)),
                        canon: match &it.kind {
                            ItemKind::Function(fd) => Some(canon::definition_hash(fd)),
                            _ => None,
                        },
                    })
                    .collect(),
            };
            Val::Items(Arc::new(infos))
        }
        Key::Item(f, id) => {
            let o = outcome(db, *f)?;
            let item = o.module.as_ref().and_then(|m| find(m, id));
            match item {
                Some(it) => Val::Item(Arc::new(it.clone())),
                None => return Err(QueryError::Failed(format!("no item `{id}`"))),
            }
        }
        Key::ItemHash(f, id) => {
            let items = fetch!(db, Key::Items(*f), Items);
            Val::ItemHash(Arc::new(items.iter().find(|i| &i.id == id).map(|i| i.hash.clone())))
        }
        Key::Names(f) => {
            let o = outcome(db, *f)?;
            let decls: Vec<NameDecl> = match &o.module {
                Some(m) => m.items.iter().flat_map(resolve::item_decls).collect(),
                None => Vec::new(),
            };
            let base = resolve::resolve_names(&decls);
            Val::Names(Arc::new(NamesOut { decls, base }))
        }
        Key::ResolveItem(f, id) => {
            let names = fetch!(db, Key::Names(*f), Names);
            let item = fetch!(db, Key::Item(*f, id.clone()), Item);
            let p = resolve::resolve_item_in(&names.decls, &item);
            Val::Pass(Arc::new(PassOut { diags: p.diagnostics, notes: Vec::new(), count: p.symbols }))
        }
        Key::ResolveContracts(f) => {
            let o = outcome(db, *f)?;
            let names = fetch!(db, Key::Names(*f), Names);
            match &o.module {
                None => empty_pass(),
                Some(m) => {
                    let p = resolve::resolve_contracts_in(&names.decls, m);
                    Val::Pass(Arc::new(PassOut { diags: p.diagnostics, notes: Vec::new(), count: p.symbols }))
                }
            }
        }
        Key::Resolve(f) => {
            let items = fetch!(db, Key::Items(*f), Items);
            let names = fetch!(db, Key::Names(*f), Names);
            let mut diagnostics = names.base.diagnostics.clone();
            let mut symbols = names.base.symbols;
            for info in items.iter() {
                let p = fetch!(db, Key::ResolveItem(*f, info.id.clone()), Pass);
                diagnostics.extend(p.diags.iter().cloned());
                symbols += p.count;
            }
            let c = fetch!(db, Key::ResolveContracts(*f), Pass);
            diagnostics.extend(c.diags.iter().cloned());
            symbols += c.count;
            Val::Resolve(Arc::new(ResolveOut { diagnostics, symbols }))
        }
        Key::ItemPass(f, p, id) => {
            let item = fetch!(db, Key::Item(*f, id.clone()), Item);
            Val::Pass(Arc::new((db.ctx.items[*p].run)(&item)))
        }
        Key::ModulePass(f, p) => {
            let o = outcome(db, *f)?;
            match &o.module {
                Some(m) => Val::Pass(Arc::new((db.ctx.modules[*p].run)(m))),
                None => empty_pass(),
            }
        }
        Key::BorrowSummary(f) => {
            let o = outcome(db, *f)?;
            match &o.module {
                Some(m) => Val::Summary(Arc::new(ModuleInfo::new(m))),
                None => Val::Summary(Arc::new(ModuleInfo::default())),
            }
        }
        Key::BorrowItem(f, id) => {
            let info = fetch!(db, Key::BorrowSummary(*f), Summary);
            let item = fetch!(db, Key::Item(*f, id.clone()), Item);
            Val::Pass(Arc::new(PassOut::diags(borrowck::check_item(&info, &item))))
        }
        Key::Borrowck(f) => {
            let items = fetch!(db, Key::Items(*f), Items);
            let mut diags = Vec::new();
            for i in items.iter() {
                diags.extend(fetch!(db, Key::BorrowItem(*f, i.id.clone()), Pass).diags.iter().cloned());
            }
            Val::Pass(Arc::new(PassOut::diags(diags)))
        }
        Key::Types(f) => {
            let o = outcome(db, *f)?;
            match &o.module {
                Some(m) => Val::Pass(Arc::new(PassOut::diags(types::check(m).diagnostics))),
                None => empty_pass(),
            }
        }
        Key::Effects(f) => {
            let o = outcome(db, *f)?;
            let out = match &o.module {
                Some(m) => {
                    let e = effects::infer_effects(m);
                    EffectsOut {
                        inferred: e.inferred.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                        diagnostics: e.diagnostics.clone(),
                        surface: e.effect_surface(),
                    }
                }
                None => EffectsOut { inferred: BTreeMap::new(), diagnostics: Vec::new(), surface: Vec::new() },
            };
            Val::Effects(Arc::new(out))
        }
        Key::Diff(f) => {
            let o = outcome(db, *f)?;
            let eff = fetch!(db, Key::Effects(*f), Effects);
            match &o.module {
                None => empty_pass(),
                Some(m) => {
                    let engine = differentiable::infer(m, &eff.infer());
                    let mut d = differentiable::check_grad_obligations(m, &engine);
                    d.extend(differentiable::train_block_errors(m, &engine));
                    Val::Pass(Arc::new(PassOut::diags(d)))
                }
            }
        }
        Key::Mlir(f) => {
            let o = outcome(db, *f)?;
            let eff = fetch!(db, Key::Effects(*f), Effects);
            match &o.module {
                None => Val::Text(Arc::new(String::new())),
                Some(m) => Val::Text(Arc::new(mlir::emit(m, &eff.infer()))),
            }
        }
        Key::Diagnostics(f) => exec_diagnostics(db, *f)?,
        Key::Checked(f) => exec_checked(db, *f)?,
        Key::Heal(f) => {
            let resolve = fetch!(db, Key::Resolve(*f), Resolve);
            let types = fetch!(db, Key::Types(*f), Pass);
            let borrow = fetch!(db, Key::Borrowck(*f), Pass);
            let eff = fetch!(db, Key::Effects(*f), Effects);
            let mut all = resolve.diagnostics.clone();
            all.extend(types.diags.iter().cloned());
            all.extend(borrow.diags.iter().cloned());
            all.extend(eff.diagnostics.iter().cloned());
            Val::Heal(Arc::new(heal::heal(&all)))
        }
        Key::Report(f) => exec_report(db, *f)?,
    })
}

/// The queries `Diagnostics(f)` reads after the fixed stages, in union order.
fn pass_keys(db: &M, f: File, items: &[ItemInfo], core: bool) -> Vec<Key> {
    let mut keys = Vec::new();
    for (p, def) in db.ctx.items.iter().enumerate() {
        let wanted = match def.role {
            Role::Diagnostics => true,
            Role::CoreOnly => core,
            Role::Shape | Role::Report => false,
        };
        if wanted {
            keys.extend(items.iter().map(|i| Key::ItemPass(f, p, i.id.clone())));
        }
    }
    keys.extend((0..db.ctx.modules.len()).map(|p| Key::ModulePass(f, p)));
    keys
}

fn exec_diagnostics(db: &M, f: File) -> Result<Val, QueryError> {
    let o = outcome(db, f)?;
    if o.module.is_none() {
        return Ok(Val::Diag(Arc::new(DiagOut { all: Vec::new(), shape: Vec::new(), surface: Vec::new() })));
    }
    let st = settings(db)?;
    let items = fetch!(db, Key::Items(f), Items);
    let mut all: Vec<Diagnostic> = Vec::new();
    all.extend(fetch!(db, Key::Resolve(f), Resolve).diagnostics.iter().cloned());
    all.extend(fetch!(db, Key::Types(f), Pass).diags.iter().cloned());
    all.extend(fetch!(db, Key::Borrowck(f), Pass).diags.iter().cloned());
    let eff = fetch!(db, Key::Effects(f), Effects);
    all.extend(eff.diagnostics.iter().cloned());
    all.extend(fetch!(db, Key::Diff(f), Pass).diags.iter().cloned());
    for key in pass_keys(db, f, &items, st.core) {
        all.extend(fetch!(db, key, Pass).diags.iter().cloned());
    }
    let shape_pass = db.ctx.item_index("shape");
    let mut shape = Vec::new();
    for i in items.iter() {
        shape.extend(fetch!(db, Key::ItemPass(f, shape_pass, i.id.clone()), Pass).notes.iter().cloned());
    }
    let surface = eff
        .surface
        .iter()
        .map(|(func, declared, inferred)| {
            serde_json::json!({ "function": func, "declared": declared, "inferred": inferred })
        })
        .collect();
    Ok(Val::Diag(Arc::new(DiagOut { all, shape, surface })))
}

fn exec_checked(db: &M, f: File) -> Result<Val, QueryError> {
    let tokens = fetch!(db, Key::Tokens(f), Tokens);
    let o = outcome(db, f)?;
    let diag = fetch!(db, Key::Diagnostics(f), Diag);
    let mut findings: Vec<FindingRec> = Vec::new();
    let mut push = |severity: &str, line: u32, col: u32, code: &str, category: &str, message: String, fix: &str| {
        findings.push(FindingRec {
            severity: severity.to_string(),
            line,
            col,
            code: code.to_string(),
            category: category.to_string(),
            message,
            fix: fix.to_string(),
        });
    };
    for tok in tokens.iter() {
        if tok.kind == lexer::TokenKind::Error {
            push(
                "error",
                tok.span.line as u32,
                tok.span.col as u32,
                "E0000",
                "LexError",
                "unexpected character".to_string(),
                "remove or correct the invalid token",
            );
        }
    }
    if let Some(e) = &o.error {
        let cat = DiagnosticCategory::SyntaxError;
        push("error", e.line as u32, e.col as u32, cat.code(), "SyntaxError", e.message.clone(), cat.fix_hint());
    }
    if o.module.is_some() {
        for d in &diag.all {
            let cat = d.category.unwrap_or(DiagnosticCategory::Other);
            let (line, col) = d.span.map(|s| (s.line, s.col)).unwrap_or((0, 0));
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "note",
            };
            let code = d.id.clone().unwrap_or_else(|| cat.code().to_string());
            push(sev, line, col, &code, &format!("{cat:?}"), d.message.clone(), cat.fix_hint());
        }
        for m in &diag.shape {
            push(
                "error",
                0,
                0,
                "E0710",
                "ShapeError",
                m.clone(),
                "align the layer dims so each layer's output dim matches the next layer's input dim",
            );
        }
    }
    findings.sort_by(|a, b| (a.line, a.col, &a.code, &a.message).cmp(&(b.line, b.col, &b.code, &b.message)));
    let effects = if o.module.is_some() { diag.surface.clone() } else { Vec::new() };
    Ok(Val::Checked(Arc::new(CheckedRec { findings, effects })))
}

fn exec_report(db: &M, f: File) -> Result<Val, QueryError> {
    let st = settings(db)?;
    let tokens = fetch!(db, Key::Tokens(f), Tokens);
    let mut r = PipelineReport {
        tokens: tokens.len(),
        lex_errors: tokens.iter().filter(|t| t.kind == lexer::TokenKind::Error).count(),
        ..PipelineReport::default()
    };
    let mut total = r.lex_errors;
    let o = outcome(db, f)?;
    if let Some(e) = &o.error {
        r.parse_error = Some(e.clone());
        r.total_errors = total;
        return Ok(Val::Report(Arc::new(r)));
    }
    let items = fetch!(db, Key::Items(f), Items);
    r.items = items.len();
    if st.elision {
        let parsed = fetch!(db, Key::Parsed(f), Outcome);
        r.elision_changed = Some(sha_debug(&parsed.module) != sha_debug(&o.module));
    }
    let count_errors = |ds: &[Diagnostic]| ds.iter().filter(|d| d.severity == Severity::Error).count();

    let resolve = fetch!(db, Key::Resolve(f), Resolve);
    r.resolve_diags = resolve.diagnostics.iter().map(|d| d.to_string()).collect();
    r.symbols = resolve.symbols;
    total += count_errors(&resolve.diagnostics);

    let types = fetch!(db, Key::Types(f), Pass);
    r.type_diags = types.diags.iter().map(|d| d.to_string()).collect();
    total += count_errors(&types.diags);

    let borrow = fetch!(db, Key::Borrowck(f), Pass);
    r.borrow_diags = borrow.diags.iter().map(|d| d.to_string()).collect();
    total += count_errors(&borrow.diags);

    // Phase 4.5: nets, then trains, then kbs, as `run_pipeline` walks them.
    let ai = db.ctx.item_index("ai");
    for kind in ["Net", "Train", "Kb"] {
        for i in items.iter().filter(|i| i.id.kind == kind) {
            let p = fetch!(db, Key::ItemPass(f, ai, i.id.clone()), Pass);
            r.ai_diags.extend(p.diags.iter().map(|d| d.to_string()));
            r.ai_errors += p.count;
            r.ai_info.extend(p.notes.iter().cloned());
        }
    }
    total += r.ai_errors;

    let eff = fetch!(db, Key::Effects(f), Effects);
    r.effect_diags = eff.diagnostics.iter().map(|d| d.to_string()).collect();
    total += count_errors(&eff.diagnostics);
    r.effects = eff
        .inferred
        .iter()
        .map(|(n, fx)| (n.clone(), fx.iter().map(|e| e.to_string()).collect()))
        .collect();

    // Phase 5.5.
    let vp = db.ctx.item_index("verify");
    let mut results: Vec<verify::VerificationResult> = Vec::new();
    let mut vdiags: Vec<Diagnostic> = Vec::new();
    for i in items.iter() {
        let p = fetch!(db, Key::ItemPass(f, vp, i.id.clone()), Pass);
        for n in &p.notes {
            results.push(serde_json::from_str(n).map_err(|e| QueryError::Failed(e.to_string()))?);
        }
        vdiags.extend(p.diags.iter().cloned());
    }
    r.contract_total = results.iter().map(|v| v.checks.len()).sum();
    let mut tally = verdict::Tally::default();
    for v in &results {
        tally.add(&v.status.evidence(&v.fqn));
    }
    r.verdict_summary = tally.summary();
    r.verify_lines = results
        .iter()
        .map(|v| format!("{} {}: {}", v.status.symbol(), v.fqn, v.status.evidence(&v.fqn).describe()))
        .collect();
    r.verify_diags = vdiags.iter().map(|d| d.to_string()).collect();
    total += vdiags.len();

    let text = fetch!(db, Key::Mlir(f), Text);
    r.mlir_lines = text.lines().count();
    r.mlir_sha256 = hex(&digest(&[text.as_bytes()]));

    let healed = fetch!(db, Key::Heal(f), Heal);
    r.heal_diagnostics = healed.len();
    r.heal_fixes = healed.iter().map(|h| h.fixes.len()).sum();
    r.total_errors = total;
    Ok(Val::Report(Arc::new(r)))
}

/// What `run_pipeline` computes, computed the old way: the stages called one
/// after another with nothing remembered. The incremental path is held to this.
pub fn reference_report(source: &str, s: Settings) -> PipelineReport {
    let tokens = lexer::lex(source);
    let mut r = PipelineReport {
        tokens: tokens.len(),
        lex_errors: tokens.iter().filter(|t| t.kind == lexer::TokenKind::Error).count(),
        ..PipelineReport::default()
    };
    let mut total = r.lex_errors;
    let parsed = match parser::parse(&tokens) {
        Ok(m) => m,
        Err(e) => {
            r.parse_error =
                Some(ParseErrRec { line: e.line, col: e.col, message: e.message.clone(), described: e.describe() });
            r.total_errors = total;
            return r;
        }
    };
    r.items = parsed.items.len();
    let module = if s.elision {
        let elided = elision::elide(&parsed);
        r.elision_changed = Some(serde_json::to_string(&parsed).ok() != serde_json::to_string(&elided).ok());
        elided
    } else {
        parsed
    };
    let count_errors = |ds: &[Diagnostic]| ds.iter().filter(|d| d.severity == Severity::Error).count();

    let resolver = resolve::resolve(&module);
    r.resolve_diags = resolver.diagnostics.iter().map(|d| d.to_string()).collect();
    r.symbols = resolver.symbols.len();
    total += count_errors(&resolver.diagnostics);

    let checker = types::check(&module);
    r.type_diags = checker.diagnostics.iter().map(|d| d.to_string()).collect();
    total += count_errors(&checker.diagnostics);

    let borrow_diags = borrowck::check_module(&module);
    r.borrow_diags = borrow_diags.iter().map(|d| d.to_string()).collect();
    total += count_errors(&borrow_diags);

    for kind in ["Net", "Train", "Kb"] {
        for item in module.items.iter().filter(|i| kind_name(&i.kind) == kind) {
            let p = pass_ai(item);
            r.ai_diags.extend(p.diags.iter().map(|d| d.to_string()));
            r.ai_errors += p.count;
            r.ai_info.extend(p.notes);
        }
    }
    total += r.ai_errors;

    let eff = effects::infer_effects(&module);
    r.effect_diags = eff.diagnostics.iter().map(|d| d.to_string()).collect();
    total += count_errors(&eff.diagnostics);
    let mut inferred: Vec<(&String, &EffectSet)> = eff.inferred.iter().collect();
    inferred.sort_by(|a, b| a.0.cmp(b.0));
    r.effects = inferred.into_iter().map(|(n, fx)| (n.clone(), fx.iter().map(|e| e.to_string()).collect())).collect();

    let results = verify::verify_module(&module);
    r.contract_total = results.iter().map(|v| v.checks.len()).sum();
    let mut tally = verdict::Tally::default();
    for v in &results {
        tally.add(&v.status.evidence(&v.fqn));
    }
    r.verdict_summary = tally.summary();
    r.verify_lines = results
        .iter()
        .map(|v| format!("{} {}: {}", v.status.symbol(), v.fqn, v.status.evidence(&v.fqn).describe()))
        .collect();
    let vdiags = verify::diagnostics(&results);
    r.verify_diags = vdiags.iter().map(|d| d.to_string()).collect();
    total += vdiags.len();

    let text = mlir::emit(&module, &eff);
    r.mlir_lines = text.lines().count();
    r.mlir_sha256 = hex(&digest(&[text.as_bytes()]));

    let mut all: Vec<Diagnostic> = resolver.diagnostics.clone();
    all.extend(checker.diagnostics.iter().cloned());
    all.extend(borrow_diags.iter().cloned());
    all.extend(eff.diagnostics.iter().cloned());
    let healed = heal::heal(&all);
    r.heal_diagnostics = healed.len();
    r.heal_fixes = healed.iter().map(|h| h.fixes.len()).sum();
    r.total_errors = total;
    r
}

// ── Content addresses ────────────────────────────────────────────────

fn h_of(parts: &[&[u8]]) -> Hash32 {
    digest(parts)
}

fn addr(db: &M, key: &Key) -> Result<Option<Hash32>, QueryError> {
    Ok(Some(match key {
        Key::Source(_) | Key::Settings | Key::Item(..) | Key::ItemHash(..) => return Ok(None),
        Key::Tokens(f) => h_of(&[&db.get_hash(&Key::Source(*f))?]),
        Key::Parsed(f) => h_of(&[&db.get_hash(&Key::Tokens(*f))?]),
        Key::Elided(f) => {
            let st = settings(db)?;
            h_of(&[&db.get_hash(&Key::Parsed(*f))?, &[st.elision as u8]])
        }
        Key::Items(f) | Key::Names(f) | Key::Types(f) | Key::Effects(f) | Key::ResolveContracts(f) => {
            let mut parts: Vec<Hash32> = vec![db.get_hash(&Key::Elided(*f))?];
            if matches!(key, Key::ResolveContracts(_)) {
                parts.push(db.get_hash(&Key::Names(*f))?);
            }
            let refs: Vec<&[u8]> = parts.iter().map(|h| h.as_slice()).collect();
            h_of(&refs)
        }
        Key::ResolveItem(f, id) => {
            let ih = item_hash(db, *f, id)?;
            h_of(&[ih.as_bytes(), &db.get_hash(&Key::Names(*f))?])
        }
        Key::ItemPass(f, p, id) => {
            let ih = item_hash(db, *f, id)?;
            let def = &db.ctx.items[*p];
            h_of(&[ih.as_bytes(), def.name.as_bytes(), def.version.as_bytes()])
        }
        Key::ModulePass(f, p) => {
            let def = &db.ctx.modules[*p];
            h_of(&[&db.get_hash(&Key::Elided(*f))?, def.name.as_bytes(), def.version.as_bytes()])
        }
        Key::BorrowSummary(f) => h_of(&[&db.get_hash(&Key::Elided(*f))?, b"borrowck/summary", BORROWCK_VERSION.as_bytes()]),
        Key::BorrowItem(f, id) => {
            let ih = item_hash(db, *f, id)?;
            h_of(&[ih.as_bytes(), &db.get_hash(&Key::BorrowSummary(*f))?, b"pass/borrowck", BORROWCK_VERSION.as_bytes()])
        }
        Key::Borrowck(f) => {
            let items = match db.get(&Key::Items(*f))? {
                Val::Items(i) => i,
                other => return Err(mismatch(key, &other)),
            };
            let mut parts: Vec<Hash32> = vec![db.get_hash(&Key::Items(*f))?];
            for i in items.iter() {
                parts.push(db.get_hash(&Key::BorrowItem(*f, i.id.clone()))?);
            }
            let refs: Vec<&[u8]> = parts.iter().map(|h| h.as_slice()).collect();
            h_of(&refs)
        }
        Key::Diff(f) | Key::Mlir(f) => {
            h_of(&[&db.get_hash(&Key::Elided(*f))?, &db.get_hash(&Key::Effects(*f))?])
        }
        Key::Resolve(f) => {
            let items = match db.get(&Key::Items(*f))? {
                Val::Items(i) => i,
                other => return Err(mismatch(key, &other)),
            };
            let mut parts: Vec<Hash32> = vec![db.get_hash(&Key::Names(*f))?, db.get_hash(&Key::Items(*f))?];
            for i in items.iter() {
                parts.push(db.get_hash(&Key::ResolveItem(*f, i.id.clone()))?);
            }
            parts.push(db.get_hash(&Key::ResolveContracts(*f))?);
            let refs: Vec<&[u8]> = parts.iter().map(|h| h.as_slice()).collect();
            h_of(&refs)
        }
        Key::Diagnostics(f) | Key::Report(f) => {
            let st = settings(db)?;
            let items = match db.get(&Key::Items(*f))? {
                Val::Items(i) => i,
                other => return Err(mismatch(key, &other)),
            };
            let mut parts: Vec<Hash32> = vec![db.get_hash(&Key::Elided(*f))?, db.get_hash(&Key::Items(*f))?];
            for k in [Key::Resolve(*f), Key::Types(*f), Key::Borrowck(*f), Key::Effects(*f), Key::Diff(*f)] {
                parts.push(db.get_hash(&k)?);
            }
            for k in pass_keys(db, *f, &items, st.core) {
                parts.push(db.get_hash(&k)?);
            }
            if matches!(key, Key::Diagnostics(_)) {
                let shape = db.ctx.item_index("shape");
                for i in items.iter() {
                    parts.push(db.get_hash(&Key::ItemPass(*f, shape, i.id.clone()))?);
                }
            } else {
                parts.push(db.get_hash(&Key::Tokens(*f))?);
                parts.push(db.get_hash(&Key::Parsed(*f))?);
                parts.push(db.get_hash(&Key::Mlir(*f))?);
                parts.push(db.get_hash(&Key::Heal(*f))?);
                for name in ["ai", "verify"] {
                    let p = db.ctx.item_index(name);
                    for i in items.iter() {
                        parts.push(db.get_hash(&Key::ItemPass(*f, p, i.id.clone()))?);
                    }
                }
            }
            let flags = [st.core as u8, st.elision as u8];
            let mut refs: Vec<&[u8]> = parts.iter().map(|h| h.as_slice()).collect();
            refs.push(&flags);
            h_of(&refs)
        }
        Key::Checked(f) => {
            h_of(&[
                &db.get_hash(&Key::Tokens(*f))?,
                &db.get_hash(&Key::Elided(*f))?,
                &db.get_hash(&Key::Diagnostics(*f))?,
            ])
        }
        Key::Heal(f) => h_of(&[
            &db.get_hash(&Key::Resolve(*f))?,
            &db.get_hash(&Key::Types(*f))?,
            &db.get_hash(&Key::Borrowck(*f))?,
            &db.get_hash(&Key::Effects(*f))?,
        ]),
    }))
}

fn item_hash(db: &M, f: File, id: &ItemId) -> Result<String, QueryError> {
    match db.get(&Key::ItemHash(f, id.clone()))? {
        Val::ItemHash(h) => h.as_ref().clone().ok_or_else(|| QueryError::Failed(format!("no item `{id}`"))),
        other => Err(mismatch(&Key::ItemHash(f, id.clone()), &other)),
    }
}

// ── Encoding ─────────────────────────────────────────────────────────

fn enc<T: Serialize>(t: &T) -> Option<String> {
    serde_json::to_string(t).ok()
}

fn dec<T: DeserializeOwned>(p: &str) -> Option<T> {
    serde_json::from_str(p).ok()
}

/// A parse outcome plus the one thing its JSON drops. serde skips `line` on kb
/// facts and rules (the published AST shape has no such field), and `logic`
/// reads it for diagnostics. They travel beside the tree and are put back.
#[derive(Serialize, Deserialize)]
struct AstRecord {
    outcome: ParseOutcome,
    kb_lines: Vec<usize>,
}

fn kb_lines(items: &mut [ast::Item], f: &mut dyn FnMut(&mut usize)) {
    for item in items {
        match &mut item.kind {
            ItemKind::Kb(def) => {
                for fact in &mut def.facts {
                    f(&mut fact.line);
                }
                for rule in &mut def.rules {
                    f(&mut rule.line);
                }
            }
            ItemKind::Module(m) => {
                if let Some(items) = &mut m.items {
                    kb_lines(items, f);
                }
            }
            ItemKind::Impl(b) => kb_lines(&mut b.items, f),
            ItemKind::Extend(b) => kb_lines(&mut b.items, f),
            ItemKind::Trait(t) => kb_lines(&mut t.items, f),
            _ => {}
        }
    }
}

fn encode_outcome(o: &ParseOutcome) -> Option<String> {
    let mut copy = o.clone();
    let mut lines = Vec::new();
    if let Some(m) = &mut copy.module {
        kb_lines(&mut m.items, &mut |l| lines.push(*l));
    }
    enc(&AstRecord { outcome: copy, kb_lines: lines })
}

fn decode_outcome(p: &str) -> Option<ParseOutcome> {
    let mut rec: AstRecord = dec(p)?;
    if let Some(m) = &mut rec.outcome.module {
        let mut it = rec.kb_lines.iter();
        let mut short = false;
        kb_lines(&mut m.items, &mut |l| match it.next() {
            Some(v) => *l = *v,
            None => short = true,
        });
        if short || it.next().is_some() {
            return None;
        }
    }
    Some(rec.outcome)
}

fn encode(key: &Key, val: &Val) -> Option<String> {
    match (key, val) {
        (Key::Tokens(_), Val::Tokens(v)) => enc(&**v),
        (Key::Parsed(_) | Key::Elided(_), Val::Outcome(v)) => encode_outcome(v),
        (Key::Items(_), Val::Items(v)) => enc(&**v),
        (Key::Names(_), Val::Names(v)) => enc(&**v),
        (Key::ResolveItem(..) | Key::ResolveContracts(_) | Key::ItemPass(..) | Key::ModulePass(..) | Key::BorrowItem(..) | Key::Borrowck(_) | Key::Types(_) | Key::Diff(_), Val::Pass(v)) => {
            enc(&**v)
        }
        (Key::Resolve(_), Val::Resolve(v)) => enc(&**v),
        (Key::BorrowSummary(_), Val::Summary(v)) => enc(&**v),
        (Key::Effects(_), Val::Effects(v)) => enc(&**v),
        (Key::Mlir(_), Val::Text(v)) => enc(&**v),
        (Key::Diagnostics(_), Val::Diag(v)) => enc(&**v),
        (Key::Checked(_), Val::Checked(v)) => enc(&**v),
        (Key::Heal(_), Val::Heal(v)) => enc(&**v),
        (Key::Report(_), Val::Report(v)) => enc(&**v),
        _ => None,
    }
}

fn decode(key: &Key, p: &str) -> Option<Val> {
    Some(match key {
        Key::Tokens(_) => Val::Tokens(Arc::new(dec(p)?)),
        Key::Parsed(_) | Key::Elided(_) => Val::Outcome(Arc::new(decode_outcome(p)?)),
        Key::Items(_) => Val::Items(Arc::new(dec(p)?)),
        Key::Names(_) => Val::Names(Arc::new(dec(p)?)),
        Key::ResolveItem(..) | Key::ResolveContracts(_) | Key::ItemPass(..) | Key::ModulePass(..) | Key::BorrowItem(..) | Key::Borrowck(_) | Key::Types(_) | Key::Diff(_) => {
            Val::Pass(Arc::new(dec(p)?))
        }
        Key::Resolve(_) => Val::Resolve(Arc::new(dec(p)?)),
        Key::BorrowSummary(_) => Val::Summary(Arc::new(dec(p)?)),
        Key::Effects(_) => Val::Effects(Arc::new(dec(p)?)),
        Key::Mlir(_) => Val::Text(Arc::new(dec(p)?)),
        Key::Diagnostics(_) => Val::Diag(Arc::new(dec(p)?)),
        Key::Checked(_) => Val::Checked(Arc::new(dec(p)?)),
        Key::Heal(_) => Val::Heal(Arc::new(dec(p)?)),
        Key::Report(_) => Val::Report(Arc::new(dec(p)?)),
        _ => return None,
    })
}

// ── The compiler ─────────────────────────────────────────────────────

/// How to build a [`Compiler`].
pub struct Config {
    /// Where answers persist between runs; `None` keeps them in memory only.
    pub cache_dir: Option<std::path::PathBuf>,
    /// Overrides [`build_id`] (tests use it to stand in for another binary).
    pub build_id: Option<String>,
    /// An already-open cache to use instead of opening `cache_dir` (many
    /// compilers in one process share one in-memory index this way).
    pub cache: Option<Arc<DiskCache>>,
    pub registry: Registry,
}

impl Default for Config {
    fn default() -> Self {
        Config { cache_dir: None, build_id: None, cache: None, registry: Registry::builtin() }
    }
}

/// The incremental compiler.
pub struct Compiler {
    db: M,
    files: std::cell::RefCell<Vec<String>>,
}

impl Compiler {
    pub fn new(cfg: Config) -> Compiler {
        let mut db = Db::<Mage>::new(cfg.registry);
        if let Some(shared) = &cfg.cache {
            let id = cfg.build_id.clone().unwrap_or_else(|| build_id(&policy_id()));
            db = db.with_store(Box::new(shared.clone()), id);
        } else if let Some(dir) = &cfg.cache_dir {
            match DiskCache::open(dir) {
                Ok(c) => {
                    let id = cfg.build_id.clone().unwrap_or_else(|| build_id(&policy_id()));
                    db = db.with_store(Box::new(c), id);
                }
                // A cache that cannot be opened is a cache that is not there.
                Err(e) => eprintln!("mage: cache directory {}: {e} — running without it", dir.display()),
            }
        }
        let c = Compiler { db, files: std::cell::RefCell::new(Vec::new()) };
        c.set_settings(Settings { elision: true, core: false });
        c
    }

    /// An in-memory compiler with the built-in passes.
    pub fn in_memory() -> Compiler {
        Compiler::new(Config::default())
    }

    /// One persistent at `dir`.
    pub fn at(dir: &Path) -> Compiler {
        Compiler::new(Config { cache_dir: Some(dir.to_path_buf()), ..Config::default() })
    }

    pub fn set_settings(&self, s: Settings) {
        self.db.set_input(Key::Settings, Val::Settings(s));
    }

    /// Set a file's text, naming it if new. Setting identical text changes nothing.
    pub fn set_source(&self, name: &str, text: &str) -> File {
        let id = {
            let mut files = self.files.borrow_mut();
            match files.iter().position(|n| n == name) {
                Some(i) => File(i as u32),
                None => {
                    files.push(name.to_string());
                    File(files.len() as u32 - 1)
                }
            }
        };
        self.db.set_input(Key::Source(id), Val::Source(Arc::new(text.to_string())));
        id
    }

    pub fn file_name(&self, f: File) -> String {
        self.files.borrow()[f.0 as usize].clone()
    }

    pub fn db(&self) -> &M {
        &self.db
    }

    pub fn stats(&self) -> Stats {
        self.db.stats()
    }

    pub fn reset_stats(&self) {
        self.db.reset_stats();
    }

    pub fn revision(&self) -> u64 {
        self.db.revision()
    }

    /// Make every stored answer durable (a process that leaves with
    /// `std::process::exit` skips destructors, so it must call this).
    pub fn flush(&self) -> bool {
        self.db.flush()
    }

    pub fn tokens(&self, f: File) -> Result<Arc<Vec<Token>>, QueryError> {
        Ok(fetch!(self.db, Key::Tokens(f), Tokens))
    }

    /// The parse, before elision.
    pub fn parsed(&self, f: File) -> Result<Arc<ParseOutcome>, QueryError> {
        Ok(fetch!(self.db, Key::Parsed(f), Outcome))
    }

    /// The parse, after elision when it is on.
    pub fn elided(&self, f: File) -> Result<Arc<ParseOutcome>, QueryError> {
        outcome(&self.db, f)
    }

    pub fn items(&self, f: File) -> Result<Arc<Vec<ItemInfo>>, QueryError> {
        Ok(fetch!(self.db, Key::Items(f), Items))
    }

    pub fn resolve(&self, f: File) -> Result<Arc<ResolveOut>, QueryError> {
        Ok(fetch!(self.db, Key::Resolve(f), Resolve))
    }

    pub fn type_diagnostics(&self, f: File) -> Result<Arc<PassOut>, QueryError> {
        Ok(fetch!(self.db, Key::Types(f), Pass))
    }

    /// The borrow checker's diagnostics for the whole module, assembled from the
    /// per-item checks.
    pub fn borrowck(&self, f: File) -> Result<Arc<PassOut>, QueryError> {
        Ok(fetch!(self.db, Key::Borrowck(f), Pass))
    }

    pub fn effects(&self, f: File) -> Result<Arc<EffectsOut>, QueryError> {
        Ok(fetch!(self.db, Key::Effects(f), Effects))
    }

    pub fn diagnostics(&self, f: File) -> Result<Arc<DiagOut>, QueryError> {
        Ok(fetch!(self.db, Key::Diagnostics(f), Diag))
    }

    /// `check::check_source`, incrementally.
    pub fn checked(&self, f: File) -> Result<Checked, QueryError> {
        Ok(fetch!(self.db, Key::Checked(f), Checked).to_checked())
    }

    pub fn checked_rec(&self, f: File) -> Result<Arc<CheckedRec>, QueryError> {
        Ok(fetch!(self.db, Key::Checked(f), Checked))
    }

    pub fn mlir(&self, f: File) -> Result<Arc<String>, QueryError> {
        Ok(fetch!(self.db, Key::Mlir(f), Text))
    }

    pub fn heal(&self, f: File) -> Result<Arc<Vec<heal::HealedDiagnostic>>, QueryError> {
        Ok(fetch!(self.db, Key::Heal(f), Heal))
    }

    /// What `run_pipeline` computes.
    pub fn report(&self, f: File) -> Result<Arc<PipelineReport>, QueryError> {
        Ok(fetch!(self.db, Key::Report(f), Report))
    }

    /// Ask for every output an agent reads (tokens, AST, diagnostics, MLIR,
    /// the pipeline report), in dependency-friendly order.
    pub fn compile(&self, f: File) -> Result<Arc<PipelineReport>, QueryError> {
        self.tokens(f)?;
        self.parsed(f)?;
        self.elided(f)?;
        self.checked(f)?;
        self.mlir(f)?;
        self.report(f)
    }
}

/// The policy in force, as a string for the build id: the digest of the SKB
/// rule set the environment selects.
fn policy_id() -> String {
    match crate::skb::RuleSet::from_env() {
        Ok(set) => set.digest().to_string(),
        Err(e) => format!("unloadable: {e}"),
    }
}

/// Per-query statistics as the JSON `--incremental-stats` prints.
pub fn stats_json(stats: &Stats, files: &[(String, usize)]) -> serde_json::Value {
    let total: QueryStats = stats.total();
    serde_json::json!({
        "revision": stats.revision,
        "files": files.iter().map(|(n, items)| serde_json::json!({ "file": n, "items": items })).collect::<Vec<_>>(),
        "total": total,
        "queries": stats.queries,
        "events": stats.events,
        "events_dropped": stats.events_dropped,
    })
}
