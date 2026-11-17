//! Ownership, move and borrow checking: E0382 (`UseAfterMove`), E0502
//! (`BorrowConflict`) and E0507 (`MoveOutOfBorrow`).
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
//! Move state is per **place**: a local plus a field path (`p`, `p.name`,
//! `p.inner.x`). Moving a non-`Copy` field moves only that place; its siblings
//! stay usable, while a use of the field, of any extension of it, or of the
//! whole value (`p`) is E0382. Moving `p` moves every `p.*`. Assigning `p.name`
//! re-initialises it (and its extensions). A field is moved only if its type is
//! *known* non-`Copy` from the struct or `data` declaration (a field of a
//! generic type, of an unknown type, or of a variable whose own type is unknown
//! is never moved). Copy fields never move.
//! States are joined over `if`/`else`, `match` arms, `&&`/`||`, `guard`; a
//! branch that cannot fall through (`ret`, `break`, `continue`, `todo`) is
//! unreachable and joins as the identity, so `if c { consume(x); ret }` does
//! not poison `x`. Loops are iterated to a fixpoint so a move in the body is
//! seen at the top of the next iteration. Assigning to a moved local
//! re-initialises it.
//!
//! # `self`, receivers and patterns
//!
//! Inside a method, `self` is a place of the impl's type (a user struct or
//! `data` type, not a generic or trait `Self`). A by-value `self` can be moved
//! out of and partially moved like any parameter (`self.name`). A `&self` /
//! `&!self` receiver is a reference: nothing is moved out of it.
//!
//! `x.m(..)` where `x` is a place of a known user type and `m` is declared on
//! that type with `self` by value moves `x` (strict: always; inferred: only if
//! `m`'s body really consumes `self`), with `&self` / `&!self` borrows `x`
//! shared / exclusive for the call.
//!
//! MAGE has no by-reference binding form in patterns (no `ref`, no `&p`; the
//! spec says borrow mode is inferred), so a `let` or `match` / `?=` pattern over
//! a place of a known type binds every name by value: a name bound to a
//! non-`Copy` part (a record field, a tuple element, the payload of `Some`/`Ok`/
//! `Err` or of a user variant) moves that part out of the scrutinee place.
//! Wildcards, literals, `Copy` parts and unknown types move nothing. A `let`
//! pattern moves at the `let`. A `match` arm moves eagerly under strict
//! ownership and, under the inferred policy, only if the arm's body consumes
//! the binding (the same "evidenced" rule as a callee that consumes).
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
//! plus a field path, so `&!p.a` and `&!p.b` do not conflict, while `&p.a`
//! conflicts with a later move of `p.a`, of `p`, or of `p.a.x`. Within one call,
//! `f(&!x, &x)` / `f(&!x, &!x)` / `f(&x, x)` are flagged as temporaries.
//!
//! # Explicit limits
//!
//! * Spans: a diagnostic points at the start of the statement where the bad
//!   use or conflicting borrow occurs (the function's start for a diagnostic
//!   outside any statement, e.g. a tail expression). There are no expression
//!   spans, so two uses in one statement share a position. A node built by
//!   hand rather than parsed has no position and gives `span: None`.
//! * Partial moves are tracked only through plain field chains (`x.a.b`) and
//!   the destructuring patterns below, not through an index (`x[i].f`), a
//!   pointer/`Box` deref, or a reference-typed parameter. A partial move never
//!   makes the *enclosing function* count as consuming its parameter (so under
//!   the inferred policy it does not turn the caller's argument into a move).
//!   `let y = x.f` does not make `y` known non-`Copy` (the type of a field
//!   expression is not inferred).
//! * Moving a non-`Copy` field out of a `&self` / `&!self` receiver, or out of a
//!   `&T` / `&!T` parameter of a known user type, is E0507 (`MoveOutOfBorrow`).
//!   A by-value use of such a field counts as the move-out under both policies
//!   with the usual consumption rule: under the inferred policy a field passed
//!   to a callee that does not consume it is only lent. Nothing is tracked as
//!   moved (the field stays usable, so no E0382 follows). Unknown, generic and
//!   Copy field types, and a reference reached any other way (a local copy of
//!   it, a method chain, an index, a deref), are never reported.
//! * Loans are created only by `let r = &place` (and copied by `let r2 = r`).
//!   References are not followed through calls, returns, struct fields,
//!   collections or `r = &y` reassignment (which only ends `r`'s old loans).
//! * A method call's receiver is judged only when the receiver is a plain place
//!   of a known user type and every method of that name on that type agrees on
//!   its receiver kind; anything else is never a move or a conflicting borrow.
//!   `for x in v` does not move `v` (MAGE infers borrow mode there).
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
    /// The type's own generic parameters (a field of such a type is unknown).
    generics: Vec<String>,
    /// Named fields of a struct or record `data` (empty for an enum or sum).
    fields: Vec<(String, Type)>,
    /// Variants of an enum or sum `data` with their positional payload types
    /// (empty for a struct or record). A struct-like variant is left out.
    variants: Vec<(String, Vec<Type>)>,
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

/// How a method takes `self`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Recv {
    /// No `self` parameter (an associated function).
    None,
    /// `self`: by value.
    Value,
    /// `&self`.
    Ref,
    /// `&!self` (and `!self`, which the parser desugars to it).
    RefMut,
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
    /// How the leading `self` parameter is taken.
    recv: Recv,
    /// The type name of the `impl` / `extend` block a method sits in.
    owner: Option<String>,
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

fn recv_of(f: &FunctionDef) -> Recv {
    match f.params.first() {
        Some(p) if p.name == "self" => match &p.ty {
            Type::Reference { mutable: false, .. } => Recv::Ref,
            Type::Reference { mutable: true, .. } => Recv::RefMut,
            Type::Path { segments, .. } if segments.len() == 1 && segments[0] == "Self" => Recv::Value,
            _ => Recv::None,
        },
        _ => Recv::None,
    }
}

