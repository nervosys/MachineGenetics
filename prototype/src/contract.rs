//! Contract clauses as expressions: `result` and `old` in `@ens` (plan C1).
//!
//! A clause is stored as source text (`SpecItem::Ensure(String)`,
//! `ContractClause::condition`), which is all the verifier's textual
//! refutation needs. Typechecking a clause, and evaluating a postcondition at
//! run time, need it as an [`Expr`], so this module parses it again, on its
//! own, after binding the two names only a postcondition has:
//!
//! * `result` — the function's return value, typed as its return type;
//! * `old(x)` — parameter `x` as it was on entry. MAGE values are immutable,
//!   so that is the argument as passed, and `old(x)` is rewritten to `x`
//!   before parsing. It still has to be written correctly: `old` takes a
//!   parameter name, and nothing else.
//!
//! Both are scope errors (`E0561`, [`DiagnosticCategory::ContractScope`])
//! anywhere but an `@ens`. Neither is a keyword: `result` is an ordinary
//! variable name in a function body, and `old` an ordinary function name.
//!
//! A clause belongs to a function when it is written on it, or in a spec
//! block (`sp name { … }`) that shares a top-level function's name. Only
//! those are typechecked and evaluated; a spec with no function has no
//! parameters or return type to check against, so only the scope rule
//! applies to it.
//!
//! [`DiagnosticCategory::ContractScope`]: crate::hir::DiagnosticCategory::ContractScope

use crate::ast::{self, ContractClauseKind, Expr, FunctionDef, ItemKind};
use crate::lexer::{self, Token, TokenKind};

/// One `@req` / `@ens` / `@inv` clause, as written.
#[derive(Debug, Clone)]
pub struct Clause {
    pub kind: ContractClauseKind,
    /// The condition's source text, without the message.
    pub condition: String,
    /// The optional human-readable message (`@ens(cond, "message")`).
    pub message: Option<String>,
    /// The function or spec the clause is attached to.
    pub function: String,
    /// Where it was written: `sp withdraw`, or `f withdraw` for a clause
    /// above the function.
    pub origin: String,
}

impl Clause {
    /// The clause as written, e.g. `@ens(result >= 0, "non-negative")`.
    pub fn display(&self) -> String {
        let tag = match self.kind {
            ContractClauseKind::Requires => "@req",
            ContractClauseKind::Ensures => "@ens",
            ContractClauseKind::Invariant => "@inv",
        };
        match &self.message {
            Some(m) => format!("{tag}({}, \"{m}\")", self.condition),
            None => format!("{tag}({})", self.condition),
        }
    }
}

/// Why a clause could not be turned into an expression.
#[derive(Debug, Clone, PartialEq)]
pub enum PrepareError {
    /// `result` or `old` used where it is not in scope (`E0561`).
    Scope(String),
    /// The condition is not an expression.
    Syntax(String),
}

