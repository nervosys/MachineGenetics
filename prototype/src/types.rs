/// MAGE Type Checker — bidirectional type checking with HM-style unification.
///
/// Implements the type judgment:  Γ; Σ; Δ ⊢ e : τ ⊣ ε
///
/// - Γ = type environment (name → Ty)
/// - Constraint generation: walk the AST, emit τ₁ ≡ τ₂ constraints
/// - Unification: Robinson's algorithm extended for MAGE types
/// - Substitution: apply solved constraints to resolve all type variables
use crate::ast;
use crate::hir;
use crate::hir::{
    Diagnostic, DiagnosticCategory, FloatTy, IntTy, Severity, Ty, TyVar, UintTy, pure,
};
use std::collections::HashMap;

// ── Type variable supply ─────────────────────────────────────────────

struct TyVarSupply {
    next: u32,
}

impl TyVarSupply {
    fn new() -> Self {
        TyVarSupply { next: 0 }
    }

    fn fresh(&mut self) -> Ty {
        let v = TyVar(self.next);
        self.next += 1;
        Ty::Var(v)
    }
}

// ── Substitution ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Subst {
    map: HashMap<TyVar, Ty>,
}

impl Subst {
    fn new() -> Self {
        Subst { map: HashMap::new() }
    }

    fn apply(&self, ty: &Ty) -> Ty {
        match ty {
            Ty::Var(v) => {
                if let Some(resolved) = self.map.get(v) {
                    // Recursively apply in case of chains: ?T0 → ?T1 → i32
                    self.apply(resolved)
                } else {
                    ty.clone()
                }
            }
            Ty::Ref(m, inner) => Ty::Ref(*m, Box::new(self.apply(inner))),
            Ty::OwnedPtr(inner) => Ty::OwnedPtr(Box::new(self.apply(inner))),
            Ty::Rc(inner) => Ty::Rc(Box::new(self.apply(inner))),
            Ty::Arc(inner) => Ty::Arc(Box::new(self.apply(inner))),
            Ty::Slice(inner) => Ty::Slice(Box::new(self.apply(inner))),
            Ty::Array(inner, n) => Ty::Array(Box::new(self.apply(inner)), *n),
            Ty::Vec(inner) => Ty::Vec(Box::new(self.apply(inner))),
            Ty::Option(inner) => Ty::Option(Box::new(self.apply(inner))),
            Ty::Ptr(inner) => Ty::Ptr(Box::new(self.apply(inner))),
            Ty::Result(ok, err) => Ty::Result(Box::new(self.apply(ok)), Box::new(self.apply(err))),
            Ty::Map(k, v) => Ty::Map(Box::new(self.apply(k)), Box::new(self.apply(v))),
            Ty::Simd(inner, w) => Ty::Simd(Box::new(self.apply(inner)), *w),
            Ty::Tuple(elems) => Ty::Tuple(elems.iter().map(|t| self.apply(t)).collect()),
            Ty::Fn(params, ret, fx) => Ty::Fn(
                params.iter().map(|t| self.apply(t)).collect(),
                Box::new(self.apply(ret)),
                fx.clone(),
            ),
            Ty::Named(sym, args) => Ty::Named(*sym, args.iter().map(|t| self.apply(t)).collect()),
            // Primitives are unchanged.
            _ => ty.clone(),
        }
    }

    fn bind(&mut self, var: TyVar, ty: Ty) {
        self.map.insert(var, ty);
    }
}

// ── Unification ──────────────────────────────────────────────────────

fn occurs_in(var: TyVar, ty: &Ty) -> bool {
    match ty {
        Ty::Var(v) => *v == var,
        Ty::Ref(_, t)
        | Ty::OwnedPtr(t)
        | Ty::Rc(t)
        | Ty::Arc(t)
        | Ty::Slice(t)
        | Ty::Vec(t)
        | Ty::Option(t)
        | Ty::Ptr(t) => occurs_in(var, t),
        Ty::Array(t, _) | Ty::Simd(t, _) => occurs_in(var, t),
        Ty::Result(a, b) | Ty::Map(a, b) => occurs_in(var, a) || occurs_in(var, b),
        Ty::Tuple(ts) => ts.iter().any(|t| occurs_in(var, t)),
        Ty::Fn(params, ret, _) => params.iter().any(|t| occurs_in(var, t)) || occurs_in(var, ret),
        Ty::Named(_, args) => args.iter().any(|t| occurs_in(var, t)),
        _ => false,
    }
}

fn unify(subst: &mut Subst, a: &Ty, b: &Ty) -> Result<(), String> {
    let a = subst.apply(a);
    let b = subst.apply(b);

    if a == b {
        return Ok(());
    }

    match (&a, &b) {
        // Var binding (Robinson's rule).
        (Ty::Var(v), _) => {
            if occurs_in(*v, &b) {
                return Err(format!("infinite type: {v} occurs in {b}"));
            }
            subst.bind(*v, b);
            Ok(())
        }
        (_, Ty::Var(v)) => {
            if occurs_in(*v, &a) {
                return Err(format!("infinite type: {v} occurs in {a}"));
            }
            subst.bind(*v, a);
            Ok(())
        }

        // Error type unifies with anything (error recovery).
        (Ty::Error, _) | (_, Ty::Error) => Ok(()),

        // Never type is a subtype of everything.
        (Ty::Never, _) | (_, Ty::Never) => Ok(()),

        // Structural rules.
        (Ty::Ref(m1, t1), Ty::Ref(m2, t2)) => {
            if m1 != m2 {
                return Err(format!(
                    "borrow mode mismatch: &{} vs &{}",
                    if *m1 { "!" } else { "" },
                    if *m2 { "!" } else { "" }
                ));
            }
            unify(subst, t1, t2)
        }
        (Ty::OwnedPtr(t1), Ty::OwnedPtr(t2)) => unify(subst, t1, t2),
        (Ty::Rc(t1), Ty::Rc(t2)) => unify(subst, t1, t2),
        (Ty::Arc(t1), Ty::Arc(t2)) => unify(subst, t1, t2),
        (Ty::Slice(t1), Ty::Slice(t2)) => unify(subst, t1, t2),
        (Ty::Vec(t1), Ty::Vec(t2)) => unify(subst, t1, t2),
        // Agentic coercion: a list literal annotated as a Vec. Agents
        // naturally write `let v: [T]~ = []` (empty) or `[a, b, c]`; those
        // literals type as fixed arrays `[T; n]`. Treat an array literal as a
        // Vec element-wise so the common collection idiom checks clean
        // instead of failing `[T]~ vs [T; n]`. (One-directional: a declared
        // fixed-size array still won't accept a Vec value.)
        (Ty::Vec(t1), Ty::Array(t2, _)) | (Ty::Array(t2, _), Ty::Vec(t1)) => {
            unify(subst, t1, t2)
        }
        (Ty::Option(t1), Ty::Option(t2)) => unify(subst, t1, t2),
        (Ty::Ptr(t1), Ty::Ptr(t2)) => unify(subst, t1, t2),
        (Ty::Array(t1, n1), Ty::Array(t2, n2)) => {
            if n1 != n2 {
                return Err(format!("array size mismatch: {n1} vs {n2}"));
            }
            unify(subst, t1, t2)
        }
        (Ty::Simd(t1, w1), Ty::Simd(t2, w2)) => {
            if w1 != w2 {
                return Err(format!("SIMD width mismatch: {w1} vs {w2}"));
            }
            unify(subst, t1, t2)
        }
        (Ty::Result(ok1, err1), Ty::Result(ok2, err2)) => {
            unify(subst, ok1, ok2)?;
            unify(subst, err1, err2)
        }
        (Ty::Map(k1, v1), Ty::Map(k2, v2)) => {
            unify(subst, k1, k2)?;
            unify(subst, v1, v2)
        }
        (Ty::Tuple(ts1), Ty::Tuple(ts2)) => {
            if ts1.len() != ts2.len() {
                return Err(format!("tuple length mismatch: {} vs {}", ts1.len(), ts2.len()));
            }
            for (t1, t2) in ts1.iter().zip(ts2.iter()) {
                unify(subst, t1, t2)?;
            }
            Ok(())
        }
        (Ty::Fn(p1, r1, _), Ty::Fn(p2, r2, _)) => {
            if p1.len() != p2.len() {
                return Err(format!("function arity mismatch: {} vs {}", p1.len(), p2.len()));
            }
            for (t1, t2) in p1.iter().zip(p2.iter()) {
                unify(subst, t1, t2)?;
            }
            unify(subst, r1, r2)
        }
        (Ty::Named(s1, args1), Ty::Named(s2, args2)) => {
            if s1 != s2 {
                return Err(format!("type mismatch: {s1} vs {s2}"));
            }
            if args1.len() != args2.len() {
                return Err(format!("type argument count mismatch for {s1}"));
            }
            for (a1, a2) in args1.iter().zip(args2.iter()) {
                unify(subst, a1, a2)?;
            }
            Ok(())
        }
        // AI types: Tensor shape unification
        (Ty::Tensor(t1, s1), Ty::Tensor(t2, s2)) => {
            unify(subst, t1, t2)?;
            unify_shapes(s1, s2)
        }
        (Ty::Param(t1, s1), Ty::Param(t2, s2)) => {
            unify(subst, t1, t2)?;
            unify_shapes(s1, s2)
        }
        (Ty::Genome(t1), Ty::Genome(t2)) => unify(subst, t1, t2),
        (Ty::Policy(s1, a1), Ty::Policy(s2, a2)) => {
            unify(subst, s1, s2)?;
            unify(subst, a1, a2)
        }
        (Ty::KnowledgeBase, Ty::KnowledgeBase) => Ok(()),
        (Ty::LlmType, Ty::LlmType) => Ok(()),
        _ => Err(format!("type mismatch: {a} vs {b}")),
    }
}

/// Unify two tensor shape dimension lists.
fn unify_shapes(
    s1: &[hir::TensorDimHir],
    s2: &[hir::TensorDimHir],
) -> Result<(), String> {
    if s1.len() != s2.len() {
        return Err(format!("tensor rank mismatch: {} vs {}", s1.len(), s2.len()));
    }
    for (d1, d2) in s1.iter().zip(s2.iter()) {
        match (d1, d2) {
            (hir::TensorDimHir::Lit(a), hir::TensorDimHir::Lit(b)) if a != b => {
                return Err(format!("tensor dimension mismatch: {a} vs {b}"));
            }
            // Var dims unify with anything (symbolic).
            _ => {}
        }
    }
    Ok(())
}

// ── Type environment ─────────────────────────────────────────────────

struct TypeEnv {
    /// Stack of scopes: name → Ty.
    scopes: Vec<HashMap<String, Ty>>,
}

impl TypeEnv {
    fn new() -> Self {
        TypeEnv { scopes: vec![HashMap::new()] }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
    }

    fn insert(&mut self, name: String, ty: Ty) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn lookup(&self, name: &str) -> Option<&Ty> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }
}

// ── Type checker ─────────────────────────────────────────────────────

/// A struct's generic parameter names, and its fields as `(name, type)`.
pub type StructDefEntry = (Vec<String>, Vec<(String, Ty)>);

/// A function's parameter types, return type, and declared effects.
pub type FnSigEntry = (Vec<Ty>, Ty, Vec<String>);

