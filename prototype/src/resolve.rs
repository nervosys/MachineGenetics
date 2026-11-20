/// MAGE Name Resolution — builds a symbol table and resolves identifiers.
///
/// Walks the AST to:
/// 1. Collect all definitions (functions, structs, enums, traits, consts, effects, modules)
/// 2. Build nested scope chains
/// 3. Resolve every identifier reference to a SymbolId
/// 4. Report unresolved names and duplicate definitions
use crate::ast;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity, SymbolId, Ty};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The builtin functions and macros `register_builtins` registers: names the resolver
/// accepts as a bare call. `every_builtin_function_is_stated` holds this table to the `builtin_function` facts in
/// both directions.
pub(crate) const STD_FNS: &[&str] = &[
    // I/O
    "println", "print", "eprintln", "eprint", "format", "write", "writeln",
    // construction / diagnostics
    "vec", "panic", "assert", "assert_eq", "assert_ne", "dbg", "todo", "unimplemented",
    "unreachable", "matches",
    // common free functions
    "min", "max", "abs", "relu", "clamp", "drop", "swap", "replace", "default",
    // modular arithmetic by name; the operators trap (§4.10)
    "wrapping_add", "wrapping_sub", "wrapping_mul",
    // smooth scalar functions (and list reductions), each with an
    // exact derivative for `grad`
    "exp", "ln", "log", "sqrt", "sin", "cos", "tan", "tanh", "sigmoid", "silu", "gelu",
    "log2", "log10", "asin", "acos", "atan", "sinh", "cosh",
    // rounding: piecewise constant, so the derivative is zero almost
    // everywhere (DIFFERENTIABILITY.md)
    "floor", "ceil", "round", "trunc",
    "mean", "dot",
    // the two list mutators: in place on the receiver's variable
    "push", "pop",
    // a trained net's forward pass on one input vector: `forward(Net, xs)`
    "forward",
    // bare enum-value constructors agents call positionally
    "Some", "None", "Ok", "Err",
];

/// The scalar math functions: each takes exactly one number, as `f(x)` or `x.f()`. The
/// evaluator's arm for them and `types.rs`'s arity and argument check use this table.
/// `every_scalar_math_name_is_stated` holds it to the `scalar_math` facts in both
/// directions.
pub(crate) const SCALAR_MATH: &[&str] = &[
    "exp", "ln", "log", "sqrt", "sin", "cos", "tan", "tanh", "sigmoid", "silu", "gelu",
    "log2", "log10", "asin", "acos", "atan", "sinh", "cosh",
    "floor", "ceil", "round", "trunc", "relu",
];

/// The list mutators: each changes its first argument (a variable or a
/// field/index path of one), as `push(xs, x)` / `xs.push(x)` or `pop(xs)` /
/// `xs.pop()`. They are in `STD_FNS` too; `types.rs` types them and `eval.rs`
/// performs them on the place, since a value-semantics call cannot.
pub(crate) const LIST_MUTATORS: &[&str] = &["push", "pop"];

/// What each `STD_FNS` name does in `--eval`: `(name, signature, summary)`. The
/// ontology's `builtins` section publishes this table, so an agent can look a
/// bare name up. `every_std_fn_is_documented` holds it to `STD_FNS` in both
/// directions, and `the_evaluator_accepts_exactly_the_published_names` holds
/// `STD_FNS`, `VOCABULARY` and `SCALAR_MATH` to the arms of `eval.rs`'s
/// `call_builtin_inner`. A name in `VOCABULARY` is documented there instead.
/// Every one is also a method: `x.f(a)` is `f(x, a)`.
pub(crate) const STD_FN_DOCS: &[(&str, &str, &str)] = &[
    ("println", "(any) -> ()", "print a value and a newline (effect io)"),
    ("print", "(any) -> ()", "print a value, no newline (effect io)"),
    ("eprintln", "(any) -> ()", "print a value and a newline to stderr (effect io)"),
    ("eprint", "(any) -> ()", "print a value to stderr, no newline (effect io)"),
    ("format", "(str) -> !", "registered but refused at run time: use an f-string, f\"hi {x}\""),
    ("write", "(any) -> ()", "print a value, no newline (effect io)"),
    ("writeln", "(any) -> ()", "print a value and a newline (effect io)"),
    ("vec", "(A, ..) -> [A]", "a list of the arguments"),
    ("panic", "(any) -> !", "stop the program with an error carrying the message"),
    ("assert", "(bool, str?) -> ()", "fail with an optional message unless the condition is true"),
    ("assert_eq", "(A, A) -> ()", "fail unless the two values are equal"),
    ("assert_ne", "(A, A) -> ()", "fail unless the two values differ"),
    ("dbg", "(A) -> A", "print a value and return it unchanged"),
    ("todo", "() -> !", "stop with an error saying it was reached"),
    ("unimplemented", "() -> !", "stop with an error saying it was reached"),
    ("unreachable", "() -> !", "stop with an error saying it was reached"),
    ("matches", "(..) -> !", "registered but refused at run time: use `?= value { pattern => 1b, _ => 0b }`"),
    ("min", "(A, A) -> A  or  [A] -> A", "the smaller of two values, or the smallest of a list; error on an empty list"),
    ("max", "(A, A) -> A  or  [A] -> A", "the larger of two values, or the largest of a list; error on an empty list"),
    ("abs", "(num) -> num", "absolute value; integer overflow is an error"),
    ("relu", "(f64) -> f64", "max(x, 0)"),
    ("clamp", "(num, num, num) -> num", "clamp(x, lo, hi): x limited to [lo, hi]; lo > hi is an error"),
    ("drop", "(any) -> ()", "discard a value"),
    ("swap", "(..) -> !", "registered but refused at run time: it needs a reference; rebind instead"),
    ("replace", "(..) -> !", "registered but refused at run time: it needs a reference; rebind instead"),
    ("default", "() -> !", "registered but refused at run time: it has no type to default to; write the value"),
    ("wrapping_add", "(int, int) -> int", "addition that wraps on overflow; the + operator traps"),
    ("wrapping_sub", "(int, int) -> int", "subtraction that wraps on overflow; the - operator traps"),
    ("wrapping_mul", "(int, int) -> int", "multiplication that wraps on overflow; the * operator traps"),
    ("exp", "(f64) -> f64", "e to the x"),
    ("ln", "(f64) -> f64", "natural logarithm"),
    ("log", "(f64) -> f64", "natural logarithm, as ln"),
    ("sqrt", "(f64) -> f64", "square root"),
    ("sin", "(f64) -> f64", "sine"),
    ("cos", "(f64) -> f64", "cosine"),
    ("tan", "(f64) -> f64", "tangent"),
    ("tanh", "(f64) -> f64", "hyperbolic tangent"),
    ("sigmoid", "(f64) -> f64", "1 / (1 + e^-x)"),
    ("silu", "(f64) -> f64", "x * sigmoid(x)"),
    ("gelu", "(f64) -> f64", "Gaussian error linear unit"),
    ("log2", "(f64) -> f64", "base-2 logarithm"),
    ("log10", "(f64) -> f64", "base-10 logarithm"),
    ("asin", "(f64) -> f64", "inverse sine"),
    ("acos", "(f64) -> f64", "inverse cosine"),
    ("atan", "(f64) -> f64", "inverse tangent"),
    ("sinh", "(f64) -> f64", "hyperbolic sine"),
    ("cosh", "(f64) -> f64", "hyperbolic cosine"),
    ("floor", "(f64) -> f64", "round down; derivative is zero almost everywhere"),
    ("ceil", "(f64) -> f64", "round up; derivative is zero almost everywhere"),
    ("round", "(f64) -> f64", "round to nearest; derivative is zero almost everywhere"),
    ("trunc", "(f64) -> f64", "round toward zero; derivative is zero almost everywhere"),
    ("mean", "([num]) -> f64", "arithmetic mean; error on an empty list"),
    ("dot", "([num], [num]) -> f64", "dot product; error if the lengths differ"),
    ("push", "([A], A) -> ()", "append a value to the list the receiver names, in place: xs.push(x) or push(xs, x); the receiver must be a variable or a field/index path of one"),
    ("pop", "([A]) -> ?A", "remove and return the last element of the list the receiver names, in place; none when it is empty: xs.pop() or pop(xs)"),
    ("forward", "(Net, [f64]) -> [f64]", "run a trained net's forward pass on one input vector (the net's train block supplies the weights)"),
    ("Some", "(A) -> ?A", "an option holding a value"),
    ("None", "() -> ?A", "the empty option"),
    ("Ok", "(A) -> Result", "the success case of a result"),
    ("Err", "(E) -> Result", "the failure case of a result"),
];

/// The standard SWE vocabulary (AB_INITIO_DESIGN §8 — the vocabulary frontier):
/// `(name, signature, summary)`. SINGLE SOURCE OF TRUTH — `register_builtins`
/// registers each name, `types::infer_vocab_call` types them, and the ontology's
/// `vocabulary` section publishes them for agent discovery (drift-proof: a test
/// asserts the ontology section covers every entry). High-frequency, single-
/// BPE-token (audited), total, capability-pure combinators so an agent NAMES an
/// intent instead of hand-rolling it (~65% fewer payload tokens, measured).
pub const VOCABULARY: &[(&str, &str, &str)] = &[
    ("map", "([A], A->B) -> [B]", "apply a function to each element"),
    ("filter", "([A], A->bool) -> [A]", "keep elements matching a predicate"),
    ("fold", "([A], B, (B,A)->B) -> B", "reduce with an initial accumulator"),
    ("reduce", "([A], (A,A)->A) -> ?A", "reduce without an initial value"),
    ("sum", "[A] -> A", "sum of the elements"),
    ("len", "[A] -> usize", "number of elements"),
    ("count", "[A] -> usize", "number of elements"),
    ("sort", "[A] -> [A]", "sorted copy"),
    ("reverse", "[A] -> [A]", "reversed copy"),
    ("zip", "([A], [B]) -> [(A,B)]", "pair up two collections"),
    ("freq", "[A] -> {A: usize}", "frequency of each element"),
    ("first", "[A] -> ?A", "first element, if any"),
    ("last", "[A] -> ?A", "last element, if any"),
    ("any", "([A], A->bool) -> bool", "does any element match"),
    ("all", "([A], A->bool) -> bool", "do all elements match"),
    ("find", "([A], A->bool) -> ?A", "first matching element, if any"),
    ("take", "([A], usize) -> [A]", "the first n elements"),
    ("range", "(usize) -> [usize]", "0..n as a list"),
    ("keys", "{K: V} -> [K]", "the map's keys"),
    ("values", "{K: V} -> [V]", "the map's values"),
    ("flatten", "[[A]] -> [A]", "flatten one level of nesting"),
    ("group", "([A], A->K) -> {K: [A]}", "group elements by key"),
    // Emits the seed first, so the result is one longer than the input:
    // `scan([1,2,3], 0, +)` is `[0, 1, 3, 6]`, not `[1, 3, 6]`. The two
    // conventions differ by exactly one element, which is invisible until you
    // count — and both are common enough that neither is obviously wrong.
    ("scan", "([A], B, (B,A)->B) -> [B]", "running fold, seed first (len+1 results)"),
    ("contains", "([A], A) -> bool", "membership test"),
    // String / text vocabulary (SWE is text-heavy).
    ("split", "(str, str) -> [str]", "split a string on a separator"),
    ("join", "([str], str) -> str", "join strings with a separator"),
    ("chars", "str -> [str]", "the characters of a string"),
    ("words", "str -> [str]", "whitespace-separated words"),
    ("lines", "str -> [str]", "newline-separated lines"),
    ("upper", "str -> str", "uppercase"),
    ("lower", "str -> str", "lowercase"),
];

