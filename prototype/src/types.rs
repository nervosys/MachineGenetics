/// MechGen Type Checker — bidirectional type checking with HM-style unification.
///
/// Implements the type judgment:  Γ; Σ; Δ ⊢ e : τ ⊣ ε
///
/// - Γ = type environment (name → Ty)
/// - Constraint generation: walk the AST, emit τ₁ ≡ τ₂ constraints
/// - Unification: Robinson's algorithm extended for MechGen types
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

pub struct TypeChecker {
    supply: TyVarSupply,
    subst: Subst,
    env: TypeEnv,
    /// Struct definitions: name → (generic_params, fields: Vec<(name, Ty)>).
    struct_defs: HashMap<String, (Vec<String>, Vec<(String, Ty)>)>,
    /// Function signatures: name → (params: Vec<Ty>, return: Ty, effects).
    fn_sigs: HashMap<String, (Vec<Ty>, Ty, Vec<String>)>,
    /// Enum definitions: enum name → its variant names. Used for match
    /// exhaustiveness checking.
    enum_defs: HashMap<String, Vec<String>>,
    pub diagnostics: Vec<Diagnostic>,
    /// Type-var ids minted for unsuffixed integer literals. They unify with
    /// any concrete int width from context; whatever stays unbound at end of a
    /// function defaults to i32 (Rust-style integer literal polymorphism). This
    /// is what lets `let x: i64 = 3` and `[i64]~ = [1,2,3]` check clean.
    int_lit_vars: Vec<u32>,
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
                // Could be a user-defined struct/enum/type alias.
                // Return a named type that we'll verify later.
                Ty::Named(crate::hir::SymbolId(u32::MAX), args)
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
            // `data X = A | B` (sum type) is the idiomatic MechGen enum.
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

        // Bind parameters.
        for param in &fd.params {
            let ty = self.lower_type(&param.ty);
            self.env.insert(param.name.clone(), ty);
        }

        // Infer body type.
        let body_ty = if let Some(be) = &fd.body_expr {
            self.infer_expr(be)
        } else {
            self.infer_block(&fd.body)
        };

        // Unify body type with the return type. When unannotated, reuse the
        // fresh var registered in the signature so the body *infers* the return
        // type (and recursive calls resolve to the same one).
        let ret_ty = match &fd.return_type {
            Some(t) => self.lower_type(t),
            None => self.fn_sigs.get(&fd.name).map(|s| s.1.clone()).unwrap_or(Ty::Unit),
        };

        if let Err(e) = unify(&mut self.subst, &ret_ty, &body_ty) {
            self.emit_error(format!("function `{}`: return type mismatch: {e}", fd.name));
        }

        // Default any integer literals left unconstrained by context to i32.
        self.default_int_literals();

        self.env.pop();
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
    /// to a concrete width, binding them to i32 (the MechGen integer default).
    /// Constrained ones are already bound by unification and are left alone.
    fn default_int_literals(&mut self) {
        let pending = std::mem::take(&mut self.int_lit_vars);
        for v in pending {
            let tv = Ty::Var(crate::hir::TyVar(v));
            if matches!(self.subst.apply(&tv), Ty::Var(_)) {
                // Still unbound → default to i32.
                let _ = unify(&mut self.subst, &tv, &Ty::Int(IntTy::I32));
            }
        }
    }

    // ── Block inference ──────────────────────────────────────────────

    fn infer_block(&mut self, block: &ast::Block) -> Ty {
        self.env.push();

        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }

        let ty = if let Some(tail) = &block.tail_expr { self.infer_expr(tail) } else { Ty::Unit };

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
                self.infer_block(else_block);
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
                if let ast::Expr::Ident { name } = func.as_ref() {
                    if name == "grad" && args.len() == 1 {
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
                }

                // Direct call to a function known by name: use its signature
                // directly. This (a) fixes zero-arg calls — a bare function
                // Ident resolves to its *return type* (see Expr::Ident), so the
                // generic `Fn` unification below would see `() vs f()->?T` for a
                // unit-returning function; and (b) yields precise arity and
                // per-argument diagnostics instead of one opaque `call:` error.
                if let ast::Expr::Ident { name } = func.as_ref() {
                    if let Some((params, ret, _)) = self.fn_sigs.get(name).cloned() {
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

            ast::Expr::MethodCall { receiver, args, .. } => {
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

            ast::Expr::StructLit { fields, .. } => {
                for fi in fields {
                    if let Some(val) = &fi.value {
                        self.infer_expr(val);
                    }
                }
                // Return the struct type — simplified for prototype.
                self.fresh()
            }

            ast::Expr::TupleLit { elements } => {
                let tys: Vec<Ty> = elements.iter().map(|e| self.infer_expr(e)).collect();
                Ty::Tuple(tys)
            }

            ast::Expr::ArrayLit { elements } => {
                if elements.is_empty() {
                    return Ty::Array(Box::new(self.fresh()), 0);
                }
                let first = self.infer_expr(&elements[0]);
                for el in &elements[1..] {
                    let t = self.infer_expr(el);
                    if let Err(e) = unify(&mut self.subst, &first, &t) {
                        self.emit_error(format!("array element type mismatch: {e}"));
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
                    if let Err(e) = unify(&mut self.subst, &then_ty, &else_ty) {
                        self.emit_error(format!("if/else branch type mismatch: {e}"));
                    }
                    self.subst.apply(&then_ty)
                } else {
                    // No else → must be unit.
                    then_ty
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
                let first_ty = self.infer_expr(&arms[0].body);
                for arm in &arms[1..] {
                    let arm_ty = self.infer_expr(&arm.body);
                    if let Err(e) = unify(&mut self.subst, &first_ty, &arm_ty) {
                        self.emit_error(format!("match arm type mismatch: {e}"));
                    }
                }
                self.subst.apply(&first_ty)
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
                if let Some(v) = value {
                    self.infer_expr(v);
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

    fn lookup_struct_field(&self, _ty: &Ty, field: &str) -> Option<Ty> {
        // For Named types with struct defs, look up the field.
        // Simplified for prototype — check all structs.
        for (_, (_, fields)) in &self.struct_defs {
            for (fname, fty) in fields {
                if fname == field {
                    return Some(fty.clone());
                }
            }
        }
        None
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
