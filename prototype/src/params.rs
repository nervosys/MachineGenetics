//! Tunable parameters (plan 6.1).
//!
//! A `const` carrying a `@param(...)` attribute declares a domain its value
//! must lie in:
//!
//! ```text
//! @param(0.0, 1.0)      c LR: f64 = 0.02;     // closed real interval
//! @param(1, 64)         c LAYERS: i64 = 4;    // closed integer interval
//! @param(in, 1, 2, 4)   c HEADS: i64 = 4;     // finite set
//! ```
//!
//! The domain is checked here, once, and every problem is reported as an
//! E0553 (`ParamDomain`) diagnostic. `types::check` appends these diagnostics,
//! so an out-of-domain parameter is a type error.

use crate::ast::{self, Expr, ItemKind, LiteralKind, Type};
use crate::hir::{self, DiagnosticCategory, Severity};

/// The set of values a tunable parameter may take.
#[derive(Debug, Clone, PartialEq)]
pub enum Domain {
    /// Closed real interval `[lo, hi]` (for `f32`/`f64` constants).
    Real { lo: f64, hi: f64 },
    /// Closed integer interval `[lo, hi]` (for integer constants).
    Int { lo: i64, hi: i64 },
    /// Finite set of admissible values.
    Set(Vec<f64>),
}

impl Domain {
    /// Whether `v` lies in the domain.
    pub fn contains(&self, v: f64) -> bool {
        match self {
            Domain::Real { lo, hi } => v >= *lo && v <= *hi,
            Domain::Int { lo, hi } => v.fract() == 0.0 && v >= *lo as f64 && v <= *hi as f64,
            Domain::Set(members) => members.contains(&v),
        }
    }

    /// Human-readable form used in diagnostics: `[0, 1]` or `{1, 2, 4}`.
    fn describe(&self) -> String {
        match self {
            Domain::Real { lo, hi } => format!("[{lo}, {hi}]"),
            Domain::Int { lo, hi } => format!("[{lo}, {hi}]"),
            Domain::Set(members) => {
                let parts: Vec<String> = members.iter().map(|m| m.to_string()).collect();
                format!("{{{}}}", parts.join(", "))
            }
        }
    }
}

/// A well-formed tunable parameter: its name, domain and default value.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDecl {
    pub name: String,
    pub domain: Domain,
    pub value: f64,
}

/// Kind of numeric type a parameter's `const` is declared with.
#[derive(Clone, Copy, PartialEq)]
enum NumKind {
    Int,
    Float,
}

/// Collect every `@param` declaration in `module`. Returns the well-formed
/// declarations and one E0553 diagnostic per problem found.
pub fn params(module: &ast::Module) -> (Vec<ParamDecl>, Vec<hir::Diagnostic>) {
    let mut decls = Vec::new();
    let mut diags = Vec::new();
    collect(&module.items, &mut decls, &mut diags);
    (decls, diags)
}

fn collect(items: &[ast::Item], decls: &mut Vec<ParamDecl>, diags: &mut Vec<hir::Diagnostic>) {
    for item in items {
        if let ItemKind::Module(m) = &item.kind {
            if let Some(inner) = &m.items {
                collect(inner, decls, diags);
            }
        }
        let attrs: Vec<&ast::Attribute> =
            item.attributes.iter().filter(|a| a.name == "param").collect();
        if attrs.is_empty() {
            continue;
        }
        if attrs.len() > 1 {
            diags.push(problem(
                "an item may carry only one @param attribute".to_string(),
            ));
            continue;
        }
        match check_param(item, attrs[0]) {
            Ok(decl) => decls.push(decl),
            Err(msg) => diags.push(problem(msg)),
        }
    }
}

fn problem(msg: String) -> hir::Diagnostic {
    hir::Diagnostic::categorized(Severity::Error, msg, DiagnosticCategory::ParamDomain, None)
}

