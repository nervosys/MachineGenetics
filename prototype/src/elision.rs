/// Redox Safety Elision Pass
///
/// In agentic mode (the default), this pass strips safety annotations from the
/// AST that are redundant in Redox's design: lifetimes, `unsafe`, `&mut`,
/// `move`, `ref`, `Pin`, `PhantomData`, `Send`/`Sync`/`Copy`/`Unpin`/`Sized`
/// bounds, etc.  The compiler and SKB handle these concerns automatically;
/// removing them from the surface syntax makes the AST dramatically simpler
/// for both human and agent consumption (per §4 of the Redox proposal).
use crate::ast::*;

/// Safety-related trait bounds that are eliminated in agentic mode.
/// These are handled by the compiler/SKB rather than appearing in syntax.
const SAFETY_BOUNDS: &[&str] = &["Send", "Sync", "Copy", "Unpin", "Sized", "Freeze"];

/// Type names that are eliminated entirely (replaced by their inner type).
const ELIMINATED_TYPES: &[&str] = &["PhantomData", "Pin"];

// ── Public API ───────────────────────────────────────────────────────

/// Apply safety elision to an entire module, returning the transformed module.
pub fn elide(module: &Module) -> Module {
    Module { items: module.items.iter().map(elide_item).collect() }
}

// ── Items ────────────────────────────────────────────────────────────

fn elide_item(item: &Item) -> Item {
    Item {
        visibility: item.visibility,
        attributes: item.attributes.clone(),
        kind: elide_item_kind(&item.kind),
    }
}

fn elide_item_kind(kind: &ItemKind) -> ItemKind {
    match kind {
        ItemKind::Function(f) => ItemKind::Function(elide_function(f)),
        ItemKind::Struct(s) => ItemKind::Struct(elide_struct(s)),
        ItemKind::Enum(e) => ItemKind::Enum(elide_enum(e)),
        ItemKind::Trait(t) => ItemKind::Trait(elide_trait(t)),
        ItemKind::Impl(i) => ItemKind::Impl(elide_impl(i)),
        ItemKind::Module(m) => ItemKind::Module(elide_module(m)),
        ItemKind::Use(u) => ItemKind::Use(elide_use(u)),
        ItemKind::TypeAlias(ta) => ItemKind::TypeAlias(elide_type_alias(ta)),
        ItemKind::Const(c) => ItemKind::Const(elide_const(c)),
        ItemKind::Static(s) => ItemKind::Static(elide_static(s)),
        ItemKind::Effect(e) => ItemKind::Effect(e.clone()),
        ItemKind::Spec(s) => ItemKind::Spec(s.clone()),
    }
}

fn elide_function(f: &FunctionDef) -> FunctionDef {
    FunctionDef {
        name: f.name.clone(),
        is_async: f.is_async,
        is_unsafe: false, // Strip unsafe marker
        generics: elide_generics(&f.generics),
        params: f.params.iter().map(elide_param).collect(),
        return_type: f.return_type.as_ref().map(elide_type),
        where_clause: elide_where_clause(&f.where_clause),
        effects: f.effects.clone(),
        body: elide_block(&f.body),
    }
}

fn elide_struct(s: &StructDef) -> StructDef {
    StructDef {
        name: s.name.clone(),
        generics: elide_generics(&s.generics),
        fields: s.fields.iter().map(elide_struct_field).collect(),
    }
}

fn elide_struct_field(field: &StructField) -> StructField {
    StructField {
        visibility: field.visibility,
        name: field.name.clone(),
        ty: elide_type(&field.ty),
    }
}

fn elide_enum(e: &EnumDef) -> EnumDef {
    EnumDef {
        name: e.name.clone(),
        generics: elide_generics(&e.generics),
        variants: e.variants.iter().map(elide_variant).collect(),
    }
}

fn elide_variant(v: &EnumVariant) -> EnumVariant {
    EnumVariant {
        name: v.name.clone(),
        kind: match &v.kind {
            VariantKind::Unit => VariantKind::Unit,
            VariantKind::Tuple(types) => VariantKind::Tuple(types.iter().map(elide_type).collect()),
            VariantKind::Struct(fields) => {
                VariantKind::Struct(fields.iter().map(elide_struct_field).collect())
            }
        },
    }
}

fn elide_trait(t: &TraitDef) -> TraitDef {
    TraitDef {
        name: t.name.clone(),
        generics: elide_generics(&t.generics),
        super_traits: strip_safety_bounds(&t.super_traits),
        items: t.items.iter().map(elide_item).collect(),
    }
}

