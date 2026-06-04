//! RMIL debugger — step-through execution with breakpoints and tracing.
//!
//! Wraps the VM to provide:
//! - **Breakpoints** on opcodes, expression depth, or custom predicates
//! - **Step-by-step** execution (step-in, step-over, step-out, continue)
//! - **Execution trace** with full expression → value history
//! - **Watch expressions** that are evaluated at each step
//! - **Variable inspection** of the current scope
//!
//! # Examples
//!
//! ```
//! use rmi::lang::debugger::{Debugger, Breakpoint};
//! use rmi::lang::{Expr, Op};
//!
//! let mut dbg = Debugger::new();
//! dbg.add_breakpoint(Breakpoint::on_op(Op::ADD));
//!
//! let program = Expr::op2(Op::ADD, Expr::int(2), Expr::int(3));
//! let trace = dbg.run(&program).unwrap();
//! assert!(trace.len() > 0);
//! ```

use crate::lang::expr::{Expr, Val};
use crate::lang::op::Op;
use crate::lang::sym::{Sym, SymbolTable};
use std::collections::HashMap;

// ── Breakpoint ───────────────────────────────────────────────────────────────

/// A debugger breakpoint that halts execution when triggered.
#[derive(Debug, Clone)]
pub enum Breakpoint {
    /// Break when a specific opcode is about to execute.
    OpCode(Op),
    /// Break when expression evaluation depth exceeds a threshold.
    Depth(usize),
    /// Break on every Nth step.
    EveryN(usize),
    /// Break when an expression node is a Cond.
    OnCond,
    /// Break on Let bindings.
    OnLet,
    /// Break on Seq (pipeline) nodes.
    OnSeq,
    /// Break on Par (parallel) nodes.
    OnPar,
    /// Break on any App node.
    OnApp,
}

impl Breakpoint {
    /// Convenience: break on a specific opcode.
    pub fn on_op(op: Op) -> Self {
        Self::OpCode(op)
    }

    /// Convenience: break at a certain evaluation depth.
    pub fn at_depth(depth: usize) -> Self {
        Self::Depth(depth)
    }

    /// Convenience: break every N steps.
    pub fn every(n: usize) -> Self {
        Self::EveryN(n)
    }

    /// Check whether this breakpoint fires for the given context.
    fn should_break(&self, ctx: &StepContext) -> bool {
        match self {
            Breakpoint::OpCode(op) => {
                if let Expr::App(expr_op, _) = ctx.expr {
                    expr_op == op
                } else {
                    false
                }
            }
            Breakpoint::Depth(d) => ctx.depth >= *d,
            Breakpoint::EveryN(n) => *n > 0 && ctx.step_count % *n == 0,
            Breakpoint::OnCond => matches!(ctx.expr, Expr::Cond { .. }),
            Breakpoint::OnLet => matches!(ctx.expr, Expr::Let { .. }),
            Breakpoint::OnSeq => matches!(ctx.expr, Expr::Seq(_, _)),
            Breakpoint::OnPar => matches!(ctx.expr, Expr::Par(_, _)),
            Breakpoint::OnApp => matches!(ctx.expr, Expr::App(_, _)),
        }
    }
}

// ── Step context / trace entry ───────────────────────────────────────────────

/// Context available at each evaluation step.
struct StepContext<'a> {
    expr: &'a Expr,
    depth: usize,
    step_count: usize,
}

/// A single entry in the execution trace.
#[derive(Debug, Clone)]
pub struct TraceEntry {
    /// Step number (0-based).
    pub step: usize,
    /// Evaluation depth when this step occurred.
    pub depth: usize,
    /// Short description of the expression being evaluated.
    pub expr_label: String,
    /// The result of evaluating the expression.
    pub result: Val,
    /// Whether a breakpoint fired at this step.
    pub breakpoint_hit: bool,
    /// Watch expression results at this step.
    pub watches: Vec<(String, Val)>,
}

/// Execution mode for the debugger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum StepMode {
    /// Execute to end or next breakpoint.
    Continue,
    /// Step into the next expression.
    StepIn,
    /// Step over (evaluate children without stopping).
    StepOver { start_depth: usize },
    /// Step out to the parent.
    StepOut { target_depth: usize },
}

// ── Debugger ─────────────────────────────────────────────────────────────────