// ── Symbol Table ─────────────────────────────────────────────────────

/// What kind of symbol was defined.
#[derive(Debug, Clone)]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    EnumVariant { parent: SymbolId },
    Trait,
    Module,
    TypeAlias,
    Const,
    Effect,
    Spec,
    Agent,
    Swarm,
    Net,
    Kb,
    Evolve,
    Train,
    Variable { mutable: bool },
    Param,
    GenericParam,
}

/// A symbol in the resolved program.
#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    /// The resolved type (filled in by the type checker, initially `None`).
    pub ty: Option<Ty>,
}

/// The symbol table produced by name resolution.
#[derive(Debug)]
pub struct SymbolTable {
    symbols: Vec<Symbol>,
    next_id: u32,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable { symbols: Vec::new(), next_id: 0 }
    }

    pub fn alloc(&mut self, name: String, kind: SymbolKind) -> SymbolId {
        let id = SymbolId(self.next_id);
        self.next_id += 1;
        self.symbols.push(Symbol { id, name, kind, ty: None });
        id
    }

    pub fn get(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }

    pub fn get_mut(&mut self, id: SymbolId) -> &mut Symbol {
        &mut self.symbols[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

// ── Scope chain ──────────────────────────────────────────────────────

/// A single lexical scope.
#[derive(Debug)]
struct Scope {
    /// name → SymbolId for items defined in this scope.
    names: HashMap<String, SymbolId>,
    /// name → SymbolId for type-namespace names (structs, enums, type aliases, traits).
    types: HashMap<String, SymbolId>,
    /// Names in this scope that came from the prelude rather than from source.
    ///
    /// A user definition may **shadow** one of these; only a collision between
    /// two *source* definitions is a duplicate. Without the distinction, the
    /// twenty capability namespaces (`io`, `net`, `fs`, `agent`, …) reserved
    /// those words globally, so `M net { … }` — the natural name for a module
    /// in a standard library — reported `duplicate definition: net` against a
    /// builtin the author never wrote and could not see.
    builtins: std::collections::HashSet<String>,
    /// Names present in `names` only as a *mirror* of a type-namespace entry.
    ///
    /// `define_type` copies its name into the value namespace so enum
    /// constructors resolve. That copy is a convenience, not a definition — but
    /// duplicate detection could not tell the difference, so every `S`, `T`,
    /// `Y`, `effect` and `sp` declaration reserved its name against functions.
    /// `S Point { … }` beside `f Point(…) -> Point` — the ordinary constructor
    /// pattern — reported `duplicate definition: Point`, and a `sp search { … }`
    /// block could not name the function it constrains, which is the entire
    /// mechanism by which a spec attaches to one.
    mirrored: std::collections::HashSet<String>,
}

impl Scope {
    fn new() -> Self {
        Scope {
            names: HashMap::new(),
            types: HashMap::new(),
            builtins: std::collections::HashSet::new(),
            mirrored: std::collections::HashSet::new(),
        }
    }
}

/// The resolver maintains a stack of scopes (innermost last).
pub struct Resolver {
    pub symbols: SymbolTable,
    pub diagnostics: Vec<Diagnostic>,
    /// Maps AST identifier strings to their resolved SymbolId, keyed by occurrence.
    /// (In a real compiler this would be per-node; here we track by name for simplicity.)
    pub resolved: HashMap<String, SymbolId>,
    scopes: Vec<Scope>,
    /// First line of the top-level item being resolved: the statement
    /// positions inside it are relative to it. `Some(0)` keeps spans
    /// item-relative (what the per-item engine query reports, rebased later);
    /// `None` means no position is known, so none is attached.
    item_start: Option<u32>,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver {
    pub fn new() -> Self {
        Resolver {
            symbols: SymbolTable::new(),
            diagnostics: Vec::new(),
            resolved: HashMap::new(),
            scopes: Vec::new(),
            item_start: None,
        }
    }

    /// Give the diagnostics reported since `before` that have no position the
    /// one `pos` names (inside the current top-level item). The innermost
    /// statement stamps first, so an outer one only fills what is left.
    fn stamp(&mut self, before: usize, pos: ast::Pos) {
        let span = pos.at(self.item_start);
        crate::hir::stamp_missing(&mut self.diagnostics[before..], span);
    }

    /// Pin what the current top-level item reported since `before`, and nothing
    /// finer placed, to the item's first line.
    fn stamp_item(&mut self, before: usize) {
        crate::hir::stamp_missing(&mut self.diagnostics[before..], crate::hir::Span::item(self.item_start));
    }

    // ── Scope management ─────────────────────────────────────────────

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define_value(&mut self, name: &str, kind: SymbolKind) -> SymbolId {
        let id = self.symbols.alloc(name.to_string(), kind);
        if let Some(scope) = self.scopes.last_mut() {
            // Shadowing a prelude name is allowed; colliding with another
            // source definition is not. `builtins` is what tells them apart.
            let shadows_builtin = scope.builtins.remove(name);
            let shadows_mirror = scope.mirrored.remove(name);
            if scope.names.contains_key(name) && !shadows_builtin && !shadows_mirror {
                self.diagnostics.push(Diagnostic::categorized(
                    Severity::Error,
                    format!("duplicate definition: `{name}`"),
                    DiagnosticCategory::DuplicateDefinition,
                    None,
                ));
            }
            scope.names.insert(name.to_string(), id);
        }
        id
    }

    /// Define a prelude name — one the compiler provides rather than one the
    /// author wrote. Source definitions shadow these silently.
    fn define_builtin(&mut self, name: &str, kind: SymbolKind) -> SymbolId {
        let id = self.define_value(name, kind);
        if let Some(scope) = self.scopes.last_mut() {
            scope.builtins.insert(name.to_string());
        }
        id
    }

    fn define_type(&mut self, name: &str, kind: SymbolKind) -> SymbolId {
        let id = self.symbols.alloc(name.to_string(), kind);
        if let Some(scope) = self.scopes.last_mut() {
            if scope.types.contains_key(name) {
                self.diagnostics.push(Diagnostic::categorized(
                    Severity::Error,
                    format!("duplicate type definition: `{name}`"),
                    DiagnosticCategory::DuplicateDefinition,
                    None,
                ));
            }
            scope.types.insert(name.to_string(), id);
            // Also make it available in value namespace (for enum constructors,
            // etc.) — but record it as a mirror, so a real value definition of
            // the same name is not reported as a duplicate against it.
            if !scope.names.contains_key(name) {
                scope.mirrored.insert(name.to_string());
            }
            scope.names.insert(name.to_string(), id);
        }
        id
    }

    fn lookup_value(&self, name: &str) -> Option<SymbolId> {
        for scope in self.scopes.iter().rev() {
            if let Some(&id) = scope.names.get(name) {
                return Some(id);
            }
        }
        None
    }

    fn lookup_type(&self, name: &str) -> Option<SymbolId> {
        for scope in self.scopes.iter().rev() {
            if let Some(&id) = scope.types.get(name) {
                return Some(id);
            }
        }
        None
    }

    // ── Top-level resolution ─────────────────────────────────────────

    pub fn resolve_module(&mut self, module: &ast::Module) {
        self.push_scope();
        self.register_builtins();

        // First pass: collect all top-level names (forward declarations). A
        // duplicate is reported against the item that defines the second one.
        for (i, item) in module.items.iter().enumerate() {
            self.item_start = module.start_of(i);
            let before = self.diagnostics.len();
            self.collect_item_name(item);
            self.stamp_item(before);
        }

        // Second pass: resolve bodies.
        for (i, item) in module.items.iter().enumerate() {
            self.item_start = module.start_of(i);
            let before = self.diagnostics.len();
            self.resolve_item(item);
            self.stamp_item(before);
        }
        self.item_start = None;

        // Third pass: contract clauses (each carries its own position).
        self.resolve_contract_clauses(module);

        self.pop_scope();
    }

    /// Contract clauses of a function resolve in its parameter scope, plus
    /// `result` in a postcondition (plan C1). A clause `contract::prepare`
    /// refuses is reported by the typechecker, once, so it is skipped here.
    fn resolve_contract_clauses(&mut self, module: &ast::Module) {
        for (clause, fd) in crate::contract::collect(module) {
            let Some(fd) = fd else { continue };
            let params: Vec<String> = fd.params.iter().map(|p| p.name.clone()).collect();
            let Ok(expr) = crate::contract::prepare(&clause, Some(&params)) else { continue };
            let before = self.diagnostics.len();
            self.push_scope();
            for p in &fd.params {
                self.define_value(&p.name, SymbolKind::Param);
            }
            if clause.kind == ast::ContractClauseKind::Ensures {
                self.define_value("result", SymbolKind::Variable { mutable: false });
            }
            self.resolve_expr(&expr);
            self.pop_scope();
            for d in &mut self.diagnostics[before..] {
                d.message = format!("{} in contract `{}` of `{}`", d.message, clause.display(), clause.function);
                d.span = d.span.or(clause.diag_span());
            }
        }
    }

    /// Register primitive type names so they can be resolved.
    fn register_builtins(&mut self) {
        let prims = [
            "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
            "f32", "f64", "bool", "char", "str",
        ];
        for name in prims {
            self.define_type(name, SymbolKind::TypeAlias);
        }
        // Builtin std container / wrapper types so annotations like `Vec<T>`,
        // `Option<T>`, `Result<T,E>`, `HashMap<K,V>` resolve in general code.
        let std_types = [
            "String", "Vec", "Option", "Result", "Box", "Rc", "Arc", "HashMap", "HashSet",
            "BTreeMap", "BTreeSet", "VecDeque",
        ];
        for name in std_types {
            self.define_type(name, SymbolKind::TypeAlias);
        }
        // Builtin functions / macros. MAGE surfaces println!/vec!/etc. as
        // ordinary calls (`println(...)`); without these registered, the most
        // basic agent-written program fails name resolution. This is the floor
        // for general-purpose (non-net) code to type-check.
        for name in STD_FNS {
            self.define_builtin(name, SymbolKind::Function);
        }
        // Standard SWE vocabulary (AB_INITIO_DESIGN §8) — registered from the
        // single-source VOCABULARY table (also typed in `types` and published in
        // the ontology). An agent names an intent instead of hand-rolling it.
        for (name, _sig, _doc) in VOCABULARY {
            self.define_builtin(name, SymbolKind::Function);
        }
        // Builtin capability namespaces. MAGE is effect-oriented: I/O is
        // performed through capability handles (`io.println(..)`, `fs.open(..)`,
        // `net.connect(..)`, `llm.complete(..)`) whose use is tracked by the
        // effect system. These are the standard library surface (like Rust's
        // std::io/std::fs) — registering them lets effect-qualified calls
        // resolve, which is how most real agent code performs side effects.
        //
        // The names come from `hir::CAPABILITY_NAMESPACES`, which also carries
        // the effect each one performs. They were two lists until the sentence
        // above turned out to be false — the names were registered here and
        // attributed nowhere, so `net.connect(…)` in a `pub` function declared
        // pure checked clean while a bare `println(…)` was caught. One list
        // means a namespace cannot be registered without an attribution
        // decision beside it.
        for (name, _) in crate::hir::CAPABILITY_NAMESPACES {
            self.define_builtin(name, SymbolKind::Const);
        }
    }

    /// First pass: register the name of a top-level item.
    fn collect_item_name(&mut self, item: &ast::Item) {
        self.apply_decls(&item_decls(item));
    }

    /// Define each declaration in the current scope, in order. A
    /// [`NameDecl::Variant`] belongs to the nearest preceding type.
    fn apply_decls(&mut self, decls: &[NameDecl]) {
        let mut parent: Option<SymbolId> = None;
        for d in decls {
            match d {
                NameDecl::Value { name, kind } => {
                    self.define_value(name, kind.symbol_kind());
                }
                NameDecl::Type { name, kind } => {
                    parent = Some(self.define_type(name, kind.symbol_kind()));
                }
                NameDecl::Variant { name } => {
                    let parent = parent.expect("a variant follows its type");
                    self.define_value(name, SymbolKind::EnumVariant { parent });
                }
            }
        }
    }

    // ── Item resolution ──────────────────────────────────────────────

    fn resolve_item(&mut self, item: &ast::Item) {
        match &item.kind {
            ast::ItemKind::Function(fd) => self.resolve_function(fd),
            ast::ItemKind::Struct(sd) => self.resolve_struct(sd),
            ast::ItemKind::Enum(ed) => self.resolve_enum(ed),
            ast::ItemKind::Trait(td) => self.resolve_trait(td),
            ast::ItemKind::Impl(ib) => self.resolve_impl(ib),
            ast::ItemKind::Module(md) => self.resolve_module_def(md),
            ast::ItemKind::Use(ud) => self.resolve_use(ud),
            ast::ItemKind::TypeAlias(ta) => self.resolve_type_alias(ta),
            ast::ItemKind::Const(cd) => self.resolve_const(cd),
            ast::ItemKind::Effect(ed) => self.resolve_effect(ed),
            // These three bodies are not resolved, but their generic
            // parameters can still carry bounds, and a bound discarded in a
            // `spec`, a `net` or a `data` is discarded exactly as silently as
            // one in a function. Reporting five of eight sites would have left
            // three to be found later.
            ast::ItemKind::Spec(sd) => {
                self.warn_discarded_bounds(&format!("`{}`", sd.name), &sd.generics, &[]);
                /* spec bodies are declarative, skip for now */
            }
            ast::ItemKind::Agent(_) => { /* agent bodies are declarative, skip for now */ }
            ast::ItemKind::Swarm(_) => { /* swarm bodies are declarative, skip for now */ }
            ast::ItemKind::Net(nd) => {
                self.warn_discarded_bounds(&format!("`{}`", nd.name), &nd.generics, &[]);
                /* net bodies resolved later */
            }
            ast::ItemKind::Kb(_) => { /* kb bodies resolved later */ }
            ast::ItemKind::Evolve(_) => { /* evolve bodies resolved later */ }
            ast::ItemKind::Train(_) => { /* train bodies resolved later */ }
            ast::ItemKind::Static(sd) => {
                self.resolve_ast_type(&sd.ty);
                self.resolve_expr(&sd.value);
            }
            ast::ItemKind::Data(dd) => {
                self.warn_discarded_bounds(&format!("`{}`", dd.name), &dd.generics, &[]);
                // A field default is an expression written in the declaration:
                // it resolves in module scope, like a `const` initialiser.
                if let ast::DataKind::Record(fields) = &dd.kind {
                    for f in fields {
                        if let Some(d) = &f.default {
                            self.resolve_expr(d);
                        }
                    }
                }
            }
            ast::ItemKind::Extend(eb) => {
                self.push_scope();
                self.resolve_ast_type(&eb.target_type);
                self.define_self_type();
                for item in &eb.items {
                    self.resolve_item(item);
                }
                self.pop_scope();
            }
        }
    }

    fn resolve_function(&mut self, fd: &ast::FunctionDef) {
        let first = self.diagnostics.len();
        self.resolve_function_inner(fd);
        // What the body left unplaced (a parameter's type, a bound) belongs to
        // the function's start.
        self.stamp(first, fd.body.pos);
    }

    fn resolve_function_inner(&mut self, fd: &ast::FunctionDef) {
        self.warn_discarded_bounds(&format!("`{}`", fd.name), &fd.generics, &fd.where_clause);
        self.push_scope();

        // Generic params.
        for gp in &fd.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }

        // Parameters.
        for param in &fd.params {
            self.define_value(&param.name, SymbolKind::Param);
            self.resolve_ast_type(&param.ty);
        }

        // Return type.
        if let Some(ret) = &fd.return_type {
            self.resolve_ast_type(ret);
        }

        // Body.
        if let Some(be) = &fd.body_expr {
            self.resolve_expr(be);
        } else {
            self.resolve_block(&fd.body);
        }

        self.pop_scope();
    }

    fn resolve_struct(&mut self, sd: &ast::StructDef) {
        self.warn_discarded_bounds(&format!("`{}`", sd.name), &sd.generics, &[]);
        self.push_scope();
        for gp in &sd.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }
        for field in &sd.fields {
            self.resolve_ast_type(&field.ty);
        }
        self.pop_scope();
    }

    fn resolve_enum(&mut self, ed: &ast::EnumDef) {
        self.warn_discarded_bounds(&format!("`{}`", ed.name), &ed.generics, &[]);
        self.push_scope();
        for gp in &ed.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }
        for variant in &ed.variants {
            match &variant.kind {
                ast::VariantKind::Unit => {}
                ast::VariantKind::Tuple(types) => {
                    for ty in types {
                        self.resolve_ast_type(ty);
                    }
                }
                ast::VariantKind::Struct(fields) => {
                    for field in fields {
                        self.resolve_ast_type(&field.ty);
                    }
                }
            }
        }
        self.pop_scope();
    }

    fn resolve_trait(&mut self, td: &ast::TraitDef) {
        self.warn_discarded_bounds(&format!("`{}`", td.name), &td.generics, &[]);
        self.push_scope();
        for gp in &td.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }
        self.define_self_type();
        for item in &td.items {
            self.resolve_item(item);
        }
        self.pop_scope();
    }

    fn resolve_impl(&mut self, ib: &ast::ImplBlock) {
        // An impl block has no name of its own; the trait it implements is the
        // most useful thing to point at, and there is not always one.
        let owner = match &ib.trait_path {
            Some(path) => format!("the impl of `{}`", path.join(".")),
            None => "an impl block".to_string(),
        };
        self.warn_discarded_bounds(&owner, &ib.generics, &[]);
        self.push_scope();
        for gp in &ib.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }
        self.resolve_ast_type(&ib.self_type);
        self.define_self_type();
        for item in &ib.items {
            self.resolve_item(item);
        }
        self.pop_scope();
    }

    /// Bind `Self` inside an `impl`, `trait`, or `extend` body.
    ///
    /// The parser desugars *every* `self` receiver into a parameter of type
    /// `Self` (see `parser.rs`), and `Self` was never put in scope — so it
    /// resolved to nothing and each method reported `unresolved type: Self`.
    /// The effect was total rather than partial: no method with a receiver, and
    /// no `-> Self` constructor, could be written in either a trait or an impl,
    /// which is why every example that used one failed. Scoped per body, so it
    /// disappears again outside.
    fn define_self_type(&mut self) {
        self.define_type("Self", SymbolKind::TypeAlias);
    }

    fn resolve_module_def(&mut self, md: &ast::ModuleDef) {
        if let Some(items) = &md.items {
            self.push_scope();
            for item in items {
                self.collect_item_name(item);
            }
            for item in items {
                self.resolve_item(item);
            }
            self.pop_scope();
        }
    }

    /// `use` is accepted and brings nothing into scope — say so.
    ///
    /// MAGE has no module system. This function was empty, under a comment
    /// describing what it would do, and `internals/03` §3.2 documented four
    /// resolution steps and six import styles none of which happen. The
    /// consequence for a generated program is the worst shape an error can
    /// take: the `use` is accepted silently, and the failure surfaces later at
    /// the *call site* as `unresolved name`, pointing at the one line the
    /// author wrote correctly. `u totally.made.up.path` is accepted too.
    ///
    /// The library surface is global and needs no import — the standard
    /// vocabulary (`map`, `filter`, `join`, …) and the capability namespaces
    /// (`io`, `fs`, `net`, …) are in scope everywhere, which is the right
    /// default for a language optimising for tokens: an import costs tokens
    /// and buys nothing here.
    ///
    /// **This was a warning until 2026-08-19, and is now an error.** The reason
    /// it was a warning — "rejecting the syntax outright would break the corpus
    /// for no gain" — held while the one-flat-namespace design was still
    /// undecided and `stdlib/` described an import-based library. Both changed:
    /// `MAGE_SPEC.md` §2.3 now states the design normatively, `stdlib/` is
    /// gone, and the corpus cost turned out to be a single line in one example.
    /// A construct that can never mean anything should not typecheck.
    fn resolve_use(&mut self, ud: &ast::UseDef) {
        let path = ud.path.join(".");
        self.diagnostics.push(Diagnostic::categorized(
            Severity::Error,
            format!(
                "`use {path}` cannot bring anything into scope — MAGE has one flat \
                 namespace and no module system (MAGE_SPEC.md §2.3). The standard \
                 vocabulary and the capability namespaces (`io`, `fs`, `net`, …) are \
                 already in scope everywhere; delete the import"
            ),
            DiagnosticCategory::UnresolvedName,
            None,
        ));
    }

    // ── Bounds ───────────────────────────────────────────────────────
    //
    // `[T: Bound]` and `~> T: Bound` both parse into a `Vec<String>` that is
    // stored and never resolved. Every consumer of that field prints it
    // (`fmt`), strips lifetimes from it (`elision`) or counts its tokens
    // (`token_budget`); `types.rs` does not mention `bounds` at all, and has
    // no obligations and no impl table to check one against. So
    //
    //     f describe[T](v: T) -> str ~> T: TotallyMadeUpTrait { "described" }
    //
    // reported `Errors: 0`, `Status: OK` — a constraint naming a trait that
    // exists nowhere, accepted in silence.
    //
    // This is a **warning**, not an error, and deliberately so on both counts.
    //
    // Not an error, because the bound is not wrong. It records what the author
    // meant, `quick-start/03-syntax-tour.md` and `migration-guide/04-types.md`
    // both teach writing one, and rejecting them would fail documentation this
    // repository certifies. Nor can the name be resolved and the unknown ones
    // rejected: `Clone`, `Display` and `Ord` are declared in no MAGE source,
    // so "unknown trait" would fire on every *correct* bound. There is no
    // trait universe to resolve against, which is the same finding from the
    // other side.
    //
    // But not silence either. A program that looks constrained and is not is
    // worse than one that never claimed to be — the shape this repository
    // keeps finding, and the reason `use` was given a diagnostic rather than
    // being left to do nothing quietly. Enforcing bounds is a feature; saying
    // they are not enforced is a sentence.

    fn warn_discarded_bound(&mut self, owner: &str, param: &str, bounds: &[String]) {
        if bounds.is_empty() {
            return;
        }
        let listed = bounds.join(" + ");
        self.diagnostics.push(Diagnostic::categorized(
            Severity::Warning,
            format!(
                "{owner}: the bound `{param}: {listed}` is parsed and then discarded — MAGE has no trait solving, so it constrains nothing and a call that violates it still reports `Errors: 0`. Keep it as documentation of intent, or remove it"
            ),
            DiagnosticCategory::Other,
            None,
        ));
    }

    /// Every place a bound can be written on one item: the inline generic
    /// bounds and, for functions, the `~>` clause. Both are discarded, so
    /// both are reported, and by one path so neither can be forgotten.
    fn warn_discarded_bounds(
        &mut self,
        owner: &str,
        generics: &[ast::GenericParam],
        where_clause: &[ast::WherePredicate],
    ) {
        for gp in generics {
            self.warn_discarded_bound(owner, &gp.name, &gp.bounds);
        }
        for pred in where_clause {
            self.warn_discarded_bound(owner, &pred.type_param, &pred.bounds);
        }
    }

    fn resolve_type_alias(&mut self, ta: &ast::TypeAlias) {
        self.warn_discarded_bounds(&format!("`{}`", ta.name), &ta.generics, &[]);
        self.push_scope();
        for gp in &ta.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }
        self.resolve_ast_type(&ta.ty);
        self.pop_scope();
    }

    fn resolve_const(&mut self, cd: &ast::ConstDef) {
        self.resolve_ast_type(&cd.ty);
        self.resolve_expr(&cd.value);
    }

    fn resolve_effect(&mut self, ed: &ast::EffectDef) {
        // The effect's generics are in scope for every operation, and each
        // operation's own generics are in scope for its signature only.
        self.warn_discarded_bounds(&format!("`{}`", ed.name), &ed.generics, &[]);
        self.push_scope();
        for gp in &ed.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }
        for op in &ed.operations {
            self.warn_discarded_bounds(&format!("`{}.{}`", ed.name, op.name), &op.generics, &[]);
            self.push_scope();
            for gp in &op.generics {
                self.define_type(&gp.name, SymbolKind::GenericParam);
            }
            for param in &op.params {
                self.resolve_ast_type(&param.ty);
            }
            if let Some(ret) = &op.return_type {
                self.resolve_ast_type(ret);
            }
            self.pop_scope();
        }
        self.pop_scope();
    }

    // ── Type resolution ──────────────────────────────────────────────

    fn resolve_ast_type(&mut self, ty: &ast::Type) {
        match ty {
            ast::Type::Path { segments, type_args } => {
                if let Some(name) = segments.first() {
                    if self.lookup_type(name).is_none() && self.lookup_value(name).is_none() {
                        self.diagnostics.push(Diagnostic::categorized(
                            Severity::Error,
                            format!("unresolved type: `{}`", segments.join(".")),
                            DiagnosticCategory::UnresolvedType,
                            None,
                        ));
                    } else {
                        // Record resolution.
                        if let Some(id) = self.lookup_type(name) {
                            self.resolved.insert(segments.join("."), id);
                        }
                    }
                }
                for arg in type_args {
                    self.resolve_ast_type(arg);
                }
            }
            ast::Type::Reference { inner, .. }
            | ast::Type::OwnedPtr { inner }
            | ast::Type::Rc { inner }
            | ast::Type::Arc { inner }
            | ast::Type::Cow { inner }
            | ast::Type::Cell { inner }
            | ast::Type::RefCell { inner }
            | ast::Type::Mutex { inner }
            | ast::Type::RwLock { inner }
            | ast::Type::Slice { inner }
            | ast::Type::Vec { inner }
            | ast::Type::Set { inner }
            | ast::Type::Option { inner }
            | ast::Type::Ptr { inner } => {
                self.resolve_ast_type(inner);
            }
            ast::Type::Array { inner, .. } => {
                self.resolve_ast_type(inner);
            }
            ast::Type::Result { ok, err } => {
                self.resolve_ast_type(ok);
                self.resolve_ast_type(err);
            }
            ast::Type::Map { key, value } => {
                self.resolve_ast_type(key);
                self.resolve_ast_type(value);
            }
            ast::Type::Simd { inner, .. } => {
                self.resolve_ast_type(inner);
            }
            ast::Type::Tuple { elements } => {
                for el in elements {
                    self.resolve_ast_type(el);
                }
            }
            ast::Type::Fn { params, ret } => {
                for p in params {
                    self.resolve_ast_type(p);
                }
                if let Some(r) = ret {
                    self.resolve_ast_type(r);
                }
            }
            // Primitives / wildcards — nothing to resolve.
            ast::Type::Never
            | ast::Type::Inferred
            | ast::Type::SelfType
            | ast::Type::StringType
            | ast::Type::KnowledgeBase
            | ast::Type::LlmType => {}
            ast::Type::Tensor { inner, .. }
            | ast::Type::ParamTy { inner, .. }
            | ast::Type::Genome { inner } => {
                self.resolve_ast_type(inner);
            }
            ast::Type::Policy { state, action } => {
                self.resolve_ast_type(state);
                self.resolve_ast_type(action);
            }
            ast::Type::Refined { base, .. } | ast::Type::Dimensioned { base, .. } => {
                self.resolve_ast_type(base);
            }
        }
    }

    // ── Block & statement resolution ─────────────────────────────────

    fn resolve_block(&mut self, block: &ast::Block) {
        self.push_scope();
        for (i, stmt) in block.stmts.iter().enumerate() {
            let before = self.diagnostics.len();
            self.resolve_stmt(stmt);
            if let Some(p) = block.stmt_pos.get(i) {
                self.stamp(before, *p);
            }
        }
        if let Some(tail) = &block.tail_expr {
            let before = self.diagnostics.len();
            self.resolve_expr(tail);
            self.stamp(before, block.tail_pos);
        }
        self.pop_scope();
    }

    fn resolve_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Let { mutable, pattern, ty, value } => {
                // Resolve RHS first (before binding the pattern).
                self.resolve_expr(value);
                if let Some(t) = ty {
                    self.resolve_ast_type(t);
                }
                self.resolve_pattern(pattern, *mutable);
            }
            ast::Stmt::Expr { expr } => {
                self.resolve_expr(expr);
            }
            ast::Stmt::Item { item } => {
                self.collect_item_name(item);
                self.resolve_item(item);
            }
            ast::Stmt::Guard { cond, else_block } => {
                self.resolve_expr(cond);
                self.resolve_block(else_block);
            }
            ast::Stmt::Defer { expr } => {
                self.resolve_expr(expr);
            }
        }
    }

    fn resolve_pattern(&mut self, pattern: &ast::Pattern, mutable: bool) {
        match pattern {
            ast::Pattern::Ident { name } => {
                self.define_value(name, SymbolKind::Variable { mutable });
            }
            ast::Pattern::Wildcard | ast::Pattern::Literal { .. } => {}
            ast::Pattern::Tuple { elements } => {
                for el in elements {
                    self.resolve_pattern(el, mutable);
                }
            }
            ast::Pattern::Struct { path, fields } => {
                if let Some(name) = path.first()
                    && self.lookup_type(name).is_none() {
                        self.diagnostics.push(Diagnostic::categorized(
                            Severity::Error,
                            format!("unresolved type in pattern: `{}`", path.join(".")),
                            DiagnosticCategory::UnresolvedType,
                            None,
                        ));
                    }
                for fp in fields {
                    if let Some(pat) = &fp.pattern {
                        self.resolve_pattern(pat, mutable);
                    } else {
                        // Shorthand field pattern — binds `fp.name`
                        self.define_value(&fp.name, SymbolKind::Variable { mutable });
                    }
                }
            }
            ast::Pattern::Enum { path, elements } => {
                if let Some(name) = path.first()
                    && self.lookup_value(name).is_none() && self.lookup_type(name).is_none() {
                        self.diagnostics.push(Diagnostic::categorized(
                            Severity::Error,
                            format!("unresolved variant in pattern: `{}`", path.join(".")),
                            DiagnosticCategory::UnresolvedName,
                            None,
                        ));
                    }
                for el in elements {
                    self.resolve_pattern(el, mutable);
                }
            }
            ast::Pattern::Slice { elements, .. } => {
                for el in elements {
                    self.resolve_pattern(el, mutable);
                }
            }
            ast::Pattern::Or { patterns } => {
                for p in patterns {
                    self.resolve_pattern(p, mutable);
                }
            }
            ast::Pattern::Ref { pattern } => {
                self.resolve_pattern(pattern, mutable);
            }
        }
    }

    // ── Expression resolution ────────────────────────────────────────

    fn resolve_expr(&mut self, expr: &ast::Expr) {
        match expr {
            ast::Expr::Ident { name } => {
                if let Some(id) = self.lookup_value(name) {
                    self.resolved.insert(name.clone(), id);
                } else {
                    // `result` and `old(x)` are names only a postcondition
                    // has (plan C1). Written in a body, say so, rather than
                    // leave a bare `unresolved name` for something the
                    // contract syntax taught.
                    let hint = match name.as_str() {
                        "result" => {
                            " (`result` names the return value only inside `@ens`)"
                        }
                        "old" => " (`old(x)` names a parameter only inside `@ens`)",
                        _ => "",
                    };
                    self.diagnostics.push(Diagnostic::categorized(
                        Severity::Error,
                        format!("unresolved name: `{name}`{hint}"),
                        DiagnosticCategory::UnresolvedName,
                        None,
                    ));
                }
            }
            ast::Expr::Literal { .. } => {}
            ast::Expr::Binary { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            ast::Expr::Unary { operand, .. } => {
                self.resolve_expr(operand);
            }
            ast::Expr::Call { func, args } => {
                self.resolve_expr(func);
                // `forward(Net, xs)` names a `net` in its first argument. A net is a type
                // with no value, so a bare name that is no value is left to the type
                // checker, which says what that argument must be.
                let net_arg = matches!(func.as_ref(), ast::Expr::Ident { name } if name == "forward")
                    && args.len() == 2
                    && matches!(&args[0], ast::Expr::Ident { name } if self.lookup_value(name).is_none());
                for (i, arg) in args.iter().enumerate() {
                    if i == 0 && net_arg {
                        continue;
                    }
                    self.resolve_expr(arg);
                }
            }
            ast::Expr::MethodCall { receiver, args, type_args, .. } => {
                self.resolve_expr(receiver);
                for arg in args {
                    self.resolve_expr(arg);
                }
                for ta in type_args {
                    self.resolve_ast_type(ta);
                }
            }
            ast::Expr::FieldAccess { object, .. } => {
                self.resolve_expr(object);
            }
            ast::Expr::Index { object, index } => {
                self.resolve_expr(object);
                self.resolve_expr(index);
            }
            ast::Expr::StructLit { path, fields } => {
                if let Some(name) = path.first()
                    && self.lookup_type(name).is_none() {
                        self.diagnostics.push(Diagnostic::categorized(
                            Severity::Error,
                            format!("unresolved struct: `{}`", path.join(".")),
                            DiagnosticCategory::UnresolvedType,
                            None,
                        ));
                    }
                for fi in fields {
                    if let Some(val) = &fi.value {
                        self.resolve_expr(val);
                    }
                }
            }
            ast::Expr::TupleLit { elements } | ast::Expr::ArrayLit { elements } => {
                for el in elements {
                    self.resolve_expr(el);
                }
            }
            ast::Expr::MapLit { entries } => {
                for (k, v) in entries {
                    self.resolve_expr(k);
                    self.resolve_expr(v);
                }
            }
            ast::Expr::ArrayRepeat { value, count } => {
                self.resolve_expr(value);
                self.resolve_expr(count);
            }
            ast::Expr::Closure { params, body } => {
                self.push_scope();
                for param in params {
                    self.define_value(&param.name, SymbolKind::Param);
                    self.resolve_ast_type(&param.ty);
                }
                self.resolve_expr(body);
                self.pop_scope();
            }
            ast::Expr::If { cond, then_block, else_block } => {
                self.resolve_expr(cond);
                self.resolve_block(then_block);
                if let Some(eb) = else_block {
                    self.resolve_block(eb);
                }
            }
            ast::Expr::Match { arms, .. } => {
                for arm in arms {
                    self.push_scope();
                    self.resolve_pattern(&arm.pattern, false);
                    self.resolve_expr(&arm.body);
                    self.pop_scope();
                }
            }
            // The handled body resolves in the enclosing scope; each arm gets
            // its own, with the operation's parameters bound as plain names.
            // Their types come from the `effect` declaration, so the resolver
            // only has to make them visible.
            ast::Expr::Handle { body, arms, .. } => {
                self.resolve_block(body);
                for arm in arms {
                    self.push_scope();
                    for p in &arm.params {
                        self.define_value(p, SymbolKind::Variable { mutable: false });
                    }
                    self.resolve_expr(&arm.body);
                    self.pop_scope();
                }
            }
            ast::Expr::Loop { body } => {
                self.resolve_block(body);
            }
            ast::Expr::While { cond, body } => {
                self.resolve_expr(cond);
                self.resolve_block(body);
            }
            ast::Expr::For { pattern, iter, body } => {
                self.resolve_expr(iter);
                self.push_scope();
                self.resolve_pattern(pattern, false);
                self.resolve_block(body);
                self.pop_scope();
            }
            ast::Expr::Block { block } => {
                self.resolve_block(block);
            }
            ast::Expr::Return { value } | ast::Expr::Break { value } => {
                if let Some(v) = value {
                    self.resolve_expr(v);
                }
            }
            ast::Expr::Continue => {}
            ast::Expr::Todo | ast::Expr::Unimplemented => {}
            ast::Expr::Try { expr } | ast::Expr::Await { expr } => {
                self.resolve_expr(expr);
            }
            ast::Expr::Cast { expr, ty } => {
                self.resolve_expr(expr);
                self.resolve_ast_type(ty);
            }
            ast::Expr::Assign { target, value } => {
                self.resolve_expr(target);
                self.resolve_expr(value);
            }
            ast::Expr::Range { start, end, .. } => {
                // An open end (`xs[a..]`, `xs[..b]`) is the parser's `_`
                // sentinel, not a name.
                for bound in [start, end] {
                    if !matches!(&**bound, ast::Expr::Ident { name } if name == "_") {
                        self.resolve_expr(bound);
                    }
                }
            }
            ast::Expr::Pipeline { left, right } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            // Both operands resolve in the enclosing scope. `w` in
            // `grad(e, w)` is a *use* of an existing binding, not a new
            // one — `grad(loss, undeclared)` must say `unresolved name`
            // like any other reference.
            ast::Expr::Grad { value, wrt } => {
                self.resolve_expr(value);
                self.resolve_expr(wrt);
            }
            ast::Expr::Is { expr, .. } => {
                self.resolve_expr(expr);
            }
            ast::Expr::Error { .. } => {}
        }
    }
}

