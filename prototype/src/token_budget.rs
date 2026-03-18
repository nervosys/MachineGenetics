/// Redox Token Budget Reporting
///
/// Implements `--token-report` (proposal §5.5 P29): per-function and
/// per-module token counts in both compact (Redox) and expanded (Rust-
/// equivalent) forms.  Agents use this to track and optimise their
/// token expenditure.
use crate::ast::*;
use serde::{Deserialize, Serialize};

// ── Data types ───────────────────────────────────────────────────────

/// Token metrics for a single function or module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMetrics {
    /// Name of the item ("f add", "M server", etc.).
    pub name: String,
    /// Kind of item.
    pub kind: ItemMetricKind,
    /// Token count in compact Redox syntax.
    pub compact_tokens: u32,
    /// Estimated token count in expanded (Rust) syntax.
    pub expanded_tokens: u32,
    /// Compression ratio: compact / expanded (lower = better).
    pub ratio: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemMetricKind {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    Other,
}

/// Full token report for a module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenReport {
    /// Per-item metrics.
    pub items: Vec<TokenMetrics>,
    /// Total compact tokens.
    pub total_compact: u32,
    /// Total expanded tokens.
    pub total_expanded: u32,
    /// Overall compression ratio.
    pub overall_ratio: f64,
}

// ── Public API ───────────────────────────────────────────────────────

/// Generate a token budget report for a parsed module.
pub fn report(module: &Module) -> TokenReport {
    let items: Vec<TokenMetrics> = module.items.iter().map(count_item).collect();
    let total_compact: u32 = items.iter().map(|m| m.compact_tokens).sum();
    let total_expanded: u32 = items.iter().map(|m| m.expanded_tokens).sum();
    let overall_ratio =
        if total_expanded > 0 { total_compact as f64 / total_expanded as f64 } else { 1.0 };
    TokenReport { items, total_compact, total_expanded, overall_ratio }
}

/// Count tokens for a single item.
fn count_item(item: &Item) -> TokenMetrics {
    match &item.kind {
        ItemKind::Function(f) => {
            let compact = count_function_compact(f, item.visibility);
            let expanded = count_function_expanded(f, item.visibility);
            let ratio = if expanded > 0 { compact as f64 / expanded as f64 } else { 1.0 };
            TokenMetrics {
                name: f.name.clone(),
                kind: ItemMetricKind::Function,
                compact_tokens: compact,
                expanded_tokens: expanded,
                ratio,
            }
        }
        ItemKind::Struct(s) => {
            let compact = count_struct_compact(s, item.visibility);
            let expanded = count_struct_expanded(s, item.visibility);
            let ratio = if expanded > 0 { compact as f64 / expanded as f64 } else { 1.0 };
            TokenMetrics {
                name: s.name.clone(),
                kind: ItemMetricKind::Struct,
                compact_tokens: compact,
                expanded_tokens: expanded,
                ratio,
            }
        }
        ItemKind::Enum(e) => {
            let compact = count_enum_compact(e, item.visibility);
            let expanded = count_enum_expanded(e, item.visibility);
            let ratio = if expanded > 0 { compact as f64 / expanded as f64 } else { 1.0 };
            TokenMetrics {
                name: e.name.clone(),
                kind: ItemMetricKind::Enum,
                compact_tokens: compact,
                expanded_tokens: expanded,
                ratio,
            }
        }
        ItemKind::Trait(t) => {
            let compact = count_trait_compact(t, item.visibility);
            let expanded = count_trait_expanded(t, item.visibility);
            let ratio = if expanded > 0 { compact as f64 / expanded as f64 } else { 1.0 };
            TokenMetrics {
                name: t.name.clone(),
                kind: ItemMetricKind::Trait,
                compact_tokens: compact,
                expanded_tokens: expanded,
                ratio,
            }
        }
        ItemKind::Impl(i) => {
            let compact = count_impl_compact(i);
            let expanded = count_impl_expanded(i);
            let ratio = if expanded > 0 { compact as f64 / expanded as f64 } else { 1.0 };
            let name = format!("impl {}", type_name(&i.self_type));
            TokenMetrics {
                name,
                kind: ItemMetricKind::Impl,
                compact_tokens: compact,
                expanded_tokens: expanded,
                ratio,
            }
        }
        ItemKind::Module(m) => {
            let compact = count_module_compact(m, item.visibility);
            let expanded = count_module_expanded(m, item.visibility);
            let ratio = if expanded > 0 { compact as f64 / expanded as f64 } else { 1.0 };
            TokenMetrics {
                name: m.name.clone(),
                kind: ItemMetricKind::Module,
                compact_tokens: compact,
                expanded_tokens: expanded,
                ratio,
            }
        }
        _ => {
            // Use/TypeAlias/Const/Static/Effect/Spec: estimate generically
            let compact = 3; // keyword + name + semicolon (rough)
            let expanded = 5;
            TokenMetrics {
                name: item_name(&item.kind),
                kind: ItemMetricKind::Other,
                compact_tokens: compact,
                expanded_tokens: expanded,
                ratio: compact as f64 / expanded as f64,
            }
        }
    }
}

