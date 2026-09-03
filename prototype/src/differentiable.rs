//! Differentiability inference — which functions have a derivative, and where.
//!
//! MAGE is meant to be differentiable *by design*, and this is the pass that
//! makes the phrase checkable. See `DIFFERENTIABILITY.md` for the argument; the
//! short version is that "fully differentiable" is not a claim any language with
//! `if` can make, so what is claimed instead is:
//!
//! > every function reported as differentiable is differentiable **almost
//! > everywhere** on its domain, and the compiler says which of the four states
//! > it is in rather than assuming the best one.
//!
//! **Modelled on `effects.rs` deliberately.** Differentiability is a
//! propagating, inferable, declarable property with a join over the call graph
//! — structurally the same analysis as effects, down to the cycle handling. It
//! is not a new kind of pass and should not look like one.
//!
//! ## The fourth state is the point
//!
//! `Unknown` is not a grade of differentiability. It is the absence of a
//! verdict, and it exists because the alternative is answering `Smooth` for "I
//! did not look at that construct". That distinction — *the claim is untested,
//! not clean* — is the one `StatodynamicAnalysis` builds its statodynamic
//! lattice around, and the one this repository spent 2026-09-01/02 removing
//! documentation that got wrong.

use crate::ast;
use crate::effects::EffectInfer;
use crate::hir::Effect;
use std::collections::HashMap;

/// How differentiable something is.
///
/// Ordered by `rank`, and joined worst-case, the way an effect set is unioned.
/// A definite negative outranks an unknown: if one branch cannot be
/// differentiated and another was not analysed, the function cannot be
/// differentiated, and saying so is more useful than saying nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Diff {
    /// Differentiable everywhere on its domain.
    Smooth,
    /// Differentiable off a set of measure zero — a kink, or a branch boundary.
    AlmostEverywhere,
    /// The pass could not determine it. Carries what it could not analyse.
    Unknown(String),
    /// Not differentiable, and here is why.
    No(String),
}

impl Diff {
    fn rank(&self) -> u8 {
        match self {
            Diff::Smooth => 0,
            Diff::AlmostEverywhere => 1,
            Diff::Unknown(_) => 2,
            Diff::No(_) => 3,
        }
    }

    /// Worst-case join. A composition is as differentiable as its least
    /// differentiable part.
    pub fn join(self, other: Diff) -> Diff {
        if other.rank() > self.rank() {
            other
        } else {
            self
        }
    }

    /// Does a derivative exist at all (a.e. or better)?
    pub fn is_differentiable(&self) -> bool {
        matches!(self, Diff::Smooth | Diff::AlmostEverywhere)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Diff::Smooth => "smooth",
            Diff::AlmostEverywhere => "almost everywhere",
            Diff::Unknown(_) => "unknown",
            Diff::No(_) => "not differentiable",
        }
    }

    /// The stated reason, for the two states that carry one.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Diff::Unknown(r) | Diff::No(r) => Some(r),
            _ => None,
        }
    }
}

/// Effects that mean a function's output is not a function of its inputs.
///
/// A derivative is a statement about how an output moves when an input moves.
/// If the output also depends on a clock, a socket or an entropy source, there
/// is nothing to differentiate — regardless of the arithmetic inside.
///
/// `Gpu` and `Npu` say *where* a function computes, not whether it is a
/// function, and are deliberately absent. So is `Alloc`. `Rng` is present: a
/// stochastic function has no derivative in this sense, and the
/// reparameterisation trick is a change of program, not a change of verdict.
///
/// This list is a decision, written down so it can be argued with rather than
/// discovered by reading the matcher.
pub const NON_FUNCTIONAL: &[Effect] = &[
    Effect::IO,
    Effect::FS,
    Effect::Net,
    Effect::Env,
    Effect::Time,
    Effect::Rng,
    Effect::Llm,
    Effect::Agent,
    Effect::Async,
];

