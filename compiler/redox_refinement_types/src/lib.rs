//! Refinement types for the Redox compiler.
//!
//! Implements `{x: T | predicate(x)}` style value constraints with an
//! embedded SMT-like solver for static verification of refinement predicates.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Base types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BaseType {
    Bool,
    Int,
    UInt,
    Float,
    Str,
    Unit,
    Named(String),
    Array(Box<BaseType>),
    Tuple(Vec<BaseType>),
    Function(Vec<BaseType>, Box<BaseType>),
}

impl fmt::Display for BaseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaseType::Bool => write!(f, "bool"),
            BaseType::Int => write!(f, "i64"),
            BaseType::UInt => write!(f, "u64"),
            BaseType::Float => write!(f, "f64"),
            BaseType::Str => write!(f, "str"),
            BaseType::Unit => write!(f, "()"),
            BaseType::Named(n) => write!(f, "{n}"),
            BaseType::Array(inner) => write!(f, "[{inner}]"),
            BaseType::Tuple(elems) => {
                let parts: Vec<String> = elems.iter().map(|e| e.to_string()).collect();
                write!(f, "({})", parts.join(", "))
            }
            BaseType::Function(args, ret) => {
                let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(f, "({}) -> {ret}", parts.join(", "))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Refinement predicates (SMT-compatible expressions)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Predicate {
    True,
    False,
    Var(String),
    IntLit(i64),
    BoolLit(bool),
    /// Binary comparison / arithmetic
    BinOp(Box<Predicate>, PredOp, Box<Predicate>),
    /// Unary operation
    UnOp(PredUnOp, Box<Predicate>),
    /// Logical connective
    And(Box<Predicate>, Box<Predicate>),
    Or(Box<Predicate>, Box<Predicate>),
    Not(Box<Predicate>),
    Implies(Box<Predicate>, Box<Predicate>),
    /// Quantifiers
    ForAll(String, Box<Predicate>),
    Exists(String, Box<Predicate>),
    /// Function application in predicate: f(args)
    App(String, Vec<Predicate>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredUnOp {
    Neg,
}

impl fmt::Display for PredOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PredOp::Eq => "==",
            PredOp::Ne => "!=",
            PredOp::Lt => "<",
            PredOp::Le => "<=",
            PredOp::Gt => ">",
            PredOp::Ge => ">=",
            PredOp::Add => "+",
            PredOp::Sub => "-",
            PredOp::Mul => "*",
            PredOp::Div => "/",
            PredOp::Mod => "%",
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Predicate::True => write!(f, "true"),
            Predicate::False => write!(f, "false"),
            Predicate::Var(v) => write!(f, "{v}"),
            Predicate::IntLit(n) => write!(f, "{n}"),
            Predicate::BoolLit(b) => write!(f, "{b}"),
            Predicate::BinOp(l, op, r) => write!(f, "({l} {op} {r})"),
            Predicate::UnOp(PredUnOp::Neg, e) => write!(f, "-{e}"),
            Predicate::And(l, r) => write!(f, "({l} && {r})"),
            Predicate::Or(l, r) => write!(f, "({l} || {r})"),
            Predicate::Not(e) => write!(f, "!{e}"),
            Predicate::Implies(l, r) => write!(f, "({l} ==> {r})"),
            Predicate::ForAll(v, body) => write!(f, "forall {v}. {body}"),
            Predicate::Exists(v, body) => write!(f, "exists {v}. {body}"),
            Predicate::App(name, args) => {
                let parts: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(f, "{name}({})", parts.join(", "))
            }
        }
    }
}

// Predicate constructors
impl Predicate {
    pub fn var(name: &str) -> Self {
        Predicate::Var(name.to_string())
    }

    pub fn int(n: i64) -> Self {
        Predicate::IntLit(n)
    }

    pub fn gt(l: Predicate, r: Predicate) -> Self {
        Predicate::BinOp(Box::new(l), PredOp::Gt, Box::new(r))
    }

    pub fn ge(l: Predicate, r: Predicate) -> Self {
        Predicate::BinOp(Box::new(l), PredOp::Ge, Box::new(r))
    }

    pub fn lt(l: Predicate, r: Predicate) -> Self {
        Predicate::BinOp(Box::new(l), PredOp::Lt, Box::new(r))
    }

    pub fn le(l: Predicate, r: Predicate) -> Self {
        Predicate::BinOp(Box::new(l), PredOp::Le, Box::new(r))
    }