// ── Compact (Redox) token counting ───────────────────────────────────

fn count_function_compact(f: &FunctionDef, vis: Visibility) -> u32 {
    let mut n: u32 = 0;
    // Keyword: +f / f / +af / af / +uf / uf
    n += 1; // function keyword
    if vis == Visibility::Public {
        n += 1; // + prefix counts as a token
    }
    if f.is_async {
        n += 1;
    }
    n += 1; // name
    // Generics
    n += count_generics_compact(&f.generics);
    // Params
    n += 1; // (
    for p in &f.params {
        n += 1; // name
        n += 1; // :
        n += count_type_compact(&p.ty);
        n += 1; // , (or close)
    }
    n += 1; // )
    // Return type
    if let Some(ret) = &f.return_type {
        n += 1; // ->
        n += count_type_compact(ret);
    }
    // Where clause
    n += count_where_compact(&f.where_clause);
    // Effects
    n += f.effects.len() as u32;
    // Body
    n += count_block_compact(&f.body);
    n
}

fn count_struct_compact(s: &StructDef, vis: Visibility) -> u32 {
    let mut n: u32 = 1; // S keyword
    if vis == Visibility::Public {
        n += 1;
    }
    n += 1; // name
    n += count_generics_compact(&s.generics);
    n += 1; // {
    for field in &s.fields {
        if field.visibility == Visibility::Public {
            n += 1;
        }
        n += 1; // name
        n += 1; // :
        n += count_type_compact(&field.ty);
        n += 1; // , or }
    }
    n += 1; // }
    n
}

fn count_enum_compact(e: &EnumDef, vis: Visibility) -> u32 {
    let mut n: u32 = 1; // E keyword
    if vis == Visibility::Public {
        n += 1;
    }
    n += 1; // name
    n += count_generics_compact(&e.generics);
    n += 1; // {
    for v in &e.variants {
        n += 1; // variant name
        match &v.kind {
            VariantKind::Unit => {}
            VariantKind::Tuple(types) => {
                n += 1; // (
                for t in types {
                    n += count_type_compact(t);
                    n += 1; // ,
                }
                n += 1; // )
            }
            VariantKind::Struct(fields) => {
                n += 1; // {
                for f in fields {
                    n += 2; // name :
                    n += count_type_compact(&f.ty);
                    n += 1; // ,
                }
                n += 1; // }
            }
        }
        n += 1; // , or }
    }
    n += 1; // }
    n
}

fn count_trait_compact(t: &TraitDef, vis: Visibility) -> u32 {
    let mut n: u32 = 1; // T keyword
    if vis == Visibility::Public {
        n += 1;
    }
    n += 1; // name
    n += count_generics_compact(&t.generics);
    if !t.super_traits.is_empty() {
        n += 1; // :
        n += t.super_traits.len() as u32;
    }
    n += 1; // {
    for item in &t.items {
        n += count_item(item).compact_tokens;
    }
    n += 1; // }
    n
}

fn count_impl_compact(i: &ImplBlock) -> u32 {
    let mut n: u32 = 1; // I keyword
    n += count_generics_compact(&i.generics);
    n += count_type_compact(&i.self_type);
    if let Some(trait_path) = &i.trait_path {
        n += trait_path.len() as u32;
    }
    n += 1; // {
    for item in &i.items {
        n += count_item(item).compact_tokens;
    }
    n += 1; // }
    n
}

fn count_module_compact(m: &ModuleDef, vis: Visibility) -> u32 {
    let mut n: u32 = 1; // M keyword
    if vis == Visibility::Public {
        n += 1;
    }
    n += 1; // name
    if let Some(items) = &m.items {
        n += 1; // {
        for item in items {
            n += count_item(item).compact_tokens;
        }
        n += 1; // }
    } else {
        n += 1; // ;
    }
    n
}

