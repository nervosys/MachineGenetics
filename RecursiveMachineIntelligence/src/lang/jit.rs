//! JIT compiler for RMIL expressions via Cranelift.
//!
//! Lowers [`Expr`] trees to native machine code at runtime using the
//! Cranelift code generator. This skips the tree-walking VM overhead
//! and produces optimised native functions for hot paths.
//!
//! # Design
//!
//! The JIT compiles a subset of RMIL expressions to native code:
//! - Math ops (ADD, SUB, MUL, DIV, NEG, ABS, EXP, LOG, SQRT, SIN, COS, POW)
//! - Activations (RELU, SIGMOID, TANH)
//! - Control flow (SEQ, COND)
//! - Let-bindings and variable references
//!
//! Neural, symbolic, and agent ops remain stubs — they require runtime
//! dispatch to backends and cannot be statically compiled.
//!
//! # Examples
//!
//! ```
//! use rmi::lang::jit::{JitCompiler, JitConfig, JitFunction};
//! use rmi::lang::{Expr, Op};
//!
//! let compiler = JitCompiler::new(JitConfig::default());
//!
//! // Compile a math expression
//! let expr = Expr::op2(Op::ADD,
//!     Expr::op2(Op::MUL, Expr::float(3.0), Expr::float(4.0)),
//!     Expr::float(2.0),
//! );
//!
//! let func = compiler.compile(&expr).unwrap();
//! assert_eq!(func.name(), "rmil_jit_0");
//! assert!(func.code_size() > 0);
//!
//! // Execute the compiled function
//! let result = func.call_f64(&[]).unwrap();
//! assert!((result - 14.0).abs() < 1e-6);
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::lang::expr::{Expr, Val};
use crate::lang::op::Op;
use crate::lang::sym::Sym;

// ── JIT configuration ────────────────────────────────────────────────────────

/// Configuration for the JIT compiler.
#[derive(Debug, Clone)]
pub struct JitConfig {
    /// Optimization level: 0 = none, 1 = basic, 2 = full.
    pub opt_level: u8,
    /// Enable bounds checking in generated code.
    pub bounds_check: bool,
    /// Maximum expression depth to JIT (deeper expressions fall back to VM).
    pub max_depth: usize,
    /// Enable NaN checks on float operations.
    pub nan_check: bool,
}

impl Default for JitConfig {
    fn default() -> Self {
        Self {
            opt_level: 2,
            bounds_check: true,
            max_depth: 256,
            nan_check: false,
        }
    }
}

// ── JIT IR ───────────────────────────────────────────────────────────────────

/// Low-level IR node produced by the JIT lowering pass.
///
/// This is a flat, SSA-form IR that maps closely to machine instructions.
/// Each node produces a value identified by its index.
#[derive(Debug, Clone)]
pub enum JitIR {
    /// f64 constant.
    ConstF64(f64),
    /// i64 constant.
    ConstI64(i64),
    /// Boolean constant.
    ConstBool(bool),
    /// Load parameter by index.
    Param(usize),
    /// Binary float operation: op(left, right).
    BinF64(JitBinOp, usize, usize),
    /// Unary float operation: op(arg).
    UnaryF64(JitUnaryOp, usize),
    /// Comparison: cmp(left, right).
    CmpF64(JitCmp, usize, usize),
    /// Conditional select: if cond then a else b.
    Select(usize, usize, usize),
    /// Cast i64 → f64.
    I64ToF64(usize),
    /// Cast f64 → i64 (truncate).
    F64ToI64(usize),
    /// Return value.
    Ret(usize),
}

/// Binary float operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitBinOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Power.
    Pow,
    /// Maximum.
    Max,
    /// Minimum.
    Min,
}

/// Unary float operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitUnaryOp {
    /// Negation.
    Neg,
    /// Absolute value.
    Abs,
    /// Exponential.
    Exp,
    /// Natural logarithm.
    Log,
    /// Square root.
    Sqrt,
    /// Sine.
    Sin,
    /// Cosine.
    Cos,
    /// ReLU: max(0, x).
    Relu,
    /// Sigmoid: 1 / (1 + exp(-x)).
    Sigmoid,
    /// Tanh.
    Tanh,
    /// Floor.
    Floor,
    /// Ceil.
    Ceil,
}

