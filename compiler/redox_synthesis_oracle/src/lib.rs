//! # Synthesis Oracle
//!
//! Generates candidate implementations from formal specifications using
//! constraint solving and template synthesis.
//!
//! Given a spec with `@req`/`@ens` clauses, the oracle produces candidate
//! code expressions that satisfy the constraints.

use std::collections::HashMap;
use std::fmt;

// ── Specification Types ──────────────────────────────────────────────

/// Expression in a spec constraint.
#[derive(Debug, Clone, PartialEq)]
pub enum SpecExpr {
    IntLit(i64),
    BoolLit(bool),
    Var(String),
    Result,
    Old(Box<SpecExpr>),
    BinOp(Box<SpecExpr>, BinOp, Box<SpecExpr>),
    UnOp(UnOp, Box<SpecExpr>),
    Call(String, Vec<SpecExpr>),
    Field(Box<SpecExpr>, String),
    MethodCall(Box<SpecExpr>, String, Vec<SpecExpr>),
    Ite(Box<SpecExpr>, Box<SpecExpr>, Box<SpecExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnOp {
    Not,
    Neg,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "+"), Self::Sub => write!(f, "-"),
            Self::Mul => write!(f, "*"), Self::Div => write!(f, "/"), Self::Mod => write!(f, "%"),
            Self::Eq => write!(f, "=="), Self::Ne => write!(f, "!="),
            Self::Lt => write!(f, "<"), Self::Le => write!(f, "<="),
            Self::Gt => write!(f, ">"), Self::Ge => write!(f, ">="),
            Self::And => write!(f, "&&"), Self::Or => write!(f, "||"),
        }
    }
}

impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Not => write!(f, "!"),
            Self::Neg => write!(f, "-"),
        }
    }
}

impl fmt::Display for SpecExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntLit(n) => write!(f, "{n}"),
            Self::BoolLit(b) => write!(f, "{b}"),
            Self::Var(v) => write!(f, "{v}"),
            Self::Result => write!(f, "result"),
            Self::Old(e) => write!(f, "old({e})"),
            Self::BinOp(l, op, r) => write!(f, "({l} {op} {r})"),
            Self::UnOp(op, e) => write!(f, "{op}{e}"),
            Self::Call(name, args) => {
                let a: Vec<String> = args.iter().map(|x| format!("{x}")).collect();
                write!(f, "{name}({})", a.join(", "))
            }
            Self::Field(e, name) => write!(f, "{e}.{name}"),
            Self::MethodCall(e, name, args) => {
                let a: Vec<String> = args.iter().map(|x| format!("{x}")).collect();
                write!(f, "{e}.{name}({})", a.join(", "))
            }
            Self::Ite(c, t, e) => write!(f, "if {c} {{ {t} }} else {{ {e} }}"),
        }
    }
}

// ── Types ────────────────────────────────────────────────────────────

/// Simple type for synthesis.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SynthType {
    Int,
    Bool,
    Float,
    String,
    Array(Box<SynthType>),
    Tuple(Vec<SynthType>),
    Custom(String),
}

impl fmt::Display for SynthType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => write!(f, "i64"),
            Self::Bool => write!(f, "bool"),
            Self::Float => write!(f, "f64"),
            Self::String => write!(f, "String"),
            Self::Array(t) => write!(f, "[{t}]"),
            Self::Tuple(ts) => {
                let parts: Vec<String> = ts.iter().map(|t| format!("{t}")).collect();
                write!(f, "({})", parts.join(", "))
            }
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

/// A parameter in a synthesis problem.
#[derive(Debug, Clone)]
pub struct SynthParam {
    pub name: String,
    pub ty: SynthType,
}

/// A synthesis problem: given params, preconditions, postconditions,
/// produce an expression for `result`.
#[derive(Debug, Clone)]
pub struct SynthProblem {
    pub name: String,
    pub params: Vec<SynthParam>,
    pub return_type: SynthType,
    pub preconditions: Vec<SpecExpr>,
    pub postconditions: Vec<SpecExpr>,
}

impl SynthProblem {
    pub fn new(name: impl Into<String>, return_type: SynthType) -> Self {
        Self {
            name: name.into(),
            params: Vec::new(),
            return_type,
            preconditions: Vec::new(),
            postconditions: Vec::new(),
        }
    }

    pub fn param(mut self, name: impl Into<String>, ty: SynthType) -> Self {
        self.params.push(SynthParam { name: name.into(), ty });
        self
    }

    pub fn pre(mut self, expr: SpecExpr) -> Self {
        self.preconditions.push(expr);
        self
    }