fn sig_of(f: &FunctionDef, owner: Option<&str>) -> FnSig {
    FnSig {
        generics: f.generics.iter().map(|g| g.name.clone()).collect(),
        param_is_ref: f.params.iter().map(|p| matches!(p.ty, Type::Reference { .. })).collect(),
        consumes: vec![false; f.params.len()],
        has_self: f.params.first().is_some_and(|p| p.name == "self"),
        recv: recv_of(f),
        owner: owner.map(str::to_string),
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

/// The type (and generic parameters) of the `impl` / `extend` block around a method.
type Owner<'a> = Option<(&'a Type, &'a [ast::GenericParam])>;

/// Every function in `items` in `collect`'s order, with where its signature lives.
fn for_each_fn(
    items: &[Item],
    in_impl: bool,
    owner: Owner<'_>,
    counters: &mut HashMap<String, usize>,
    f: &mut dyn FnMut(Slot, &FunctionDef, Owner<'_>),
) {
    for item in items {
        match &item.kind {
            ItemKind::Function(def) => {
                if in_impl {
                    let i = counters.entry(def.name.clone()).or_insert(0);
                    f(Slot::Method(def.name.clone(), *i), def, owner);
                    *i += 1;
                } else {
                    f(Slot::Free(def.name.clone()), def, None);
                }
            }
            ItemKind::Impl(i) => {
                for_each_fn(&i.items, true, Some((&i.self_type, &i.generics)), counters, f)
            }
            ItemKind::Extend(e) => for_each_fn(&e.items, true, Some((&e.target_type, &[])), counters, f),
            ItemKind::Trait(t) => for_each_fn(&t.items, true, None, counters, f),
            ItemKind::Module(m) => {
                if let Some(items) = &m.items {
                    for_each_fn(items, in_impl, owner, counters, f);
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
        info.collect(&m.items, false, None);
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
            for_each_fn(&m.items, false, None, &mut counters, &mut |slot, f, owner| {
                let moved = moved_params(self, f, owner);
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

    fn collect(&mut self, items: &[Item], in_impl: bool, owner: Option<&str>) {
        for item in items {
            match &item.kind {
                ItemKind::Function(f) => {
                    if in_impl {
                        self.methods.entry(f.name.clone()).or_default().push(sig_of(f, owner));
                    } else {
                        insert_unique(&mut self.fns, &f.name, sig_of(f, None));
                    }
                }
                ItemKind::Struct(s) => {
                    let info = TypeInfo {
                        derives_copy: derives_copy(item),
                        generics: s.generics.iter().map(|g| g.name.clone()).collect(),
                        fields: s.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect(),
                        variants: Vec::new(),
                    };
                    insert_unique(&mut self.types, &s.name, info);
                }
                ItemKind::Enum(e) => {
                    let info = TypeInfo {
                        derives_copy: derives_copy(item),
                        generics: e.generics.iter().map(|g| g.name.clone()).collect(),
                        fields: Vec::new(),
                        variants: e
                            .variants
                            .iter()
                            .filter_map(|v| match &v.kind {
                                ast::VariantKind::Unit => Some((v.name.clone(), Vec::new())),
                                ast::VariantKind::Tuple(ts) => Some((v.name.clone(), ts.clone())),
                                ast::VariantKind::Struct(_) => None,
                            })
                            .collect(),
                    };
                    insert_unique(&mut self.types, &e.name, info);
                    for v in &e.variants {
                        insert_unique(&mut self.ctors, &v.name, e.name.clone());
                    }
                }
                ItemKind::Data(d) => {
                    let fields = match &d.kind {
                        DataKind::Record(fs) => fs.iter().map(|f| (f.name.clone(), f.ty.clone())).collect(),
                        DataKind::Sum(_) => Vec::new(),
                    };
                    let variants = match &d.kind {
                        DataKind::Record(_) => Vec::new(),
                        DataKind::Sum(vs) => vs.iter().map(|v| (v.name.clone(), v.fields.clone())).collect(),
                    };
                    let info = TypeInfo {
                        derives_copy: derives_copy(item),
                        generics: d.generics.iter().map(|g| g.name.clone()).collect(),
                        fields,
                        variants,
                    };
                    insert_unique(&mut self.types, &d.name, info);
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
                ItemKind::Impl(i) => self.collect(&i.items, true, type_name(&i.self_type).as_deref()),
                ItemKind::Extend(e) => self.collect(&e.items, true, type_name(&e.target_type).as_deref()),
                ItemKind::Trait(t) => self.collect(&t.items, true, None),
                ItemKind::Module(m) => {
                    if let Some(items) = &m.items {
                        self.collect(items, in_impl, owner);
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
    for (i, item) in m.items.iter().enumerate() {
        let mut d = check_item(&info, item);
        rebase(&mut d, m.starts.get(i).copied());
        out.extend(d);
    }
    out
}

/// Turn the item-relative positions [`check_item`] reports into absolute ones:
/// `start` is the 1-based line of the item's first token ([`Module::starts`]).
/// With no `start` the positions are dropped rather than left meaning
/// something else.
pub fn rebase(diags: &mut [Diagnostic], start: Option<u32>) {
    crate::hir::rebase(diags, start);
}

fn walk_items(items: &[Item], prefix: &str, owner: Owner<'_>, info: &ModuleInfo, out: &mut Vec<Diagnostic>) {
    for item in items {
        match &item.kind {
            ItemKind::Function(f) => {
                let name = format!("{prefix}{}", f.name);
                out.extend(analyse(info, &name, f, owner).0);
            }
            ItemKind::Impl(i) => {
                let p = type_name(&i.self_type).map(|n| format!("{n}.")).unwrap_or_default();
                walk_items(&i.items, &p, Some((&i.self_type, &i.generics)), info, out);
            }
            ItemKind::Extend(e) => {
                let p = type_name(&e.target_type).map(|n| format!("{n}.")).unwrap_or_default();
                walk_items(&e.items, &p, Some((&e.target_type, &[])), info, out);
            }
            ItemKind::Trait(t) => walk_items(&t.items, &format!("{}.", t.name), None, info, out),
            ItemKind::Module(m) => {
                if let Some(items) = &m.items {
                    walk_items(items, &format!("{prefix}{}.", m.name), None, info, out);
                }
            }
            _ => {}
        }
    }
}

/// Check one top-level item (a function, or an impl/extend/trait/module and
/// every function inside it) against a module summary. Concatenating this over
/// `m.items` in order and applying [`rebase`] with each item's start gives
/// exactly [`check_module_with`]'s output, so an incremental driver can
/// re-check only the items that changed.
///
/// Diagnostic spans are *relative to the item*: `line` is the offset from the
/// item's first line. That makes the answer a pure function of the item's text
/// (moving the item changes nothing), which is what early cutoff needs.
pub fn check_item(info: &ModuleInfo, item: &Item) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    walk_items(std::slice::from_ref(item), "", None, info, &mut out);
    out
}

/// Check one free function body. `qualified_name` is only used in messages. A
/// method needs its `impl` block to know what `self` is: use [`check_item`].
pub fn check_function(info: &ModuleInfo, qualified_name: &str, f: &FunctionDef) -> Vec<Diagnostic> {
    analyse(info, qualified_name, f, None).0
}

/// The parameters of `f` its body moves.
fn moved_params(info: &ModuleInfo, f: &FunctionDef, owner: Owner<'_>) -> BTreeSet<String> {
    analyse(info, &f.name, f, owner).1
}

fn analyse(
    info: &ModuleInfo,
    qualified_name: &str,
    f: &FunctionDef,
    owner: Owner<'_>,
) -> (Vec<Diagnostic>, BTreeSet<String>) {
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
        cur: f.body.pos,
    };
    let mut st = State { reachable: true, vars: BTreeMap::new(), loans: BTreeSet::new() };
    fcx.push_scope();
    // What `self` is: the impl's type, if it is a known user type and not one
    // of the impl's own generic parameters.
    let impl_generics: Vec<String> = owner.map(|(_, g)| g.iter().map(|g| g.name.clone()).collect()).unwrap_or_default();
    let self_ty: Option<String> = owner.and_then(|(t, _)| fcx.ty_name_of(t, &impl_generics));
    for p in &f.params {
        let mut by_ref = false;
        let (copy, ty_name) = if p.name == "self" {
            match &p.ty {
                Type::Reference { mutable, .. } => {
                    by_ref = true;
                    (if *mutable { Copyness::Unknown } else { Copyness::Copy }, self_ty.clone())
                }
                Type::Path { segments, .. } if segments.len() == 1 && segments[0] == "Self" => (
                    self_ty.as_deref().map_or(Copyness::Unknown, |n| info.user_type_copyness(n)),
                    self_ty.clone(),
                ),
                _ => (Copyness::Unknown, None),
            }
        } else if let Type::Reference { inner, .. } = &p.ty {
            // `p: &T` / `p: &!T` of a known user type: a reference to a `T`, so
            // its fields cannot be moved out (E0507).
            let name = fcx.ty_name_of(inner, &fcx.generics);
            by_ref = name.is_some();
            (info.type_copyness(&p.ty, &fcx.generics), name)
        } else {
            (info.type_copyness(&p.ty, &fcx.generics), fcx.ty_name_of(&p.ty, &fcx.generics))
        };
        fcx.bind(&mut st, &p.name, copy);
        if let Some(v) = st.vars.get_mut(&p.name) {
            v.is_param = true;
            v.ty_name = ty_name;
            v.by_ref = by_ref;
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

/// The places a format string interpolates: `f"{x} and {y.z}"` gives `[x]` and
/// `[y, z]` (a root and its field chain).
fn fmt_places(s: &str) -> Vec<Vec<String>> {
    let b: Vec<char> = s.chars().collect();
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != '{' {
            i += 1;
            continue;
        }
        if b.get(i + 1) == Some(&'{') {
            i += 2;
            continue;
        }
        let mut j = i + 1;
        let mut segs: Vec<String> = Vec::new();
        loop {
            let from = j;
            while j < b.len() && word(b[j]) {
                j += 1;
            }
            if j == from {
                break;
            }
            segs.push(b[from..j].iter().collect());
            if b.get(j) == Some(&'.') && b.get(j + 1).is_some_and(|c| word(*c)) {
                j += 1;
            } else {
                break;
            }
        }
        if !segs.is_empty() {
            out.push(segs);
        }
        i = j.max(i + 1);
    }
    out
}

// ── Places ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    root: String,
    path: Vec<String>,
}

/// A type and the generic parameter names it is written under.
type Ty = (Type, Vec<String>);

/// A name a pattern binds, and the scrutinee part it takes by value.
struct PBind {
    name: String,
    src: Option<(Place, Ty)>,
}

impl Place {
    fn child(&self, seg: &str) -> Place {
        let mut p = self.clone();
        p.path.push(seg.to_string());
        p
    }

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

/// `x` or `x.a.b`: a plain field chain with no index, call or deref in it.
fn field_place_of(e: &Expr) -> Option<Place> {
    match e {
        Expr::Ident { name } => Some(Place { root: name.clone(), path: Vec::new() }),
        Expr::FieldAccess { object, field } => {
            let mut p = field_place_of(object)?;
            p.path.push(field.clone());
            Some(p)
        }
        _ => None,
    }
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
    /// The user struct/`data` type this local is known to be a value of.
    ty_name: Option<String>,
    /// A `&self` / `&!self` receiver: a reference to `ty_name`, not an owned
    /// value. Nothing is moved out of it, and its method calls autoref.
    by_ref: bool,
    /// Moved field places (`p.name` -> `["name"]`): never empty paths, and
    /// only while the whole of `p` (`own`) is `Init`.
    fields: BTreeMap<Vec<String>, (Own, String)>,
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
                let keys: BTreeSet<Vec<String>> = v.fields.keys().chain(o.fields.keys()).cloned().collect();
                for k in keys {
                    let joined = match (v.fields.get(&k).cloned(), o.fields.get(&k).cloned()) {
                        (Some((x, hx)), Some((y, hy))) => (if x == y { x } else { Own::MaybeMoved }, hx.min(hy)),
                        (Some((_, h)), None) | (None, Some((_, h))) => (Own::MaybeMoved, h),
                        (None, None) => continue,
                    };
                    v.fields.insert(k, joined);
                }
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

/// How a method call treats its receiver.
enum RecvUse {
    /// Not judged: unknown type, unknown method, or the methods disagree.
    Plain,
    /// `self` by value, and the method consumes it.
    Move,
    /// `&self` (shared) or `&!self` (exclusive, `true`).
    Borrow(bool),
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
    moved: BTreeSet<Place>,
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
    /// Start of the statement being executed (the function's start outside any
    /// statement). Relative to the top-level item: see [`rebase`].
    cur: crate::ast::Pos,
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
                span: self.cur.is_set().then_some(crate::hir::Span { line: self.cur.line, col: self.cur.col }),
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
        let v = Var {
            own: Own::Init,
            copy,
            moved_by: None,
            depth: self.closures.len(),
            is_param: false,
            origin: None,
            ty_name: None,
            by_ref: false,
            fields: BTreeMap::new(),
        };
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
            let outer = self.cur;
            if let Some(p) = b.stmt_pos.get(i).filter(|p| p.is_set()) {
                self.cur = *p;
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
            self.cur = outer;
        }
        if st.reachable {
            if let Some(t) = &b.tail_expr {
                let outer = self.cur;
                if b.tail_pos.is_set() {
                    self.cur = b.tail_pos;
                }
                self.eval(t, st, tail);
                self.cur = outer;
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
                let ty_name = match ty {
                    Some(t) => self.ty_name_of(t, &self.generics),
                    None => self.infer_type_name(value, st),
                };
                self.eval(value, st, &alias(format!("assigned to `{name}`")));
                self.bind(st, name, copy);
                if let Some(v) = st.vars.get_mut(name.as_str()) {
                    v.origin = origin;
                    v.ty_name = ty_name;
                }
                self.record_loans(block, idx, name, value, st);
            }
            _ => {
                // A place of a known type: move only the parts the pattern binds.
                if let Some(tr) = self.tracked_scrutinee(value, st) {
                    let at = self.nums.start(value);
                    self.read_scrutinee(&tr.0, st);
                    self.bind_destructured(st, pattern, Some(tr), at, true);
                } else {
                    self.eval(value, st, &alias("destructured by a `let` pattern"));
                    self.bind_pattern(st, pattern);
                }
            }
        }
    }

    /// A scrutinee that is a plain field place of a known user type, owned here.
    fn tracked_scrutinee(&self, e: &Expr, st: &State) -> Option<(Place, Ty)> {
        let p = field_place_of(e)?;
        let v = st.vars.get(&p.root)?;
        // Matching the reference itself (`match self`) binds through it; only a
        // field place behind it is a move-out (E0507) for what the pattern binds.
        if v.by_ref && p.path.is_empty() {
            return None;
        }
        let ty = self.place_ty(v, &p.path)?;
        Some((p, ty))
    }

    /// Reading a scrutinee place touches only what the pattern binds, so only a
    /// move of the place itself or of a prefix of it is an error here (the bound
    /// parts are checked one by one).
    fn read_scrutinee(&mut self, place: &Place, st: &State) {
        let Some(v) = st.vars.get(&place.root) else { return };
        let hit = if v.own != Own::Init {
            Some((Vec::new(), v.own, v.moved_by.clone().unwrap_or_else(|| "moved".into())))
        } else {
            v.fields
                .iter()
                .find(|(p, _)| p.len() <= place.path.len() && place.path[..p.len()] == p[..])
                .map(|(p, (own, how))| (p.clone(), *own, how.clone()))
        };
        if let Some((p, own, how)) = hit {
            let shown = Place { root: place.root.clone(), path: p }.show();
            self.report_moved(&shown, own, &how);
        }
    }

    /// Bind the names of `pat`, typed from `src` when the scrutinee is a tracked
    /// place. With `eager`, a name bound to a non-`Copy` part moves that part out
    /// of the scrutinee now; otherwise the part is only checked (see
    /// [`Fcx::finish_arm`]).
    fn bind_destructured(
        &mut self,
        st: &mut State,
        pat: &Pattern,
        src: Option<(Place, Ty)>,
        at: u32,
        eager: bool,
    ) -> Vec<PBind> {
        let mut binds = Vec::new();
        self.pat_binds(pat, src, &mut binds);
        for b in &binds {
            if let Some((pl, ty)) = &b.src {
                if eager {
                    let copy = self.ty_copy(ty);
                    self.use_place_with(pl, at, st, &alias("destructured by a pattern"), copy);
                } else {
                    self.use_place_with(pl, at, st, &Use::Read, Copyness::Unknown);
                }
            }
        }
        for b in &binds {
            let (copy, ty_name) = match &b.src {
                Some((_, ty)) => (self.ty_copy(ty), self.ty_name_of(&ty.0, &ty.1)),
                None => (Copyness::Unknown, None),
            };
            self.bind(st, &b.name, copy);
            if let Some(v) = st.vars.get_mut(&b.name) {
                v.ty_name = ty_name;
            }
        }
        binds
    }

    /// End of a `match` arm whose bindings were not moved eagerly (the inferred
    /// policy): a binding the arm consumed took its part out of the scrutinee.
    fn finish_arm(&mut self, binds: &[PBind], st: &mut State, at: u32) {
        if !self.info.policy.infer_call_modes || !st.reachable {
            return;
        }
        for b in binds {
            let Some((pl, ty)) = &b.src else { continue };
            let copy = self.ty_copy(ty);
            if copy == Copyness::NonCopy && st.vars.get(&b.name).is_some_and(|v| v.own != Own::Init) {
                self.use_place_with(pl, at, st, &alias("destructured by a `match` arm"), copy);
            }
        }
    }

    /// The names a pattern binds, each with the scrutinee part it takes by value
    /// (when that part and its type are known).
    fn pat_binds(&self, p: &Pattern, src: Option<(Place, Ty)>, out: &mut Vec<PBind>) {
        let unknown = |p: &Pattern, out: &mut Vec<PBind>| {
            let mut names = Vec::new();
            pattern_names(p, &mut names);
            out.extend(names.into_iter().map(|name| PBind { name, src: None }));
        };
        match p {
            Pattern::Ident { name } => {
                // `None`, a unit variant: a constructor, not a binding.
                if name == "None" || matches!(self.info.ctors.get(name), Some(Some(_))) {
                    return;
                }
                out.push(PBind { name: name.clone(), src });
            }
            Pattern::Wildcard | Pattern::Literal { .. } => {}
            Pattern::Tuple { elements } => {
                let kids: Vec<Option<(Place, Ty)>> = match &src {
                    Some((pl, (Type::Tuple { elements: tys }, ctx))) if tys.len() == elements.len() => tys
                        .iter()
                        .enumerate()
                        .map(|(i, t)| Some((pl.child(&i.to_string()), (t.clone(), ctx.clone()))))
                        .collect(),
                    _ => vec![None; elements.len()],
                };
                for (e, k) in elements.iter().zip(kids) {
                    self.pat_binds(e, k, out);
                }
            }
            Pattern::Struct { path, fields } => {
                let rec = src.as_ref().and_then(|(pl, (t, ctx))| {
                    let Type::Path { segments, type_args } = t else { return None };
                    if segments.len() != 1 || !type_args.is_empty() || ctx.contains(&segments[0]) {
                        return None;
                    }
                    if path.last() != segments.last() {
                        return None;
                    }
                    match self.info.types.get(&segments[0]) {
                        Some(Some(info)) => Some((pl.clone(), info)),
                        _ => None,
                    }
                });
                for f in fields {
                    let child = rec.as_ref().and_then(|(pl, info)| {
                        info.fields
                            .iter()
                            .find(|(n, _)| *n == f.name)
                            .map(|(_, ty)| (pl.child(&f.name), (ty.clone(), info.generics.clone())))
                    });
                    match &f.pattern {
                        Some(sp) => self.pat_binds(sp, child, out),
                        None => out.push(PBind { name: f.name.clone(), src: child }),
                    }
                }
            }
            Pattern::Enum { path, elements } => {
                let kids: Vec<Option<(Place, Ty)>> = match (&src, path.last()) {
                    (Some((pl, (t, ctx))), Some(vname)) => match self.variant_payload(t, ctx, vname) {
                        Some((tys, vctx)) if tys.len() == elements.len() => tys
                            .into_iter()
                            .enumerate()
                            .map(|(i, t)| Some((pl.child(&format!("{vname}.{i}")), (t, vctx.clone()))))
                            .collect(),
                        _ => vec![None; elements.len()],
                    },
                    _ => vec![None; elements.len()],
                };
                for (e, k) in elements.iter().zip(kids) {
                    self.pat_binds(e, k, out);
                }
            }
            Pattern::Slice { .. } | Pattern::Or { .. } | Pattern::Ref { .. } => unknown(p, out),
        }
    }

    /// The payload types of variant `vname` of `t`, with the generics they are
    /// written in.
    fn variant_payload(&self, t: &Type, ctx: &[String], vname: &str) -> Option<(Vec<Type>, Vec<String>)> {
        let ctx_v = ctx.to_vec();
        match t {
            Type::Option { inner } if vname == "Some" => Some((vec![(**inner).clone()], ctx_v)),
            Type::Result { ok, .. } if vname == "Ok" => Some((vec![(**ok).clone()], ctx_v)),
            Type::Result { err, .. } if vname == "Err" => Some((vec![(**err).clone()], ctx_v)),
            Type::Path { segments, type_args } if segments.len() == 1 && !ctx.contains(&segments[0]) => {
                let n = segments[0].as_str();
                if let Some(Some(info)) = self.info.types.get(n) {
                    if !type_args.is_empty() {
                        return None;
                    }
                    let (_, tys) = info.variants.iter().find(|(v, _)| v == vname)?;
                    return Some((tys.clone(), info.generics.clone()));
                }
                match (n, vname, type_args.as_slice()) {
                    ("Option", "Some", [a]) => Some((vec![a.clone()], ctx_v)),
                    ("Result", "Ok", [a, _]) => Some((vec![a.clone()], ctx_v)),
                    ("Result", "Err", [_, b]) => Some((vec![b.clone()], ctx_v)),
                    _ => None,
                }
            }
            _ => None,
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

    // ── Types of places ──────────────────────────────────────────────

    /// The user struct/`data` type `t` names, if it is one (and not generic here).
    fn ty_name_of(&self, t: &Type, generics: &[String]) -> Option<String> {
        match t {
            Type::Path { segments, .. } if segments.len() == 1 => {
                let n = &segments[0];
                (!generics.contains(n) && matches!(self.info.types.get(n), Some(Some(_)))).then(|| n.clone())
            }
            _ => None,
        }
    }

    /// The user type an initialiser evidently produces.
    fn infer_type_name(&self, e: &Expr, st: &State) -> Option<String> {
        match e {
            Expr::Ident { name } => st.vars.get(name).filter(|v| !v.by_ref).and_then(|v| v.ty_name.clone()),
            Expr::StructLit { path, .. } => {
                let n = path.last()?;
                matches!(self.info.types.get(n), Some(Some(_))).then(|| n.clone())
            }
            Expr::Call { func, .. } => match &**func {
                Expr::Ident { name } if !st.vars.contains_key(name) => {
                    if let Some(Some(sig)) = self.info.fns.get(name) {
                        sig.ret.as_ref().and_then(|t| self.ty_name_of(t, &sig.generics))
                    } else if let Some(Some(ty)) = self.info.ctors.get(name) {
                        matches!(self.info.types.get(ty), Some(Some(_))).then(|| ty.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether the value at `v` + `path` is `Copy`, from the field types the
    /// module declares. Anything it cannot follow is `Unknown`.
    fn place_copy(&self, v: &Var, path: &[String]) -> Copyness {
        // Behind a reference the field's own type decides (a `&T` being `Copy`
        // says nothing about its fields); a non-`Copy` one cannot be moved out
        // of it, which `use_place_with` reports as E0507.
        if path.is_empty() || (v.copy == Copyness::Copy && !v.by_ref) {
            return if path.is_empty() { v.copy } else { Copyness::Copy };
        }
        let Some(mut cur) = v.ty_name.clone() else { return Copyness::Unknown };
        for (i, seg) in path.iter().enumerate() {
            let Some(Some(info)) = self.info.types.get(&cur) else { return Copyness::Unknown };
            let Some((_, ty)) = info.fields.iter().find(|(n, _)| n == seg) else { return Copyness::Unknown };
            if i + 1 == path.len() {
                return self.info.type_copyness(ty, &info.generics);
            }
            match ty {
                Type::Path { segments, type_args } if segments.len() == 1 && type_args.is_empty() => {
                    cur = segments[0].clone();
                }
                _ => return Copyness::Unknown,
            }
        }
        Copyness::Unknown
    }

    /// The type of the place `v` + `path`, following declared record fields.
    fn place_ty(&self, v: &Var, path: &[String]) -> Option<Ty> {
        let name = v.ty_name.clone()?;
        let mut cur: Ty = (Type::Path { segments: vec![name], type_args: Vec::new() }, Vec::new());
        for seg in path {
            let Type::Path { segments, type_args } = &cur.0 else { return None };
            if segments.len() != 1 || !type_args.is_empty() || cur.1.contains(&segments[0]) {
                return None;
            }
            let Some(Some(info)) = self.info.types.get(&segments[0]) else { return None };
            let (_, ty) = info.fields.iter().find(|(n, _)| n == seg)?;
            cur = (ty.clone(), info.generics.clone());
        }
        Some(cur)
    }

    fn ty_copy(&self, t: &Ty) -> Copyness {
        self.info.type_copyness(&t.0, &t.1)
    }

    // ── Uses of places ───────────────────────────────────────────────

    fn use_local(&mut self, name: &str, at: u32, st: &mut State, mode: &Use) {
        self.use_place(&Place { root: name.to_string(), path: Vec::new() }, at, st, mode);
    }

    /// The move (if any) that makes touching `path` of `v` an error. A use
    /// conflicts with a move of the place, of a prefix of it, or of an extension
    /// of it; an assignment re-initialises its own place and extensions, so only
    /// a moved prefix (or the whole value) matters.
    fn moved_conflict(v: &Var, path: &[String], assign: bool) -> Option<(Vec<String>, Own, String)> {
        if v.own != Own::Init {
            return Some((Vec::new(), v.own, v.moved_by.clone().unwrap_or_else(|| "moved".into())));
        }
        v.fields
            .iter()
            .find(|(p, _)| {
                if assign {
                    p.len() < path.len() && path[..p.len()] == p[..]
                } else {
                    let n = p.len().min(path.len());
                    p[..n] == path[..n]
                }
            })
            .map(|(p, (own, how))| (p.clone(), *own, how.clone()))
    }

    fn report_moved(&mut self, shown: &str, own: Own, how: &str) {
        let qual = if own == Own::Moved { "" } else { "possibly " };
        self.report(
            DiagnosticCategory::UseAfterMove,
            format!(
                "use of moved value: `{shown}` in fn `{}` (value {qual}moved earlier: {how}; it is used again here)",
                self.fname
            ),
        );
    }

    fn use_place(&mut self, place: &Place, at: u32, st: &mut State, mode: &Use) {
        let Some(v) = st.vars.get(place.root.as_str()) else { return };
        let copy = self.place_copy(v, &place.path);
        self.use_place_with(place, at, st, mode, copy);
    }

    /// [`Fcx::use_place`] with the place's `Copy`-ness already decided.
    fn use_place_with(&mut self, place: &Place, at: u32, st: &mut State, mode: &Use, copy: Copyness) {
        let name = place.root.as_str();
        let Some(v) = st.vars.get(name).cloned() else { return };
        if let Some((_, own, how)) = Self::moved_conflict(&v, &place.path, false) {
            self.report_moved(&place.show(), own, &how);
        }
        let (why, escapes) = match mode {
            Use::Read => (None, false),
            Use::Move(w) => (Some(w), true),
            Use::Alias(w) => (Some(w), false),
        };
        if let Some(why) = why {
            if v.by_ref && !place.path.is_empty() {
                // A field of a borrowed value (`&self`, or a reference-typed
                // parameter) used by value: the move is out of the borrow, so
                // nothing is tracked (the field stays usable) and it is E0507.
                if copy == Copyness::NonCopy {
                    self.report(
                        DiagnosticCategory::MoveOutOfBorrow,
                        format!(
                            "cannot move out of `{}` which is behind a reference in fn `{}` ({why}; clone it, or take `self` by value)",
                            place.show(),
                            self.fname
                        ),
                    );
                }
                return;
            }
            if copy == Copyness::NonCopy {
                self.check_action(st, place, Action::Move, at);
                for (k, f) in self.closures.iter_mut().enumerate() {
                    if v.depth <= k {
                        f.moved.insert(place.clone());
                    }
                }
                if place.path.is_empty() {
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
                        var.fields.clear();
                    }
                } else if let Some(var) = st.vars.get_mut(name) {
                    var.fields.retain(|p, _| !p.starts_with(&place.path));
                    var.fields.insert(place.path.clone(), (Own::Moved, format!("`{}` {why}", place.show())));
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

    /// What `recv.method(..)` does to its receiver, when the receiver is a
    /// plain place of a known user type that declares `method` with a `self`.
    fn receiver_use(&self, receiver: &Expr, method: &str, st: &State) -> RecvUse {
        let Some(p) = field_place_of(receiver) else { return RecvUse::Plain };
        let Some(v) = st.vars.get(&p.root) else { return RecvUse::Plain };
        let Some(ty) = self.place_ty(v, &p.path).and_then(|t| self.ty_name_of(&t.0, &t.1)) else {
            return RecvUse::Plain;
        };
        let cands: Vec<&FnSig> = self
            .info
            .methods
            .get(method)
            .map(|c| c.iter().filter(|s| s.recv != Recv::None && s.owner.as_deref() == Some(ty.as_str())).collect())
            .unwrap_or_default();
        let Some(first) = cands.first() else { return RecvUse::Plain };
        if cands.iter().any(|s| s.recv != first.recv) {
            return RecvUse::Plain;
        }
        match first.recv {
            Recv::Value if cands.iter().all(|s| self.takes(s, 0)) => RecvUse::Move,
            Recv::Ref => RecvUse::Borrow(false),
            Recv::RefMut => RecvUse::Borrow(true),
            _ => RecvUse::Plain,
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
                    st.vars.get(&p.root).is_some_and(|v| self.place_copy(v, &p.path) != Copyness::NonCopy)
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
                for segs in fmt_places(value) {
                    let place = Place { root: segs[0].clone(), path: segs[1..].to_vec() };
                    self.use_place(&place, self.nums.start(e), st, &Use::Read);
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
                match self.receiver_use(receiver, method, st) {
                    RecvUse::Move => {
                        self.eval(receiver, st, &mv(format!("passed by value as `self` to `{method}`")));
                    }
                    RecvUse::Borrow(mutable) => {
                        self.eval(receiver, st, &Use::Read);
                        if let Some(p) = field_place_of(receiver) {
                            let action = if mutable { Action::BorrowMut } else { Action::BorrowShared };
                            let at = self.nums.start(receiver);
                            self.check_action(st, &p, action, at);
                        }
                    }
                    RecvUse::Plain => self.eval(receiver, st, &Use::Read),
                }
                let moves = self.method_moves(method, args.len());
                self.eval_args(method, &moves, args, st);
            }
            Expr::FieldAccess { object, .. } => match field_place_of(e) {
                Some(p) if st.vars.contains_key(&p.root) => self.use_place(&p, self.nums.start(e), st, mode),
                _ => self.eval(object, st, &Use::Read),
            },
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
                let tracked = scrutinee.as_deref().and_then(|s| self.tracked_scrutinee(s, st));
                let at = scrutinee.as_deref().map_or(0, |s| self.nums.start(s));
                match (scrutinee, &tracked) {
                    (_, Some((pl, _))) => self.read_scrutinee(pl, st),
                    (Some(s), None) => self.eval(s, st, &Use::Read),
                    _ => {}
                }
                if arms.is_empty() {
                    return;
                }
                let base = st.clone();
                let mut out: Option<State> = None;
                for arm in arms {
                    let mut s = base.clone();
                    self.push_scope();
                    let eager = !self.info.policy.infer_call_modes;
                    let binds = match &tracked {
                        Some(tr) => self.bind_destructured(&mut s, &arm.pattern, Some(tr.clone()), at, eager),
                        None => {
                            self.bind_pattern(&mut s, &arm.pattern);
                            Vec::new()
                        }
                    };
                    self.eval(&arm.body, &mut s, mode);
                    self.finish_arm(&binds, &mut s, at);
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
                        v.fields.clear();
                    }
                    // Whatever `name` held, it holds no longer.
                    st.loans.retain(|l| l.holder != *name);
                }
            }
            Expr::FieldAccess { .. } if field_place_of(target).is_some_and(|p| st.vars.contains_key(&p.root)) => {
                // `x.f = v` re-initialises `x.f` (and its extensions), but assigning
                // into a moved `x` (or moved `x.a` for `x.a.f`) is an error.
                if let Some(p) = field_place_of(target) {
                    let v = st.vars.get(&p.root).cloned();
                    if let Some((cp, own, how)) = v.and_then(|v| Self::moved_conflict(&v, &p.path, true)) {
                        let shown = Place { root: p.root.clone(), path: cp }.show();
                        self.report_moved(&shown, own, &how);
                    }
                    self.check_action(st, &p, Action::Assign, after);
                    if let Some(v) = st.vars.get_mut(&p.root) {
                        v.fields.retain(|k, _| !k.starts_with(&p.path));
                    }
                }
            }
            _ => {
                // `x[i] = v` uses `x`: assigning into a moved value is an error.
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
        for place in frame.moved {
            if let Some(v) = st.vars.get_mut(&place.root) {
                let how = "captured by a closure that consumes it".to_string();
                if place.path.is_empty() {
                    v.own = Own::Moved;
                    v.moved_by = Some(how);
                    v.fields.clear();
                } else {
                    v.fields.retain(|p, _| !p.starts_with(&place.path));
                    v.fields.insert(place.path.clone(), (Own::Moved, format!("`{}` {how}", place.show())));
                }
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

    fn line_cols(src: &str, code: &str) -> Vec<(u32, u32)> {
        run(src).iter().filter(|d| d.id.as_deref() == Some(code)).map(|d| d.span.map(|s| (s.line, s.col)).expect("a span")).collect()
    }

    #[test]
    fn a_use_after_move_points_at_the_statement_that_uses_it() {
        // PRELUDE is lines 1-4, `f host` is line 5, the body starts on line 6.
        let src = with("take(a)\nv k = 1\n  take(a)");
        assert_eq!(line_cols(&src, "E0382"), vec![(8, 3)], "{src}");
        // A statement in the middle (not the tail).
        let src = with("take(a)\nv k = 1\n  take(a);\nv j = 2");
        assert_eq!(line_cols(&src, "E0382"), vec![(8, 3)], "{src}");
    }

    #[test]
    fn a_borrow_conflict_points_at_the_conflicting_statement() {
        let src = with("v r = &a\nv k = 1\n  take(a)\npeek(r)");
        assert_eq!(line_cols(&src, "E0502"), vec![(8, 3)], "{src}");
    }

    #[test]
    fn a_nested_statement_is_located_inside_its_own_block() {
        let src = with("take(a)\nif n > 0 {\n    v z = 1\n      take(a)\n}");
        assert_eq!(line_cols(&src, "E0382"), vec![(9, 7)], "{src}");
    }

    #[test]
    fn check_item_is_item_relative_and_rebase_makes_it_absolute() {
        let src = with("take(a)\ntake(a);\nv j = 2");
        let module = parser::parse(&lexer::lex(&src)).unwrap();
        let info = ModuleInfo::with_policy(&module, Policy::STRICT);
        let host = module.items.last().unwrap();
        let mut d = check_item(&info, host);
        assert_eq!(d[0].span.map(|s| (s.line, s.col)), Some((2, 1)), "offset from the `f host` line");
        rebase(&mut d, module.starts.last().copied());
        assert_eq!(d[0].span.map(|s| (s.line, s.col)), Some((7, 1)));
        rebase(&mut d, None);
        assert!(d[0].span.is_none());
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

    // ── places: partial moves and field borrows ──────────────────────

    const SHAPES: &str = "data Pair(name: s, age: i32)\ndata Outer(inner: Pair, tag: s)\n@d(Copy)\ndata Dot(x: i32, y: i32)\ndata Mixed(d: Dot, tag: s)\nf eat(x: Pair) { }\nf take_n(x: i32) { }\nf take(x: s) { }\nf peek(x: &s) { }\nf peek_p(x: &Pair) { }\n";

    /// `f host(p: Pair, o: Outer, m: Mixed, n: i32)` with `body`.
    fn pl(body: &str) -> String {
        format!("{SHAPES}f host(p: Pair, o: Outer, m: Mixed, n: i32) {{\n{body}\n}}\n")
    }

    #[test]
    fn moving_a_field_leaves_its_siblings_usable() {
        clean(&pl("take(p.name)\ntake_n(p.age)\ntake_n(p.age)"));
        clean(&pl("take(o.tag)\nv k = o.inner\npeek(&k.name)"));
        flags(&pl("take(o.tag)\nv k = o.inner\npeek(&o.inner.name)"), "E0382", "`o.inner.name`");
        clean(&pl("take(p.name)\nv a = p.age"));
    }

    #[test]
    fn using_a_moved_field_is_e0382() {
        let src = pl("take(p.name)\ntake(p.name)");
        flags(&src, "E0382", "use of moved value: `p.name`");
        flags(&src, "E0382", "`p.name` passed by value to `take`");
        let d = run(&src);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].category, Some(DiagnosticCategory::UseAfterMove));
        // A read and a borrow are uses too.
        flags(&pl("take(p.name)\npeek(&p.name)"), "E0382", "`p.name`");
        flags(&pl("take(p.name)\nv q = p.name"), "E0382", "`p.name`");
        flags(&pl("take(p.name)\nv q = f\"{p.name}\""), "E0382", "`p.name`");
        // A path through the field is the field.
        flags(&pl("take(o.inner)\ntake_n(o.inner.age)"), "E0382", "`o.inner.age`");
    }

    #[test]
    fn using_the_whole_struct_after_a_field_move_is_e0382() {
        flags(&pl("take(p.name)\nv q = p"), "E0382", "use of moved value: `p`");
        flags(&pl("take(p.name)\neat(p)"), "E0382", "use of moved value: `p`");
        flags(&pl("take(p.name)\npeek_p(&p)"), "E0382", "use of moved value: `p`");
        flags(&pl("take(p.name)\nret p"), "E0382", "use of moved value: `p`");
        // Even when the moved field was Copy-looking-free: a sibling read is fine, the whole is not.
        clean(&pl("take(p.name)\nv k = p.age"));
    }

    #[test]
    fn using_a_field_after_the_whole_struct_moved_is_e0382() {
        flags(&pl("eat(p)\npeek(&p.name)"), "E0382", "use of moved value: `p.name`");
        flags(&pl("eat(p)\ntake_n(p.age)"), "E0382", "use of moved value: `p.age`");
        flags(&pl("v q = p\ntake(p.name)"), "E0382", "`p.name`");
    }

    #[test]
    fn reassigning_a_field_reinitialises_it() {
        clean(&pl("take(p.name)\np.name = \"x\"\ntake(p.name)"));
        // Reassigning the whole struct re-initialises every field.
        clean(&pl("take(p.name)\np = o.inner\ntake(p.name)"));
        // Assigning into a moved whole is an error, though.
        flags(&pl("eat(p)\np.name = \"x\""), "E0382", "use of moved value: `p`");
        // A moved field's sibling is still moved after an unrelated assignment.
        flags(&pl("take(o.tag)\no.inner.name = \"x\"\ntake(o.tag)"), "E0382", "`o.tag`");
    }

    #[test]
    fn moves_join_over_branches_per_field() {
        flags(&pl("if n > 0 { take(p.name) }\ntake(p.name)"), "E0382", "possibly moved");
        clean(&pl("if n > 0 { take(p.name) }\ntake_n(p.age)"));
        clean(&pl("if n > 0 {\n take(p.name)\n ret\n}\ntake(p.name)"));
        // Re-initialised on the only path that moved it.
        clean(&pl("if n > 0 {\n take(p.name)\n p.name = \"x\"\n}\ntake(p.name)"));
        flags(&pl("m k = 0\nwhile k < n {\n take(p.name)\n k = k + 1\n}"), "E0382", "`p.name`");
    }

    #[test]
    fn a_closure_that_consumes_a_field_takes_it() {
        flags(&pl("v cl = |z| take(p.name)\ntake(p.name)"), "E0382", "`p.name`");
        clean(&pl("v cl = |z| take(p.name)\ntake_n(p.age)"));
    }

    #[test]
    fn borrowing_a_field_then_moving_a_sibling_is_fine() {
        clean(&pl("v r = &o.tag\ntake(o.inner.name)\npeek(r)"));
        clean(&pl("v r = &p.name\ntake_n(p.age)\npeek(r)"));
        clean(&pl("v r = &o.inner\ntake(o.tag)\npeek_p(r)"));
        clean(&pl("v r = &o.inner.name\ntake(o.tag)\nv w = &!o.inner.age\npeek(r)"));
    }

    #[test]
    fn borrowing_a_field_then_moving_it_or_its_whole_is_an_error() {
        flags(&pl("v r = &p.name\ntake(p.name)\npeek(r)"), "E0502", "cannot move out of `p.name`");
        flags(&pl("v r = &p.name\neat(p)\npeek(r)"), "E0502", "cannot move out of `p`");
        // A prefix borrow conflicts with a move of an extension.
        flags(&pl("v r = &o.inner\ntake(o.inner.name)\npeek_p(r)"), "E0502", "cannot move out of `o.inner.name`");
        flags(&pl("v r = &p\ntake(p.name)\npeek_p(r)"), "E0502", "cannot move out of `p.name`");
        // Not while the borrow is dead.
        clean(&pl("v r = &p.name\npeek(r)\ntake(p.name)"));
        // Mutable borrows of overlapping places.
        flags(&pl("v r = &!o.inner\nv w = &!o.inner.name\npeek_p(r)"), "E0502", "more than once");
        clean(&pl("v r = &!o.inner.name\nv w = &!o.inner.age\npeek(r)\npeek(w)"));
    }

    #[test]
    fn nested_paths() {
        clean(&pl("take(o.inner.name)\ntake_n(o.inner.age)\ntake(o.tag)"));
        flags(&pl("take(o.inner.name)\neat(o.inner)"), "E0382", "use of moved value: `o.inner`");
        flags(&pl("take(o.inner.name)\nv q = o"), "E0382", "use of moved value: `o`");
        flags(&pl("take(o.inner)\ntake(o.inner.name)"), "E0382", "`o.inner.name`");
        flags(&pl("take(o.inner)\no.inner.name = \"x\""), "E0382", "use of moved value: `o.inner`");
        // Re-initialising the prefix re-initialises the extension.
        clean(&pl("take(o.inner.name)\no.inner = p\ntake(o.inner.name)"));
        clean(&pl("take(o.inner.name)\no.inner.name = \"x\"\neat(o.inner)"));
    }

    #[test]
    fn copy_fields_never_move() {
        clean(&pl("take_n(p.age)\ntake_n(p.age)\nv q = p.age\nv r = p.age"));
        // A `Copy` struct inside a non-Copy one, and its fields.
        clean(&pl("v a = m.d\nv b = m.d\ntake_n(m.d.x)\ntake_n(m.d.x)"));
        flags(&pl("take(m.tag)\nv q = m"), "E0382", "use of moved value: `m`");
        clean(&pl("take(m.tag)\nv a = m.d"));
    }

    #[test]
    fn unknown_field_types_are_not_reported() {
        let g = "data G[T](v: T, name: s)\ndata W(m: Mystery, name: s)\nf take(x: s) { }\n";
        // A field of a generic type, of an undeclared type, of an undeclared field,
        // and of a variable whose type is not a known struct.
        clean(&format!("{g}f host(a: G[s]) {{\n take(a.v)\n take(a.v)\n}}\n"));
        clean(&format!("{g}f host(w: W) {{\n take(w.m)\n take(w.m)\n}}\n"));
        clean(&format!("{g}f host(w: W) {{\n take(w.nope)\n take(w.nope)\n}}\n"));
        clean(&format!("{g}f host(x: Mystery) {{\n take(x.f)\n take(x.f)\n}}\n"));
        clean(&format!("{g}f host[T](x: T) {{\n take(x.f)\n take(x.f)\n}}\n"));
        clean(&format!("{g}f host(w: &W) {{\n take(w.m)\n take(w.nope)\n}}\n"));
        // Behind a reference the known field is E0507, not a use after move.
        flags(&format!("{g}f host(w: &W) {{\n take(w.name)\n take(w.name)\n}}\n"), "E0507", "`w.name`");
        clean(&format!("{g}f host() {{\n v w = make()\n take(w.name)\n take(w.name)\n}}\n"));
        // The known field of the same struct is still tracked.
        flags(&format!("{g}f host(w: W) {{\n take(w.name)\n take(w.name)\n}}\n"), "E0382", "`w.name`");
        // Indexing ends the path: nothing is moved out of an element.
        clean(&pl("take(o.tag[0])\ntake(o.tag[0])"));
    }

    #[test]
    fn a_field_passed_to_a_call_moves_per_policy() {
        let src = "data Pair(name: s, age: i32)\nf keep(x: s) -> s { x }\nf look(x: s) { }\nf host(p: Pair) {\n look(p.name)\n look(p.name)\n}\nf host2(p: Pair) {\n keep(p.name)\n keep(p.name)\n}\n";
        // INFERRED: only the consuming callee takes the field.
        let d = dflt(src);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("use of moved value: `p.name`") && d[0].contains("fn `host2`"), "{d:?}");
        // STRICT: any by-value parameter does.
        let s = msgs(src);
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(s.iter().any(|m| m.contains("fn `host`")), "{s:?}");
        // The same field is not an aggregate member under INFERRED, but is under STRICT.
        let agg = "data Pair(name: s, age: i32)\ndata Holder(inner: s)\nf host(p: Pair) {\n v h = @Holder { inner: p.name }\n v k = p.name\n}\n";
        assert!(dflt(agg).is_empty(), "{:?}", dflt(agg));
        assert_eq!(msgs(agg).len(), 1, "{:?}", msgs(agg));
        // `let` moves under both.
        let lets = "data Pair(name: s, age: i32)\nf host(p: Pair) {\n v a = p.name\n v b = p.name\n}\n";
        assert_eq!(dflt(lets).len(), 1);
        assert_eq!(msgs(lets).len(), 1);
    }

    #[test]
    fn a_field_move_does_not_make_the_function_consume_its_parameter() {
        // `f` moves `p.name` out; callers are not told `p` is consumed, so
        // reusing the argument is not flagged under the inferred policy.
        let src = "data Pair(name: s, age: i32)\nf grab(p: Pair) -> s { p.name }\nf host(a: Pair) {\n grab(a)\n grab(a)\n}\n";
        assert!(dflt(src).is_empty(), "{:?}", dflt(src));
    }

    #[test]
    fn a_field_move_points_at_the_statement_that_uses_it() {
        let src = pl("take(p.name)\nv k = 1\n  take(p.name)");
        let n = SHAPES.matches('\n').count() as u32;
        // `f host` is on line n+1, the body starts on line n+2.
        assert_eq!(line_cols(&src, "E0382"), vec![(n + 4, 3)], "{src}");
        let src = pl("v r = &p.name\nv k = 1\n  take(p.name)\npeek(r)");
        assert_eq!(line_cols(&src, "E0502"), vec![(n + 4, 3)], "{src}");
    }

    #[test]
    fn same_call_temporaries_follow_the_field_type() {
        // A shared borrow beside a by-value Copy field of the same struct is fine ...
        clean(&format!("{SHAPES}f both(a: &Pair, b: i32) {{ }}\nf host(p: Pair) {{\n both(&p, p.age)\n}}\n"));
        // ... a non-Copy one is not.
        flags(
            &format!("{SHAPES}f both(a: &Pair, b: s) {{ }}\nf host(p: Pair) {{\n both(&p, p.name)\n}}\n"),
            "E0502",
            "cannot move out of `p`",
        );
    }

    // ── `self` and its fields ────────────────────────────────────────

    const OWNED: &str = "data Pair(name: s, age: i32)\ndata Outer(inner: Pair, tag: s)\n@d(Copy)\ndata Dot(x: i32, y: i32)\nf take(x: s) { }\nf take_n(x: i32) { }\nf peek(x: &s) { }\nf peek_p(x: &Pair) { }\nf keep(x: s) -> s { x }\n";

    /// `OWNED` plus `extend Pair { methods }` and `body` as the rest of the module.
    fn on_pair(methods: &str, rest: &str) -> String {
        format!("{OWNED}extend Pair {{\n{methods}\n}}\n{rest}\n")
    }

    fn dflt_msgs(src: &str) -> Vec<String> {
        dflt(src)
    }

    #[test]
    fn a_by_value_self_field_is_a_place() {
        let src = on_pair(" f twice(self) {\n  take(self.name)\n  take(self.name)\n }", "");
        flags(&src, "E0382", "use of moved value: `self.name`");
        flags(&src, "E0382", "fn `Pair.twice`");
        // Siblings stay usable; a Copy field never moves.
        clean(&on_pair(" f ok(self) {\n  take(self.name)\n  take_n(self.age)\n  take_n(self.age)\n }", ""));
        // The whole value after a field moved out.
        flags(&on_pair(" f whole(self) {\n  take(self.name)\n  v q = self\n }", ""), "E0382", "use of moved value: `self`");
        // And a field after the whole value moved.
        flags(&on_pair(" f rest(self) {\n  v q = self\n  take(self.name)\n }", ""), "E0382", "`self.name`");
        // Reassigning the field re-initialises it.
        clean(&on_pair(" f again(self) {\n  take(self.name)\n  self.name = \"x\"\n  take(self.name)\n }", ""));
        // A nested field of a by-value self.
        let nested = format!("{OWNED}extend Outer {{\n f deep(self) {{\n  take(self.inner.name)\n  take(self.inner.name)\n }}\n}}\n");
        flags(&nested, "E0382", "`self.inner.name`");
        clean(&format!("{OWNED}extend Outer {{\n f deep(self) {{\n  take(self.inner.name)\n  take(self.tag)\n  take_n(self.inner.age)\n }}\n}}\n"));
    }

    fn codes(src: &str, policy: Policy) -> Vec<String> {
        run_with(src, policy).into_iter().filter_map(|d| d.id).collect()
    }

    #[test]
    fn a_field_moved_out_of_a_reference_receiver_is_e0507() {
        for recv in ["&self", "&!self", "!self"] {
            let by_call = on_pair(&format!(" f once({recv}) {{\n  take(self.name)\n }}"), "");
            flags(&by_call, "E0507", "cannot move out of `self.name` which is behind a reference");
            flags(&on_pair(&format!(" f once({recv}) {{\n  v a = self.name\n  v b = self\n }}"), ""), "E0507", "`self.name`");
            flags(&on_pair(&format!(" f once({recv}) -> s {{\n  self.name\n }}"), ""), "E0507", "`self.name`");
            // Nothing is moved, so the field stays usable: no use-after-move follows.
            let twice = on_pair(&format!(" f twice({recv}) {{\n  take(self.name)\n  take(self.name)\n  v q = self.name\n }}"), "");
            let c = codes(&twice, Policy::STRICT);
            assert!(!c.is_empty() && c.iter().all(|c| c == "E0507"), "{c:?}");
        }
        // A nested field is one too; a Copy field, a borrow and a copy of the reference are not.
        flags(&format!("{OWNED}extend Outer {{\n f deep(&self) {{\n  take(self.inner.name)\n }}\n}}\n"), "E0507", "`self.inner.name`");
        clean(&on_pair(" f ok(&self) -> i32 {\n  take(&self.name)\n  peek(&self.name)\n  take_n(self.age)\n  v b = self\n  self.age\n }", ""));
        // A by-value self is not a borrow: that is a plain partial move.
        assert!(!codes(&on_pair(" f own(self) {\n  take(self.name)\n }", ""), Policy::STRICT).contains(&"E0507".to_string()));
    }

    #[test]
    fn e0507_follows_the_consumption_rule_of_the_policy() {
        // `take` ignores its parameter, so the inferred policy only lends it.
        let lent = on_pair(" f once(&self) {\n  take(self.name)\n }", "");
        assert!(codes(&lent, Policy::default()).is_empty());
        assert_eq!(codes(&lent, Policy::STRICT), vec!["E0507"]);
        // `keep` returns its parameter, so it consumes it under both.
        let kept = on_pair(" f once(&self) -> s {\n  keep(self.name)\n }", "");
        assert_eq!(codes(&kept, Policy::default()), vec!["E0507"]);
        assert_eq!(codes(&kept, Policy::STRICT), vec!["E0507"]);
        // Returning the field is a move under both.
        let ret = on_pair(" f once(&self) -> s {\n  self.name\n }", "");
        assert_eq!(codes(&ret, Policy::default()), vec!["E0507"]);
    }

    #[test]
    fn a_field_moved_out_of_a_reference_parameter_is_e0507() {
        flags(&format!("{OWNED}f host(p: &Pair) {{\n take(p.name)\n}}\n"), "E0507", "`p.name`");
        flags(&format!("{OWNED}f host(p: &!Pair) {{\n v a = p.name\n}}\n"), "E0507", "`p.name`");
        flags(&format!("{OWNED}f host(p: &Outer) {{\n take(p.inner.name)\n}}\n"), "E0507", "`p.inner.name`");
        // Copy fields, borrows and the reference itself are fine; so are unknown
        // and generic types, and a by-value parameter (a plain partial move).
        clean(&format!("{OWNED}f host(p: &Pair) {{\n take_n(p.age)\n peek(&p.name)\n peek_p(p)\n peek_p(p)\n}}\n"));
        clean(&format!("{OWNED}f host(p: &Mystery) {{\n take(p.name)\n}}\n"));
        clean(&format!("{OWNED}f host[T](p: &T) {{\n take(p.name)\n}}\n"));
        assert!(codes(&format!("{OWNED}f host(p: Pair) {{\n take(p.name)\n}}\n"), Policy::STRICT).iter().all(|c| c != "E0507"));
    }

    #[test]
    fn a_reference_receiver_keeps_its_borrows() {
        // A reference receiver's fields can still be borrowed and conflict.
        flags(
            &on_pair(" f clash(&!self) {\n  v r = &self.name\n  v w = &!self.name\n  peek(r)\n }", ""),
            "E0502",
            "cannot borrow `self.name` as mutable because it is also borrowed as immutable",
        );
        // `let y = self` copies the reference, so `y.name` is no owned field either.
        clean(&on_pair(" f via(&self) {\n  v y = self\n  take(y.name)\n  take(y.name)\n }", ""));
    }

    #[test]
    fn self_of_an_unknown_or_copy_type_is_not_followed() {
        // A trait's `Self` is unknown.
        clean(&format!("{OWNED}trait Named {{\n f twice(self) {{\n  take(self.name)\n  take(self.name)\n }}\n}}\n"));
        // A Copy type: `self` is a copy.
        clean("@d(Copy)\ndata Dot(x: i32, y: i32)\nf take_n(x: i32) { }\nextend Dot {\n f twice(self) {\n  take_n(self.x)\n  take_n(self.x)\n  v a = self\n  v b = self\n }\n}\n");
        // An impl for a generic parameter, and for an undeclared type.
        clean("f take(x: s) { }\nimpl[T] Show for T {\n f twice(self) {\n  take(self.name)\n  take(self.name)\n }\n}\n");
        clean("f take(x: s) { }\nextend Mystery {\n f twice(self) {\n  take(self.name)\n  take(self.name)\n }\n}\n");
    }

    #[test]
    fn a_by_value_self_that_is_returned_is_consumed() {
        // `ret self` moves it, so the inferred policy treats the method as consuming.
        let src = on_pair(" f finish(self) -> Pair { self }", "f host(p: Pair) {\n p.finish()\n p.finish()\n}");
        assert_eq!(dflt_msgs(&src).len(), 1, "{:?}", dflt_msgs(&src));
        flags(&on_pair(" f twice(self) -> Pair {\n  v a = self\n  self\n }", ""), "E0382", "`self`");
    }

    // ── method-call receivers ────────────────────────────────────────

    const RECV: &str = " f finish(self) -> Pair { self }\n f show(self) -> i32 { self.age }\n f get(&self) -> i32 { self.age }\n f bump(&!self) { }";

    fn host_of(body: &str) -> String {
        on_pair(RECV, &format!("f host(p: Pair, o: Outer) {{\n{body}\n}}"))
    }

    #[test]
    fn a_by_value_receiver_moves_under_strict_always() {
        let src = host_of(" p.show()\n p.show()");
        flags(&src, "E0382", "use of moved value: `p`");
        flags(&src, "E0382", "passed by value as `self` to `show`");
        // It also moves when a later use is of something else.
        flags(&host_of(" p.show()\n peek_p(&p)"), "E0382", "`p`");
        // A reference receiver does not.
        clean(&host_of(" p.get()\n p.get()\n p.bump()\n p.get()"));
        // Re-initialised in between.
        clean(&host_of(" p.show()\n p = o.inner\n p.show()"));
    }

    #[test]
    fn a_by_value_receiver_moves_under_inferred_only_if_it_consumes_self() {
        // `show` takes `self` but only reads it: lent, not moved.
        assert!(dflt_msgs(&host_of(" p.show()\n p.show()")).is_empty());
        // `finish` returns `self`: consumed.
        let m = dflt_msgs(&host_of(" p.finish()\n p.finish()"));
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(m[0].contains("use of moved value: `p`"), "{m:?}");
        // Through another consuming method, to a fixpoint.
        let relay = on_pair(" f finish(self) -> Pair { self }\n f relay(self) -> Pair { self.finish() }", "f host(p: Pair) {\n p.relay()\n p.relay()\n}");
        assert_eq!(dflt_msgs(&relay).len(), 1, "{:?}", dflt_msgs(&relay));
    }

    #[test]
    fn a_receiver_field_moves_that_field() {
        let src = format!("{OWNED}extend Pair {{\n f show(self) -> i32 {{ self.age }}\n}}\nf host(o: Outer) {{\n o.inner.show()\n take(o.inner.name)\n}}\n");
        flags(&src, "E0382", "use of moved value: `o.inner.name`");
        // The sibling field is untouched.
        clean(&format!("{OWNED}extend Pair {{\n f show(self) -> i32 {{ self.age }}\n}}\nf host(o: Outer) {{\n o.inner.show()\n take(o.tag)\n}}\n"));
    }

    #[test]
    fn a_reference_receiver_borrows_for_the_call() {
        // A live exclusive loan conflicts with a `&self` call, a live shared one
        // with a `&!self` call.
        flags(
            &host_of(" v r = &!p\n p.get()\n peek_p(r)"),
            "E0502",
            "cannot borrow `p` as immutable because it is also borrowed as mutable",
        );
        flags(
            &host_of(" v r = &p\n p.bump()\n peek_p(r)"),
            "E0502",
            "cannot borrow `p` as mutable because it is also borrowed as immutable",
        );
        // Shared with shared, or after the last use of the loan, is fine.
        clean(&host_of(" v r = &p\n p.get()\n peek_p(r)"));
        clean(&host_of(" v r = &!p\n peek_p(r)\n p.bump()"));
        // A loan on a sibling field does not conflict with a call on another field.
        clean(&host_of(" v r = &o.tag\n o.inner.bump()\n peek(r)"));
        // Both policies.
        assert_eq!(dflt_msgs(&host_of(" v r = &!p\n p.get()\n peek_p(r)")).len(), 1);
    }

    #[test]
    fn unknown_receivers_and_methods_are_not_reported() {
        // An unknown method, an unknown type, a method of another type, and a
        // receiver that is not a place.
        clean(&host_of(" p.nope()\n p.nope()"));
        clean("f host(x: Mystery) {\n x.show()\n x.show()\n}\n");
        clean(&host_of(" o.show()\n o.show()"));
        clean(&host_of(" make().show()\n make().show()"));
        // Methods on different types sharing a name are told apart by the receiver's type.
        let two = format!("{OWNED}data Other(n: s)\nextend Pair {{\n f go(self) {{ }}\n}}\nextend Other {{\n f go(&self) {{ }}\n}}\nf host(a: Pair, b: Other) {{\n b.go()\n b.go()\n}}\nf host2(a: Pair, b: Other) {{\n a.go()\n a.go()\n}}\n");
        let d = msgs(&two);
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("fn `host2`"), "{d:?}");
        // The same name with two receiver kinds on one type is not judged.
        let mixed = format!("{OWNED}extend Pair {{\n f go(self) {{ }}\n}}\nextend Pair {{\n f go(&self) {{ }}\n}}\nf host(a: Pair) {{\n a.go()\n a.go()\n}}\n");
        clean(&mixed);
        // A method on a Copy type never moves it.
        clean("@d(Copy)\ndata Dot(x: i32)\nextend Dot {\n f go(self) { }\n}\nf host(d: Dot) {\n d.go()\n d.go()\n}\n");
    }

    // ── destructuring patterns ───────────────────────────────────────

    const PAT: &str = "data Pair(name: s, age: i32)\ndata Outer(inner: Pair, tag: s)\ndata Msg = Text(s) | Num(i32)\ndata Env(msg: Msg, opt: ?s, pair: (s, i32), tag: s)\ndata Duo(both: (Pair, i32), tag: s)\nf take(x: s) { }\nf take_n(x: i32) { }\nf peek(x: &s) { }\nf peek_m(x: &Msg) { }\nf keep(x: s) -> s { x }\n";

    fn pt(body: &str) -> String {
        format!("{PAT}f host(p: Pair, o: Outer, e: Env, d: Duo) {{\n{body}\n}}\n")
    }

    // A `val` binder is `(..)`, `[..]` or a record pattern `@T { .. }`.
    #[test]
    fn a_let_pattern_moves_the_parts_it_binds_by_value() {
        let both = " val (@Pair { name, age }, k) = d.both";
        // The bound non-Copy field is gone, and so is the tuple it sits in.
        flags(&pt(&format!("{both}\n v q = d.both")), "E0382", "use of moved value: `d.both`");
        // Copy parts and wildcards move nothing.
        clean(&pt(" val (@Pair { name: _, age }, k) = d.both\n v q = d.both"));
        clean(&pt(" val (_, k) = d.both\n v q = d.both"));
        clean(&pt(&format!("{both}\n take(d.tag)\n take(name)")));
        // A bound name is itself a value that moves.
        flags(&pt(&format!("{both}\n take(name)\n v z = name")), "E0382", "`name`");
        // Under the inferred policy `let` is evidence enough, as for `let y = x.f`.
        assert_eq!(dflt(&pt(&format!("{both}\n v q = d.both"))).len(), 1);
        assert_eq!(dflt(&pt(" val (@Pair { name: _, age }, k) = d.both\n v q = d.both")).len(), 0);
        // A binding inside a record pattern inside a tuple pattern: renamed fields and nesting.
        clean(&pt(" val (@Pair { name: n, age: a }, k) = d.both\n take(n)\n take_n(a)"));
        flags(&pt(" val (@Pair { name: n, age: a }, k) = d.both\n take(n)\n take(n)"), "E0382", "`n`");
    }

    #[test]
    fn a_let_pattern_over_a_tuple_field_or_payload() {
        flags(&pt(" val (a, b) = e.pair\n v q = e.pair"), "E0382", "`e.pair`");
        clean(&pt(" val (a, b) = e.pair\n take_n(b)\n take(e.tag)"));
        clean(&pt(" val (_, b) = e.pair\n v q = e.pair"));
        // Only the part that moved is gone: its sibling is not.
        clean(&pt(" val (a, _) = e.pair\n take(e.tag)"));
        // A binding moves a whole non-Copy element, not just its parts.
        flags(&pt(" val (a, b) = e.pair\n val (c, d) = e.pair"), "E0382", "`e.pair.0`");
        // A payload pattern in a `let`.
        flags(&pt(" val Some(x) = e.opt\n v q = e.opt"), "E0382", "`e.opt`");
    }

    #[test]
    fn a_val_record_pattern_moves_the_fields_it_binds() {
        // `val @T { .. } = e` (MAGE_SPEC 4.8): the bound non-Copy field moves, the
        // rest stay usable, and the whole value is partly moved.
        let src = pt(" val @Pair { name } = p\n take(name)\n take(p.name)");
        flags(&src, "E0382", "use of moved value: `p.name`");
        flags(&pt(" val @Pair { name } = p\n v q = p"), "E0382", "use of moved value: `p`");
        clean(&pt(" val @Pair { name, age } = p\n take_n(age)\n take_n(p.age)\n take(name)"));
        // A Copy field, a wildcard field and a rename that is not used move nothing else.
        clean(&pt(" val @Pair { age } = p\n take(p.name)\n take_n(p.age)"));
        clean(&pt(" val @Pair { name: _, age } = p\n v q = p"));
        flags(&pt(" val @Pair { name: n } = p\n take(n)\n take(n)"), "E0382", "`n`");
        // Nested: the inner record's field.
        flags(&pt(" val @Outer { inner: @Pair { name } } = o\n take(o.inner.name)"), "E0382", "`o.inner.name`");
        clean(&pt(" val @Outer { inner: @Pair { name } } = o\n take(o.tag)"));
        // `var` is the same; destructuring the reference itself binds through it.
        flags(&pt(" var @Pair { name } = p\n take(p.name)"), "E0382", "`p.name`");
        clean(&format!("{PAT}f host(p: &Pair) {{\n val @Pair {{ name }} = p\n}}\n"));
    }

    #[test]
    fn a_let_pattern_over_something_unknown_moves_nothing_new() {
        // An unknown type, a call, a reference parameter: not followed.
        clean("f take(x: s) { }\nf host(w: Mystery) {\n val (a, b) = w\n take(w.name)\n take(w.name)\n}\n");
        clean("f take(x: s) { }\nf host() {\n val (a, b) = make()\n take(a)\n take(a)\n}\n");
        clean(&format!("{PAT}f host(p: &Mystery) {{\n val (a, b) = p.pair\n take(p.pair.x)\n v q = p.pair\n v z = p.pair\n}}\n"));
        // A part with a generic type.
        clean("data G[T](pair: (T, i32), name: s)\nf take(x: s) { }\nf host(g: G[s]) {\n val (a, b) = g.pair\n v q = g.pair\n v z = g.pair\n}\n");
        // A tuple pattern of the wrong arity binds nothing it can follow.
        clean(&pt(" val (a, b, c) = e.pair\n v q = e.pair"));
    }

    #[test]
    fn a_match_arm_moves_the_parts_it_binds_under_strict() {
        let m = " ?= p {\n  @Pair { name, age } => take(name)\n }";
        flags(&pt(&format!("{m}\n take(p.name)")), "E0382", "use of moved value: `p.name`");
        // The arm sees the scrutinee already partly moved.
        flags(&pt(" ?= p {\n  @Pair { name, age } => take(p.name)\n }"), "E0382", "`p.name`");
        // Copy parts and wildcards move nothing.
        clean(&pt(" ?= p {\n  @Pair { name: _, age } => take_n(age)\n }\n peek(&p.name)\n peek(&p.name)"));
        // A variant payload moves in only the arm that binds it, so afterwards it is
        // only possibly moved.
        let two = " ?= e.msg {\n  Text(s) => take(s)\n  Num(n) => take_n(n)\n }\n v q = e.msg";
        flags(&pt(two), "E0382", "possibly moved");
        clean(&pt(" ?= e.msg {\n  Text(_) => { }\n  Num(n) => take_n(n)\n }\n v q = e.msg"));
        // `Some` / `None` of an optional field.
        flags(&pt(" ?= e.opt {\n  Some(x) => take(x)\n  None => { }\n }\n v q = e.opt"), "E0382", "`e.opt`");
        // `match` is the same thing.
        flags(&pt(" match p {\n  @Pair { name, age } => take(name)\n }\n take(p.name)"), "E0382", "`p.name`");
        // A bound name known non-Copy moves in the arm.
        flags(&pt(" ?= e.msg {\n  Text(s) => {\n   take(s)\n   take(s)\n  }\n  Num(n) => { }\n }"), "E0382", "use of moved value: `s`");
    }

    #[test]
    fn a_match_arm_moves_under_inferred_only_if_it_consumes_the_binding() {
        let looks = " ?= p {\n  @Pair { name, age } => take(name)\n }\n take(p.name)";
        assert!(dflt(&pt(looks)).is_empty(), "{:?}", dflt(&pt(looks)));
        assert!(!msgs(&pt(looks)).is_empty());
        // `keep` consumes `name`, so `p.name` is gone after the match.
        let eats = " ?= p {\n  @Pair { name, age } => keep(name)\n }\n take(p.name)";
        let d = dflt(&pt(eats));
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("use of moved value: `p.name`"), "{d:?}");
        // A `let y = name` in the arm is evidence too.
        let lets = " ?= e.msg {\n  Text(s) => {\n   v t = s\n  }\n  Num(n) => { }\n }\n v q = e.msg";
        assert_eq!(dflt(&pt(lets)).len(), 1, "{:?}", dflt(&pt(lets)));
        // A payload merely looked at is not.
        assert!(dflt(&pt(" ?= e.msg {\n  Text(s) => peek(&s)\n  Num(n) => { }\n }\n v q = e.msg")).is_empty());
    }

    #[test]
    fn matching_a_partly_moved_place_reads_only_what_it_binds() {
        // `p.name` moved, the pattern touches only `p.age`: fine in Rust.
        clean(&pt(" take(p.name)\n ?= p {\n  @Pair { name: _, age } => take_n(age)\n }"));
        // The pattern binds the moved field: not fine.
        flags(&pt(" take(p.name)\n ?= p {\n  @Pair { name, age } => take(name)\n }"), "E0382", "`p.name`");
        // The place itself moved.
        flags(&pt(" v q = p\n ?= p {\n  @Pair { name: _, age } => take_n(age)\n }"), "E0382", "`p`");
        // `let` destructuring a partly moved value likewise.
        clean(&pt(" take(e.tag)\n val (a, b) = e.pair"));
        flags(&pt(" v q = e.pair\n val (a, b) = e.pair"), "E0382", "`e.pair`");
    }

    #[test]
    fn patterns_over_self() {
        let body = " f eat(self) {\n  ?= self {\n   @Pair { name, age } => take(name)\n  }\n  take(self.name)\n }";
        flags(&on_pair(body, ""), "E0382", "`self.name`");
        // A reference receiver binds by reference: nothing moves out of `self`
        // itself (only a field moved by value is E0507, never an E0382).
        for recv in ["&self", "&!self"] {
            let b = format!(" f eat({recv}) {{\n  ?= self {{\n   @Pair {{ name, age }} => take(name)\n  }}\n  take(&self.name)\n  take(&self.name)\n }}");
            clean(&on_pair(&b, ""));
            // A field behind the reference, destructured by value, is a move out of it.
            let l = format!(" f eat({recv}) {{\n  val (a, b) = self.pair\n  v q = self.pair\n }}");
            let src = format!("{PAT}extend Env {{\n{l}\n}}\n");
            flags(&src, "E0507", "`self.pair.0`");
            assert!(codes(&src, Policy::STRICT).iter().all(|c| c == "E0507"), "{:?}", codes(&src, Policy::STRICT));
            // Its Copy parts, and a wildcard, move nothing.
            clean(&format!("{PAT}extend Env {{\n f eat({recv}) {{\n  val (_, b) = self.pair\n  take_n(b)\n }}\n}}\n"));
        }
        // By value, a `let` destructure moves the field.
        flags(&format!("{PAT}extend Env {{\n f eat(self) {{\n  val (a, b) = self.pair\n  v q = self.pair\n }}\n}}\n"), "E0382", "`self.pair`");
    }
}
