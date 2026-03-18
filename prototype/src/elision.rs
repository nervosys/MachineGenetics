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
        ItemKind::Agent(a) => ItemKind::Agent(a.clone()),
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
        contracts: f.contracts.clone(),
        body: elide_block(&f.body),
    }
}

fn elide_struct(s: &StructDef) -> StructDef {
    StructDef {
        name: s.name.clone(),
        generics: elide_generics(&s.generics),
        contracts: s.contracts.clone(),
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
        refinement: ta.refinement.clone(),
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

        // Refinement types: recurse into base type, preserve predicate
        Type::Refined { base, predicate } => Type::Refined {
            base: Box::new(elide_type(base)),
            predicate: predicate.clone(),
        },
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

// ── Attribute Compression System (Step 35) ──────────────────────────
//
// Maps compressed Redox attribute shorthands (`@d`, `@r`, …) to their
// full Rust equivalents (`derive`, `repr`, …).

/// Expand a compressed Redox attribute name to its full Rust equivalent.
/// Returns `None` if the name is already full-form or unknown.
pub fn expand_attribute_name(name: &str) -> Option<&'static str> {
    match name {
        "d"   => Some("derive"),
        "r"   => Some("repr"),
        "mu"  => Some("must_use"),
        "a"   => Some("allow"),
        "x"   => Some("deny"),
        "cfg" => Some("cfg"),   // already full, included for completeness
        "t"   => Some("test"),
        "b"   => Some("bench"),
        "se"  => Some("serde"),
        "pi"  => Some("proc_macro"),
        "pnb" => Some("non_blocking"),
        "pv"  => Some("visibility"),
        "pt"  => Some("target_feature"),
        "pa"  => Some("align"),
        "pp"  => Some("packed"),
        "at"  => Some("async_trait"),
        "co"  => Some("cold"),
        "il"  => Some("inline"),
        "ila" => Some("inline_always"),
        "na"  => Some("no_alloc"),
        "nm"  => Some("no_mangle"),
        "dp"  => Some("deprecated"),
        "dc"  => Some("doc"),
        "gl"  => Some("global_allocator"),
        // Agent discovery attributes
        "as"  => Some("agent_skill"),
        "ac"  => Some("agent_capability"),
        "ax"  => Some("agent_export"),
        "ao"  => Some("agent_observable"),
        "ae"  => Some("agent_effect"),
        _     => None,
    }
}

/// Expand a single compressed attribute to its full-form equivalent.
pub fn expand_attribute(attr: &Attribute) -> Attribute {
    let expanded_name = expand_attribute_name(&attr.name)
        .map(|s| s.to_string())
        .unwrap_or_else(|| attr.name.clone());
    Attribute {
        name: expanded_name,
        args: attr.args.clone(),
        bang: attr.bang,
    }
}

/// Expand all compressed attributes in an item.
pub fn expand_attributes(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs.iter().map(expand_attribute).collect()
}

/// Return the compressed form for a Rust attribute name, if one exists.
pub fn compress_attribute_name(rust_name: &str) -> Option<&'static str> {
    match rust_name {
        "derive"           => Some("d"),
        "repr"             => Some("r"),
        "must_use"         => Some("mu"),
        "allow"            => Some("a"),
        "deny"             => Some("x"),
        "cfg"              => Some("cfg"),
        "test"             => Some("t"),
        "bench"            => Some("b"),
        "serde"            => Some("se"),
        "proc_macro"       => Some("pi"),
        "non_blocking"     => Some("pnb"),
        "visibility"       => Some("pv"),
        "target_feature"   => Some("pt"),
        "align"            => Some("pa"),
        "packed"           => Some("pp"),
        "async_trait"        => Some("at"),
        "cold"               => Some("co"),
        "inline"             => Some("il"),
        "inline_always"      => Some("ila"),
        "no_alloc"           => Some("na"),
        "no_mangle"          => Some("nm"),
        "deprecated"         => Some("dp"),
        "doc"                => Some("dc"),
        "global_allocator"   => Some("gl"),
        // Agent discovery attributes
        "agent_skill"        => Some("as"),
        "agent_capability"   => Some("ac"),
        "agent_export"       => Some("ax"),
        "agent_observable"   => Some("ao"),
        "agent_effect"       => Some("ae"),
        _                    => None,
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

    // ── Attribute compression tests (Step 35) ──────────────

    #[test]
    fn expand_derive_shorthand() {
        assert_eq!(expand_attribute_name("d"), Some("derive"));
    }

    #[test]
    fn expand_repr_shorthand() {
        assert_eq!(expand_attribute_name("r"), Some("repr"));
    }

    #[test]
    fn expand_must_use_shorthand() {
        assert_eq!(expand_attribute_name("mu"), Some("must_use"));
    }

    #[test]
    fn expand_test_shorthand() {
        assert_eq!(expand_attribute_name("t"), Some("test"));
    }

    #[test]
    fn expand_inline_shorthand() {
        assert_eq!(expand_attribute_name("il"), Some("inline"));
    }

    #[test]
    fn expand_unknown_returns_none() {
        assert_eq!(expand_attribute_name("zzz"), None);
    }

    #[test]
    fn compress_derive_roundtrip() {
        assert_eq!(compress_attribute_name("derive"), Some("d"));
    }

    #[test]
    fn compress_must_use_roundtrip() {
        assert_eq!(compress_attribute_name("must_use"), Some("mu"));
    }

    #[test]
    fn expand_attribute_struct() {
        let attr = Attribute { name: "d".into(), args: vec!["Eq".into(), "Hash".into()], bang: false };
        let expanded = expand_attribute(&attr);
        assert_eq!(expanded.name, "derive");
        assert_eq!(expanded.args, vec!["Eq", "Hash"]);
        assert!(!expanded.bang);
    }

    #[test]
    fn expand_attribute_with_bang() {
        let attr = Attribute { name: "pi".into(), args: vec![], bang: true };
        let expanded = expand_attribute(&attr);
        assert_eq!(expanded.name, "proc_macro");
        assert!(expanded.bang);
    }

    #[test]
    fn expand_all_known_attributes() {
        let known = vec![
            ("d", "derive"), ("r", "repr"), ("mu", "must_use"),
            ("a", "allow"), ("x", "deny"), ("cfg", "cfg"),
            ("t", "test"), ("b", "bench"), ("se", "serde"),
            ("pi", "proc_macro"), ("pnb", "non_blocking"),
            ("pv", "visibility"), ("pt", "target_feature"),
            ("pa", "align"), ("pp", "packed"), ("at", "async_trait"),
            ("co", "cold"), ("il", "inline"), ("ila", "inline_always"),
            ("na", "no_alloc"), ("nm", "no_mangle"),
            ("dp", "deprecated"), ("dc", "doc"), ("gl", "global_allocator"),
            // Agent discovery attributes
            ("as", "agent_skill"), ("ac", "agent_capability"),
            ("ax", "agent_export"), ("ao", "agent_observable"),
            ("ae", "agent_effect"),
        ];
        for (short, full) in &known {
            assert_eq!(expand_attribute_name(short), Some(*full), "expand({short}) should be {full}");
            assert_eq!(compress_attribute_name(full), Some(*short), "compress({full}) should be {short}");
        }
    }

    // ── Agent Discovery Attribute Tests ──────────────────────────────

    #[test]
    fn expand_agent_skill() {
        assert_eq!(expand_attribute_name("as"), Some("agent_skill"));
    }

    #[test]
    fn expand_agent_capability() {
        assert_eq!(expand_attribute_name("ac"), Some("agent_capability"));
    }

    #[test]
    fn expand_agent_export() {
        assert_eq!(expand_attribute_name("ax"), Some("agent_export"));
    }

    #[test]
    fn expand_agent_observable() {
        assert_eq!(expand_attribute_name("ao"), Some("agent_observable"));
    }

    #[test]
    fn expand_agent_effect() {
        assert_eq!(expand_attribute_name("ae"), Some("agent_effect"));
    }

    #[test]
    fn compress_agent_skill() {
        assert_eq!(compress_attribute_name("agent_skill"), Some("as"));
    }

    #[test]
    fn compress_agent_capability() {
        assert_eq!(compress_attribute_name("agent_capability"), Some("ac"));
    }

    #[test]
    fn compress_agent_export() {
        assert_eq!(compress_attribute_name("agent_export"), Some("ax"));
    }

    #[test]
    fn compress_agent_observable() {
        assert_eq!(compress_attribute_name("agent_observable"), Some("ao"));
    }

    #[test]
    fn compress_agent_effect() {
        assert_eq!(compress_attribute_name("agent_effect"), Some("ae"));
    }

    #[test]
    fn parse_agent_skill_attribute() {
        let m = parse_and_elide("@as(code_review)\nf review() {}");
        let attr = &m.items[0].attributes[0];
        assert_eq!(attr.name, "as");
        assert_eq!(attr.args, vec!["code_review"]);
    }

    #[test]
    fn parse_agent_capability_attribute() {
        let m = parse_and_elide("@ac(read_source, write_source)\nf edit() {}");
        let attr = &m.items[0].attributes[0];
        assert_eq!(attr.name, "ac");
        assert_eq!(attr.args, vec!["read_source", "write_source"]);
    }

    #[test]
    fn parse_agent_export_attribute() {
        let m = parse_and_elide("@ax(tool_api)\nf analyze() {}");
        let attr = &m.items[0].attributes[0];
        assert_eq!(attr.name, "ax");
        assert_eq!(attr.args, vec!["tool_api"]);
    }

    #[test]
    fn parse_agent_observable_attribute() {
        let m = parse_and_elide("@ao(metrics)\nf compute() {}");
        let attr = &m.items[0].attributes[0];
        assert_eq!(attr.name, "ao");
        assert_eq!(attr.args, vec!["metrics"]);
    }

    #[test]
    fn parse_agent_effect_attribute() {
        let m = parse_and_elide("@ae(io, network)\nf fetch() {}");
        let attr = &m.items[0].attributes[0];
        assert_eq!(attr.name, "ae");
        assert_eq!(attr.args, vec!["io", "network"]);
    }
}