/// Check one annotated item. Returns the first problem found, so each
/// annotated item yields at most one diagnostic.
fn check_param(item: &ast::Item, attr: &ast::Attribute) -> Result<ParamDecl, String> {
    // (a) only constants can be parameters.
    let ItemKind::Const(c) = &item.kind else {
        return Err("`@param` is only valid on a `const` declaration".to_string());
    };
    let name = &c.name;

    // (c) the constant's type must be an integer or a float.
    let kind = match num_kind(&c.ty) {
        Some(k) => k,
        None => {
            return Err(format!(
                "`{name}` has type `{}`, which is neither an integer nor a float, so it cannot carry a @param domain",
                type_name(&c.ty)
            ))
        }
    };

    // (b, c) the domain itself.
    let args = join_signs(&attr.args);
    let domain = if args.first().map(String::as_str) == Some("in") {
        let members = &args[1..];
        if members.is_empty() {
            return Err(format!("`{name}` has an empty @param set; `@param(in, ...)` needs at least one member"));
        }
        let mut values = Vec::with_capacity(members.len());
        for m in members {
            values.push(parse_bound(name, m, kind, "set member")?);
        }
        Domain::Set(values)
    } else {
        if args.len() != 2 {
            return Err(format!(
                "`{name}` has a malformed @param domain: expected `@param(lo, hi)` or `@param(in, ...)`, found {} argument(s)",
                args.len()
            ));
        }
        match kind {
            NumKind::Int => {
                let lo = parse_int_bound(name, &args[0], "lower bound")?;
                let hi = parse_int_bound(name, &args[1], "upper bound")?;
                if lo > hi {
                    return Err(format!("`{name}` has an empty @param domain: lower bound {lo} is above upper bound {hi}"));
                }
                Domain::Int { lo, hi }
            }
            NumKind::Float => {
                let lo = parse_bound(name, &args[0], kind, "lower bound")?;
                let hi = parse_bound(name, &args[1], kind, "upper bound")?;
                if lo > hi {
                    return Err(format!("`{name}` has an empty @param domain: lower bound {lo} is above upper bound {hi}"));
                }
                Domain::Real { lo, hi }
            }
        }
    };

    // (d) the default must be readable without evaluation.
    let value = literal_value(&c.value).ok_or_else(|| {
        format!("`{name}` must have a numeric literal value (optionally negated) so its default can be read without evaluation")
    })?;

    // (e) the default must lie in the domain.
    if !domain.contains(value) {
        return Err(format!(
            "`{name}` = {value} is outside its domain {}",
            domain.describe()
        ));
    }

    Ok(ParamDecl {
        name: name.clone(),
        domain,
        value,
    })
}

/// Integer or float kind of a type, if it is one.
fn num_kind(ty: &Type) -> Option<NumKind> {
    let Type::Path { segments, type_args } = ty else {
        return None;
    };
    if !type_args.is_empty() || segments.len() != 1 {
        return None;
    }
    match segments[0].as_str() {
        "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "isize" | "usize" => {
            Some(NumKind::Int)
        }
        "f32" | "f64" => Some(NumKind::Float),
        _ => None,
    }
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path { segments, .. } => segments.join("::"),
        _ => "a non-primitive type".to_string(),
    }
}

/// The parser emits `-` and the number as separate argument tokens; rejoin
/// them so `@param(-1.5, 2)` arrives as `["-1.5", "2"]`.
fn join_signs(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-" && i + 1 < args.len() {
            out.push(format!("-{}", args[i + 1]));
            i += 2;
        } else {
            out.push(args[i].clone());
            i += 1;
        }
    }
    out
}

/// Strip digit separators from a literal's text.
fn clean_literal(text: &str) -> String {
    text.replace('_', "")
}

/// Parse a domain bound of the given kind as `f64`; rejects a non-numeric
/// token (b) and, for integer constants, a non-integer one (c).
fn parse_bound(name: &str, text: &str, kind: NumKind, role: &str) -> Result<f64, String> {
    match kind {
        NumKind::Int => parse_int_bound(name, text, role).map(|v| v as f64),
        NumKind::Float => clean_literal(text).parse::<f64>().map_err(|_| {
            format!("`{name}` has a malformed @param domain: {role} `{text}` is not a number")
        }),
    }
}

fn parse_int_bound(name: &str, text: &str, role: &str) -> Result<i64, String> {
    if let Ok(v) = clean_literal(text).parse::<i64>() {
        return Ok(v);
    }
    if clean_literal(text).parse::<f64>().is_ok() {
        return Err(format!(
            "`{name}` is an integer constant but its @param domain {role} `{text}` is not an integer"
        ));
    }
    Err(format!(
        "`{name}` has a malformed @param domain: {role} `{text}` is not a number"
    ))
}