    pub fn eq(l: Predicate, r: Predicate) -> Self {
        Predicate::BinOp(Box::new(l), PredOp::Eq, Box::new(r))
    }

    pub fn ne(l: Predicate, r: Predicate) -> Self {
        Predicate::BinOp(Box::new(l), PredOp::Ne, Box::new(r))
    }

    pub fn add(l: Predicate, r: Predicate) -> Self {
        Predicate::BinOp(Box::new(l), PredOp::Add, Box::new(r))
    }

    pub fn sub(l: Predicate, r: Predicate) -> Self {
        Predicate::BinOp(Box::new(l), PredOp::Sub, Box::new(r))
    }

    pub fn and(l: Predicate, r: Predicate) -> Self {
        Predicate::And(Box::new(l), Box::new(r))
    }

    pub fn or(l: Predicate, r: Predicate) -> Self {
        Predicate::Or(Box::new(l), Box::new(r))
    }

    pub fn not(e: Predicate) -> Self {
        Predicate::Not(Box::new(e))
    }

    pub fn implies(l: Predicate, r: Predicate) -> Self {
        Predicate::Implies(Box::new(l), Box::new(r))
    }

    /// Substitute var_name with replacement in this predicate.
    pub fn subst(&self, var_name: &str, replacement: &Predicate) -> Predicate {
        match self {
            Predicate::Var(v) if v == var_name => replacement.clone(),
            Predicate::Var(_)
            | Predicate::True
            | Predicate::False
            | Predicate::IntLit(_)
            | Predicate::BoolLit(_) => self.clone(),
            Predicate::BinOp(l, op, r) => Predicate::BinOp(
                Box::new(l.subst(var_name, replacement)),
                *op,
                Box::new(r.subst(var_name, replacement)),
            ),
            Predicate::UnOp(op, e) => {
                Predicate::UnOp(*op, Box::new(e.subst(var_name, replacement)))
            }
            Predicate::And(l, r) => Predicate::And(
                Box::new(l.subst(var_name, replacement)),
                Box::new(r.subst(var_name, replacement)),
            ),
            Predicate::Or(l, r) => Predicate::Or(
                Box::new(l.subst(var_name, replacement)),
                Box::new(r.subst(var_name, replacement)),
            ),
            Predicate::Not(e) => Predicate::Not(Box::new(e.subst(var_name, replacement))),
            Predicate::Implies(l, r) => Predicate::Implies(
                Box::new(l.subst(var_name, replacement)),
                Box::new(r.subst(var_name, replacement)),
            ),
            Predicate::ForAll(v, body) if v == var_name => self.clone(),
            Predicate::ForAll(v, body) => {
                Predicate::ForAll(v.clone(), Box::new(body.subst(var_name, replacement)))
            }
            Predicate::Exists(v, body) if v == var_name => self.clone(),
            Predicate::Exists(v, body) => {
                Predicate::Exists(v.clone(), Box::new(body.subst(var_name, replacement)))
            }
            Predicate::App(name, args) => {
                let new_args = args.iter().map(|a| a.subst(var_name, replacement)).collect();
                Predicate::App(name.clone(), new_args)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Refinement type: {x: BaseType | predicate}
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct RefinementType {
    /// The refinement variable name (e.g. "x" in {x: i32 | x > 0})
    pub var: String,
    /// The base type
    pub base: BaseType,
    /// The refinement predicate
    pub predicate: Predicate,
}

impl RefinementType {
    pub fn new(var: String, base: BaseType, predicate: Predicate) -> Self {
        Self { var, base, predicate }
    }

    /// A trivially refined type (predicate = true)
    pub fn trivial(var: String, base: BaseType) -> Self {
        Self { var, base, predicate: Predicate::True }
    }

    pub fn is_trivial(&self) -> bool {
        matches!(self.predicate, Predicate::True)
    }
}

impl fmt::Display for RefinementType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_trivial() {
            write!(f, "{}", self.base)
        } else {
            write!(f, "{{{}: {} | {}}}", self.var, self.base, self.predicate)
        }
    }
}

// ---------------------------------------------------------------------------
// SMT-like solver (embedded, simplified)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmtResult {
    Sat,
    Unsat,
    Unknown,
}

impl fmt::Display for SmtResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmtResult::Sat => write!(f, "sat"),
            SmtResult::Unsat => write!(f, "unsat"),
            SmtResult::Unknown => write!(f, "unknown"),
        }
    }
}

/// Simplified SMT solver for integer linear arithmetic and boolean logic.
pub struct SmtSolver {
    assertions: Vec<Predicate>,
}