/// Every contract clause of `module`'s top-level items, each with the
/// function it belongs to, if any.
pub fn collect(module: &ast::Module) -> Vec<(Clause, Option<&FunctionDef>)> {
    let functions: std::collections::HashMap<&str, &FunctionDef> = module
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Function(fd) => Some((fd.name.as_str(), fd)),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for item in &module.items {
        match &item.kind {
            ItemKind::Function(fd) => {
                for c in &fd.contracts {
                    out.push((
                        Clause {
                            kind: c.kind,
                            condition: c.condition.clone(),
                            message: c.message.clone(),
                            function: fd.name.clone(),
                            origin: format!("f {}", fd.name),
                        },
                        Some(fd),
                    ));
                }
            }
            ItemKind::Spec(sp) => {
                let fd = functions.get(sp.name.as_str()).copied();
                for si in &sp.items {
                    let (kind, text) = match si {
                        ast::SpecItem::Require(t) => (ContractClauseKind::Requires, t),
                        ast::SpecItem::Ensure(t) => (ContractClauseKind::Ensures, t),
                        ast::SpecItem::Invariant(t) => (ContractClauseKind::Invariant, t),
                        _ => continue,
                    };
                    let (condition, message) = split_message(text);
                    out.push((
                        Clause {
                            kind,
                            condition,
                            message,
                            function: sp.name.clone(),
                            origin: format!("sp {}", sp.name),
                        },
                        fd,
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

/// Split a spec item's text, `cond, "message"`, at its top-level comma.
///
/// A spec block keeps the whole parenthesised text, message included; a
/// clause written above a function was split by the parser already.
fn split_message(text: &str) -> (String, Option<String>) {
    let tokens = lexer::lex(text);
    let mut depth: i32 = 0;
    for (i, t) in tokens.iter().enumerate() {
        match t.kind {
            TokenKind::LParen | TokenKind::LBrack | TokenKind::LBrace => depth += 1,
            TokenKind::RParen | TokenKind::RBrack | TokenKind::RBrace => depth -= 1,
            TokenKind::Comma if depth == 0 => {
                let cond = text[..t.span.offset].trim().to_string();
                let message = tokens
                    .get(i + 1)
                    .filter(|m| m.kind == TokenKind::StringLiteral)
                    .map(|m| m.text.trim_matches('"').to_string());
                return (cond, message);
            }
            _ => {}
        }
    }
    (text.trim().to_string(), None)
}

/// Parse `clause` into an expression, enforcing where `result` and `old` may
/// appear and rewriting `old(x)` to `x`.
///
/// `params` are the function's parameter names, when the clause belongs to
/// one; `None` for a spec with no function, which checks only that `result`
/// and `old` appear in an `@ens`.
pub fn prepare(clause: &Clause, params: Option<&[String]>) -> Result<Expr, PrepareError> {
    let tokens = lexer::lex(&clause.condition);
    let is_ens = clause.kind == ContractClauseKind::Ensures;
    let f = &clause.function;
    let is_param = |name: &str| params.is_some_and(|ps| ps.iter().any(|p| p == name));
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        let after_dot = i > 0 && tokens[i - 1].kind == TokenKind::Dot;
        if t.kind == TokenKind::Ident && !after_dot {
            if t.text == "old" && tokens.get(i + 1).map(|n| n.kind) == Some(TokenKind::LParen) {
                if !is_ens {
                    return Err(PrepareError::Scope(format!(
                        "`old(..)` is only in scope inside `@ens`, not in `{}` of `{f}`: \
                         a precondition already sees each parameter as passed, so write the parameter name",
                        clause.display()
                    )));
                }
                let arg = tokens.get(i + 2).filter(|a| a.kind == TokenKind::Ident);
                let closed = tokens.get(i + 3).map(|c| c.kind) == Some(TokenKind::RParen);
                let Some(arg) = arg.filter(|_| closed) else {
                    return Err(PrepareError::Scope(format!(
                        "`old` takes a parameter name, as in `old(x)`, in `{}` of `{f}`",
                        clause.display()
                    )));
                };
                if params.is_some() && !is_param(&arg.text) {
                    return Err(PrepareError::Scope(format!(
                        "`old({0})` in `{1}`: `{0}` is not a parameter of `{f}`, and `old` takes a parameter name",
                        arg.text,
                        clause.display()
                    )));
                }
                out.push(arg.clone());
                i += 4;
                continue;
            }
            if t.text == "result" {
                if is_ens && is_param("result") {
                    return Err(PrepareError::Scope(format!(
                        "`{f}` has a parameter named `result`, which `@ens` reserves \
                         for the return value; rename the parameter"
                    )));
                }
                if !is_ens && !is_param("result") {
                    return Err(PrepareError::Scope(format!(
                        "`result` is only in scope inside `@ens`, not in `{}` of `{f}`: \
                         it names the return value, which does not exist yet",
                        clause.display()
                    )));
                }
            }
        }
        out.push(t.clone());
        i += 1;
    }
    crate::parser::parse_expression(&out).map_err(|e| {
        PrepareError::Syntax(format!(
            "contract `{}` of `{f}` is not an expression: {}",
            clause.display(),
            e.message
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module(src: &str) -> ast::Module {
        crate::parser::parse(&lexer::lex(src)).expect("parses")
    }

    fn params(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_spec_clause_is_split_from_its_message_and_finds_its_function() {
        let m = module(
            "sp add {\n    @req(a >= 0)\n    @ens(result >= a, \"no smaller\")\n}\n\
             f add(a: i64, b: i64) -> i64 { a + b }\n",
        );
        let clauses = collect(&m);
        assert_eq!(clauses.len(), 2);
        let (ens, fd) = &clauses[1];
        assert_eq!(ens.kind, ContractClauseKind::Ensures);
        assert_eq!(ens.condition, "result >= a");
        assert_eq!(ens.message.as_deref(), Some("no smaller"));
        assert_eq!(ens.origin, "sp add");
        assert_eq!(fd.map(|f| f.name.as_str()), Some("add"));
        assert_eq!(ens.display(), "@ens(result >= a, \"no smaller\")");
    }

    #[test]
    fn result_and_old_parse_inside_ens() {
        let c = Clause {
            kind: ContractClauseKind::Ensures,
            condition: "result == old(a) + b".into(),
            message: None,
            function: "add".into(),
            origin: "sp add".into(),
        };
        let e = prepare(&c, Some(&params(&["a", "b"]))).expect("prepares");
        // `old(a)` is the argument as passed, so it is rewritten to `a`.
        let Expr::Binary { left, right, .. } = e else { panic!("binary") };
        assert!(matches!(*left, Expr::Ident { ref name } if name == "result"));
        let Expr::Binary { left: l2, .. } = *right else { panic!("binary") };
        assert!(matches!(*l2, Expr::Ident { ref name } if name == "a"));
    }

    #[test]
    fn result_and_old_are_scope_errors_outside_ens() {
        let mut c = Clause {
            kind: ContractClauseKind::Requires,
            condition: "result > 0".into(),
            message: None,
            function: "g".into(),
            origin: "sp g".into(),
        };
        let p = params(&["x"]);
        assert!(matches!(prepare(&c, Some(&p)), Err(PrepareError::Scope(m)) if m.contains("only in scope inside `@ens`")));
        c.condition = "old(x) > 0".into();
        assert!(matches!(prepare(&c, Some(&p)), Err(PrepareError::Scope(m)) if m.contains("`old(..)` is only in scope")));
        // Field access named `result` is not the return value.
        c.condition = "x.result > 0".into();
        assert!(prepare(&c, Some(&p)).is_ok());
    }

    #[test]
    fn old_takes_only_a_parameter_name() {
        let mut c = Clause {
            kind: ContractClauseKind::Ensures,
            condition: "result == old(y)".into(),
            message: None,
            function: "g".into(),
            origin: "sp g".into(),
        };
        let p = params(&["x"]);
        assert!(matches!(prepare(&c, Some(&p)), Err(PrepareError::Scope(m)) if m.contains("`y` is not a parameter of `g`")));
        c.condition = "result == old(x + 1)".into();
        assert!(matches!(prepare(&c, Some(&p)), Err(PrepareError::Scope(m)) if m.contains("takes a parameter name")));
        // Without a function the parameters are unknown, so a name is accepted.
        c.condition = "result == old(y)".into();
        assert!(prepare(&c, None).is_ok());
    }

    // ── Typechecking ─────────────────────────────────────────────

    /// `(code, message)` of every error `resolve` and `types::check` report.
    fn errors(src: &str) -> Vec<(String, String)> {
        let m = module(src);
        let mut diags = crate::resolve::resolve(&m).diagnostics;
        diags.extend(crate::types::check(&m).diagnostics);
        diags
            .into_iter()
            .filter(|d| d.severity == crate::hir::Severity::Error)
            .map(|d| (d.category.map_or("", |c| c.code()).to_string(), d.message))
            .collect()
    }

    const ADD: &str = "f add(a: i64, b: i64) -> i64 { a + b }\n";

    #[test]
    fn a_postcondition_over_result_and_old_typechecks() {
        let src = format!(
            "sp add {{\n    @req(a >= 0)\n    @ens(result == old(a) + b, \"sum\")\n    @ens(result >= a)\n}}\n{ADD}"
        );
        assert_eq!(errors(&src), Vec::<(String, String)>::new());
    }

    #[test]
    fn a_clause_above_the_function_is_checked_the_same_way() {
        let ok = format!("@ens(result == a + b)\n{ADD}");
        assert_eq!(errors(&ok), Vec::<(String, String)>::new());
        let bad = format!("@req(result > 0)\n{ADD}");
        assert!(errors(&bad).iter().any(|(c, _)| c == "E0561"), "{:?}", errors(&bad));
    }

    #[test]
    fn result_is_typed_as_the_return_type() {
        let src = format!("sp add {{\n    @ens(result == \"five\")\n}}\n{ADD}");
        let errs = errors(&src);
        assert!(errs.iter().any(|(c, m)| c == "E0308" && m.contains("result == \"five\"")), "{errs:?}");
    }

    #[test]
    fn a_contract_must_be_a_bool() {
        let src = format!("sp add {{\n    @ens(result + 1)\n}}\n{ADD}");
        let errs = errors(&src);
        assert!(errs.iter().any(|(c, m)| c == "E0308" && m.contains("must be a `bool`")), "{errs:?}");
    }

    #[test]
    fn result_in_a_precondition_is_e0561() {
        let src = format!("sp add {{\n    @req(result > 0)\n}}\n{ADD}");
        let errs = errors(&src);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert_eq!(errs[0].0, "E0561");
        assert!(errs[0].1.contains("`result` is only in scope inside `@ens`"), "{errs:?}");
    }

    #[test]
    fn old_of_a_non_parameter_is_e0561() {
        let src = format!("sp add {{\n    @ens(result == old(c))\n}}\n{ADD}");
        let errs = errors(&src);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert_eq!(errs[0].0, "E0561");
        assert!(errs[0].1.contains("`c` is not a parameter of `add`"), "{errs:?}");
    }

    #[test]
    fn an_unknown_name_in_a_contract_is_unresolved() {
        let src = format!("sp add {{\n    @ens(result == a + zz)\n}}\n{ADD}");
        let errs = errors(&src);
        assert!(
            errs.iter().any(|(c, m)| c == "E0425" && m.contains("`zz` in contract `@ens(result == a + zz)` of `add`")),
            "{errs:?}"
        );
    }

    #[test]
    fn result_and_old_outside_a_contract_say_where_they_belong() {
        let errs = errors("f h(x: i64) -> i64 { result + old(x) }\n");
        assert!(errs.iter().any(|(c, m)| c == "E0425" && m.contains("`result` names the return value only inside `@ens`")), "{errs:?}");
        assert!(errs.iter().any(|(c, m)| c == "E0425" && m.contains("`old(x)` names a parameter only inside `@ens`")), "{errs:?}");
        // Still ordinary names: a local `result` is just a variable.
        assert_eq!(errors("f h(x: i64) -> i64 {\n    v result = x + 1\n    result\n}\n"), Vec::<(String, String)>::new());
    }

    #[test]
    fn a_spec_with_no_function_keeps_only_the_scope_rule() {
        // framewerx's specs name no function, and write closures.
        assert_eq!(errors("spec S {\n    @ens(|result| result >= 0.0);\n}\n"), Vec::<(String, String)>::new());
        let errs = errors("spec S {\n    @req(result >= 0.0);\n}\n");
        assert_eq!(errs.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(), ["E0561"]);
    }

    // ── Evaluation ───────────────────────────────────────────────

    fn run(src: &str) -> Result<crate::eval::Value, crate::eval::BoundedError> {
        crate::eval::run_bounded(&module(src), "main", vec![], 100_000)
    }

    #[test]
    fn a_postcondition_that_holds_returns_the_value() {
        let src = format!(
            "sp add {{\n    @ens(result == old(a) + b, \"sum\")\n}}\n{ADD}f main() -> i64 {{ add(2, 3) }}\n"
        );
        assert_eq!(run(&src), Ok(crate::eval::Value::Int(5)));
    }

    #[test]
    fn a_violated_postcondition_is_an_error_naming_the_contract() {
        let src = "sp add {\n    @ens(result == a + b, \"sum\")\n}\n\
                   f add(a: i64, b: i64) -> i64 { a - b }\n\
                   f main() -> i64 { add(2, 3) }\n";
        let Err(crate::eval::BoundedError::Error(e)) = run(src) else {
            panic!("a violated postcondition must fail the run, got {:?}", run(src))
        };
        assert_eq!(
            e,
            "contract violated: `@ens(result == a + b, \"sum\")` of `add` (sp add) is false: `add` returned -1"
        );
    }

    #[test]
    fn every_return_is_checked_including_an_early_one() {
        let src = "sp clamp {\n    @ens(result >= 0)\n}\n\
                   f clamp(x: i64) -> i64 {\n    ? x > 100 { ret 0 - 1 }\n    x\n}\n\
                   f main() -> i64 { clamp(5) + clamp(500) }\n";
        let r = run(src);
        assert!(
            matches!(&r, Err(crate::eval::BoundedError::Error(e)) if e.contains("`clamp` returned -1")),
            "{r:?}"
        );
    }
}
