//! MAGE-core: the checked subset the RSI kernel runs (`MAGE_SPEC.md` §4.12).
//!
//! Plan task 1.1. Code the kernel runs on the loop's behalf — generated
//! candidates, the reward, and later every policy the harness evolves — must
//! be code whose authority is stated and whose reach is bounded. Until this
//! module, each consumer enforced its own ad-hoc rules (the arena's substrate
//! wanted `f gen`, the reward loader wanted `@role(evaluator)`), and nothing
//! said what the subset *was*. It is:
//!
//! 1. **Every function and method declares a role** (`@role(…)`, §11.6). A
//!    missing role is `E0551`, the role violation, because it is one: code
//!    with no stated ceiling.
//! 2. **No `unsafe`** — neither an `unsafe` function nor an `unsafe` block.
//! 3. **Only core items**: functions, `struct`/`data`/`enum`, type aliases,
//!    constants, effect declarations, traits and `impl`/`extend` blocks, and
//!    `spec` contracts. `use` and `mod` (there is no module system, §2.3),
//!    `static` (mutable global state), and the domain constructs the kernel
//!    does not run — `net`, `train`, `evolve`, `kb`, `agent`, `swarm` — are
//!    outside it. Those are `E0552`.
//!
//! What a role *allows* is checked by the effect pass, not here: this module is
//! structural, so the two cannot disagree about the same fact.

use crate::ast::{self, ItemKind};
use crate::hir::{Diagnostic, DiagnosticCategory, Severity};

/// Every MAGE-core violation in `module`, in item order.
pub fn check(module: &ast::Module) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for item in &module.items {
        check_item(item, "", &mut out);
    }
    out
}

fn violation(msg: String) -> Diagnostic {
    Diagnostic::categorized(Severity::Error, msg, DiagnosticCategory::CoreViolation, None)
}

fn check_function(item: &ast::Item, f: &ast::FunctionDef, key: &str, out: &mut Vec<Diagnostic>) {
    if !item.attributes.iter().any(|a| a.name == "role") {
        out.push(Diagnostic::categorized(
            Severity::Error,
            format!("`{key}` declares no role; MAGE-core code must state its authority with @role(…)"),
            DiagnosticCategory::RoleViolation,
            None,
        ));
    }
    if f.is_unsafe {
        out.push(violation(format!("`{key}` is `unsafe`, which MAGE-core does not admit")));
    }
    // An `unsafe { … }` block anywhere in the body. Found on the serde form,
    // as `canon` walks it, so a new expression form cannot hide one.
    let body = serde_json::to_value(f).unwrap_or_default();
    if contains_type(&body, "UnsafeBlock") {
        out.push(violation(format!("`{key}` contains an `unsafe` block, which MAGE-core does not admit")));
    }
}

fn check_item(item: &ast::Item, prefix: &str, out: &mut Vec<Diagnostic>) {
    match &item.kind {
        ItemKind::Function(f) => {
            let key = if prefix.is_empty() { f.name.clone() } else { format!("{prefix}.{}", f.name) };
            check_function(item, f, &key, out);
        }
        ItemKind::Impl(ast::ImplBlock { self_type: target, items, .. })
        | ItemKind::Extend(ast::ExtendBlock { target_type: target, items, .. }) => {
            let ty = crate::eval::type_head_name(target).unwrap_or_else(|| "?".into());
            for member in items {
                check_item(member, &ty, out);
            }
        }
        ItemKind::Trait(t) => {
            for member in &t.items {
                // A trait's required methods have no body to run; only those
                // with a default body are code the kernel could execute.
                if let ItemKind::Function(f) = &member.kind {
                    if !f.body.stmts.is_empty() || f.body.tail_expr.is_some() || f.body_expr.is_some() {
                        check_function(member, f, &format!("{}.{}", t.name, f.name), out);
                    }
                }
            }
        }
        ItemKind::Struct(_)
        | ItemKind::Enum(_)
        | ItemKind::Data(_)
        | ItemKind::TypeAlias(_)
        | ItemKind::Const(_)
        | ItemKind::Effect(_)
        | ItemKind::Spec(_) => {}
        other => {
            let what = serde_json::to_value(other)
                .ok()
                .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_string))
                .unwrap_or_else(|| "this item".into());
            out.push(violation(format!("`{what}` items are outside MAGE-core (§4.12)")));
        }
    }
}

fn contains_type(v: &serde_json::Value, ty: &str) -> bool {
    match v {
        serde_json::Value::Object(m) => {
            m.get("type").and_then(|t| t.as_str()) == Some(ty) || m.values().any(|c| contains_type(c, ty))
        }
        serde_json::Value::Array(xs) => xs.iter().any(|c| contains_type(c, ty)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diags(src: &str) -> Vec<Diagnostic> {
        check(&crate::parser::parse(&crate::lexer::lex(src)).expect("parses"))
    }

    fn categories(src: &str) -> Vec<DiagnosticCategory> {
        diags(src).iter().filter_map(|d| d.category).collect()
    }

    #[test]
    fn roled_pure_code_is_core() {
        let src = "@role(candidate)\nf gen(s: usize) -> [usize] { range(3) }\n\
                   D Point(x: i64, y: i64)\n\
                   @role(evaluator)\nf score(a: f64) -> f64 { a }";
        assert_eq!(diags(src).len(), 0, "{:?}", diags(src));
    }

    #[test]
    fn a_function_without_a_role_is_a_role_violation() {
        assert_eq!(categories("f g() -> i64 { 1 }"), vec![DiagnosticCategory::RoleViolation]);
    }

    #[test]
    fn domain_constructs_and_modules_are_outside_core() {
        // Each of these parses — asserted, so the test cannot pass vacuously
        // on a source the parser rejects.
        for src in [
            "use std.io;",
            "mod inner { }",
            "agent A { }",
            "net N {
    layer fc1: Linear(8, 16);
}",
        ] {
            let m = crate::parser::parse(&crate::lexer::lex(src)).unwrap_or_else(|e| panic!("{src}: {e:?}"));
            let c: Vec<_> = check(&m).iter().filter_map(|d| d.category).collect();
            assert!(c.contains(&DiagnosticCategory::CoreViolation), "{src}: {c:?}");
        }
    }

    #[test]
    fn unsafe_is_outside_core() {
        // `unsafe` has an AST form and no parser arm (HANDOFF, Expr::UnsafeBlock),
        // so it can only arrive in an AST built programmatically — which is
        // exactly how an agent emitting AST would deliver it. Built here.
        let mut m = crate::parser::parse(&crate::lexer::lex("@role(candidate)
f g() -> i64 { 1 }")).unwrap();
        if let ItemKind::Function(f) = &mut m.items[0].kind {
            f.is_unsafe = true;
        }
        let c: Vec<_> = check(&m).iter().filter_map(|d| d.category).collect();
        assert_eq!(c, vec![DiagnosticCategory::CoreViolation]);
    }

    #[test]
    fn methods_need_roles_too() {
        let src = "S P { x: i64 }\nI P { f get(&self) -> i64 { self.x } }";
        assert_eq!(categories(src), vec![DiagnosticCategory::RoleViolation]);
        let ok = "S P { x: i64 }\nI P { @role(candidate)\n f get(&self) -> i64 { self.x } }";
        assert_eq!(categories(ok), vec![]);
    }
}