// ── Per-item resolution ──────────────────────────────────────────────
//
// `resolve_module` is three passes: collect every top-level *name*, resolve
// each item's body against those names, then resolve contract clauses. Pass two
// reads nothing of another item but its name, so what an item's resolution
// depends on is that item plus the list of names — not the other bodies. The
// incremental engine (`query.rs`) uses these entry points to re-resolve one
// edited function against an unchanged name list. `resolve_module` and these
// share every helper, and the engine's corpus test holds the concatenation of
// their output to `resolve()`'s.

/// Which symbol a [`NameDecl`] defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeclKind {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    TypeAlias,
    Const,
    Effect,
    Spec,
    Agent,
    Swarm,
    Net,
    Kb,
    Evolve,
    Train,
}

impl DeclKind {
    fn symbol_kind(self) -> SymbolKind {
        match self {
            DeclKind::Function => SymbolKind::Function,
            DeclKind::Struct => SymbolKind::Struct,
            DeclKind::Enum => SymbolKind::Enum,
            DeclKind::Trait => SymbolKind::Trait,
            DeclKind::Module => SymbolKind::Module,
            DeclKind::TypeAlias => SymbolKind::TypeAlias,
            DeclKind::Const => SymbolKind::Const,
            DeclKind::Effect => SymbolKind::Effect,
            DeclKind::Spec => SymbolKind::Spec,
            DeclKind::Agent => SymbolKind::Agent,
            DeclKind::Swarm => SymbolKind::Swarm,
            DeclKind::Net => SymbolKind::Net,
            DeclKind::Kb => SymbolKind::Kb,
            DeclKind::Evolve => SymbolKind::Evolve,
            DeclKind::Train => SymbolKind::Train,
        }
    }
}

