/// MAGE Name Resolution — builds a symbol table and resolves identifiers.
///
/// Walks the AST to:
/// 1. Collect all definitions (functions, structs, enums, traits, consts, effects, modules)
/// 2. Build nested scope chains
/// 3. Resolve every identifier reference to a SymbolId
/// 4. Report unresolved names and duplicate definitions
use crate::ast;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity, SymbolId, Ty};
use std::collections::HashMap;

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
        }
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

        // First pass: collect all top-level names (forward declarations).
        for item in &module.items {
            self.collect_item_name(item);
        }

        // Second pass: resolve bodies.
        for item in &module.items {
            self.resolve_item(item);
        }

        self.pop_scope();
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
        let std_fns = [
            // I/O
            "println", "print", "eprintln", "eprint", "format", "write", "writeln",
            // construction / diagnostics
            "vec", "panic", "assert", "assert_eq", "assert_ne", "dbg", "todo", "unimplemented",
            "unreachable", "matches",
            // common free functions
            "min", "max", "abs", "drop", "swap", "replace", "default",
            // bare enum-value constructors agents call positionally
            "Some", "None", "Ok", "Err",
        ];
        for name in std_fns {
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
        match &item.kind {
            ast::ItemKind::Function(fd) => {
                self.define_value(&fd.name, SymbolKind::Function);
            }
            ast::ItemKind::Struct(sd) => {
                self.define_type(&sd.name, SymbolKind::Struct);
            }
            ast::ItemKind::Enum(ed) => {
                let parent = self.define_type(&ed.name, SymbolKind::Enum);
                for variant in &ed.variants {
                    self.define_value(&variant.name, SymbolKind::EnumVariant { parent });
                }
            }
            ast::ItemKind::Trait(td) => {
                self.define_type(&td.name, SymbolKind::Trait);
            }
            ast::ItemKind::Module(md) => {
                self.define_value(&md.name, SymbolKind::Module);
            }
            ast::ItemKind::TypeAlias(ta) => {
                self.define_type(&ta.name, SymbolKind::TypeAlias);
            }
            ast::ItemKind::Const(cd) => {
                self.define_value(&cd.name, SymbolKind::Const);
            }
            ast::ItemKind::Effect(ed) => {
                self.define_type(&ed.name, SymbolKind::Effect);
            }
            ast::ItemKind::Spec(sd) => {
                self.define_type(&sd.name, SymbolKind::Spec);
            }
            ast::ItemKind::Static(sd) => {
                self.define_value(&sd.name, SymbolKind::Const);
            }
            ast::ItemKind::Agent(ad) => {
                self.define_value(&ad.name, SymbolKind::Agent);
            }
            ast::ItemKind::Swarm(s) => {
                self.define_value(&s.name, SymbolKind::Swarm);
            }
            ast::ItemKind::Net(n) => {
                self.define_type(&n.name, SymbolKind::Net);
            }
            ast::ItemKind::Kb(k) => {
                self.define_type(&k.name, SymbolKind::Kb);
            }
            ast::ItemKind::Evolve(e) => {
                self.define_type(&e.name, SymbolKind::Evolve);
            }
            ast::ItemKind::Train(t) => {
                self.define_value(&t.name, SymbolKind::Train);
            }
            ast::ItemKind::Data(dd) => match &dd.kind {
                ast::DataKind::Record(_) => {
                    self.define_type(&dd.name, SymbolKind::Struct);
                }
                // `data Shape = Circle(f64) | Rect(f64, f64)` is a sum, and its
                // variants are values — exactly as `E Shape { … }` variants
                // are, three arms up. Only the type name was registered here,
                // so every variant was invisible: `Rect(3.0, 4.0)` gave
                // `unresolved name: Rect` and `?= s { Circle(r) => … }` gave
                // `unresolved variant in pattern`. The record half worked, so
                // `data` looked implemented.
                //
                // The ontology publishes `data` as "record or sum type". Half
                // of that was true.
                ast::DataKind::Sum(variants) => {
                    let parent = self.define_type(&dd.name, SymbolKind::Enum);
                    for variant in variants {
                        self.define_value(&variant.name, SymbolKind::EnumVariant { parent });
                    }
                }
            },
            ast::ItemKind::Extend(_) => {
                // Extend blocks don't introduce a new name
            }
            ast::ItemKind::Impl(_) | ast::ItemKind::Use(_) => {
                // Impl blocks and use decls don't introduce a single name
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
                /* data fields are simple, skip for now */
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
        for op in &ed.operations {
            for param in &op.params {
                self.resolve_ast_type(&param.ty);
            }
            if let Some(ret) = &op.return_type {
                self.resolve_ast_type(ret);
            }
        }
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
            ast::Type::Refined { base, .. } => {
                self.resolve_ast_type(base);
            }
        }
    }

    // ── Block & statement resolution ─────────────────────────────────

    fn resolve_block(&mut self, block: &ast::Block) {
        self.push_scope();
        for stmt in &block.stmts {
            self.resolve_stmt(stmt);
        }
        if let Some(tail) = &block.tail_expr {
            self.resolve_expr(tail);
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
                    self.diagnostics.push(Diagnostic::categorized(
                        Severity::Error,
                        format!("unresolved name: `{name}`"),
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
                for arg in args {
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
            ast::Expr::UnsafeBlock { block } => {
                self.resolve_block(block);
            }
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
                self.resolve_expr(start);
                self.resolve_expr(end);
            }
            ast::Expr::Pipeline { left, right } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            ast::Expr::Is { expr, .. } => {
                self.resolve_expr(expr);
            }
            ast::Expr::Error { .. } => {}
        }
    }
}

// ── Public API ───────────────────────────────────────────────────────

/// Run name resolution on a parsed module.
/// Returns the resolver with its symbol table and diagnostics.
pub fn resolve(module: &ast::Module) -> Resolver {
    let mut resolver = Resolver::new();
    resolver.resolve_module(module);
    resolver
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
    /// Two of the nine are absent on purpose: `Y` (type alias) and `D`
    /// (data) carry a `generics` field that **no surface syntax reaches** —
    /// `Y Alias[T] = T` and `D Rec[T] { v: T, }` are both parse errors. The
    /// resolver reports their bounds anyway, so the day the parser accepts
    /// them nothing is silently skipped.
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
}