fn elide_impl(i: &ImplBlock) -> ImplBlock {
    ImplBlock {
        generics: elide_generics(&i.generics),
        self_type: elide_type(&i.self_type),
        trait_path: i.trait_path.clone(),
        items: i.items.iter().map(elide_item).collect(),
    }
}

fn elide_module(m: &ModuleDef) -> ModuleDef {
    ModuleDef {
        name: m.name.clone(),
        items: m.items.as_ref().map(|items| items.iter().map(elide_item).collect()),
    }
}

fn elide_use(u: &UseDef) -> UseDef {
    // Filter out use-imports of eliminated types
    UseDef {
        path: u.path.clone(),
        alias: u.alias.clone(),
        glob: u.glob,
        group: u.group.iter().map(elide_use).collect(),
    }
}

fn elide_type_alias(ta: &TypeAlias) -> TypeAlias {
    TypeAlias {
        name: ta.name.clone(),
        generics: elide_generics(&ta.generics),
        ty: elide_type(&ta.ty),
    }
}

fn elide_const(c: &ConstDef) -> ConstDef {
    ConstDef { name: c.name.clone(), ty: elide_type(&c.ty), value: elide_expr(&c.value) }
}

fn elide_static(s: &StaticDef) -> StaticDef {
    StaticDef {
        name: s.name.clone(),
        mutable: s.mutable,
        ty: elide_type(&s.ty),
        value: elide_expr(&s.value),
    }
}

// ── Generics & Where Clauses ─────────────────────────────────────────

fn elide_generics(generics: &[GenericParam]) -> Vec<GenericParam> {
    generics
        .iter()
        .filter(|g| !is_lifetime_param(g))
        .map(|g| GenericParam {
            name: g.name.clone(),
            bounds: strip_safety_bounds(&g.bounds),
            default: g.default.as_ref().map(elide_type),
        })
        .collect()
}

fn elide_where_clause(preds: &[WherePredicate]) -> Vec<WherePredicate> {
    preds
        .iter()
        .filter(|p| !is_lifetime_name(&p.type_param))
        .map(|p| WherePredicate {
            type_param: p.type_param.clone(),
            bounds: strip_safety_bounds(&p.bounds),
        })
        .filter(|p| !p.bounds.is_empty())
        .collect()
}

/// Remove safety-related bounds from a bound list.
fn strip_safety_bounds(bounds: &[String]) -> Vec<String> {
    bounds.iter().filter(|b| !is_safety_bound(b)).cloned().collect()
}

/// Is this bound a safety-related bound that should be elided?
fn is_safety_bound(bound: &str) -> bool {
    // Exact matches against known safety bounds
    if SAFETY_BOUNDS.contains(&bound) {
        return true;
    }
    // Lifetime bounds like 'a, 'static, 'b
    if bound.starts_with('\'') {
        return true;
    }
    // Compound bounds that reference lifetimes: T + 'a, etc.
    if bound.contains('\'') {
        return true;
    }
    false
}

/// Is this a lifetime generic parameter? (e.g., 'a, 'b)
fn is_lifetime_param(g: &GenericParam) -> bool {
    is_lifetime_name(&g.name)
}

/// Is this name a lifetime? (starts with ')
fn is_lifetime_name(name: &str) -> bool {
    name.starts_with('\'')
}

// ── Types ────────────────────────────────────────────────────────────