fn count_generics_compact(generics: &[GenericParam]) -> u32 {
    if generics.is_empty() {
        return 0;
    }
    let mut n: u32 = 1; // [
    for g in generics {
        n += 1; // name
        n += g.bounds.len() as u32; // bounds
        if g.default.is_some() {
            n += 1; // =
            n += 1; // default type
        }
        n += 1; // , or ]
    }
    n += 1; // ]
    n
}

fn count_where_compact(preds: &[WherePredicate]) -> u32 {
    if preds.is_empty() {
        return 0;
    }
    let mut n: u32 = 1; // ~>
    for p in preds {
        n += 1; // type_param
        n += 1; // :
        n += p.bounds.len() as u32;
        n += 1; // ,
    }
    n
}

fn count_type_compact(ty: &Type) -> u32 {
    match ty {
        Type::Path { segments, type_args } => {
            let mut n = segments.len() as u32;
            if !type_args.is_empty() {
                n += 1; // [
                for ta in type_args {
                    n += count_type_compact(ta);
                    n += 1; // , or ]
                }
                n += 1; // ]
            }
            n
        }
        Type::Reference { inner, .. } => 1 + count_type_compact(inner), // & or &!
        Type::OwnedPtr { inner } => 1 + count_type_compact(inner),      // ^
        Type::Rc { inner } => 1 + count_type_compact(inner),            // $
        Type::Arc { inner } => 1 + count_type_compact(inner),           // @
        Type::Cow { inner } => 1 + count_type_compact(inner),           // &~
        Type::Cell { inner } => 1 + count_type_compact(inner),          // %
        Type::RefCell { inner } => 1 + count_type_compact(inner),       // %!
        Type::Mutex { inner } => 1 + count_type_compact(inner),         // #
        Type::RwLock { inner } => 1 + count_type_compact(inner),        // #~
        Type::Slice { inner } => 2 + count_type_compact(inner),         // [ T ]
        Type::Array { inner, .. } => 3 + count_type_compact(inner),     // [ T ; N ]
        Type::Vec { inner } => 2 + count_type_compact(inner),           // [T]~
        Type::Set { inner } => 2 + count_type_compact(inner),           // {T}
        Type::Tuple { elements } => {
            let mut n: u32 = 2; // ( )
            for e in elements {
                n += count_type_compact(e);
                n += 1; // ,
            }
            n
        }
        Type::Option { inner } => 1 + count_type_compact(inner), // ?T
        Type::Result { ok, err } => {
            2 + count_type_compact(ok) + count_type_compact(err) // R[ , ]
        }
        Type::Map { key, value } => {
            2 + count_type_compact(key) + count_type_compact(value) // { K: V }
        }
        Type::Ptr { inner } => 2 + count_type_compact(inner), // Ptr[T]
        Type::Simd { inner, .. } => 3 + count_type_compact(inner), // Simd[T, N]
        Type::Fn { params, ret } => {
            let mut n: u32 = 2; // f( )
            for p in params {
                n += count_type_compact(p);
                n += 1;
            }
            if let Some(r) = ret {
                n += 1; // ->
                n += count_type_compact(r);
            }
            n
        }
        Type::Never | Type::Inferred | Type::SelfType | Type::StringType => 1,
        Type::Refined { base, .. } => count_type_compact(base) + 4,
    }
}

fn count_block_compact(block: &Block) -> u32 {
    let mut n: u32 = 2; // { }
    for stmt in &block.stmts {
        n += count_stmt_compact(stmt);
    }
    if let Some(tail) = &block.tail_expr {
        n += count_expr_compact(tail);
    }
    n
}

fn count_stmt_compact(stmt: &Stmt) -> u32 {
    match stmt {
        Stmt::Let { mutable, ty, value, .. } => {
            let mut n: u32 = 1; // v or m
            if *mutable { /* m already counted */ }
            n += 1; // pattern name
            if ty.is_some() {
                n += 1; // :
                n += 1; // type (rough)
            }
            n += 1; // =
            n += count_expr_compact(value);
            n += 1; // ;
            n
        }
        Stmt::Expr { expr } => count_expr_compact(expr) + 1, // expr ;
        Stmt::Item { item } => count_item(item).compact_tokens,
    }
}