/// Debug-mode RMIL evaluator with breakpoints and tracing.
pub struct Debugger {
    /// Interned symbols.
    pub symbols: SymbolTable,
    /// Breakpoints.
    breakpoints: Vec<Breakpoint>,
    /// Watch expressions (label → expression to evaluate at each step).
    watches: Vec<(String, Expr)>,
    /// Execution trace.
    trace: Vec<TraceEntry>,
    /// Variable environment (scope stack).
    env: Vec<HashMap<Sym, Val>>,
    /// Current evaluation depth.
    depth: usize,
    /// Step counter.
    step_count: usize,
    /// Maximum evaluation depth (stack overflow guard).
    pub max_depth: usize,
    /// Current step mode.
    step_mode: StepMode,
    /// Whether tracing is enabled (recording each step).
    pub tracing_enabled: bool,
    /// Maximum number of trace entries to keep (0 = unlimited).
    pub max_trace_entries: usize,
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

impl Debugger {
    /// Create a new debugger with default settings.
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            breakpoints: Vec::new(),
            watches: Vec::new(),
            trace: Vec::new(),
            env: vec![HashMap::new()],
            depth: 0,
            step_count: 0,
            max_depth: 256,
            step_mode: StepMode::Continue,
            tracing_enabled: true,
            max_trace_entries: 10_000,
        }
    }

    // ── Breakpoint management ────────────────────────────────────────────

    /// Add a breakpoint.
    pub fn add_breakpoint(&mut self, bp: Breakpoint) {
        self.breakpoints.push(bp);
    }

    /// Remove all breakpoints.
    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    /// Number of active breakpoints.
    pub fn breakpoint_count(&self) -> usize {
        self.breakpoints.len()
    }

    // ── Watch management ─────────────────────────────────────────────────

    /// Add a watch expression (evaluated at every step).
    pub fn add_watch(&mut self, label: &str, expr: Expr) {
        self.watches.push((label.to_string(), expr));
    }

    /// Remove all watches.
    pub fn clear_watches(&mut self) {
        self.watches.clear();
    }

    // ── Trace access ─────────────────────────────────────────────────────

    /// Get the full execution trace.
    pub fn trace(&self) -> &[TraceEntry] {
        &self.trace
    }

    /// Number of steps executed.
    pub fn step_count(&self) -> usize {
        self.step_count
    }

    /// Clear the trace buffer.
    pub fn clear_trace(&mut self) {
        self.trace.clear();
    }

    // ── Variable inspection ──────────────────────────────────────────────

    /// Get a variable from the current scope.
    pub fn get_var(&self, sym: Sym) -> Option<&Val> {
        for frame in self.env.iter().rev() {
            if let Some(v) = frame.get(&sym) {
                return Some(v);
            }
        }
        None
    }

    /// List all variables in scope.
    pub fn all_vars(&self) -> Vec<(Sym, Val)> {
        let mut result = HashMap::new();
        for frame in &self.env {
            for (k, v) in frame {
                result.insert(*k, v.clone());
            }
        }
        result.into_iter().collect()
    }

    /// Current evaluation depth.
    pub fn current_depth(&self) -> usize {
        self.depth
    }

    // ── Execution ────────────────────────────────────────────────────────

    /// Run a program to completion, collecting a trace.
    ///
    /// Breakpoints will fire and be recorded but execution continues.
    pub fn run(&mut self, expr: &Expr) -> Result<Vec<TraceEntry>, DebugError> {
        self.depth = 0;
        self.step_count = 0;
        self.trace.clear();
        self.env = vec![HashMap::new()];
        self.step_mode = StepMode::Continue;

        let _result = self.eval(expr)?;
        Ok(self.trace.clone())
    }

    /// Run and return both the final value and the trace.
    pub fn run_with_result(&mut self, expr: &Expr) -> Result<(Val, Vec<TraceEntry>), DebugError> {
        self.depth = 0;
        self.step_count = 0;
        self.trace.clear();
        self.env = vec![HashMap::new()];
        self.step_mode = StepMode::Continue;

        let result = self.eval(expr)?;
        Ok((result, self.trace.clone()))
    }

    /// Step into the next sub-expression. Returns the trace up to and
    /// including this step.
    pub fn step_in(&mut self, expr: &Expr) -> Result<Vec<TraceEntry>, DebugError> {
        self.step_mode = StepMode::StepIn;
        self.depth = 0;
        self.step_count = 0;
        self.trace.clear();
        self.env = vec![HashMap::new()];
        let _result = self.eval(expr)?;
        Ok(self.trace.clone())
    }

    // ── Internal evaluation ──────────────────────────────────────────────

    fn eval(&mut self, expr: &Expr) -> Result<Val, DebugError> {
        self.depth += 1;
        if self.depth > self.max_depth {
            self.depth -= 1;
            return Err(DebugError::StackOverflow);
        }

        // Check breakpoints
        let ctx = StepContext {
            expr,
            depth: self.depth,
            step_count: self.step_count,
        };
        let bp_hit = self.breakpoints.iter().any(|bp| bp.should_break(&ctx));

        // Evaluate watches
        let watch_results = self.eval_watches();

        // Evaluate the expression
        let result = self.eval_inner(expr)?;

        // Record trace entry
        if self.tracing_enabled {
            let entry = TraceEntry {
                step: self.step_count,
                depth: self.depth,
                expr_label: expr_label(expr),
                result: result.clone(),
                breakpoint_hit: bp_hit,
                watches: watch_results,
            };
            if self.max_trace_entries == 0 || self.trace.len() < self.max_trace_entries {
                self.trace.push(entry);
            }
        }

        self.step_count += 1;
        self.depth -= 1;
        Ok(result)
    }

    fn eval_watches(&self) -> Vec<(String, Val)> {
        if self.watches.is_empty() {
            return Vec::new();
        }
        // Evaluate each watch expression in the current env.
        // Use a very limited approach: only resolve Lit and Ref.
        let mut results = Vec::new();
        for (label, watch_expr) in &self.watches {
            let val = self.eval_watch_simple(watch_expr);
            results.push((label.clone(), val));
        }
        results
    }

    /// Simple watch evaluator (handles Lit/Ref only to avoid recursion).
    fn eval_watch_simple(&self, expr: &Expr) -> Val {
        match expr {
            Expr::Lit(v) => v.clone(),
            Expr::Ref(sym) => self.get_var(*sym).cloned().unwrap_or(Val::Nil),
            _ => Val::Nil, // complex expressions not supported as watches
        }
    }

    fn eval_inner(&mut self, expr: &Expr) -> Result<Val, DebugError> {
        match expr {
            Expr::Lit(val) => Ok(val.clone()),

            Expr::Ref(sym) => self
                .get_var(*sym)
                .cloned()
                .ok_or(DebugError::UnboundSymbol(*sym)),

            Expr::App(op, args) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a)?);
                }
                self.exec_op(*op, vals)
            }

            Expr::Seq(a, b) => {
                let _left = self.eval(a)?;
                self.eval(b)
            }

            Expr::Par(a, b) => {
                let va = self.eval(a)?;
                let vb = self.eval(b)?;
                Ok(Val::Tuple(vec![va, vb]))
            }

            Expr::Cond { pred, yes, no } => {
                let p = self.eval(pred)?;
                match p {
                    Val::Bool(true) => self.eval(yes),
                    Val::Bool(false) => self.eval(no),
                    _ => Err(DebugError::TypeMismatch {
                        op: "cond",
                        expected: "Bool",
                    }),
                }
            }

            Expr::Let { name, val, body } => {
                let v = self.eval(val)?;
                self.env.push(HashMap::new());
                self.env.last_mut().expect("env stack empty in Let").insert(*name, v);
                let result = self.eval(body);
                self.env.pop();
                result
            }

            Expr::Lam { .. } => Ok(Val::Nil),

            Expr::Call(_func, args) => {
                let mut result = Val::Nil;
                for a in args {
                    result = self.eval(a)?;
                }
                Ok(result)
            }

            Expr::Block(exprs) => {
                let mut result = Val::Nil;
                for e in exprs {
                    result = self.eval(e)?;
                }
                Ok(result)
            }
        }
    }

    // ── Op execution (mirrors VM but produces DebugError) ────────────────

    fn exec_op(&self, op: Op, args: Vec<Val>) -> Result<Val, DebugError> {
        match op {
            Op::ADD => self.binary_arith(&args, "add", |a, b| a + b, |a, b| a + b),
            Op::SUB => self.binary_arith(&args, "sub", |a, b| a - b, |a, b| a - b),
            Op::MUL => self.binary_arith(&args, "mul", |a, b| a * b, |a, b| a * b),
            Op::DIV => {
                if args.len() != 2 {
                    return Err(DebugError::ArityMismatch {
                        op: "div",
                        expected: 2,
                        got: args.len(),
                    });
                }
                if let (Val::I64(_), Val::I64(0)) = (&args[0], &args[1]) {
                    return Err(DebugError::DivisionByZero);
                }
                self.binary_arith(&args, "div", |a, b| a / b, |a, b| a / b)
            }
            Op::NEG => {
                self.require_args("neg", &args, 1)?;
                match &args[0] {
                    Val::I64(v) => Ok(Val::I64(-v)),
                    Val::F32(bits) => Ok(Val::f32(-f32::from_bits(*bits))),
                    Val::F64(bits) => Ok(Val::f64(-f64::from_bits(*bits))),
                    _ => Err(DebugError::TypeMismatch {
                        op: "neg",
                        expected: "numeric",
                    }),
                }
            }
            Op::ABS => {
                self.require_args("abs", &args, 1)?;
                match &args[0] {
                    Val::I64(v) => Ok(Val::I64(v.abs())),
                    Val::F32(bits) => Ok(Val::f32(f32::from_bits(*bits).abs())),
                    Val::F64(bits) => Ok(Val::f64(f64::from_bits(*bits).abs())),
                    _ => Err(DebugError::TypeMismatch {
                        op: "abs",
                        expected: "numeric",
                    }),
                }
            }
            Op::EXP | Op::LOG | Op::SQRT | Op::SIN | Op::COS => {
                self.require_args(op.name(), &args, 1)?;
                match &args[0] {
                    Val::F32(bits) => {
                        let v = f32::from_bits(*bits);
                        let r = match op {
                            Op::EXP => v.exp(),
                            Op::LOG => v.ln(),
                            Op::SQRT => v.sqrt(),
                            Op::SIN => v.sin(),
                            Op::COS => v.cos(),
                            _ => unreachable!(),
                        };
                        Ok(Val::f32(r))
                    }
                    Val::F64(bits) => {
                        let v = f64::from_bits(*bits);
                        let r = match op {
                            Op::EXP => v.exp(),
                            Op::LOG => v.ln(),
                            Op::SQRT => v.sqrt(),
                            Op::SIN => v.sin(),
                            Op::COS => v.cos(),
                            _ => unreachable!(),
                        };
                        Ok(Val::f64(r))
                    }
                    _ => Err(DebugError::TypeMismatch {
                        op: op.name(),
                        expected: "float",
                    }),
                }
            }
            Op::MAX => {
                self.require_args("max", &args, 2)?;
                match (&args[0], &args[1]) {
                    (Val::I64(a), Val::I64(b)) => Ok(Val::I64(*a.max(b))),
                    (Val::F32(a), Val::F32(b)) => {
                        Ok(Val::f32(f32::from_bits(*a).max(f32::from_bits(*b))))
                    }
                    _ => Err(DebugError::TypeMismatch {
                        op: "max",
                        expected: "matching numeric",
                    }),
                }
            }
            Op::MIN => {
                self.require_args("min", &args, 2)?;
                match (&args[0], &args[1]) {
                    (Val::I64(a), Val::I64(b)) => Ok(Val::I64(*a.min(b))),
                    (Val::F32(a), Val::F32(b)) => {
                        Ok(Val::f32(f32::from_bits(*a).min(f32::from_bits(*b))))
                    }
                    _ => Err(DebugError::TypeMismatch {
                        op: "min",
                        expected: "matching numeric",
                    }),
                }
            }
            Op::RELU => {
                self.require_args("relu", &args, 1)?;
                match &args[0] {
                    Val::F32(bits) => {
                        let v = f32::from_bits(*bits);
                        Ok(Val::f32(if v > 0.0 { v } else { 0.0 }))
                    }
                    Val::I64(v) => Ok(Val::I64((*v).max(0))),
                    _ => Err(DebugError::TypeMismatch {
                        op: "relu",
                        expected: "numeric",
                    }),
                }
            }
            Op::SIGMOID => {
                self.require_args("sigmoid", &args, 1)?;
                match &args[0] {
                    Val::F32(bits) => {
                        let v = f32::from_bits(*bits);
                        Ok(Val::f32(1.0 / (1.0 + (-v).exp())))
                    }
                    _ => Err(DebugError::TypeMismatch {
                        op: "sigmoid",
                        expected: "F32",
                    }),
                }
            }
            Op::IDENTITY => {
                if args.is_empty() {
                    Ok(Val::Nil)
                } else {
                    Ok(args.into_iter().next().expect("IDENTITY op has non-empty args"))
                }
            }
            Op::HASH => {
                if args.is_empty() {
                    Ok(Val::I64(0))
                } else {
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut h = DefaultHasher::new();
                    args[0].hash(&mut h);
                    Ok(Val::I64(h.finish() as i64))
                }
            }
            Op::RES_ADD => {
                self.require_args("res_add", &args, 2)?;
                match (&args[0], &args[1]) {
                    (Val::I64(a), Val::I64(b)) => Ok(Val::I64(a + b)),
                    (Val::F32(a), Val::F32(b)) => {
                        Ok(Val::f32(f32::from_bits(*a) + f32::from_bits(*b)))
                    }
                    _ => Ok(Val::Tuple(args)),
                }
            }
            Op::CONCAT => {
                let mut result = Vec::new();
                for a in args {
                    match a {
                        Val::Tuple(vs) => result.extend(vs),
                        other => result.push(other),
                    }
                }
                Ok(Val::Tuple(result))
            }
            // All other ops return Nil (stubs)
            _ => Ok(Val::Nil),
        }
    }

    fn require_args(&self, op: &'static str, args: &[Val], n: usize) -> Result<(), DebugError> {
        if args.len() != n {
            Err(DebugError::ArityMismatch {
                op,
                expected: n,
                got: args.len(),
            })
        } else {
            Ok(())
        }
    }

    fn binary_arith(
        &self,
        args: &[Val],
        name: &'static str,
        fi: impl Fn(i64, i64) -> i64,
        ff: impl Fn(f32, f32) -> f32,
    ) -> Result<Val, DebugError> {
        if args.len() != 2 {
            return Err(DebugError::ArityMismatch {
                op: name,
                expected: 2,
                got: args.len(),
            });
        }
        match (&args[0], &args[1]) {
            (Val::I64(a), Val::I64(b)) => Ok(Val::I64(fi(*a, *b))),
            (Val::F32(a), Val::F32(b)) => Ok(Val::f32(ff(f32::from_bits(*a), f32::from_bits(*b)))),
            (Val::F64(a), Val::F64(b)) => {
                let fa = f64::from_bits(*a);
                let fb = f64::from_bits(*b);
                Ok(Val::f64(ff(fa as f32, fb as f32) as f64))
            }
            _ => Err(DebugError::TypeMismatch {
                op: name,
                expected: "matching numeric",
            }),
        }
    }
}