    pub fn post(mut self, expr: SpecExpr) -> Self {
        self.postconditions.push(expr);
        self
    }
}

// ── Code Templates ───────────────────────────────────────────────────

/// A code expression template with holes.
#[derive(Debug, Clone, PartialEq)]
pub enum CodeExpr {
    Lit(i64),
    BoolLit(bool),
    Var(String),
    BinOp(Box<CodeExpr>, BinOp, Box<CodeExpr>),
    UnOp(UnOp, Box<CodeExpr>),
    Ite(Box<CodeExpr>, Box<CodeExpr>, Box<CodeExpr>),
    Call(String, Vec<CodeExpr>),
    /// A hole to be filled by synthesis.
    Hole(String, SynthType),
}

impl fmt::Display for CodeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lit(n) => write!(f, "{n}"),
            Self::BoolLit(b) => write!(f, "{b}"),
            Self::Var(v) => write!(f, "{v}"),
            Self::BinOp(l, op, r) => write!(f, "({l} {op} {r})"),
            Self::UnOp(op, e) => write!(f, "{op}{e}"),
            Self::Ite(c, t, e) => write!(f, "if {c} {{ {t} }} else {{ {e} }}"),
            Self::Call(name, args) => {
                let a: Vec<String> = args.iter().map(|x| format!("{x}")).collect();
                write!(f, "{name}({})", a.join(", "))
            }
            Self::Hole(name, ty) => write!(f, "??{name}:{ty}"),
        }
    }
}

impl CodeExpr {
    /// Collect all hole names in this expression.
    pub fn holes(&self) -> Vec<String> {
        let mut result = Vec::new();
        self.collect_holes(&mut result);
        result
    }

    fn collect_holes(&self, out: &mut Vec<String>) {
        match self {
            Self::Hole(name, _) => out.push(name.clone()),
            Self::BinOp(l, _, r) => { l.collect_holes(out); r.collect_holes(out); }
            Self::UnOp(_, e) => e.collect_holes(out),
            Self::Ite(c, t, e) => { c.collect_holes(out); t.collect_holes(out); e.collect_holes(out); }
            Self::Call(_, args) => { for a in args { a.collect_holes(out); } }
            Self::Lit(_) | Self::BoolLit(_) | Self::Var(_) => {}
        }
    }

    /// Substitute holes with given mappings.
    pub fn fill_holes(&self, mapping: &HashMap<String, CodeExpr>) -> CodeExpr {
        match self {
            Self::Hole(name, _) => {
                mapping.get(name).cloned().unwrap_or_else(|| self.clone())
            }
            Self::BinOp(l, op, r) => {
                CodeExpr::BinOp(Box::new(l.fill_holes(mapping)), *op, Box::new(r.fill_holes(mapping)))
            }
            Self::UnOp(op, e) => CodeExpr::UnOp(*op, Box::new(e.fill_holes(mapping))),
            Self::Ite(c, t, e) => {
                CodeExpr::Ite(
                    Box::new(c.fill_holes(mapping)),
                    Box::new(t.fill_holes(mapping)),
                    Box::new(e.fill_holes(mapping)),
                )
            }
            Self::Call(name, args) => {
                CodeExpr::Call(name.clone(), args.iter().map(|a| a.fill_holes(mapping)).collect())
            }
            other => other.clone(),
        }
    }

    /// Check if this expression has any remaining holes.
    pub fn is_complete(&self) -> bool {
        self.holes().is_empty()
    }

    /// Expression size (number of AST nodes).
    pub fn size(&self) -> usize {
        match self {
            Self::Lit(_) | Self::BoolLit(_) | Self::Var(_) | Self::Hole(_, _) => 1,
            Self::BinOp(l, _, r) => 1 + l.size() + r.size(),
            Self::UnOp(_, e) => 1 + e.size(),
            Self::Ite(c, t, e) => 1 + c.size() + t.size() + e.size(),
            Self::Call(_, args) => 1 + args.iter().map(|a| a.size()).sum::<usize>(),
        }
    }
}

// ── Constraint Solver ────────────────────────────────────────────────

/// A value during evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Bool(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// Environment for evaluation.
pub type Env = HashMap<String, Value>;