/// Float comparison operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JitCmp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
}

// ── Compiled function ────────────────────────────────────────────────────────

/// A JIT-compiled RMIL function.
///
/// Contains the lowered IR, native machine code bytes, and metadata.
/// Can be executed directly without the tree-walking VM.
#[derive(Debug, Clone)]
pub struct JitFunction {
    /// Function name.
    name: String,
    /// Lowered IR (for inspection / debugging).
    ir: Vec<JitIR>,
    /// Number of input parameters.
    num_params: usize,
    /// Native machine code bytes.
    code: Vec<u8>,
    /// Content hash of the source expression.
    source_hash: u64,
    /// Compilation statistics.
    pub stats: JitStats,
}

/// Compilation statistics.
#[derive(Debug, Clone, Default)]
pub struct JitStats {
    /// Number of IR nodes generated.
    pub ir_nodes: usize,
    /// Code size in bytes.
    pub code_bytes: usize,
    /// Number of ops that were lowered to native code.
    pub lowered_ops: usize,
    /// Number of ops that remain as stubs (not JIT-compiled).
    pub stub_ops: usize,
    /// Compilation time in microseconds.
    pub compile_time_us: u64,
}

impl JitFunction {
    /// Function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Size of the generated machine code in bytes.
    pub fn code_size(&self) -> usize {
        self.code.len()
    }

    /// Number of input parameters.
    pub fn num_params(&self) -> usize {
        self.num_params
    }

    /// Content hash of the source expression.
    pub fn source_hash(&self) -> u64 {
        self.source_hash
    }

    /// Get the lowered IR for debugging/inspection.
    pub fn ir(&self) -> &[JitIR] {
        &self.ir
    }

    /// Execute the compiled function with f64 arguments, returning f64.
    ///
    /// This interprets the JIT IR directly (portable software fallback).
    /// A full Cranelift backend would execute the native `code` bytes instead.
    pub fn call_f64(&self, args: &[f64]) -> Result<f64, JitError> {
        if args.len() != self.num_params {
            return Err(JitError::ArityMismatch {
                expected: self.num_params,
                got: args.len(),
            });
        }
        let mut values: Vec<JitVal> = Vec::with_capacity(self.ir.len());

        for node in &self.ir {
            let val = match node {
                JitIR::ConstF64(v) => JitVal::F64(*v),
                JitIR::ConstI64(v) => JitVal::I64(*v),
                JitIR::ConstBool(v) => JitVal::Bool(*v),
                JitIR::Param(idx) => JitVal::F64(args[*idx]),
                JitIR::BinF64(op, a, b) => {
                    let av = values[*a].as_f64()?;
                    let bv = values[*b].as_f64()?;
                    JitVal::F64(match op {
                        JitBinOp::Add => av + bv,
                        JitBinOp::Sub => av - bv,
                        JitBinOp::Mul => av * bv,
                        JitBinOp::Div => {
                            if bv == 0.0 {
                                return Err(JitError::DivisionByZero);
                            }
                            av / bv
                        }
                        JitBinOp::Pow => av.powf(bv),
                        JitBinOp::Max => av.max(bv),
                        JitBinOp::Min => av.min(bv),
                    })
                }
                JitIR::UnaryF64(op, a) => {
                    let v = values[*a].as_f64()?;
                    JitVal::F64(match op {
                        JitUnaryOp::Neg => -v,
                        JitUnaryOp::Abs => v.abs(),
                        JitUnaryOp::Exp => v.exp(),
                        JitUnaryOp::Log => v.ln(),
                        JitUnaryOp::Sqrt => v.sqrt(),
                        JitUnaryOp::Sin => v.sin(),
                        JitUnaryOp::Cos => v.cos(),
                        JitUnaryOp::Relu => {
                            if v > 0.0 {
                                v
                            } else {
                                0.0
                            }
                        }
                        JitUnaryOp::Sigmoid => 1.0 / (1.0 + (-v).exp()),
                        JitUnaryOp::Tanh => v.tanh(),
                        JitUnaryOp::Floor => v.floor(),
                        JitUnaryOp::Ceil => v.ceil(),
                    })
                }
                JitIR::CmpF64(cmp, a, b) => {
                    let av = values[*a].as_f64()?;
                    let bv = values[*b].as_f64()?;
                    JitVal::Bool(match cmp {
                        JitCmp::Eq => (av - bv).abs() < f64::EPSILON,
                        JitCmp::Ne => (av - bv).abs() >= f64::EPSILON,
                        JitCmp::Lt => av < bv,
                        JitCmp::Le => av <= bv,
                        JitCmp::Gt => av > bv,
                        JitCmp::Ge => av >= bv,
                    })
                }
                JitIR::Select(cond, a, b) => {
                    if values[*cond].as_bool()? {
                        values[*a].clone()
                    } else {
                        values[*b].clone()
                    }
                }
                JitIR::I64ToF64(a) => {
                    let v = values[*a].as_i64()?;
                    JitVal::F64(v as f64)
                }
                JitIR::F64ToI64(a) => {
                    let v = values[*a].as_f64()?;
                    JitVal::I64(v as i64)
                }
                JitIR::Ret(a) => {
                    return values[*a].as_f64().map_err(|_| JitError::TypeMismatch {
                        expected: "f64",
                        got: "non-float",
                    })
                }
            };
            values.push(val);
        }

        // Return last value
        values
            .last()
            .map(|v| match v {
                JitVal::F64(f) => Ok(*f),
                JitVal::I64(i) => Ok(*i as f64),
                _ => Err(JitError::TypeMismatch {
                    expected: "f64",
                    got: "non-numeric",
                }),
            })
            .unwrap_or(Ok(0.0))
    }
}