/// One name a top-level item puts in scope.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NameDecl {
    /// A name in the value namespace.
    Value { name: String, kind: DeclKind },
    /// A name in the type namespace (also mirrored into the value namespace).
    Type { name: String, kind: DeclKind },
    /// An enum variant, a value whose parent is the nearest preceding `Type`.
    Variant { name: String },
}

/// The names `item` brings into the module scope, in the order they are defined.
pub fn item_decls(item: &ast::Item) -> Vec<NameDecl> {
    let value = |name: &str, kind| vec![NameDecl::Value { name: name.to_string(), kind }];
    let ty = |name: &str, kind| vec![NameDecl::Type { name: name.to_string(), kind }];
    let sum = |name: &str, variants: Vec<&String>| {
        let mut out = vec![NameDecl::Type { name: name.to_string(), kind: DeclKind::Enum }];
        out.extend(variants.into_iter().map(|v| NameDecl::Variant { name: v.clone() }));
        out
    };
    match &item.kind {
        ast::ItemKind::Function(fd) => value(&fd.name, DeclKind::Function),
        ast::ItemKind::Struct(sd) => ty(&sd.name, DeclKind::Struct),
        ast::ItemKind::Enum(ed) => sum(&ed.name, ed.variants.iter().map(|v| &v.name).collect()),
        ast::ItemKind::Trait(td) => ty(&td.name, DeclKind::Trait),
        ast::ItemKind::Module(md) => value(&md.name, DeclKind::Module),
        ast::ItemKind::TypeAlias(ta) => ty(&ta.name, DeclKind::TypeAlias),
        ast::ItemKind::Const(cd) => value(&cd.name, DeclKind::Const),
        ast::ItemKind::Effect(ed) => ty(&ed.name, DeclKind::Effect),
        ast::ItemKind::Spec(sd) => ty(&sd.name, DeclKind::Spec),
        ast::ItemKind::Static(sd) => value(&sd.name, DeclKind::Const),
        ast::ItemKind::Agent(ad) => value(&ad.name, DeclKind::Agent),
        ast::ItemKind::Swarm(s) => value(&s.name, DeclKind::Swarm),
        ast::ItemKind::Net(n) => ty(&n.name, DeclKind::Net),
        ast::ItemKind::Kb(k) => ty(&k.name, DeclKind::Kb),
        ast::ItemKind::Evolve(e) => ty(&e.name, DeclKind::Evolve),
        ast::ItemKind::Train(t) => value(&t.name, DeclKind::Train),
        ast::ItemKind::Data(dd) => match &dd.kind {
            ast::DataKind::Record(_) => ty(&dd.name, DeclKind::Struct),
            ast::DataKind::Sum(variants) => sum(&dd.name, variants.iter().map(|v| &v.name).collect()),
        },
        // These introduce no single name.
        ast::ItemKind::Extend(_) | ast::ItemKind::Impl(_) | ast::ItemKind::Use(_) => Vec::new(),
    }
}

