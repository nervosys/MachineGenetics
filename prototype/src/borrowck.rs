//! Ownership, move and borrow checking: E0382 (`UseAfterMove`) and E0502
//! (`BorrowConflict`).
//!
//! # What this is
//!
//! A **pure, per-function** pass over the AST. [`ModuleInfo::new`] summarises
//! what the *module* says about types and signatures (which user types are
//! `Copy`, what each function's parameters take by value), and
//! [`check_function`] then analyses one function body against that summary and
//! returns its diagnostics. Nothing is cached, nothing is global, and one
//! function's result depends only on its own body and the summary, so an
//! incremental query engine can call `check_function` once per function and
//! memoise on (function body, `ModuleInfo` of the signatures it touches).
//! [`check_module`] is the whole-module convenience over both.
//!
//! The pass runs on the AST **after** safety elision. Elision rewrites `&!T`
//! *types* to `&T` (so a `&!T` parameter reads as a shared reference here, which
//! is `Copy`: never moved), but leaves the *expressions* `&x` and `&!x` alone,
//! and those are the only borrow sites this pass looks at.
//!
//! # Precision rule
//!
//! A false positive is worse than a miss. Every rule below therefore reports
//! only on evidence: a value is moved only if its type is *known* non-`Copy`
//! (see [`Copyness`]); a call moves its arguments only if the callee is known
//! to take them by value; a borrow conflicts only when both places and the
//! holder's later use are syntactically visible. Anything else is assumed
//! fine.
//!
//! # Moves
//!
//! Each local and parameter is `Init`, `Moved` or `MaybeMoved` (a `let` always
//! has an initialiser in MAGE, so there is no uninitialised state to track).
//! States are joined over `if`/`else`, `match` arms, `&&`/`||`, `guard`; a
//! branch that cannot fall through (`ret`, `break`, `continue`, `todo`) is
//! unreachable and joins as the identity, so `if c { consume(x); ret }` does
//! not poison `x`. Loops are iterated to a fixpoint so a move in the body is
//! seen at the top of the next iteration. Assigning to a moved local
//! re-initialises it.
//!
//! What moves a non-`Copy` local: passing it to a user function whose
//! parameter is by value (or to a data constructor, `Some`/`Ok`/`Err`/`Box`/
//! `drop`, or the by-value builtins `push`/`insert`/`send`), `let y = x`,
//! assigning it, returning it (`ret x` or a tail expression), putting it in a
//! struct/tuple/array/map literal, `?`/`.await` on it, and a closure whose
//! body consumes it.
//!
//! # Borrows (NLL-lite, no lifetimes)
//!
//! `let r = &p` / `let r = &!p` records a loan on place `p` held by `r`. The
//! loan is live from that `let` until the last mention of `r` in the rest of
//! its block (an evaluation-order position, so `if c { use(r); x = 1 }` is
//! fine). While live, these conflict: `&!p` again, `&!p` under a shared loan,
//! `&p` under a mutable loan, moving `p`, assigning `p`. Places are a local
//! plus a field path, so `&!p.a` and `&!p.b` do not conflict. Within one call,
//! `f(&!x, &x)` / `f(&!x, &!x)` / `f(&x, x)` are flagged as temporaries.
//!
//! # Explicit limits
//!
//! * Spans: the AST has none, so diagnostics carry the function and variable
//!   in the message and `span: None`.
//! * Only whole locals are moved. Partial moves (`x.f` moved out) are not
//!   tracked; `x` stays usable.
//! * Loans are created only by `let r = &place` (and copied by `let r2 = r`).
//!   References are not followed through calls, returns, struct fields,
//!   collections or `r = &y` reassignment (which only ends `r`'s old loans).
//! * A method call's receiver is never a move and never a conflicting borrow
//!   (the method's `self` mode is unknown), and `for x in v` / `match v` do not
//!   move `v` (MAGE infers borrow mode there).
//! * A use order inside one expression is approximated: only calls extend a
//!   holder's last use to the end of the call.
//! * `defer` bodies are analysed where written, as reads, not at scope exit.
//! * A loan held across a loop back-edge is not detected when the conflicting
//!   action follows the last use inside the body.
//! * Only `f`s and methods are checked; `net`/`train`/`kb`/`swarm` bodies are
//!   not.
//! * Assignment to an immutable binding is `types.rs`'s check (it reports
//!   "cannot assign to `x`: it is bound with `val`"); it is not repeated here.

use crate::ast::{
    self, Block, DataKind, Expr, FunctionDef, Item, ItemKind, LiteralKind, Module, Pattern, Stmt,
    Type,
};
use crate::hir::{Diagnostic, DiagnosticCategory, Severity};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use serde::{Deserialize, Serialize};
use std::rc::Rc;

// ── Copy-ness ────────────────────────────────────────────────────────

/// What is known about whether a value is `Copy`. Only `NonCopy` can be moved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Copyness {
    Copy,
    NonCopy,
    /// Not known. Treated as `Copy`: nothing is reported.
    Unknown,
}

impl Copyness {
    /// A product (tuple, `Result`): non-`Copy` if any part is, `Copy` if every
    /// part is, otherwise unknown.
    fn all(parts: impl IntoIterator<Item = Copyness>) -> Copyness {
        let mut out = Copyness::Copy;
        for p in parts {
            match p {
                Copyness::NonCopy => return Copyness::NonCopy,
                Copyness::Unknown => out = Copyness::Unknown,
                Copyness::Copy => {}
            }
        }
        out
    }
}

const PRIMITIVES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f16",
    "bf16", "f32", "f64", "bool", "char", "byte", "unit",
];
const OWNING_PATHS: &[&str] = &[
    "str", "String", "Vec", "Box", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "VecDeque", "Rc",
    "Arc", "RefCell", "Mutex", "RwLock", "Cell",
];
/// Callees that take every argument by value.
const BY_VALUE_CTORS: &[&str] = &["Some", "Ok", "Err", "Box", "Rc", "Arc", "drop"];
/// Methods that take every argument by value, whoever the receiver is.
const BY_VALUE_METHODS: &[&str] = &["push", "push_back", "push_front", "insert", "send"];

// ── What the module says ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TypeInfo {
    derives_copy: bool,
}

/// What counts as a move. MAGE's evaluator has value semantics and the spec
/// says borrow mode is "inferred from usage context", while the example corpus
/// freely reuses values Rust would consider moved (`live: seed, previous: seed`,
/// `assign(tasks)` then `filter(tasks, ..)`). The default policy therefore
/// reports only transfers of ownership the program itself evidences; the strict
/// policy is Rust's rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    /// A call moves an argument only if the callee's body really consumes that
    /// parameter (a fixpoint over the module). When `false`, any parameter
    /// declared by value is taken to consume its argument, as in Rust.
    pub infer_call_modes: bool,
    /// Storing a value in a struct/tuple/array/map literal, a data
    /// constructor, `Some`/`Ok`/`Err`, or a collection (`push`, `insert`,
    /// `send`) moves it.
    pub aggregate_moves: bool,
}

impl Policy {
    /// Rust's rule: by-value parameters and aggregates consume.
    pub const STRICT: Policy = Policy { infer_call_modes: false, aggregate_moves: true };
    /// Only evidenced transfers: `let y = x`, `ret x`, assignment, `?`, a
    /// closure that consumes, and a call whose callee consumes the parameter.
    pub const INFERRED: Policy = Policy { infer_call_modes: true, aggregate_moves: false };
}

impl Default for Policy {
    fn default() -> Self {
        Policy::INFERRED
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FnSig {
    generics: Vec<String>,
    /// `true` for a parameter whose type is a reference (not moved by a call).
    param_is_ref: Vec<bool>,
    /// `true` for a parameter the body moves (computed under `infer_call_modes`).
    consumes: Vec<bool>,
    /// The leading `self` parameter is not part of a method call's arguments.
    has_self: bool,
    ret: Option<Type>,
}

/// The facts about a module that `check_function` needs. Build it once per
/// module (or per change to a signature) and share it across functions.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    policy: Policy,
    /// `None` marks a name defined twice: nothing is assumed about it.
    types: HashMap<String, Option<TypeInfo>>,
    aliases: HashMap<String, Option<Type>>,
    fns: HashMap<String, Option<FnSig>>,
    methods: HashMap<String, Vec<FnSig>>,
    /// Variant (or record) constructor name -> its type.
    ctors: HashMap<String, Option<String>>,
}

fn derives_copy(item: &Item) -> bool {
    item.attributes
        .iter()
        .any(|a| matches!(a.name.as_str(), "d" | "derive") && a.args.iter().any(|x| x == "Copy"))
}

fn sig_of(f: &FunctionDef) -> FnSig {
    FnSig {
        generics: f.generics.iter().map(|g| g.name.clone()).collect(),
        param_is_ref: f.params.iter().map(|p| matches!(p.ty, Type::Reference { .. })).collect(),
        consumes: vec![false; f.params.len()],
        has_self: f.params.first().is_some_and(|p| p.name == "self"),
        ret: f.return_type.clone(),
    }
}

fn insert_unique<V>(map: &mut HashMap<String, Option<V>>, name: &str, v: V) {
    match map.get(name) {
        None => {
            map.insert(name.to_string(), Some(v));
        }
        Some(_) => {
            map.insert(name.to_string(), None);
        }
    }
}

fn type_name(t: &Type) -> Option<String> {
    match t {
        Type::Path { segments, .. } => Some(segments.join(".")),
        _ => None,
    }
}

enum Slot {
    Free(String),
    Method(String, usize),
}

/// Every function in `items` in `collect`'s order, with where its signature lives.
fn for_each_fn(
    items: &[Item],
    in_impl: bool,
    counters: &mut HashMap<String, usize>,
    f: &mut dyn FnMut(Slot, &FunctionDef),
) {
    for item in items {
        match &item.kind {
            ItemKind::Function(def) => {
                if in_impl {
                    let i = counters.entry(def.name.clone()).or_insert(0);
                    f(Slot::Method(def.name.clone(), *i), def);
                    *i += 1;
                } else {
                    f(Slot::Free(def.name.clone()), def);
                }
            }
            ItemKind::Impl(i) => for_each_fn(&i.items, true, counters, f),
            ItemKind::Extend(e) => for_each_fn(&e.items, true, counters, f),
            ItemKind::Trait(t) => for_each_fn(&t.items, true, counters, f),
            ItemKind::Module(m) => {
                if let Some(items) = &m.items {
                    for_each_fn(items, in_impl, counters, f);
                }
            }
            _ => {}
        }
    }
}

impl ModuleInfo {
    /// The summary under the default ([`Policy::INFERRED`]) policy.
    pub fn new(m: &Module) -> Self {
        Self::with_policy(m, Policy::default())
    }