/// Evaluate a spec expression in an environment.
pub fn eval_spec(expr: &SpecExpr, env: &Env) -> Result<Value, String> {
    match expr {
        SpecExpr::IntLit(n) => Ok(Value::Int(*n)),
        SpecExpr::BoolLit(b) => Ok(Value::Bool(*b)),
        SpecExpr::Var(v) => env.get(v).cloned().ok_or(format!("undefined: {v}")),
        SpecExpr::Result => env.get("__result").cloned().ok_or("no result".into()),
        SpecExpr::Old(e) => {
            // In evaluation, old(x) looks up __old_<varname> or falls back to x
            if let SpecExpr::Var(v) = e.as_ref() {
                let old_key = format!("__old_{v}");
                if let Some(val) = env.get(&old_key) {
                    return Ok(val.clone());
                }
            }
            eval_spec(e, env)
        }
        SpecExpr::BinOp(l, op, r) => {
            let lv = eval_spec(l, env)?;
            let rv = eval_spec(r, env)?;
            eval_binop(&lv, *op, &rv)
        }
        SpecExpr::UnOp(op, e) => {
            let v = eval_spec(e, env)?;
            eval_unop(*op, &v)
        }
        SpecExpr::Ite(cond, then, else_) => {
            let c = eval_spec(cond, env)?;
            match c {
                Value::Bool(true) => eval_spec(then, env),
                Value::Bool(false) => eval_spec(else_, env),
                _ => Err("ite condition must be bool".into()),
            }
        }
        SpecExpr::Call(_, _) | SpecExpr::Field(_, _) | SpecExpr::MethodCall(_, _, _) => {
            Err("cannot evaluate calls/fields in synthesis".into())
        }
    }
}

fn eval_binop(l: &Value, op: BinOp, r: &Value) -> Result<Value, String> {
    match (l, r) {
        (Value::Int(a), Value::Int(b)) => match op {
            BinOp::Add => Ok(Value::Int(a + b)),
            BinOp::Sub => Ok(Value::Int(a - b)),
            BinOp::Mul => Ok(Value::Int(a * b)),
            BinOp::Div => {
                if *b == 0 { return Err("division by zero".into()); }
                Ok(Value::Int(a / b))
            }
            BinOp::Mod => {
                if *b == 0 { return Err("modulo by zero".into()); }
                Ok(Value::Int(a % b))
            }
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::Ne => Ok(Value::Bool(a != b)),
            BinOp::Lt => Ok(Value::Bool(a < b)),
            BinOp::Le => Ok(Value::Bool(a <= b)),
            BinOp::Gt => Ok(Value::Bool(a > b)),
            BinOp::Ge => Ok(Value::Bool(a >= b)),
            _ => Err(format!("invalid int op: {op}")),
        },
        (Value::Bool(a), Value::Bool(b)) => match op {
            BinOp::And => Ok(Value::Bool(*a && *b)),
            BinOp::Or => Ok(Value::Bool(*a || *b)),
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::Ne => Ok(Value::Bool(a != b)),
            _ => Err(format!("invalid bool op: {op}")),
        },
        _ => Err("type mismatch in binop".into()),
    }
}

