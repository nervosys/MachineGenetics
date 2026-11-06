//! Static convexity analysis by disciplined convex programming (DCP).
//!
//! The question this pass answers is narrow: **can DCP certify the curvature of
//! an expression?** DCP (Grant & Boyd) builds the curvature of a whole
//! expression from the curvature of its parts, using a fixed table of atoms
//! with known curvature and monotonicity, plus a small set of composition
//! rules. A verdict of `Convex` or `Concave` is a proof, by those rules, that
//! the expression has that curvature.
//!
//! **`Unknown` is not "non-convex".** It means the rules cannot certify the
//! expression. `sin(x)` is not convex, and `2 * x * x` is convex, yet DCP
//! answers `Unknown` for it because the rules do not reassociate products. The
//! analysis is syntactic on purpose: a solver can rely on a `Convex` verdict,
//! and must treat `Unknown` as "use a general method", not as "reject".
//!
//! Plan 6.3 uses a `Convex`/`Concave` verdict to choose an exact solver for a
//! problem; everything else falls back to a general method.
//!
//! Modelled on `differentiable.rs`: a walk over the AST with one function per
//! construct, an `Unknown` answer for anything not analysed, and tests that
//! parse source snippets through the real lexer and parser.
//!
//! ## The lattice
//!
//! Curvature is `Constant` and `Affine` (both convex and concave), `Convex`,
//! `Concave`, and `Unknown` (no verdict). Sign is `Nonneg`, `Nonpos` or
//! `Unknown`; DCP needs it for scaling by constants and for the monotone atoms
//! (`abs`, `min`, `max`).
//!
//! ## The rules
//!
//! - A numeric literal is `Constant`, with the sign of its value. A variable or
//!   parameter is `Affine` with unknown sign. A `val` binding carries the facts
//!   of its value.
//! - `a + b` adds curvatures: convex + convex is convex, concave + concave is
//!   concave, a mix is `Unknown`, and an affine operand takes the other's
//!   curvature. `a - b` is `a + (-b)`; negation swaps convex and concave.
//! - `c * a` and `a / c` with `c` constant scale `a` by the sign of `c`.
//!   A product of two non-constant operands is `Unknown`, except `x * x` for
//!   affine `x` (and `-x * x`, which is concave): the square is convex.
//! - Atoms: `exp` is convex and increasing; `ln` and `log` are concave and
//!   increasing; `sqrt` is concave and increasing; `abs` is convex, increasing
//!   where its argument is nonnegative and decreasing where it is nonpositive.
//!   `max(a, b, ..)` is convex and `min(a, b, ..)` concave, when every argument
//!   is. A one-argument `min`/`max` reduces a list and is `Unknown`. Every other
//!   call is `Unknown`, including `sin`, `cos`, `tanh`, `sigmoid`.
//! - Composition: `f(g)` is convex if `f` is convex and either `g` is affine, or
//!   `f` is nondecreasing on the range of `g` and `g` is convex, or `f` is
//!   nonincreasing there and `g` is concave. Dually for concave.
//!
//! ## What is not checked
//!
//! Domains are not checked: `ln` needs a positive argument and `sqrt` a
//! nonnegative one, and a verdict holds wherever the expression is defined.
//! Constant sub-expressions are not evaluated beyond their sign, so
//! `ln(2) * x * x` is `Unknown`. Parameters are assumed affine with no sign.
//! Within a function body, a binding is tracked only if it is an immutable
//! `val` of a plain name; any other statement makes the whole body `Unknown`,
//! except a call or a nested item, which cannot rebind a local.

use crate::ast;
use std::collections::HashMap;

/// The curvature DCP can certify for an expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curvature {
    /// Does not depend on any variable. Both convex and concave.
    Constant,
    /// Linear in its variables. Both convex and concave.
    Affine,
    /// Certified convex.
    Convex,
    /// Certified concave.
    Concave,
    /// DCP cannot certify a curvature. This is not a claim of non-convexity.
    Unknown,
}

