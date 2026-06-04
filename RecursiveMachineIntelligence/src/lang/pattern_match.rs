//! Pattern matching and rewriting engine for RMIL expressions.
//!
//! Provides structural patterns that can match against [`Expr`] trees, bind
//! sub-expressions to named slots, and produce rewritten expressions from
//! templates. The engine applies rules bottom-up to a fixed point.
//!
//! # Examples
//!
//! ```
//! use rmi::lang::pattern_match::*;
//! use rmi::lang::{Expr, Op};
//!
//! // Rule: identity >> X  ==>  X
//! let rule = RewriteRule::new(
//!     "identity-left",
//!     Pat::seq(Pat::op0(Op::IDENTITY), Pat::var("x")),
//!     Tmpl::var("x"),
//! );
//!
//! let expr = Expr::op0(Op::IDENTITY) >> Expr::op1(Op::RELU);
//! let result = rule.try_apply(&expr).unwrap();
//! assert_eq!(result, Expr::op1(Op::RELU));
//! ```

use crate::lang::expr::{Expr, Val};
use crate::lang::op::Op;
use std::collections::HashMap;

// ── Pattern ──────────────────────────────────────────────────────────────────

/// A pattern that matches structural shapes in an `Expr` tree.
///
/// Patterns form a small language:
/// - `Var(name)` — matches any expression, binding it to `name`
/// - `Any` — matches any expression, discarding the match
/// - `Lit(val)` — matches an exact literal value
/// - `OpApp(op, sub-patterns)` — matches `Expr::App(op, args)` with position sub-patterns
/// - `Op0(op)` — matches a zero-arg op application `Expr::App(op, [])`
/// - `Seq(l, r)` — matches `Expr::Seq(l, r)`
/// - `Par(l, r)` — matches `Expr::Par(l, r)`
/// - `Cond(p, y, n)` — matches `Expr::Cond { pred, yes, no }`
#[derive(Debug, Clone)]
pub enum Pat {
    /// Match anything, bind to named slot.
    Var(String),
    /// Match anything, discard.
    Any,
    /// Match an exact literal value.
    Lit(Val),
    /// Match `Expr::App(op, args)` with sub-patterns on args.
    OpApp(Op, Vec<Pat>),
    /// Match a zero-arg op exactly (sugar for `OpApp(op, [])`).
    Op0(Op),
    /// Match `Expr::Seq(l, r)`.
    Seq(Box<Pat>, Box<Pat>),
    /// Match `Expr::Par(l, r)`.
    Par(Box<Pat>, Box<Pat>),
    /// Match `Expr::Cond { pred, yes, no }`.
    Cond(Box<Pat>, Box<Pat>, Box<Pat>),
    /// Match either alternative (first match wins).
    Alt(Box<Pat>, Box<Pat>),
}

impl Pat {
    /// Named wildcard — captures the matched sub-expression.
    pub fn var(name: &str) -> Self {
        Pat::Var(name.to_string())
    }

    /// Zero-arg op pattern.
    pub fn op0(op: Op) -> Self {
        Pat::Op0(op)
    }

    /// N-arg op pattern.
    pub fn op(op: Op, args: Vec<Pat>) -> Self {
        Pat::OpApp(op, args)
    }

    /// Sequential composition pattern.
    pub fn seq(l: Pat, r: Pat) -> Self {
        Pat::Seq(Box::new(l), Box::new(r))
    }

    /// Parallel composition pattern.
    pub fn par(l: Pat, r: Pat) -> Self {
        Pat::Par(Box::new(l), Box::new(r))
    }

    /// Conditional pattern.
    pub fn cond(p: Pat, y: Pat, n: Pat) -> Self {
        Pat::Cond(Box::new(p), Box::new(y), Box::new(n))
    }

    /// Either/or pattern (first match wins).
    pub fn alt(a: Pat, b: Pat) -> Self {
        Pat::Alt(Box::new(a), Box::new(b))
    }

