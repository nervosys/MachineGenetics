//! Gradient checking — the inductive half of the differentiability claim.
//!
//! `differentiable.rs` answers a **deductive** question: does this function
//! have a derivative, on every input? This module answers an **inductive** one:
//! is the derivative the compiler computes the *right* one, at the points we
//! tried?
//!
//! They are different claims and neither substitutes for the other. A pass can
//! prove a derivative exists while `eval.rs` computes it wrongly — a sign
//! flipped in the quotient rule is invisible to any amount of static analysis.
//! Conversely, agreement at a thousand points is not proof: the points may all
//! have missed the kink. `DIFFERENTIABILITY.md` sets the pairing out, and
//! `verdict.rs` is where the two verdicts get words that cannot be mistaken for
//! each other.
//!
//! ## The oracle is central differences
//!
//! (f(w+h) − f(w−h)) / 2h, accurate to O(h²). Comparing an exact method against
//! a numerical one is the standard evidence in every AD implementation, and it
//! is *evidence*, not proof — which is why the verdict it produces carries its
//! sample count and refuses to render without it.
//!
//! ## The samples are fixed, not random
//!
//! A random campaign would make the report unreproducible, and a figure nobody
//! can reproduce is the thing this repository keeps removing from its own
//! documents. The points are a fixed spread over positive, negative and
//! near-zero values; `n` is reported so the strength of the evidence is visible
//! rather than implied.
//!
//! Near-zero matters: it is where a kink lives, and where a central difference
//! straddling the kink *should* disagree with the exact one-sided derivative.
//! Such a point is reported as skipped rather than as a refutation — the
//! disagreement is the oracle's limitation, not the compiler's error, and
//! counting it either way would be a lie in one direction or the other.

use crate::ast;
use crate::differentiable::{self, Diff};
use crate::eval::{Interp, Value};
use crate::verdict::{Evidence, Tally};

/// Where the derivative is sampled. Fixed, so the report is reproducible.
/// `0.0` is in the list on purpose: it is where a kink sits, and a sample
/// set that avoids the hard point tests the easy half of the design. Adding
/// it is what showed that the first kink-detection rule reported a correct
/// derivative as refuted.
const SAMPLES: &[f64] = &[-3.25, -1.5, -0.75, -0.125, 0.0, 0.125, 0.75, 1.5, 3.25, 7.5];

/// Step for the central difference. Large enough that `f(w+h) - f(w-h)` does
/// not vanish into floating-point noise, small enough that the O(h²) error
/// stays well under the tolerance.
const H: f64 = 1e-6;

/// Relative tolerance between the exact derivative and the central difference.
const TOL: f64 = 1e-4;

/// What gradient checking found for one function.
#[derive(Debug, Clone)]
pub struct Checked {
    pub name: String,
    /// The deductive claim: does a derivative exist at all?
    pub deductive: Evidence,
    /// The inductive claim: is the computed one right, where we looked?
    pub inductive: Evidence,
    /// Points that were sampled and agreed.
    pub agreed: usize,
    /// Points skipped, with the reason — a non-finite value, or a kink the
    /// central difference straddles.
    pub skipped: Vec<(f64, String)>,
}

/// Gradient-check every function in the module that has something to check.
///
/// The subject is a function of exactly one continuous parameter whose body is
/// a `grad(e, w)` over that parameter: that is a *derivative implementation*,
/// and the primal `e` is right there to difference against. A function that is
/// not one of those is reported `Unreached` rather than skipped — there is no
/// verdict, and saying so is the whole discipline this module is built around.
pub fn check_module(module: &ast::Module, effects: &crate::effects::EffectInfer) -> Vec<Checked> {
    let engine = differentiable::infer(module, effects);
    let mut out = Vec::new();

    for item in &module.items {
        let ast::ItemKind::Function(fd) = &item.kind else {
            continue;
        };
        let deductive = match engine.diff_of(&fd.name) {
            Diff::Smooth => Evidence::Proved {
                by: "differentiability inference (smooth)".into(),
            },
            Diff::AlmostEverywhere => Evidence::Proved {
                by: "differentiability inference (almost everywhere)".into(),
            },
            Diff::Unknown(why) => Evidence::Unreached { why },
            Diff::No(why) => Evidence::Refuted { at: why },
        };

        let Some((param, primal)) = gradient_body(fd) else {
            out.push(Checked {
                name: fd.name.clone(),
                deductive,
                inductive: Evidence::Unreached {
                    why: "not a derivative implementation: the body is not `grad(e, w)` \
                          over this function's own parameter, so there is nothing to \
                          difference against"
                        .into(),
                },
                agreed: 0,
                skipped: Vec::new(),
            });
            continue;
        };

        // `param` is not passed on: `gradient_body` has already checked
        // that the `grad` differentiates this function's own parameter,
        // which is the only thing `check_one` would use it for.
        let _ = &param;
        let (inductive, agreed, skipped) = check_one(module, fd, &primal);
        out.push(Checked {
            name: fd.name.clone(),
            deductive,
            inductive,
            agreed,
            skipped,
        });
    }
    out
}