impl Curvature {
    /// True for `Constant`, `Affine` and `Convex`.
    pub fn is_convex(self) -> bool {
        matches!(self, Curvature::Constant | Curvature::Affine | Curvature::Convex)
    }

    /// True for `Constant`, `Affine` and `Concave`.
    pub fn is_concave(self) -> bool {
        matches!(self, Curvature::Constant | Curvature::Affine | Curvature::Concave)
    }

    fn is_affine(self) -> bool {
        matches!(self, Curvature::Constant | Curvature::Affine)
    }

    fn flip(self) -> Curvature {
        match self {
            Curvature::Convex => Curvature::Concave,
            Curvature::Concave => Curvature::Convex,
            other => other,
        }
    }
}

/// The sign DCP can certify for an expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    Nonneg,
    Nonpos,
    Unknown,
}

impl Sign {
    fn flip(self) -> Sign {
        match self {
            Sign::Nonneg => Sign::Nonpos,
            Sign::Nonpos => Sign::Nonneg,
            Sign::Unknown => Sign::Unknown,
        }
    }

    fn mul(self, other: Sign) -> Sign {
        match (self, other) {
            (Sign::Unknown, _) | (_, Sign::Unknown) => Sign::Unknown,
            (a, b) if a == b => Sign::Nonneg,
            _ => Sign::Nonpos,
        }
    }

    fn add(self, other: Sign) -> Sign {
        match (self, other) {
            (Sign::Nonneg, Sign::Nonneg) => Sign::Nonneg,
            (Sign::Nonpos, Sign::Nonpos) => Sign::Nonpos,
            _ => Sign::Unknown,
        }
    }
}

/// Which way an outer function moves its argument's range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Monotone {
    Up,
    Down,
    Neither,
}

/// What the analysis knows about one expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Facts {
    curvature: Curvature,
    sign: Sign,
}

impl Facts {
    const UNKNOWN: Facts = Facts { curvature: Curvature::Unknown, sign: Sign::Unknown };
    /// A variable or parameter: affine, with no sign known.
    const VARIABLE: Facts = Facts { curvature: Curvature::Affine, sign: Sign::Unknown };

    fn negate(self) -> Facts {
        Facts { curvature: self.curvature.flip(), sign: self.sign.flip() }
    }
}

/// Facts for the immutable `val` bindings in scope.
type Env = HashMap<String, Facts>;

/// The curvature DCP certifies for `expr`, with every name treated as an
/// affine variable of unknown sign.
pub fn curvature(expr: &ast::Expr) -> Curvature {
    expr_facts(expr, &Env::new()).curvature
}

/// The sign certified for `expr`, with every name of unknown sign.
pub fn sign(expr: &ast::Expr) -> Sign {
    expr_facts(expr, &Env::new()).sign
}

/// The curvature of a function's result, in terms of its parameters.
///
/// Parameters are affine. The result is the body's final expression, with any
/// immutable `val` bindings substituted; see the module documentation for what
/// makes a body `Unknown`.
pub fn function_curvature(fd: &ast::FunctionDef) -> Curvature {
    let facts = match &fd.body_expr {
        Some(e) => expr_facts(e, &Env::new()),
        None => block_facts(&fd.body, &mut Env::new()),
    };
    facts.curvature
}

fn expr_facts(expr: &ast::Expr, env: &Env) -> Facts {
    match expr {
        ast::Expr::Literal { value, kind } => literal_facts(value, kind),
        ast::Expr::Ident { name } => env.get(name).copied().unwrap_or(Facts::VARIABLE),
        ast::Expr::Unary { op, operand } if op == "-" => expr_facts(operand, env).negate(),
        ast::Expr::Binary { op, left, right } => binary_facts(op, left, right, env),
        ast::Expr::Call { func, args } => call_facts(func, args, env),
        ast::Expr::Block { block } => block_facts(block, &mut env.clone()),
        _ => Facts::UNKNOWN,
    }
}