    pub fn with_policy(m: &Module, policy: Policy) -> Self {
        let mut info = ModuleInfo { policy, ..ModuleInfo::default() };
        info.collect(&m.items, false);
        if policy.infer_call_modes {
            info.infer_consumption(m);
        }
        info
    }

    /// Which parameters each function really consumes: the least fixpoint of
    /// "the body moves it", where passing it to a parameter that is consumed
    /// counts. Starts from "nothing is consumed", so a cycle or an unknown
    /// callee can only lose a finding, never invent one.
    fn infer_consumption(&mut self, m: &Module) {
        for _ in 0..8 {
            let mut updates: Vec<(Slot, Vec<bool>)> = Vec::new();
            let mut counters: HashMap<String, usize> = HashMap::new();
            for_each_fn(&m.items, false, &mut counters, &mut |slot, f| {
                let moved = moved_params(self, f);
                let flags: Vec<bool> = f.params.iter().map(|p| moved.contains(&p.name)).collect();
                updates.push((slot, flags));
            });
            let mut changed = false;
            for (slot, flags) in updates {
                let sig = match &slot {
                    Slot::Free(n) => self.fns.get_mut(n).and_then(|s| s.as_mut()),
                    Slot::Method(n, i) => self.methods.get_mut(n).and_then(|v| v.get_mut(*i)),
                };
                if let Some(sig) = sig {
                    for (c, f) in sig.consumes.iter_mut().zip(flags) {
                        if f && !*c {
                            *c = true;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn collect(&mut self, items: &[Item], in_impl: bool) {
        for item in items {
            match &item.kind {
                ItemKind::Function(f) => {
                    if in_impl {
                        self.methods.entry(f.name.clone()).or_default().push(sig_of(f));
                    } else {
                        insert_unique(&mut self.fns, &f.name, sig_of(f));
                    }
                }
                ItemKind::Struct(s) => {
                    insert_unique(&mut self.types, &s.name, TypeInfo { derives_copy: derives_copy(item) });
                }
                ItemKind::Enum(e) => {
                    insert_unique(&mut self.types, &e.name, TypeInfo { derives_copy: derives_copy(item) });
                    for v in &e.variants {
                        insert_unique(&mut self.ctors, &v.name, e.name.clone());
                    }
                }
                ItemKind::Data(d) => {
                    insert_unique(&mut self.types, &d.name, TypeInfo { derives_copy: derives_copy(item) });
                    match &d.kind {
                        DataKind::Record(_) => insert_unique(&mut self.ctors, &d.name, d.name.clone()),
                        DataKind::Sum(vs) => {
                            for v in vs {
                                insert_unique(&mut self.ctors, &v.name, d.name.clone());
                            }
                        }
                    }
                }
                ItemKind::TypeAlias(t) if t.generics.is_empty() => {
                    insert_unique(&mut self.aliases, &t.name, t.ty.clone());
                }
                ItemKind::Impl(i) => self.collect(&i.items, true),
                ItemKind::Extend(e) => self.collect(&e.items, true),
                ItemKind::Trait(t) => self.collect(&t.items, true),
                ItemKind::Module(m) => {
                    if let Some(items) = &m.items {
                        self.collect(items, in_impl);
                    }
                }
                _ => {}
            }
        }
    }

    /// Whether a value of `ty` is `Copy`, as far as the module says.
    pub fn type_copyness(&self, ty: &Type, generics: &[String]) -> Copyness {
        self.tc(ty, generics, 0)
    }

    fn tc(&self, ty: &Type, generics: &[String], depth: u32) -> Copyness {
        if depth > 8 {
            return Copyness::Unknown;
        }
        let rec = |t: &Type| self.tc(t, generics, depth + 1);
        match ty {
            Type::Path { segments, type_args } => {
                let Some(name) = segments.last() else { return Copyness::Unknown };
                if segments.len() == 1 && generics.contains(name) {
                    return Copyness::Unknown;
                }
                if let Some(t) = self.types.get(name) {
                    return match t {
                        None => Copyness::Unknown,
                        Some(t) if t.derives_copy => {
                            // `@d(Copy)` on a generic type is Copy only for Copy arguments.
                            Copyness::all(type_args.iter().map(&rec)).min_known()
                        }
                        Some(_) => Copyness::NonCopy,
                    };
                }
                if let Some(a) = self.aliases.get(name) {
                    return a.as_ref().map_or(Copyness::Unknown, |t| rec(t));
                }
                let n = name.as_str();
                if PRIMITIVES.contains(&n) {
                    Copyness::Copy
                } else if OWNING_PATHS.contains(&n) {
                    Copyness::NonCopy
                } else if n == "Option" {
                    type_args.first().map_or(Copyness::Unknown, rec)
                } else if n == "Result" {
                    Copyness::all(type_args.iter().map(rec))
                } else {
                    Copyness::Unknown
                }
            }
            Type::Reference { mutable: false, .. } => Copyness::Copy,
            Type::Reference { mutable: true, .. } => Copyness::Unknown,
            Type::OwnedPtr { .. }
            | Type::Rc { .. }
            | Type::Arc { .. }
            | Type::Cow { .. }
            | Type::Cell { .. }
            | Type::RefCell { .. }
            | Type::Mutex { .. }
            | Type::RwLock { .. }
            | Type::Vec { .. }
            | Type::Set { .. }
            | Type::Map { .. }
            | Type::StringType => Copyness::NonCopy,
            Type::Array { inner, .. } | Type::Option { inner } => rec(inner),
            Type::Tuple { elements } => Copyness::all(elements.iter().map(rec)),
            Type::Result { ok, err } => Copyness::all([rec(ok), rec(err)]),
            Type::Ptr { .. } | Type::Simd { .. } => Copyness::Copy,
            Type::Refined { base, .. } | Type::Dimensioned { base, .. } => rec(base),
            // Tensors, policies, slices, fn types, `_`, `Self`: not known.
            _ => Copyness::Unknown,
        }
    }

    fn user_type_copyness(&self, name: &str) -> Copyness {
        match self.types.get(name) {
            Some(Some(t)) if t.derives_copy => Copyness::Unknown,
            Some(Some(_)) => Copyness::NonCopy,
            _ => Copyness::Unknown,
        }
    }
}

impl Copyness {
    /// `Copy` stays `Copy`; anything less certain is `Unknown`.
    fn min_known(self) -> Copyness {
        if self == Copyness::Copy { Copyness::Copy } else { Copyness::Unknown }
    }
}

// ── Public entry points ──────────────────────────────────────────────

/// Check every function (free, method, nested module) in `m`.
pub fn check_module(m: &Module) -> Vec<Diagnostic> {
    check_module_with(m, Policy::default())
}

/// [`check_module`] under an explicit [`Policy`].
pub fn check_module_with(m: &Module, policy: Policy) -> Vec<Diagnostic> {
    let info = ModuleInfo::with_policy(m, policy);
    let mut out = Vec::new();
    walk_items(&m.items, "", &info, &mut out);
    out
}

fn walk_items(items: &[Item], prefix: &str, info: &ModuleInfo, out: &mut Vec<Diagnostic>) {
    for item in items {
        match &item.kind {
            ItemKind::Function(f) => {
                let name = format!("{prefix}{}", f.name);
                out.extend(check_function(info, &name, f));
            }
            ItemKind::Impl(i) => {
                let p = type_name(&i.self_type).map(|n| format!("{n}.")).unwrap_or_default();
                walk_items(&i.items, &p, info, out);
            }
            ItemKind::Extend(e) => {
                let p = type_name(&e.target_type).map(|n| format!("{n}.")).unwrap_or_default();
                walk_items(&e.items, &p, info, out);
            }
            ItemKind::Trait(t) => walk_items(&t.items, &format!("{}.", t.name), info, out),
            ItemKind::Module(m) => {
                if let Some(items) = &m.items {
                    walk_items(items, &format!("{prefix}{}.", m.name), info, out);
                }
            }
            _ => {}
        }
    }
}

/// Check one top-level item (a function, or an impl/extend/trait/module and
/// every function inside it) against a module summary. Concatenating this over
/// `m.items` in order gives exactly [`check_module_with`]'s output, so an
/// incremental driver can re-check only the items that changed.
pub fn check_item(info: &ModuleInfo, item: &Item) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    walk_items(std::slice::from_ref(item), "", info, &mut out);
    out
}

/// Check one function body. `qualified_name` is only used in messages.
pub fn check_function(info: &ModuleInfo, qualified_name: &str, f: &FunctionDef) -> Vec<Diagnostic> {
    analyse(info, qualified_name, f).0
}

/// The parameters of `f` its body moves.
fn moved_params(info: &ModuleInfo, f: &FunctionDef) -> BTreeSet<String> {
    analyse(info, &f.name, f).1
}

fn analyse(info: &ModuleInfo, qualified_name: &str, f: &FunctionDef) -> (Vec<Diagnostic>, BTreeSet<String>) {
    let mut nums = Nums::default();
    nums.block(&f.body);
    if let Some(e) = &f.body_expr {
        nums.expr(e);
    }
    let generics: Vec<String> = f.generics.iter().map(|g| g.name.clone()).collect();
    let mut fcx = Fcx {
        info,
        fname: qualified_name.to_string(),
        generics,
        nums,
        diags: Vec::new(),
        seen: BTreeSet::new(),
        loops: Vec::new(),
        closures: Vec::new(),
        scopes: Vec::new(),
        nested: Vec::new(),
        moved_params: BTreeSet::new(),
    };
    let mut st = State { reachable: true, vars: BTreeMap::new(), loans: BTreeSet::new() };
    fcx.push_scope();
    for p in &f.params {
        let copy = if p.name == "self" {
            Copyness::Unknown
        } else {
            info.type_copyness(&p.ty, &fcx.generics)
        };
        fcx.bind(&mut st, &p.name, copy);
        if let Some(v) = st.vars.get_mut(&p.name) {
            v.is_param = true;
        }
    }
    let ret = Use::Move(Rc::from("returned from the function"));
    fcx.exec_block(&f.body, &mut st, &ret);
    if let Some(e) = &f.body_expr {
        fcx.eval(e, &mut st, &ret);
    }
    let mut diags = std::mem::take(&mut fcx.diags);
    // Nested `f` items are functions in their own right.
    for (n, g) in std::mem::take(&mut fcx.nested) {
        diags.extend(check_function(info, &format!("{qualified_name}.{n}"), g));
    }
    (diags, fcx.moved_params)
}

// ── Positions ────────────────────────────────────────────────────────
//
// The AST has no spans or node ids, so an expression is identified by its
// address (stable: the pass only borrows the tree) and numbered in evaluation
// order. Numbers are doubled so an assignment can act *after* its right-hand
// side (`x = *r + 1` ends `r`'s loan before `x` is written).

fn key(e: &Expr) -> usize {
    e as *const Expr as usize
}

#[derive(Default)]
struct Nums {
    /// address -> (position of the node, position just after its subtree)
    pos: HashMap<usize, (u32, u32)>,
    /// (start, end) of every call, to extend a holder's last use to the call's end
    calls: Vec<(u32, u32)>,
    counter: u32,
}

enum Child<'a> {
    E(&'a Expr),
    B(&'a Block),
}

fn children<'a>(e: &'a Expr, f: &mut dyn FnMut(Child<'a>)) {
    use Child::{B, E};
    match e {
        Expr::Literal { .. }
        | Expr::Ident { .. }
        | Expr::Continue
        | Expr::Todo
        | Expr::Unimplemented
        | Expr::Error { .. } => {}
        Expr::Binary { left, right, .. } => {
            f(E(left));
            f(E(right));
        }
        Expr::Unary { operand, .. } => f(E(operand)),
        Expr::Call { func, args } => {
            f(E(func));
            args.iter().for_each(|a| f(E(a)));
        }
        Expr::MethodCall { receiver, args, .. } => {
            f(E(receiver));
            args.iter().for_each(|a| f(E(a)));
        }
        Expr::FieldAccess { object, .. } => f(E(object)),
        Expr::Index { object, index } => {
            f(E(object));
            f(E(index));
        }
        Expr::StructLit { fields, .. } => {
            fields.iter().filter_map(|x| x.value.as_ref()).for_each(|v| f(E(v)));
        }
        Expr::TupleLit { elements } | Expr::ArrayLit { elements } => {
            elements.iter().for_each(|a| f(E(a)));
        }
        Expr::ArrayRepeat { value, count } => {
            f(E(value));
            f(E(count));
        }
        Expr::MapLit { entries } => {
            for (k, v) in entries {
                f(E(k));
                f(E(v));
            }
        }
        Expr::Closure { body, .. } => f(E(body)),
        Expr::If { cond, then_block, else_block } => {
            f(E(cond));
            f(B(then_block));
            if let Some(b) = else_block {
                f(B(b));
            }
        }
        Expr::Match { scrutinee, arms } => {
            if let Some(s) = scrutinee {
                f(E(s));
            }
            arms.iter().for_each(|a| f(E(&a.body)));
        }
        Expr::Handle { body, arms, .. } => {
            f(B(body));
            arms.iter().for_each(|a| f(E(&a.body)));
        }
        Expr::Loop { body } => f(B(body)),
        Expr::While { cond, body } => {
            f(E(cond));
            f(B(body));
        }
        Expr::For { iter, body, .. } => {
            f(E(iter));
            f(B(body));
        }
        Expr::Block { block } => f(B(block)),
        Expr::Return { value } | Expr::Break { value } => {
            if let Some(v) = value {
                f(E(v));
            }
        }
        Expr::Try { expr } | Expr::Await { expr } | Expr::Cast { expr, .. } | Expr::Is { expr, .. } => {
            f(E(expr))
        }
        Expr::Assign { target, value } => {
            f(E(target));
            f(E(value));
        }
        Expr::Range { start, end, .. } => {
            f(E(start));
            f(E(end));
        }
        Expr::Pipeline { left, right } => {
            f(E(left));
            f(E(right));
        }
        Expr::Grad { value, wrt } => {
            f(E(value));
            f(E(wrt));
        }
    }
}

impl Nums {
    fn block(&mut self, b: &Block) {
        for s in &b.stmts {
            match s {
                Stmt::Let { value, .. } => self.expr(value),
                Stmt::Expr { expr } | Stmt::Defer { expr } => self.expr(expr),
                Stmt::Guard { cond, else_block } => {
                    self.expr(cond);
                    self.block(else_block);
                }
                Stmt::Item { .. } => {}
            }
        }
        if let Some(t) = &b.tail_expr {
            self.expr(t);
        }
    }

    fn expr(&mut self, e: &Expr) {
        let i = self.counter;
        self.counter += 1;
        children(e, &mut |c| match c {
            Child::E(x) => self.expr(x),
            Child::B(b) => self.block(b),
        });
        let range = (2 * i, 2 * (self.counter - 1) + 1);
        self.pos.insert(key(e), range);
        if matches!(e, Expr::Call { .. } | Expr::MethodCall { .. }) {
            self.calls.push(range);
        }
    }

    fn start(&self, e: &Expr) -> u32 {
        self.pos.get(&key(e)).map_or(0, |p| p.0)
    }

    /// Just after the whole subtree of `e`: where an assignment takes effect.
    fn after(&self, e: &Expr) -> u32 {
        self.pos.get(&key(e)).map_or(0, |p| p.1)
    }

    /// A mention at `q` is a use until the end of the innermost call around it.
    fn extend_to_call_end(&self, q: u32) -> u32 {
        self.calls
            .iter()
            .filter(|(s, e)| *s <= q && q <= *e)
            .max_by_key(|(s, _)| *s)
            .map_or(q, |(_, e)| *e)
    }
}

// ── Patterns and names ───────────────────────────────────────────────

fn pattern_names(p: &Pattern, out: &mut Vec<String>) {
    match p {
        Pattern::Ident { name } => out.push(name.clone()),
        Pattern::Literal { .. } | Pattern::Wildcard => {}
        Pattern::Tuple { elements } | Pattern::Enum { elements, .. } => {
            elements.iter().for_each(|x| pattern_names(x, out));
        }
        Pattern::Struct { fields, .. } => {
            for f in fields {
                match &f.pattern {
                    Some(p) => pattern_names(p, out),
                    None => out.push(f.name.clone()),
                }
            }
        }
        Pattern::Slice { elements, rest_name, .. } => {
            elements.iter().for_each(|x| pattern_names(x, out));
            out.extend(rest_name.iter().cloned());
        }
        Pattern::Or { patterns } => {
            if let Some(first) = patterns.first() {
                pattern_names(first, out);
            }
        }
        Pattern::Ref { pattern } => pattern_names(pattern, out),
    }
}

fn pattern_binds(p: &Pattern, name: &str) -> bool {
    let mut v = Vec::new();
    pattern_names(p, &mut v);
    v.iter().any(|n| n == name)
}

/// The names a format string interpolates: `f"{x} and {y.z}"`.
fn fmt_idents(s: &str) -> Vec<String> {
    let b: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '{' {
            if b.get(i + 1) == Some(&'{') {
                i += 2;
                continue;
            }
            let mut j = i + 1;
            while j < b.len() && (b[j].is_alphanumeric() || b[j] == '_') {
                j += 1;
            }
            if j > i + 1 {
                out.push(b[i + 1..j].iter().collect());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

// ── Places ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    root: String,
    path: Vec<String>,
}

impl Place {
    fn overlaps(&self, o: &Place) -> bool {
        self.root == o.root && {
            let n = self.path.len().min(o.path.len());
            self.path[..n] == o.path[..n]
        }
    }

    fn show(&self) -> String {
        let mut s = self.root.clone();
        for p in &self.path {
            s.push('.');
            s.push_str(p);
        }
        s
    }
}

/// `x`, `x.a.b`, `x[i]` (the index ends the path: any element may be meant).
fn place_of(e: &Expr) -> Option<Place> {
    fn go(e: &Expr) -> Option<(Place, bool)> {
        match e {
            Expr::Ident { name } => Some((Place { root: name.clone(), path: Vec::new() }, false)),
            Expr::FieldAccess { object, field } => {
                let (mut p, closed) = go(object)?;
                if !closed {
                    p.path.push(field.clone());
                }
                Some((p, closed))
            }
            Expr::Index { object, .. } => {
                let (p, _) = go(object)?;
                Some((p, true))
            }
            _ => None,
        }
    }
    go(e).map(|(p, _)| p)
}

// ── Analysis state ───────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Own {
    Init,
    Moved,
    MaybeMoved,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Var {
    own: Own,
    copy: Copyness,
    moved_by: Option<String>,
    /// How many closures deep it was declared (0 = the function body).
    depth: usize,
    /// A parameter of the function being analysed (and not shadowed).
    is_param: bool,
    /// The parameter this local was copied from by `let y = x`, if any.
    origin: Option<String>,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Loan {
    holder: String,
    place: Place,
    mutable: bool,
    from: u32,
    until: u32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct State {
    reachable: bool,
    vars: BTreeMap<String, Var>,
    loans: BTreeSet<Loan>,
}

impl State {
    fn join(&mut self, other: &State) {
        if !other.reachable {
            return;
        }
        if !self.reachable {
            *self = other.clone();
            return;
        }
        for (k, v) in self.vars.iter_mut() {
            if let Some(o) = other.vars.get(k) {
                if v.own != o.own {
                    v.own = Own::MaybeMoved;
                }
                v.moved_by = match (v.moved_by.take(), o.moved_by.clone()) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
            }
        }
        for (k, o) in &other.vars {
            self.vars.entry(k.clone()).or_insert_with(|| o.clone());
        }
        self.loans.extend(other.loans.iter().cloned());
    }
}

/// How an expression's value is used. `Move` carries why, for the message.
#[derive(Clone)]
enum Use {
    Read,
    /// The value is handed on: returned, passed to a consuming callee, closed over.
    Move(Rc<str>),
    /// The value is rebound to another local (`let y = x`). `x` is moved as far
    /// as this function can tell, but nothing escapes: it counts as the
    /// function consuming its parameter only if `y` is itself moved on.
    Alias(Rc<str>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    BorrowShared,
    BorrowMut,
    Move,
    Assign,
}

#[derive(Default)]
struct LoopCtx {
    breaks: Vec<State>,
    continues: Vec<State>,
}

#[derive(Default)]
struct ClosureFrame {
    moved: BTreeSet<String>,
}

struct Fcx<'a> {
    info: &'a ModuleInfo,
    fname: String,
    generics: Vec<String>,
    nums: Nums,
    diags: Vec<Diagnostic>,
    seen: BTreeSet<String>,
    loops: Vec<LoopCtx>,
    closures: Vec<ClosureFrame>,
    /// Per open block, the bindings it introduced and what each shadowed.
    scopes: Vec<Vec<(String, Option<Var>)>>,
    nested: Vec<(&'a str, &'a FunctionDef)>,
    /// Parameters moved at some point (read by `infer_consumption`).
    moved_params: BTreeSet<String>,
}

#[derive(Clone, Copy)]
enum LoopKind<'b> {
    Loop(&'b Block),
    While(&'b Expr, &'b Block),
    For(&'b Pattern, &'b Block),
}

fn mv(why: impl AsRef<str>) -> Use {
    Use::Move(Rc::from(why.as_ref()))
}

fn alias(why: impl AsRef<str>) -> Use {
    Use::Alias(Rc::from(why.as_ref()))
}

impl<'a> Fcx<'a> {
    // ── Reporting ────────────────────────────────────────────────────

    fn report(&mut self, cat: DiagnosticCategory, message: String) {
        if self.seen.insert(message.clone()) {
            self.diags.push(Diagnostic {
                severity: Severity::Error,
                message,
                span: None,
                id: Some(cat.code().to_string()),
                category: Some(cat),
            });
        }
    }

    // ── Scopes ───────────────────────────────────────────────────────

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self, st: &mut State) {
        for (name, prev) in self.scopes.pop().unwrap_or_default().into_iter().rev() {
            match prev {
                Some(v) => {
                    st.vars.insert(name, v);
                }
                None => {
                    st.vars.remove(&name);
                }
            }
        }
    }

    fn bind(&mut self, st: &mut State, name: &str, copy: Copyness) {
        let v = Var { own: Own::Init, copy, moved_by: None, depth: self.closures.len(), is_param: false, origin: None };
        let prev = st.vars.insert(name.to_string(), v);
        if let Some(top) = self.scopes.last_mut() {
            top.push((name.to_string(), prev));
        }
    }

    fn bind_pattern(&mut self, st: &mut State, p: &Pattern) {
        let mut names = Vec::new();
        pattern_names(p, &mut names);
        for n in names {
            self.bind(st, &n, Copyness::Unknown);
        }
    }

    // ── Blocks and statements ────────────────────────────────────────

    fn exec_block(&mut self, b: &'a Block, st: &mut State, tail: &Use) {
        self.push_scope();
        for (i, s) in b.stmts.iter().enumerate() {
            if !st.reachable {
                break;
            }
            match s {
                Stmt::Let { mutable: _, pattern, ty, value } => {
                    self.exec_let(b, i, pattern, ty.as_ref(), value, st)
                }
                Stmt::Expr { expr } => self.eval(expr, st, &Use::Read),
                Stmt::Defer { expr } => self.eval(expr, st, &Use::Read),
                Stmt::Guard { cond, else_block } => {
                    self.eval(cond, st, &Use::Read);
                    let mut other = st.clone();
                    self.exec_block(else_block, &mut other, &Use::Read);
                    st.join(&other);
                }
                Stmt::Item { item } => {
                    if let ItemKind::Function(f) = &item.kind {
                        self.nested.push((&f.name, f));
                    }
                }
            }
        }
        if st.reachable {
            if let Some(t) = &b.tail_expr {
                self.eval(t, st, tail);
            }
        }
        self.pop_scope(st);
    }

    fn exec_let(
        &mut self,
        block: &'a Block,
        idx: usize,
        pattern: &'a Pattern,
        ty: Option<&'a Type>,
        value: &'a Expr,
        st: &mut State,
    ) {
        match pattern {
            // `let _ = x` does not move `x`.
            Pattern::Wildcard | Pattern::Ref { .. } => {
                self.eval(value, st, &Use::Read);
                self.bind_pattern(st, pattern);
            }
            Pattern::Ident { name } => {
                let copy = match ty {
                    Some(t) => self.info.type_copyness(t, &self.generics),
                    None => self.infer_copy(value, st),
                };
                // `let y = x`: `y` stands for the parameter `x` came from.
                let origin = match value {
                    Expr::Ident { name: from } => st
                        .vars
                        .get(from)
                        .and_then(|v| if v.is_param { Some(from.clone()) } else { v.origin.clone() }),
                    _ => None,
                };
                self.eval(value, st, &alias(format!("assigned to `{name}`")));
                self.bind(st, name, copy);
                if let Some(v) = st.vars.get_mut(name.as_str()) {
                    v.origin = origin;
                }
                self.record_loans(block, idx, name, value, st);
            }
            _ => {
                self.eval(value, st, &alias("destructured by a `let` pattern"));
                self.bind_pattern(st, pattern);
            }
        }
    }

    /// `let r = &p` / `let r = &!p` / `let r2 = r`: `holder` now holds a loan.
    fn record_loans(&mut self, block: &Block, idx: usize, holder: &str, value: &Expr, st: &mut State) {
        let mut made: Vec<(Place, bool, u32)> = Vec::new();
        match value {
            Expr::Unary { op, operand } if op == "&" || op == "&!" => {
                if let Some(p) = place_of(operand) {
                    if st.vars.contains_key(&p.root) && p.root != holder {
                        made.push((p, op == "&!", self.nums.start(value)));
                    }
                }
            }
            Expr::Ident { name } if name != holder => {
                for l in st.loans.iter().filter(|l| l.holder == *name) {
                    made.push((l.place.clone(), l.mutable, l.from));
                }
            }
            _ => {}
        }
        if made.is_empty() {
            return;
        }
        let until = self.last_use(block, idx + 1, holder);
        for (place, mutable, from) in made {
            st.loans.insert(Loan { holder: holder.to_string(), place, mutable, from, until });
        }
    }

    /// The position of `name`'s last mention in `block[from..]` and its tail.
    fn last_use(&self, block: &Block, from: usize, name: &str) -> u32 {
        let mut out = Vec::new();
        self.mentions_block(block, from, name, &mut out);
        out.into_iter().map(|q| self.nums.extend_to_call_end(q)).max().unwrap_or(0)
    }

    fn mentions_block(&self, b: &Block, from: usize, name: &str, out: &mut Vec<u32>) {
        for s in b.stmts.iter().skip(from) {
            match s {
                Stmt::Let { pattern, value, .. } => {
                    self.mentions_expr(value, name, out);
                    if pattern_binds(pattern, name) {
                        return;
                    }
                }
                Stmt::Expr { expr } | Stmt::Defer { expr } => self.mentions_expr(expr, name, out),
                Stmt::Guard { cond, else_block } => {
                    self.mentions_expr(cond, name, out);
                    self.mentions_block(else_block, 0, name, out);
                }
                Stmt::Item { .. } => {}
            }
        }
        if let Some(t) = &b.tail_expr {
            self.mentions_expr(t, name, out);
        }
    }

    fn mentions_expr(&self, e: &Expr, name: &str, out: &mut Vec<u32>) {
        match e {
            Expr::Ident { name: n } if n == name => out.push(self.nums.start(e)),
            Expr::Literal { value, kind: LiteralKind::FormatString } => {
                if fmt_idents(value).iter().any(|n| n == name) {
                    out.push(self.nums.start(e));
                }
            }
            Expr::StructLit { fields, .. } => {
                if fields.iter().any(|f| f.value.is_none() && f.name == name) {
                    out.push(self.nums.start(e));
                }
                for f in fields.iter().filter_map(|f| f.value.as_ref()) {
                    self.mentions_expr(f, name, out);
                }
            }
            Expr::Closure { params, body } => {
                if !params.iter().any(|p| p.name == name) {
                    self.mentions_expr(body, name, out);
                }
            }
            Expr::Match { scrutinee, arms } => {
                if let Some(s) = scrutinee {
                    self.mentions_expr(s, name, out);
                }
                for a in arms {
                    if !pattern_binds(&a.pattern, name) {
                        self.mentions_expr(&a.body, name, out);
                    }
                }
            }
            Expr::For { pattern, iter, body } => {
                self.mentions_expr(iter, name, out);
                if !pattern_binds(pattern, name) {
                    self.mentions_block(body, 0, name, out);
                }
            }
            Expr::Handle { body, arms, .. } => {
                self.mentions_block(body, 0, name, out);
                for a in arms {
                    if !a.params.iter().any(|p| p == name) {
                        self.mentions_expr(&a.body, name, out);
                    }
                }
            }
            _ => children(e, &mut |c| match c {
                Child::E(x) => self.mentions_expr(x, name, out),
                Child::B(b) => self.mentions_block(b, 0, name, out),
            }),
        }
    }

    // ── Copy-ness of values ──────────────────────────────────────────

    fn infer_copy(&self, e: &Expr, st: &State) -> Copyness {
        match e {
            Expr::Literal { kind, .. } => match kind {
                LiteralKind::Int
                | LiteralKind::Float
                | LiteralKind::Bool
                | LiteralKind::Char
                | LiteralKind::Byte => Copyness::Copy,
                LiteralKind::FormatString => Copyness::NonCopy,
                // `"abc"` may be a borrowed `str`.
                LiteralKind::String => Copyness::Unknown,
            },
            Expr::Ident { name } => st.vars.get(name).map_or(Copyness::Unknown, |v| v.copy),
            Expr::Unary { op, operand } => match op.as_str() {
                "&" => Copyness::Copy,
                "-" | "!" => self.infer_copy(operand, st),
                _ => Copyness::Unknown,
            },
            Expr::Binary { op, .. } => match op.as_str() {
                "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" => Copyness::Copy,
                _ => Copyness::Unknown,
            },
            Expr::Cast { ty, .. } => self.info.type_copyness(ty, &self.generics),
            Expr::Call { func, .. } => match &**func {
                Expr::Ident { name } if !st.vars.contains_key(name) => {
                    if let Some(Some(sig)) = self.info.fns.get(name) {
                        match &sig.ret {
                            Some(t) => self.info.type_copyness(t, &sig.generics),
                            None => Copyness::Copy,
                        }
                    } else if let Some(Some(ty)) = self.info.ctors.get(name) {
                        self.info.user_type_copyness(ty)
                    } else {
                        Copyness::Unknown
                    }
                }
                Expr::FieldAccess { object, .. } => match &**object {
                    Expr::Ident { name } => self.info.user_type_copyness(name),
                    _ => Copyness::Unknown,
                },
                _ => Copyness::Unknown,
            },
            Expr::StructLit { path, .. } => {
                path.last().map_or(Copyness::Unknown, |n| self.info.user_type_copyness(n))
            }
            Expr::TupleLit { elements } => Copyness::all(elements.iter().map(|x| self.infer_copy(x, st))),
            // `[a, b]` may be a fixed array or a vector: certain only if an element is not Copy.
            Expr::ArrayLit { elements } => {
                if elements.iter().any(|x| self.infer_copy(x, st) == Copyness::NonCopy) {
                    Copyness::NonCopy
                } else {
                    Copyness::Unknown
                }
            }
            Expr::MapLit { .. } => Copyness::NonCopy,
            Expr::MethodCall { receiver, method, .. } => match method.as_str() {
                "clone" => self.infer_copy(receiver, st),
                "to_string" => Copyness::NonCopy,
                _ => Copyness::Unknown,
            },
            _ => Copyness::Unknown,
        }
    }

    // ── Uses of locals ───────────────────────────────────────────────

    fn use_local(&mut self, name: &str, at: u32, st: &mut State, mode: &Use) {
        let Some(v) = st.vars.get(name).cloned() else { return };
        match v.own {
            Own::Init => {}
            Own::Moved | Own::MaybeMoved => {
                let how = v.moved_by.clone().unwrap_or_else(|| "moved".into());
                let qual = if v.own == Own::Moved { "" } else { "possibly " };
                self.report(
                    DiagnosticCategory::UseAfterMove,
                    format!(
                        "use of moved value: `{name}` in fn `{}` (value {qual}moved earlier: {how}; it is used again here)",
                        self.fname
                    ),
                );
            }
        }
        let (why, escapes) = match mode {
            Use::Read => (None, false),
            Use::Move(w) => (Some(w), true),
            Use::Alias(w) => (Some(w), false),
        };
        if let Some(why) = why {
            if v.copy == Copyness::NonCopy {
                self.check_action(st, &Place { root: name.to_string(), path: Vec::new() }, Action::Move, at);
                for (k, f) in self.closures.iter_mut().enumerate() {
                    if v.depth <= k {
                        f.moved.insert(name.to_string());
                    }
                }
                if escapes {
                    if v.is_param {
                        self.moved_params.insert(name.to_string());
                    }
                    if let Some(o) = &v.origin {
                        self.moved_params.insert(o.clone());
                    }
                }
                if let Some(var) = st.vars.get_mut(name) {
                    var.own = Own::Moved;
                    var.moved_by = Some(why.to_string());
                }
            }
        }
    }

    /// Does `action` on `place` at position `at` hit a live loan?
    fn check_action(&mut self, st: &State, place: &Place, action: Action, at: u32) {
        let hits: Vec<Loan> = st
            .loans
            .iter()
            .filter(|l| l.place.overlaps(place) && l.from < at && at <= l.until)
            .cloned()
            .collect();
        for l in hits {
            let shown = place.show();
            let (msg, conflicts) = match action {
                Action::BorrowMut if l.mutable => {
                    (format!("cannot borrow `{shown}` as mutable more than once at a time"), true)
                }
                Action::BorrowMut => (
                    format!("cannot borrow `{shown}` as mutable because it is also borrowed as immutable"),
                    true,
                ),
                Action::BorrowShared => (
                    format!("cannot borrow `{shown}` as immutable because it is also borrowed as mutable"),
                    l.mutable,
                ),
                Action::Move => (format!("cannot move out of `{shown}` because it is borrowed"), true),
                Action::Assign => (format!("cannot assign to `{shown}` because it is borrowed"), true),
            };
            if conflicts {
                self.report(
                    DiagnosticCategory::BorrowConflict,
                    format!(
                        "{msg} in fn `{}` (the earlier borrow of `{}` is held by `{}`, which is used later)",
                        self.fname,
                        l.place.show(),
                        l.holder
                    ),
                );
            }
        }
    }

    // ── Calls ────────────────────────────────────────────────────────

    /// For each of `n` arguments: moved (`true`) or only read.
    fn callee_moves(&self, func: &Expr, n: usize, st: &State) -> (String, Vec<bool>) {
        let none = vec![false; n];
        match func {
            Expr::Ident { name } if !st.vars.contains_key(name) => {
                if let Some(entry) = self.info.fns.get(name) {
                    return match entry {
                        Some(sig) => (name.clone(), (0..n).map(|i| self.takes(sig, i)).collect()),
                        None => (name.clone(), none),
                    };
                }
                if self.info.policy.aggregate_moves
                    && (BY_VALUE_CTORS.contains(&name.as_str()) || matches!(self.info.ctors.get(name), Some(Some(_))))
                {
                    return (name.clone(), vec![true; n]);
                }
                (name.clone(), none)
            }
            Expr::FieldAccess { object, field } => match &**object {
                Expr::Ident { name }
                    if self.info.policy.aggregate_moves
                        && !st.vars.contains_key(name)
                        && matches!(self.info.ctors.get(field), Some(Some(t)) if t == name) =>
                {
                    (format!("{name}.{field}"), vec![true; n])
                }
                _ => (field.clone(), none),
            },
            _ => ("a call".to_string(), none),
        }
    }

    /// Does a call hand over its `i`th argument to `sig`'s function?
    fn takes(&self, sig: &FnSig, i: usize) -> bool {
        if self.info.policy.infer_call_modes {
            sig.consumes.get(i) == Some(&true)
        } else {
            sig.param_is_ref.get(i) == Some(&false)
        }
    }

    fn method_moves(&self, method: &str, n: usize) -> Vec<bool> {
        match self.info.methods.get(method) {
            Some(cands) if !cands.is_empty() => (0..n)
                .map(|i| {
                    cands.iter().all(|s| {
                        self.takes(s, i + usize::from(s.has_self))
                    })
                })
                .collect(),
            _ => vec![self.info.policy.aggregate_moves && BY_VALUE_METHODS.contains(&method); n],
        }
    }

    /// How a value put into a literal is used: a move under strict ownership.
    fn aggregate(&self, why: impl AsRef<str>) -> Use {
        if self.info.policy.aggregate_moves { mv(why) } else { Use::Read }
    }

    fn eval_args(&mut self, callee: &str, moves: &[bool], args: &'a [Expr], st: &mut State) {
        for (i, a) in args.iter().enumerate() {
            let mode = if moves.get(i) == Some(&true) {
                mv(format!("passed by value to `{callee}`"))
            } else {
                Use::Read
            };
            self.eval(a, st, &mode);
        }
        self.temporaries(callee, moves, args, st);
    }

    /// Conflicts between the arguments of one call: `f(&!x, &x)`.
    fn temporaries(&mut self, callee: &str, moves: &[bool], args: &[Expr], st: &State) {
        #[derive(Clone, Copy, PartialEq)]
        enum K {
            Shared,
            Mut,
            ByValue,
        }
        let mut temps: Vec<(Place, K)> = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let t = match a {
                Expr::Unary { op, operand } if op == "&" || op == "&!" => {
                    place_of(operand).map(|p| (p, if op == "&!" { K::Mut } else { K::Shared }))
                }
                Expr::Ident { .. } | Expr::FieldAccess { .. } if moves.get(i) == Some(&true) => {
                    place_of(a).map(|p| (p, K::ByValue))
                }
                _ => None,
            };
            if let Some((p, k)) = t {
                if st.vars.contains_key(&p.root) {
                    temps.push((p, k));
                }
            }
        }
        for i in 0..temps.len() {
            for j in i + 1..temps.len() {
                let ((a, ka), (b, kb)) = (&temps[i], &temps[j]);
                if !a.overlaps(b) {
                    continue;
                }
                let shown = if a.path.len() <= b.path.len() { a.show() } else { b.show() };
                let by_value_copy = |p: &Place| {
                    st.vars.get(&p.root).is_some_and(|v| v.copy != Copyness::NonCopy)
                };
                let msg = match (*ka, *kb) {
                    (K::Mut, K::Mut) => format!("cannot borrow `{shown}` as mutable more than once at a time"),
                    (K::Mut, K::Shared) | (K::Shared, K::Mut) => {
                        format!("cannot borrow `{shown}` as immutable because it is also borrowed as mutable")
                    }
                    (K::Mut, K::ByValue) => format!("cannot use `{shown}` while it is mutably borrowed in the same call"),
                    (K::ByValue, K::Mut) => format!("cannot use `{shown}` while it is mutably borrowed in the same call"),
                    (K::Shared, K::ByValue) if !by_value_copy(b) => {
                        format!("cannot move out of `{shown}` because it is borrowed")
                    }
                    (K::ByValue, K::Shared) if !by_value_copy(a) => {
                        format!("cannot move out of `{shown}` because it is borrowed")
                    }
                    _ => continue,
                };
                self.report(
                    DiagnosticCategory::BorrowConflict,
                    format!("{msg} in fn `{}` (arguments of the call to `{callee}`)", self.fname),
                );
            }
        }
    }

    // ── Expressions ──────────────────────────────────────────────────

    fn eval(&mut self, e: &'a Expr, st: &mut State, mode: &Use) {
        if !st.reachable {
            return;
        }
        match e {
            Expr::Literal { value, kind: LiteralKind::FormatString } => {
                for n in fmt_idents(value) {
                    self.use_local(&n, self.nums.start(e), st, &Use::Read);
                }
            }
            Expr::Literal { .. } | Expr::Error { .. } => {}
            Expr::Ident { name } => self.use_local(name, self.nums.start(e), st, mode),
            Expr::Continue => {
                if let Some(l) = self.loops.last_mut() {
                    l.continues.push(st.clone());
                }
                st.reachable = false;
            }
            Expr::Todo | Expr::Unimplemented => st.reachable = false,
            Expr::Binary { op, left, right } => match op.as_str() {
                "&&" | "||" => {
                    self.eval(left, st, &Use::Read);
                    let mut rhs = st.clone();
                    self.eval(right, &mut rhs, &Use::Read);
                    st.join(&rhs);
                }
                "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" => {
                    self.eval(right, st, &Use::Read);
                    self.eval(left, st, &Use::Read);
                    if let Some(p) = place_of(left) {
                        if st.vars.contains_key(&p.root) {
                            let at = self.nums.after(e);
                            self.check_action(st, &p, Action::Assign, at);
                        }
                    }
                }
                _ => {
                    self.eval(left, st, &Use::Read);
                    self.eval(right, st, &Use::Read);
                }
            },
            Expr::Unary { op, operand } => {
                self.eval(operand, st, &Use::Read);
                if op == "&" || op == "&!" {
                    if let Some(p) = place_of(operand) {
                        if st.vars.contains_key(&p.root) {
                            let action = if op == "&!" { Action::BorrowMut } else { Action::BorrowShared };
                            let at = self.nums.start(e);
                            self.check_action(st, &p, action, at);
                        }
                    }
                }
            }
            Expr::Call { func, args } => {
                self.eval(func, st, &Use::Read);
                let (name, moves) = self.callee_moves(func, args.len(), st);
                self.eval_args(&name, &moves, args, st);
            }
            Expr::MethodCall { receiver, method, args, .. } => {
                self.eval(receiver, st, &Use::Read);
                let moves = self.method_moves(method, args.len());
                self.eval_args(method, &moves, args, st);
            }
            Expr::FieldAccess { object, .. } => self.eval(object, st, &Use::Read),
            Expr::Index { object, index } => {
                self.eval(object, st, &Use::Read);
                self.eval(index, st, &Use::Read);
            }
            Expr::StructLit { path, fields } => {
                let sname = path.join(".");
                for f in fields {
                    let why = self.aggregate(format!("moved into field `{}` of `{sname}`", f.name));
                    match &f.value {
                        Some(v) => self.eval(v, st, &why),
                        None => self.use_local(&f.name, self.nums.start(e), st, &why),
                    }
                }
            }
            Expr::TupleLit { elements } => {
                let why = self.aggregate("moved into a tuple");
                elements.iter().for_each(|x| self.eval(x, st, &why));
            }
            Expr::ArrayLit { elements } => {
                let why = self.aggregate("moved into an array");
                elements.iter().for_each(|x| self.eval(x, st, &why));
            }
            Expr::ArrayRepeat { value, count } => {
                self.eval(value, st, &Use::Read);
                self.eval(count, st, &Use::Read);
            }
            Expr::MapLit { entries } => {
                let why = self.aggregate("moved into a map");
                for (k, v) in entries {
                    self.eval(k, st, &why);
                    self.eval(v, st, &why);
                }
            }
            Expr::Closure { params, body } => self.eval_closure(params, body, st),
            Expr::If { cond, then_block, else_block } => {
                self.eval(cond, st, &Use::Read);
                let mut other = st.clone();
                self.exec_block(then_block, st, mode);
                if let Some(b) = else_block {
                    self.exec_block(b, &mut other, mode);
                }
                st.join(&other);
            }
            Expr::Match { scrutinee, arms } => {
                if let Some(s) = scrutinee {
                    self.eval(s, st, &Use::Read);
                }
                if arms.is_empty() {
                    return;
                }
                let base = st.clone();
                let mut out: Option<State> = None;
                for arm in arms {
                    let mut s = base.clone();
                    self.push_scope();
                    self.bind_pattern(&mut s, &arm.pattern);
                    self.eval(&arm.body, &mut s, mode);
                    self.pop_scope(&mut s);
                    match &mut out {
                        Some(o) => o.join(&s),
                        None => out = Some(s),
                    }
                }
                if let Some(o) = out {
                    *st = o;
                }
            }
            Expr::Handle { body, arms, .. } => {
                let base = st.clone();
                self.exec_block(body, st, mode);
                for arm in arms {
                    let mut s = base.clone();
                    self.push_scope();
                    for p in &arm.params {
                        self.bind(&mut s, p, Copyness::Unknown);
                    }
                    self.eval(&arm.body, &mut s, &Use::Read);
                    self.pop_scope(&mut s);
                    st.join(&s);
                }
            }
            Expr::Loop { body } => self.eval_loop(LoopKind::Loop(body), st),
            Expr::While { cond, body } => self.eval_loop(LoopKind::While(cond, body), st),
            Expr::For { pattern, iter, body } => {
                self.eval(iter, st, &Use::Read);
                self.eval_loop(LoopKind::For(pattern, body), st);
            }
            Expr::Block { block } => self.exec_block(block, st, mode),
            Expr::Return { value } => {
                if let Some(v) = value {
                    self.eval(v, st, &mv("returned"));
                }
                st.reachable = false;
            }
            Expr::Break { value } => {
                if let Some(v) = value {
                    self.eval(v, st, &mv("returned by `break`"));
                }
                if let Some(l) = self.loops.last_mut() {
                    l.breaks.push(st.clone());
                }
                st.reachable = false;
            }
            Expr::Try { expr } => self.eval(expr, st, &mv("consumed by `?`")),
            Expr::Await { expr } => self.eval(expr, st, &mv("consumed by `.await`")),
            Expr::Cast { expr, .. } | Expr::Is { expr, .. } => self.eval(expr, st, &Use::Read),
            Expr::Assign { target, value } => self.eval_assign(e, target, value, st),
            Expr::Range { start, end, .. } => {
                self.eval(start, st, &Use::Read);
                self.eval(end, st, &Use::Read);
            }
            Expr::Pipeline { left, right } => self.eval_pipeline(left, right, st),
            Expr::Grad { value, wrt } => {
                self.eval(value, st, &Use::Read);
                self.eval(wrt, st, &Use::Read);
            }
        }
    }

    fn eval_assign(&mut self, e: &'a Expr, target: &'a Expr, value: &'a Expr, st: &mut State) {
        self.eval(value, st, &alias("assigned"));
        let after = self.nums.after(e);
        match target {
            Expr::Ident { name } => {
                if st.vars.contains_key(name) {
                    self.check_action(st, &Place { root: name.clone(), path: Vec::new() }, Action::Assign, after);
                    if let Some(v) = st.vars.get_mut(name) {
                        v.own = Own::Init;
                        v.moved_by = None;
                        v.origin = None;
                    }
                    // Whatever `name` held, it holds no longer.
                    st.loans.retain(|l| l.holder != *name);
                }
            }
            _ => {
                // `x.f = v` and `x[i] = v` use `x`: assigning into a moved value is an error.
                self.eval(target, st, &Use::Read);
                if let Some(p) = place_of(target) {
                    if st.vars.contains_key(&p.root) {
                        self.check_action(st, &p, Action::Assign, after);
                    }
                }
            }
        }
    }

    fn eval_pipeline(&mut self, left: &'a Expr, right: &'a Expr, st: &mut State) {
        // `x |> f(a)` is `f(x, a)`.
        match right {
            Expr::Call { func, args } => {
                self.eval(func, st, &Use::Read);
                let (name, moves) = self.callee_moves(func, args.len() + 1, st);
                let lmode = if moves[0] { mv(format!("passed by value to `{name}`")) } else { Use::Read };
                self.eval(left, st, &lmode);
                self.eval_args(&name, &moves[1..], args, st);
            }
            Expr::Ident { name } if !st.vars.contains_key(name) => {
                let (name, moves) = self.callee_moves(right, 1, st);
                let lmode = if moves[0] { mv(format!("passed by value to `{name}`")) } else { Use::Read };
                self.eval(left, st, &lmode);
            }
            _ => {
                self.eval(left, st, &Use::Read);
                self.eval(right, st, &Use::Read);
            }
        }
    }

    fn eval_closure(&mut self, params: &'a [ast::Param], body: &'a Expr, st: &mut State) {
        let mut cs = st.clone();
        self.closures.push(ClosureFrame::default());
        self.push_scope();
        for p in params {
            let copy = self.info.type_copyness(&p.ty, &self.generics);
            self.bind(&mut cs, &p.name, copy);
        }
        // `break`/`continue` inside the body do not reach this function's loops.
        let outer_loops = std::mem::take(&mut self.loops);
        self.eval(body, &mut cs, &mv("returned from a closure"));
        self.loops = outer_loops;
        self.pop_scope(&mut cs);
        let frame = self.closures.pop().unwrap_or_default();
        // A body that consumes a captured variable takes it by move.
        for name in frame.moved {
            if let Some(v) = st.vars.get_mut(&name) {
                v.own = Own::Moved;
                v.moved_by = Some("captured by a closure that consumes it".to_string());
            }
        }
    }

    fn eval_loop(&mut self, kind: LoopKind<'a>, st: &mut State) {
        let entry = st.clone();
        let mut head = entry.clone();
        let mut exit = State { reachable: false, ..entry.clone() };
        let mut breaks: Vec<State> = Vec::new();
        for _ in 0..8 {
            let mut s = head.clone();
            self.loops.push(LoopCtx::default());
            match kind {
                LoopKind::Loop(body) => {
                    self.exec_block(body, &mut s, &Use::Read);
                    exit = State { reachable: false, ..entry.clone() };
                }
                LoopKind::While(cond, body) => {
                    self.eval(cond, &mut s, &Use::Read);
                    exit = s.clone();
                    self.exec_block(body, &mut s, &Use::Read);
                }
                LoopKind::For(pat, body) => {
                    exit = s.clone();
                    self.push_scope();
                    self.bind_pattern(&mut s, pat);
                    self.exec_block(body, &mut s, &Use::Read);
                    self.pop_scope(&mut s);
                }
            }
            let ctx = self.loops.pop().unwrap_or_default();
            let mut next = entry.clone();
            next.join(&s);
            for c in &ctx.continues {
                next.join(c);
            }
            breaks = ctx.breaks;
            if next == head {
                break;
            }
            head = next;
        }
        for b in &breaks {
            exit.join(b);
        }
        *st = exit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{elision, lexer, parser};

    /// Under Rust's rule: a by-value parameter and an aggregate consume.
    fn run(src: &str) -> Vec<Diagnostic> {
        run_with(src, Policy::STRICT)
    }

    fn run_with(src: &str, policy: Policy) -> Vec<Diagnostic> {
        let module = parser::parse(&lexer::lex(src)).unwrap_or_else(|e| panic!("does not parse: {e:?}\n{src}"));
        check_module_with(&elision::elide(&module), policy)
    }

    fn msgs(src: &str) -> Vec<String> {
        run(src).into_iter().map(|d| d.message).collect()
    }

    fn clean(src: &str) {
        let m = msgs(src);
        assert!(m.is_empty(), "expected no findings, got {m:?}\n{src}");
    }

    fn flags(src: &str, code: &str, needle: &str) {
        let d = run(src);
        assert!(
            d.iter().any(|d| d.id.as_deref() == Some(code) && d.message.contains(needle)),
            "expected {code} containing {needle:?}, got {:?}\n{src}",
            d.iter().map(|d| (&d.id, &d.message)).collect::<Vec<_>>()
        );
    }

    const PRELUDE: &str = "f take(x: s) { }\nf peek(x: &s) { }\nf two(a: &!s, b: &s) { }\nf two_mut(a: &!s, b: &!s) { }\n";

    /// A function `host(a: s, n: i32)` with `body`, after a prelude of helpers.
    fn with(body: &str) -> String {
        format!("{PRELUDE}f host(a: s, n: i32) {{\n{body}\n}}\n")
    }

    // ── moves ────────────────────────────────────────────────────────

    #[test]
    fn definite_use_after_move() {
        let src = with("take(a)\ntake(a)");
        flags(&src, "E0382", "use of moved value: `a`");
        let d = run(&src);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].category, Some(DiagnosticCategory::UseAfterMove));
        assert_eq!(d[0].severity, Severity::Error);
        assert!(d[0].message.contains("fn `host`"), "{}", d[0].message);
    }

    #[test]
    fn let_and_return_move() {
        flags(&with("v b = a\ntake(a)"), "E0382", "`a`");
        flags(&with("take(a)\nret a"), "E0382", "`a`");
    }

    #[test]
    fn a_move_in_one_if_arm_then_use() {
        flags(&with("if n > 0 { take(a) }\ntake(a)"), "E0382", "possibly moved");
        // Both arms moving it is a definite move.
        flags(&with("if n > 0 { take(a) } else { take(a) }\ntake(a)"), "E0382", "`a`");
    }

    #[test]
    fn a_diverging_arm_does_not_poison() {
        clean(&with("if n > 0 {\n take(a)\n ret\n}\ntake(a)"));
    }

    #[test]
    fn a_loop_carried_move() {
        let src = with("m k = 0\nwhile k < n {\n take(a)\n k = k + 1\n}");
        flags(&src, "E0382", "`a`");
        // Moved and then left the loop by `break`: not seen again.
        clean(&with("loop {\n take(a)\n break\n}"));
    }

    #[test]
    fn reinit_after_move() {
        clean(&with("m b = a\ntake(b)\nb = \"again\"\ntake(b)"));
        // Re-initialised in the loop before the next iteration.
        clean(&with("m b = a\nm k = 0\nwhile k < n {\n take(b)\n b = \"x\"\n k = k + 1\n}"));
    }

    #[test]
    fn copy_types_are_never_flagged() {
        clean("f take_i(x: i32) { }\nf host(n: i32, fl: f64, b: bool, ch: char, r: &s, pr: (i32, f64), arr: [i32; 3]) {\n take_i(n)\n take_i(n)\n take_i(n)\n}\n");
        clean("f use_r(r: &s) { }\nf host(r: &s) {\n use_r(r)\n use_r(r)\n}\n");
        clean("f use_t(p: (i32, f64)) { }\nf host(p: (i32, f64)) {\n use_t(p)\n use_t(p)\n}\n");
        clean("@d(Copy)\ndata P(x: f64, y: f64)\nf use_p(p: P) { }\nf host(p: P) {\n use_p(p)\n use_p(p)\n}\n");
    }

    #[test]
    fn unknown_types_are_not_flagged() {
        // `T` is generic and `Mystery` is declared nowhere.
        clean("f take_t[T](x: T) { }\nf host[T](x: T) {\n take_t(x)\n take_t(x)\n}\n");
        clean("f take_m(x: Mystery) { }\nf host(x: Mystery) {\n take_m(x)\n take_m(x)\n}\n");
        // An unknown callee is not known to take its argument by value.
        clean("f host(a: s) {\n unknown_fn(a)\n unknown_fn(a)\n}\n");
        // Its type is inferred from a call to something unknown.
        clean("f take_m(x: Mystery) { }\nf host() {\n v x = make()\n take_m(x)\n take_m(x)\n}\n");
    }

    #[test]
    fn a_borrowing_parameter_does_not_move() {
        clean(&with("peek(&a)\npeek(&a)\ntake(a)"));
    }

    #[test]
    fn a_closure_that_consumes_takes_the_value() {
        flags(&with("v cl = |z| take(a)\ntake(a)"), "E0382", "`a`");
        // One that only reads does not.
        clean(&with("v cl = |z| peek(&a)\ntake(a)"));
    }

    #[test]
    fn moving_into_a_struct_literal() {
        let src = "data Holder(inner: s)\nf take(x: s) { }\nf host(a: s) {\n v b = @Holder { inner: a }\n take(a)\n}\n";
        flags(src, "E0382", "`a`");
    }

    #[test]
    fn a_shadowing_binding_is_a_new_value() {
        clean(&with("take(a)\nv a = \"fresh\"\ntake(a)"));
        // The inner `a` ends with its block; the outer one is untouched.
        clean(&with("if n > 0 {\n v a = \"inner\"\n take(a)\n}\ntake(a)"));
    }

    // ── borrows ──────────────────────────────────────────────────────

    #[test]
    fn mut_borrow_while_shared_is_live() {
        let src = with("v r = &a\nv w = &!a\npeek(r)");
        flags(&src, "E0502", "cannot borrow `a` as mutable because it is also borrowed as immutable");
        let d = run(&src);
        assert_eq!(d[0].category, Some(DiagnosticCategory::BorrowConflict));
    }

    #[test]
    fn the_same_without_overlap_is_fine() {
        // The shared borrow's last use is before the mutable one.
        clean(&with("v r = &a\npeek(r)\nv w = &!a\npeek(w)"));
        clean(&with("v r = &a\nv q = &a\npeek(r)\npeek(q)"));
    }

    #[test]
    fn shared_borrow_while_mut_is_live() {
        flags(
            &with("v w = &!a\nv r = &a\npeek(w)"),
            "E0502",
            "cannot borrow `a` as immutable because it is also borrowed as mutable",
        );
    }

    #[test]
    fn two_mut_borrows_overlap() {
        flags(&with("v w = &!a\nv x = &!a\npeek(w)\npeek(x)"), "E0502", "more than once");
    }

    #[test]
    fn moving_or_assigning_while_borrowed() {
        flags(&with("v r = &a\ntake(a)\npeek(r)"), "E0502", "cannot move out of `a`");
        flags(&with("m b = \"x\"\nv r = &b\nb = \"y\"\npeek(r)"), "E0502", "cannot assign to `b`");
        // After the last use, or with the old value read first, it is fine.
        clean(&with("m b = \"x\"\nv r = &b\npeek(r)\nb = \"y\""));
        clean(&with("m b = 1\nv r = &b\nb = *r + 1"));
    }

    #[test]
    fn a_double_mut_borrow_in_one_call() {
        flags(&with("two_mut(&!a, &!a)"), "E0502", "more than once");
        flags(&with("two(&!a, &a)"), "E0502", "also borrowed as mutable");
        flags(&with("two(&a, &!a)"), "E0502", "also borrowed as mutable");
        // Different variables do not.
        clean(&with("v b = \"y\"\ntwo(&!a, &b)"));
    }

    #[test]
    fn disjoint_fields_do_not_conflict() {
        let ok = "data Pair(a: s, b: s)\nf peek(x: &s) { }\nf host(p: Pair) {\n v x = &!p.a\n v y = &!p.b\n peek(x)\n peek(y)\n}\n";
        clean(ok);
        let bad = "data Pair(a: s, b: s)\nf peek(x: &s) { }\nf host(p: Pair) {\n v x = &!p.a\n v y = &!p\n peek(x)\n}\n";
        flags(bad, "E0502", "more than once");
    }

    #[test]
    fn a_borrow_used_only_in_one_branch_is_not_live_after_it() {
        clean(&with("v r = &a\nif n > 0 {\n peek(r)\n take(a)\n}"));
    }

    // ── plumbing ─────────────────────────────────────────────────────

    #[test]
    fn methods_are_checked_under_their_type() {
        let src = "data P(x: s)\nextend P {\n f eat(self, y: s) { }\n f twice(self, a: s) {\n  self.eat(a)\n  self.eat(a)\n }\n}\n";
        flags(src, "E0382", "fn `P.twice`");
    }

    #[test]
    fn diagnostics_are_deduplicated() {
        let d = run(&with("take(a)\ntake(a)\ntake(a)"));
        assert_eq!(d.len(), 1, "{d:?}");
    }

    #[test]
    fn check_function_is_pure_over_its_inputs() {
        let src = with("take(a)\ntake(a)");
        let module = parser::parse(&lexer::lex(&src)).unwrap();
        let info = ModuleInfo::with_policy(&module, Policy::STRICT);
        let host = module
            .items
            .iter()
            .find_map(|i| match &i.kind {
                ast::ItemKind::Function(f) if f.name == "host" => Some(f),
                _ => None,
            })
            .unwrap();
        let first: Vec<_> = check_function(&info, "host", host).into_iter().map(|d| d.message).collect();
        let again: Vec<_> = check_function(&info, "host", host).into_iter().map(|d| d.message).collect();
        assert_eq!(first, again);
        assert_eq!(first.len(), 1);
    }

    // ── the default policy: only evidenced transfers ─────────────────

    fn dflt(src: &str) -> Vec<String> {
        run_with(src, Policy::default()).into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn the_default_policy_is_inferred() {
        assert_eq!(Policy::default(), Policy::INFERRED);
    }

    #[test]
    fn a_callee_that_only_reads_does_not_take_ownership() {
        // `take` ignores its parameter, so MAGE's inferred borrow mode lends it.
        assert!(dflt(&with("take(a)\ntake(a)")).is_empty());
        // Rust's rule would move it.
        assert!(!run(&with("take(a)\ntake(a)")).is_empty());
    }

    #[test]
    fn a_callee_that_consumes_takes_ownership() {
        let src = "f keep(x: s) -> s { x }\nf host(a: s) {\n keep(a)\n keep(a)\n}\n";
        let m = dflt(src);
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(m[0].contains("use of moved value: `a`"), "{m:?}");
        // Through another function, to a fixpoint.
        let two = "f keep(x: s) -> s { x }\nf relay(y: s) -> s { keep(y) }\nf host(a: s) {\n relay(a)\n relay(a)\n}\n";
        assert_eq!(dflt(two).len(), 1, "{:?}", dflt(two));
        // Mutual recursion terminates and, with no consuming base case, consumes nothing.
        let cyc = "f ping(x: s) { pong(x) }\nf pong(y: s) { ping(y) }\nf host(a: s) {\n ping(a)\n ping(a)\n}\n";
        assert!(dflt(cyc).is_empty());
    }

    #[test]
    fn let_assign_and_return_still_move_by_default() {
        assert_eq!(dflt("f host(a: s) -> s {\n v b = a\n a\n}\n").len(), 1);
        assert_eq!(dflt("f host(a: s) {\n m b = \"x\"\n b = a\n v c = a\n}\n").len(), 1);
    }

    #[test]
    fn aggregates_move_only_under_strict_ownership() {
        let src = "data Holder(inner: s, copy: s)\nf host(a: s) {\n v h = @Holder { inner: a, copy: a }\n}\n";
        assert!(dflt(src).is_empty());
        assert!(!run(src).is_empty());
    }

    #[test]
    fn rebinding_a_parameter_locally_is_not_consuming_it() {
        // `var rem = t` is a snapshot (the program mutates `rem`, not the caller's `t`).
        let local = "f work(t: s) {\n var rem = t\n rem = \"x\"\n}\nf host(a: s) {\n work(a)\n work(a)\n}\n";
        assert!(dflt(local).is_empty(), "{:?}", dflt(local));
        // Reassigned before it is returned: no longer the parameter.
        let escapes = "f work(t: s) -> s {\n var rem = t\n rem = \"x\"\n rem\n}\nf host(a: s) {\n work(a)\n work(a)\n}\n";
        assert!(dflt(escapes).is_empty(), "reassigned, so no longer the parameter: {:?}", dflt(escapes));
        let direct = "f work(t: s) -> s {\n var rem = t\n rem\n}\nf host(a: s) {\n work(a)\n work(a)\n}\n";
        assert_eq!(dflt(direct).len(), 1, "{:?}", dflt(direct));
    }
}
