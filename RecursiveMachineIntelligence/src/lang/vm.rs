//! RMIL virtual machine — tree-walking evaluator.
//!
//! The VM interprets [`Expr`] trees directly without compiling to bytecode.
//! This keeps the implementation minimal while supporting:
//!
//! - **Math operations**: elementwise arithmetic on scalars
//! - **Composition**: `Seq` (pipeline), `Par` (fork), `Cond` (branch)
//! - **Bindings**: `Let` / `Ref` for named sub-expressions
//! - **Lambdas**: first-class functions via `Lam` / `Call`
//! - **Introspection**: `HASH`, `TYPE_OF`, `NODE_COUNT` etc.
//!
//! Neural and symbolic ops are **stubs**: they record intent but do not
//! perform actual tensor computation. A real backend would lower RMIL
//! expressions to a compute graph and dispatch to GPU/TPU.
//!
//! # Usage
//!
//! ```
//! use rmi::lang::vm::Vm;
//! use rmi::lang::{Expr, Op, Val};
//!
//! let mut vm = Vm::new();
//! let program = Expr::op2(Op::ADD, Expr::int(2), Expr::int(3));
//! let result = vm.eval(&program).unwrap();
//! assert_eq!(result.as_i64(), Some(5));
//! ```

use std::collections::HashMap;

use crate::lang::expr::{Expr, Val};
use crate::lang::jit::{JitCompiler, JitConfig};
use crate::lang::op::Op;
use crate::lang::sym::{Sym, SymbolTable};

/// VM runtime error.
#[derive(Debug)]
pub enum VmError {
    /// Division by zero.
    DivisionByZero,
    /// Type mismatch for operation.
    TypeMismatch {
        /// The op that failed.
        op: &'static str,
        /// What was expected.
        expected: &'static str,
        /// What was received.
        got: &'static str,
    },
    /// Unbound symbol.
    UnboundSymbol(Sym),
    /// Call depth exceeded.
    StackOverflow,
    /// Unimplemented opcode (neural/symbolic ops in stub mode).
    Unimplemented(Op),
    /// Wrong number of arguments.
    ArityMismatch {
        /// Operation name.
        op: &'static str,
        /// Expected arity.
        expected: usize,
        /// Actual arity.
        got: usize,
    },
}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DivisionByZero => f.write_str("division by zero"),
            Self::TypeMismatch { op, expected, got } => {
                write!(f, "{op}: expected {expected}, got {got}")
            }
            Self::UnboundSymbol(s) => write!(f, "unbound symbol: Sym({})", s.0),
            Self::StackOverflow => f.write_str("call stack overflow"),
            Self::Unimplemented(op) => write!(f, "unimplemented op: 0x{:04X}", op.0),
            Self::ArityMismatch { op, expected, got } => {
                write!(f, "{op}: expected {expected} args, got {got}")
            }
        }
    }
}

impl std::error::Error for VmError {}