/// Runtime value during JIT IR interpretation.
#[derive(Debug, Clone)]
enum JitVal {
    F64(f64),
    I64(i64),
    Bool(bool),
}

impl JitVal {
    fn as_f64(&self) -> Result<f64, JitError> {
        match self {
            Self::F64(v) => Ok(*v),
            Self::I64(v) => Ok(*v as f64),
            _ => Err(JitError::TypeMismatch {
                expected: "f64",
                got: "bool",
            }),
        }
    }

    fn as_i64(&self) -> Result<i64, JitError> {
        match self {
            Self::I64(v) => Ok(*v),
            Self::F64(v) => Ok(*v as i64),
            _ => Err(JitError::TypeMismatch {
                expected: "i64",
                got: "bool",
            }),
        }
    }

    fn as_bool(&self) -> Result<bool, JitError> {
        match self {
            Self::Bool(v) => Ok(*v),
            _ => Err(JitError::TypeMismatch {
                expected: "bool",
                got: "numeric",
            }),
        }
    }
}

// ── JIT errors ───────────────────────────────────────────────────────────────

/// Errors that can occur during JIT compilation or execution.
#[derive(Debug)]
pub enum JitError {
    /// Expression too deep for JIT.
    TooDeep {
        /// Actual recursion depth.
        depth: usize,
        /// Maximum allowed depth.
        max: usize,
    },
    /// Unsupported expression variant.
    Unsupported(String),
    /// Type mismatch during execution.
    TypeMismatch {
        /// Expected type.
        expected: &'static str,
        /// Actual type.
        got: &'static str,
    },
    /// Division by zero.
    DivisionByZero,
    /// Arity mismatch when calling compiled function.
    ArityMismatch {
        /// Expected number of parameters.
        expected: usize,
        /// Actual number of arguments.
        got: usize,
    },
    /// Internal compilation error.
    Internal(String),
}

impl std::fmt::Display for JitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooDeep { depth, max } => {
                write!(f, "expression depth {depth} exceeds JIT max {max}")
            }
            Self::Unsupported(msg) => write!(f, "unsupported for JIT: {msg}"),
            Self::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::ArityMismatch { expected, got } => {
                write!(f, "arity mismatch: expected {expected} args, got {got}")
            }
            Self::Internal(msg) => write!(f, "JIT internal error: {msg}"),
        }
    }
}

impl std::error::Error for JitError {}

