/// MechGen Name Resolution — builds a symbol table and resolves identifiers.
///
/// Walks the AST to:
/// 1. Collect all definitions (functions, structs, enums, traits, consts, effects, modules)
/// 2. Build nested scope chains
/// 3. Resolve every identifier reference to a SymbolId
/// 4. Report unresolved names and duplicate definitions
use crate::ast;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity, SymbolId, Ty};
use std::collections::HashMap;

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
}

// ── Scope chain ──────────────────────────────────────────────────────

/// A single lexical scope.
#[derive(Debug)]
struct Scope {
    /// name → SymbolId for items defined in this scope.
    names: HashMap<String, SymbolId>,
    /// name → SymbolId for type-namespace names (structs, enums, type aliases, traits).
    types: HashMap<String, SymbolId>,
}

impl Scope {
    fn new() -> Self {
        Scope { names: HashMap::new(), types: HashMap::new() }
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
            if scope.names.contains_key(name) {
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
            // Also make it available in value namespace (for enum constructors, etc.)
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
            ast::ItemKind::Data(dd) => {
                self.define_type(&dd.name, SymbolKind::Struct);
            }
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
            ast::ItemKind::Spec(_) => { /* spec bodies are declarative, skip for now */ }
            ast::ItemKind::Agent(_) => { /* agent bodies are declarative, skip for now */ }
            ast::ItemKind::Swarm(_) => { /* swarm bodies are declarative, skip for now */ }
            ast::ItemKind::Net(_) => { /* net bodies resolved later */ }
            ast::ItemKind::Kb(_) => { /* kb bodies resolved later */ }
            ast::ItemKind::Evolve(_) => { /* evolve bodies resolved later */ }
            ast::ItemKind::Train(_) => { /* train bodies resolved later */ }
            ast::ItemKind::Static(sd) => {
                self.resolve_ast_type(&sd.ty);
                self.resolve_expr(&sd.value);
            }
            ast::ItemKind::Data(_) => { /* data fields are simple, skip for now */ }
            ast::ItemKind::Extend(eb) => {
                self.push_scope();
                self.resolve_ast_type(&eb.target_type);
                for item in &eb.items {
                    self.resolve_item(item);
                }
                self.pop_scope();
            }
        }
    }

    fn resolve_function(&mut self, fd: &ast::FunctionDef) {
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
        self.push_scope();
        for gp in &td.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }
        for item in &td.items {
            self.resolve_item(item);
        }
        self.pop_scope();
    }

    fn resolve_impl(&mut self, ib: &ast::ImplBlock) {
        self.push_scope();
        for gp in &ib.generics {
            self.define_type(&gp.name, SymbolKind::GenericParam);
        }
        self.resolve_ast_type(&ib.self_type);
        for item in &ib.items {
            self.resolve_item(item);
        }
        self.pop_scope();
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

    fn resolve_use(&mut self, _ud: &ast::UseDef) {
        // Use declarations bring external names into scope.
        // For the prototype, we just note that they exist.
    }

    fn resolve_type_alias(&mut self, ta: &ast::TypeAlias) {
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
                if let Some(name) = path.first() {
                    if self.lookup_type(name).is_none() {
                        self.diagnostics.push(Diagnostic::categorized(
                            Severity::Error,
                            format!("unresolved type in pattern: `{}`", path.join(".")),
                            DiagnosticCategory::UnresolvedType,
                            None,
                        ));
                    }
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
                if let Some(name) = path.first() {
                    if self.lookup_value(name).is_none() && self.lookup_type(name).is_none() {
                        self.diagnostics.push(Diagnostic::categorized(
                            Severity::Error,
                            format!("unresolved variant in pattern: `{}`", path.join(".")),
                            DiagnosticCategory::UnresolvedName,
                            None,
                        ));
                    }
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
                if let Some(name) = path.first() {
                    if self.lookup_type(name).is_none() {
                        self.diagnostics.push(Diagnostic::categorized(
                            Severity::Error,
                            format!("unresolved struct: `{}`", path.join(".")),
                            DiagnosticCategory::UnresolvedType,
                            None,
                        ));
                    }
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
    fn test_unresolved_type() {
        let r = resolve_source("f foo(x: UnknownType) -> i32 { 0 }");
        assert!(r.diagnostics.iter().any(|d| d.message.contains("unresolved type")));
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
}