    /// Literal match.
    pub fn lit(v: Val) -> Self {
        Pat::Lit(v)
    }
}

/// Binding environment from a successful pattern match.
pub type Bindings = HashMap<String, Expr>;

/// Attempt to match `pat` against `expr`, returning bindings on success.
pub fn match_pat(pat: &Pat, expr: &Expr) -> Option<Bindings> {
    let mut bindings = Bindings::new();
    if match_inner(pat, expr, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

fn match_inner(pat: &Pat, expr: &Expr, bindings: &mut Bindings) -> bool {
    match pat {
        Pat::Var(name) => {
            if let Some(existing) = bindings.get(name) {
                // If already bound, check equality
                *existing == *expr
            } else {
                bindings.insert(name.clone(), expr.clone());
                true
            }
        }
        Pat::Any => true,
        Pat::Lit(v) => matches!(expr, Expr::Lit(ev) if ev == v),
        Pat::Op0(op) => matches!(expr, Expr::App(eop, args) if eop == op && args.is_empty()),
        Pat::OpApp(op, sub_pats) => {
            if let Expr::App(eop, args) = expr {
                if eop != op || args.len() != sub_pats.len() {
                    return false;
                }
                for (sp, arg) in sub_pats.iter().zip(args.iter()) {
                    if !match_inner(sp, arg, bindings) {
                        return false;
                    }
                }
                true
            } else {
                false
            }
        }
        Pat::Seq(lp, rp) => {
            if let Expr::Seq(le, re) = expr {
                match_inner(lp, le, bindings) && match_inner(rp, re, bindings)
            } else {
                false
            }
        }
        Pat::Par(lp, rp) => {
            if let Expr::Par(le, re) = expr {
                match_inner(lp, le, bindings) && match_inner(rp, re, bindings)
            } else {
                false
            }
        }
        Pat::Cond(pp, yp, np) => {
            if let Expr::Cond { pred, yes, no } = expr {
                match_inner(pp, pred, bindings)
                    && match_inner(yp, yes, bindings)
                    && match_inner(np, no, bindings)
            } else {
                false
            }
        }
        Pat::Alt(a, b) => {
            let mut a_bindings = bindings.clone();
            if match_inner(a, expr, &mut a_bindings) {
                *bindings = a_bindings;
                true
            } else {
                match_inner(b, expr, bindings)
            }
        }
    }
}

// ── Template ─────────────────────────────────────────────────────────────────

/// A template for constructing an output expression from pattern bindings.
#[derive(Debug, Clone)]
pub enum Tmpl {
    /// Substitute a named binding from the pattern match.
    Var(String),
    /// A literal value.
    Lit(Val),
    /// An op application with template arguments.
    OpApp(Op, Vec<Tmpl>),
    /// A zero-arg op.
    Op0(Op),
    /// Sequential composition.
    Seq(Box<Tmpl>, Box<Tmpl>),
    /// Parallel composition.
    Par(Box<Tmpl>, Box<Tmpl>),
    /// Conditional.
    Cond(Box<Tmpl>, Box<Tmpl>, Box<Tmpl>),
}

impl Tmpl {
    /// Named variable reference.
    pub fn var(name: &str) -> Self {
        Tmpl::Var(name.to_string())
    }

    /// Zero-arg op.
    pub fn op0(op: Op) -> Self {
        Tmpl::Op0(op)
    }

    /// N-arg op.
    pub fn op(op: Op, args: Vec<Tmpl>) -> Self {
        Tmpl::OpApp(op, args)
    }

    /// Sequential composition.
    pub fn seq(l: Tmpl, r: Tmpl) -> Self {
        Tmpl::Seq(Box::new(l), Box::new(r))
    }

    /// Parallel composition.
    pub fn par(l: Tmpl, r: Tmpl) -> Self {
        Tmpl::Par(Box::new(l), Box::new(r))
    }

    /// Literal template.
    pub fn lit(v: Val) -> Self {
        Tmpl::Lit(v)
    }

    /// Instantiate this template with bindings from a pattern match.
    pub fn instantiate(&self, bindings: &Bindings) -> Option<Expr> {
        match self {
            Tmpl::Var(name) => bindings.get(name).cloned(),
            Tmpl::Lit(v) => Some(Expr::Lit(v.clone())),
            Tmpl::Op0(op) => Some(Expr::App(*op, vec![])),
            Tmpl::OpApp(op, args) => {
                let mut out = Vec::with_capacity(args.len());
                for a in args {
                    out.push(a.instantiate(bindings)?);
                }
                Some(Expr::App(*op, out))
            }
            Tmpl::Seq(l, r) => {
                let le = l.instantiate(bindings)?;
                let re = r.instantiate(bindings)?;
                Some(Expr::Seq(Box::new(le), Box::new(re)))
            }
            Tmpl::Par(l, r) => {
                let le = l.instantiate(bindings)?;
                let re = r.instantiate(bindings)?;
                Some(Expr::Par(Box::new(le), Box::new(re)))
            }
            Tmpl::Cond(p, y, n) => {
                let pe = p.instantiate(bindings)?;
                let ye = y.instantiate(bindings)?;
                let ne = n.instantiate(bindings)?;
                Some(Expr::Cond {
                    pred: Box::new(pe),
                    yes: Box::new(ye),
                    no: Box::new(ne),
                })
            }
        }
    }
}

// ── Rewrite Rule ─────────────────────────────────────────────────────────────

/// A named rewrite rule: pattern → template.
#[derive(Debug, Clone)]
pub struct RewriteRule {
    /// Human-readable name for diagnostics.
    pub name: String,
    /// Pattern to match.
    pub pattern: Pat,
    /// Template to produce.
    pub template: Tmpl,
}

impl RewriteRule {
    /// Create a new rewrite rule.
    pub fn new(name: &str, pattern: Pat, template: Tmpl) -> Self {
        Self {
            name: name.to_string(),
            pattern,
            template,
        }
    }

    /// Try to apply this rule at the root of `expr`.
    pub fn try_apply(&self, expr: &Expr) -> Option<Expr> {
        let bindings = match_pat(&self.pattern, expr)?;
        self.template.instantiate(&bindings)
    }
}

// ── Rewriter ─────────────────────────────────────────────────────────────────

/// Applies a set of rewrite rules bottom-up to a fixed point.
///
/// The rewriter traverses the expression tree bottom-up, attempting each
/// rule at every node. It repeats until no more rules fire or a maximum
/// iteration count is reached.
pub struct Rewriter {
    /// Ordered rules (earlier rules have higher priority).
    pub rules: Vec<RewriteRule>,
    /// Maximum rewrite iterations (prevents infinite loops).
    pub max_iterations: usize,
}

impl Rewriter {
    /// Create a rewriter with default iteration limit (100).
    pub fn new(rules: Vec<RewriteRule>) -> Self {
        Self {
            rules,
            max_iterations: 100,
        }
    }

    /// Create an empty rewriter.
    pub fn empty() -> Self {
        Self::new(vec![])
    }

    /// Add a rule.
    pub fn add_rule(&mut self, rule: RewriteRule) {
        self.rules.push(rule);
    }

    /// Rewrite an expression to a fixed point.
    ///
    /// Returns the rewritten expression and the number of rule applications.
    pub fn rewrite(&self, expr: &Expr) -> (Expr, usize) {
        let mut current = expr.clone();
        let mut total_applications = 0;

        for _ in 0..self.max_iterations {
            let (next, apps) = self.rewrite_once(&current);
            total_applications += apps;
            if apps == 0 {
                break; // fixed point reached
            }
            current = next;
        }

        (current, total_applications)
    }

    /// One bottom-up pass over the tree.
    fn rewrite_once(&self, expr: &Expr) -> (Expr, usize) {
        let mut apps = 0;

        // First, recursively rewrite children
        let rebuilt = match expr {
            Expr::Lit(_) | Expr::Ref(_) => expr.clone(),

            Expr::App(op, args) => {
                let mut new_args = Vec::with_capacity(args.len());
                for a in args {
                    let (new_a, a_apps) = self.rewrite_once(a);
                    apps += a_apps;
                    new_args.push(new_a);
                }
                Expr::App(*op, new_args)
            }

            Expr::Seq(l, r) => {
                let (nl, la) = self.rewrite_once(l);
                let (nr, ra) = self.rewrite_once(r);
                apps += la + ra;
                Expr::Seq(Box::new(nl), Box::new(nr))
            }

            Expr::Par(l, r) => {
                let (nl, la) = self.rewrite_once(l);
                let (nr, ra) = self.rewrite_once(r);
                apps += la + ra;
                Expr::Par(Box::new(nl), Box::new(nr))
            }

            Expr::Cond { pred, yes, no } => {
                let (np, pa) = self.rewrite_once(pred);
                let (ny, ya) = self.rewrite_once(yes);
                let (nn, na) = self.rewrite_once(no);
                apps += pa + ya + na;
                Expr::Cond {
                    pred: Box::new(np),
                    yes: Box::new(ny),
                    no: Box::new(nn),
                }
            }

            Expr::Let { name, val, body } => {
                let (nv, va) = self.rewrite_once(val);
                let (nb, ba) = self.rewrite_once(body);
                apps += va + ba;
                Expr::Let {
                    name: *name,
                    val: Box::new(nv),
                    body: Box::new(nb),
                }
            }

            Expr::Lam { params, body } => {
                let (nb, ba) = self.rewrite_once(body);
                apps += ba;
                Expr::Lam {
                    params: params.clone(),
                    body: Box::new(nb),
                }
            }

            Expr::Call(f, args) => {
                let (nf, fa) = self.rewrite_once(f);
                apps += fa;
                let mut new_args = Vec::with_capacity(args.len());
                for a in args {
                    let (na, aa) = self.rewrite_once(a);
                    apps += aa;
                    new_args.push(na);
                }
                Expr::Call(Box::new(nf), new_args)
            }

            Expr::Block(exprs) => {
                let mut new_exprs = Vec::with_capacity(exprs.len());
                for e in exprs {
                    let (ne, ea) = self.rewrite_once(e);
                    apps += ea;
                    new_exprs.push(ne);
                }
                Expr::Block(new_exprs)
            }
        };

        // Then try to apply rules at this node
        for rule in &self.rules {
            if let Some(rewritten) = rule.try_apply(&rebuilt) {
                return (rewritten, apps + 1);
            }
        }

        (rebuilt, apps)
    }
}

// ── Built-in rules ──────────────────────────────────────────────────────────

/// Standard RMIL rewrite rules covering identity elimination, idempotent ops,
/// constant folding, and common algebraic simplifications.
pub fn standard_rules() -> Vec<RewriteRule> {
    vec![
        // Identity elimination
        RewriteRule::new(
            "identity-left",
            Pat::seq(Pat::op0(Op::IDENTITY), Pat::var("x")),
            Tmpl::var("x"),
        ),
        RewriteRule::new(
            "identity-right",
            Pat::seq(Pat::var("x"), Pat::op0(Op::IDENTITY)),
            Tmpl::var("x"),
        ),
        // relu(relu(x)) = relu(x)  (idempotent)
        RewriteRule::new(
            "relu-idempotent",
            Pat::seq(Pat::op0(Op::RELU), Pat::op0(Op::RELU)),
            Tmpl::op0(Op::RELU),
        ),
        // drop >> drop = drop  (idempotent)
        RewriteRule::new(
            "drop-idempotent",
            Pat::seq(Pat::op0(Op::DROP), Pat::op0(Op::DROP)),
            Tmpl::op0(Op::DROP),
        ),
        // layer_norm >> layer_norm = layer_norm  (idempotent)
        RewriteRule::new(
            "layer-norm-idempotent",
            Pat::seq(Pat::op0(Op::LAYER_NORM), Pat::op0(Op::LAYER_NORM)),
            Tmpl::op0(Op::LAYER_NORM),
        ),
        // softmax >> softmax = softmax  (idempotent)
        RewriteRule::new(
            "softmax-idempotent",
            Pat::seq(Pat::op0(Op::SOFTMAX), Pat::op0(Op::SOFTMAX)),
            Tmpl::op0(Op::SOFTMAX),
        ),
        // if true then x else y => x
        RewriteRule::new(
            "cond-true",
            Pat::cond(Pat::lit(Val::Bool(true)), Pat::var("x"), Pat::Any),
            Tmpl::var("x"),
        ),
        // if false then x else y => y
        RewriteRule::new(
            "cond-false",
            Pat::cond(Pat::lit(Val::Bool(false)), Pat::Any, Pat::var("y")),
            Tmpl::var("y"),
        ),
        // res_add(identity, x) = x  (trivial residual)
        RewriteRule::new(
            "trivial-residual",
            Pat::op(Op::RES_ADD, vec![Pat::op0(Op::IDENTITY), Pat::var("x")]),
            Tmpl::var("x"),
        ),
    ]
}

/// Create a `Rewriter` loaded with `standard_rules`.
pub fn standard_rewriter() -> Rewriter {
    Rewriter::new(standard_rules())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_wildcard() {
        let pat = Pat::var("x");
        let expr = Expr::int(42);
        let bindings = match_pat(&pat, &expr).unwrap();
        assert_eq!(bindings["x"], Expr::int(42));
    }

    #[test]
    fn match_any() {
        let pat = Pat::Any;
        assert!(match_pat(&pat, &Expr::int(1)).is_some());
        assert!(match_pat(&pat, &Expr::boolean(true)).is_some());
    }

    #[test]
    fn match_literal() {
        let pat = Pat::lit(Val::I64(42));
        assert!(match_pat(&pat, &Expr::int(42)).is_some());
        assert!(match_pat(&pat, &Expr::int(43)).is_none());
    }

    #[test]
    fn match_op0() {
        let pat = Pat::op0(Op::RELU);
        assert!(match_pat(&pat, &Expr::op0(Op::RELU)).is_some());
        assert!(match_pat(&pat, &Expr::op0(Op::GELU)).is_none());
        assert!(match_pat(&pat, &Expr::op1(Op::RELU)).is_some());
    }

    #[test]
    fn match_op_app_with_args() {
        let pat = Pat::op(Op::ADD, vec![Pat::var("a"), Pat::var("b")]);
        let expr = Expr::op2(Op::ADD, Expr::int(1), Expr::int(2));
        let bindings = match_pat(&pat, &expr).unwrap();
        assert_eq!(bindings["a"], Expr::int(1));
        assert_eq!(bindings["b"], Expr::int(2));
    }

    #[test]
    fn match_seq() {
        let pat = Pat::seq(Pat::var("a"), Pat::var("b"));
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        let bindings = match_pat(&pat, &expr).unwrap();
        assert_eq!(bindings["a"], Expr::op1(Op::RELU));
        assert_eq!(bindings["b"], Expr::op1(Op::LINEAR));
    }

    #[test]
    fn match_par() {
        let pat = Pat::par(Pat::op0(Op::RELU), Pat::op0(Op::SIGMOID));
        let expr = Expr::op1(Op::RELU) | Expr::op1(Op::SIGMOID);
        assert!(match_pat(&pat, &expr).is_some());
    }

    #[test]
    fn match_cond() {
        let pat = Pat::cond(Pat::var("p"), Pat::var("y"), Pat::var("n"));
        let expr = Expr::Cond {
            pred: Box::new(Expr::boolean(true)),
            yes: Box::new(Expr::int(1)),
            no: Box::new(Expr::int(0)),
        };
        let bindings = match_pat(&pat, &expr).unwrap();
        assert_eq!(bindings["p"], Expr::boolean(true));
    }

    #[test]
    fn match_alt() {
        let pat = Pat::alt(Pat::op0(Op::RELU), Pat::op0(Op::GELU));
        assert!(match_pat(&pat, &Expr::op1(Op::RELU)).is_some());
        assert!(match_pat(&pat, &Expr::op1(Op::GELU)).is_some());
        assert!(match_pat(&pat, &Expr::op1(Op::LINEAR)).is_none());
    }

    #[test]
    fn match_repeated_var_same_value() {
        let pat = Pat::seq(Pat::var("x"), Pat::var("x"));
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::RELU);
        assert!(match_pat(&pat, &expr).is_some());
    }

    #[test]
    fn match_repeated_var_different_value() {
        let pat = Pat::seq(Pat::var("x"), Pat::var("x"));
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        assert!(match_pat(&pat, &expr).is_none());
    }

    #[test]
    fn template_instantiate() {
        let tmpl = Tmpl::seq(Tmpl::var("b"), Tmpl::var("a"));
        let mut bindings = Bindings::new();
        bindings.insert("a".into(), Expr::op1(Op::RELU));
        bindings.insert("b".into(), Expr::op1(Op::LINEAR));
        let result = tmpl.instantiate(&bindings).unwrap();
        let expected = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        assert_eq!(result, expected);
    }

    #[test]
    fn template_missing_var() {
        let tmpl = Tmpl::var("missing");
        let bindings = Bindings::new();
        assert!(tmpl.instantiate(&bindings).is_none());
    }

    #[test]
    fn rule_identity_left() {
        let rule = RewriteRule::new(
            "id-left",
            Pat::seq(Pat::op0(Op::IDENTITY), Pat::var("x")),
            Tmpl::var("x"),
        );
        let expr = Expr::op0(Op::IDENTITY) >> Expr::op1(Op::RELU);
        let result = rule.try_apply(&expr).unwrap();
        assert_eq!(result, Expr::op1(Op::RELU));
    }

    #[test]
    fn rule_identity_right() {
        let rule = RewriteRule::new(
            "id-right",
            Pat::seq(Pat::var("x"), Pat::op0(Op::IDENTITY)),
            Tmpl::var("x"),
        );
        let expr = Expr::op1(Op::RELU) >> Expr::op0(Op::IDENTITY);
        let result = rule.try_apply(&expr).unwrap();
        assert_eq!(result, Expr::op1(Op::RELU));
    }

    #[test]
    fn rewriter_identity_elimination() {
        let rw = standard_rewriter();
        let expr = Expr::op0(Op::IDENTITY) >> Expr::op1(Op::RELU) >> Expr::op0(Op::IDENTITY);
        let (result, apps) = rw.rewrite(&expr);
        assert!(apps > 0);
        assert_eq!(result, Expr::op1(Op::RELU));
    }

    #[test]
    fn rewriter_relu_idempotent() {
        let rw = standard_rewriter();
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::RELU) >> Expr::op1(Op::RELU);
        let (result, apps) = rw.rewrite(&expr);
        assert!(apps > 0);
        assert_eq!(result.node_count(), 1); // collapsed to single relu
    }

    #[test]
    fn rewriter_cond_true() {
        let rw = standard_rewriter();
        let expr = Expr::Cond {
            pred: Box::new(Expr::boolean(true)),
            yes: Box::new(Expr::op1(Op::RELU)),
            no: Box::new(Expr::op1(Op::GELU)),
        };
        let (result, apps) = rw.rewrite(&expr);
        assert!(apps > 0);
        assert_eq!(result, Expr::op1(Op::RELU));
    }

    #[test]
    fn rewriter_cond_false() {
        let rw = standard_rewriter();
        let expr = Expr::Cond {
            pred: Box::new(Expr::boolean(false)),
            yes: Box::new(Expr::op1(Op::RELU)),
            no: Box::new(Expr::op1(Op::GELU)),
        };
        let (result, apps) = rw.rewrite(&expr);
        assert!(apps > 0);
        assert_eq!(result, Expr::op1(Op::GELU));
    }

    #[test]
    fn rewriter_fixed_point() {
        let rw = standard_rewriter();
        // Already optimal — no rules should fire
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        let (result, apps) = rw.rewrite(&expr);
        assert_eq!(apps, 0);
        assert_eq!(result, expr);
    }

    #[test]
    fn rewriter_deep_tree() {
        let rw = standard_rewriter();
        // identity >> (identity >> (relu >> identity))
        let expr = Expr::op0(Op::IDENTITY)
            >> (Expr::op0(Op::IDENTITY) >> (Expr::op1(Op::RELU) >> Expr::op0(Op::IDENTITY)));
        let (result, apps) = rw.rewrite(&expr);
        assert!(apps >= 3);
        assert_eq!(result, Expr::op1(Op::RELU));
    }

    #[test]
    fn rewriter_trivial_residual() {
        let rw = standard_rewriter();
        let expr = Expr::App(
            Op::RES_ADD,
            vec![Expr::op0(Op::IDENTITY), Expr::op1(Op::LINEAR)],
        );
        let (result, apps) = rw.rewrite(&expr);
        assert!(apps > 0);
        assert_eq!(result, Expr::op1(Op::LINEAR));
    }

    #[test]
    fn custom_rule() {
        // Custom: sigmoid >> sigmoid => sigmoid
        let rule = RewriteRule::new(
            "sigmoid-idempotent",
            Pat::seq(Pat::op0(Op::SIGMOID), Pat::op0(Op::SIGMOID)),
            Tmpl::op0(Op::SIGMOID),
        );
        let mut rw = Rewriter::empty();
        rw.add_rule(rule);

        let expr = Expr::op1(Op::SIGMOID) >> Expr::op1(Op::SIGMOID);
        let (result, apps) = rw.rewrite(&expr);
        assert_eq!(apps, 1);
        assert_eq!(result.node_count(), 1);
    }

    #[test]
    fn rewriter_stats() {
        let rw = standard_rewriter();
        let expr = Expr::op0(Op::IDENTITY) >> Expr::op1(Op::RELU);
        let (_, apps) = rw.rewrite(&expr);
        assert_eq!(apps, 1);
    }

    #[test]
    fn standard_rules_count() {
        let rules = standard_rules();
        assert!(rules.len() >= 9);
    }

    #[test]
    fn nested_match_and_rewrite() {
        // relu >> identity should simplify to relu inside a parallel
        let rw = standard_rewriter();
        let expr = (Expr::op1(Op::RELU) >> Expr::op0(Op::IDENTITY))
            | (Expr::op0(Op::IDENTITY) >> Expr::op1(Op::LINEAR));
        let (result, apps) = rw.rewrite(&expr);
        assert!(apps >= 2);
        assert!(matches!(result, Expr::Par(_, _)));
    }

    #[test]
    fn match_no_match() {
        let pat = Pat::seq(Pat::op0(Op::RELU), Pat::op0(Op::LINEAR));
        let expr = Expr::int(42); // not a Seq at all
        assert!(match_pat(&pat, &expr).is_none());
    }

    #[test]
    fn template_op_with_args() {
        let tmpl = Tmpl::op(Op::ADD, vec![Tmpl::var("a"), Tmpl::var("b")]);
        let mut bindings = Bindings::new();
        bindings.insert("a".into(), Expr::int(1));
        bindings.insert("b".into(), Expr::int(2));
        let result = tmpl.instantiate(&bindings).unwrap();
        assert_eq!(result, Expr::op2(Op::ADD, Expr::int(1), Expr::int(2)));
    }
}