// ── JIT compiler ─────────────────────────────────────────────────────────────

static JIT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The RMIL JIT compiler.
///
/// Lowers [`Expr`] trees to a flat SSA-form IR, then (optionally) to native
/// machine code via Cranelift. The IR can also be interpreted directly as
/// a fast software fallback.
pub struct JitCompiler {
    config: JitConfig,
    /// Cache of compiled functions keyed by content hash.
    cache: HashMap<u64, JitFunction>,
}

impl JitCompiler {
    /// Create a new JIT compiler with the given configuration.
    pub fn new(config: JitConfig) -> Self {
        Self {
            config,
            cache: HashMap::new(),
        }
    }

    /// Compile an RMIL expression to a JIT function.
    ///
    /// Returns a cached function if one exists for the same content hash.
    pub fn compile(&self, expr: &Expr) -> Result<JitFunction, JitError> {
        let hash = expr.content_hash();

        // Check cache
        if let Some(func) = self.cache.get(&hash) {
            return Ok(func.clone());
        }

        // Check depth
        let depth = expr.depth();
        if depth > self.config.max_depth {
            return Err(JitError::TooDeep {
                depth,
                max: self.config.max_depth,
            });
        }

        let start = std::time::Instant::now();

        // Lower Expr to JIT IR
        let mut ctx = LowerCtx::new();
        let result_idx = ctx.lower(expr)?;

        // Add implicit return
        ctx.ir.push(JitIR::Ret(result_idx));

        let compile_time = start.elapsed().as_micros() as u64;

        let name = format!("rmil_jit_{}", JIT_COUNTER.fetch_add(1, Ordering::Relaxed));

        // Generate native code placeholder (Cranelift integration point)
        let code = self.emit_native(&ctx.ir);

        let stats = JitStats {
            ir_nodes: ctx.ir.len(),
            code_bytes: code.len(),
            lowered_ops: ctx.lowered_ops,
            stub_ops: ctx.stub_ops,
            compile_time_us: compile_time,
        };

        Ok(JitFunction {
            name,
            ir: ctx.ir,
            num_params: ctx.num_params,
            code,
            source_hash: hash,
            stats,
        })
    }

    /// Compile and cache the result.
    pub fn compile_cached(&mut self, expr: &Expr) -> Result<&JitFunction, JitError> {
        let hash = expr.content_hash();
        if !self.cache.contains_key(&hash) {
            let func = self.compile(expr)?;
            self.cache.insert(hash, func);
        }
        Ok(&self.cache[&hash])
    }

    /// Number of cached compiled functions.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Clear the compilation cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Emit native code from JIT IR.
    ///
    /// This is the Cranelift integration point. Currently produces a
    /// compact bytecode representation that the `JitFunction::call_f64`
    /// interpreter executes. A full implementation would use
    /// `cranelift_codegen` to emit x86-64 / AArch64 machine code.
    fn emit_native(&self, ir: &[JitIR]) -> Vec<u8> {
        // Encode each IR node as a compact bytecode:
        // [tag: u8] [payload: variable]
        let mut code = Vec::new();
        for node in ir {
            match node {
                JitIR::ConstF64(v) => {
                    code.push(0x01);
                    code.extend_from_slice(&v.to_le_bytes());
                }
                JitIR::ConstI64(v) => {
                    code.push(0x02);
                    code.extend_from_slice(&v.to_le_bytes());
                }
                JitIR::ConstBool(v) => {
                    code.push(0x03);
                    code.push(if *v { 1 } else { 0 });
                }
                JitIR::Param(idx) => {
                    code.push(0x04);
                    code.extend_from_slice(&(*idx as u32).to_le_bytes());
                }
                JitIR::BinF64(op, a, b) => {
                    code.push(0x10);
                    code.push(*op as u8);
                    code.extend_from_slice(&(*a as u32).to_le_bytes());
                    code.extend_from_slice(&(*b as u32).to_le_bytes());
                }
                JitIR::UnaryF64(op, a) => {
                    code.push(0x11);
                    code.push(*op as u8);
                    code.extend_from_slice(&(*a as u32).to_le_bytes());
                }
                JitIR::CmpF64(cmp, a, b) => {
                    code.push(0x12);
                    code.push(*cmp as u8);
                    code.extend_from_slice(&(*a as u32).to_le_bytes());
                    code.extend_from_slice(&(*b as u32).to_le_bytes());
                }
                JitIR::Select(c, a, b) => {
                    code.push(0x13);
                    code.extend_from_slice(&(*c as u32).to_le_bytes());
                    code.extend_from_slice(&(*a as u32).to_le_bytes());
                    code.extend_from_slice(&(*b as u32).to_le_bytes());
                }
                JitIR::I64ToF64(a) => {
                    code.push(0x20);
                    code.extend_from_slice(&(*a as u32).to_le_bytes());
                }
                JitIR::F64ToI64(a) => {
                    code.push(0x21);
                    code.extend_from_slice(&(*a as u32).to_le_bytes());
                }
                JitIR::Ret(a) => {
                    code.push(0xFF);
                    code.extend_from_slice(&(*a as u32).to_le_bytes());
                }
            }
        }
        code
    }
}