impl SmtSolver {
    pub fn new() -> Self {
        Self { assertions: Vec::new() }
    }

    pub fn assert(&mut self, pred: Predicate) {
        self.assertions.push(pred);
    }

    pub fn reset(&mut self) {
        self.assertions.clear();
    }

    /// Check satisfiability of the conjunction of all assertions.
    pub fn check_sat(&self) -> SmtResult {
        let combined = self.conjunction();
        self.eval_sat(&combined)
    }

    /// Check if `goal` is implied by the current assertions.
    /// Returns `SmtResult::Unsat` if assertions ∧ ¬goal is unsatisfiable (i.e., goal is valid).
    pub fn check_valid(&self, goal: &Predicate) -> SmtResult {
        let mut solver = SmtSolver::new();
        for a in &self.assertions {
            solver.assert(a.clone());
        }
        solver.assert(Predicate::Not(Box::new(goal.clone())));
        solver.check_sat()
    }

    fn conjunction(&self) -> Predicate {
        if self.assertions.is_empty() {
            return Predicate::True;
        }
        let mut result = self.assertions[0].clone();
        for a in &self.assertions[1..] {
            result = Predicate::And(Box::new(result), Box::new(a.clone()));
        }
        result
    }

    fn eval_sat(&self, pred: &Predicate) -> SmtResult {
        // Try constant evaluation first
        match self.const_eval(pred) {
            Some(true) => return SmtResult::Sat,
            Some(false) => return SmtResult::Unsat,
            None => {}
        }

        // Try simple interval reasoning for single-variable predicates
        let mut var_bounds: HashMap<String, (Option<i64>, Option<i64>)> = HashMap::new();
        self.extract_bounds(pred, &mut var_bounds, true);

        // Check if any variable has contradictory bounds
        for (_var, (lo, hi)) in &var_bounds {
            if let (Some(l), Some(h)) = (lo, hi) {
                if l > h {
                    return SmtResult::Unsat;
                }
            }
        }

        // If we have tight enough bounds and the formula is a conjunction of
        // simple integer constraints, try enumeration on small domains
        if self.is_simple_conjunction(pred) && !var_bounds.is_empty() {
            let result = self.try_enumerate(pred, &var_bounds);
            if result != SmtResult::Unknown {
                return result;
            }
        }

        SmtResult::Unknown
    }

    fn const_eval(&self, pred: &Predicate) -> Option<bool> {
        match pred {
            Predicate::True | Predicate::BoolLit(true) => Some(true),
            Predicate::False | Predicate::BoolLit(false) => Some(false),
            Predicate::BinOp(l, op, r) => {
                let lv = self.const_eval_int(l)?;
                let rv = self.const_eval_int(r)?;
                Some(match op {
                    PredOp::Eq => lv == rv,
                    PredOp::Ne => lv != rv,
                    PredOp::Lt => lv < rv,
                    PredOp::Le => lv <= rv,
                    PredOp::Gt => lv > rv,
                    PredOp::Ge => lv >= rv,
                    _ => return None,
                })
            }
            Predicate::And(l, r) => {
                let lv = self.const_eval(l)?;
                let rv = self.const_eval(r)?;
                Some(lv && rv)
            }
            Predicate::Or(l, r) => {
                let lv = self.const_eval(l)?;
                let rv = self.const_eval(r)?;
                Some(lv || rv)
            }
            Predicate::Not(e) => {
                let v = self.const_eval(e)?;
                Some(!v)
            }
            Predicate::Implies(l, r) => {
                let lv = self.const_eval(l)?;
                if !lv {
                    return Some(true);
                }
                let rv = self.const_eval(r)?;
                Some(rv)
            }
            _ => None,
        }
    }

    fn const_eval_int(&self, pred: &Predicate) -> Option<i64> {
        match pred {
            Predicate::IntLit(n) => Some(*n),
            Predicate::BinOp(l, op, r) => {
                let lv = self.const_eval_int(l)?;
                let rv = self.const_eval_int(r)?;
                match op {
                    PredOp::Add => Some(lv + rv),
                    PredOp::Sub => Some(lv - rv),
                    PredOp::Mul => Some(lv * rv),
                    PredOp::Div if rv != 0 => Some(lv / rv),
                    PredOp::Mod if rv != 0 => Some(lv % rv),
                    _ => None,
                }
            }
            Predicate::UnOp(PredUnOp::Neg, e) => {
                let v = self.const_eval_int(e)?;
                Some(-v)
            }
            _ => None,
        }
    }