pub struct DiffInfer {
    /// Inferred status per function.
    pub inferred: HashMap<String, Diff>,
    /// Call graph: caller → callees, the same shape `effects.rs` builds.
    call_graph: HashMap<String, Vec<String>>,
    /// Cycle detection for mutually recursive functions.
    in_progress: Vec<String>,
    /// Locally-determined status, before callees are folded in.
    local: HashMap<String, Diff>,
}

impl Default for DiffInfer {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffInfer {
    pub fn new() -> Self {
        DiffInfer {
            inferred: HashMap::new(),
            call_graph: HashMap::new(),
            in_progress: Vec::new(),
            local: HashMap::new(),
        }
    }

    /// The status of a function, or `Unknown` if this pass never saw it.
    ///
    /// Not `Smooth`. A name this pass has no record of is exactly the case the
    /// fourth state exists for.
    pub fn diff_of(&self, name: &str) -> Diff {
        self.inferred
            .get(name)
            .cloned()
            .unwrap_or_else(|| Diff::Unknown(format!("no analysis for `{name}`")))
    }

    /// Every function, in a stable order, for reporting.
    pub fn all(&self) -> Vec<(String, Diff)> {
        let mut v: Vec<_> = self
            .inferred
            .iter()
            .map(|(k, d)| (k.clone(), d.clone()))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }
}

// ── Types ────────────────────────────────────────────────────────────

/// Is this a type a derivative can be taken with respect to?
///
/// Floats and tensors of floats. Integers, `bool`, `str` and `char` are
/// discrete: there is no limit to take. A reference or a box is transparent.
fn type_is_continuous(ty: &ast::Type) -> bool {
    match ty {
        ast::Type::Path { segments, type_args } => {
            let name = segments.last().map(String::as_str).unwrap_or("");
            match name {
                "f32" | "f64" => true,
                // A tensor is differentiable when its element type is.
                "tensor" | "Tensor" | "Param" => {
                    type_args.first().map(type_is_continuous).unwrap_or(true)
                }
                // A container is differentiable when its contents are.
                "Vec" | "Option" => type_args.first().map(type_is_continuous).unwrap_or(false),
                _ => false,
            }
        }
        // Every wrapper is transparent: a box of floats is still floats.
        ast::Type::Reference { inner, .. }
        | ast::Type::OwnedPtr { inner }
        | ast::Type::Rc { inner }
        | ast::Type::Arc { inner }
        | ast::Type::Cow { inner }
        | ast::Type::Cell { inner }
        | ast::Type::RefCell { inner }
        | ast::Type::Mutex { inner }
        | ast::Type::RwLock { inner }
        | ast::Type::Slice { inner }
        | ast::Type::Array { inner, .. }
        | ast::Type::Vec { inner }
        | ast::Type::Option { inner } => type_is_continuous(inner),
        _ => false,
    }
}

// ── Operators ────────────────────────────────────────────────────────

/// Binary operators whose result is `bool`: a comparison has no derivative,
/// and the discreteness enters the program here rather than at the `if`.
fn is_comparison(op: &str) -> bool {
    matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||")
}

/// Operators that are smooth on their domain.
fn is_smooth_arith(op: &str) -> bool {
    matches!(op, "+" | "-" | "*" | "/")
}

/// Builtins with a kink: differentiable off a measure-zero set.
const KINKED: &[&str] = &[
    "abs", "relu", "max", "min", "clamp", "sign", "floor", "ceil", "round", "trunc",
];

/// Builtins that are smooth wherever they are defined.
const SMOOTH_BUILTINS: &[&str] = &[
    "exp", "ln", "log", "sqrt", "sin", "cos", "tan", "tanh", "sigmoid", "softmax", "gelu", "silu",
    "sum", "mean", "dot", "matmul",
];

/// Builtins that produce a discrete result.
const DISCRETE_BUILTINS: &[&str] = &[
    "len", "count", "keys", "argmax", "argmin", "index", "any", "all", "freq",
];

// ── The pass ─────────────────────────────────────────────────────────

/// Infer differentiability for every function in a module.
///
/// Takes the effect inference result rather than recomputing it: the
/// necessary condition (a differentiable function's output is a function of its
/// inputs) is *already* computed by `effects.rs`, and duplicating that
/// judgement here would be a second copy of a list that can drift — which this
/// repository has enough of.
pub fn infer(module: &ast::Module, effects: &EffectInfer) -> DiffInfer {
    let mut engine = DiffInfer::new();

    // Pass 1: local status and the call graph.
    for item in &module.items {
        if let ast::ItemKind::Function(fd) = &item.kind {
            let mut callees = Vec::new();
            collect_calls_block(&fd.body, &mut callees);
            if let Some(be) = &fd.body_expr {
                collect_calls_expr(be, &mut callees);
            }
            engine.call_graph.insert(fd.name.clone(), callees);
            engine
                .local
                .insert(fd.name.clone(), local_status(fd, effects));
        }
    }

    // Pass 2: fold callees in, worst-case, with cycle detection.
    let names: Vec<String> = engine.local.keys().cloned().collect();
    for name in names {
        let d = resolve(&mut engine, &name);
        engine.inferred.insert(name, d);
    }
    engine
}

fn resolve(engine: &mut DiffInfer, name: &str) -> Diff {
    if let Some(d) = engine.inferred.get(name) {
        return d.clone();
    }
    // A recursive call contributes nothing on the way down. The fixed point is
    // the join of everything else, which is how `effects.rs` treats the same
    // situation.
    if engine.in_progress.iter().any(|n| n == name) {
        return Diff::Smooth;
    }
    engine.in_progress.push(name.to_string());

    let mut d = engine
        .local
        .get(name)
        .cloned()
        .unwrap_or_else(|| Diff::Unknown(format!("no body for `{name}`")));

    for callee in engine.call_graph.get(name).cloned().unwrap_or_default() {
        // A call to something outside this module is not assumed smooth.
        let cd = if engine.local.contains_key(&callee) {
            resolve(engine, &callee)
        } else {
            builtin_status(&callee)
        };
        d = d.join(cd);
    }

    engine.in_progress.retain(|n| n != name);
    engine.inferred.insert(name.to_string(), d.clone());
    d
}

/// A function's status from its own signature, body and effects, before callees.
fn local_status(fd: &ast::FunctionDef, effects: &EffectInfer) -> Diff {
    // The effect condition first: it disqualifies regardless of the arithmetic.
    let es = effects.effects_of(&fd.name);
    for e in NON_FUNCTIONAL {
        if es.contains(e) {
            return Diff::No(format!(
                "performs the `{e:?}` effect, so its output is not a function of its inputs"
            ));
        }
    }

    // A derivative needs something continuous to move.
    if !fd.params.iter().any(|p| type_is_continuous(&p.ty)) {
        return Diff::No("no floating-point parameter to differentiate with respect to".into());
    }
    match &fd.return_type {
        Some(t) if !type_is_continuous(t) => {
            return Diff::No("returns a discrete type".into());
        }
        None => return Diff::No("returns nothing".into()),
        _ => {}
    }

    let mut d = block_status(&fd.body);
    if let Some(be) = &fd.body_expr {
        d = d.join(expr_status(be));
    }
    d
}

fn stmt_status(s: &ast::Stmt) -> Diff {
    match s {
        ast::Stmt::Expr { expr } | ast::Stmt::Defer { expr } => expr_status(expr),
        ast::Stmt::Let { value, .. } => expr_status(value),
        // A guard's condition selects; its else-block diverges. Neither is
        // differentiated, but the boundary is real, so it is a.e. like `if`.
        ast::Stmt::Guard { else_block, .. } => {
            block_status(else_block).join(Diff::AlmostEverywhere)
        }
        ast::Stmt::Item { .. } => Diff::Smooth,
    }
}

fn block_status(b: &ast::Block) -> Diff {
    let mut d = Diff::Smooth;
    for s in &b.stmts {
        d = d.join(stmt_status(s));
    }
    // The tail expression is the block's *value*, and omitting it made every
    // one-expression function read as Smooth -- including `x > y`. Three tests
    // caught it; without them the pass would have reported a comparison as
    // differentiable, which is the exact false-positive it exists to prevent.
    if let Some(t) = &b.tail_expr {
        d = d.join(expr_status(t));
    }
    d
}

fn expr_status(e: &ast::Expr) -> Diff {
    match e {
        // A constant's derivative is zero, which is perfectly smooth.
        ast::Expr::Literal { .. } | ast::Expr::Ident { .. } => Diff::Smooth,

        ast::Expr::Binary { op, left, right } => {
            if is_comparison(op) {
                // The discreteness enters here, not at the `if` that consumes it.
                Diff::No(format!("`{op}` produces a discrete result"))
            } else if is_smooth_arith(op) {
                expr_status(left).join(expr_status(right))
            } else {
                // Bitwise, shifts, modulo: integral operations.
                Diff::No(format!("`{op}` is not defined on a continuum"))
            }
        }

        ast::Expr::Unary { op, operand } => match op.as_str() {
            "-" => expr_status(operand),
            "!" => Diff::No("`!` produces a discrete result".into()),
            _ => Diff::Unknown(format!("unary `{op}`")),
        },

        // A branch on a continuous quantity is differentiable off the boundary,
        // which is measure zero. The condition itself is *not* differentiated —
        // it selects. So a comparison in a condition is expected and does not
        // disqualify the `if`, unlike a comparison whose value is returned.
        ast::Expr::If {
            cond,
            then_block,
            else_block,
        } => {
            let branches = block_status(then_block).join(
                else_block
                    .as_ref()
                    .map(block_status)
                    .unwrap_or(Diff::Smooth),
            );
            let boundary = if matches!(cond.as_ref(), ast::Expr::Binary { op, .. } if is_comparison(op))
            {
                Diff::AlmostEverywhere
            } else {
                Diff::AlmostEverywhere
            };
            branches.join(boundary)
        }

        ast::Expr::Block { block } => block_status(block),
        ast::Expr::Return { value } => {
            value.as_ref().map(|v| expr_status(v)).unwrap_or(Diff::Smooth)
        }

        ast::Expr::Call { func, args } => {
            let mut d = Diff::Smooth;
            for a in args {
                d = d.join(expr_status(a));
            }
            // The callee's own status is folded in by `resolve`, from the call
            // graph. Here only the arguments are judged, so a call is not
            // double-counted.
            let _ = func;
            d
        }

        ast::Expr::ArrayLit { elements } | ast::Expr::TupleLit { elements } => {
            let mut d = Diff::Smooth;
            for i in elements {
                d = d.join(expr_status(i));
            }
            d
        }

        ast::Expr::Cast { expr, ty } => {
            if type_is_continuous(ty) {
                expr_status(expr)
            } else {
                Diff::No("a cast to a discrete type destroys the derivative".into())
            }
        }

        ast::Expr::For { body, .. } => block_status(body),

        // A data-dependent trip count means the number of compositions depends
        // on the value being differentiated. That is analysable and this pass
        // does not analyse it.
        ast::Expr::Loop { .. } => Diff::Unknown("`loop` — trip count not analysed".into()),
        ast::Expr::While { .. } => Diff::Unknown("`while` — trip count not analysed".into()),

        ast::Expr::Index { object, .. } | ast::Expr::FieldAccess { object, .. } => {
            expr_status(object)
        }
        ast::Expr::Assign { value, .. } => expr_status(value),

        // Everything else is unanalysed rather than assumed fine. Naming the
        // variant makes the gap actionable instead of silent.
        other => Diff::Unknown(format!("{} not analysed", variant_name(other))),
    }
}

fn builtin_status(name: &str) -> Diff {
    let bare = name.rsplit('.').next().unwrap_or(name);
    if SMOOTH_BUILTINS.contains(&bare) {
        Diff::Smooth
    } else if KINKED.contains(&bare) {
        Diff::AlmostEverywhere
    } else if DISCRETE_BUILTINS.contains(&bare) {
        Diff::No(format!("`{bare}` produces a discrete result"))
    } else {
        Diff::Unknown(format!("`{bare}` is not in the differentiability tables"))
    }
}

fn variant_name(e: &ast::Expr) -> &'static str {
    match e {
        ast::Expr::MethodCall { .. } => "a method call",
        ast::Expr::Closure { .. } => "a closure",
        ast::Expr::Match { .. } => "`match`",
        ast::Expr::Handle { .. } => "`handle`",
        ast::Expr::Try { .. } => "`?`",
        ast::Expr::Await { .. } => "`.await`",
        ast::Expr::Range { .. } => "a range",
        ast::Expr::Pipeline { .. } => "a pipeline",
        ast::Expr::StructLit { .. } => "a struct literal",
        ast::Expr::MapLit { .. } => "a map literal",
        _ => "this expression",
    }
}