// ── Debug error ──────────────────────────────────────────────────────────────

/// Debugger runtime error.
#[derive(Debug)]
pub enum DebugError {
    /// Division by zero.
    DivisionByZero,
    /// Type mismatch.
    TypeMismatch {
        /// Operation name.
        op: &'static str,
        /// What was expected.
        expected: &'static str,
    },
    /// Unbound symbol.
    UnboundSymbol(Sym),
    /// Stack overflow.
    StackOverflow,
    /// Wrong arity.
    ArityMismatch {
        /// Operation name.
        op: &'static str,
        /// Expected arity.
        expected: usize,
        /// Actual arity.
        got: usize,
    },
}

impl std::fmt::Display for DebugError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DivisionByZero => f.write_str("division by zero"),
            Self::TypeMismatch { op, expected } => {
                write!(f, "{op}: expected {expected}")
            }
            Self::UnboundSymbol(s) => write!(f, "unbound symbol: Sym({})", s.0),
            Self::StackOverflow => f.write_str("call stack overflow"),
            Self::ArityMismatch { op, expected, got } => {
                write!(f, "{op}: expected {expected} args, got {got}")
            }
        }
    }
}

impl std::error::Error for DebugError {}

// ── Helper ───────────────────────────────────────────────────────────────────

/// Short human-readable label for an expression node.
fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::Lit(val) => match val {
            Val::Nil => "nil".into(),
            Val::Bool(b) => format!("{b}"),
            Val::I64(n) => format!("{n}"),
            Val::F32(bits) => format!("{:.4}", f32::from_bits(*bits)),
            Val::F64(bits) => format!("{:.4}", f64::from_bits(*bits)),
            Val::Tensor { .. } => "tensor".into(),
            Val::Sym(s) => format!("sym({})", s.0),
            Val::Tuple(vs) => format!("tuple({})", vs.len()),
        },
        Expr::Ref(sym) => format!("ref({})", sym.0),
        Expr::App(op, args) => format!("{}({})", op.name(), args.len()),
        Expr::Seq(_, _) => "seq".into(),
        Expr::Par(_, _) => "par".into(),
        Expr::Cond { .. } => "cond".into(),
        Expr::Let { name, .. } => format!("let({})", name.0),
        Expr::Lam { params, .. } => format!("lam({})", params.len()),
        Expr::Call(_, args) => format!("call({})", args.len()),
        Expr::Block(exprs) => format!("block({})", exprs.len()),
    }
}