fn eval_unop(op: UnOp, v: &Value) -> Result<Value, String> {
    match (op, v) {
        (UnOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
        (UnOp::Neg, Value::Int(n)) => Ok(Value::Int(-n)),
        _ => Err("type mismatch in unop".into()),
    }
}

/// Evaluate a code expression in an environment.
pub fn eval_code(expr: &CodeExpr, env: &Env) -> Result<Value, String> {
    match expr {
        CodeExpr::Lit(n) => Ok(Value::Int(*n)),
        CodeExpr::BoolLit(b) => Ok(Value::Bool(*b)),
        CodeExpr::Var(v) => env.get(v).cloned().ok_or(format!("undefined: {v}")),
        CodeExpr::BinOp(l, op, r) => {
            let lv = eval_code(l, env)?;
            let rv = eval_code(r, env)?;
            eval_binop(&lv, *op, &rv)
        }
        CodeExpr::UnOp(op, e) => {
            let v = eval_code(e, env)?;
            eval_unop(*op, &v)
        }
        CodeExpr::Ite(cond, then, else_) => {
            let c = eval_code(cond, env)?;
            match c {
                Value::Bool(true) => eval_code(then, env),
                Value::Bool(false) => eval_code(else_, env),
                _ => Err("ite condition must be bool".into()),
            }
        }
        CodeExpr::Hole(name, _) => Err(format!("unfilled hole: {name}")),
        CodeExpr::Call(_, _) => Err("cannot evaluate calls in synthesis".into()),
    }
}

// ── Template Library ─────────────────────────────────────────────────

/// Generate candidate templates for a given problem shape.
pub fn generate_templates(problem: &SynthProblem) -> Vec<CodeExpr> {
    let mut templates = Vec::new();
    let param_names: Vec<&str> = problem.params.iter().map(|p| p.name.as_str()).collect();

    match &problem.return_type {
        SynthType::Int => {
            // Template: param directly
            for p in &param_names {
                templates.push(CodeExpr::Var(p.to_string()));
            }
            // Template: param + constant
            for p in &param_names {
                templates.push(CodeExpr::BinOp(
                    Box::new(CodeExpr::Var(p.to_string())),
                    BinOp::Add,
                    Box::new(CodeExpr::Hole("c".into(), SynthType::Int)),
                ));
            }
            // Template: param - constant
            for p in &param_names {
                templates.push(CodeExpr::BinOp(
                    Box::new(CodeExpr::Var(p.to_string())),
                    BinOp::Sub,
                    Box::new(CodeExpr::Hole("c".into(), SynthType::Int)),
                ));
            }
            // Template: param * constant
            for p in &param_names {
                templates.push(CodeExpr::BinOp(
                    Box::new(CodeExpr::Var(p.to_string())),
                    BinOp::Mul,
                    Box::new(CodeExpr::Hole("c".into(), SynthType::Int)),
                ));
            }
            // Template: a op b (two params)
            if param_names.len() >= 2 {
                for op in &[BinOp::Add, BinOp::Sub, BinOp::Mul] {
                    templates.push(CodeExpr::BinOp(
                        Box::new(CodeExpr::Var(param_names[0].to_string())),
                        *op,
                        Box::new(CodeExpr::Var(param_names[1].to_string())),
                    ));
                }
            }
            // Template: if (p > 0) p else -p  (abs-like)
            for p in &param_names {
                templates.push(CodeExpr::Ite(
                    Box::new(CodeExpr::BinOp(
                        Box::new(CodeExpr::Var(p.to_string())),
                        BinOp::Ge,
                        Box::new(CodeExpr::Lit(0)),
                    )),
                    Box::new(CodeExpr::Var(p.to_string())),
                    Box::new(CodeExpr::UnOp(UnOp::Neg, Box::new(CodeExpr::Var(p.to_string())))),
                ));
            }
            // Template: constant
            templates.push(CodeExpr::Hole("c".into(), SynthType::Int));
        }
        SynthType::Bool => {
            // Template: param comparison constant
            for p in &param_names {
                for op in &[BinOp::Gt, BinOp::Ge, BinOp::Lt, BinOp::Le, BinOp::Eq, BinOp::Ne] {
                    templates.push(CodeExpr::BinOp(
                        Box::new(CodeExpr::Var(p.to_string())),
                        *op,
                        Box::new(CodeExpr::Hole("c".into(), SynthType::Int)),
                    ));
                }
            }
            // Template: true/false constants
            templates.push(CodeExpr::BoolLit(true));
            templates.push(CodeExpr::BoolLit(false));
        }
        _ => {
            // Generic: just return the first param or a hole
            if !param_names.is_empty() {
                templates.push(CodeExpr::Var(param_names[0].to_string()));
            }
            templates.push(CodeExpr::Hole("v".into(), problem.return_type.clone()));
        }
    }

    templates
}

// ── Constraint-Based Synthesis ───────────────────────────────────────

/// Test inputs for CEGIS (counterexample-guided inductive synthesis).
#[derive(Debug, Clone)]
pub struct TestCase {
    pub inputs: Env,
    pub expected_result: Option<Value>,
}

/// Synthesis result.
#[derive(Debug, Clone)]
pub struct SynthResult {
    pub expression: CodeExpr,
    pub verified_cases: usize,
    pub confidence: SynthConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthConfidence {
    /// All test cases passed.
    High,
    /// Some test cases passed.
    Medium,
    /// Heuristic match only.
    Low,
}

impl fmt::Display for SynthConfidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::High => write!(f, "high"),
            Self::Medium => write!(f, "medium"),
            Self::Low => write!(f, "low"),
        }
    }
}

impl fmt::Display for SynthResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (confidence: {}, verified: {} cases)",
            self.expression, self.confidence, self.verified_cases)
    }
}

/// Generate test cases from preconditions (small input enumeration).
pub fn generate_test_cases(problem: &SynthProblem, count: usize) -> Vec<TestCase> {
    let mut cases = Vec::new();
    let range = -(count as i64 / 2)..=(count as i64 / 2);

    match problem.params.len() {
        0 => {
            cases.push(TestCase { inputs: Env::new(), expected_result: None });
        }
        1 => {
            let name = &problem.params[0].name;
            for i in range {
                let mut env = Env::new();
                env.insert(name.clone(), Value::Int(i));
                // Check preconditions
                if check_preconditions(&env, &problem.preconditions) {
                    cases.push(TestCase { inputs: env, expected_result: None });
                }
                if cases.len() >= count {
                    break;
                }
            }
        }
        _ => {
            let name0 = &problem.params[0].name;
            let name1 = &problem.params[1].name;
            let small_range = -3i64..=3;
            for i in small_range.clone() {
                for j in small_range.clone() {
                    let mut env = Env::new();
                    env.insert(name0.clone(), Value::Int(i));
                    env.insert(name1.clone(), Value::Int(j));
                    if check_preconditions(&env, &problem.preconditions) {
                        cases.push(TestCase { inputs: env, expected_result: None });
                    }
                    if cases.len() >= count {
                        break;
                    }
                }
                if cases.len() >= count {
                    break;
                }
            }
        }
    }
    cases
}