fn count_expr_compact(expr: &Expr) -> u32 {
    match expr {
        Expr::Literal { .. } => 1,
        Expr::Ident { .. } => 1,
        Expr::Binary { left, right, .. } => {
            1 + count_expr_compact(left) + count_expr_compact(right) // op
        }
        Expr::Unary { operand, .. } => 1 + count_expr_compact(operand),
        Expr::Call { func, args } => {
            let mut n = count_expr_compact(func) + 2; // ( )
            for a in args {
                n += count_expr_compact(a);
                n += 1; // ,
            }
            n
        }
        Expr::MethodCall { receiver, args, .. } => {
            let mut n = count_expr_compact(receiver) + 1 + 2; // .method( )
            for a in args {
                n += count_expr_compact(a);
                n += 1;
            }
            n
        }
        Expr::FieldAccess { object, .. } => count_expr_compact(object) + 2, // .field
        Expr::Index { object, index } => {
            count_expr_compact(object) + 2 + count_expr_compact(index) // [idx]
        }
        Expr::StructLit { fields, .. } => {
            let mut n: u32 = 2; // Name { }
            for f in fields {
                n += 1; // name
                if f.value.is_some() {
                    n += 1; // :
                    n += 1; // value (rough)
                }
                n += 1; // ,
            }
            n
        }
        Expr::TupleLit { elements } => {
            let mut n: u32 = 2; // ( )
            for e in elements {
                n += count_expr_compact(e);
                n += 1;
            }
            n
        }
        Expr::ArrayLit { elements } => {
            let mut n: u32 = 2; // [ ]
            for e in elements {
                n += count_expr_compact(e);
                n += 1;
            }
            n
        }
        Expr::ArrayRepeat { value, count } => {
            3 + count_expr_compact(value) + count_expr_compact(count) // [v; n]
        }
        Expr::Closure { params, body } => {
            let mut n: u32 = 2; // | |
            n += params.len() as u32 * 2; // name: type per param (rough)
            n += count_expr_compact(body);
            n
        }
        Expr::If { cond, then_block, else_block } => {
            let mut n = 1 + count_expr_compact(cond) + count_block_compact(then_block); // ?
            if let Some(eb) = else_block {
                n += 1 + count_block_compact(eb); // :
            }
            n
        }
        Expr::Match { scrutinee, arms } => {
            let mut n: u32 = 1; // ?=
            if let Some(s) = scrutinee {
                n += count_expr_compact(s);
            }
            n += 1; // {
            for arm in arms {
                n += 1; // pattern (rough)
                n += 1; // =>
                n += count_expr_compact(&arm.body);
                n += 1; // ,
            }
            n += 1; // }
            n
        }
        Expr::Loop { body } => 1 + count_block_compact(body), // @@
        Expr::While { cond, body } => {
            1 + count_expr_compact(cond) + count_block_compact(body) // @w
        }
        Expr::For { iter, body, .. } => {
            2 + count_expr_compact(iter) + count_block_compact(body) // @ pat in
        }
        Expr::Block { block } => count_block_compact(block),
        Expr::Return { value } => {
            let mut n: u32 = 1; // ret
            if let Some(v) = value {
                n += count_expr_compact(v);
            }
            n
        }
        Expr::Break { value } => {
            let mut n: u32 = 1; // !
            if let Some(v) = value {
                n += count_expr_compact(v);
            }
            n
        }
        Expr::Continue => 1,                                     // >>
        Expr::Try { expr } => count_expr_compact(expr) + 1,      // ~
        Expr::Await { expr } => count_expr_compact(expr) + 1,    // .await
        Expr::Cast { expr, .. } => count_expr_compact(expr) + 2, // as T
        Expr::Assign { target, value } => {
            count_expr_compact(target) + 1 + count_expr_compact(value) // =
        }
        Expr::Range { start, end, .. } => {
            count_expr_compact(start) + 1 + count_expr_compact(end) // ..
        }
        Expr::Todo => 1,          // ??
        Expr::Unimplemented => 1, // ???
        Expr::UnsafeBlock { block } => 1 + count_block_compact(block),
        Expr::Error { .. } => 1,
    }
}

// ── Expanded (Rust-equivalent) token counting ─────────────────────────
//
// We estimate the Rust token count by applying the known expansion ratios:
// - Compact keywords are 1 token each in Redox but expand to 1-2 in Rust
//   (`+f` → `pub fn` = 2, `v` → `let` = 1, `m` → `let mut` = 2, etc.)
// - Type wrappers like `?T` → `Option<T>` are +2 tokens, `^T` → `Box<T>` +2, etc.