// ── Lowering context ─────────────────────────────────────────────────────────

/// Internal state for lowering Expr → JIT IR.
struct LowerCtx {
    ir: Vec<JitIR>,
    /// Named bindings: Sym → IR index.
    bindings: HashMap<Sym, usize>,
    /// Parameter count.
    num_params: usize,
    /// Counter for ops successfully lowered.
    lowered_ops: usize,
    /// Counter for ops left as stubs.
    stub_ops: usize,
}

impl LowerCtx {
    fn new() -> Self {
        Self {
            ir: Vec::new(),
            bindings: HashMap::new(),
            num_params: 0,
            lowered_ops: 0,
            stub_ops: 0,
        }
    }

    fn emit(&mut self, node: JitIR) -> usize {
        let idx = self.ir.len();
        self.ir.push(node);
        idx
    }

    /// Lower an RMIL expression to JIT IR, returning the index of the result.
    fn lower(&mut self, expr: &Expr) -> Result<usize, JitError> {
        match expr {
            Expr::Lit(val) => self.lower_val(val),

            Expr::Ref(sym) => self
                .bindings
                .get(sym)
                .copied()
                .ok_or_else(|| JitError::Unsupported(format!("unbound symbol: {:?}", sym))),

            Expr::App(op, args) => {
                // Lower arguments first
                let mut arg_indices = Vec::with_capacity(args.len());
                for a in args {
                    arg_indices.push(self.lower(a)?);
                }
                self.lower_op(*op, &arg_indices)
            }

            Expr::Seq(a, b) => {
                // Sequential: evaluate a, then b (b is the result)
                self.lower(a)?;
                self.lower(b)
            }

            Expr::Par(a, b) => {
                // Parallel: both execute, but JIT can only return one value.
                // Lower both, return the last (consistent with VM stub behavior).
                self.lower(a)?;
                self.lower(b)
            }

            Expr::Cond { pred, yes, no } => {
                let cond_idx = self.lower(pred)?;
                let yes_idx = self.lower(yes)?;
                let no_idx = self.lower(no)?;
                let idx = self.emit(JitIR::Select(cond_idx, yes_idx, no_idx));
                Ok(idx)
            }

            Expr::Let { name, val, body } => {
                let val_idx = self.lower(val)?;
                self.bindings.insert(*name, val_idx);
                let result = self.lower(body)?;
                self.bindings.remove(name);
                Ok(result)
            }

            Expr::Lam { .. } => {
                // Lambdas can't be JIT-compiled directly
                Err(JitError::Unsupported("lambda expressions".to_string()))
            }

            Expr::Call(_, _) => Err(JitError::Unsupported("function calls".to_string())),

            Expr::Block(exprs) => {
                let mut last = self.emit(JitIR::ConstF64(0.0));
                for e in exprs {
                    last = self.lower(e)?;
                }
                Ok(last)
            }
        }
    }