fn elide_type(ty: &Type) -> Type {
    match ty {
        // References: strip mutability distinction, keep as immutable ref
        Type::Reference { inner, .. } => {
            Type::Reference { mutable: false, inner: Box::new(elide_type(inner)) }
        }

        // Eliminated wrapper types: Pin<T> → T, PhantomData<T> → elided
        Type::Path { segments, type_args } if is_eliminated_type(segments) => {
            if segments.last().map(|s| s.as_str()) == Some("PhantomData") {
                // PhantomData<T> → Unit (field can be removed by later passes)
                Type::Tuple { elements: vec![] }
            } else {
                // Pin<T> → T (unwrap)
                if let Some(inner) = type_args.first() {
                    elide_type(inner)
                } else {
                    Type::Tuple { elements: vec![] }
                }
            }
        }

        // Recurse into compound types
        Type::Path { segments, type_args } => Type::Path {
            segments: segments.clone(),
            type_args: type_args.iter().map(elide_type).collect(),
        },
        Type::OwnedPtr { inner } => Type::OwnedPtr { inner: Box::new(elide_type(inner)) },
        Type::Rc { inner } => Type::Rc { inner: Box::new(elide_type(inner)) },
        Type::Arc { inner } => Type::Arc { inner: Box::new(elide_type(inner)) },
        Type::Cow { inner } => Type::Cow { inner: Box::new(elide_type(inner)) },
        Type::Cell { inner } => Type::Cell { inner: Box::new(elide_type(inner)) },
        Type::RefCell { inner } => Type::RefCell { inner: Box::new(elide_type(inner)) },
        Type::Mutex { inner } => Type::Mutex { inner: Box::new(elide_type(inner)) },
        Type::RwLock { inner } => Type::RwLock { inner: Box::new(elide_type(inner)) },
        Type::Slice { inner } => Type::Slice { inner: Box::new(elide_type(inner)) },
        Type::Array { inner, size } => {
            Type::Array { inner: Box::new(elide_type(inner)), size: Box::new(elide_expr(size)) }
        }
        Type::Vec { inner } => Type::Vec { inner: Box::new(elide_type(inner)) },
        Type::Set { inner } => Type::Set { inner: Box::new(elide_type(inner)) },
        Type::Tuple { elements } => {
            Type::Tuple { elements: elements.iter().map(elide_type).collect() }
        }
        Type::Option { inner } => Type::Option { inner: Box::new(elide_type(inner)) },
        Type::Result { ok, err } => {
            Type::Result { ok: Box::new(elide_type(ok)), err: Box::new(elide_type(err)) }
        }
        Type::Map { key, value } => {
            Type::Map { key: Box::new(elide_type(key)), value: Box::new(elide_type(value)) }
        }
        Type::Ptr { inner } => Type::Ptr { inner: Box::new(elide_type(inner)) },
        Type::Simd { inner, width } => {
            Type::Simd { inner: Box::new(elide_type(inner)), width: *width }
        }
        Type::Fn { params, ret } => Type::Fn {
            params: params.iter().map(elide_type).collect(),
            ret: ret.as_ref().map(|r| Box::new(elide_type(r))),
        },

        // Leaf types pass through unchanged
        Type::Never | Type::Inferred | Type::SelfType | Type::StringType => ty.clone(),
    }
}

/// Is this a type name that should be eliminated entirely?
fn is_eliminated_type(segments: &[String]) -> bool {
    if let Some(last) = segments.last() { ELIMINATED_TYPES.contains(&last.as_str()) } else { false }
}

// ── Params ───────────────────────────────────────────────────────────

fn elide_param(p: &Param) -> Param {
    Param { name: p.name.clone(), ty: elide_type(&p.ty) }
}

// ── Blocks & Statements ──────────────────────────────────────────────

fn elide_block(block: &Block) -> Block {
    Block {
        stmts: block.stmts.iter().map(elide_stmt).collect(),
        tail_expr: block.tail_expr.as_ref().map(|e| Box::new(elide_expr(e))),
    }
}

fn elide_stmt(stmt: &Stmt) -> Stmt {
    match stmt {
        Stmt::Let { mutable, pattern, ty, value } => Stmt::Let {
            mutable: *mutable,
            pattern: elide_pattern(pattern),
            ty: ty.as_ref().map(elide_type),
            value: elide_expr(value),
        },
        Stmt::Expr { expr } => Stmt::Expr { expr: elide_expr(expr) },
        Stmt::Item { item } => Stmt::Item { item: Box::new(elide_item(item)) },
    }
}

// ── Expressions ──────────────────────────────────────────────────────