fn count_function_expanded(f: &FunctionDef, vis: Visibility) -> u32 {
    let mut n: u32 = 0;
    if vis == Visibility::Public {
        n += 1; // pub
    }
    if f.is_async {
        n += 1; // async
    }
    if f.is_unsafe {
        n += 1; // unsafe
    }
    n += 1; // fn
    n += 1; // name
    n += count_generics_expanded(&f.generics);
    n += 1; // (
    for p in &f.params {
        n += 1; // name
        n += 1; // :
        n += count_type_expanded(&p.ty);
        n += 1; // ,
    }
    n += 1; // )
    if let Some(ret) = &f.return_type {
        n += 1; // ->
        n += count_type_expanded(ret);
    }
    n += count_where_expanded(&f.where_clause);
    // Effects don't exist in Rust, so no expanded equivalent (add as comment?)
    n += count_block_expanded(&f.body);
    n
}

fn count_struct_expanded(s: &StructDef, vis: Visibility) -> u32 {
    let mut n: u32 = 0;
    if vis == Visibility::Public {
        n += 1;
    }
    n += 1; // struct
    n += 1; // name
    n += count_generics_expanded(&s.generics);
    n += 1; // {
    for field in &s.fields {
        if field.visibility == Visibility::Public {
            n += 1; // pub
        }
        n += 1; // name
        n += 1; // :
        n += count_type_expanded(&field.ty);
        n += 1; // ,
    }
    n += 1; // }
    n
}

fn count_enum_expanded(e: &EnumDef, vis: Visibility) -> u32 {
    let mut n: u32 = 0;
    if vis == Visibility::Public {
        n += 1;
    }
    n += 1; // enum
    n += 1; // name
    n += count_generics_expanded(&e.generics);
    n += 1; // {
    for v in &e.variants {
        n += 1; // variant name
        match &v.kind {
            VariantKind::Unit => {}
            VariantKind::Tuple(types) => {
                n += 1;
                for t in types {
                    n += count_type_expanded(t);
                    n += 1;
                }
                n += 1;
            }
            VariantKind::Struct(fields) => {
                n += 1;
                for f in fields {
                    n += 2;
                    n += count_type_expanded(&f.ty);
                    n += 1;
                }
                n += 1;
            }
        }
        n += 1; // ,
    }
    n += 1; // }
    n
}

fn count_trait_expanded(t: &TraitDef, vis: Visibility) -> u32 {
    let mut n: u32 = 0;
    if vis == Visibility::Public {
        n += 1;
    }
    n += 1; // trait
    n += 1; // name
    n += count_generics_expanded(&t.generics);
    if !t.super_traits.is_empty() {
        n += 1; // :
        n += t.super_traits.len() as u32;
        n += (t.super_traits.len().saturating_sub(1)) as u32; // + separators
    }
    n += 1; // {
    for item in &t.items {
        n += count_item(item).expanded_tokens;
    }
    n += 1; // }
    n
}

fn count_impl_expanded(i: &ImplBlock) -> u32 {
    let mut n: u32 = 1; // impl
    n += count_generics_expanded(&i.generics);
    if let Some(trait_path) = &i.trait_path {
        n += trait_path.len() as u32;
        n += 1; // for
    }
    n += count_type_expanded(&i.self_type);
    n += 1; // {
    for item in &i.items {
        n += count_item(item).expanded_tokens;
    }
    n += 1; // }
    n
}

fn count_module_expanded(m: &ModuleDef, vis: Visibility) -> u32 {
    let mut n: u32 = 0;
    if vis == Visibility::Public {
        n += 1;
    }
    n += 1; // mod
    n += 1; // name
    if let Some(items) = &m.items {
        n += 1;
        for item in items {
            n += count_item(item).expanded_tokens;
        }
        n += 1;
    } else {
        n += 1;
    }
    n
}

fn count_generics_expanded(generics: &[GenericParam]) -> u32 {
    if generics.is_empty() {
        return 0;
    }
    let mut n: u32 = 1; // <
    for g in generics {
        n += 1; // name
        if !g.bounds.is_empty() {
            n += 1; // :
            n += g.bounds.len() as u32;
            n += (g.bounds.len().saturating_sub(1)) as u32; // + separators
        }
        n += 1; // ,
    }
    n += 1; // >
    n
}

fn count_where_expanded(preds: &[WherePredicate]) -> u32 {
    if preds.is_empty() {
        return 0;
    }
    let mut n: u32 = 1; // where
    for p in preds {
        n += 1; // type
        n += 1; // :
        n += p.bounds.len() as u32;
        n += (p.bounds.len().saturating_sub(1)) as u32; // +
        n += 1; // ,
    }
    n
}