/// What one slice of resolution produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partial {
    pub diagnostics: Vec<Diagnostic>,
    /// Symbols the slice allocated, so the parts add up to `Resolver::symbols.len()`.
    pub symbols: usize,
}

impl Resolver {
    /// A resolver stopped after pass one: builtins and `decls` defined.
    fn with_env(decls: &[NameDecl]) -> Resolver {
        let mut r = Resolver::new();
        r.push_scope();
        r.register_builtins();
        r.apply_decls(decls);
        r
    }

    fn since(&self, diags: usize, symbols: usize) -> Partial {
        Partial {
            diagnostics: self.diagnostics[diags..].to_vec(),
            symbols: self.symbols.len() - symbols,
        }
    }
}

/// Pass one alone: the duplicate-definition diagnostics of the name list, and the
/// symbols the builtins and the list allocate.
pub fn resolve_names(decls: &[NameDecl]) -> Partial {
    Resolver::with_env(decls).since(0, 0)
}

/// Pass one over the names of each top-level item (`per_item[i]` are item `i`'s
/// declarations). The same diagnostics as [`resolve_names`] over the
/// concatenation, plus, for each, the index of the item that caused it, so a
/// driver can pin it to that item's line.
pub fn resolve_names_by_item(per_item: &[Vec<NameDecl>]) -> (Partial, Vec<usize>) {
    let mut r = Resolver::with_env(&[]);
    let mut owners = Vec::new();
    for (i, decls) in per_item.iter().enumerate() {
        r.apply_decls(decls);
        owners.resize(r.diagnostics.len(), i);
    }
    (r.since(0, 0), owners)
}

/// Pass two for one item, against `decls`. Spans are relative to the item's
/// first line (0 on that line), so the answer does not change when the item
/// moves; [`crate::hir::rebase`] with the item's start makes them absolute.
pub fn resolve_item_in(decls: &[NameDecl], item: &ast::Item) -> Partial {
    let mut r = Resolver::with_env(decls);
    r.item_start = Some(0);
    let (d, s) = (r.diagnostics.len(), r.symbols.len());
    r.resolve_item(item);
    r.stamp_item(d);
    r.since(d, s)
}

/// Pass three: the contract clauses of `module`, against `decls`.
pub fn resolve_contracts_in(decls: &[NameDecl], module: &ast::Module) -> Partial {
    let mut r = Resolver::with_env(decls);
    let (d, s) = (r.diagnostics.len(), r.symbols.len());
    r.resolve_contract_clauses(module);
    r.since(d, s)
}

// ── Public API ───────────────────────────────────────────────────────

/// Run name resolution on a parsed module.
/// Returns the resolver with its symbol table and diagnostics.
pub fn resolve(module: &ast::Module) -> Resolver {
    let mut resolver = Resolver::new();
    resolver.resolve_module(module);
    resolver
}

// ── Calls the evaluator cannot run (E0714) ───────────────────────────

/// Every name `--eval` can resolve in call position without a user definition:
/// `STD_FNS`, `VOCABULARY`, `SCALAR_MATH` and `LIST_MUTATORS`.
fn published_call_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = STD_FNS.to_vec();
    v.extend(VOCABULARY.iter().map(|(n, _, _)| *n));
    v.extend(SCALAR_MATH.iter().copied());
    v.extend(LIST_MUTATORS.iter().copied());
    v.sort_unstable();
    v.dedup();
    v
}

type NameSet = std::collections::HashSet<String>;