pub struct TypeChecker {
    supply: TyVarSupply,
    subst: Subst,
    env: TypeEnv,
    /// Struct definitions: name → its generic params and fields.
    struct_defs: HashMap<String, StructDefEntry>,
    /// Function signatures: name → params, return type, declared effects.
    fn_sigs: HashMap<String, FnSigEntry>,
    /// Enum definitions: enum name → its variant names. Used for match
    /// exhaustiveness checking.
    enum_defs: HashMap<String, Vec<String>>,
    /// Operations of each declared `effect` block: `(effect, op)` → parameter
    /// types and return type.
    ///
    /// Before this the operations in an `effect` block were parsed, stored, and
    /// read by nothing — an `effect` declaration was decoration. They are the
    /// signature an operation call is checked against and the types a handler
    /// arm's parameters are bound at, so they exist in exactly one place.
    effect_ops: HashMap<(String, String), (Vec<Ty>, Ty)>,
    /// Names of declared `effect` blocks, in declaration order.
    effect_defs: Vec<String>,
    /// Interned names of user-defined types: name → id, and id → name.
    ///
    /// Gives `Ty::Named` real identity. Two vectors rather than one bimap
    /// because the id is just the index.
    named_ids: HashMap<String, u32>,
    named_names: Vec<String>,
    /// Declared return type of each enclosing function, innermost last.
    ///
    /// A stack rather than a field because function bodies nest. Exists so
    /// `Expr::Return` can check its value: before this, `ret` inferred its
    /// operand and threw the type away, so `f() -> i32 { ret "s" }` was accepted.
    /// That hole was masked while `ret x;` failed for an unrelated reason
    /// (block typed `()`), and fixing that reason exposed it.
    ret_stack: Vec<Ty>,
    pub diagnostics: Vec<Diagnostic>,
    /// Type-var ids minted for unsuffixed integer literals. They unify with
    /// any concrete int width from context; whatever stays unbound at end of a
    /// function defaults to i32 (Rust-style integer literal polymorphism). This
    /// is what lets `let x: i64 = 3` and `[i64]~ = [1,2,3]` check clean.
    int_lit_vars: Vec<u32>,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            supply: TyVarSupply::new(),
            subst: Subst::new(),
            env: TypeEnv::new(),
            struct_defs: HashMap::new(),
            fn_sigs: HashMap::new(),
            enum_defs: HashMap::new(),
            effect_ops: HashMap::new(),
            effect_defs: Vec::new(),
            named_ids: HashMap::new(),
            named_names: Vec::new(),
            ret_stack: Vec::new(),
            diagnostics: Vec::new(),
            int_lit_vars: Vec::new(),
        }
    }

    fn fresh(&mut self) -> Ty {
        self.supply.fresh()
    }

    fn emit_error(&mut self, msg: impl Into<String>) {
        self.diagnostics.push(Diagnostic::categorized(
            Severity::Error,
            msg,
            DiagnosticCategory::TypeMismatch,
            None,
        ));
    }

    // ── AST type → HIR type conversion ───────────────────────────────

    fn lower_type(&mut self, ty: &ast::Type) -> Ty {
        match ty {
            ast::Type::Path { segments, type_args } => {
                let name = segments.join(".");
                let args: Vec<Ty> = type_args.iter().map(|t| self.lower_type(t)).collect();
                self.resolve_named_type(&name, args)
            }
            ast::Type::Reference { mutable, inner } => {
                Ty::Ref(*mutable, Box::new(self.lower_type(inner)))
            }
            ast::Type::OwnedPtr { inner } => Ty::OwnedPtr(Box::new(self.lower_type(inner))),
            ast::Type::Rc { inner } => Ty::Rc(Box::new(self.lower_type(inner))),
            ast::Type::Arc { inner } => Ty::Arc(Box::new(self.lower_type(inner))),
            ast::Type::Slice { inner } => Ty::Slice(Box::new(self.lower_type(inner))),
            ast::Type::Array { inner, .. } => {
                // For prototype: array size as constant (simplified).
                Ty::Array(Box::new(self.lower_type(inner)), 0)
            }
            ast::Type::Vec { inner } => Ty::Vec(Box::new(self.lower_type(inner))),
            ast::Type::Tuple { elements } => {
                Ty::Tuple(elements.iter().map(|t| self.lower_type(t)).collect())
            }
            ast::Type::Option { inner } => Ty::Option(Box::new(self.lower_type(inner))),
            ast::Type::Result { ok, err } => {
                Ty::Result(Box::new(self.lower_type(ok)), Box::new(self.lower_type(err)))
            }
            ast::Type::Map { key, value } => {
                Ty::Map(Box::new(self.lower_type(key)), Box::new(self.lower_type(value)))
            }
            ast::Type::Ptr { inner } => Ty::Ptr(Box::new(self.lower_type(inner))),
            ast::Type::Simd { inner, width } => Ty::Simd(Box::new(self.lower_type(inner)), *width),
            ast::Type::Fn { params, ret } => {
                let ps: Vec<Ty> = params.iter().map(|t| self.lower_type(t)).collect();
                let r = ret.as_ref().map(|t| self.lower_type(t)).unwrap_or(Ty::Unit);
                Ty::Fn(ps, Box::new(r), pure())
            }
            ast::Type::Never => Ty::Never,
            ast::Type::Inferred => self.fresh(),
            ast::Type::SelfType => {
                // In a real compiler, resolve to the impl's Self type.
                self.fresh()
            }
            ast::Type::StringType => Ty::Str,
            ast::Type::Cow { inner } => {
                let inner_ty = self.lower_type(inner);
                Ty::Named(crate::hir::SymbolId(u32::MAX), vec![inner_ty])
            }
            ast::Type::Cell { inner } => {
                let inner_ty = self.lower_type(inner);
                Ty::Named(crate::hir::SymbolId(u32::MAX), vec![inner_ty])
            }
            ast::Type::RefCell { inner } => {
                let inner_ty = self.lower_type(inner);
                Ty::Named(crate::hir::SymbolId(u32::MAX), vec![inner_ty])
            }
            ast::Type::Mutex { inner } => {
                let inner_ty = self.lower_type(inner);
                Ty::Named(crate::hir::SymbolId(u32::MAX), vec![inner_ty])
            }
            ast::Type::RwLock { inner } => {
                let inner_ty = self.lower_type(inner);
                Ty::Named(crate::hir::SymbolId(u32::MAX), vec![inner_ty])
            }
            ast::Type::Set { inner } => {
                let inner_ty = self.lower_type(inner);
                Ty::Named(crate::hir::SymbolId(u32::MAX), vec![inner_ty])
            }
            ast::Type::Tensor { inner, shape } => {
                let inner_ty = self.lower_type(inner);
                let dims: Vec<crate::hir::TensorDimHir> = shape.iter().map(|d| match d {
                    ast::TensorDim::Lit(n) => crate::hir::TensorDimHir::Lit(*n),
                    ast::TensorDim::Var(v) => crate::hir::TensorDimHir::Var(v.clone()),
                }).collect();
                Ty::Tensor(Box::new(inner_ty), dims)
            }
            ast::Type::ParamTy { inner, shape } => {
                let inner_ty = self.lower_type(inner);
                let dims: Vec<crate::hir::TensorDimHir> = shape.iter().map(|d| match d {
                    ast::TensorDim::Lit(n) => crate::hir::TensorDimHir::Lit(*n),
                    ast::TensorDim::Var(v) => crate::hir::TensorDimHir::Var(v.clone()),
                }).collect();
                Ty::Param(Box::new(inner_ty), dims)
            }
            ast::Type::Genome { inner } => {
                Ty::Genome(Box::new(self.lower_type(inner)))
            }
            ast::Type::Policy { state, action } => {
                Ty::Policy(Box::new(self.lower_type(state)), Box::new(self.lower_type(action)))
            }
            ast::Type::KnowledgeBase => Ty::KnowledgeBase,
            ast::Type::LlmType => Ty::LlmType,
            ast::Type::Refined { base, .. } => {
                // Lower to the base type; predicate is checked separately by verify
                self.lower_type(base)
            }
        }
    }

    /// Map a named type path to a canonical Ty.
    fn resolve_named_type(&mut self, name: &str, args: Vec<Ty>) -> Ty {
        match name {
            "i8" => Ty::Int(IntTy::I8),
            "i16" => Ty::Int(IntTy::I16),
            "i32" => Ty::Int(IntTy::I32),
            "i64" => Ty::Int(IntTy::I64),
            "i128" => Ty::Int(IntTy::I128),
            "isize" => Ty::Int(IntTy::Isize),
            "u8" => Ty::Uint(UintTy::U8),
            "u16" => Ty::Uint(UintTy::U16),
            "u32" => Ty::Uint(UintTy::U32),
            "u64" => Ty::Uint(UintTy::U64),
            "u128" => Ty::Uint(UintTy::U128),
            "usize" => Ty::Uint(UintTy::Usize),
            "f32" => Ty::Float(FloatTy::F32),
            "f64" => Ty::Float(FloatTy::F64),
            "bool" => Ty::Bool,
            "char" => Ty::Char,
            "str" => Ty::Str,
            "String" => Ty::Str,
            _ => {
                // A user-defined struct/enum/type alias. Interned to a stable
                // id so distinct names are distinct types.
                //
                // Every such type used to lower to `SymbolId(u32::MAX)` — the
                // *same* sentinel for all of them — and `unify` compares those
                // ids, so every user-defined type was interchangeable with every
                // other: `S A{} S B{}` let an `A` satisfy `B` and be passed to a
                // function expecting one. It also left `lookup_struct_field`
                // unable to tell which struct it was looking at, which is why it
                // searched all of them and invented a fresh type for a field
                // that existed nowhere.
                let id = self.intern_named(name);
                Ty::Named(crate::hir::SymbolId(id), args)
            }
        }
    }

    // ── Module-level checking ────────────────────────────────────────

    pub fn check_module(&mut self, module: &ast::Module) {
        // First pass: collect function signatures and struct definitions.
        for item in &module.items {
            self.collect_item_sig(item);
        }

        // Second pass: type-check function bodies.
        for item in &module.items {
            self.check_item(item);
        }
    }

    fn collect_item_sig(&mut self, item: &ast::Item) {
        match &item.kind {
            ast::ItemKind::Effect(ed) => {
                self.effect_defs.push(ed.name.clone());
                for op in &ed.operations {
                    let params: Vec<Ty> =
                        op.params.iter().map(|p| self.lower_type(&p.ty)).collect();
                    // An operation with no `->` returns unit, like a function.
                    let ret = match &op.return_type {
                        Some(t) => self.lower_type(t),
                        None => Ty::Unit,
                    };
                    self.effect_ops
                        .insert((ed.name.clone(), op.name.clone()), (params, ret));
                }
            }
            ast::ItemKind::Function(fd) => {
                let params: Vec<Ty> = fd.params.iter().map(|p| self.lower_type(&p.ty)).collect();
                // No return annotation → a fresh inference var, resolved from the
                // body in pass 2. Sharing it here means recursive calls and
                // external callers unify against the same (eventually inferred)
                // type, so return-type inference is sound even under recursion.
                let ret = match &fd.return_type {
                    Some(t) => self.lower_type(t),
                    None => self.fresh(),
                };
                self.fn_sigs.insert(fd.name.clone(), (params, ret, fd.effects.clone()));
            }
            ast::ItemKind::Struct(sd) => {
                let generics: Vec<String> = sd.generics.iter().map(|g| g.name.clone()).collect();
                let fields: Vec<(String, Ty)> =
                    sd.fields.iter().map(|f| (f.name.clone(), self.lower_type(&f.ty))).collect();
                self.struct_defs.insert(sd.name.clone(), (generics, fields));
            }
            ast::ItemKind::Enum(ed) => {
                let variants: Vec<String> = ed.variants.iter().map(|v| v.name.clone()).collect();
                self.enum_defs.insert(ed.name.clone(), variants);
            }
            // `data X = A | B` (sum type) is the idiomatic MAGE enum.
            ast::ItemKind::Data(dd) => {
                if let ast::DataKind::Sum(variants) = &dd.kind {
                    let names: Vec<String> = variants.iter().map(|v| v.name.clone()).collect();
                    self.enum_defs.insert(dd.name.clone(), names);
                }
            }
            _ => {}
        }
    }

    fn check_item(&mut self, item: &ast::Item) {
        match &item.kind {
            ast::ItemKind::Function(fd) => self.check_function(fd),
            ast::ItemKind::Const(cd) => {
                let declared = self.lower_type(&cd.ty);
                let inferred = self.infer_expr(&cd.value);
                if let Err(e) = unify(&mut self.subst, &declared, &inferred) {
                    self.emit_error(format!("const `{}`: {e}", cd.name));
                }
            }
            _ => {}
        }
    }

    fn check_function(&mut self, fd: &ast::FunctionDef) {
        self.env.push();

        // Bind generic params as fresh type vars.
        for gp in &fd.generics {
            let tv = self.fresh();
            self.env.insert(gp.name.clone(), tv);
        }

        // Bind parameters. An un-annotated param (`Type::Inferred`) reuses the
        // fresh var registered in the signature, so the body's use *and* callers
        // constrain the same type — sound inference. Annotated params (including
        // generics, which need the generic env bound above) keep the exact
        // existing path, so no existing program changes.
        for (i, param) in fd.params.iter().enumerate() {
            let ty = if matches!(param.ty, ast::Type::Inferred) {
                self.fn_sigs
                    .get(&fd.name)
                    .and_then(|s| s.0.get(i).cloned())
                    .unwrap_or_else(|| self.fresh())
            } else {
                self.lower_type(&param.ty)
            };
            self.env.insert(param.name.clone(), ty);
        }

        // Resolve the return type *before* the body, so `return` statements
        // inside it can be checked against it. When unannotated, reuse the fresh
        // var registered in the signature so the body *infers* the return type
        // (and recursive calls resolve to the same one).
        let ret_ty = match &fd.return_type {
            Some(t) => self.lower_type(t),
            None => self.fn_sigs.get(&fd.name).map(|s| s.1.clone()).unwrap_or(Ty::Unit),
        };

        // Infer body type.
        self.ret_stack.push(ret_ty.clone());
        let body_ty = if let Some(be) = &fd.body_expr {
            self.infer_expr(be)
        } else {
            self.infer_block(&fd.body)
        };
        self.ret_stack.pop();

        if let Err(e) = unify(&mut self.subst, &ret_ty, &body_ty) {
            self.emit_error(format!("function `{}`: return type mismatch: {e}", fd.name));
        }

        // Default any integer literals left unconstrained by context to i32.
        self.default_int_literals();

        self.env.pop();
    }

    /// Element type of a collection argument. Extracts from Vec/Slice/Array,
    /// constrains an unconstrained var to a Vec, and reports a precise error for
    /// a non-collection (e.g. `sum(5)`).
    fn collection_elem(&mut self, ty: &Ty) -> Ty {
        let t = self.subst.apply(ty);
        match &t {
            Ty::Array(e, _) | Ty::Slice(e) | Ty::Vec(e) => self.subst.apply(e),
            // An unconstrained integer literal (e.g. `sum(5)`) is NOT a collection.
            Ty::Var(tv) if self.int_lit_vars.contains(&tv.0) => {
                self.emit_error("expected a collection, found an integer".to_string());
                self.fresh()
            }
            // A genuinely unconstrained type var → constrain it to a Vec.
            Ty::Var(_) => {
                let e = self.fresh();
                let _ = unify(&mut self.subst, &t, &Ty::Vec(Box::new(e.clone())));
                e
            }
            _ => {
                self.emit_error(format!("expected a collection, found {t}"));
                self.fresh()
            }
        }
    }

    /// Argument to a *length* query: any collection, or a string.
    ///
    /// Separate from [`Self::collection_elem`] on purpose. `len(s)` and
    /// `count(s)` on a `str` are ordinary — and the string vocabulary
    /// (`upper`, `lower`, `split`, `chars`) already accepted one, so a `str` was
    /// a string for some builtins and not a collection for others, which is what
    /// broke `examples/hello-world` and its comment about "the standard
    /// vocabulary (len) over a string".
    ///
    /// Widening `collection_elem` itself was the first attempt and was wrong:
    /// it also made `sum("hi")` typecheck, which an existing test correctly
    /// forbids. Length-like and element-like uses want different rules, so they
    /// get different helpers.
    fn sized_arg(&mut self, ty: &Ty) {
        if matches!(self.subst.apply(ty), Ty::Str) {
            return;
        }
        self.collection_elem(ty);
    }

    fn vocab_arity(&mut self, name: &str, got: usize, want: usize) {
        if got != want {
            self.emit_error(format!("`{name}` expects {want} argument(s), found {got}"));
        }
    }

    /// Precise, fresh-per-call types for the §8 standard vocabulary, so misuse is
    /// caught (the reliability win) rather than inferred loosely. Returns `Some`
    /// for a typed combinator; `None` lets the call fall through to generic
    /// inference (min/max/abs, group/scan, or a user function that shadows it —
    /// user functions are handled before this is reached). No args are inferred
    /// for names it does not type, so there is no double inference.
    fn infer_vocab_call(&mut self, name: &str, args: &[ast::Expr]) -> Option<Ty> {
        const TYPED: &[&str] = &[
            "len", "count", "sum", "first", "last", "sort", "reverse", "take",
            "flatten", "contains", "zip", "freq", "keys", "values", "range", "map",
            "filter", "any", "all", "find", "fold", "reduce", "split", "join",
            "chars", "words", "lines", "upper", "lower",
        ];
        if !TYPED.contains(&name) {
            return None;
        }
        let usize_ty = Ty::Uint(crate::hir::UintTy::Usize);
        let a: Vec<Ty> = args.iter().map(|e| self.infer_expr(e)).collect();
        let n = a.len();
        let res = match name {
            "len" | "count" => {
                self.vocab_arity(name, n, 1);
                if n >= 1 {
                    self.sized_arg(&a[0]);
                }
                usize_ty
            }
            "sum" => {
                self.vocab_arity(name, n, 1);
                if n >= 1 {
                    self.collection_elem(&a[0])
                } else {
                    self.fresh()
                }
            }
            "first" | "last" => {
                self.vocab_arity(name, n, 1);
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                Ty::Option(Box::new(e))
            }
            "sort" | "reverse" | "take" => {
                self.vocab_arity(name, n, if name == "take" { 2 } else { 1 });
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                Ty::Vec(Box::new(e))
            }
            "flatten" => {
                self.vocab_arity(name, n, 1);
                let outer = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                let inner = self.collection_elem(&outer);
                Ty::Vec(Box::new(inner))
            }
            "contains" => {
                self.vocab_arity(name, n, 2);
                if n >= 1 {
                    let e = self.collection_elem(&a[0]);
                    if n >= 2 {
                        let _ = unify(&mut self.subst, &e, &a[1]);
                    }
                }
                Ty::Bool
            }
            "zip" => {
                self.vocab_arity(name, n, 2);
                let x = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                let y = if n >= 2 { self.collection_elem(&a[1]) } else { self.fresh() };
                Ty::Vec(Box::new(Ty::Tuple(vec![x, y])))
            }
            "freq" => {
                self.vocab_arity(name, n, 1);
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                Ty::Map(Box::new(e), Box::new(usize_ty))
            }
            "keys" | "values" => {
                self.vocab_arity(name, n, 1);
                let m = if n >= 1 { self.subst.apply(&a[0]) } else { self.fresh() };
                let (k, v) = match &m {
                    Ty::Map(k, v) => (self.subst.apply(k), self.subst.apply(v)),
                    _ => {
                        let k = self.fresh();
                        let v = self.fresh();
                        let _ = unify(
                            &mut self.subst,
                            &m,
                            &Ty::Map(Box::new(k.clone()), Box::new(v.clone())),
                        );
                        (k, v)
                    }
                };
                Ty::Vec(Box::new(if name == "keys" { k } else { v }))
            }
            "range" => {
                for t in &a {
                    let _ = unify(&mut self.subst, t, &usize_ty);
                }
                Ty::Vec(Box::new(usize_ty))
            }
            "map" => {
                self.vocab_arity(name, n, 2);
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                let b = self.fresh();
                if n >= 2 {
                    let f = Ty::Fn(vec![e], Box::new(b.clone()), pure());
                    let _ = unify(&mut self.subst, &a[1], &f);
                }
                Ty::Vec(Box::new(self.subst.apply(&b)))
            }
            "filter" => {
                self.vocab_arity(name, n, 2);
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                if n >= 2 {
                    let f = Ty::Fn(vec![e.clone()], Box::new(Ty::Bool), pure());
                    let _ = unify(&mut self.subst, &a[1], &f);
                }
                Ty::Vec(Box::new(e))
            }
            "any" | "all" => {
                self.vocab_arity(name, n, 2);
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                if n >= 2 {
                    let f = Ty::Fn(vec![e], Box::new(Ty::Bool), pure());
                    let _ = unify(&mut self.subst, &a[1], &f);
                }
                Ty::Bool
            }
            "find" => {
                self.vocab_arity(name, n, 2);
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                if n >= 2 {
                    let f = Ty::Fn(vec![e.clone()], Box::new(Ty::Bool), pure());
                    let _ = unify(&mut self.subst, &a[1], &f);
                }
                Ty::Option(Box::new(e))
            }
            "fold" => {
                self.vocab_arity(name, n, 3);
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                let acc = if n >= 2 { a[1].clone() } else { self.fresh() };
                if n >= 3 {
                    let f = Ty::Fn(vec![acc.clone(), e], Box::new(acc.clone()), pure());
                    let _ = unify(&mut self.subst, &a[2], &f);
                }
                self.subst.apply(&acc)
            }
            "reduce" => {
                self.vocab_arity(name, n, 2);
                let e = if n >= 1 { self.collection_elem(&a[0]) } else { self.fresh() };
                if n >= 2 {
                    let f = Ty::Fn(vec![e.clone(), e.clone()], Box::new(e.clone()), pure());
                    let _ = unify(&mut self.subst, &a[1], &f);
                }
                Ty::Option(Box::new(e))
            }
            // String / text vocabulary.
            "split" => {
                self.vocab_arity(name, n, 2);
                for t in a.iter().take(2) {
                    let _ = unify(&mut self.subst, t, &Ty::Str);
                }
                Ty::Vec(Box::new(Ty::Str))
            }
            "chars" | "words" | "lines" => {
                self.vocab_arity(name, n, 1);
                if n >= 1 {
                    let _ = unify(&mut self.subst, &a[0], &Ty::Str);
                }
                Ty::Vec(Box::new(Ty::Str))
            }
            "join" => {
                self.vocab_arity(name, n, 2);
                if n >= 1 {
                    let _ = unify(&mut self.subst, &a[0], &Ty::Vec(Box::new(Ty::Str)));
                }
                if n >= 2 {
                    let _ = unify(&mut self.subst, &a[1], &Ty::Str);
                }
                Ty::Str
            }
            "upper" | "lower" => {
                self.vocab_arity(name, n, 1);
                if n >= 1 {
                    let _ = unify(&mut self.subst, &a[0], &Ty::Str);
                }
                Ty::Str
            }
            _ => return None,
        };
        Some(self.subst.apply(&res))
    }

    /// Conservative match-exhaustiveness check over user-declared enums.
    ///
    /// Catches a common agent bug: a `match` on a sum type that forgets a
    /// variant and has no catch-all. Deliberately conservative — it only fires
    /// when (a) no arm is a catch-all (`_` or a bare binding), and (b) the
    /// covered variant names belong to exactly one known enum. Builtin sums
    /// (Option/Result) aren't in `enum_defs`, so they're never flagged → no
    /// false positives on the common cases.
    fn check_match_exhaustive(&mut self, arms: &[ast::MatchArm]) {
        fn collect(pat: &ast::Pattern, covered: &mut Vec<String>, catch_all: &mut bool) {
            match pat {
                ast::Pattern::Wildcard | ast::Pattern::Ident { .. } => *catch_all = true,
                ast::Pattern::Enum { path, .. } => {
                    if let Some(v) = path.last() {
                        covered.push(v.clone());
                    }
                }
                ast::Pattern::Or { patterns } => {
                    for p in patterns {
                        collect(p, covered, catch_all);
                    }
                }
                _ => {}
            }
        }

        let mut covered = Vec::new();
        let mut catch_all = false;
        for arm in arms {
            collect(&arm.pattern, &mut covered, &mut catch_all);
        }
        if catch_all || covered.is_empty() {
            return;
        }

        // Identify the unique enum whose variant set covers every matched name.
        let candidates: Vec<(&String, &Vec<String>)> = self
            .enum_defs
            .iter()
            .filter(|(_, variants)| covered.iter().all(|c| variants.contains(c)))
            .collect();
        if candidates.len() != 1 {
            return; // ambiguous or unknown (e.g. builtin sum) → stay silent
        }
        let (enum_name, variants) = candidates[0];
        let mut missing: Vec<String> =
            variants.iter().filter(|v| !covered.contains(v)).cloned().collect();
        if !missing.is_empty() {
            missing.sort();
            self.diagnostics.push(Diagnostic::categorized(
                Severity::Error,
                format!(
                    "non-exhaustive match on `{enum_name}`: missing variant(s) [{}] — add the arm(s) or a `_` catch-all",
                    missing.join(", ")
                ),
                DiagnosticCategory::TypeMismatch,
                None,
            ));
        }
    }

    /// Resolve unsuffixed integer-literal type vars that context never pinned
    /// to a concrete width, binding them to i32 (the MAGE integer default).
    /// Constrained ones are already bound by unification and are left alone.
    /// Settle every unsuffixed integer literal: default the unconstrained ones
    /// to `i32`, and reject any that context forced to a non-numeric type.
    ///
    /// The literal's type variable is minted completely free, so it unifies with
    /// *anything* — the comment at the mint site says "any int width the
    /// surroundings demand", but nothing enforced that, and
    /// `f() -> bool { 1 }` typechecked. Properly this wants an integer-kind
    /// constraint carried through `unify`; that is a larger change to the
    /// inference core. Checking after the fact catches the same programs
    /// without touching unification, at the cost of a diagnostic that names the
    /// function rather than the literal's span.
    ///
    /// Floats are permitted deliberately: `1` in a float context is an ordinary
    /// numeric literal here, and rejecting it would be a language change rather
    /// than a bug fix.
    fn default_int_literals(&mut self) {
        let pending = std::mem::take(&mut self.int_lit_vars);
        for v in pending {
            let tv = Ty::Var(crate::hir::TyVar(v));
            let resolved = self.subst.apply(&tv);
            match resolved {
                Ty::Var(_) => {
                    // Still unbound → default to i32.
                    let _ = unify(&mut self.subst, &tv, &Ty::Int(IntTy::I32));
                }
                Ty::Int(_) | Ty::Uint(_) | Ty::Float(_) | Ty::Never => {}
                other => self.emit_error(format!(
                    "integer literal used where `{}` is required",
                    other
                )),
            }
        }
    }

    // ── Block inference ──────────────────────────────────────────────

    fn infer_block(&mut self, block: &ast::Block) -> Ty {
        self.env.push();

        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }

        // A block with no tail expression is `()` — *unless* it ends by
        // diverging, in which case it produces no value at all and must not be
        // unified with the declared return type.
        //
        // Without this, `f() -> i32 { ret 9; }` failed with "return type
        // mismatch: I32 vs ()", because `ret 9;` is a statement, so the block
        // had no tail and was typed `()`. Dropping the semicolon worked, which
        // is a strange thing for a compiler to insist on — and it made
        // `--syntax=legacy` unusable end to end, since the Rust transpiler
        // faithfully renders `return a + b;` as `ret a + b;`, the failing form.
        //
        // `Expr::Return` already infers to `Ty::Never`; this only propagates
        // that through the statement position. Deliberately limited to `return`:
        // `Ty::Never` unifies with anything, so widening this can only turn
        // errors into silence, and `break`/`continue` in a function-body tail
        // have no case demanding it yet.
        let diverges = block.tail_expr.is_none()
            && matches!(
                block.stmts.last(),
                Some(ast::Stmt::Expr { expr }) if matches!(expr, ast::Expr::Return { .. })
            );

        let ty = if let Some(tail) = &block.tail_expr {
            self.infer_expr(tail)
        } else if diverges {
            Ty::Never
        } else {
            Ty::Unit
        };

        self.env.pop();
        ty
    }

    fn check_stmt(&mut self, stmt: &ast::Stmt) {
        match stmt {
            ast::Stmt::Let { pattern, ty, value, .. } => {
                let val_ty = self.infer_expr(value);

                if let Some(declared) = ty {
                    let declared_ty = self.lower_type(declared);
                    if let Err(e) = unify(&mut self.subst, &declared_ty, &val_ty) {
                        self.emit_error(format!("let binding: {e}"));
                    }
                    self.bind_pattern(pattern, &declared_ty);
                } else {
                    self.bind_pattern(pattern, &val_ty);
                }
            }
            ast::Stmt::Expr { expr } => {
                self.infer_expr(expr);
            }
            ast::Stmt::Item { item } => {
                self.collect_item_sig(item);
                self.check_item(item);
            }
            ast::Stmt::Guard { cond, else_block } => {
                let cond_ty = self.infer_expr(cond);
                if let Err(e) = unify(&mut self.subst, &cond_ty, &Ty::Bool) {
                    self.emit_error(format!("guard condition must be bool: {e}"));
                }
                // The else block has to *leave*. A guard whose else block
                // merely evaluates to a value falls through and the function
                // carries on with the precondition it just found to be false —
                // so `guard n > 0 else { 0 }` does not return `0`, it runs the
                // body anyway, and `a(-5)` answered `-10`. That checked clean
                // and produced a wrong number, which is the worst pair of
                // properties a construct can have.
                //
                // Rust's `let`-else requires divergence for exactly this
                // reason. Enforcing it here turns the silent wrong answer into
                // a diagnostic at the point of the mistake.
                let else_ty = self.infer_block(else_block);
                if !matches!(self.subst.apply(&else_ty), Ty::Never) {
                    self.emit_error(
                        "`guard` else block must leave the function: it needs a \
                         `return`, or the guard falls through and the body runs \
                         with the condition false",
                    );
                }
            }
            ast::Stmt::Defer { expr } => {
                self.infer_expr(expr);
            }
        }
    }

    fn bind_pattern(&mut self, pattern: &ast::Pattern, ty: &Ty) {
        match pattern {
            ast::Pattern::Ident { name } => {
                self.env.insert(name.clone(), ty.clone());
            }
            ast::Pattern::Wildcard => {}
            ast::Pattern::Tuple { elements } => {
                if let Ty::Tuple(tys) = ty {
                    for (pat, t) in elements.iter().zip(tys.iter()) {
                        self.bind_pattern(pat, t);
                    }
                }
            }
            _ => {
                // For more complex patterns, just bind identifiers found inside.
                self.bind_pattern_names(pattern, ty);
            }
        }
    }

    fn bind_pattern_names(&mut self, pattern: &ast::Pattern, ty: &Ty) {
        match pattern {
            ast::Pattern::Ident { name } => {
                self.env.insert(name.clone(), ty.clone());
            }
            ast::Pattern::Tuple { elements }
            | ast::Pattern::Slice { elements, .. }
            | ast::Pattern::Enum { elements, .. } => {
                for el in elements {
                    let fty = self.fresh();
                    self.bind_pattern_names(el, &fty);
                }
            }
            ast::Pattern::Struct { fields, .. } => {
                for fp in fields {
                    if let Some(pat) = &fp.pattern {
                        let fty = self.fresh();
                        self.bind_pattern_names(pat, &fty);
                    } else {
                        let fty = self.fresh();
                        self.env.insert(fp.name.clone(), fty);
                    }
                }
            }
            ast::Pattern::Or { patterns } => {
                if let Some(first) = patterns.first() {
                    self.bind_pattern_names(first, ty);
                }
            }
            ast::Pattern::Ref { pattern } => {
                self.bind_pattern_names(pattern, ty);
            }
            ast::Pattern::Wildcard | ast::Pattern::Literal { .. } => {}
        }
    }

    // ── Expression inference (synth mode) ────────────────────────────

    /// Join two types that have to agree because they are alternatives for the
    /// same value — the arms of a `match`, the branches of an `if`, the
    /// elements of a list literal. Ordinary unification, except that two arrays
    /// disagreeing *only* in length widen to a `Vec`.
    ///
    /// A fixed length belongs to an array literal, not to the value a branch
    /// produces. `? found { [finding] } : { [] }` has no statically known
    /// length, and `[T]~` is precisely that type; reporting `array size
    /// mismatch: 1 vs 0` made the most ordinary shape in the language — return
    /// a result or nothing — unwritable. The widening already existed for list
    /// literals, where it was needed for ragged nesting; it was never applied
    /// to branches, so three of the twelve examples could not be typechecked.
    ///
    /// A declared fixed-size array is unaffected: this only fires when two
    /// arrays reach each other as alternatives, and the result is then a `Vec`,
    /// which `unify` will not accept where a `[T; n]` was asked for.
    fn join_branches(&mut self, a: &Ty, b: &Ty) -> Result<Ty, String> {
        let Err(e) = unify(&mut self.subst, a, b) else {
            return Ok(self.subst.apply(a));
        };
        if let (Ty::Array(e1, n1), Ty::Array(e2, n2)) =
            (self.subst.apply(a), self.subst.apply(b))
            && n1 != n2
            && unify(&mut self.subst, &e1, &e2).is_ok()
        {
            return Ok(Ty::Vec(Box::new(self.subst.apply(&e1))));
        }
        Err(e)
    }

    fn infer_expr(&mut self, expr: &ast::Expr) -> Ty {
        match expr {
            ast::Expr::Literal { kind, value } => self.infer_literal(kind, value),

            ast::Expr::Ident { name } => {
                if let Some(ty) = self.env.lookup(name).cloned() {
                    self.subst.apply(&ty)
                } else if let Some((params, ret, _)) = self.fn_sigs.get(name).cloned() {
                    if params.is_empty() {
                        // Allow bare function name to resolve to its return type
                        // when used as an expression (forward reference).
                        ret
                    } else {
                        Ty::Fn(params, Box::new(ret), pure())
                    }
                } else {
                    // Unknown — produce fresh var (may be resolved later or error).
                    self.fresh()
                }
            }

            ast::Expr::Binary { op, left, right } => {
                let lt = self.infer_expr(left);
                let rt = self.infer_expr(right);

                match op.as_str() {
                    // Comparison operators always produce bool.
                    "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                        if let Err(e) = unify(&mut self.subst, &lt, &rt) {
                            self.emit_error(format!("comparison `{op}`: {e}"));
                        }
                        Ty::Bool
                    }
                    // Logical operators require bool operands.
                    "&&" | "||" => {
                        if let Err(e) = unify(&mut self.subst, &lt, &Ty::Bool) {
                            self.emit_error(format!("logical `{op}` lhs: {e}"));
                        }
                        if let Err(e) = unify(&mut self.subst, &rt, &Ty::Bool) {
                            self.emit_error(format!("logical `{op}` rhs: {e}"));
                        }
                        Ty::Bool
                    }
                    // Arithmetic operators: operands must be same numeric type.
                    "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<" | ">>" => {
                        if let Err(e) = unify(&mut self.subst, &lt, &rt) {
                            self.emit_error(format!("arithmetic `{op}`: {e}"));
                        }
                        self.subst.apply(&lt)
                    }
                    // Tensor operators: operands must be tensor types.
                    "\u{2297}" | "\u{2299}" => {
                        // ⊗ (matmul), ⊙ (hadamard) — both operands tensor, result tensor.
                        if let Err(e) = unify(&mut self.subst, &lt, &rt) {
                            self.emit_error(format!("tensor `{op}`: {e}"));
                        }
                        self.subst.apply(&lt)
                    }
                    "\u{25b8}" => {
                        // ▸ (pipeline) — chain operations, result of rhs.
                        self.subst.apply(&rt)
                    }
                    _ => {
                        self.emit_error(format!("unknown operator: `{op}`"));
                        Ty::Error
                    }
                }
            }

            ast::Expr::Unary { op, operand } => {
                let t = self.infer_expr(operand);
                match op.as_str() {
                    "-" => t,
                    "!" => {
                        // Could be bool negation or bitwise not.
                        t
                    }
                    "*" => {
                        // Dereference: &T → T, ^T → T, etc.
                        match &t {
                            Ty::Ref(_, inner) => *inner.clone(),
                            Ty::OwnedPtr(inner) => *inner.clone(),
                            Ty::Rc(inner) => *inner.clone(),
                            Ty::Arc(inner) => *inner.clone(),
                            _ => {
                                let inner = self.fresh();
                                if let Err(e) = unify(
                                    &mut self.subst,
                                    &t,
                                    &Ty::Ref(false, Box::new(inner.clone())),
                                ) {
                                    self.emit_error(format!("dereference: {e}"));
                                }
                                inner
                            }
                        }
                    }
                    "&" => Ty::Ref(false, Box::new(t)),
                    // Tensor postfix: ⊤ (transpose) keeps type, ⊥ (flatten) unwraps inner.
                    "\u{22a4}" => t, // transpose — same tensor type
                    "\u{22a5}" => {
                        // flatten — unwrap one nesting level
                        match &t {
                            Ty::Tensor(inner, _) => Ty::Tensor(inner.clone(), vec![]),
                            _ => t,
                        }
                    }
                    _ => {
                        self.emit_error(format!("unknown unary operator: `{op}`"));
                        Ty::Error
                    }
                }
            }

            ast::Expr::Call { func, args } => {
                // Built-in `grad` typing: grad(f) where f: Tensor → same Tensor type.
                if let ast::Expr::Ident { name } = func.as_ref()
                    && name == "grad" && args.len() == 1 {
                        let arg_ty = self.infer_expr(&args[0]);
                        let resolved = self.subst.apply(&arg_ty);
                        match &resolved {
                            Ty::Tensor(..) | Ty::Param(..) => return resolved,
                            _ => {
                                self.emit_error(format!(
                                    "grad requires tensor or param type, found {resolved}"
                                ));
                                return Ty::Error;
                            }
                        }
                    }

                // Direct call to a function known by name: use its signature
                // directly. This (a) fixes zero-arg calls — a bare function
                // Ident resolves to its *return type* (see Expr::Ident), so the
                // generic `Fn` unification below would see `() vs f()->?T` for a
                // unit-returning function; and (b) yields precise arity and
                // per-argument diagnostics instead of one opaque `call:` error.
                if let ast::Expr::Ident { name } = func.as_ref()
                    && let Some((params, ret, _)) = self.fn_sigs.get(name).cloned() {
                        let arg_tys: Vec<Ty> = args.iter().map(|a| self.infer_expr(a)).collect();
                        if params.len() != arg_tys.len() {
                            self.emit_error(format!(
                                "call `{name}`: expected {} argument(s), found {}",
                                params.len(),
                                arg_tys.len()
                            ));
                        } else {
                            for (i, (p, a)) in params.iter().zip(arg_tys.iter()).enumerate() {
                                if let Err(e) = unify(&mut self.subst, p, a) {
                                    self.emit_error(format!(
                                        "call `{name}`: argument {} type mismatch: {e}",
                                        i + 1
                                    ));
                                }
                            }
                        }
                        return self.subst.apply(&ret);
                    }

                // §8 standard vocabulary: precise, fresh-per-call types so misuse
                // is caught (the reliability win). User functions (handled above)
                // shadow these; unhandled names fall through to generic inference.
                if let ast::Expr::Ident { name } = func.as_ref()
                    && let Some(t) = self.infer_vocab_call(name, args) {
                        return t;
                    }

                let func_ty = self.infer_expr(func);
                let arg_tys: Vec<Ty> = args.iter().map(|a| self.infer_expr(a)).collect();

                let ret = self.fresh();
                let expected = Ty::Fn(arg_tys.clone(), Box::new(ret.clone()), pure());

                if let Err(e) = unify(&mut self.subst, &func_ty, &expected) {
                    self.emit_error(format!("call: {e}"));
                    return Ty::Error;
                }

                self.subst.apply(&ret)
            }

            ast::Expr::MethodCall { receiver, method, args, .. } => {
                // `Audit.record(x)` — an effect operation, not a method. The
                // receiver is the effect's name, so it is checked before the
                // receiver is inferred as a value; there is no value there to
                // infer, and treating it as one is what made an operation call
                // return a fresh variable and accept any arguments at all.
                if let ast::Expr::Ident { name } = receiver.as_ref()
                    && let Some((params, ret)) =
                        self.effect_ops.get(&(name.clone(), method.clone())).cloned()
                {
                    if params.len() != args.len() {
                        self.emit_error(format!(
                            "effect operation `{name}.{method}` takes {} argument(s), \
                             given {}",
                            params.len(),
                            args.len()
                        ));
                    }
                    for (arg, want) in args.iter().zip(params.iter()) {
                        let got = self.infer_expr(arg);
                        if let Err(e) = unify(&mut self.subst, &got, want) {
                            self.emit_error(format!(
                                "effect operation `{name}.{method}`: {e}"
                            ));
                        }
                    }
                    return ret;
                }
                // The receiver names a declared effect, but the operation is
                // not one of its declarations. That is a misspelling, and it
                // has to be an error here: the effect analysis attributes the
                // effect on the *receiver* alone, so `Audit.recrod(x)` was
                // accepted, counted as performing `audit`, and then died at run
                // time with `unknown function`. Exactly the shape of bug this
                // whole feature exists to stop being possible.
                if let ast::Expr::Ident { name } = receiver.as_ref()
                    && self.effect_defs.contains(name)
                {
                    let mut ops: Vec<&str> = self
                        .effect_ops
                        .keys()
                        .filter(|(e, _)| e == name)
                        .map(|(_, op)| op.as_str())
                        .collect();
                    ops.sort_unstable();
                    self.emit_error(format!(
                        "effect `{name}` declares no operation `{method}` (it declares: {})",
                        if ops.is_empty() { "none".to_string() } else { ops.join(", ") }
                    ));
                }
                self.infer_expr(receiver);
                for arg in args {
                    self.infer_expr(arg);
                }
                // Method resolution requires trait lookup — return fresh var.
                self.fresh()
            }

            ast::Expr::FieldAccess { object, field } => {
                let obj_ty = self.infer_expr(object);
                let obj_ty = self.subst.apply(&obj_ty);

                // Try to look up the field in struct defs.
                // This is simplified — in a real compiler we'd resolve through Named types.
                if let Some(field_ty) = self.lookup_struct_field(&obj_ty, field) {
                    field_ty
                } else if self.is_known_struct(&obj_ty) {
                    // The object's struct is known and has no such field. This
                    // used to hand back a fresh type variable, which unifies
                    // with anything — so `p.nope` typechecked against any
                    // expected type.
                    let name = self.named_name(&obj_ty).unwrap_or("?").to_string();
                    self.emit_error(format!("no field `{field}` on `{name}`"));
                    self.fresh()
                } else {
                    self.fresh()
                }
            }

            ast::Expr::Index { object, index } => {
                let obj_ty = self.infer_expr(object);
                self.infer_expr(index);

                match &obj_ty {
                    Ty::Array(inner, _) | Ty::Slice(inner) | Ty::Vec(inner) => *inner.clone(),
                    Ty::Map(_, v) => *v.clone(),
                    _ => self.fresh(),
                }
            }

            ast::Expr::StructLit { path, fields } => {
                // Checked against the struct's declaration: field names must
                // exist, their values must match the declared types, no declared
                // field may be omitted, and the literal's type is the struct —
                // not a fresh variable.
                //
                // This inferred each value, discarded the result, and returned
                // `self.fresh()`. A fresh variable unifies with anything, so
                // `@P { y: 1 }` (no such field), `@P { x: 1 }` on a two-field
                // struct, and `-> Q { @P { .. } }` all typechecked. It could not
                // have been written before user-defined types carried identity;
                // there was nothing to look the declaration up by.
                let name = path.join(".");
                let decl = self
                    .struct_defs
                    .get(&name)
                    .map(|(_, f)| f.clone());

                let Some(decl_fields) = decl else {
                    // Unknown struct (alias, generic, or not yet declared):
                    // infer the values and stay permissive.
                    for fi in fields {
                        if let Some(val) = &fi.value {
                            self.infer_expr(val);
                        }
                    }
                    return self.fresh();
                };

                for fi in fields {
                    let got = match &fi.value {
                        Some(val) => self.infer_expr(val),
                        // Shorthand `@P { x }`: the value is the local `x`.
                        None => match self.env.lookup(&fi.name) {
                            Some(t) => t.clone(),
                            None => self.fresh(),
                        },
                    };
                    match decl_fields.iter().find(|(f, _)| *f == fi.name) {
                        Some((_, want)) => {
                            if let Err(e) = unify(&mut self.subst, want, &got) {
                                self.emit_error(format!(
                                    "field `{}` of `{name}`: {e}",
                                    fi.name
                                ));
                            }
                        }
                        None => self
                            .emit_error(format!("no field `{}` on `{name}`", fi.name)),
                    }
                }

                for (want_name, _) in &decl_fields {
                    if !fields.iter().any(|fi| fi.name == *want_name) {
                        self.emit_error(format!(
                            "missing field `{want_name}` in `{name}` literal"
                        ));
                    }
                }

                Ty::Named(crate::hir::SymbolId(self.intern_named(&name)), Vec::new())
            }

            ast::Expr::TupleLit { elements } => {
                let tys: Vec<Ty> = elements.iter().map(|e| self.infer_expr(e)).collect();
                Ty::Tuple(tys)
            }

            ast::Expr::ArrayLit { elements } => {
                if elements.is_empty() {
                    return Ty::Array(Box::new(self.fresh()), 0);
                }
                // The elements of a list literal are independent list *values*,
                // so a nested literal whose rows differ in length
                // (`[[1, 2], [3]]`) is ragged, not ill-typed — only their
                // element types have to agree. `join_branches` is what widens
                // the row type to a Vec; without it `flatten: [[A]] -> [A]`
                // could flatten nothing but a rectangle.
                let mut first = self.infer_expr(&elements[0]);
                for el in &elements[1..] {
                    let t = self.infer_expr(el);
                    match self.join_branches(&first, &t) {
                        Ok(joined) => first = joined,
                        Err(e) => {
                            self.emit_error(format!("array element type mismatch: {e}"));
                        }
                    }
                }
                Ty::Array(Box::new(self.subst.apply(&first)), elements.len() as u64)
            }

            ast::Expr::MapLit { entries } => {
                if entries.is_empty() {
                    return Ty::Map(Box::new(self.fresh()), Box::new(self.fresh()));
                }
                let (k0, v0) = (
                    self.infer_expr(&entries[0].0),
                    self.infer_expr(&entries[0].1),
                );
                for (k, v) in &entries[1..] {
                    let kt = self.infer_expr(k);
                    let vt = self.infer_expr(v);
                    if let Err(e) = unify(&mut self.subst, &k0, &kt) {
                        self.emit_error(format!("map key type mismatch: {e}"));
                    }
                    if let Err(e) = unify(&mut self.subst, &v0, &vt) {
                        self.emit_error(format!("map value type mismatch: {e}"));
                    }
                }
                Ty::Map(
                    Box::new(self.subst.apply(&k0)),
                    Box::new(self.subst.apply(&v0)),
                )
            }

            ast::Expr::ArrayRepeat { value, .. } => {
                let t = self.infer_expr(value);
                Ty::Array(Box::new(t), 0) // size unknown at type level
            }

            ast::Expr::Closure { params, body } => {
                self.env.push();
                let param_tys: Vec<Ty> = params
                    .iter()
                    .map(|p| {
                        let ty = self.lower_type(&p.ty);
                        self.env.insert(p.name.clone(), ty.clone());
                        ty
                    })
                    .collect();
                let ret = self.infer_expr(body);
                self.env.pop();
                Ty::Fn(param_tys, Box::new(ret), pure())
            }

            ast::Expr::If { cond, then_block, else_block } => {
                let cond_ty = self.infer_expr(cond);
                if let Err(e) = unify(&mut self.subst, &cond_ty, &Ty::Bool) {
                    self.emit_error(format!("if condition must be bool: {e}"));
                }

                let then_ty = self.infer_block(then_block);

                if let Some(else_blk) = else_block {
                    let else_ty = self.infer_block(else_blk);
                    match self.join_branches(&then_ty, &else_ty) {
                        Ok(joined) => joined,
                        Err(e) => {
                            self.emit_error(format!("if/else branch type mismatch: {e}"));
                            self.subst.apply(&then_ty)
                        }
                    }
                } else {
                    // No else → the whole `if` is `()`, and the then-branch must
                    // be `()` too: there is no other branch to supply a value on
                    // the false path.
                    //
                    // This returned `then_ty`, contradicting the comment that
                    // was already here. The effect was a soundness hole:
                    // `f(c: bool) -> i32 { ? c { 1 } }` typechecked, so a
                    // function could fall off its end while claiming to return
                    // a value. A diverging then-branch (`ret`/`loop`) is still
                    // fine — `Ty::Never` unifies with `()` — which is what keeps
                    // `? c { ret 1; }` legal as a guard.
                    if let Err(e) = unify(&mut self.subst, &then_ty, &Ty::Unit) {
                        self.emit_error(format!(
                            "`if` without `else` must produce (): {e}"
                        ));
                    }
                    Ty::Unit
                }
            }

            ast::Expr::Match { scrutinee, arms } => {
                if let Some(s) = scrutinee {
                    self.infer_expr(s);
                }
                if arms.is_empty() {
                    return Ty::Never;
                }
                self.check_match_exhaustive(arms);
                // The running type, not the first arm's: a widening from one
                // arm has to be visible to the next, or `[a] | [] | [b, c]`
                // reports the same length disagreement all over again.
                let mut result = self.infer_expr(&arms[0].body);
                for arm in &arms[1..] {
                    let arm_ty = self.infer_expr(&arm.body);
                    match self.join_branches(&result, &arm_ty) {
                        Ok(joined) => result = joined,
                        Err(e) => self.emit_error(format!("match arm type mismatch: {e}")),
                    }
                }
                result
            }

            // `handle { body } with E { op(p) => … }`. The value is the body's,
            // exactly as if the handler were not there — a handler discharges
            // an effect, it does not change what the computation produces.
            ast::Expr::Handle { body, effect, arms } => {
                if !self.effect_defs.contains(effect) {
                    self.emit_error(format!(
                        "unknown effect `{effect}`: `handle … with` needs an \
                         `effect {effect} {{ … }}` declaration"
                    ));
                }
                for arm in arms {
                    let Some((params, ret)) =
                        self.effect_ops.get(&(effect.clone(), arm.op.clone())).cloned()
                    else {
                        self.emit_error(format!(
                            "effect `{effect}` declares no operation `{}`",
                            arm.op
                        ));
                        continue;
                    };
                    if params.len() != arm.params.len() {
                        self.emit_error(format!(
                            "handler for `{effect}.{}` binds {} parameter(s), the \
                             operation declares {}",
                            arm.op,
                            arm.params.len(),
                            params.len()
                        ));
                    }
                    // The arm's parameters take their types from the effect
                    // declaration rather than from annotations on the arm, so
                    // there is only one place for them to be written.
                    self.env.push();
                    for (name, ty) in arm.params.iter().zip(params.iter()) {
                        self.env.insert(name.clone(), ty.clone());
                    }
                    let body_ty = self.infer_expr(&arm.body);
                    self.env.pop();
                    if let Err(e) = unify(&mut self.subst, &body_ty, &ret) {
                        self.emit_error(format!(
                            "handler for `{effect}.{}` must produce what the \
                             operation returns: {e}",
                            arm.op
                        ));
                    }
                }
                self.infer_block(body)
            }
            ast::Expr::Loop { body } => {
                self.infer_block(body);
                // Loop type is determined by break expressions.
                self.fresh()
            }

            ast::Expr::While { cond, body } => {
                let cond_ty = self.infer_expr(cond);
                if let Err(e) = unify(&mut self.subst, &cond_ty, &Ty::Bool) {
                    self.emit_error(format!("while condition must be bool: {e}"));
                }
                self.infer_block(body);
                Ty::Unit
            }

            ast::Expr::For { pattern, iter, body } => {
                let _iter_ty = self.infer_expr(iter);
                self.env.push();
                // The pattern binds the element type.
                let elem_ty = self.fresh();
                self.bind_pattern(pattern, &elem_ty);
                self.infer_block(body);
                self.env.pop();
                Ty::Unit
            }

            ast::Expr::Block { block } => self.infer_block(block),

            ast::Expr::Return { value } => {
                // Check the returned value against the enclosing function's
                // declared return type. `ret` used to infer its operand and
                // discard the result, so a wrong type was silently accepted.
                let got = match value {
                    Some(v) => self.infer_expr(v),
                    None => Ty::Unit,
                };
                if let Some(want) = self.ret_stack.last().cloned()
                    && let Err(e) = unify(&mut self.subst, &want, &got) {
                        self.emit_error(format!("return type mismatch: {e}"));
                    }
                Ty::Never
            }

            ast::Expr::Break { value } => {
                if let Some(v) = value {
                    self.infer_expr(v);
                }
                Ty::Never
            }

            ast::Expr::Continue => Ty::Never,

            ast::Expr::Todo | ast::Expr::Unimplemented => Ty::Never,

            ast::Expr::UnsafeBlock { block } => self.infer_block(block),

            ast::Expr::Try { expr } => {
                let t = self.infer_expr(expr);
                // ? operator: Result<T, E> → T (propagating E).
                match &t {
                    Ty::Result(ok, _) => *ok.clone(),
                    Ty::Option(inner) => *inner.clone(),
                    _ => {
                        let ok = self.fresh();
                        let err = self.fresh();
                        if let Err(e) = unify(
                            &mut self.subst,
                            &t,
                            &Ty::Result(Box::new(ok.clone()), Box::new(err)),
                        ) {
                            self.emit_error(format!("try `?` operator: {e}"));
                        }
                        ok
                    }
                }
            }

            ast::Expr::Await { expr } => {
                // Simplified: await strips the future wrapper.
                self.infer_expr(expr)
            }

            ast::Expr::Cast { expr, ty } => {
                self.infer_expr(expr);
                self.lower_type(ty)
            }

            ast::Expr::Assign { target, value } => {
                let lt = self.infer_expr(target);
                let rt = self.infer_expr(value);
                if let Err(e) = unify(&mut self.subst, &lt, &rt) {
                    self.emit_error(format!("assignment type mismatch: {e}"));
                }
                Ty::Unit
            }

            ast::Expr::Range { start, end, .. } => {
                let st = self.infer_expr(start);
                let et = self.infer_expr(end);
                if let Err(e) = unify(&mut self.subst, &st, &et) {
                    self.emit_error(format!("range type mismatch: {e}"));
                }
                // Range<T> — simplified.
                self.fresh()
            }

            ast::Expr::Pipeline { left, right } => {
                self.infer_expr(left);
                self.infer_expr(right)
            }

            ast::Expr::Is { expr, .. } => {
                self.infer_expr(expr);
                Ty::Bool
            }

            ast::Expr::Error { .. } => Ty::Error,
        }
    }

    fn infer_literal(&mut self, kind: &ast::LiteralKind, value: &str) -> Ty {
        match kind {
            ast::LiteralKind::Int => {
                // Check for type suffix.
                if value.ends_with("i8") {
                    Ty::Int(IntTy::I8)
                } else if value.ends_with("i16") {
                    Ty::Int(IntTy::I16)
                } else if value.ends_with("i32") {
                    Ty::Int(IntTy::I32)
                } else if value.ends_with("i64") {
                    Ty::Int(IntTy::I64)
                } else if value.ends_with("i128") {
                    Ty::Int(IntTy::I128)
                } else if value.ends_with("u8") {
                    Ty::Uint(UintTy::U8)
                } else if value.ends_with("u16") {
                    Ty::Uint(UintTy::U16)
                } else if value.ends_with("u32") {
                    Ty::Uint(UintTy::U32)
                } else if value.ends_with("u64") {
                    Ty::Uint(UintTy::U64)
                } else if value.ends_with("u128") {
                    Ty::Uint(UintTy::U128)
                } else if value.ends_with("usize") {
                    Ty::Uint(UintTy::Usize)
                } else if value.ends_with("isize") {
                    Ty::Int(IntTy::Isize)
                } else {
                    // Unsuffixed integer literal: a fresh, context-polymorphic
                    // type var (unifies with any int width the surroundings
                    // demand). Unbound ones default to i32 after the function
                    // (see `default_int_literals`).
                    let ty = self.fresh();
                    if let Ty::Var(v) = &ty {
                        self.int_lit_vars.push(v.0);
                    }
                    ty
                }
            }
            ast::LiteralKind::Float => {
                if value.ends_with("f32") {
                    Ty::Float(FloatTy::F32)
                } else {
                    Ty::Float(FloatTy::F64)
                }
            }
            ast::LiteralKind::String | ast::LiteralKind::FormatString => Ty::Str,
            ast::LiteralKind::Char => Ty::Char,
            ast::LiteralKind::Bool => Ty::Bool,
            ast::LiteralKind::Byte => Ty::Uint(UintTy::U8),
        }
    }

    /// Intern a user-defined type name, returning its stable id.
    fn intern_named(&mut self, name: &str) -> u32 {
        if let Some(id) = self.named_ids.get(name) {
            return *id;
        }
        let id = self.named_names.len() as u32;
        self.named_ids.insert(name.to_string(), id);
        self.named_names.push(name.to_string());
        id
    }

    /// The struct name behind a `Ty::Named`, if it is one we interned.
    fn named_name(&self, ty: &Ty) -> Option<&str> {
        match ty {
            Ty::Named(sym, _) => self.named_names.get(sym.0 as usize).map(|s| s.as_str()),
            _ => None,
        }
    }

    /// The type of `field` on `ty`.
    ///
    /// Resolves through the *object's own* struct definition when the type is a
    /// known named struct. It used to ignore `ty` entirely and return the first
    /// field of that name found in any struct, so `a.x` could hand back an
    /// unrelated struct's `x` — possible only because `Ty::Named` carried no
    /// identity. Now that it does, this can be exact.
    fn lookup_struct_field(&self, ty: &Ty, field: &str) -> Option<Ty> {
        if let Some(name) = self.named_name(ty)
            && let Some((_, fields)) = self.struct_defs.get(name) {
                return fields.iter().find(|(f, _)| f == field).map(|(_, t)| t.clone());
            }
        // Unknown or not-yet-resolved object type: fall back to the old
        // name-only search rather than inventing a type. Still permissive, but
        // only where the object's type genuinely is not known yet.
        for (_, fields) in self.struct_defs.values() {
            for (fname, fty) in fields {
                if fname == field {
                    return Some(fty.clone());
                }
            }
        }
        None
    }

    /// Whether `ty` is a struct we know the full field list of — i.e. whether a
    /// failed field lookup is real evidence of a mistake rather than of
    /// incomplete inference.
    fn is_known_struct(&self, ty: &Ty) -> bool {
        self.named_name(ty).is_some_and(|n| self.struct_defs.contains_key(n))
    }
}