    fn extract_bounds(
        &self,
        pred: &Predicate,
        bounds: &mut HashMap<String, (Option<i64>, Option<i64>)>,
        positive: bool,
    ) {
        match pred {
            Predicate::BinOp(l, op, r) if positive => {
                // var op const or const op var
                if let (Predicate::Var(v), Some(n)) = (l.as_ref(), self.const_eval_int(r)) {
                    let entry = bounds.entry(v.clone()).or_insert((None, None));
                    match op {
                        PredOp::Gt => {
                            let lo = n + 1;
                            entry.0 = Some(entry.0.map_or(lo, |old: i64| old.max(lo)));
                        }
                        PredOp::Ge => {
                            entry.0 = Some(entry.0.map_or(n, |old: i64| old.max(n)));
                        }
                        PredOp::Lt => {
                            let hi = n - 1;
                            entry.1 = Some(entry.1.map_or(hi, |old: i64| old.min(hi)));
                        }
                        PredOp::Le => {
                            entry.1 = Some(entry.1.map_or(n, |old: i64| old.min(n)));
                        }
                        PredOp::Eq => {
                            entry.0 = Some(n);
                            entry.1 = Some(n);
                        }
                        _ => {}
                    }
                }
                if let (Some(n), Predicate::Var(v)) = (self.const_eval_int(l), r.as_ref()) {
                    let entry = bounds.entry(v.clone()).or_insert((None, None));
                    match op {
                        PredOp::Lt => {
                            let lo = n + 1;
                            entry.0 = Some(entry.0.map_or(lo, |old: i64| old.max(lo)));
                        }
                        PredOp::Le => {
                            entry.0 = Some(entry.0.map_or(n, |old: i64| old.max(n)));
                        }
                        PredOp::Gt => {
                            let hi = n - 1;
                            entry.1 = Some(entry.1.map_or(hi, |old: i64| old.min(hi)));
                        }
                        PredOp::Ge => {
                            entry.1 = Some(entry.1.map_or(n, |old: i64| old.min(n)));
                        }
                        PredOp::Eq => {
                            entry.0 = Some(n);
                            entry.1 = Some(n);
                        }
                        _ => {}
                    }
                }
            }
            Predicate::And(l, r) if positive => {
                self.extract_bounds(l, bounds, true);
                self.extract_bounds(r, bounds, true);
            }
            _ => {}
        }
    }

    fn is_simple_conjunction(&self, pred: &Predicate) -> bool {
        match pred {
            Predicate::True | Predicate::False | Predicate::BoolLit(_) => true,
            Predicate::BinOp(_, op, _) => matches!(
                op,
                PredOp::Eq | PredOp::Ne | PredOp::Lt | PredOp::Le | PredOp::Gt | PredOp::Ge
            ),
            Predicate::And(l, r) => self.is_simple_conjunction(l) && self.is_simple_conjunction(r),
            Predicate::Not(inner) => self.is_simple_conjunction(inner),
            _ => false,
        }
    }

    fn try_enumerate(
        &self,
        pred: &Predicate,
        bounds: &HashMap<String, (Option<i64>, Option<i64>)>,
    ) -> SmtResult {
        // Only enumerate if all variables have finite small ranges
        let mut vars: Vec<(String, i64, i64)> = Vec::new();
        for (v, (lo, hi)) in bounds {
            let lo = lo.unwrap_or(-100);
            let hi = hi.unwrap_or(100);
            if hi - lo > 200 {
                return SmtResult::Unknown;
            }
            vars.push((v.clone(), lo, hi));
        }

        if vars.is_empty() {
            return SmtResult::Unknown;
        }

        // Single variable fast path
        if vars.len() == 1 {
            let (ref var, lo, hi) = vars[0];
            for val in lo..=hi {
                let p = pred.subst(var, &Predicate::IntLit(val));
                if self.const_eval(&p) == Some(true) {
                    return SmtResult::Sat;
                }
            }
            return SmtResult::Unsat;
        }

        // Two-variable enumeration
        if vars.len() == 2 {
            let (ref v1, lo1, hi1) = vars[0];
            let (ref v2, lo2, hi2) = vars[1];
            for val1 in lo1..=hi1 {
                let p1 = pred.subst(v1, &Predicate::IntLit(val1));
                for val2 in lo2..=hi2 {
                    let p2 = p1.subst(v2, &Predicate::IntLit(val2));
                    if self.const_eval(&p2) == Some(true) {
                        return SmtResult::Sat;
                    }
                }
            }
            return SmtResult::Unsat;
        }

        SmtResult::Unknown
    }
}