/// The names this module itself defines or binds, anywhere: a call of one of
/// them is never reported, whatever its receiver. Functions and methods
/// (`impl`, `extend`, `trait`, nested), struct, enum, variant, effect-operation
/// and item names, struct fields (a field may hold a callable), imports, and
/// every local binding (`let`, parameters, closure parameters, patterns).
fn defined_names(items: &[ast::Item], out: &mut NameSet) {
    use crate::borrowck::{Child, children, pattern_names};
    fn pat(p: &ast::Pattern, out: &mut NameSet) {
        let mut v = Vec::new();
        pattern_names(p, &mut v);
        out.extend(v);
    }
    fn expr(e: &ast::Expr, out: &mut NameSet) {
        match e {
            ast::Expr::Closure { params, .. } => out.extend(params.iter().map(|p| p.name.clone())),
            ast::Expr::For { pattern, .. } | ast::Expr::Is { pattern, .. } => pat(pattern, out),
            ast::Expr::Match { arms, .. } => arms.iter().for_each(|a| pat(&a.pattern, out)),
            ast::Expr::Handle { arms, .. } => {
                for a in arms {
                    out.insert(a.op.clone());
                    out.extend(a.params.iter().cloned());
                }
            }
            _ => {}
        }
        children(e, &mut |c| match c {
            Child::E(x) => expr(x, out),
            Child::B(b) => block(b, out),
        });
    }
    fn block(b: &ast::Block, out: &mut NameSet) {
        for s in &b.stmts {
            match s {
                ast::Stmt::Let { pattern, value, .. } => {
                    pat(pattern, out);
                    expr(value, out);
                }
                ast::Stmt::Expr { expr: e } | ast::Stmt::Defer { expr: e } => expr(e, out),
                ast::Stmt::Guard { cond, else_block } => {
                    expr(cond, out);
                    block(else_block, out);
                }
                ast::Stmt::Item { item } => defined_names(std::slice::from_ref(item), out),
            }
        }
        if let Some(t) = &b.tail_expr {
            expr(t, out);
        }
    }
    fn use_names(u: &ast::UseDef, out: &mut NameSet) {
        out.extend(u.path.iter().cloned());
        out.extend(u.alias.iter().cloned());
        u.group.iter().for_each(|g| use_names(g, out));
    }
    for item in items {
        match &item.kind {
            ast::ItemKind::Function(f) => {
                out.insert(f.name.clone());
                out.extend(f.params.iter().map(|p| p.name.clone()));
                block(&f.body, out);
                if let Some(e) = &f.body_expr {
                    expr(e, out);
                }
            }
            ast::ItemKind::Struct(s) => {
                out.insert(s.name.clone());
                out.extend(s.fields.iter().map(|f| f.name.clone()));
            }
            ast::ItemKind::Enum(e) => {
                out.insert(e.name.clone());
                out.extend(e.variants.iter().map(|v| v.name.clone()));
            }
            ast::ItemKind::Trait(t) => {
                out.insert(t.name.clone());
                defined_names(&t.items, out);
            }
            ast::ItemKind::Impl(b) => defined_names(&b.items, out),
            ast::ItemKind::Extend(b) => defined_names(&b.items, out),
            ast::ItemKind::Module(m) => {
                out.insert(m.name.clone());
                if let Some(items) = &m.items {
                    defined_names(items, out);
                }
            }
            ast::ItemKind::Use(u) => use_names(u, out),
            ast::ItemKind::Const(c) => {
                out.insert(c.name.clone());
            }
            ast::ItemKind::Static(s) => {
                out.insert(s.name.clone());
            }
            ast::ItemKind::Effect(e) => {
                out.insert(e.name.clone());
                out.extend(e.operations.iter().map(|o| o.name.clone()));
            }
            ast::ItemKind::Net(n) => {
                out.insert(n.name.clone());
            }
            ast::ItemKind::Data(d) => {
                out.insert(d.name.clone());
                match &d.kind {
                    ast::DataKind::Record(fields) => out.extend(fields.iter().map(|f| f.name.clone())),
                    ast::DataKind::Sum(variants) => out.extend(variants.iter().map(|v| v.name.clone())),
                }
            }
            _ => {}
        }
    }
}

/// One call the evaluator would not run.
struct UnknownCall {
    name: String,
    /// The capability namespace it was called on (`fs` in `fs.zork()`), if any.
    namespace: Option<String>,
    /// Where the enclosing statement (or tail expression) starts, relative to its
    /// top-level item. The AST keeps no position on an expression, so this is the
    /// nearest spanned node: the call's own line, the statement's column.
    pos: ast::Pos,
}

fn unknown_calls_in_expr(
    e: &ast::Expr,
    defined: &NameSet,
    published: &[&str],
    cur: ast::Pos,
    out: &mut Vec<UnknownCall>,
) {
    use crate::borrowck::{Child, children};
    let runs = |n: &str| defined.contains(n) || published.contains(&n);
    match e {
        ast::Expr::MethodCall { receiver, method, .. } => {
            let ns = match receiver.as_ref() {
                ast::Expr::Ident { name }
                    if !defined.contains(name)
                        && crate::hir::CAPABILITY_NAMESPACES.iter().any(|(n, _)| n == name) =>
                {
                    Some(name.clone())
                }
                _ => None,
            };
            match ns {
                Some(ns) => {
                    if !crate::eval::CAPABILITY_OPS.iter().any(|r| r.0 == ns && r.1 == method) {
                        out.push(UnknownCall { name: method.clone(), namespace: Some(ns), pos: cur });
                    }
                }
                None if !runs(method) => out.push(UnknownCall { name: method.clone(), namespace: None, pos: cur }),
                None => {}
            }
        }
        ast::Expr::Call { func, .. } => {
            if let ast::Expr::Ident { name } = func.as_ref()
                && !runs(name)
            {
                out.push(UnknownCall { name: name.clone(), namespace: None, pos: cur });
            }
        }
        _ => {}
    }
    children(e, &mut |c| match c {
        Child::E(x) => unknown_calls_in_expr(x, defined, published, cur, out),
        Child::B(b) => unknown_calls_in_block(b, defined, published, cur, out),
    });
}

fn unknown_calls_in_block(
    b: &ast::Block,
    defined: &NameSet,
    published: &[&str],
    outer: ast::Pos,
    out: &mut Vec<UnknownCall>,
) {
    let known = |p: Option<&ast::Pos>, fallback: ast::Pos| match p {
        Some(p) if p.is_set() => *p,
        _ => fallback,
    };
    let outer = known(Some(&b.pos), outer);
    for (i, s) in b.stmts.iter().enumerate() {
        let cur = known(b.stmt_pos.get(i), outer);
        match s {
            ast::Stmt::Let { value, .. } => unknown_calls_in_expr(value, defined, published, cur, out),
            ast::Stmt::Expr { expr: e } | ast::Stmt::Defer { expr: e } => {
                unknown_calls_in_expr(e, defined, published, cur, out)
            }
            ast::Stmt::Guard { cond, else_block } => {
                unknown_calls_in_expr(cond, defined, published, cur, out);
                unknown_calls_in_block(else_block, defined, published, cur, out);
            }
            ast::Stmt::Item { item } => {
                let mut nested = Vec::new();
                functions_under(std::slice::from_ref(item), &mut nested);
                for g in nested {
                    unknown_calls_in_block(&g.body, defined, published, cur, out);
                }
            }
        }
    }
    if let Some(t) = &b.tail_expr {
        unknown_calls_in_expr(t, defined, published, known(Some(&b.tail_pos), outer), out);
    }
}

/// Every function under `items`, in source order.
fn functions_under<'a>(items: &'a [ast::Item], out: &mut Vec<&'a ast::FunctionDef>) {
    for item in items {
        match &item.kind {
            ast::ItemKind::Function(f) => out.push(f),
            ast::ItemKind::Impl(b) => functions_under(&b.items, out),
            ast::ItemKind::Extend(b) => functions_under(&b.items, out),
            ast::ItemKind::Trait(t) => functions_under(&t.items, out),
            ast::ItemKind::Module(m) => functions_under(m.items.as_deref().unwrap_or(&[]), out),
            _ => {}
        }
    }
}

/// Warn about each call, `x.name(args)` or `name(args)`, that names nothing the
/// evaluator can run (E0714).
///
/// `--eval` runs a call by name: a user function or method, a closure in scope,
/// a published builtin (`STD_FNS`, `VOCABULARY`, `SCALAR_MATH`, `LIST_MUTATORS`),
/// a capability operation (`fs.read_to_string`), an effect operation, or a
/// variant constructor. The type checker gives any other method a fresh type
/// variable, so `xs.zork(1)` checks clean and then stops with `unknown
/// function`. This reports it before the run, as a warning: the program is
/// still accepted, and `--check`'s exit status does not change.
///
/// The rule is by name, never by receiver type, so it cannot be wrong about a
/// type it cannot see. Not reported: a name the module defines or binds
/// anywhere (even when the receiver's own type lacks it), a struct field name,
/// an import, a bound variable, and anything in a `net`, `train`, `kb` or
/// `swarm` body (those are not evaluator calls). A capability namespace's
/// operations are checked against `CAPABILITY_OPS` instead.
pub fn unknown_call_warnings(module: &ast::Module) -> Vec<Diagnostic> {
    unknown_call_warnings_owned(module).into_iter().map(|(_, d)| d).collect()
}