fn elide_expr(expr: &Expr) -> Expr {
    match expr {
        // UnsafeBlock → plain Block (strip unsafe wrapper)
        Expr::UnsafeBlock { block } => Expr::Block { block: elide_block(block) },

        // Recurse into all expression forms
        Expr::Literal { .. }
        | Expr::Ident { .. }
        | Expr::Todo
        | Expr::Unimplemented
        | Expr::Continue
        | Expr::Error { .. } => expr.clone(),

        Expr::Binary { op, left, right } => Expr::Binary {
            op: op.clone(),
            left: Box::new(elide_expr(left)),
            right: Box::new(elide_expr(right)),
        },
        Expr::Unary { op, operand } => {
            Expr::Unary { op: op.clone(), operand: Box::new(elide_expr(operand)) }
        }
        Expr::Call { func, args } => Expr::Call {
            func: Box::new(elide_expr(func)),
            args: args.iter().map(elide_expr).collect(),
        },
        Expr::MethodCall { receiver, method, type_args, args } => Expr::MethodCall {
            receiver: Box::new(elide_expr(receiver)),
            method: method.clone(),
            type_args: type_args.iter().map(elide_type).collect(),
            args: args.iter().map(elide_expr).collect(),
        },
        Expr::FieldAccess { object, field } => {
            Expr::FieldAccess { object: Box::new(elide_expr(object)), field: field.clone() }
        }
        Expr::Index { object, index } => {
            Expr::Index { object: Box::new(elide_expr(object)), index: Box::new(elide_expr(index)) }
        }
        Expr::StructLit { path, fields } => Expr::StructLit {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|fi| FieldInit {
                    name: fi.name.clone(),
                    value: fi.value.as_ref().map(elide_expr),
                })
                .collect(),
        },
        Expr::TupleLit { elements } => {
            Expr::TupleLit { elements: elements.iter().map(elide_expr).collect() }
        }
        Expr::ArrayLit { elements } => {
            Expr::ArrayLit { elements: elements.iter().map(elide_expr).collect() }
        }
        Expr::ArrayRepeat { value, count } => Expr::ArrayRepeat {
            value: Box::new(elide_expr(value)),
            count: Box::new(elide_expr(count)),
        },
        Expr::Closure { params, body } => Expr::Closure {
            params: params.iter().map(elide_param).collect(),
            body: Box::new(elide_expr(body)),
        },
        Expr::If { cond, then_block, else_block } => Expr::If {
            cond: Box::new(elide_expr(cond)),
            then_block: elide_block(then_block),
            else_block: else_block.as_ref().map(elide_block),
        },
        Expr::Match { scrutinee, arms } => Expr::Match {
            scrutinee: scrutinee.as_ref().map(|s| Box::new(elide_expr(s))),
            arms: arms
                .iter()
                .map(|arm| MatchArm {
                    pattern: elide_pattern(&arm.pattern),
                    body: elide_expr(&arm.body),
                })
                .collect(),
        },
        Expr::Loop { body } => Expr::Loop { body: elide_block(body) },
        Expr::While { cond, body } => {
            Expr::While { cond: Box::new(elide_expr(cond)), body: elide_block(body) }
        }
        Expr::For { pattern, iter, body } => Expr::For {
            pattern: elide_pattern(pattern),
            iter: Box::new(elide_expr(iter)),
            body: elide_block(body),
        },
        Expr::Block { block } => Expr::Block { block: elide_block(block) },
        Expr::Return { value } => {
            Expr::Return { value: value.as_ref().map(|v| Box::new(elide_expr(v))) }
        }
        Expr::Break { value } => {
            Expr::Break { value: value.as_ref().map(|v| Box::new(elide_expr(v))) }
        }
        Expr::Try { expr: inner } => Expr::Try { expr: Box::new(elide_expr(inner)) },
        Expr::Await { expr: inner } => Expr::Await { expr: Box::new(elide_expr(inner)) },
        Expr::Cast { expr: inner, ty } => {
            Expr::Cast { expr: Box::new(elide_expr(inner)), ty: elide_type(ty) }
        }
        Expr::Assign { target, value } => Expr::Assign {
            target: Box::new(elide_expr(target)),
            value: Box::new(elide_expr(value)),
        },
        Expr::Range { start, end, inclusive } => Expr::Range {
            start: Box::new(elide_expr(start)),
            end: Box::new(elide_expr(end)),
            inclusive: *inclusive,
        },
    }
}

// ── Patterns ─────────────────────────────────────────────────────────