// ── Call collection ──────────────────────────────────────────────────
//
// Deliberately the same shape as `effects.rs::collect_calls_in_*`. Only direct
// calls by name are collected; a method call has no receiver type at this stage
// and is left to `expr_status`, which reports it as unanalysed rather than
// attributing it to the wrong function.

fn collect_calls_block(b: &ast::Block, out: &mut Vec<String>) {
    for s in &b.stmts {
        collect_calls_stmt(s, out);
    }
    if let Some(t) = &b.tail_expr {
        collect_calls_expr(t, out);
    }
}

fn collect_calls_stmt(s: &ast::Stmt, out: &mut Vec<String>) {
    match s {
        ast::Stmt::Expr { expr } | ast::Stmt::Defer { expr } => collect_calls_expr(expr, out),
        ast::Stmt::Let { value, .. } => collect_calls_expr(value, out),
        ast::Stmt::Guard { cond, else_block } => {
            collect_calls_expr(cond, out);
            collect_calls_block(else_block, out);
        }
        ast::Stmt::Item { .. } => {}
    }
}

fn collect_calls_expr(e: &ast::Expr, out: &mut Vec<String>) {
    match e {
        ast::Expr::Call { func, args } => {
            if let ast::Expr::Ident { name } = func.as_ref() {
                out.push(name.clone());
            }
            for a in args {
                collect_calls_expr(a, out);
            }
        }
        ast::Expr::Binary { left, right, .. } => {
            collect_calls_expr(left, out);
            collect_calls_expr(right, out);
        }
        ast::Expr::Unary { operand, .. } => collect_calls_expr(operand, out),
        ast::Expr::If {
            cond,
            then_block,
            else_block,
        } => {
            collect_calls_expr(cond, out);
            collect_calls_block(then_block, out);
            if let Some(b) = else_block {
                collect_calls_block(b, out);
            }
        }
        ast::Expr::Block { block } => collect_calls_block(block, out),
        ast::Expr::Return { value: Some(inner) } => collect_calls_expr(inner, out),
        ast::Expr::For { body, .. } => collect_calls_block(body, out),
        ast::Expr::ArrayLit { elements } | ast::Expr::TupleLit { elements } => {
            for i in elements {
                collect_calls_expr(i, out);
            }
        }
        ast::Expr::Cast { expr, .. } => collect_calls_expr(expr, out),
        ast::Expr::Index { object, .. } | ast::Expr::FieldAccess { object, .. } => {
            collect_calls_expr(object, out)
        }
        ast::Expr::Assign { value, .. } => collect_calls_expr(value, out),
        _ => {}
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{effects, lexer, parser};

    fn infer_src(src: &str) -> DiffInfer {
        let tokens = lexer::lex(src);
        let module = parser::parse(&tokens).expect("parse failed");
        let eff = effects::infer_effects(&module);
        infer(&module, &eff)
    }

    #[test]
    fn float_arithmetic_is_smooth() {
        let d = infer_src("f scale(x: f32, k: f32) -> f32 { x * k + 1.0 }");
        assert_eq!(d.diff_of("scale"), Diff::Smooth, "{:?}", d.diff_of("scale"));
    }

    #[test]
    fn a_branch_is_differentiable_off_the_boundary() {
        let d = infer_src("f relu(x: f32) -> f32 { ? x > 0.0 { x } : { 0.0 } }");
        assert_eq!(d.diff_of("relu"), Diff::AlmostEverywhere, "{:?}", d.diff_of("relu"));
    }

    /// The discreteness enters where the `bool` is made, not where it is used.
    #[test]
    fn returning_a_comparison_is_not_differentiable() {
        let d = infer_src("f gt(x: f32, y: f32) -> f32 { x > y }");
        assert!(!d.diff_of("gt").is_differentiable());
        assert!(d.diff_of("gt").reason().unwrap().contains('>'));
    }

    #[test]
    fn a_discrete_signature_is_refused_on_both_sides() {
        let d = infer_src("f n(a: i32, b: i32) -> i32 { a + b }");
        assert!(d.diff_of("n").reason().unwrap().contains("no floating-point parameter"));

        let d = infer_src("f trunc(x: f32) -> i32 { 0 }");
        assert!(d.diff_of("trunc").reason().unwrap().contains("discrete"));
    }

    /// The necessary condition that comes free from the effect pass: an output
    /// that also depends on a clock or a socket is not a function of its inputs,
    /// so there is nothing to differentiate regardless of the arithmetic.
    #[test]
    fn an_effectful_function_is_not_differentiable() {
        let d = infer_src("+f noisy(x: f32) -> f32 / io { println(\"hi\"); x * 2.0 }");
        assert!(!d.diff_of("noisy").is_differentiable(), "{:?}", d.diff_of("noisy"));
        assert!(d.diff_of("noisy").reason().unwrap().contains("effect"));
    }

    /// Worst-case join across the call graph: a smooth caller of a
    /// non-differentiable callee is not differentiable.
    #[test]
    fn non_differentiability_propagates_through_calls() {
        let d = infer_src(
            "f inner(x: f32) -> f32 { x > 1.0 }\n\
             f outer(x: f32) -> f32 { inner(x) * 2.0 }",
        );
        assert!(!d.diff_of("outer").is_differentiable(), "{:?}", d.diff_of("outer"));
    }

    /// **The property this pass exists for.** An unanalysed construct must not
    /// read as smooth. `while`'s trip count can depend on the value being
    /// differentiated, and this pass does not analyse that -- so it says so.
    #[test]
    fn an_unanalysed_construct_is_unknown_not_smooth() {
        let d = infer_src("f spin(x: f32) -> f32 { @w x > 1.0 { x } x }");
        let s = d.diff_of("spin");
        assert!(
            matches!(s, Diff::Unknown(_)),
            "an unanalysed loop must be Unknown, not {:?}",
            s
        );
        assert!(!s.is_differentiable(), "Unknown must not count as differentiable");
    }

    /// A name the pass never saw is Unknown, never Smooth. The default answer
    /// for "no analysis" is the absence of a verdict.
    #[test]
    fn an_unseen_name_is_unknown() {
        let d = infer_src("f id(x: f32) -> f32 { x }");
        assert!(matches!(d.diff_of("nonexistent"), Diff::Unknown(_)));
    }

    /// Join order: a definite negative outranks an unknown, because a stated
    /// reason is more useful than silence; and unknown outranks a positive,
    /// because the alternative is claiming what was not checked.
    #[test]
    fn the_join_prefers_a_stated_negative_then_unknown() {
        let no = Diff::No("r".into());
        let unk = Diff::Unknown("u".into());
        assert_eq!(unk.clone().join(no.clone()), no);
        assert_eq!(no.clone().join(unk.clone()), no);
        assert_eq!(Diff::Smooth.join(unk.clone()), unk);
        assert_eq!(Diff::AlmostEverywhere.join(Diff::Smooth), Diff::AlmostEverywhere);
    }

    /// Mutual recursion must terminate rather than blow the stack.
    #[test]
    fn mutual_recursion_terminates() {
        let d = infer_src(
            "f a(x: f32) -> f32 { b(x) }\n\
             f b(x: f32) -> f32 { a(x) }",
        );
        assert!(d.inferred.contains_key("a") && d.inferred.contains_key("b"));
    }
}