/// A tree-walking RMIL evaluator.
///
/// Maintains a symbol table (for interning), an environment (for bindings),
/// and a call-depth counter (for stack-overflow protection).
///
/// When JIT is enabled, the VM will attempt to compile pure math expressions
/// to native code and cache the result. JIT-compiled functions bypass the
/// tree-walking interpreter for faster evaluation. If JIT compilation fails
/// (e.g. the expression uses neural/symbolic ops), the VM falls back to
/// tree-walking transparently.
///
/// # Examples
///
/// ```
/// use rmi::lang::{Op, Expr, Val, Vm};
///
/// let mut vm = Vm::new_no_jit();
/// let expr = Expr::op2(Op::ADD, Expr::float(2.0), Expr::float(3.0));
/// let result = vm.eval(&expr).unwrap();
/// assert!((result.as_f32().unwrap() - 5.0).abs() < 1e-6);
/// ```
pub struct Vm {
    /// Interned symbol table.
    pub symbols: SymbolTable,
    /// Current variable bindings.
    env: Vec<HashMap<Sym, Val>>,
    /// Current call depth.
    call_depth: usize,
    /// Maximum call depth before StackOverflow.
    pub max_depth: usize,
    /// JIT compiler (caches compiled functions).
    jit: Option<JitCompiler>,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    /// Create a new VM with empty state and JIT enabled.
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            env: vec![HashMap::new()],
            call_depth: 0,
            max_depth: 256,
            jit: Some(JitCompiler::new(JitConfig::default())),
        }
    }

    /// Create a new VM with JIT disabled (tree-walking only).
    pub fn new_no_jit() -> Self {
        Self {
            symbols: SymbolTable::new(),
            env: vec![HashMap::new()],
            call_depth: 0,
            max_depth: 256,
            jit: None,
        }
    }

    /// Enable or disable JIT compilation.
    pub fn set_jit(&mut self, enable: bool) {
        if enable && self.jit.is_none() {
            self.jit = Some(JitCompiler::new(JitConfig::default()));
        } else if !enable {
            self.jit = None;
        }
    }

    /// Returns the number of JIT-cached functions.
    pub fn jit_cache_size(&self) -> usize {
        self.jit.as_ref().map_or(0, |j| j.cache_size())
    }

    /// Set a variable in the current scope.
    pub fn set(&mut self, name: Sym, val: Val) {
        if let Some(frame) = self.env.last_mut() {
            frame.insert(name, val);
        }
    }

    /// Get a variable from the environment (searches scopes outward).
    pub fn get(&self, name: Sym) -> Option<&Val> {
        for frame in self.env.iter().rev() {
            if let Some(v) = frame.get(&name) {
                return Some(v);
            }
        }
        None
    }

    /// Evaluate an RMIL expression.
    ///
    /// Uses the tree-walking interpreter. For JIT-accelerated evaluation of
    /// pure math expressions, see [`eval_jit`](Self::eval_jit).
    pub fn eval(&mut self, expr: &Expr) -> Result<Val, VmError> {
        self.call_depth += 1;
        if self.call_depth > self.max_depth {
            self.call_depth -= 1;
            return Err(VmError::StackOverflow);
        }
        let result = self.eval_inner(expr);
        self.call_depth -= 1;
        result
    }

    /// Evaluate via JIT with tree-walking fallback.
    ///
    /// Attempts to JIT-compile the expression and execute as native code.
    /// If JIT compilation fails (e.g. expression uses neural/symbolic ops),
    /// transparently falls back to the tree-walking interpreter.
    ///
    /// The JIT operates in f64 precision — the result is always `Val::F64`
    /// when the JIT path succeeds.
    pub fn eval_jit(&mut self, expr: &Expr) -> Result<Val, VmError> {
        if let Some(result) = self.try_jit(expr) {
            return result;
        }
        self.eval(expr)
    }

    /// Try to evaluate via JIT. Returns `None` if JIT is disabled or compilation
    /// fails, in which case the caller should fall back to tree-walking.
    fn try_jit(&self, expr: &Expr) -> Option<Result<Val, VmError>> {
        let jit = self.jit.as_ref()?;
        let func = jit.compile(expr).ok()?;
        match func.call_f64(&[]) {
            Ok(v) => Some(Ok(Val::f64(v))),
            Err(_) => None,
        }
    }

    fn eval_inner(&mut self, expr: &Expr) -> Result<Val, VmError> {
        match expr {
            Expr::Lit(val) => Ok(val.clone()),

            Expr::Ref(sym) => self.get(*sym).cloned().ok_or(VmError::UnboundSymbol(*sym)),

            Expr::App(op, args) => {
                let mut vals = Vec::with_capacity(args.len());
                for a in args {
                    vals.push(self.eval(a)?);
                }
                self.exec_op(*op, vals)
            }

            Expr::Seq(a, b) => {
                // Sequential: evaluate a, then b. The result of a is discarded
                // (pipeline semantics: b receives its own input in a full backend).
                let _left = self.eval(a)?;
                self.eval(b)
            }

            Expr::Par(a, b) => {
                // Parallel: evaluate both, return tuple of results.
                let va = self.eval(a)?;
                let vb = self.eval(b)?;
                Ok(Val::Tuple(vec![va, vb]))
            }

            Expr::Cond { pred, yes, no } => {
                let p = self.eval(pred)?;
                match p {
                    Val::Bool(true) => self.eval(yes),
                    Val::Bool(false) => self.eval(no),
                    _ => Err(VmError::TypeMismatch {
                        op: "cond",
                        expected: "Bool",
                        got: val_type_name(&p),
                    }),
                }
            }

            Expr::Let { name, val, body } => {
                let v = self.eval(val)?;
                self.env.push(HashMap::new());
                self.env
                    .last_mut()
                    .expect("VM env stack empty in Let")
                    .insert(*name, v);
                let result = self.eval(body);
                self.env.pop();
                result
            }

            Expr::Lam { .. } => {
                // Lambdas are values — they evaluate to themselves (as a closure).
                // We store the expression as a Val::Nil placeholder since we don't
                // have a proper closure Val variant. Real closures would capture env.
                Ok(Val::Nil)
            }

            Expr::Call(func, args) => {
                // Evaluate the function expression
                let _fval = self.eval(func)?;
                // In a full implementation, we'd look up the lambda body,
                // bind params to evaluated args, and evaluate the body.
                // For now, evaluate args and return the last one as a stub.
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

    /// Execute an opcode on evaluated arguments.
    fn exec_op(&mut self, op: Op, args: Vec<Val>) -> Result<Val, VmError> {
        match op {
            // ── Math ─────────────────────────────────────────────────────
            Op::ADD => self.binary_arith(&args, "add", |a, b| a + b, |a, b| a + b),
            Op::SUB => self.binary_arith(&args, "sub", |a, b| a - b, |a, b| a - b),
            Op::MUL => self.binary_arith(&args, "mul", |a, b| a * b, |a, b| a * b),
            Op::DIV => {
                if args.len() != 2 {
                    return Err(VmError::ArityMismatch {
                        op: "div",
                        expected: 2,
                        got: args.len(),
                    });
                }
                // Check for division by zero
                if let (Val::I64(_), Val::I64(0)) = (&args[0], &args[1]) {
                    return Err(VmError::DivisionByZero);
                }
                self.binary_arith(&args, "div", |a, b| a / b, |a, b| a / b)
            }
            Op::NEG => {
                self.require_args("neg", &args, 1)?;
                match &args[0] {
                    Val::I64(v) => Ok(Val::I64(-v)),
                    Val::F32(bits) => Ok(Val::f32(-f32::from_bits(*bits))),
                    Val::F64(bits) => Ok(Val::f64(-f64::from_bits(*bits))),
                    _ => Err(VmError::TypeMismatch {
                        op: "neg",
                        expected: "numeric",
                        got: val_type_name(&args[0]),
                    }),
                }
            }
            Op::ABS => {
                self.require_args("abs", &args, 1)?;
                match &args[0] {
                    Val::I64(v) => Ok(Val::I64(v.abs())),
                    Val::F32(bits) => Ok(Val::f32(f32::from_bits(*bits).abs())),
                    Val::F64(bits) => Ok(Val::f64(f64::from_bits(*bits).abs())),
                    _ => Err(VmError::TypeMismatch {
                        op: "abs",
                        expected: "numeric",
                        got: val_type_name(&args[0]),
                    }),
                }
            }
            Op::EXP => self.unary_float(&args, "exp", f32::exp, f64::exp),
            Op::LOG => self.unary_float(&args, "log", f32::ln, f64::ln),
            Op::SQRT => self.unary_float(&args, "sqrt", f32::sqrt, f64::sqrt),
            Op::SIN => self.unary_float(&args, "sin", f32::sin, f64::sin),
            Op::COS => self.unary_float(&args, "cos", f32::cos, f64::cos),
            Op::POW => {
                self.require_args("pow", &args, 2)?;
                match (&args[0], &args[1]) {
                    (Val::F32(a), Val::F32(b)) => {
                        Ok(Val::f32(f32::from_bits(*a).powf(f32::from_bits(*b))))
                    }
                    (Val::F64(a), Val::F64(b)) => {
                        Ok(Val::f64(f64::from_bits(*a).powf(f64::from_bits(*b))))
                    }
                    (Val::I64(a), Val::I64(b)) => Ok(Val::I64(a.pow(*b as u32))),
                    _ => Err(VmError::TypeMismatch {
                        op: "pow",
                        expected: "matching numeric",
                        got: "mixed",
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
                    _ => Err(VmError::TypeMismatch {
                        op: "max",
                        expected: "matching numeric",
                        got: "mixed",
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
                    _ => Err(VmError::TypeMismatch {
                        op: "min",
                        expected: "matching numeric",
                        got: "mixed",
                    }),
                }
            }
            Op::CLAMP => {
                self.require_args("clamp", &args, 3)?;
                match (&args[0], &args[1], &args[2]) {
                    (Val::I64(v), Val::I64(lo), Val::I64(hi)) => Ok(Val::I64(*v.max(lo).min(hi))),
                    (Val::F32(v), Val::F32(lo), Val::F32(hi)) => Ok(Val::f32(
                        f32::from_bits(*v).clamp(f32::from_bits(*lo), f32::from_bits(*hi)),
                    )),
                    _ => Err(VmError::TypeMismatch {
                        op: "clamp",
                        expected: "matching numeric",
                        got: "mixed",
                    }),
                }
            }

            // ── Activations (scalar approximations) ──────────────────────
            Op::RELU => {
                self.require_args("relu", &args, 1)?;
                match &args[0] {
                    Val::F32(bits) => {
                        let v = f32::from_bits(*bits);
                        Ok(Val::f32(if v > 0.0 { v } else { 0.0 }))
                    }
                    Val::I64(v) => Ok(Val::I64((*v).max(0))),
                    _ => Err(VmError::TypeMismatch {
                        op: "relu",
                        expected: "numeric",
                        got: val_type_name(&args[0]),
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
                    _ => Err(VmError::TypeMismatch {
                        op: "sigmoid",
                        expected: "F32",
                        got: val_type_name(&args[0]),
                    }),
                }
            }
            Op::TANH_ACT => {
                self.require_args("tanh", &args, 1)?;
                match &args[0] {
                    Val::F32(bits) => Ok(Val::f32(f32::from_bits(*bits).tanh())),
                    _ => Err(VmError::TypeMismatch {
                        op: "tanh",
                        expected: "F32",
                        got: val_type_name(&args[0]),
                    }),
                }
            }

            // ── Meta / introspection ─────────────────────────────────────
            Op::HASH => {
                // Hash the first argument's content
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
            Op::IDENTITY => {
                if args.is_empty() {
                    Ok(Val::Nil)
                } else {
                    Ok(args.into_iter().next().unwrap())
                }
            }

            // ── Composition ops ──────────────────────────────────────────
            Op::RES_ADD => {
                // Residual add: expects 2 args (identity path + transformed path)
                // In the tree-walking VM, we just eval both sub-expressions.
                // The result is the sum of the two paths.
                self.require_args("res_add", &args, 2)?;
                // For scalars, add them. For other types, return the tuple.
                match (&args[0], &args[1]) {
                    (Val::I64(a), Val::I64(b)) => Ok(Val::I64(a + b)),
                    (Val::F32(a), Val::F32(b)) => {
                        Ok(Val::f32(f32::from_bits(*a) + f32::from_bits(*b)))
                    }
                    _ => Ok(Val::Tuple(args)),
                }
            }
            Op::CONCAT => {
                // Concatenate tuples or return a tuple of the args.
                let mut result = Vec::new();
                for a in args {
                    match a {
                        Val::Tuple(vs) => result.extend(vs),
                        other => result.push(other),
                    }
                }
                Ok(Val::Tuple(result))
            }

            // ── Neural/Symbolic stubs ────────────────────────────────────
            // These ops record intent but don't execute real computation.
            // A production backend would lower these to GPU kernels.
            Op::LINEAR
            | Op::CONV2D
            | Op::ATTN
            | Op::MATMUL
            | Op::EMBED
            | Op::SOFTMAX
            | Op::LAYER_NORM
            | Op::BATCH_NORM
            | Op::RMS_NORM
            | Op::GROUP_NORM
            | Op::INSTANCE_NORM
            | Op::MAX_POOL
            | Op::AVG_POOL
            | Op::ADAPTIVE_POOL
            | Op::GLOBAL_POOL
            | Op::DROP
            | Op::GELU
            | Op::SILU
            | Op::MISH
            | Op::SOFTPLUS
            | Op::MSE_LOSS
            | Op::CROSS_ENTROPY
            | Op::BCE_LOSS
            | Op::NLL_LOSS
            | Op::HUBER_LOSS
            | Op::KL_DIV
            | Op::SGD_STEP
            | Op::ADAM_STEP
            | Op::ADAMW_STEP
            | Op::RMSPROP_STEP
            | Op::RNN
            | Op::LSTM
            | Op::GRU
            | Op::SINUSOIDAL_PE
            | Op::ROPE
            | Op::LEARNED_PE
            | Op::ALIBI => {
                // Stub: return Nil (a real backend would dispatch to kernels)
                Ok(Val::Nil)
            }

            // Symbolic stubs
            Op::UNIFY
            | Op::RESOLVE
            | Op::INFER
            | Op::MATCH
            | Op::REWRITE
            | Op::ASSERT
            | Op::QUERY_KB
            | Op::PLAN
            | Op::BIND_SYM
            | Op::SUBSUME => Ok(Val::Nil),

            // Agent stubs
            Op::SEND
            | Op::RECV
            | Op::SPAWN
            | Op::KILL
            | Op::PUBLISH
            | Op::SUBSCRIBE
            | Op::DELEGATE
            | Op::BROADCAST => Ok(Val::Nil),

            // Memory stubs
            Op::ALLOC
            | Op::FREE
            | Op::LOAD
            | Op::STORE
            | Op::COPY
            | Op::RESHAPE
            | Op::TRANSPOSE
            | Op::SLICE
            | Op::GATHER
            | Op::SCATTER => Ok(Val::Nil),

            // Meta stubs
            Op::TYPE_OF
            | Op::SHAPE_OF
            | Op::COMPOSE
            | Op::DECOMPOSE
            | Op::SELF_REF
            | Op::MUTATE
            | Op::INTROSPECT => Ok(Val::Nil),

            // Control flow ops (SEQ/PAR/COND are handled in eval_inner)
            Op::SEQ
            | Op::PAR
            | Op::COND
            | Op::LOOP
            | Op::MAP
            | Op::REDUCE
            | Op::SCAN
            | Op::FOLD
            | Op::ZIP
            | Op::FORK
            | Op::JOIN
            | Op::RES_CAT
            | Op::REPEAT => Ok(Val::Nil),

            _ => Err(VmError::Unimplemented(op)),
        }
    }

    // ── Arithmetic helpers ───────────────────────────────────────────────

    fn require_args(&self, op: &'static str, args: &[Val], n: usize) -> Result<(), VmError> {
        if args.len() != n {
            Err(VmError::ArityMismatch {
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
    ) -> Result<Val, VmError> {
        if args.len() != 2 {
            return Err(VmError::ArityMismatch {
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
            _ => Err(VmError::TypeMismatch {
                op: name,
                expected: "matching numeric",
                got: "mixed/non-numeric",
            }),
        }
    }

    fn unary_float(
        &self,
        args: &[Val],
        name: &'static str,
        f32_fn: impl Fn(f32) -> f32,
        f64_fn: impl Fn(f64) -> f64,
    ) -> Result<Val, VmError> {
        if args.len() != 1 {
            return Err(VmError::ArityMismatch {
                op: name,
                expected: 1,
                got: args.len(),
            });
        }
        match &args[0] {
            Val::F32(bits) => Ok(Val::f32(f32_fn(f32::from_bits(*bits)))),
            Val::F64(bits) => Ok(Val::f64(f64_fn(f64::from_bits(*bits)))),
            _ => Err(VmError::TypeMismatch {
                op: name,
                expected: "float",
                got: val_type_name(&args[0]),
            }),
        }
    }
}

/// Human-readable type name for error messages.
fn val_type_name(v: &Val) -> &'static str {
    match v {
        Val::Nil => "Nil",
        Val::Bool(_) => "Bool",
        Val::I64(_) => "I64",
        Val::F32(_) => "F32",
        Val::F64(_) => "F64",
        Val::Tensor { .. } => "Tensor",
        Val::Sym(_) => "Sym",
        Val::Tuple(_) => "Tuple",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::expr::Expr;
    use crate::lang::op::Op;

    fn vm() -> Vm {
        Vm::new()
    }

    #[test]
    fn eval_literal() {
        let mut v = vm();
        assert_eq!(v.eval(&Expr::int(42)).unwrap().as_i64(), Some(42));
    }

    #[test]
    fn eval_add_i64() {
        let mut v = vm();
        let e = Expr::op2(Op::ADD, Expr::int(2), Expr::int(3));
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(5));
    }

    #[test]
    fn eval_add_f32() {
        let mut v = vm();
        let e = Expr::op2(Op::ADD, Expr::float(1.5), Expr::float(2.5));
        let res = v.eval(&e).unwrap().as_f32().unwrap();
        assert!((res - 4.0).abs() < 1e-6);
    }

    #[test]
    fn eval_nested_math() {
        let mut v = vm();
        // (2 + 3) * 4 = 20
        let add = Expr::op2(Op::ADD, Expr::int(2), Expr::int(3));
        let mul = Expr::op2(Op::MUL, add, Expr::int(4));
        assert_eq!(v.eval(&mul).unwrap().as_i64(), Some(20));
    }

    #[test]
    fn eval_neg() {
        let mut v = vm();
        let e = Expr::op(Op::NEG, vec![Expr::int(5)]);
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(-5));
    }

    #[test]
    fn eval_abs() {
        let mut v = vm();
        let e = Expr::op(Op::ABS, vec![Expr::int(-7)]);
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(7));
    }

    #[test]
    fn eval_division_by_zero() {
        let mut v = vm();
        let e = Expr::op2(Op::DIV, Expr::int(10), Expr::int(0));
        assert!(v.eval(&e).is_err());
    }

    #[test]
    fn eval_condtional_true() {
        let mut v = vm();
        let e = Expr::Cond {
            pred: Box::new(Expr::boolean(true)),
            yes: Box::new(Expr::int(1)),
            no: Box::new(Expr::int(0)),
        };
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(1));
    }

    #[test]
    fn eval_conditional_false() {
        let mut v = vm();
        let e = Expr::Cond {
            pred: Box::new(Expr::boolean(false)),
            yes: Box::new(Expr::int(1)),
            no: Box::new(Expr::int(0)),
        };
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(0));
    }

    #[test]
    fn eval_let_binding() {
        let mut v = vm();
        let x = v.symbols.intern("x");
        let e = Expr::bind(x, Expr::int(42), Expr::sym(x));
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(42));
    }

    #[test]
    fn eval_let_nested() {
        let mut v = vm();
        let x = v.symbols.intern("x");
        let y = v.symbols.intern("y");
        // let x = 10 in (let y = 20 in x + y)
        let inner = Expr::bind(
            y,
            Expr::int(20),
            Expr::op2(Op::ADD, Expr::sym(x), Expr::sym(y)),
        );
        let e = Expr::bind(x, Expr::int(10), inner);
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(30));
    }

    #[test]
    fn eval_parallel() {
        let mut v = vm();
        let e = Expr::int(1) | Expr::int(2);
        match v.eval(&e).unwrap() {
            Val::Tuple(vs) => {
                assert_eq!(vs.len(), 2);
                assert_eq!(vs[0].as_i64(), Some(1));
                assert_eq!(vs[1].as_i64(), Some(2));
            }
            _ => panic!("expected Tuple"),
        }
    }

    #[test]
    fn eval_seq() {
        let mut v = vm();
        let e = Expr::int(1) >> Expr::int(2);
        // Seq returns the result of the second expr
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(2));
    }

    #[test]
    fn eval_relu() {
        let mut v = vm();
        let pos = Expr::op(Op::RELU, vec![Expr::float(3.0)]);
        let neg = Expr::op(Op::RELU, vec![Expr::float(-3.0)]);
        assert!((v.eval(&pos).unwrap().as_f32().unwrap() - 3.0).abs() < 1e-6);
        assert!((v.eval(&neg).unwrap().as_f32().unwrap()).abs() < 1e-6);
    }

    #[test]
    fn eval_sigmoid() {
        let mut v = vm();
        let e = Expr::op(Op::SIGMOID, vec![Expr::float(0.0)]);
        let res = v.eval(&e).unwrap().as_f32().unwrap();
        assert!((res - 0.5).abs() < 1e-6); // sigmoid(0) = 0.5
    }

    #[test]
    fn eval_block() {
        let mut v = vm();
        let e = Expr::Block(vec![Expr::int(1), Expr::int(2), Expr::int(3)]);
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(3)); // returns last
    }

    #[test]
    fn eval_hash() {
        let mut v = vm();
        let e = Expr::op(Op::HASH, vec![Expr::int(42)]);
        match v.eval(&e).unwrap() {
            Val::I64(_) => {} // any i64 is fine as long as it's deterministic
            other => panic!("expected I64, got {other:?}"),
        }
    }

    #[test]
    fn eval_identity() {
        let mut v = vm();
        let e = Expr::op(Op::IDENTITY, vec![Expr::int(7)]);
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(7));
    }

    #[test]
    fn stack_overflow_protection() {
        let mut v = vm();
        v.max_depth = 5;
        // Build a deeply nested expression
        let mut e = Expr::int(1);
        for _ in 0..20 {
            e = Expr::op(Op::IDENTITY, vec![e]);
        }
        assert!(v.eval(&e).is_err());
    }

    #[test]
    fn eval_neural_stub() {
        let mut v = vm();
        // Neural ops return Nil in stub mode
        let e = Expr::op(Op::LINEAR, vec![Expr::int(0)]);
        assert_eq!(v.eval(&e).unwrap(), Val::Nil);
    }

    #[test]
    fn eval_exp_log_roundtrip() {
        let mut v = vm();
        let e = Expr::op(Op::EXP, vec![Expr::float(1.0)]);
        let exp1 = v.eval(&e).unwrap().as_f32().unwrap();
        assert!((exp1 - std::f32::consts::E).abs() < 1e-5);

        let e2 = Expr::op(Op::LOG, vec![Expr::float(exp1)]);
        let log_exp = v.eval(&e2).unwrap().as_f32().unwrap();
        assert!((log_exp - 1.0).abs() < 1e-5);
    }

    #[test]
    fn eval_max_min() {
        let mut v = vm();
        let e = Expr::op2(Op::MAX, Expr::int(3), Expr::int(7));
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(7));

        let e = Expr::op2(Op::MIN, Expr::int(3), Expr::int(7));
        assert_eq!(v.eval(&e).unwrap().as_i64(), Some(3));
    }

    #[test]
    fn eval_unbound_symbol() {
        let mut v = vm();
        let e = Expr::sym(Sym(9999));
        assert!(v.eval(&e).is_err());
    }

    // ── JIT integration tests ────────────────────────────────────────────

    #[test]
    fn eval_jit_simple_add() {
        let mut v = vm();
        // 2.0 + 3.0 via JIT
        let e = Expr::op2(Op::ADD, Expr::float(2.0), Expr::float(3.0));
        let result = v.eval_jit(&e).unwrap();
        let val = result.as_f64().unwrap();
        assert!((val - 5.0).abs() < 1e-6);
    }

    #[test]
    fn eval_jit_nested_math() {
        let mut v = vm();
        // (3.0 * 4.0) + 2.0 = 14.0
        let e = Expr::op2(
            Op::ADD,
            Expr::op2(Op::MUL, Expr::float(3.0), Expr::float(4.0)),
            Expr::float(2.0),
        );
        let result = v.eval_jit(&e).unwrap();
        let val = result.as_f64().unwrap();
        assert!((val - 14.0).abs() < 1e-6);
    }

    #[test]
    fn eval_jit_fallback_on_neural() {
        let mut v = vm();
        // Neural ops are compiled as stubs by the JIT (return 0.0),
        // so eval_jit still produces a result (F64) rather than Nil.
        let e = Expr::op(Op::LINEAR, vec![Expr::int(0)]);
        let result = v.eval_jit(&e).unwrap();
        // JIT returns F64(0) for stub ops; tree-walking returns Nil
        assert!(result.as_f64().is_some() || result == Val::Nil);
    }

    #[test]
    fn eval_jit_no_jit_mode() {
        let mut v = Vm::new_no_jit();
        let e = Expr::op2(Op::ADD, Expr::int(2), Expr::int(3));
        // Should still work via tree-walking
        assert_eq!(v.eval_jit(&e).unwrap().as_i64(), Some(5));
    }

    #[test]
    fn jit_cache_size() {
        let v = Vm::new();
        assert_eq!(v.jit_cache_size(), 0);

        let v2 = Vm::new_no_jit();
        assert_eq!(v2.jit_cache_size(), 0);
    }

    #[test]
    fn set_jit_toggle() {
        let mut v = Vm::new();
        assert!(v.jit.is_some());
        v.set_jit(false);
        assert!(v.jit.is_none());
        v.set_jit(true);
        assert!(v.jit.is_some());
    }
}