/// If `fd` is `f g(w: f64) -> f64 { grad(e, w) }`, return `(w, e)`.
///
/// The `wrt` has to be the function's *own* parameter. `grad(e, k)` over some
/// captured constant is a valid expression and a derivative of zero; it is not
/// a derivative implementation of `g`, and differencing `g` against `e` would
/// be comparing two unrelated functions.
fn gradient_body(fd: &ast::FunctionDef) -> Option<(String, ast::Expr)> {
    if fd.params.len() != 1 {
        return None;
    }
    let param = fd.params[0].name.clone();
    let body = fd
        .body_expr
        .as_deref()
        .or(fd.body.tail_expr.as_deref())?;
    let ast::Expr::Grad { value, wrt } = body else {
        return None;
    };
    match wrt.as_ref() {
        ast::Expr::Ident { name } if *name == param => Some((param, (**value).clone())),
        _ => None,
    }
}

/// Compare the compiled derivative against a central difference of the primal.
fn check_one(
    module: &ast::Module,
    fd: &ast::FunctionDef,
    primal: &ast::Expr,
) -> (Evidence, usize, Vec<(f64, String)>) {
    // Two interpreters over two modules: the original, whose `fd` returns the
    // derivative, and a rewritten one whose `fd` returns the primal. Rewriting
    // the module rather than evaluating the expression directly means the
    // primal is evaluated by exactly the path the derivative was — same scope
    // rules, same builtins, same everything but the `grad`.
    let derivative = Interp::new(module);
    let primal_module = with_body(module, &fd.name, primal.clone());
    let primal_interp = Interp::new(&primal_module);

    let mut agreed = 0usize;
    let mut skipped = Vec::new();
    let mut refuted: Option<String> = None;

    for &w in SAMPLES {
        let exact = match as_f64(derivative.run(&fd.name, vec![Value::Float(w)])) {
            Some(v) if v.is_finite() => v,
            _ => {
                skipped.push((w, "the derivative did not evaluate to a finite number".into()));
                continue;
            }
        };
        let hi = as_f64(primal_interp.run(&fd.name, vec![Value::Float(w + H)]));
        let lo = as_f64(primal_interp.run(&fd.name, vec![Value::Float(w - H)]));
        let (Some(hi), Some(lo)) = (hi, lo) else {
            skipped.push((w, "the primal did not evaluate to a number".into()));
            continue;
        };
        if !hi.is_finite() || !lo.is_finite() {
            skipped.push((w, "the primal is not finite here".into()));
            continue;
        }
        let scale = exact.abs().max(1.0);

        // **Detect the kink from the primal, before comparing anything.**
        //
        // A central difference straddling a kink averages two different
        // one-sided slopes, so it disagrees with *any* correct derivative. That
        // is the oracle's limitation, not the compiler's error, and at exactly
        // the points `AlmostEverywhere` exists to describe.
        //
        // The first version of this asked whether the exact answer matched the
        // right-hand slope, which is the wrong question in two ways: it assumes
        // the answer it is trying to check, and it still reports a false
        // refutation whenever the convention picks the *other* side. A
        // step-like `? w > 0.0 { w * 3.0 } : { 0.0 }` at `w = 0` has exact
        // derivative 0 (the branch taken), right slope 3 and left slope 0 — so
        // that version called a correct derivative wrong. Asking the primal
        // whether its two one-sided slopes agree needs no such assumption.
        let mid = match as_f64(primal_interp.run(&fd.name, vec![Value::Float(w)])) {
            Some(v) if v.is_finite() => v,
            _ => {
                skipped.push((w, "the primal is not finite here".into()));
                continue;
            }
        };
        let left = (mid - lo) / H;
        let right = (hi - mid) / H;
        if (left - right).abs() > 1e-3 * left.abs().max(right.abs()).max(1.0) {
            skipped.push((
                w,
                format!(
                    "a kink lies inside the difference window (left slope {left}, \
                     right slope {right}), so a central difference cannot adjudicate here"
                ),
            ));
            continue;
        }

        let fd_approx = (hi - lo) / (2.0 * H);
        if (exact - fd_approx).abs() <= TOL * scale {
            agreed += 1;
        } else {
            refuted = Some(format!(
                "{}({w}) = {exact}, central difference = {fd_approx}",
                fd.name
            ));
            break;
        }
    }

    let inductive = match refuted {
        Some(at) => Evidence::Refuted { at },
        None if agreed == 0 => Evidence::Unreached {
            why: "every sample point was skipped, so nothing was compared".into(),
        },
        None => Evidence::Evidenced {
            n: agreed,
            by: "central differences".into(),
        },
    };
    (inductive, agreed, skipped)
}