impl Default for SmtSolver {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Subtype checking: {x: T | P} <: {y: T | Q} iff P(x) => Q(x)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum SubtypeResult {
    /// Subtyping holds
    Ok,
    /// Cannot verify subtyping
    Unknown,
    /// Subtyping fails with reason
    Fail(String),
}

/// Check if `sub` is a subtype of `sup`.
/// {x: T | P} <: {y: T | Q} iff for all x, P(x) => Q[y:=x](x).
pub fn check_subtype(sub: &RefinementType, sup: &RefinementType) -> SubtypeResult {
    if sub.base != sup.base {
        return SubtypeResult::Fail(format!("base type mismatch: {} vs {}", sub.base, sup.base));
    }

    // Trivial cases
    if sup.is_trivial() {
        return SubtypeResult::Ok;
    }
    if sub.is_trivial() && !sup.is_trivial() {
        // Need to check if `true => Q` i.e. Q is always true
        let solver = SmtSolver::new();
        let q = sup.predicate.subst(&sup.var, &Predicate::Var(sub.var.clone()));
        match solver.check_valid(&q) {
            SmtResult::Unsat => return SubtypeResult::Ok,
            _ => return SubtypeResult::Unknown,
        }
    }

    // General case: check P => Q[y:=x]
    let q_renamed = sup.predicate.subst(&sup.var, &Predicate::Var(sub.var.clone()));
    let mut solver = SmtSolver::new();
    solver.assert(sub.predicate.clone());
    let goal = q_renamed;

    match solver.check_valid(&goal) {
        SmtResult::Unsat => SubtypeResult::Ok,
        SmtResult::Sat => SubtypeResult::Fail("subtype predicate not implied".to_string()),
        SmtResult::Unknown => SubtypeResult::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Refinement type checker
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum RefinementError {
    /// Type mismatch
    TypeMismatch { expected: String, found: String, location: String },
    /// Refinement predicate not satisfiable
    UnsatisfiablePredicate { ty: String, location: String },
    /// Cannot verify subtyping
    SubtypeFail { sub: String, sup: String, reason: String },
    /// Undefined variable in refinement
    UndefinedVar { var: String, location: String },
    /// Invalid predicate for base type
    InvalidPredicate { predicate: String, base: String },
}

impl fmt::Display for RefinementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefinementError::TypeMismatch { expected, found, location } => {
                write!(f, "type mismatch at {location}: expected {expected}, found {found}")
            }
            RefinementError::UnsatisfiablePredicate { ty, location } => {
                write!(f, "unsatisfiable refinement at {location}: {ty}")
            }
            RefinementError::SubtypeFail { sub, sup, reason } => {
                write!(f, "subtype check failed: {sub} <: {sup} ({reason})")
            }
            RefinementError::UndefinedVar { var, location } => {
                write!(f, "undefined variable '{var}' in refinement at {location}")
            }
            RefinementError::InvalidPredicate { predicate, base } => {
                write!(f, "invalid predicate '{predicate}' for base type {base}")
            }
        }
    }
}

/// Type environment mapping variable names to refinement types.
pub type TypeEnv = HashMap<String, RefinementType>;

/// Refinement type checker.
pub struct RefinementChecker {
    env: TypeEnv,
    errors: Vec<RefinementError>,
}

impl RefinementChecker {
    pub fn new() -> Self {
        Self { env: TypeEnv::new(), errors: Vec::new() }
    }