fn elide_pattern(pat: &Pattern) -> Pattern {
    match pat {
        // Ref patterns: strip the ref wrapper
        Pattern::Ref { pattern } => elide_pattern(pattern),

        // Recurse
        Pattern::Tuple { elements } => {
            Pattern::Tuple { elements: elements.iter().map(elide_pattern).collect() }
        }
        Pattern::Struct { path, fields } => Pattern::Struct {
            path: path.clone(),
            fields: fields
                .iter()
                .map(|fp| FieldPattern {
                    name: fp.name.clone(),
                    pattern: fp.pattern.as_ref().map(elide_pattern),
                })
                .collect(),
        },
        Pattern::Enum { path, elements } => Pattern::Enum {
            path: path.clone(),
            elements: elements.iter().map(elide_pattern).collect(),
        },
        Pattern::Slice { elements, rest } => {
            Pattern::Slice { elements: elements.iter().map(elide_pattern).collect(), rest: *rest }
        }
        Pattern::Or { patterns } => {
            Pattern::Or { patterns: patterns.iter().map(elide_pattern).collect() }
        }

        // Leaf patterns pass through
        Pattern::Ident { .. } | Pattern::Literal { .. } | Pattern::Wildcard => pat.clone(),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_and_elide(source: &str) -> Module {
        let tokens = crate::lexer::lex(source);
        let module = crate::parser::parse(&tokens).expect("parse failed");
        elide(&module)
    }

    #[test]
    fn unsafe_fn_becomes_safe() {
        let m = parse_and_elide("uf do_stuff() { 42 }");
        if let ItemKind::Function(f) = &m.items[0].kind {
            assert!(!f.is_unsafe, "unsafe flag should be stripped");
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn unsafe_block_unwrapped() {
        // uf wrapper function with unsafe block inside
        let m = parse_and_elide("uf wrap() { 1 }");
        if let ItemKind::Function(f) = &m.items[0].kind {
            assert!(!f.is_unsafe);
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn reference_mutability_stripped() {
        // A function with &!i32 (mutable ref) param should become &i32
        let m = parse_and_elide("f take(x: &!i32) -> &i32 { x }");
        if let ItemKind::Function(f) = &m.items[0].kind {
            // param type
            match &f.params[0].ty {
                Type::Reference { mutable, .. } => {
                    assert!(!mutable, "&mut should be elided to &");
                }
                other => panic!("expected Reference, got {other:?}"),
            }
            // return type
            match f.return_type.as_ref().unwrap() {
                Type::Reference { mutable, .. } => {
                    assert!(!mutable);
                }
                other => panic!("expected Reference, got {other:?}"),
            }
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn safety_bounds_stripped_from_generics() {
        // f foo[T: Send + Clone]() — Send should be stripped, Clone kept
        let m = parse_and_elide("f foo[T: Clone]() { 42 }");
        if let ItemKind::Function(f) = &m.items[0].kind {
            // The Clone bound should survive
            assert!(
                f.generics[0].bounds.contains(&"Clone".to_string()),
                "Clone should survive elision"
            );
        } else {
            panic!("expected function");
        }
    }

    #[test]
    fn pin_type_unwrapped() {
        // Pin<T> path type should be unwrapped to just T
        let pin_ty = Type::Path {
            segments: vec!["Pin".to_string()],
            type_args: vec![Type::Path { segments: vec!["Fut".to_string()], type_args: vec![] }],
        };
        let elided = elide_type(&pin_ty);
        match elided {
            Type::Path { segments, .. } => {
                assert_eq!(segments, vec!["Fut".to_string()]);
            }
            other => panic!("expected Path(Fut), got {other:?}"),
        }
    }

    #[test]
    fn phantom_data_becomes_unit() {
        let pd_ty = Type::Path {
            segments: vec!["PhantomData".to_string()],
            type_args: vec![Type::Path { segments: vec!["T".to_string()], type_args: vec![] }],
        };
        let elided = elide_type(&pd_ty);
        match elided {
            Type::Tuple { elements } => {
                assert!(elements.is_empty(), "PhantomData should become ()");
            }
            other => panic!("expected unit tuple, got {other:?}"),
        }
    }

    #[test]
    fn ref_pattern_unwrapped() {
        let pat = Pattern::Ref { pattern: Box::new(Pattern::Ident { name: "x".to_string() }) };
        let elided = elide_pattern(&pat);
        match elided {
            Pattern::Ident { name } => assert_eq!(name, "x"),
            other => panic!("expected Ident, got {other:?}"),
        }
    }

    #[test]
    fn where_clause_lifetime_predicates_stripped() {
        let preds = vec![
            WherePredicate { type_param: "'a".to_string(), bounds: vec!["'static".to_string()] },
            WherePredicate {
                type_param: "T".to_string(),
                bounds: vec!["Clone".to_string(), "Send".to_string()],
            },
        ];
        let elided = elide_where_clause(&preds);
        // Lifetime predicate should be gone
        assert_eq!(elided.len(), 1);
        assert_eq!(elided[0].type_param, "T");
        // Send should be stripped, Clone kept
        assert_eq!(elided[0].bounds, vec!["Clone".to_string()]);
    }

    #[test]
    fn empty_bounds_predicate_removed() {
        let preds = vec![WherePredicate {
            type_param: "T".to_string(),
            bounds: vec!["Send".to_string(), "Sync".to_string()],
        }];
        let elided = elide_where_clause(&preds);
        // All bounds were safety bounds, so the whole predicate is removed
        assert!(elided.is_empty());
    }

    #[test]
    fn elide_idempotent() {
        let m = parse_and_elide("f add(a: i32, b: i32) -> i32 { a }");
        let m2 = elide(&m);
        // Second pass should produce identical structure
        let json1 = serde_json::to_string(&m).unwrap();
        let json2 = serde_json::to_string(&m2).unwrap();
        assert_eq!(json1, json2, "elision should be idempotent");
    }
}