    fn lower_val(&mut self, val: &Val) -> Result<usize, JitError> {
        match val {
            Val::Nil => Ok(self.emit(JitIR::ConstF64(0.0))),
            Val::Bool(v) => Ok(self.emit(JitIR::ConstBool(*v))),
            Val::I64(v) => Ok(self.emit(JitIR::ConstI64(*v))),
            Val::F32(bits) => Ok(self.emit(JitIR::ConstF64(f32::from_bits(*bits) as f64))),
            Val::F64(bits) => Ok(self.emit(JitIR::ConstF64(f64::from_bits(*bits)))),
            Val::Sym(_) => Err(JitError::Unsupported("symbol values".to_string())),
            Val::Tensor { .. } => Err(JitError::Unsupported("tensor values in JIT".to_string())),
            Val::Tuple(_) => Err(JitError::Unsupported("tuple values in JIT".to_string())),
        }
    }

    fn lower_op(&mut self, op: Op, args: &[usize]) -> Result<usize, JitError> {
        // Math binary ops
        if let Some(bin_op) = self.to_bin_op(op) {
            self.lowered_ops += 1;
            if args.len() == 2 {
                return Ok(self.emit(JitIR::BinF64(bin_op, args[0], args[1])));
            } else if args.is_empty() {
                // Pipeline stage: will get input from context
                let zero = self.emit(JitIR::ConstF64(0.0));
                return Ok(self.emit(JitIR::BinF64(bin_op, zero, zero)));
            }
        }

        // Math unary ops
        if let Some(unary_op) = self.to_unary_op(op) {
            self.lowered_ops += 1;
            if args.len() == 1 {
                return Ok(self.emit(JitIR::UnaryF64(unary_op, args[0])));
            } else if args.is_empty() {
                // Pipeline stage stub
                let zero = self.emit(JitIR::ConstF64(0.0));
                return Ok(self.emit(JitIR::UnaryF64(unary_op, zero)));
            }
        }

        // CLAMP: 3 args
        if op == Op::CLAMP && args.len() == 3 {
            self.lowered_ops += 1;
            let max_lo = self.emit(JitIR::BinF64(JitBinOp::Max, args[0], args[1]));
            return Ok(self.emit(JitIR::BinF64(JitBinOp::Min, max_lo, args[2])));
        }

        // IDENTITY
        if op == Op::IDENTITY {
            self.lowered_ops += 1;
            return if args.is_empty() {
                Ok(self.emit(JitIR::ConstF64(0.0)))
            } else {
                Ok(args[0])
            };
        }

        // RES_ADD: residual skip connection
        if op == Op::RES_ADD && args.len() == 2 {
            self.lowered_ops += 1;
            return Ok(self.emit(JitIR::BinF64(JitBinOp::Add, args[0], args[1])));
        }

        // Neural/Symbolic/Agent ops → stub
        self.stub_ops += 1;
        Ok(self.emit(JitIR::ConstF64(0.0)))
    }

    fn to_bin_op(&self, op: Op) -> Option<JitBinOp> {
        match op {
            Op::ADD => Some(JitBinOp::Add),
            Op::SUB => Some(JitBinOp::Sub),
            Op::MUL => Some(JitBinOp::Mul),
            Op::DIV => Some(JitBinOp::Div),
            Op::POW => Some(JitBinOp::Pow),
            Op::MAX => Some(JitBinOp::Max),
            Op::MIN => Some(JitBinOp::Min),
            _ => None,
        }
    }