/// Value of a numeric literal, optionally negated. `None` for anything else.
fn literal_value(e: &Expr) -> Option<f64> {
    match e {
        Expr::Literal { value, kind } if matches!(kind, LiteralKind::Int | LiteralKind::Float) => {
            clean_literal(value).parse::<f64>().ok()
        }
        Expr::Unary { op, operand } if op == "-" => literal_value(operand).map(|v| -v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_src(src: &str) -> ast::Module {
        crate::parser::parse(&crate::lexer::lex(src)).expect("parse failed")
    }

    /// Exactly one E0553 diagnostic and no declarations.
    fn one_problem(src: &str) -> String {
        let (decls, diags) = params(&parse_src(src));
        assert!(decls.is_empty(), "expected no decls, got {decls:?}");
        assert_eq!(diags.len(), 1, "expected one diagnostic, got {diags:?}");
        assert_eq!(diags[0].category.unwrap().code(), "E0553");
        assert_eq!(diags[0].severity, Severity::Error);
        diags[0].message.clone()
    }

    fn passes(src: &str) -> ParamDecl {
        let (mut decls, diags) = params(&parse_src(src));
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");
        assert_eq!(decls.len(), 1, "expected one decl");
        decls.remove(0)
    }

    // ── Each domain kind ─────────────────────────────────────

    #[test]
    fn real_interval_passes_inside() {
        let d = passes("@param(0.0, 1.0)\nconst LR: f64 = 0.02;");
        assert_eq!(d.name, "LR");
        assert_eq!(d.domain, Domain::Real { lo: 0.0, hi: 1.0 });
        assert_eq!(d.value, 0.02);
    }

    #[test]
    fn real_interval_is_closed_at_both_ends() {
        passes("@param(0.0, 1.0)\nconst A: f64 = 0.0;");
        passes("@param(0.0, 1.0)\nconst B: f64 = 1.0;");
    }

    #[test]
    fn real_interval_fails_outside() {
        let msg = one_problem("@param(0.0, 1.0)\nconst LR: f64 = 1.5;");
        assert_eq!(msg, "`LR` = 1.5 is outside its domain [0, 1]");
    }

    #[test]
    fn int_interval_passes_inside() {
        let d = passes("@param(1, 64)\nconst LAYERS: i64 = 4;");
        assert_eq!(d.domain, Domain::Int { lo: 1, hi: 64 });
        assert_eq!(d.value, 4.0);
    }

    #[test]
    fn int_interval_fails_outside() {
        let msg = one_problem("@param(1, 64)\nconst LAYERS: i64 = 65;");
        assert_eq!(msg, "`LAYERS` = 65 is outside its domain [1, 64]");
    }

    #[test]
    fn set_passes_member_and_fails_non_member() {
        let d = passes("@param(in, 1, 2, 4, 8)\nconst HEADS: i64 = 4;");
        assert_eq!(d.domain, Domain::Set(vec![1.0, 2.0, 4.0, 8.0]));
        let msg = one_problem("@param(in, 1, 2, 4, 8)\nconst HEADS: i64 = 3;");
        assert_eq!(msg, "`HEADS` = 3 is outside its domain {1, 2, 4, 8}");
    }

    #[test]
    fn contains_respects_each_kind() {
        assert!(Domain::Real { lo: 0.0, hi: 1.0 }.contains(0.5));
        assert!(!Domain::Real { lo: 0.0, hi: 1.0 }.contains(1.0001));
        assert!(Domain::Int { lo: 1, hi: 3 }.contains(2.0));
        assert!(!Domain::Int { lo: 1, hi: 3 }.contains(2.5));
        assert!(!Domain::Int { lo: 1, hi: 3 }.contains(4.0));
        assert!(Domain::Set(vec![1.0, 4.0]).contains(4.0));
        assert!(!Domain::Set(vec![1.0, 4.0]).contains(2.0));
    }

    // ── Negative numbers ─────────────────────────────────────

    #[test]
    fn negative_real_bounds_and_negative_value_pass() {
        let d = passes("@param(-1.0, 1.0)\nconst X: f64 = -0.5;");
        assert_eq!(d.domain, Domain::Real { lo: -1.0, hi: 1.0 });
        assert_eq!(d.value, -0.5);
    }

    #[test]
    fn negative_int_bounds_pass() {
        let d = passes("@param(-5, -1)\nconst N: i64 = -3;");
        assert_eq!(d.domain, Domain::Int { lo: -5, hi: -1 });
        assert_eq!(d.value, -3.0);
    }

    #[test]
    fn negative_set_member_passes() {
        passes("@param(in, -2, 2)\nconst S: i64 = -2;");
    }

    #[test]
    fn negative_value_outside_domain_fails() {
        let msg = one_problem("@param(0.0, 1.0)\nconst X: f64 = -0.5;");
        assert_eq!(msg, "`X` = -0.5 is outside its domain [0, 1]");
    }

    // ── Problems (a)-(e): each exactly one E0553 ─────────────

    #[test]
    fn a_param_on_non_const_is_refused() {
        let msg = one_problem("@param(0.0, 1.0)\nf g() -> i64 { 0 }");
        assert!(msg.contains("only valid on a `const`"), "{msg}");
    }

    #[test]
    fn b_wrong_argument_count_is_refused() {
        let msg = one_problem("@param(0.0)\nconst LR: f64 = 0.5;");
        assert!(msg.contains("malformed @param domain"), "{msg}");
    }

    #[test]
    fn b_non_numeric_bound_is_refused() {
        let msg = one_problem("@param(a, 1.0)\nconst LR: f64 = 0.5;");
        assert!(msg.contains("is not a number"), "{msg}");
    }

    #[test]
    fn b_inverted_bounds_are_refused() {
        let msg = one_problem("@param(2.0, 1.0)\nconst LR: f64 = 1.5;");
        assert!(msg.contains("empty @param domain"), "{msg}");
    }

    #[test]
    fn b_empty_set_is_refused() {
        let msg = one_problem("@param(in)\nconst H: i64 = 1;");
        assert!(msg.contains("empty @param set"), "{msg}");
    }

    #[test]
    fn c_float_bound_on_integer_const_is_refused() {
        let msg = one_problem("@param(0.0, 1)\nconst N: i64 = 0;");
        assert!(msg.contains("is not an integer"), "{msg}");
    }

    #[test]
    fn c_float_set_member_on_integer_const_is_refused() {
        let msg = one_problem("@param(in, 1, 2.5)\nconst H: i64 = 1;");
        assert!(msg.contains("is not an integer"), "{msg}");
    }

    #[test]
    fn c_non_numeric_type_is_refused() {
        let msg = one_problem("@param(0, 1)\nconst B: bool = true;");
        assert!(msg.contains("neither an integer nor a float"), "{msg}");
    }

    #[test]
    fn d_non_literal_value_is_refused() {
        let msg = one_problem("@param(0.0, 1.0)\nconst LR: f64 = 0.5 + 0.1;");
        assert!(msg.contains("numeric literal value"), "{msg}");
    }

    #[test]
    fn e_out_of_domain_value_is_refused() {
        let msg = one_problem("@param(0.0, 1.0)\nconst LR: f64 = 1.5;");
        assert_eq!(msg, "`LR` = 1.5 is outside its domain [0, 1]");
    }

    // ── Pass-through and end-to-end ──────────────────────────

    #[test]
    fn const_without_param_is_unaffected() {
        let m = parse_src("const N: i64 = 1000;");
        let (decls, diags) = params(&m);
        assert!(decls.is_empty());
        assert!(diags.is_empty());
        let tc = crate::types::check(&m);
        assert!(
            tc.diagnostics
                .iter()
                .all(|d| d.category != Some(DiagnosticCategory::ParamDomain)),
            "{:?}",
            tc.diagnostics
        );
    }

    #[test]
    fn types_check_refuses_out_of_domain_parameter() {
        let m = parse_src("@param(0.0, 1.0)\nconst LR: f64 = 1.5;");
        let tc = crate::types::check(&m);
        let hits: Vec<&hir::Diagnostic> = tc
            .diagnostics
            .iter()
            .filter(|d| d.category.map(|c| c.code()) == Some("E0553"))
            .collect();
        assert_eq!(hits.len(), 1, "{:?}", tc.diagnostics);
        assert_eq!(hits[0].message, "`LR` = 1.5 is outside its domain [0, 1]");
    }

    #[test]
    fn types_check_accepts_in_domain_parameter() {
        let m = parse_src("@param(1, 64)\nconst LAYERS: i64 = 4;");
        let tc = crate::types::check(&m);
        assert!(
            tc.diagnostics
                .iter()
                .all(|d| d.category != Some(DiagnosticCategory::ParamDomain)),
            "{:?}",
            tc.diagnostics
        );
    }
}