/// [`unknown_call_warnings`], each with the index of the top-level item it is in,
/// for a caller that runs on a module whose start lines are not its own.
pub fn unknown_call_warnings_owned(module: &ast::Module) -> Vec<(usize, Diagnostic)> {
    let mut defined = NameSet::new();
    defined_names(&module.items, &mut defined);
    let published = published_call_names();
    let unresolved: NameSet = resolve(module)
        .diagnostics
        .iter()
        .filter(|d| d.category == Some(DiagnosticCategory::UnresolvedName))
        .filter_map(|d| d.message.split('`').nth(1).map(str::to_string))
        .collect();

    let mut diags = Vec::new();
    for (i, item) in module.items.iter().enumerate() {
        let mut fns = Vec::new();
        functions_under(std::slice::from_ref(item), &mut fns);
        for f in fns {
            let mut sites = Vec::new();
            unknown_calls_in_block(&f.body, &defined, &published, f.body.pos, &mut sites);
            if let Some(e) = &f.body_expr {
                unknown_calls_in_expr(e, &defined, &published, f.body.pos, &mut sites);
            }
            for s in sites {
                // A bare call of a name nothing defines is already E0425, an error.
                if s.namespace.is_none() && unresolved.contains(&s.name) {
                    continue;
                }
                let (what, hint) = match &s.namespace {
                    Some(ns) => {
                        let ops: Vec<&str> =
                            crate::eval::CAPABILITY_OPS.iter().filter(|r| r.0 == ns).map(|r| r.1).collect();
                        let hint = match crate::builder::nearest(&s.name, &ops, 2) {
                            Some(c) => format!("did you mean `{ns}.{c}`?"),
                            None if ops.is_empty() => format!("`{ns}` has no operation the evaluator runs"),
                            None => format!("`{ns}` offers: {}", ops.join(", ")),
                        };
                        (format!("`{ns}.{}`", s.name), hint)
                    }
                    None => {
                        let hint = match crate::builder::nearest(&s.name, &published, if s.name.len() <= 4 { 1 } else { 2 }) {
                            Some(c) => format!("did you mean `{c}`?"),
                            None => format!("see `mage-parse --ontology-query {}`", s.name),
                        };
                        (format!("`{}`", s.name), hint)
                    }
                };
                let mut d = Diagnostic::categorized(
                    Severity::Warning,
                    format!(
                        "in `{}`: {what} is not a function the evaluator has, so `--eval` stops with \
                         `unknown function` when this runs ({hint})",
                        f.name
                    ),
                    DiagnosticCategory::UnknownFunction,
                    None,
                );
                // The call's own statement; the item's first line when no position was recorded.
                d.span = s.pos.at(module.start_of(i)).or_else(|| crate::hir::Span::item(module.start_of(i)));
                diags.push((i, d));
            }
        }
    }
    diags
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn resolve_source(src: &str) -> Resolver {
        let tokens = lexer::lex(src);
        let module = parser::parse(&tokens).expect("parse failed");
        resolve(&module)
    }

    #[test]
    fn test_simple_function_resolves() {
        let r = resolve_source("f add(a: i32, b: i32) -> i32 { a + b }");
        assert!(r.diagnostics.is_empty(), "unexpected errors: {:?}", r.diagnostics);
        // Should have symbols: builtins + add + a + b
        assert!(r.symbols.len() > 3);
    }

    #[test]
    fn test_unresolved_name() {
        let r = resolve_source("f foo() -> i32 { unknown_var }");
        assert!(!r.diagnostics.is_empty());
        assert!(r.diagnostics.iter().any(|d| d.message.contains("unresolved name: `unknown_var`")));
    }

    #[test]
    fn an_unbound_field_default_is_unresolved() {
        let r = resolve_source("data P(x: i32, y: i32 = k)
f main() -> i32 { val k = 99
 1 }");
        assert!(r.diagnostics.iter().any(|d| d.message.contains("unresolved name: `k`")), "{:?}", r.diagnostics);
    }

    #[test]
    fn a_const_field_default_resolves_clean() {
        let r = resolve_source("const K: i32 = 7;
f g() -> i32 { 1 }
data P(x: i32, y: i32 = K, z: i32 = g())");
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    }

    #[test]
    fn the_eleven_math_functions_resolve_as_free_calls() {
        // `MAGE_SPEC.md` promises `f(x)` for these. Each was refused as
        // `unresolved name` until they were registered beside `sqrt`.
        for name in [
            "floor", "ceil", "round", "trunc", "log2", "log10", "asin", "acos", "atan", "sinh",
            "cosh", "relu",
        ] {
            let r = resolve_source(&format!("f g(x: f64) -> f64 {{ {name}(x) }}"));
            assert!(
                r.diagnostics.is_empty(),
                "`{name}(x)` should resolve: {:?}",
                r.diagnostics
            );
        }
        let r = resolve_source("f g(x: f64) -> f64 { floor(x) }");
        assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    }

    #[test]
    fn test_struct_and_field_types() {
        let r = resolve_source("S Point { x: f64, y: f64, }");
        assert!(r.diagnostics.is_empty(), "unexpected errors: {:?}", r.diagnostics);
    }

    #[test]
    fn builtin_fns_and_types_resolve() {
        // Agentic-fix regression: println/vec/Some/etc. and std container
        // types must resolve so general-purpose programs check clean.
        let r = resolve_source(
            "+f main() / io { println(\"hi\"); val x = Some(1); }",
        );
        assert!(
            !r.diagnostics.iter().any(|d| d.message.contains("unresolved name: `println`")),
            "println should resolve as a builtin: {:?}",
            r.diagnostics
        );
        assert!(
            !r.diagnostics.iter().any(|d| d.message.contains("unresolved name: `Some`")),
            "Some should resolve: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn test_unresolved_type() {
        let r = resolve_source("f foo(x: UnknownType) -> i32 { 0 }");
        assert!(r.diagnostics.iter().any(|d| d.message.contains("unresolved type")));
    }

    #[test]
    fn self_resolves_inside_impl_trait_and_extend_bodies() {
        // The parser desugars every `self` receiver to a parameter of type
        // `Self`, so leaving `Self` unbound made *any* method unwritable —
        // receiver methods and `-> Self` constructors alike, in impls, traits,
        // and `extend` blocks. Cover each of those shapes: they failed for one
        // reason and would regress for one reason. `extend` is listed because
        // binding it only in impls and traits left that third body still broken.
        for src in [
            "S P { x: i32 }\nI P { +f get(&self) -> i32 { self.x } }",
            "S P { x: i32 }\nI P { +f into(self) -> i32 { self.x } }",
            "S P { x: i32 }\nI P { +f new(x: i32) -> Self { @P { x: x } } }",
            "T Shape { f area(&self) -> i32; }",
            "S P { x: i32 }\nextend P { f get(&self) -> i32 { self.x } }",
        ] {
            let r = resolve_source(src);
            assert!(
                !r.diagnostics.iter().any(|d| d.message.contains("unresolved type: `Self`")),
                "`Self` should be in scope for:\n{src}\ngot: {:?}",
                r.diagnostics
            );
        }
    }

    #[test]
    fn self_does_not_leak_outside_impl_and_trait_bodies() {
        // `Self` is bound per body, not globally — a free function naming it is
        // still an error. Without this the fix above could be "define Self
        // everywhere", which would accept nonsense.
        let r = resolve_source("S P { x: i32 }\nf loose(p: Self) -> i32 { 0 }");
        assert!(
            r.diagnostics.iter().any(|d| d.message.contains("unresolved type: `Self`")),
            "`Self` must not resolve outside an impl/trait: {:?}",
            r.diagnostics
        );
    }

    #[test]
    fn test_let_binding_scope() {
        let src = r#"
            f foo() -> i32 {
                v x: i32 = 10;
                x
            }
        "#;
        let r = resolve_source(src);
        assert!(r.diagnostics.is_empty(), "unexpected errors: {:?}", r.diagnostics);
    }

    #[test]
    fn test_nested_scopes() {
        let src = r#"
            f foo() -> i32 {
                v x: i32 = 1;
                v y: i32 = {
                    v z: i32 = x;
                    z
                };
                y
            }
        "#;
        let r = resolve_source(src);
        assert!(r.diagnostics.is_empty(), "unexpected errors: {:?}", r.diagnostics);
    }

    #[test]
    fn test_enum_variant_resolution() {
        let src = r#"
            E Color { Red, Green, Blue, }
            f pick() -> Color { Red }
        "#;
        let r = resolve_source(src);
        assert!(r.diagnostics.is_empty(), "unexpected errors: {:?}", r.diagnostics);
    }

    #[test]
    fn test_forward_reference() {
        let src = r#"
            f caller() -> i32 { callee() }
            f callee() -> i32 { 42 }
        "#;
        let r = resolve_source(src);
        assert!(r.diagnostics.is_empty(), "unexpected errors: {:?}", r.diagnostics);
    }

    /// `use` reports that it does nothing.
    ///
    /// It used to be accepted in silence, which made the failure surface at
    /// the call site instead: `u std.io.read_to_string` then
    /// `read_to_string("x")` gave `unresolved name`, pointing at the one line
    /// the author wrote correctly. And `u totally.made.up.path` was accepted,
    /// so an import naming nothing was indistinguishable from one naming
    /// something.
    #[test]
    fn use_is_an_error_because_it_can_never_mean_anything() {
        for src in ["u std.io", "u totally.made.up.path", "u std.col.{Map, Set}"] {
            let r = resolve_source(&format!("{src}
f main() -> i32 {{ 0 }}"));
            let named = r
                .diagnostics
                .iter()
                .any(|d| d.message.contains("cannot bring anything into scope"));
            assert!(named, "`{src}` should be rejected, got {:?}", r.diagnostics);
            // An **error** since 2026-08-19. This asserted the opposite until
            // then — "a warning, not an error, because rejecting the syntax
            // outright would break the corpus for no gain" — which was true
            // while item 1 was open and `stdlib/` still described an
            // import-based library. Item 1 resolved as *no module system*
            // (spec §2.3), `stdlib/` is gone, and the corpus cost was one line
            // in one example. A construct that can never mean anything should
            // not typecheck.
            assert!(
                r.diagnostics.iter().any(|d| matches!(d.severity, Severity::Error)),
                "`{src}` must be an error: {:?}",
                r.diagnostics
            );
            // The diagnostic has to name the section that decided it, or the
            // reader has no way to tell a removed feature from a broken one.
            assert!(
                r.diagnostics.iter().any(|d| d.message.contains("§2.3")),
                "the diagnostic must cite MAGE_SPEC.md §2.3: {:?}",
                r.diagnostics
            );
        }
    }

    /// A type-namespace name does not block a function of the same name.
    ///
    /// `define_type` mirrors its name into the value namespace so enum
    /// constructors resolve, and duplicate detection could not tell that copy
    /// from a definition. So every `S`, `T`, `Y`, `effect` and `sp`
    /// declaration reserved its name against functions: `S Point { … }` beside
    /// `f Point(…) -> Point` — the ordinary constructor pattern — reported
    /// `duplicate definition: Point`.
    ///
    /// Worst for `sp`, where a spec block *names the function it constrains*.
    /// That is the entire mechanism by which a spec attaches to one, so the
    /// contract feature could not be used as designed at all.
    #[test]
    fn a_type_name_does_not_block_a_function_of_the_same_name() {
        let dup = |src: &str| {
            resolve_source(src)
                .diagnostics
                .iter()
                .any(|d| d.message.contains("duplicate"))
        };
        assert!(!dup("sp search { @req(1b) }
f search(x: i32) -> i32 { x }"));
        assert!(!dup("S Point { x: i32 }
f Point(x: i32) -> i32 { x }"));
        assert!(!dup("effect Audit { f record(e: str) -> i32; }
f Audit(x: i32) -> i32 { x }"));
        assert!(!dup("T Shape { f area(self) -> i32; }
f Shape(x: i32) -> i32 { x }"));
    }

    /// The mirror is forgiven exactly once. Two real definitions in either
    /// namespace are still duplicates — a rule that quietly disabled duplicate
    /// detection for every type name would be worse than the bug it fixes.
    #[test]
    fn the_type_mirror_does_not_disable_duplicate_detection() {
        let dup = |src: &str| {
            resolve_source(src)
                .diagnostics
                .iter()
                .any(|d| d.message.contains("duplicate"))
        };
        assert!(dup("f g() -> i32 { 1 }
f g() -> i32 { 2 }"), "plain duplicate missed");
        assert!(dup("S P { x: i32 }
S P { y: i32 }"), "duplicate type missed");
        assert!(dup("sp s { @fx() }
sp s { @fx() }"), "duplicate spec missed");
        assert!(
            dup("S P { x: i32 }
f P(x: i32) -> i32 { x }
f P(x: i32) -> i32 { x }"),
            "the second function after a struct must still collide"
        );
    }

    /// A source definition may shadow a prelude name.
    ///
    /// The prelude registers ~80 names — the capability namespaces, the
    /// vocabulary, the builtin functions — into the same root scope as the
    /// program's own items. Every one of those words was therefore reserved
    /// globally, so `M net { … }` reported `duplicate definition: net` against
    /// a definition the author never wrote and could not see. That makes the
    /// obvious module names for a standard library — `io`, `net`, `fs`,
    /// `agent` — unusable, which is the shape `stdlib/` wants.
    #[test]
    fn a_source_definition_shadows_a_prelude_name() {
        for name in ["io", "net", "fs", "agent", "swarm", "kb", "llm", "gpu", "time", "env"] {
            let r = resolve_source(&format!("M {name} {{ }}\nf main() -> i32 {{ 0 }}"));
            assert!(
                r.diagnostics.is_empty(),
                "`M {name}` should shadow the prelude name, got {:?}",
                r.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
            );
        }
        // Vocabulary and builtin functions shadow the same way.
        for src in ["f map() -> i32 { 1 }", "f println() -> i32 { 1 }"] {
            let r = resolve_source(src);
            assert!(r.diagnostics.is_empty(), "`{src}` should shadow: {:?}", r.diagnostics);
        }
    }

    /// Shadowing a builtin is allowed exactly once — a second source
    /// definition of the same name is still a duplicate. Without this the
    /// shadowing rule would silently disable duplicate detection for every
    /// prelude name, which is a worse bug than the one it fixes.
    #[test]
    fn shadowing_a_prelude_name_does_not_disable_duplicate_detection() {
        let dup = |src: &str| {
            resolve_source(src)
                .diagnostics
                .iter()
                .any(|d| d.message.contains("duplicate definition"))
        };
        assert!(dup("f g() -> i32 { 1 }\nf g() -> i32 { 2 }"), "plain duplicate missed");
        assert!(dup("M h { }\nM h { }"), "duplicate module missed");
        // Two definitions of a *prelude* name: the first shadows, the second
        // collides with the first.
        assert!(dup("M net { }\nM net { }"), "duplicate after shadowing missed");
        assert!(dup("f map() -> i32 { 1 }\nf map() -> i32 { 2 }"), "duplicate vocab missed");
    }

    // ── Discarded bounds ─────────────────────────────────────────────

    fn bound_warnings(src: &str) -> Vec<String> {
        resolve_source(src)
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("is parsed and then discarded"))
            .map(|d| d.message.clone())
            .collect()
    }

    /// HANDOFF item 21, exactly as it was reported: the bound names a trait
    /// that exists nowhere, and the program was accepted in silence.
    #[test]
    fn a_where_clause_bound_naming_no_trait_is_reported() {
        let warnings = bound_warnings(
            "f describe[T](v: T) -> s ~> T: TotallyMadeUpTrait { \"described\" }",
        );
        assert_eq!(warnings.len(), 1, "expected one warning: {warnings:?}");
        assert!(
            warnings[0].contains("`T: TotallyMadeUpTrait`"),
            "the warning must name the bound it is about: {}",
            warnings[0]
        );
    }

    /// And it stays a *warning*. Turning it into an error would reject
    /// `quick-start/03-syntax-tour.md` and `migration-guide/04-types.md`,
    /// which teach writing bounds and which `check-doc-blocks.sh` certifies.
    #[test]
    fn a_discarded_bound_is_never_an_error() {
        let r = resolve_source("f g[T](x: T) -> T ~> T: Clone { x }");
        assert!(
            !r.diagnostics.iter().any(|d| d.severity == Severity::Error),
            "a bound must not fail the program: {:?}",
            r.diagnostics
        );
    }

    /// The other direction, and the one that matters most: a program with no
    /// bounds must produce no bound warnings. A check that fires on
    /// everything reports nothing.
    #[test]
    fn a_program_without_bounds_is_left_alone() {
        for src in [
            "f id[T](x: T) -> T { x }",
            "f add(a: i32, b: i32) -> i32 { a + b }",
            "S Point { x: f64, y: f64, }",
        ] {
            assert!(
                bound_warnings(src).is_empty(),
                "unprompted bound warning on `{src}`"
            );
        }
    }

    /// Every surface form that can carry a bound reports one. This is the
    /// test that would have caught a fix applied to functions alone — the
    /// AST has nine generic-bearing items and `resolve_item` visits six.
    ///
    /// **All nine are reachable from source, including `Y` and `D`.** An
    /// earlier version of this comment said those two carried a `generics`
    /// field no syntax could reach, citing `Y Alias[T] = T` and
    /// `D Rec[T] { v: T, }` as parse errors. Both are — for reasons that have
    /// nothing to do with generics: a type alias needs its `;`, and a `data`
    /// record is written with parentheses, not braces. `parse_type_alias` and
    /// `parse_data_def` have both called `parse_generic_params` all along.
    /// A parse error is evidence about the input, not about the language.
    #[test]
    fn every_form_that_can_carry_a_bound_reports_it() {
        for src in [
            "f g[T: Clone](x: T) -> T { x }",           // inline, on a function
            "f h[T](x: T) -> T ~> T: Clone { x }",      // the `~>` clause
            "S Box[T: Clone] { v: T, }",                // struct
            "E Opt[T: Clone] { Some(T), None, }",       // enum
            "T Show[T: Clone] { }",                     // trait
            "T Sh { }\nI[T: Clone] Sh { }",             // impl block
            "sp Contract[T: Clone] { }",                // spec
            "net Model[T: Clone] { }",                  // net
            "Y Alias[T: Clone] = T;",                   // type alias
            "D Rec[T: Clone](v: T)",                    // data record
        ] {
            let warnings = bound_warnings(src);
            assert!(
                !warnings.is_empty(),
                "no bound warning for `{src}` — a discarded bound went unreported"
            );
        }
    }

    /// Each predicate is reported separately, so a function with two
    /// unenforced constraints does not look like one.
    #[test]
    fn each_bound_is_reported_separately() {
        let warnings =
            bound_warnings("f k[T, U](x: T, y: U) -> T ~> T: Clone, U: Debug { x }");
        assert_eq!(warnings.len(), 2, "expected one per predicate: {warnings:?}");
    }

    /// Every `VOCABULARY` word is stated as a `vocabulary` fact with its
    /// signature and doc, and every stated word is in the table, both ways.
    #[test]
    fn every_vocabulary_word_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let mut stated: Vec<Vec<String>> = kb.facts("vocabulary");
        stated.sort();
        let mut table: Vec<Vec<String>> = VOCABULARY
            .iter()
            .map(|(name, sig, doc)| vec![name.to_string(), sig.to_string(), doc.to_string()])
            .collect();
        table.sort();
        assert_eq!(stated, table, "vocabulary facts vs VOCABULARY");
    }

    /// Every `STD_FNS` name is stated as a `builtin_function` fact, and every
    /// stated name is in the table, both ways.
    #[test]
    fn every_builtin_function_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let mut stated: Vec<Vec<String>> = kb.facts("builtin_function");
        stated.sort();
        let mut table: Vec<Vec<String>> = STD_FNS.iter().map(|n| vec![n.to_string()]).collect();
        table.sort();
        assert_eq!(stated, table, "builtin_function facts vs STD_FNS");
    }

    /// Every `SCALAR_MATH` name is stated as a `scalar_math` fact, and every stated
    /// name is in the table, both ways. Each is also a name the resolver accepts.
    #[test]
    fn every_scalar_math_name_is_stated() {
        let kb = crate::kb::load().expect("the knowledge base loads");
        let mut stated: Vec<Vec<String>> = kb.facts("scalar_math");
        stated.sort();
        let mut table: Vec<Vec<String>> = SCALAR_MATH.iter().map(|n| vec![n.to_string()]).collect();
        table.sort();
        assert_eq!(stated, table, "scalar_math facts vs SCALAR_MATH");
        for name in SCALAR_MATH {
            assert!(STD_FNS.contains(name), "`{name}` is scalar math and not in STD_FNS");
        }
    }

    /// `STD_FN_DOCS` documents exactly the names of `STD_FNS`, each once, and
    /// none of them is a `VOCABULARY` word (those are documented there).
    #[test]
    fn every_std_fn_is_documented() {
        let mut documented: Vec<&str> = STD_FN_DOCS.iter().map(|(n, _, _)| *n).collect();
        documented.sort_unstable();
        let mut table: Vec<&str> = STD_FNS.to_vec();
        table.sort_unstable();
        assert_eq!(documented, table, "STD_FN_DOCS vs STD_FNS");
        for (n, sig, doc) in STD_FN_DOCS {
            assert!(!sig.is_empty() && !doc.is_empty(), "`{n}` has an empty row");
            assert!(!VOCABULARY.iter().any(|(v, _, _)| v == n), "`{n}` is in VOCABULARY too");
        }
    }
    // ── Calls the evaluator cannot run (E0714) ──────────────────────

    /// The messages of each E0714 warning `check_source` reports for `src`, and
    /// whether the program was refused.
    fn unknown_calls(src: &str) -> (Vec<String>, bool) {
        let c = crate::check::check_source(src, false, false);
        let warns = c
            .findings
            .iter()
            .filter(|f| f.code == "E0714")
            .inspect(|f| assert_eq!(f.severity, "warning", "E0714 is a warning"))
            .map(|f| f.message.clone())
            .collect();
        (warns, !c.verdict().accepted())
    }

    #[test]
    fn an_unknown_method_on_a_list_is_a_warning_that_names_it() {
        let (w, refused) = unknown_calls("f g(xs: [i64]) -> i64 { xs.zork(1) }\n");
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("`zork`") && w[0].contains("`g`") && w[0].contains("ontology-query zork"), "{}", w[0]);
        assert!(!refused, "a warning is not a refusal");
    }

    #[test]
    fn a_near_miss_names_the_published_word() {
        let (w, _) = unknown_calls("f g(xs: [i64]) -> [i64] { xs.sorted() }\n");
        assert!(w[0].contains("did you mean `sort`?"), "{}", w[0]);
        let (w, _) = unknown_calls("f g(xs: [i64]) -> i64 { xs.contians(1) }\n");
        assert!(w[0].contains("did you mean `contains`?"), "{}", w[0]);
        // A short name is not matched at two edits: `zork` is not `sort`.
        let (w, _) = unknown_calls("f g(xs: [i64]) -> i64 { xs.zork() }\n");
        assert!(!w[0].contains("did you mean"), "{}", w[0]);
    }

    #[test]
    fn calls_the_evaluator_runs_are_not_warned() {
        let clean = [
            // published builtins, as methods and as calls
            "f g(xs: [i64]) -> i64 { xs.len() + len(xs) + xs.sum() }\n",
            "f g(xs: [i64]) -> [i64] { xs.push(1); xs.sort() }\n",
            "f g(x: f64) -> f64 { x.sqrt() + exp(x) }\n",
            // a user method in an `impl`, an `extend`, and a trait (also through a bound)
            "S P { x: i32 }\nI P { +f dbl(&self) -> i32 { self.x * 2 } }\nf s() { v p = @P { x: 4 }; p.dbl() }\n",
            "S P { x: i32 }\nextend P { f dbl(&self) -> i32 { self.x * 2 } }\nf s() { v p = @P { x: 4 }; p.dbl() }\n",
            "T Shape { f area(&self) -> i32; }\nS Sq { side: i32 }\nI Shape for Sq { f area(&self) -> i32 { self.side } }\nf total<A: Shape>(a: A) -> i32 { a.area() }\n",
            // a user function, a closure, a closure parameter
            "f helper(n: i64) -> i64 { n }\nf g() -> i64 { helper(1) }\n",
            "f g() -> i64 { v inc = |x| x + 1; inc(2) }\n",
            "f g(cb: fn(i64) -> i64) -> i64 { cb(2) }\n",
            // a capability operation
            "f g(p: s) -> s / fs { fs.read_to_string(p) }\n",
            // a `data` variant, bare and qualified
            "data Shape = Circle(f64) | Rect(f64, f64)\nf g() -> Shape { Shape.Circle(1.0) }\nf h() -> Shape { Rect(1.0, 2.0) }\n",
            // a receiver of no known type, calling a method the module defines
            "extend [i64] { f zork(self: [i64], n: i64) -> i64 { n } }\nf g(xs: [i64]) -> i64 { xs.zork(1) }\n",
            "S P { x: i32 }\nI P { +f dbl(&self) -> i32 { self.x * 2 } }\nf g(a, b) { a.dbl() + b.dbl() }\n",
        ];
        for src in clean {
            let (w, _) = unknown_calls(src);
            assert!(w.is_empty(), "{src}\n{w:?}");
        }
    }

    #[test]
    fn an_unknown_capability_operation_is_named_with_its_namespace() {
        let (w, _) = unknown_calls("f g(p: s) -> s / fs { fs.read_to_strng(p) }\n");
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("`fs.read_to_strng`") && w[0].contains("did you mean `fs.read_to_string`?"), "{}", w[0]);
    }

    #[test]
    fn a_net_or_kb_body_is_not_an_evaluator_call() {
        let (w, _) = unknown_calls("net A { layer fc: Linear(3, 1); forward { fc } }\n");
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn a_bare_call_the_resolver_already_refuses_is_not_warned_twice() {
        let (w, refused) = unknown_calls("f g() -> i64 { zork2(1) }\n");
        assert!(w.is_empty() && refused, "{w:?}");
    }

}