fn count_type_expanded(ty: &Type) -> u32 {
    match ty {
        Type::Path { segments, type_args } => {
            // In Rust: path::segments<T1, T2>
            let mut n = segments.len() as u32;
            n += (segments.len().saturating_sub(1)) as u32; // :: separators
            if !type_args.is_empty() {
                n += 1; // <
                for ta in type_args {
                    n += count_type_expanded(ta);
                    n += 1; // ,
                }
                n += 1; // >
            }
            n
        }
        Type::Reference { mutable, inner } => {
            let mut n: u32 = 1; // &
            if *mutable {
                n += 1; // mut
            }
            n + count_type_expanded(inner)
        }
        // Box<T>, Rc<T>, Arc<T> in Rust = Name + < + T + > = 3 + inner
        Type::OwnedPtr { inner } => 3 + count_type_expanded(inner),
        Type::Rc { inner } => 3 + count_type_expanded(inner),
        Type::Arc { inner } => 3 + count_type_expanded(inner),
        Type::Cow { inner } => 3 + count_type_expanded(inner),
        Type::Cell { inner } => 3 + count_type_expanded(inner),
        Type::RefCell { inner } => 3 + count_type_expanded(inner),
        Type::Mutex { inner } => 3 + count_type_expanded(inner),
        Type::RwLock { inner } => 3 + count_type_expanded(inner),
        // Vec<T> in Rust = 3 + inner
        Type::Vec { inner } => 3 + count_type_expanded(inner),
        Type::Set { inner } => 5 + count_type_expanded(inner), // HashSet<T>
        Type::Slice { inner } => 2 + count_type_expanded(inner),
        Type::Array { inner, .. } => 4 + count_type_expanded(inner),
        Type::Tuple { elements } => {
            let mut n: u32 = 2;
            for e in elements {
                n += count_type_expanded(e);
                n += 1;
            }
            n
        }
        // Option<T> in Rust = 3 + inner
        Type::Option { inner } => 3 + count_type_expanded(inner),
        // Result<T, E> in Rust = 4 + ok + err
        Type::Result { ok, err } => 4 + count_type_expanded(ok) + count_type_expanded(err),
        // HashMap<K, V> in Rust = 4 + key + value
        Type::Map { key, value } => 4 + count_type_expanded(key) + count_type_expanded(value),
        Type::Ptr { inner } => 3 + count_type_expanded(inner),
        Type::Simd { inner, .. } => 5 + count_type_expanded(inner),
        Type::Fn { params, ret } => {
            let mut n: u32 = 3; // fn ( )
            for p in params {
                n += count_type_expanded(p);
                n += 1;
            }
            if let Some(r) = ret {
                n += 1;
                n += count_type_expanded(r);
            }
            n
        }
        Type::Never | Type::Inferred | Type::SelfType | Type::StringType => 1,
        Type::Refined { base, .. } => count_type_expanded(base) + 6,
    }
}

fn count_block_expanded(block: &Block) -> u32 {
    let mut n: u32 = 2; // { }
    for stmt in &block.stmts {
        n += count_stmt_expanded(stmt);
    }
    if let Some(tail) = &block.tail_expr {
        n += count_expr_expanded(tail);
    }
    n
}

fn count_stmt_expanded(stmt: &Stmt) -> u32 {
    match stmt {
        Stmt::Let { mutable, ty, value, .. } => {
            let mut n: u32 = 1; // let
            if *mutable {
                n += 1; // mut
            }
            n += 1; // pattern
            if ty.is_some() {
                n += 1; // :
                n += 1; // type
            }
            n += 1; // =
            n += count_expr_expanded(value);
            n += 1; // ;
            n
        }
        Stmt::Expr { expr } => count_expr_expanded(expr) + 1,
        Stmt::Item { item } => count_item(item).expanded_tokens,
    }
}