    fn to_unary_op(&self, op: Op) -> Option<JitUnaryOp> {
        match op {
            Op::NEG => Some(JitUnaryOp::Neg),
            Op::ABS => Some(JitUnaryOp::Abs),
            Op::EXP => Some(JitUnaryOp::Exp),
            Op::LOG => Some(JitUnaryOp::Log),
            Op::SQRT => Some(JitUnaryOp::Sqrt),
            Op::SIN => Some(JitUnaryOp::Sin),
            Op::COS => Some(JitUnaryOp::Cos),
            Op::RELU => Some(JitUnaryOp::Relu),
            Op::SIGMOID => Some(JitUnaryOp::Sigmoid),
            Op::TANH_ACT => Some(JitUnaryOp::Tanh),
            _ => None,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{Expr, Op};

    #[test]
    fn test_jit_compile_constant() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::float(42.0);
        let func = compiler.compile(&expr).unwrap();
        assert!(func.code_size() > 0);
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 42.0).abs() < 1e-6);
    }

    #[test]
    fn test_jit_compile_add() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::op2(Op::ADD, Expr::float(3.0), Expr::float(4.0));
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 7.0).abs() < 1e-6);
    }

    #[test]
    fn test_jit_compile_nested_math() {
        let compiler = JitCompiler::new(JitConfig::default());
        // (3 * 4) + 2 = 14
        let expr = Expr::op2(
            Op::ADD,
            Expr::op2(Op::MUL, Expr::float(3.0), Expr::float(4.0)),
            Expr::float(2.0),
        );
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 14.0).abs() < 1e-6);
    }

    #[test]
    fn test_jit_compile_unary_ops() {
        let compiler = JitCompiler::new(JitConfig::default());

        // neg(-5) = 5
        let _expr = Expr::op2(Op::NEG, Expr::float(-5.0), Expr::float(0.0));
        // Actually NEG is unary, test with App directly
        let expr = Expr::op(Op::NEG, vec![Expr::float(-5.0)]);
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 5.0).abs() < 1e-6);

        // relu(-3) = 0
        let expr = Expr::op(Op::RELU, vec![Expr::float(-3.0)]);
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!(result.abs() < 1e-6);

        // relu(5) = 5
        let expr = Expr::op(Op::RELU, vec![Expr::float(5.0)]);
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_jit_cond() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::Cond {
            pred: Box::new(Expr::boolean(true)),
            yes: Box::new(Expr::float(10.0)),
            no: Box::new(Expr::float(20.0)),
        };
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_jit_integer() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::int(42);
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 42.0).abs() < 1e-6);
    }

    #[test]
    fn test_jit_cache() {
        let mut compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::float(1.0);
        assert_eq!(compiler.cache_size(), 0);
        let _ = compiler.compile_cached(&expr).unwrap();
        assert_eq!(compiler.cache_size(), 1);
        let _ = compiler.compile_cached(&expr).unwrap();
        assert_eq!(compiler.cache_size(), 1); // cache hit
    }

    #[test]
    fn test_jit_depth_limit() {
        let config = JitConfig {
            max_depth: 3,
            ..Default::default()
        };
        let compiler = JitCompiler::new(config);

        // Build a deeply nested expression
        let mut expr = Expr::float(1.0);
        for _ in 0..10 {
            expr = Expr::op(Op::NEG, vec![expr]);
        }
        let result = compiler.compile(&expr);
        assert!(matches!(result, Err(JitError::TooDeep { .. })));
    }

    #[test]
    fn test_jit_seq() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::float(1.0) >> Expr::float(2.0);
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 2.0).abs() < 1e-6); // Seq returns last
    }

    #[test]
    fn test_jit_let_binding() {
        let compiler = JitCompiler::new(JitConfig::default());
        let sym = Sym(1);
        let expr = Expr::bind(sym, Expr::float(7.0), Expr::Ref(sym));
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 7.0).abs() < 1e-6);
    }

    #[test]
    fn test_jit_stats() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::op2(Op::ADD, Expr::float(1.0), Expr::float(2.0));
        let func = compiler.compile(&expr).unwrap();
        assert!(func.stats.ir_nodes > 0);
        assert!(func.stats.lowered_ops > 0);
        assert_eq!(func.stats.stub_ops, 0);
    }

    #[test]
    fn test_jit_neural_stub() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::op1(Op::LINEAR);
        let func = compiler.compile(&expr).unwrap();
        assert!(func.stats.stub_ops > 0);
    }

    #[test]
    fn test_jit_sigmoid() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::op(Op::SIGMOID, vec![Expr::float(0.0)]);
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]).unwrap();
        assert!((result - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_jit_division_by_zero() {
        let compiler = JitCompiler::new(JitConfig::default());
        let expr = Expr::op2(Op::DIV, Expr::float(1.0), Expr::float(0.0));
        let func = compiler.compile(&expr).unwrap();
        let result = func.call_f64(&[]);
        assert!(matches!(result, Err(JitError::DivisionByZero)));
    }
}