fn as_f64(r: Result<Value, String>) -> Option<f64> {
    match r {
        Ok(Value::Float(f)) => Some(f),
        Ok(Value::Int(n)) => Some(n as f64),
        _ => None,
    }
}

/// A copy of `module` in which `name`'s body is `body`.
fn with_body(module: &ast::Module, name: &str, body: ast::Expr) -> ast::Module {
    let mut m = module.clone();
    for item in &mut m.items {
        if let ast::ItemKind::Function(fd) = &mut item.kind
            && fd.name == name
        {
            fd.body_expr = None;
            fd.body = ast::Block {
                stmts: Vec::new(),
                tail_expr: Some(Box::new(body)),
            };
            break;
        }
    }
    m
}

// ── Reporting ────────────────────────────────────────────────────────

pub fn tally(rows: &[Checked]) -> (Tally, Tally) {
    let mut deductive = Tally::default();
    let mut inductive = Tally::default();
    for r in rows {
        deductive.add(&r.deductive);
        inductive.add(&r.inductive);
    }
    (deductive, inductive)
}

/// A deterministic report, with the two claims side by side and never merged.
pub fn report(rows: &[Checked], path: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("// gradient check — {path}\n"));
    for r in rows {
        s.push_str(&format!("{}\n", r.name));
        s.push_str(&format!("    deductive  {}\n", r.deductive.describe()));
        s.push_str(&format!("    inductive  {}\n", r.inductive.describe()));
        for (w, why) in &r.skipped {
            s.push_str(&format!("      skipped at {w}: {why}\n"));
        }
    }
    let (d, i) = tally(rows);
    s.push_str(&format!("deductive: {}\n", d.summary()));
    s.push_str(&format!("inductive: {}\n", i.summary()));
    if rows.is_empty() {
        s.push_str("nothing to check: no function in this module\n");
    }
    s
}