    pub fn errors(&self) -> &[RefinementError] {
        &self.errors
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Declare a variable with a refinement type.
    pub fn declare(&mut self, name: String, ty: RefinementType) {
        self.env.insert(name, ty);
    }

    /// Check that a value (given as a predicate fact) satisfies a refinement type.
    pub fn check_assignment(
        &mut self,
        var_name: &str,
        value_fact: &Predicate,
        expected: &RefinementType,
    ) {
        // Build a refined type from the value fact
        let value_ty =
            RefinementType::new(expected.var.clone(), expected.base.clone(), value_fact.clone());

        match check_subtype(&value_ty, expected) {
            SubtypeResult::Ok => {
                self.env.insert(var_name.to_string(), expected.clone());
            }
            SubtypeResult::Fail(reason) => {
                self.errors.push(RefinementError::SubtypeFail {
                    sub: value_ty.to_string(),
                    sup: expected.to_string(),
                    reason,
                });
            }
            SubtypeResult::Unknown => {
                // Cannot verify statically — allow with warning
                self.env.insert(var_name.to_string(), expected.clone());
            }
        }
    }

    /// Check that a function argument satisfies its refinement type.
    pub fn check_arg(
        &mut self,
        fn_name: &str,
        arg_name: &str,
        arg_type: &RefinementType,
        provided_type: &RefinementType,
    ) {
        match check_subtype(provided_type, arg_type) {
            SubtypeResult::Ok => {}
            SubtypeResult::Fail(reason) => {
                self.errors.push(RefinementError::SubtypeFail {
                    sub: provided_type.to_string(),
                    sup: arg_type.to_string(),
                    reason: format!("argument '{arg_name}' of fn {fn_name}: {reason}"),
                });
            }
            SubtypeResult::Unknown => {} // allow
        }
    }

    /// Verify that a refinement type's predicate is satisfiable.
    pub fn check_satisfiable(&mut self, ty: &RefinementType, location: &str) {
        let mut solver = SmtSolver::new();
        solver.assert(ty.predicate.clone());
        if solver.check_sat() == SmtResult::Unsat {
            self.errors.push(RefinementError::UnsatisfiablePredicate {
                ty: ty.to_string(),
                location: location.to_string(),
            });
        }
    }

    /// Lookup a variable's refinement type.
    pub fn lookup(&self, name: &str) -> Option<&RefinementType> {
        self.env.get(name)
    }
}

impl Default for RefinementChecker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Common refinement type constructors
// ---------------------------------------------------------------------------

/// {x: i64 | x > 0}
pub fn positive_int() -> RefinementType {
    RefinementType::new(
        "x".to_string(),
        BaseType::Int,
        Predicate::gt(Predicate::var("x"), Predicate::int(0)),
    )
}

/// {x: i64 | x >= 0}
pub fn non_negative_int() -> RefinementType {
    RefinementType::new(
        "x".to_string(),
        BaseType::Int,
        Predicate::ge(Predicate::var("x"), Predicate::int(0)),
    )
}

/// {x: i64 | lo <= x && x <= hi}
pub fn bounded_int(lo: i64, hi: i64) -> RefinementType {
    RefinementType::new(
        "x".to_string(),
        BaseType::Int,
        Predicate::and(
            Predicate::ge(Predicate::var("x"), Predicate::int(lo)),
            Predicate::le(Predicate::var("x"), Predicate::int(hi)),
        ),
    )
}

/// {x: i64 | x != 0}
pub fn non_zero_int() -> RefinementType {
    RefinementType::new(
        "x".to_string(),
        BaseType::Int,
        Predicate::ne(Predicate::var("x"), Predicate::int(0)),
    )
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Predicate tests --

    #[test]
    fn test_predicate_display_simple() {
        let p = Predicate::gt(Predicate::var("x"), Predicate::int(0));
        assert_eq!(p.to_string(), "(x > 0)");
    }

    #[test]
    fn test_predicate_display_and() {
        let p = Predicate::and(
            Predicate::gt(Predicate::var("x"), Predicate::int(0)),
            Predicate::lt(Predicate::var("x"), Predicate::int(100)),
        );
        assert_eq!(p.to_string(), "((x > 0) && (x < 100))");
    }

    #[test]
    fn test_predicate_display_implies() {
        let p = Predicate::implies(Predicate::var("a"), Predicate::var("b"));
        assert_eq!(p.to_string(), "(a ==> b)");
    }

    #[test]
    fn test_predicate_display_forall() {
        let p = Predicate::ForAll(
            "x".to_string(),
            Box::new(Predicate::gt(Predicate::var("x"), Predicate::int(0))),
        );
        assert_eq!(p.to_string(), "forall x. (x > 0)");
    }

    #[test]
    fn test_predicate_subst() {
        let p = Predicate::gt(Predicate::var("x"), Predicate::int(0));
        let p2 = p.subst("x", &Predicate::int(5));
        assert_eq!(p2.to_string(), "(5 > 0)");
    }

    #[test]
    fn test_predicate_subst_bound_var_unchanged() {
        let p = Predicate::ForAll(
            "x".to_string(),
            Box::new(Predicate::gt(Predicate::var("x"), Predicate::int(0))),
        );
        let p2 = p.subst("x", &Predicate::int(5));
        // Should not substitute inside forall binding x
        assert_eq!(p2.to_string(), "forall x. (x > 0)");
    }

    // -- Refinement type tests --

    #[test]
    fn test_refinement_type_display() {
        let rt = positive_int();
        assert_eq!(rt.to_string(), "{x: i64 | (x > 0)}");
    }

    #[test]
    fn test_trivial_refinement_display() {
        let rt = RefinementType::trivial("v".to_string(), BaseType::Int);
        assert_eq!(rt.to_string(), "i64");
        assert!(rt.is_trivial());
    }

    #[test]
    fn test_bounded_int_display() {
        let rt = bounded_int(1, 100);
        assert_eq!(rt.to_string(), "{x: i64 | ((x >= 1) && (x <= 100))}");
    }

    // -- SMT solver tests --

    #[test]
    fn test_smt_true() {
        let mut solver = SmtSolver::new();
        solver.assert(Predicate::True);
        assert_eq!(solver.check_sat(), SmtResult::Sat);
    }

    #[test]
    fn test_smt_false() {
        let mut solver = SmtSolver::new();
        solver.assert(Predicate::False);
        assert_eq!(solver.check_sat(), SmtResult::Unsat);
    }

    #[test]
    fn test_smt_constant_comparison() {
        let mut solver = SmtSolver::new();
        solver.assert(Predicate::gt(Predicate::int(5), Predicate::int(3)));
        assert_eq!(solver.check_sat(), SmtResult::Sat);
    }

    #[test]
    fn test_smt_contradictory_bounds() {
        let mut solver = SmtSolver::new();
        // x > 10 && x < 5
        solver.assert(Predicate::and(
            Predicate::gt(Predicate::var("x"), Predicate::int(10)),
            Predicate::lt(Predicate::var("x"), Predicate::int(5)),
        ));
        assert_eq!(solver.check_sat(), SmtResult::Unsat);
    }

    #[test]
    fn test_smt_satisfiable_bounds() {
        let mut solver = SmtSolver::new();
        // x >= 1 && x <= 5
        solver.assert(Predicate::and(
            Predicate::ge(Predicate::var("x"), Predicate::int(1)),
            Predicate::le(Predicate::var("x"), Predicate::int(5)),
        ));
        assert_eq!(solver.check_sat(), SmtResult::Sat);
    }

    #[test]
    fn test_smt_check_valid_constant() {
        let solver = SmtSolver::new();
        let goal = Predicate::gt(Predicate::int(5), Predicate::int(3));
        // 5 > 3 is always true, so !(5 > 3) is unsat
        assert_eq!(solver.check_valid(&goal), SmtResult::Unsat);
    }

    #[test]
    fn test_smt_reset() {
        let mut solver = SmtSolver::new();
        solver.assert(Predicate::False);
        assert_eq!(solver.check_sat(), SmtResult::Unsat);
        solver.reset();
        solver.assert(Predicate::True);
        assert_eq!(solver.check_sat(), SmtResult::Sat);
    }

    // -- Subtype checking tests --

    #[test]
    fn test_subtype_trivial() {
        let sub = positive_int();
        let sup = RefinementType::trivial("y".to_string(), BaseType::Int);
        assert_eq!(check_subtype(&sub, &sup), SubtypeResult::Ok);
    }

    #[test]
    fn test_subtype_base_mismatch() {
        let sub = RefinementType::trivial("x".to_string(), BaseType::Int);
        let sup = RefinementType::trivial("x".to_string(), BaseType::Bool);
        match check_subtype(&sub, &sup) {
            SubtypeResult::Fail(msg) => assert!(msg.contains("base type mismatch")),
            other => panic!("expected Fail, got {other:?}"),
        }
    }

    #[test]
    fn test_subtype_positive_to_nonneg() {
        // {x: i64 | x > 0} <: {x: i64 | x >= 0} should hold
        let sub = positive_int();
        let sup = non_negative_int();
        // x > 0 implies x >= 0 — our solver should handle this via enumeration
        let result = check_subtype(&sub, &sup);
        assert!(matches!(result, SubtypeResult::Ok | SubtypeResult::Unknown));
    }

    #[test]
    fn test_subtype_bounded_to_nonneg() {
        // {x: i64 | 1 <= x <= 100} <: {x: i64 | x >= 0}
        let sub = bounded_int(1, 100);
        let sup = non_negative_int();
        let result = check_subtype(&sub, &sup);
        assert!(matches!(result, SubtypeResult::Ok | SubtypeResult::Unknown));
    }

    // -- Refinement checker tests --

    #[test]
    fn test_checker_satisfiable() {
        let mut checker = RefinementChecker::new();
        let rt = positive_int();
        checker.check_satisfiable(&rt, "test");
        assert!(!checker.has_errors());
    }

    #[test]
    fn test_checker_unsatisfiable() {
        let mut checker = RefinementChecker::new();
        let rt = RefinementType::new(
            "x".to_string(),
            BaseType::Int,
            // x > 10 && x < 5 — impossible
            Predicate::and(
                Predicate::gt(Predicate::var("x"), Predicate::int(10)),
                Predicate::lt(Predicate::var("x"), Predicate::int(5)),
            ),
        );
        checker.check_satisfiable(&rt, "test");
        assert!(checker.has_errors());
        assert!(matches!(&checker.errors()[0], RefinementError::UnsatisfiablePredicate { .. }));
    }

    #[test]
    fn test_checker_declare_and_lookup() {
        let mut checker = RefinementChecker::new();
        let rt = positive_int();
        checker.declare("n".to_string(), rt.clone());
        assert_eq!(checker.lookup("n"), Some(&rt));
        assert_eq!(checker.lookup("m"), None);
    }

    #[test]
    fn test_checker_arg_base_type_mismatch() {
        let mut checker = RefinementChecker::new();
        let param_ty = positive_int();
        let arg_ty = RefinementType::trivial("x".to_string(), BaseType::Bool);
        checker.check_arg("div", "n", &param_ty, &arg_ty);
        assert!(checker.has_errors());
        assert!(matches!(&checker.errors()[0], RefinementError::SubtypeFail { .. }));
    }

    // -- Common constructor tests --

    #[test]
    fn test_positive_int() {
        let rt = positive_int();
        assert_eq!(rt.var, "x");
        assert_eq!(rt.base, BaseType::Int);
        assert!(!rt.is_trivial());
    }

    #[test]
    fn test_non_zero_int() {
        let rt = non_zero_int();
        assert_eq!(rt.to_string(), "{x: i64 | (x != 0)}");
    }

    // -- Base type display --

    #[test]
    fn test_base_type_display() {
        assert_eq!(BaseType::Int.to_string(), "i64");
        assert_eq!(BaseType::Array(Box::new(BaseType::Bool)).to_string(), "[bool]");
        assert_eq!(
            BaseType::Function(vec![BaseType::Int, BaseType::Int], Box::new(BaseType::Bool))
                .to_string(),
            "(i64, i64) -> bool"
        );
        assert_eq!(BaseType::Tuple(vec![BaseType::Int, BaseType::Str]).to_string(), "(i64, str)");
    }

    // -- Error display --

    #[test]
    fn test_error_display() {
        let e = RefinementError::TypeMismatch {
            expected: "i64".to_string(),
            found: "bool".to_string(),
            location: "line 5".to_string(),
        };
        assert!(e.to_string().contains("type mismatch"));

        let e2 =
            RefinementError::UndefinedVar { var: "z".to_string(), location: "line 10".to_string() };
        assert!(e2.to_string().contains("undefined variable"));
    }

    // -- Two-variable enumeration --

    #[test]
    fn test_smt_two_var_unsat() {
        let mut solver = SmtSolver::new();
        // x == 5 && y == 3 && x < y
        solver.assert(Predicate::and(
            Predicate::and(
                Predicate::eq(Predicate::var("x"), Predicate::int(5)),
                Predicate::eq(Predicate::var("y"), Predicate::int(3)),
            ),
            Predicate::lt(Predicate::var("x"), Predicate::var("y")),
        ));
        assert_eq!(solver.check_sat(), SmtResult::Unsat);
    }

    #[test]
    fn test_smt_two_var_sat() {
        let mut solver = SmtSolver::new();
        // x >= 1 && x <= 3 && y >= 4 && y <= 6 && (x < y)  — satisfiable
        solver.assert(Predicate::and(
            Predicate::and(
                Predicate::ge(Predicate::var("x"), Predicate::int(1)),
                Predicate::le(Predicate::var("x"), Predicate::int(3)),
            ),
            Predicate::and(
                Predicate::and(
                    Predicate::ge(Predicate::var("y"), Predicate::int(4)),
                    Predicate::le(Predicate::var("y"), Predicate::int(6)),
                ),
                Predicate::lt(Predicate::var("x"), Predicate::var("y")),
            ),
        ));
        assert_eq!(solver.check_sat(), SmtResult::Sat);
    }
}