fn count_expr_expanded(expr: &Expr) -> u32 {
    match expr {
        Expr::Literal { .. } => 1,
        Expr::Ident { .. } => 1,
        Expr::Binary { left, right, .. } => {
            1 + count_expr_expanded(left) + count_expr_expanded(right)
        }
        Expr::Unary { operand, .. } => 1 + count_expr_expanded(operand),
        Expr::Call { func, args } => {
            let mut n = count_expr_expanded(func) + 2;
            for a in args {
                n += count_expr_expanded(a);
                n += 1;
            }
            n
        }
        Expr::MethodCall { receiver, args, .. } => {
            let mut n = count_expr_expanded(receiver) + 1 + 2;
            for a in args {
                n += count_expr_expanded(a);
                n += 1;
            }
            n
        }
        Expr::FieldAccess { object, .. } => count_expr_expanded(object) + 2,
        Expr::Index { object, index } => {
            count_expr_expanded(object) + 2 + count_expr_expanded(index)
        }
        Expr::StructLit { fields, .. } => {
            let mut n: u32 = 2;
            for f in fields {
                n += 1;
                if f.value.is_some() {
                    n += 1;
                    n += 1;
                }
                n += 1;
            }
            n
        }
        Expr::TupleLit { elements } => {
            let mut n: u32 = 2;
            for e in elements {
                n += count_expr_expanded(e);
                n += 1;
            }
            n
        }
        Expr::ArrayLit { elements } => {
            let mut n: u32 = 2;
            for e in elements {
                n += count_expr_expanded(e);
                n += 1;
            }
            n
        }
        Expr::ArrayRepeat { value, count } => {
            3 + count_expr_expanded(value) + count_expr_expanded(count)
        }
        Expr::Closure { params, body } => {
            let mut n: u32 = 2; // | |
            n += params.len() as u32 * 3; // name : type per param (Rust is more verbose)
            n += count_expr_expanded(body);
            n
        }
        Expr::If { cond, then_block, else_block } => {
            let mut n = 1 + count_expr_expanded(cond) + count_block_expanded(then_block);
            if let Some(eb) = else_block {
                n += 1 + count_block_expanded(eb); // else
            }
            n
        }
        Expr::Match { scrutinee, arms } => {
            let mut n: u32 = 1; // match
            if let Some(s) = scrutinee {
                n += count_expr_expanded(s);
            }
            n += 1; // {
            for arm in arms {
                n += 1; // pattern
                n += 1; // =>
                n += count_expr_expanded(&arm.body);
                n += 1;
            }
            n += 1; // }
            n
        }
        Expr::Loop { body } => 1 + count_block_expanded(body),
        Expr::While { cond, body } => 1 + count_expr_expanded(cond) + count_block_expanded(body),
        Expr::For { iter, body, .. } => 3 + count_expr_expanded(iter) + count_block_expanded(body),
        Expr::Block { block } => count_block_expanded(block),
        Expr::Return { value } => {
            let mut n: u32 = 1;
            if let Some(v) = value {
                n += count_expr_expanded(v);
            }
            n
        }
        Expr::Break { value } => {
            let mut n: u32 = 1;
            if let Some(v) = value {
                n += count_expr_expanded(v);
            }
            n
        }
        Expr::Continue => 1,
        Expr::Try { expr } => count_expr_expanded(expr) + 1,
        Expr::Await { expr } => count_expr_expanded(expr) + 2, // .await
        Expr::Cast { expr, .. } => count_expr_expanded(expr) + 2,
        Expr::Assign { target, value } => {
            count_expr_expanded(target) + 1 + count_expr_expanded(value)
        }
        Expr::Range { start, end, .. } => count_expr_expanded(start) + 1 + count_expr_expanded(end),
        Expr::Todo => 2,                                                // todo!()
        Expr::Unimplemented => 2,                                       // unimplemented!()
        Expr::UnsafeBlock { block } => 1 + count_block_expanded(block), // unsafe
        Expr::Error { .. } => 1,
    }
}

// ── Utilities ────────────────────────────────────────────────────────

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path { segments, .. } => segments.join("."),
        Type::SelfType => "_T".to_string(),
        _ => "<type>".to_string(),
    }
}

fn item_name(kind: &ItemKind) -> String {
    match kind {
        ItemKind::Use(u) => u.path.join("."),
        ItemKind::TypeAlias(ta) => ta.name.clone(),
        ItemKind::Const(c) => c.name.clone(),
        ItemKind::Static(s) => s.name.clone(),
        ItemKind::Effect(e) => e.name.clone(),
        ItemKind::Spec(s) => s.name.clone(),
        ItemKind::Agent(a) => a.name.clone(),
        _ => "<item>".to_string(),
    }
}

// ── Display ──────────────────────────────────────────────────────────