pub fn report_json(rows: &[Checked], path: &str) -> serde_json::Value {
    let (d, i) = tally(rows);
    serde_json::json!({
        "path": path,
        "samples": SAMPLES,
        "step": H,
        "relative_tolerance": TOL,
        "functions": rows.iter().map(|r| serde_json::json!({
            "name": r.name,
            "deductive": { "verdict": r.deductive.label(), "detail": r.deductive.describe() },
            "inductive": { "verdict": r.inductive.label(), "detail": r.inductive.describe() },
            "agreed": r.agreed,
            "skipped": r.skipped.iter().map(|(w, why)| serde_json::json!({
                "at": w, "why": why,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "summary": {
            "deductive": d.summary(),
            "inductive": i.summary(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{effects, lexer, parser};

    fn check(src: &str) -> Vec<Checked> {
        let tokens = lexer::lex(src);
        let module = parser::parse(&tokens).expect("parse failed");
        let eff = effects::infer_effects(&module);
        check_module(&module, &eff)
    }

    fn one(src: &str) -> Checked {
        check(src).into_iter().next().expect("one function")
    }

    /// A correct derivative is evidenced at every sample, and the verdict
    /// carries the count. It is **not** `Proved`: sampling cannot prove.
    ///
    /// The deductive verdict on the same function is `Unreached`, and that is
    /// right rather than a gap in the test. The *body* of `d` is a `grad`, so
    /// asking whether `d` is differentiable asks for a **second** derivative —
    /// which `differentiable.rs` does not analyse and says so. Two honest
    /// verdicts about two different questions, on one function, neither
    /// standing in for the other. This is the pairing the module exists for,
    /// and it took a failing assertion of mine to notice it was the
    /// interesting case rather than the boring one.
    #[test]
    fn a_correct_derivative_is_evidenced_and_its_own_slope_is_unreached() {
        let c = one("f d(w: f64) -> f64 { grad(w * w * 3.0 + w * 2.0, w) }");
        assert!(
            matches!(c.inductive, Evidence::Evidenced { n, .. } if n == SAMPLES.len()),
            "{:?}",
            c.inductive
        );
        assert!(!c.inductive.is_deductive(), "sampling is not proof");
        assert!(
            matches!(c.deductive, Evidence::Unreached { .. }),
            "the differentiability of a derivative is a second derivative, \
             which this pass does not analyse: {:?}",
            c.deductive
        );
        assert!(!c.deductive.holds(), "an unreached claim is not a hold");
    }

    /// The two claims are about different things and are reported separately.
    /// A function can have a proved derivative and no inductive evidence at
    /// all, and that combination has to be visible rather than averaged.
    #[test]
    fn the_two_claims_are_kept_apart() {
        let c = one("f plain(w: f64) -> f64 { w * 2.0 }");
        assert!(c.deductive.is_deductive(), "it is differentiable: {:?}", c.deductive);
        assert!(
            matches!(c.inductive, Evidence::Unreached { .. }),
            "nothing computed a derivative here to check: {:?}",
            c.inductive
        );
        assert!(!c.inductive.holds(), "an unreached claim is not a hold");
    }

    /// The quotient rule is where a plausible-but-wrong implementation gives a
    /// close answer, which is exactly what a numerical oracle catches and
    /// reading the code does not.
    #[test]
    fn the_quotient_and_product_rules_are_evidenced() {
        for body in [
            "(w * w + 1.0) / (w + 11.0)",
            "(w + 1.0) * (w * 2.0 - 3.0)",
            "w / 4.0 + 1.0",
        ] {
            let c = one(&format!("f d(w: f64) -> f64 {{ grad({body}, w) }}"));
            assert!(
                matches!(c.inductive, Evidence::Evidenced { .. }),
                "{body}: {:?}",
                c.inductive
            );
        }
    }

    /// **A kink is skipped with its reason, never refuted.** The central
    /// difference across one averages two different slopes and disagrees with
    /// any correct derivative; that is the oracle's limitation, not the
    /// compiler's error.
    ///
    /// The step function here is the case that broke the first implementation.
    /// At `w = 0` the branch taken is the `else`, so the exact derivative is 0,
    /// while the right-hand slope is 3 and the left-hand slope is 0. A rule
    /// that asked "does the exact answer match the right-hand slope?" called
    /// that a wrong derivative. Asking the *primal* whether its two one-sided
    /// slopes agree needs no assumption about the answer.
    #[test]
    fn a_kink_is_skipped_with_its_reason_rather_than_refuted() {
        for body in ["abs(w) * 2.0", "? w > 0.0 { w * 3.0 } : { 0.0 }"] {
            let c = one(&format!("f d(w: f64) -> f64 {{ grad({body}, w) }}"));
            assert!(
                !matches!(c.inductive, Evidence::Refuted { .. }),
                "{body}: a kink must not read as a wrong derivative: {:?}",
                c.inductive
            );
            assert!(
                matches!(c.inductive, Evidence::Evidenced { .. }),
                "{body}: the smooth samples still count: {:?}",
                c.inductive
            );
        }
        // And the skip is *named*, not silent: the report says where and why.
        let c = one("f d(w: f64) -> f64 { grad(? w > 0.0 { w * 3.0 } : { 0.0 }, w) }");
        assert!(
            c.skipped.iter().any(|(at, why)| *at == 0.0 && why.contains("kink")),
            "the kink at 0 must be reported: {:?}",
            c.skipped
        );
    }

    /// A function that is not differentiable at all gets a refuted *deductive*
    /// verdict, and its inductive verdict does not quietly become a pass.
    #[test]
    fn a_non_differentiable_function_is_refuted_deductively() {
        let c = one("f d(w: f64) -> f64 { w > 1.0 }");
        assert!(matches!(c.deductive, Evidence::Refuted { .. }), "{:?}", c.deductive);
        assert!(!c.inductive.holds(), "{:?}", c.inductive);
    }

    /// The report is deterministic: fixed sample points, no RNG. A figure
    /// nobody can reproduce is the thing this repository keeps deleting.
    #[test]
    fn the_report_is_deterministic() {
        let src = "f d(w: f64) -> f64 { grad(w * w, w) }";
        assert_eq!(report(&check(src), "t.mg"), report(&check(src), "t.mg"));
    }
}