fn check_preconditions(env: &Env, preconds: &[SpecExpr]) -> bool {
    for pre in preconds {
        match eval_spec(pre, env) {
            Ok(Value::Bool(true)) => {}
            _ => return false,
        }
    }
    true
}

/// Try to fill a single integer hole by enumeration.
fn try_fill_int_hole(
    template: &CodeExpr,
    hole_name: &str,
    test_cases: &[TestCase],
    postconditions: &[SpecExpr],
) -> Option<i64> {
    // Try constants from -20 to 20
    for c in -20i64..=20 {
        let mut mapping = HashMap::new();
        mapping.insert(hole_name.to_string(), CodeExpr::Lit(c));
        let filled = template.fill_holes(&mapping);

        let mut all_pass = true;
        for tc in test_cases {
            let mut env = tc.inputs.clone();
            match eval_code(&filled, &env) {
                Ok(result) => {
                    env.insert("__result".to_string(), result);
                    for post in postconditions {
                        match eval_spec(post, &env) {
                            Ok(Value::Bool(true)) => {}
                            _ => { all_pass = false; break; }
                        }
                    }
                }
                Err(_) => { all_pass = false; break; }
            }
            if !all_pass { break; }
        }
        if all_pass && !test_cases.is_empty() {
            return Some(c);
        }
    }
    None
}

/// Synthesize a candidate implementation.
pub fn synthesize(problem: &SynthProblem) -> Vec<SynthResult> {
    let templates = generate_templates(problem);
    let test_cases = generate_test_cases(problem, 20);
    let mut results = Vec::new();

    if test_cases.is_empty() {
        return results;
    }

    for template in &templates {
        let holes = template.holes();

        if holes.is_empty() {
            // No holes: just verify the template
            let verified = verify_candidate(template, &test_cases, &problem.postconditions);
            if verified > 0 {
                let confidence = if verified == test_cases.len() {
                    SynthConfidence::High
                } else if verified * 2 >= test_cases.len() {
                    SynthConfidence::Medium
                } else {
                    SynthConfidence::Low
                };
                results.push(SynthResult {
                    expression: template.clone(),
                    verified_cases: verified,
                    confidence,
                });
            }
        } else if holes.len() == 1 {
            // Single hole: try enumeration
            let hole_name = &holes[0];
            if let Some(c) = try_fill_int_hole(template, hole_name, &test_cases, &problem.postconditions) {
                let mut mapping = HashMap::new();
                mapping.insert(hole_name.clone(), CodeExpr::Lit(c));
                let filled = template.fill_holes(&mapping);
                let verified = verify_candidate(&filled, &test_cases, &problem.postconditions);
                let confidence = if verified == test_cases.len() {
                    SynthConfidence::High
                } else {
                    SynthConfidence::Medium
                };
                results.push(SynthResult {
                    expression: filled,
                    verified_cases: verified,
                    confidence,
                });
            }
        }
    }

    // Sort by confidence then verified cases
    results.sort_by(|a, b| {
        let conf_ord = (a.confidence as u8).cmp(&(b.confidence as u8));
        if conf_ord == std::cmp::Ordering::Equal {
            b.verified_cases.cmp(&a.verified_cases)
        } else {
            conf_ord
        }
    });

    results
}

/// Count how many test cases a candidate passes.
fn verify_candidate(
    candidate: &CodeExpr,
    test_cases: &[TestCase],
    postconditions: &[SpecExpr],
) -> usize {
    let mut passed = 0;
    for tc in test_cases {
        let mut env = tc.inputs.clone();
        match eval_code(candidate, &env) {
            Ok(result) => {
                env.insert("__result".to_string(), result);
                let all_post = postconditions.iter().all(|post| {
                    matches!(eval_spec(post, &env), Ok(Value::Bool(true)))
                });
                if all_post {
                    passed += 1;
                }
            }
            Err(_) => {}
        }
    }
    passed
}

// ── Oracle Pipeline ──────────────────────────────────────────────────