impl TokenReport {
    /// Format as a human-readable report string.
    pub fn display(&self) -> String {
        let mut out = String::new();
        out.push_str("╔══════════════════════════════════════════════════════════════╗\n");
        out.push_str("║  Token Budget Report                                        ║\n");
        out.push_str("╚══════════════════════════════════════════════════════════════╝\n");
        out.push_str(&format!(
            "\n  {:30} {:>8} {:>8} {:>7}\n",
            "Item", "Compact", "Expanded", "Ratio"
        ));
        out.push_str(&format!("  {:-<30} {:->8} {:->8} {:->7}\n", "", "", "", ""));
        for m in &self.items {
            let kind_prefix = match m.kind {
                ItemMetricKind::Function => "f",
                ItemMetricKind::Struct => "S",
                ItemMetricKind::Enum => "E",
                ItemMetricKind::Trait => "T",
                ItemMetricKind::Impl => "I",
                ItemMetricKind::Module => "M",
                ItemMetricKind::Other => " ",
            };
            out.push_str(&format!(
                "  {kind_prefix} {:28} {:>8} {:>8} {:>6.0}%\n",
                m.name,
                m.compact_tokens,
                m.expanded_tokens,
                m.ratio * 100.0,
            ));
        }
        out.push_str(&format!("  {:-<30} {:->8} {:->8} {:->7}\n", "", "", "", ""));
        out.push_str(&format!(
            "  {:30} {:>8} {:>8} {:>6.0}%\n",
            "TOTAL",
            self.total_compact,
            self.total_expanded,
            self.overall_ratio * 100.0,
        ));
        out.push_str(&format!(
            "\n  Tokens saved: {} ({:.0}% reduction)\n",
            self.total_expanded.saturating_sub(self.total_compact),
            (1.0 - self.overall_ratio) * 100.0,
        ));
        out
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_module(source: &str) -> Module {
        let tokens = crate::lexer::lex(source);
        crate::parser::parse(&tokens).expect("parse failed")
    }

    #[test]
    fn simple_function_tokens() {
        let m = parse_module("f add(a: i32, b: i32) -> i32 { a }");
        let r = report(&m);
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].kind, ItemMetricKind::Function);
        assert!(r.items[0].compact_tokens > 0);
        assert!(r.items[0].expanded_tokens >= r.items[0].compact_tokens);
        assert!(r.items[0].ratio <= 1.0);
    }

    #[test]
    fn pub_function_more_expanded() {
        let m = parse_module("+f greet(name: s) -> s { name }");
        let r = report(&m);
        // pub fn in expanded Rust adds at least: `pub` and `fn` as separate tokens
        // plus Rust uses `String` (1 token) where we use `s` (1 token), so types are same.
        // The expanded count should be >= compact count.
        assert!(
            r.items[0].expanded_tokens >= r.items[0].compact_tokens,
            "expanded={} compact={}",
            r.items[0].expanded_tokens,
            r.items[0].compact_tokens
        );
    }

    #[test]
    fn struct_tokens() {
        let m = parse_module("+S Point { x: f64, y: f64 }");
        let r = report(&m);
        assert_eq!(r.items[0].kind, ItemMetricKind::Struct);
        assert!(r.items[0].compact_tokens > 0);
    }

    #[test]
    fn report_has_totals() {
        let m = parse_module("f foo() { 1 }\nf bar() { 2 }");
        let r = report(&m);
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.total_compact, r.items[0].compact_tokens + r.items[1].compact_tokens);
    }

    #[test]
    fn option_type_more_compact() {
        // ?i32 (1 token) vs Option<i32> (3 tokens)
        let compact = count_type_compact(&Type::Option {
            inner: Box::new(Type::Path { segments: vec!["i32".to_string()], type_args: vec![] }),
        });
        let expanded = count_type_expanded(&Type::Option {
            inner: Box::new(Type::Path { segments: vec!["i32".to_string()], type_args: vec![] }),
        });
        assert!(compact < expanded, "compact={compact} expanded={expanded}");
    }

    #[test]
    fn display_report() {
        let m = parse_module("f add(a: i32, b: i32) -> i32 { a }");
        let r = report(&m);
        let text = r.display();
        assert!(text.contains("Token Budget Report"));
        assert!(text.contains("add"));
        assert!(text.contains("TOTAL"));
    }

    #[test]
    fn ratio_one_for_trivial() {
        // An empty-ish construct should have ratio <= 1.0
        let m = parse_module("f noop() { 1 }");
        let r = report(&m);
        assert!(r.overall_ratio <= 1.0);
    }
}