fn literal_facts(value: &str, kind: &ast::LiteralKind) -> Facts {
    if !matches!(kind, ast::LiteralKind::Int | ast::LiteralKind::Float) {
        return Facts::UNKNOWN;
    }
    let sign = match numeric_value(value) {
        Some(v) if v >= 0.0 => Sign::Nonneg,
        Some(v) if v < 0.0 => Sign::Nonpos,
        _ => Sign::Unknown,
    };
    Facts { curvature: Curvature::Constant, sign }
}

/// The value of a numeric literal, or `None` if it is not one we read.
/// Underscores and a type suffix (`2f32`) are ignored. Radix literals are not
/// read, so their sign is unknown.
fn numeric_value(text: &str) -> Option<f64> {
    let s: String = text.chars().filter(|c| *c != '_').collect();
    if ["0x", "0X", "0b", "0o"].iter().any(|p| s.starts_with(p)) {
        return None;
    }
    let end = s
        .find(|c: char| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
        .unwrap_or(s.len());
    s[..end].parse::<f64>().ok()
}

fn binary_facts(op: &str, left: &ast::Expr, right: &ast::Expr, env: &Env) -> Facts {
    let l = expr_facts(left, env);
    let r = expr_facts(right, env);
    match op {
        "+" => sum(l, r),
        "-" => sum(l, r.negate()),
        "*" => product(left, l, right, r),
        "/" => quotient(l, right, r),
        _ => Facts::UNKNOWN,
    }
}

fn sum(l: Facts, r: Facts) -> Facts {
    Facts { curvature: sum_curvature(l.curvature, r.curvature), sign: l.sign.add(r.sign) }
}

fn sum_curvature(a: Curvature, b: Curvature) -> Curvature {
    if a == Curvature::Constant && b == Curvature::Constant {
        Curvature::Constant
    } else if a.is_affine() && b.is_affine() {
        Curvature::Affine
    } else if a.is_convex() && b.is_convex() {
        Curvature::Convex
    } else if a.is_concave() && b.is_concave() {
        Curvature::Concave
    } else {
        Curvature::Unknown
    }
}

/// Scale a fact by a constant of the given sign.
fn scale(f: Facts, by: Sign) -> Facts {
    let curvature = match (f.curvature, by) {
        // Constants and affine expressions keep their curvature at any sign.
        (Curvature::Constant | Curvature::Affine | Curvature::Unknown, _) => f.curvature,
        (Curvature::Convex, Sign::Nonneg) | (Curvature::Concave, Sign::Nonpos) => {
            Curvature::Convex
        }
        (Curvature::Convex, Sign::Nonpos) | (Curvature::Concave, Sign::Nonneg) => {
            Curvature::Concave
        }
        (Curvature::Convex | Curvature::Concave, Sign::Unknown) => Curvature::Unknown,
    };
    Facts { curvature, sign: f.sign.mul(by) }
}

fn product(left: &ast::Expr, l: Facts, right: &ast::Expr, r: Facts) -> Facts {
    if l.curvature == Curvature::Constant {
        return scale(r, l.sign);
    }
    if r.curvature == Curvature::Constant {
        return scale(l, r.sign);
    }
    // x * x for affine x is a square, which is convex. DCP has no rule for a
    // product of two non-constant operands, so this is the one exception, and
    // it is exact: the square of an affine function is convex and nonnegative.
    if l.curvature == Curvature::Affine && r.curvature == Curvature::Affine {
        if let Some(negated) = same_up_to_sign(left, right) {
            return if negated {
                Facts { curvature: Curvature::Concave, sign: Sign::Nonpos }
            } else {
                Facts { curvature: Curvature::Convex, sign: Sign::Nonneg }
            };
        }
    }
    Facts { curvature: Curvature::Unknown, sign: l.sign.mul(r.sign) }
}

fn quotient(l: Facts, right: &ast::Expr, r: Facts) -> Facts {
    // Dividing by a literal zero is not a scaling, whatever its sign says.
    let zero = matches!(right, ast::Expr::Literal { value, .. } if numeric_value(value) == Some(0.0));
    if r.curvature == Curvature::Constant && !zero {
        return scale(l, r.sign);
    }
    Facts { curvature: Curvature::Unknown, sign: l.sign.mul(r.sign) }
}

/// `Some(negated)` when `a` and `b` are the same expression up to leading
/// negations. `negated` is true when the negations differ in parity, so that
/// `-x * x` is recognised as minus a square.
fn same_up_to_sign(a: &ast::Expr, b: &ast::Expr) -> Option<bool> {
    let (core_a, neg_a) = strip_negation(a);
    let (core_b, neg_b) = strip_negation(b);
    // Structural equality: the AST does not derive `PartialEq`, and the
    // derived `Debug` form covers every field.
    (format!("{:?}", core_a) == format!("{:?}", core_b)).then_some(neg_a != neg_b)
}

fn strip_negation(e: &ast::Expr) -> (&ast::Expr, bool) {
    let mut core = e;
    let mut negated = false;
    loop {
        match core {
            ast::Expr::Unary { op, operand } if op == "-" => {
                negated = !negated;
                core = operand.as_ref();
            }
            _ => return (core, negated),
        }
    }
}

fn call_facts(func: &ast::Expr, args: &[ast::Expr], env: &Env) -> Facts {
    let name = match func {
        ast::Expr::Ident { name } => name.as_str(),
        _ => return Facts::UNKNOWN,
    };
    let a: Vec<Facts> = args.iter().map(|x| expr_facts(x, env)).collect();
    match (name, a.as_slice()) {
        ("exp", [x]) => atom(Curvature::Convex, Monotone::Up, Sign::Nonneg, *x),
        ("ln" | "log", [x]) => atom(Curvature::Concave, Monotone::Up, Sign::Unknown, *x),
        ("sqrt", [x]) => atom(Curvature::Concave, Monotone::Up, Sign::Nonneg, *x),
        ("abs", [x]) => {
            // |x| increases where x is nonnegative and decreases where it is
            // nonpositive, so the monotonicity depends on the argument's sign.
            let mono = match x.sign {
                Sign::Nonneg => Monotone::Up,
                Sign::Nonpos => Monotone::Down,
                Sign::Unknown => Monotone::Neither,
            };
            atom(Curvature::Convex, mono, Sign::Nonneg, *x)
        }
        ("max", [_, _, ..]) => extremum(true, &a),
        ("min", [_, _, ..]) => extremum(false, &a),
        // A one-argument min or max reduces a list, which is not analysed.
        _ => Facts::UNKNOWN,
    }
}

/// An atom of the given curvature applied to one argument.
fn atom(curvature: Curvature, mono: Monotone, sign: Sign, arg: Facts) -> Facts {
    let curvature = match arg.curvature {
        Curvature::Constant => Curvature::Constant,
        Curvature::Unknown => Curvature::Unknown,
        Curvature::Affine => curvature,
        inner => compose(curvature, mono, inner),
    };
    Facts { curvature, sign }
}

/// The composition rule for a convex or concave outer function over a convex
/// or concave inner one.
fn compose(outer: Curvature, mono: Monotone, inner: Curvature) -> Curvature {
    match (outer, mono, inner) {
        (Curvature::Convex, Monotone::Up, Curvature::Convex)
        | (Curvature::Convex, Monotone::Down, Curvature::Concave) => Curvature::Convex,
        (Curvature::Concave, Monotone::Up, Curvature::Concave)
        | (Curvature::Concave, Monotone::Down, Curvature::Convex) => Curvature::Concave,
        _ => Curvature::Unknown,
    }
}

/// `max` is convex when every argument is; `min` is concave when every
/// argument is. Each is nondecreasing in its arguments, so the composition
/// rule needs the arguments to have the matching curvature.
fn extremum(is_max: bool, args: &[Facts]) -> Facts {
    let all = |p: fn(&Facts) -> bool| args.iter().all(p);
    let curvature = if all(|f| f.curvature == Curvature::Constant) {
        Curvature::Constant
    } else if is_max && all(|f| f.curvature.is_convex()) {
        Curvature::Convex
    } else if !is_max && all(|f| f.curvature.is_concave()) {
        Curvature::Concave
    } else {
        Curvature::Unknown
    };
    // max is at least each argument, and min at most each argument.
    let sign = if is_max {
        if args.iter().any(|f| f.sign == Sign::Nonneg) {
            Sign::Nonneg
        } else if all(|f| f.sign == Sign::Nonpos) {
            Sign::Nonpos
        } else {
            Sign::Unknown
        }
    } else if all(|f| f.sign == Sign::Nonneg) {
        Sign::Nonneg
    } else if args.iter().any(|f| f.sign == Sign::Nonpos) {
        Sign::Nonpos
    } else {
        Sign::Unknown
    };
    Facts { curvature, sign }
}

/// The facts of a block's final expression. Immutable `val` bindings of a
/// plain name extend `env`. Anything else that could change a binding, or
/// that is not a call, makes the block `Unknown`.
fn block_facts(block: &ast::Block, env: &mut Env) -> Facts {
    for stmt in &block.stmts {
        match stmt {
            ast::Stmt::Let { mutable: false, pattern: ast::Pattern::Ident { name }, value, .. } => {
                let facts = expr_facts(value, env);
                env.insert(name.clone(), facts);
            }
            ast::Stmt::Expr { expr: ast::Expr::Call { .. } | ast::Expr::MethodCall { .. } } => {}
            ast::Stmt::Item { .. } => {}
            _ => return Facts::UNKNOWN,
        }
    }
    match &block.tail_expr {
        Some(tail) => expr_facts(tail, env),
        None => Facts::UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser};

    /// Parse `body` as the body of `f probe(x: f64, y: f64) -> f64`, through
    /// the real lexer and parser.
    fn function(body: &str) -> ast::FunctionDef {
        let src = format!("f probe(x: f64, y: f64) -> f64 {{ {body} }}");
        let tokens = lexer::lex(&src);
        let module = parser::parse(&tokens).expect("parse failed");
        match module.items.into_iter().next().map(|item| item.kind) {
            Some(ast::ItemKind::Function(fd)) => fd,
            _ => panic!("`{body}` did not parse to a function"),
        }
    }

    /// The curvature of `body`, as a function body.
    fn verdict(body: &str) -> Curvature {
        function_curvature(&function(body))
    }

    /// The final expression of `body`, for the expression-level API.
    fn tail(body: &str) -> ast::Expr {
        *function(body).body.tail_expr.expect("body has no final expression")
    }

    const CONVEX: &[&str] = &[
        "x * x",
        "exp(x)",
        "abs(x - 3)",
        "max(x, y)",
        "exp(2 * x + 1)",
        "x * x + exp(y)",
        "-log(x)",
        "x * x + y * y",
        "abs(x)",
        "exp(x) + exp(y)",
        "max(exp(x), 0)",
        "exp(x * x)",
        "abs(x * x)",
        "abs(-x * x)",
        "x * x - 2 * x + 1",
        "(x - 1) * (x - 1)",
        "0.5 * exp(x)",
        "exp(x) / 2",
        "max(x * x, y)",
        "-(-x * x)",
    ];

    const CONCAVE: &[&str] = &[
        "log(x)",
        "sqrt(x)",
        "-x * x",
        "min(x, y)",
        "log(x) + sqrt(y)",
        "-exp(x)",
        "-abs(x)",
        "log(x + 1)",
        "-(x * x)",
        "sqrt(x) * 3",
        "log(x) - exp(y)",
        "min(sqrt(x), log(y))",
        "2 * log(x)",
        "-x * x - y * y",
    ];

    const AFFINE_OR_CONSTANT: &[(&str, Curvature)] = &[
        ("x", Curvature::Affine),
        ("x + y", Curvature::Affine),
        ("2 * x + 1", Curvature::Affine),
        ("x - y", Curvature::Affine),
        ("-x", Curvature::Affine),
        ("x / 4", Curvature::Affine),
        ("-3 * x", Curvature::Affine),
        ("(x + 1) * 3", Curvature::Affine),
        ("3", Curvature::Constant),
        ("2 * 3 + 1", Curvature::Constant),
        ("exp(2)", Curvature::Constant),
        ("max(1, 2)", Curvature::Constant),
        ("-2.5", Curvature::Constant),
        ("sqrt(4) / 2", Curvature::Constant),
    ];

    /// Convex in fact, or not convex, but DCP cannot certify either way.
    const UNKNOWN: &[&str] = &[
        "x * y",
        "exp(x) - exp(y)",
        "log(exp(x) + exp(y))",
        "sin(x)",
        "x * x * x",
        "sqrt(x * x)",
        "exp(-x * x)",
        "x / y",
        "2 * x * x",
        "min(x * x, y)",
        "1 / x",
        "x * exp(y)",
        "sqrt(x) * sqrt(y)",
        "abs(sin(x))",
        "max(x)",
        "x / 0",
    ];

    #[test]
    fn known_convex_expressions_are_certified_convex() {
        assert!(CONVEX.len() >= 15, "corpus has {} convex entries", CONVEX.len());
        for body in CONVEX {
            assert_eq!(verdict(body), Curvature::Convex, "`{body}`");
        }
    }

    #[test]
    fn known_concave_expressions_are_certified_concave() {
        assert!(CONCAVE.len() >= 8, "corpus has {} concave entries", CONCAVE.len());
        for body in CONCAVE {
            assert_eq!(verdict(body), Curvature::Concave, "`{body}`");
        }
    }

    #[test]
    fn affine_and_constant_expressions_are_certified_as_such() {
        assert!(
            AFFINE_OR_CONSTANT.len() >= 8,
            "corpus has {} affine or constant entries",
            AFFINE_OR_CONSTANT.len()
        );
        for (body, expected) in AFFINE_OR_CONSTANT {
            assert_eq!(verdict(body), *expected, "`{body}`");
        }
    }

    #[test]
    fn expressions_dcp_cannot_certify_are_unknown() {
        assert!(UNKNOWN.len() >= 10, "corpus has {} unknown entries", UNKNOWN.len());
        for body in UNKNOWN {
            assert_eq!(verdict(body), Curvature::Unknown, "`{body}`");
        }
    }

    #[test]
    fn the_expression_api_agrees_with_the_function_api() {
        for body in CONVEX.iter().chain(CONCAVE).chain(UNKNOWN) {
            assert_eq!(curvature(&tail(body)), verdict(body), "`{body}`");
        }
    }

    #[test]
    fn a_square_of_an_affine_argument_is_nonnegative() {
        assert_eq!(sign(&tail("x * x")), Sign::Nonneg);
        assert_eq!(sign(&tail("-x * x")), Sign::Nonpos);
        assert_eq!(sign(&tail("(x - 1) * (x - 1)")), Sign::Nonneg);
    }

    #[test]
    fn a_square_is_only_recognised_for_the_same_operand() {
        assert_eq!(curvature(&tail("x * y")), Curvature::Unknown);
        assert_eq!(curvature(&tail("x * (x + 1)")), Curvature::Unknown);
    }

    #[test]
    fn negation_swaps_convex_and_concave() {
        assert_eq!(verdict("-log(x)"), Curvature::Convex);
        assert_eq!(verdict("-(x * x)"), Curvature::Concave);
        assert_eq!(verdict("-(-(x * x))"), Curvature::Convex);
    }

    #[test]
    fn a_negative_constant_flips_the_curvature_of_its_product() {
        assert_eq!(verdict("-2 * exp(x)"), Curvature::Concave);
        assert_eq!(verdict("-2 * log(x)"), Curvature::Convex);
        assert_eq!(verdict("2 * exp(x)"), Curvature::Convex);
    }

    #[test]
    fn a_constant_of_unknown_sign_does_not_fix_a_curvature() {
        // `ln(2)` is a constant, but its sign is not read off its value.
        assert_eq!(verdict("ln(2) * exp(x)"), Curvature::Unknown);
        assert_eq!(verdict("ln(2) * x"), Curvature::Affine);
    }

    #[test]
    fn a_composition_needs_the_monotonicity_of_the_outer_function() {
        // exp is increasing: it preserves convexity of its argument but not
        // concavity.
        assert_eq!(verdict("exp(x * x)"), Curvature::Convex);
        assert_eq!(verdict("exp(-x * x)"), Curvature::Unknown);
        // sqrt is increasing and concave: it needs a concave argument.
        assert_eq!(verdict("sqrt(x * x)"), Curvature::Unknown);
    }

    #[test]
    fn abs_follows_the_sign_of_its_argument() {
        // Increasing on a nonnegative argument, decreasing on a nonpositive one.
        assert_eq!(verdict("abs(x * x)"), Curvature::Convex);
        assert_eq!(verdict("abs(-x * x)"), Curvature::Convex);
        assert_eq!(verdict("abs(sin(x))"), Curvature::Unknown);
    }

    #[test]
    fn a_let_binding_carries_the_curvature_of_its_value() {
        assert_eq!(verdict("val s = x * x; s + exp(y)"), Curvature::Convex);
        assert_eq!(verdict("val s = x * x; -s"), Curvature::Concave);
        assert_eq!(verdict("val s = x * y; s + x"), Curvature::Unknown);
    }

    #[test]
    fn a_let_binding_ignores_an_unused_unknown_value() {
        assert_eq!(verdict("val s = sin(x); x * x"), Curvature::Convex);
    }

    #[test]
    fn a_mutable_binding_makes_the_body_unknown() {
        assert_eq!(verdict("var s = x * x; s"), Curvature::Unknown);
    }

    #[test]
    fn a_reassignment_makes_the_body_unknown() {
        assert_eq!(verdict("var s = x; s = x * x; s"), Curvature::Unknown);
    }

    #[test]
    fn a_call_statement_does_not_change_the_result() {
        assert_eq!(verdict("println(x); x * x"), Curvature::Convex);
    }

    #[test]
    fn a_binding_that_is_not_a_plain_name_is_unknown() {
        assert_eq!(verdict("val (a, b) = (x, y); a * a"), Curvature::Unknown);
    }

    #[test]
    fn a_body_without_a_final_expression_is_unknown() {
        assert_eq!(verdict("val s = x * x;"), Curvature::Unknown);
    }

    #[test]
    fn the_sign_of_literals_and_atoms() {
        assert_eq!(sign(&tail("3")), Sign::Nonneg);
        assert_eq!(sign(&tail("-2")), Sign::Nonpos);
        assert_eq!(sign(&tail("0")), Sign::Nonneg);
        assert_eq!(sign(&tail("exp(y)")), Sign::Nonneg);
        assert_eq!(sign(&tail("sqrt(y)")), Sign::Nonneg);
        assert_eq!(sign(&tail("abs(y)")), Sign::Nonneg);
        assert_eq!(sign(&tail("exp(x) + sqrt(y)")), Sign::Nonneg);
    }

    #[test]
    fn a_variable_has_no_known_sign() {
        assert_eq!(sign(&tail("x")), Sign::Unknown);
        assert_eq!(sign(&tail("x + x")), Sign::Unknown);
        assert_eq!(sign(&tail("log(x)")), Sign::Unknown);
    }

    #[test]
    fn helpers_follow_the_lattice() {
        assert!(Curvature::Affine.is_convex() && Curvature::Affine.is_concave());
        assert!(Curvature::Constant.is_convex() && Curvature::Constant.is_concave());
        assert!(Curvature::Convex.is_convex() && !Curvature::Convex.is_concave());
        assert!(!Curvature::Concave.is_convex() && Curvature::Concave.is_concave());
        assert!(!Curvature::Unknown.is_convex() && !Curvature::Unknown.is_concave());
    }

    #[test]
    fn numeric_literals_are_read_with_their_suffixes_and_signs() {
        assert_eq!(numeric_value("2f32"), Some(2.0));
        assert_eq!(numeric_value("1_000"), Some(1000.0));
        assert_eq!(numeric_value("1.5e-3f64"), Some(0.0015));
        assert_eq!(numeric_value("0xff"), None);
    }
}