// ── Trace analysis helpers ───────────────────────────────────────────────────

/// Summary statistics for a trace.
#[derive(Debug, Clone)]
pub struct TraceSummary {
    /// Total steps executed.
    pub total_steps: usize,
    /// Maximum evaluation depth reached.
    pub max_depth: usize,
    /// Number of breakpoints hit.
    pub breakpoints_hit: usize,
    /// Count of each expression type encountered.
    pub expr_type_counts: HashMap<String, usize>,
}

/// Compute summary statistics from a trace.
pub fn summarize_trace(trace: &[TraceEntry]) -> TraceSummary {
    let mut max_depth = 0;
    let mut bp_hits = 0;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for entry in trace {
        if entry.depth > max_depth {
            max_depth = entry.depth;
        }
        if entry.breakpoint_hit {
            bp_hits += 1;
        }
        // Group by first word of expression label
        let kind = entry
            .expr_label
            .split('(')
            .next()
            .unwrap_or(&entry.expr_label)
            .to_string();
        *counts.entry(kind).or_insert(0) += 1;
    }

    TraceSummary {
        total_steps: trace.len(),
        max_depth,
        breakpoints_hit: bp_hits,
        expr_type_counts: counts,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::expr::Expr;
    use crate::lang::op::Op;

    #[test]
    fn basic_eval() {
        let mut dbg = Debugger::new();
        let program = Expr::op2(Op::ADD, Expr::int(2), Expr::int(3));
        let (result, _trace) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result.as_i64(), Some(5));
    }

    #[test]
    fn trace_records_steps() {
        let mut dbg = Debugger::new();
        let program = Expr::op2(Op::ADD, Expr::int(10), Expr::int(20));
        let trace = dbg.run(&program).unwrap();
        assert!(!trace.is_empty());
        // Final step should have the result
        let last = trace.last().unwrap();
        assert_eq!(last.result.as_i64(), Some(30));
    }

    #[test]
    fn breakpoint_on_op() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::on_op(Op::MUL));
        let program = Expr::op2(Op::MUL, Expr::int(3), Expr::int(4));
        let trace = dbg.run(&program).unwrap();
        assert!(trace.iter().any(|e| e.breakpoint_hit));
    }

    #[test]
    fn breakpoint_on_depth() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::at_depth(3));
        // depth 1: App(ADD), depth 2: Lit(2), Lit(3)
        let program = Expr::op2(Op::ADD, Expr::int(2), Expr::int(3));
        let trace = dbg.run(&program).unwrap();
        // depth 1 for the ADD, depth 2 for the literals — no depth 3
        assert!(!trace.iter().any(|e| e.breakpoint_hit));
    }

    #[test]
    fn breakpoint_at_depth_fires() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::at_depth(2));
        let program = Expr::op2(Op::ADD, Expr::int(2), Expr::int(3));
        let trace = dbg.run(&program).unwrap();
        // Depth 2 reached when evaluating literals inside Add
        assert!(trace.iter().any(|e| e.breakpoint_hit));
    }

    #[test]
    fn breakpoint_every_n() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::every(2));
        let program = Expr::op2(Op::ADD, Expr::int(1), Expr::int(2));
        let trace = dbg.run(&program).unwrap();
        // Step 0 should hit (0 % 2 == 0), step 2 should hit
        let hits: Vec<_> = trace.iter().filter(|e| e.breakpoint_hit).collect();
        assert!(!hits.is_empty());
    }

    #[test]
    fn breakpoint_on_cond() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::OnCond);
        let program = Expr::Cond {
            pred: Box::new(Expr::boolean(true)),
            yes: Box::new(Expr::int(1)),
            no: Box::new(Expr::int(2)),
        };
        let trace = dbg.run(&program).unwrap();
        assert!(trace.iter().any(|e| e.breakpoint_hit));
    }

    #[test]
    fn breakpoint_on_let() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::OnLet);
        let sym = Sym(1);
        let program = Expr::Let {
            name: sym,
            val: Box::new(Expr::int(42)),
            body: Box::new(Expr::Ref(sym)),
        };
        let trace = dbg.run(&program).unwrap();
        assert!(trace.iter().any(|e| e.breakpoint_hit));
    }

    #[test]
    fn clear_breakpoints() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::on_op(Op::ADD));
        dbg.add_breakpoint(Breakpoint::on_op(Op::MUL));
        assert_eq!(dbg.breakpoint_count(), 2);
        dbg.clear_breakpoints();
        assert_eq!(dbg.breakpoint_count(), 0);
    }

    #[test]
    fn let_binding_in_debugger() {
        let mut dbg = Debugger::new();
        let x = Sym(42);
        let program = Expr::Let {
            name: x,
            val: Box::new(Expr::int(10)),
            body: Box::new(Expr::op2(Op::ADD, Expr::Ref(x), Expr::int(5))),
        };
        let (result, _trace) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result.as_i64(), Some(15));
    }

    #[test]
    fn conditional_true() {
        let mut dbg = Debugger::new();
        let program = Expr::Cond {
            pred: Box::new(Expr::boolean(true)),
            yes: Box::new(Expr::int(1)),
            no: Box::new(Expr::int(2)),
        };
        let (result, _) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result.as_i64(), Some(1));
    }

    #[test]
    fn conditional_false() {
        let mut dbg = Debugger::new();
        let program = Expr::Cond {
            pred: Box::new(Expr::boolean(false)),
            yes: Box::new(Expr::int(1)),
            no: Box::new(Expr::int(2)),
        };
        let (result, _) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result.as_i64(), Some(2));
    }

    #[test]
    fn parallel_evaluation() {
        let mut dbg = Debugger::new();
        let program = Expr::Par(Box::new(Expr::int(10)), Box::new(Expr::int(20)));
        let (result, _) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result, Val::Tuple(vec![Val::I64(10), Val::I64(20)]));
    }

    #[test]
    fn sequential_evaluation() {
        let mut dbg = Debugger::new();
        let program = Expr::Seq(Box::new(Expr::int(1)), Box::new(Expr::int(2)));
        let (result, _) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result.as_i64(), Some(2));
    }

    #[test]
    fn block_evaluation() {
        let mut dbg = Debugger::new();
        let program = Expr::Block(vec![Expr::int(1), Expr::int(2), Expr::int(3)]);
        let (result, _) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result.as_i64(), Some(3));
    }

    #[test]
    fn stack_overflow_detection() {
        let mut dbg = Debugger::new();
        dbg.max_depth = 5;
        // Build deeply nested expression
        let mut prog = Expr::int(1);
        for _ in 0..10 {
            prog = Expr::op2(Op::ADD, prog, Expr::int(1));
        }
        let result = dbg.run_with_result(&prog);
        assert!(result.is_err());
    }

    #[test]
    fn division_by_zero_error() {
        let mut dbg = Debugger::new();
        let program = Expr::op2(Op::DIV, Expr::int(10), Expr::int(0));
        let result = dbg.run_with_result(&program);
        assert!(result.is_err());
    }

    #[test]
    fn watch_expression() {
        let mut dbg = Debugger::new();
        dbg.add_watch("literal-42", Expr::int(42));
        let program = Expr::int(1);
        let trace = dbg.run(&program).unwrap();
        assert!(!trace.is_empty());
        let watches = &trace[0].watches;
        assert_eq!(watches.len(), 1);
        assert_eq!(watches[0].0, "literal-42");
        assert_eq!(watches[0].1, Val::I64(42));
    }

    #[test]
    fn trace_summary() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::on_op(Op::ADD));
        let program = Expr::op2(Op::ADD, Expr::int(1), Expr::int(2));
        let trace = dbg.run(&program).unwrap();
        let summary = summarize_trace(&trace);
        assert!(summary.total_steps > 0);
        assert!(summary.max_depth > 0);
        assert_eq!(summary.breakpoints_hit, 1);
    }

    #[test]
    fn expr_label_formatting() {
        assert_eq!(expr_label(&Expr::int(42)), "42");
        assert_eq!(expr_label(&Expr::boolean(true)), "true");
        assert_eq!(expr_label(&Expr::Lit(Val::Nil)), "nil");
        assert_eq!(
            expr_label(&Expr::op2(Op::ADD, Expr::int(1), Expr::int(2))),
            "add(2)"
        );
    }

    #[test]
    fn tracing_disabled() {
        let mut dbg = Debugger::new();
        dbg.tracing_enabled = false;
        let program = Expr::op2(Op::ADD, Expr::int(1), Expr::int(2));
        let trace = dbg.run(&program).unwrap();
        assert!(trace.is_empty());
    }

    #[test]
    fn all_vars() {
        let mut dbg = Debugger::new();
        let x = Sym(1);
        let program = Expr::Let {
            name: x,
            val: Box::new(Expr::int(99)),
            body: Box::new(Expr::Ref(x)),
        };
        let (result, _) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result.as_i64(), Some(99));
    }

    #[test]
    fn step_in_basic() {
        let mut dbg = Debugger::new();
        let program = Expr::op2(Op::ADD, Expr::int(5), Expr::int(6));
        let trace = dbg.step_in(&program).unwrap();
        assert!(!trace.is_empty());
    }

    #[test]
    fn max_trace_entries_limit() {
        let mut dbg = Debugger::new();
        dbg.max_trace_entries = 2;
        // Build a long block
        let program = Expr::Block(vec![
            Expr::int(1),
            Expr::int(2),
            Expr::int(3),
            Expr::int(4),
            Expr::int(5),
        ]);
        let trace = dbg.run(&program).unwrap();
        assert!(trace.len() <= 2);
    }

    #[test]
    fn neural_stubs_return_nil() {
        let mut dbg = Debugger::new();
        let program = Expr::op1(Op::LINEAR);
        let (result, _) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result, Val::Nil);
    }

    #[test]
    fn default_debugger() {
        let dbg = Debugger::default();
        assert_eq!(dbg.max_depth, 256);
        assert_eq!(dbg.step_count(), 0);
        assert!(dbg.tracing_enabled);
    }

    #[test]
    fn clear_watches() {
        let mut dbg = Debugger::new();
        dbg.add_watch("w1", Expr::int(1));
        dbg.add_watch("w2", Expr::int(2));
        dbg.clear_watches();
        let program = Expr::int(1);
        let trace = dbg.run(&program).unwrap();
        assert!(trace[0].watches.is_empty());
    }

    #[test]
    fn multiple_breakpoints() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::on_op(Op::ADD));
        dbg.add_breakpoint(Breakpoint::OnApp);
        let program = Expr::op2(Op::ADD, Expr::int(1), Expr::int(2));
        let trace = dbg.run(&program).unwrap();
        let bp_entries: Vec<_> = trace.iter().filter(|e| e.breakpoint_hit).collect();
        assert!(!bp_entries.is_empty());
    }

    #[test]
    fn breakpoint_on_seq() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::OnSeq);
        let program = Expr::Seq(Box::new(Expr::int(1)), Box::new(Expr::int(2)));
        let trace = dbg.run(&program).unwrap();
        assert!(trace.iter().any(|e| e.breakpoint_hit));
    }

    #[test]
    fn breakpoint_on_par() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(Breakpoint::OnPar);
        let program = Expr::Par(Box::new(Expr::int(1)), Box::new(Expr::int(2)));
        let trace = dbg.run(&program).unwrap();
        assert!(trace.iter().any(|e| e.breakpoint_hit));
    }

    #[test]
    fn activation_relu_in_debugger() {
        let mut dbg = Debugger::new();
        let program = Expr::op(Op::RELU, vec![Expr::Lit(Val::f32(-3.0))]);
        let (result, _) = dbg.run_with_result(&program).unwrap();
        assert_eq!(result, Val::f32(0.0));
    }
}