// ── Public API ───────────────────────────────────────────────────────

/// Run type checking on a parsed module.
pub fn check(module: &ast::Module) -> TypeChecker {
    let mut checker = TypeChecker::new();
    checker.check_module(module);
    checker
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn check_source(src: &str) -> TypeChecker {
        let tokens = lexer::lex(src);
        let module = parser::parse(&tokens).expect("parse failed");
        check(&module)
    }

    #[test]
    fn test_simple_function_types() {
        let tc = check_source("f add(a: i32, b: i32) -> i32 { a + b }");
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    #[test]
    fn test_type_mismatch() {
        let tc = check_source("f bad() -> i32 { 1b }");
        // 1b is a bool literal, but return is i32.
        assert!(!tc.diagnostics.is_empty(), "expected type error");
    }

    // ── §8 standard-vocabulary precise typing ────────────────────────────
    #[test]
    fn vocab_scalar_returns_are_precise() {
        // len → usize, sum → element type — both type-check cleanly.
        let tc = check_source("f a() -> usize { len([1, 2, 3]) }\nf b() -> i32 { sum([1, 2, 3]) }");
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    #[test]
    fn vocab_rejects_non_collection() {
        // The reliability win: a non-collection argument is caught (concrete and
        // integer-literal forms).
        assert!(!check_source("f t() { sum(\"hi\") }").diagnostics.is_empty());
        assert!(!check_source("f t() { sum(5) }").diagnostics.is_empty());
        assert!(!check_source("f t() { len(42) }").diagnostics.is_empty());
    }

    #[test]
    fn ragged_nested_list_literals_are_not_length_errors() {
        // A list literal's elements are independent list values, so rows of
        // differing length are ragged, not ill-typed. Unifying the fixed
        // lengths rejected these, which made `flatten: [[A]] -> [A]` able to
        // flatten only a rectangle — a function that cannot do the one thing
        // its published signature describes.
        let tc = check_source("f t() { val a = [[1, 2], [3]]\n len(a) }");
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);

        let tc = check_source("f t() -> i32 { sum(flatten([[1, 2], [3]])) }");
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    #[test]
    fn ragged_widening_still_rejects_real_element_mismatches() {
        // The widening above must key on the *length* disagreeing, not swallow
        // every nested-list error: rows whose element types genuinely differ
        // are still wrong, at equal and unequal lengths alike.
        assert!(!check_source("f t() { [[1, 2], [\"a\"]] }").diagnostics.is_empty());
        assert!(!check_source("f t() { [[1, 2], [\"a\", \"b\"]] }").diagnostics.is_empty());
    }

    #[test]
    fn vocab_totality_first_returns_option() {
        // `first` returns `?A`, so using it as a bare value is an error — the
        // agent is forced to handle the empty case (totality).
        let tc = check_source("f t() -> i32 { first([1, 2, 3]) }");
        assert!(!tc.diagnostics.is_empty(), "first must return an Option");
    }

    #[test]
    fn vocab_higher_order_compose() {
        // map + fold with named functions, the result threaded into an i32.
        let tc = check_source(
            "f sq(n) { n * n }\nf add(acc, x) { acc + x }\nf t() -> i32 { fold(map([1, 2, 3], sq), 0, add) }",
        );
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    #[test]
    fn vocab_arity_is_checked() {
        assert!(!check_source("f t() { sum() }").diagnostics.is_empty());
        assert!(!check_source("f t() { map([1, 2, 3]) }").diagnostics.is_empty());
    }

    #[test]
    fn user_function_shadows_vocab() {
        // A user-defined `sum` takes precedence over the builtin — `sum(5)` is
        // then a valid call to the user function, not a vocabulary misuse.
        let tc = check_source("f sum(x: i32) -> i32 { x }\nf t() -> i32 { sum(5) }");
        assert!(tc.diagnostics.is_empty(), "user fn should shadow: {:?}", tc.diagnostics);
    }

    #[test]
    fn int_literals_are_width_polymorphic() {
        // Agentic-fix regression: an unsuffixed int literal adopts the
        // annotated width (was hard-typed i32 → `i64 vs i32` mismatch).
        let tc = check_source("f f() -> i64 { val x: i64 = 3; x }");
        assert!(tc.diagnostics.is_empty(), "i64 literal should unify: {:?}", tc.diagnostics);
        // And still defaults to i32 when unconstrained.
        let tc2 = check_source("f g() -> i32 { val y = 5; y }");
        assert!(tc2.diagnostics.is_empty(), "default-i32 should hold: {:?}", tc2.diagnostics);
    }

    #[test]
    fn non_exhaustive_match_is_caught() {
        let src = "data Color = Red | Green | Blue\nf n(c: Color) -> i32 { match c { Color.Red => 0, Color.Green => 1, } }";
        let tc = check_source(src);
        assert!(
            tc.diagnostics.iter().any(|d| d.message.contains("non-exhaustive")),
            "missing Blue should be caught: {:?}",
            tc.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn exhaustive_and_catchall_matches_are_clean() {
        let full = "data Color = Red | Green | Blue\nf n(c: Color) -> i32 { match c { Color.Red => 0, Color.Green => 1, Color.Blue => 2, } }";
        assert!(!check_source(full).diagnostics.iter().any(|d| d.message.contains("non-exhaustive")));
        let wild = "data Color = Red | Green | Blue\nf n(c: Color) -> i32 { match c { Color.Red => 0, _ => 9, } }";
        assert!(!check_source(wild).diagnostics.iter().any(|d| d.message.contains("non-exhaustive")));
    }

    #[test]
    fn array_literal_coerces_to_vec() {
        // Agentic-fix regression: `[a,b,c]` / `[]` literal assigned to a Vec
        // annotation type-checks (was `[T]~ vs [T; n]`).
        let tc = check_source("f f() -> i64 { val xs: [i64]~ = [1, 2, 3]; 0 }");
        assert!(tc.diagnostics.is_empty(), "array→Vec coercion: {:?}", tc.diagnostics);
    }

    #[test]
    fn test_let_binding_inference() {
        let src = r#"
            f foo() -> i32 {
                v x: i32 = 42;
                x
            }
        "#;
        let tc = check_source(src);
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    #[test]
    fn test_if_branch_types() {
        let src = r#"
            f pick(flag: bool) -> i32 {
                ? flag { 1 } : { 2 }
            }
        "#;
        let tc = check_source(src);
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    #[test]
    fn test_if_branch_mismatch() {
        let src = r#"
            f pick(flag: bool) -> i32 {
                ? flag { 1 } : { 1b }
            }
        "#;
        let tc = check_source(src);
        assert!(!tc.diagnostics.is_empty(), "expected branch type mismatch");
    }

    /// One branch returns a result, the other returns nothing. This is the
    /// single most common shape in the language and it was rejected outright
    /// as `array size mismatch: 1 vs 0`, because branch types were unified
    /// including the literal lengths.
    #[test]
    fn branches_returning_lists_of_different_lengths_widen_to_a_vec() {
        let src = r#"
            f flag(bad: bool) -> [i32]~ {
                ? bad { [1] } : { [] }
            }
        "#;
        let tc = check_source(src);
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    /// Three arms, three lengths. The widening has to carry forward: comparing
    /// every arm against the *first* one reports the mismatch again on arm 3.
    #[test]
    fn match_arms_returning_lists_of_different_lengths_widen_to_a_vec() {
        let src = r#"
            E Level { Lo, Mid, Hi }
            f items(level: Level) -> [i32]~ {
                ?= level {
                    Level.Lo => [],
                    Level.Mid => [1],
                    Level.Hi => [1, 2],
                }
            }
        "#;
        let tc = check_source(src);
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    /// The widening is only about *length*. Branches whose element types
    /// genuinely disagree are still an error, or the rule would swallow real
    /// mismatches under the guise of being helpful.
    #[test]
    fn branches_whose_element_types_differ_are_still_rejected() {
        let src = r#"
            f pick(flag: bool) -> [i32]~ {
                ? flag { [1] } : { [1b, 0b] }
            }
        "#;
        let tc = check_source(src);
        assert!(!tc.diagnostics.is_empty(), "expected an element type mismatch");
    }

    #[test]
    fn test_binary_op_type_propagation() {
        let src = r#"
            f calc(x: f64, y: f64) -> f64 {
                x * y + x
            }
        "#;
        let tc = check_source(src);
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    #[test]
    fn test_closure_typing() {
        let src = r#"
            f apply() -> i32 {
                v double = f(x: i32) => x * 2;
                double(21)
            }
        "#;
        let tc = check_source(src);
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }

    #[test]
    fn test_comparison_returns_bool() {
        let src = r#"
            f is_positive(x: i32) -> bool {
                x > 0
            }
        "#;
        let tc = check_source(src);
        assert!(tc.diagnostics.is_empty(), "errors: {:?}", tc.diagnostics);
    }
}

#[cfg(test)]
mod return_stmt_tests {
    use crate::{lexer, parser, types};

    fn errors(src: &str) -> Vec<String> {
        let toks = lexer::lex(src);
        let module = parser::parse(&toks).expect("parses");
        let mut tc = types::TypeChecker::new();
        tc.check_module(&module);
        tc.diagnostics.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn a_semicolon_terminated_return_is_not_a_unit_block() {
        // `ret 9;` is a statement, so the block had no tail expression and was
        // typed `()`, which then failed to unify with `-> i32`. Dropping the
        // semicolon worked — a strange thing for a compiler to require, and it
        // made `--syntax=legacy` unusable, since the Rust transpiler renders
        // `return a + b;` as exactly the failing form.
        assert!(errors("+f nine() -> i32 { ret 9; }").is_empty());
        assert!(errors("+f add(a: i32, b: i32) -> i32 { ret a + b; }").is_empty());
        assert!(errors("+f early(a: i32) -> i32 { ? a > 0 { ret 1; } ret 0; }").is_empty());
    }

    #[test]
    fn a_bare_return_still_suits_a_unit_function() {
        assert!(errors("+f nothing() { ret; }").is_empty());
    }

    #[test]
    fn the_returned_value_is_checked_against_the_declared_type() {
        // The hole the fix above exposed: `ret` inferred its operand and threw
        // the result away, so this was accepted. It only looked checked because
        // the block-typing bug rejected the function for an unrelated reason.
        let e = errors(r#"+f wrong() -> i32 { ret "s"; }"#);
        assert!(!e.is_empty(), "returning a string from an i32 function must fail");
        assert!(
            e.iter().any(|m| m.contains("return type mismatch")),
            "expected a return type mismatch, got {e:?}"
        );
    }

    #[test]
    fn a_wrong_tail_expression_is_still_caught() {
        // Guard against the fix silencing the original check: `Ty::Never`
        // unifies with anything, so a too-eager divergence rule would make
        // every mismatch disappear.
        assert!(!errors(r#"+f wrong() -> i32 { "s" }"#).is_empty());
    }
}

#[cfg(test)]
mod if_without_else_tests {
    use crate::{lexer, parser, types};

    fn errors(src: &str) -> Vec<String> {
        let toks = lexer::lex(src);
        let module = parser::parse(&toks).expect("parses");
        let mut tc = types::TypeChecker::new();
        tc.check_module(&module);
        tc.diagnostics.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn a_function_cannot_fall_off_its_end() {
        // `if` without `else` returned its then-branch type, so a function
        // could claim `-> i32` and produce nothing on the false path. The
        // comment in the code already said "No else → must be unit"; the code
        // did the opposite.
        assert!(
            !errors("+f a(c: bool) -> i32 { ? c { 1 } }").is_empty(),
            "a then-branch value with no else cannot satisfy -> i32"
        );
        assert!(
            !errors("+f h(c: bool) -> i32 { ? c { ret 1; } }").is_empty(),
            "returning only on the true path still falls through on the false one"
        );
    }

    #[test]
    fn diverging_guards_remain_legal() {
        // The point of allowing `Ty::Never` to unify: an early-return guard is
        // the single most common shape in real code, and a fix that outlawed it
        // would be worse than the bug.
        assert!(errors("+f g(c: bool) -> i32 { ? c { ret 1; } ret 0; }").is_empty());
        assert!(errors("+f f(c: bool) -> i32 { ? c { ret 1; } : { ret 2; } }").is_empty());
    }

    #[test]
    fn a_unit_if_is_still_fine() {
        assert!(errors("+f u(c: bool) { ? c { } }").is_empty());
    }

    /// The shape that produced a wrong answer rather than an error: the else
    /// block evaluates to a value, nothing leaves, and the body runs anyway
    /// with the precondition false. `a(-5)` returned `-10`, not `0`.
    #[test]
    fn a_guard_whose_else_does_not_leave_is_rejected() {
        assert!(
            !errors("+f a(n: i32) -> i32 { guard n > 0 else { 0 }\n n * 2 }").is_empty(),
            "an else block that merely produces a value does not stop anything"
        );
    }

    /// An empty else stops nothing at all, and is the easiest version of the
    /// mistake to write.
    #[test]
    fn a_guard_with_an_empty_else_is_rejected() {
        assert!(!errors("+f a(n: i32) -> i32 { guard n > 0 else { }\n n * 2 }").is_empty());
    }

    /// Every spelling that really does leave stays legal, including a block
    /// that does work first — the rule is about the block diverging, not about
    /// `return` being its only statement.
    #[test]
    fn guards_that_actually_return_stay_legal() {
        for src in [
            "+f a(n: i32) -> i32 { guard n > 0 else { return 0 }\n n * 2 }",
            "+f a(n: i32) -> i32 { guard n > 0 else { return 0; }\n n * 2 }",
            "+f a(n: i32) -> i32 { guard n > 0 else { ret 0 }\n n * 2 }",
            "+f a(n: i32) -> i32 { guard n > 0 else { v x = 1\n return x }\n n * 2 }",
        ] {
            assert!(errors(src).is_empty(), "should check clean: {src}");
        }
    }
}

#[cfg(test)]
mod int_literal_tests {
    use crate::{lexer, parser, types};

    fn errors(src: &str) -> Vec<String> {
        let toks = lexer::lex(src);
        let module = parser::parse(&toks).expect("parses");
        let mut tc = types::TypeChecker::new();
        tc.check_module(&module);
        tc.diagnostics.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn an_integer_literal_is_not_a_bool_or_a_string() {
        // The literal's type var was minted completely free, so it unified with
        // anything at all — the comment at the mint site claimed "any int width
        // the surroundings demand" and nothing enforced the "int" part.
        assert!(!errors("+f f() -> bool { 1 }").is_empty());
        assert!(!errors("+f f() -> str { 1 }").is_empty());
    }

    #[test]
    fn every_numeric_context_still_accepts_one() {
        // The constraint must not become a false positive: an unsuffixed literal
        // is polymorphic across int widths, unsigned, and float on purpose.
        for src in [
            "+f f() -> i32 { 1 }",
            "+f f() -> i64 { 1 }",
            "+f f() -> u8 { 1 }",
            "+f f() -> f64 { 1 }",
            "+f f() -> i64 { val x = 1; ret x; }",
        ] {
            assert!(errors(src).is_empty(), "{src} should typecheck, got {:?}", errors(src));
        }
    }
}

#[cfg(test)]
mod nominal_typing_tests {
    use crate::{lexer, parser, types};

    fn errors(src: &str) -> Vec<String> {
        let toks = lexer::lex(src);
        let module = parser::parse(&toks).expect("parses");
        let mut tc = types::TypeChecker::new();
        tc.check_module(&module);
        tc.diagnostics.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn distinct_structs_are_distinct_types() {
        // Every user-defined type lowered to `SymbolId(u32::MAX)` — the same
        // sentinel for all of them — and `unify` compares those ids, so any
        // user type satisfied any other.
        assert!(!errors("S A { x: i32 } S B { y: i32 } +f f(a: A) -> B { a }").is_empty());
        assert!(
            !errors(
                "S A { x: i32 } S B { y: i32 } \
                 +f g(b: B) -> i32 { b.y } +f f(a: A) -> i32 { g(a) }"
            )
            .is_empty(),
            "passing an A where B is expected must fail"
        );
    }

    #[test]
    fn a_struct_is_still_itself() {
        assert!(errors("S A { x: i32 } +f f(a: A) -> A { a }").is_empty());
        assert!(errors("S P { x: i32 } +f f(p: P) -> i32 { p.x }").is_empty());
    }

    #[test]
    fn a_field_that_does_not_exist_is_an_error() {
        // Returned a fresh type variable, which unifies with anything, so
        // `p.nope` satisfied any expected type.
        let e = errors("S P { x: i32 } +f f(p: P) -> i32 { p.nope }");
        assert!(e.iter().any(|m| m.contains("no field `nope`")), "got {e:?}");
    }

    #[test]
    fn a_field_is_resolved_on_the_right_struct() {
        // Field lookup ignored the object type and returned the first match in
        // *any* struct, so this handed back B's `y: str` for an A.
        assert!(!errors("S A { x: i32 } S B { y: str } +f f(a: A) -> str { a.y }").is_empty());
    }
}

#[cfg(test)]
mod struct_literal_tests {
    use crate::{lexer, parser, types};

    fn errors(src: &str) -> Vec<String> {
        let toks = lexer::lex(src);
        let module = parser::parse(&toks).expect("parses");
        let mut tc = types::TypeChecker::new();
        tc.check_module(&module);
        tc.diagnostics.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn a_valid_literal_typechecks_and_has_the_struct_type() {
        assert!(errors("S P { x: i32 } +f f() -> P { @P { x: 1 } }").is_empty());
    }

    #[test]
    fn unknown_and_missing_fields_are_caught() {
        // The literal inferred its values, discarded them, and returned a fresh
        // type variable — which unifies with anything.
        assert!(!errors("S P { x: i32 } +f f() -> P { @P { y: 1 } }").is_empty());
        assert!(
            !errors("S P { x: i32, z: i32 } +f f() -> P { @P { x: 1 } }").is_empty(),
            "omitting a declared field must fail"
        );
    }

    #[test]
    fn a_field_value_must_match_its_declared_type() {
        // `1b` / `0b` are MAGE's boolean literals — there is no `true`/`false`
        // keyword, as `MAGE_ONTOLOGY.json` states. Worth knowing before writing
        // a test in Rust's syntax and misreading the resolver's "unresolved
        // name: `true`" as a type-checking failure.
        assert!(!errors("S P { x: i32 } +f f() -> P { @P { x: 1b } }").is_empty());
    }

    #[test]
    fn the_literal_has_its_own_struct_type_not_a_free_variable() {
        // Two structs with identical shapes are still different types.
        assert!(
            !errors("S P { x: i32 } S Q { x: i32 } +f f() -> Q { @P { x: 1 } }").is_empty()
        );
    }
}

#[cfg(test)]
mod string_length_tests {
    use crate::{lexer, parser, types};

    fn errors(src: &str) -> Vec<String> {
        let toks = lexer::lex(src);
        let module = parser::parse(&toks).expect("parses");
        let mut tc = types::TypeChecker::new();
        tc.check_module(&module);
        tc.diagnostics.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn len_and_count_accept_a_string() {
        // A `str` was a string for `upper`/`lower`/`split` and "not a
        // collection" for `len`/`count`. This is what broke
        // examples/hello-world, whose comment advertises exactly this.
        assert!(errors("+f f(s: str) -> usize { len(s) }").is_empty());
        assert!(errors("+f f(s: str) -> usize { count(s) }").is_empty());
    }

    #[test]
    fn collections_still_work_and_nonsense_still_fails() {
        // The first attempt widened `collection_elem` itself, which also made
        // `sum("hi")` typecheck. Length-like and element-like uses need
        // different rules, and the existing test for `sum` was right.
        assert!(errors("+f f(v: [i32]) -> usize { len(v) }").is_empty());
        assert!(!errors(r#"+f f() -> i32 { sum("hi") }"#).is_empty());
        assert!(!errors("+f f() -> i32 { sum(5) }").is_empty());
    }
}