/// Full synthesis oracle result.
#[derive(Debug)]
pub struct OracleResult {
    pub problem_name: String,
    pub candidates: Vec<SynthResult>,
    pub best: Option<SynthResult>,
    pub template_count: usize,
    pub test_case_count: usize,
}

impl fmt::Display for OracleResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Synthesis oracle for `{}`:", self.problem_name)?;
        writeln!(f, "  templates tried: {}, test cases: {}",
            self.template_count, self.test_case_count)?;
        writeln!(f, "  candidates found: {}", self.candidates.len())?;
        if let Some(ref best) = self.best {
            writeln!(f, "  best: {best}")?;
        }
        Ok(())
    }
}

/// Run the full synthesis oracle on a problem.
pub fn oracle(problem: &SynthProblem) -> OracleResult {
    let templates = generate_templates(problem);
    let template_count = templates.len();
    let test_cases = generate_test_cases(problem, 20);
    let test_case_count = test_cases.len();
    let candidates = synthesize(problem);
    let best = candidates.first().cloned();

    OracleResult {
        problem_name: problem.name.clone(),
        candidates,
        best,
        template_count,
        test_case_count,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // spec inc(x: i64) -> i64 { @req true; @ens result == x + 1 }
    fn inc_problem() -> SynthProblem {
        SynthProblem::new("inc", SynthType::Int)
            .param("x", SynthType::Int)
            .post(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Eq,
                Box::new(SpecExpr::BinOp(
                    Box::new(SpecExpr::Var("x".into())),
                    BinOp::Add,
                    Box::new(SpecExpr::IntLit(1)),
                )),
            ))
    }

    // spec double(x: i64) -> i64 { @req true; @ens result == x * 2 }
    fn double_problem() -> SynthProblem {
        SynthProblem::new("double", SynthType::Int)
            .param("x", SynthType::Int)
            .post(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Eq,
                Box::new(SpecExpr::BinOp(
                    Box::new(SpecExpr::Var("x".into())),
                    BinOp::Mul,
                    Box::new(SpecExpr::IntLit(2)),
                )),
            ))
    }

    // spec abs(x: i64) -> i64 { @req true; @ens result >= 0; @ens result >= x; @ens result >= -x }
    fn abs_problem() -> SynthProblem {
        SynthProblem::new("abs", SynthType::Int)
            .param("x", SynthType::Int)
            .post(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Ge,
                Box::new(SpecExpr::IntLit(0)),
            ))
            .post(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Ge,
                Box::new(SpecExpr::Var("x".into())),
            ))
            .post(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Ge,
                Box::new(SpecExpr::UnOp(UnOp::Neg, Box::new(SpecExpr::Var("x".into())))),
            ))
    }

    #[test]
    fn test_eval_spec_int() {
        let env = Env::new();
        assert_eq!(eval_spec(&SpecExpr::IntLit(42), &env).unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_spec_bool() {
        let env = Env::new();
        assert_eq!(eval_spec(&SpecExpr::BoolLit(true), &env).unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_eval_spec_var() {
        let mut env = Env::new();
        env.insert("x".into(), Value::Int(5));
        assert_eq!(eval_spec(&SpecExpr::Var("x".into()), &env).unwrap(), Value::Int(5));
    }

    #[test]
    fn test_eval_spec_binop() {
        let mut env = Env::new();
        env.insert("x".into(), Value::Int(3));
        let expr = SpecExpr::BinOp(
            Box::new(SpecExpr::Var("x".into())),
            BinOp::Add,
            Box::new(SpecExpr::IntLit(1)),
        );
        assert_eq!(eval_spec(&expr, &env).unwrap(), Value::Int(4));
    }

    #[test]
    fn test_eval_spec_comparison() {
        let mut env = Env::new();
        env.insert("x".into(), Value::Int(5));
        let expr = SpecExpr::BinOp(
            Box::new(SpecExpr::Var("x".into())),
            BinOp::Gt,
            Box::new(SpecExpr::IntLit(0)),
        );
        assert_eq!(eval_spec(&expr, &env).unwrap(), Value::Bool(true));
    }

    #[test]
    fn test_eval_spec_unop() {
        let env = Env::new();
        let expr = SpecExpr::UnOp(UnOp::Neg, Box::new(SpecExpr::IntLit(5)));
        assert_eq!(eval_spec(&expr, &env).unwrap(), Value::Int(-5));
    }

    #[test]
    fn test_eval_spec_old() {
        let mut env = Env::new();
        env.insert("x".into(), Value::Int(10));
        env.insert("__old_x".into(), Value::Int(5));
        let expr = SpecExpr::Old(Box::new(SpecExpr::Var("x".into())));
        assert_eq!(eval_spec(&expr, &env).unwrap(), Value::Int(5));
    }

    #[test]
    fn test_eval_spec_ite() {
        let env = Env::new();
        let expr = SpecExpr::Ite(
            Box::new(SpecExpr::BoolLit(true)),
            Box::new(SpecExpr::IntLit(1)),
            Box::new(SpecExpr::IntLit(2)),
        );
        assert_eq!(eval_spec(&expr, &env).unwrap(), Value::Int(1));
    }

    #[test]
    fn test_eval_code_lit() {
        assert_eq!(eval_code(&CodeExpr::Lit(42), &Env::new()).unwrap(), Value::Int(42));
    }

    #[test]
    fn test_eval_code_binop() {
        let mut env = Env::new();
        env.insert("x".into(), Value::Int(3));
        let expr = CodeExpr::BinOp(
            Box::new(CodeExpr::Var("x".into())),
            BinOp::Add,
            Box::new(CodeExpr::Lit(1)),
        );
        assert_eq!(eval_code(&expr, &env).unwrap(), Value::Int(4));
    }

    #[test]
    fn test_code_expr_holes() {
        let expr = CodeExpr::BinOp(
            Box::new(CodeExpr::Var("x".into())),
            BinOp::Add,
            Box::new(CodeExpr::Hole("c".into(), SynthType::Int)),
        );
        assert_eq!(expr.holes(), vec!["c"]);
        assert!(!expr.is_complete());
    }

    #[test]
    fn test_code_expr_fill_holes() {
        let template = CodeExpr::BinOp(
            Box::new(CodeExpr::Var("x".into())),
            BinOp::Add,
            Box::new(CodeExpr::Hole("c".into(), SynthType::Int)),
        );
        let mut mapping = HashMap::new();
        mapping.insert("c".into(), CodeExpr::Lit(1));
        let filled = template.fill_holes(&mapping);
        assert!(filled.is_complete());
        assert_eq!(format!("{filled}"), "(x + 1)");
    }

    #[test]
    fn test_code_expr_size() {
        let expr = CodeExpr::BinOp(
            Box::new(CodeExpr::Var("x".into())),
            BinOp::Add,
            Box::new(CodeExpr::Lit(1)),
        );
        assert_eq!(expr.size(), 3);
    }

    #[test]
    fn test_generate_templates() {
        let problem = inc_problem();
        let templates = generate_templates(&problem);
        assert!(!templates.is_empty());
    }

    #[test]
    fn test_generate_test_cases() {
        let problem = inc_problem();
        let cases = generate_test_cases(&problem, 10);
        assert!(!cases.is_empty());
        assert!(cases.len() <= 10);
    }

    #[test]
    fn test_generate_test_cases_with_precondition() {
        // @req x > 0
        let problem = SynthProblem::new("pos_inc", SynthType::Int)
            .param("x", SynthType::Int)
            .pre(SpecExpr::BinOp(
                Box::new(SpecExpr::Var("x".into())),
                BinOp::Gt,
                Box::new(SpecExpr::IntLit(0)),
            ));
        let cases = generate_test_cases(&problem, 10);
        for tc in &cases {
            let x = match tc.inputs.get("x") {
                Some(Value::Int(n)) => *n,
                _ => panic!("expected int"),
            };
            assert!(x > 0, "precondition violated: x={x}");
        }
    }

    #[test]
    fn test_synthesize_inc() {
        let problem = inc_problem();
        let results = synthesize(&problem);
        assert!(!results.is_empty(), "should find at least one candidate for inc");
        // The best result should be x + 1
        let best = &results[0];
        assert_eq!(best.confidence, SynthConfidence::High);
    }

    #[test]
    fn test_synthesize_double() {
        let problem = double_problem();
        let results = synthesize(&problem);
        assert!(!results.is_empty(), "should find at least one candidate for double");
        let best = &results[0];
        assert_eq!(best.confidence, SynthConfidence::High);
    }

    #[test]
    fn test_synthesize_abs() {
        let problem = abs_problem();
        let results = synthesize(&problem);
        assert!(!results.is_empty(), "should find candidate for abs");
    }

    #[test]
    fn test_oracle_pipeline() {
        let problem = inc_problem();
        let result = oracle(&problem);
        assert_eq!(result.problem_name, "inc");
        assert!(result.template_count > 0);
        assert!(result.test_case_count > 0);
        assert!(result.best.is_some());
    }

    #[test]
    fn test_oracle_display() {
        let problem = inc_problem();
        let result = oracle(&problem);
        let s = format!("{result}");
        assert!(s.contains("inc"));
        assert!(s.contains("templates tried"));
    }

    #[test]
    fn test_synth_result_display() {
        let r = SynthResult {
            expression: CodeExpr::Lit(42),
            verified_cases: 10,
            confidence: SynthConfidence::High,
        };
        let s = format!("{r}");
        assert!(s.contains("42"));
        assert!(s.contains("high"));
    }

    #[test]
    fn test_confidence_display() {
        assert_eq!(format!("{}", SynthConfidence::High), "high");
        assert_eq!(format!("{}", SynthConfidence::Medium), "medium");
        assert_eq!(format!("{}", SynthConfidence::Low), "low");
    }

    #[test]
    fn test_value_display() {
        assert_eq!(format!("{}", Value::Int(42)), "42");
        assert_eq!(format!("{}", Value::Bool(true)), "true");
    }

    #[test]
    fn test_spec_expr_display() {
        let e = SpecExpr::Ite(
            Box::new(SpecExpr::BoolLit(true)),
            Box::new(SpecExpr::IntLit(1)),
            Box::new(SpecExpr::IntLit(2)),
        );
        let s = format!("{e}");
        assert!(s.contains("if true"));
    }

    #[test]
    fn test_synth_type_display() {
        assert_eq!(format!("{}", SynthType::Int), "i64");
        assert_eq!(format!("{}", SynthType::Bool), "bool");
        assert_eq!(format!("{}", SynthType::Array(Box::new(SynthType::Int))), "[i64]");
        assert_eq!(format!("{}", SynthType::Tuple(vec![SynthType::Int, SynthType::Bool])), "(i64, bool)");
    }

    #[test]
    fn test_binop_display() {
        assert_eq!(format!("{}", BinOp::Add), "+");
        assert_eq!(format!("{}", BinOp::Eq), "==");
        assert_eq!(format!("{}", BinOp::And), "&&");
    }

    #[test]
    fn test_code_expr_display() {
        let e = CodeExpr::Ite(
            Box::new(CodeExpr::BoolLit(true)),
            Box::new(CodeExpr::Lit(1)),
            Box::new(CodeExpr::Lit(2)),
        );
        let s = format!("{e}");
        assert!(s.contains("if true"));
    }

    #[test]
    fn test_hole_display() {
        let h = CodeExpr::Hole("c".into(), SynthType::Int);
        assert_eq!(format!("{h}"), "??c:i64");
    }

    #[test]
    fn test_eval_div_by_zero() {
        let env = Env::new();
        let expr = SpecExpr::BinOp(
            Box::new(SpecExpr::IntLit(1)),
            BinOp::Div,
            Box::new(SpecExpr::IntLit(0)),
        );
        assert!(eval_spec(&expr, &env).is_err());
    }

    #[test]
    fn test_eval_hole_error() {
        let expr = CodeExpr::Hole("x".into(), SynthType::Int);
        assert!(eval_code(&expr, &Env::new()).is_err());
    }

    #[test]
    fn test_verify_candidate() {
        let candidate = CodeExpr::BinOp(
            Box::new(CodeExpr::Var("x".into())),
            BinOp::Add,
            Box::new(CodeExpr::Lit(1)),
        );
        let post = SpecExpr::BinOp(
            Box::new(SpecExpr::Result),
            BinOp::Eq,
            Box::new(SpecExpr::BinOp(
                Box::new(SpecExpr::Var("x".into())),
                BinOp::Add,
                Box::new(SpecExpr::IntLit(1)),
            )),
        );
        let mut env = Env::new();
        env.insert("x".into(), Value::Int(5));
        let cases = vec![TestCase { inputs: env, expected_result: None }];
        let verified = verify_candidate(&candidate, &cases, &[post]);
        assert_eq!(verified, 1);
    }

    #[test]
    fn test_two_param_problem() {
        // spec add(a, b) -> i64 { @ens result == a + b }
        let problem = SynthProblem::new("add", SynthType::Int)
            .param("a", SynthType::Int)
            .param("b", SynthType::Int)
            .post(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Eq,
                Box::new(SpecExpr::BinOp(
                    Box::new(SpecExpr::Var("a".into())),
                    BinOp::Add,
                    Box::new(SpecExpr::Var("b".into())),
                )),
            ));
        let results = synthesize(&problem);
        assert!(!results.is_empty());
        let best = &results[0];
        assert_eq!(best.confidence, SynthConfidence::High);
    }

    #[test]
    fn test_no_param_problem() {
        let problem = SynthProblem::new("const5", SynthType::Int)
            .post(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Eq,
                Box::new(SpecExpr::IntLit(5)),
            ));
        let results = synthesize(&problem);
        assert!(!results.is_empty(), "should find constant 5");
    }
}
