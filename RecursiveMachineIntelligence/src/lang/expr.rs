//! RMIL expression AST and algebraic composition.
//!
//! Every RMIL program is a tree of [`Expr`] nodes. Composition operators
//! let agents build complex programs from atomic ops:
//!
//! ```text
//!   a >> b        sequential (category composition)
//!   a | b         parallel   (product)
//!   a.cond(p, b)  conditional
//!   a.repeat(n)   bounded iteration
//!   a.residual()  skip connection: x + a(x)
//! ```
//!
//! Every expression is **content-addressed** via [`Expr::content_hash`]:
//! structurally identical sub-trees share the same u64 hash.
//!
//! # Constructing expressions
//!
//! ```
//! use rmi::lang::{Op, Expr, Val};
//!
//! // A transformer block in ~5 lines:
//! let block =
//!     Expr::op1(Op::LAYER_NORM)
//!     >> Expr::op1(Op::ATTN)
//!     >> Expr::op1(Op::DROP)
//!     >> Expr::op1(Op::LAYER_NORM)
//!     >> Expr::op1(Op::LINEAR)
//!     >> Expr::op1(Op::GELU)
//!     >> Expr::op1(Op::LINEAR)
//!     >> Expr::op1(Op::DROP);
//! ```

use std::hash::{Hash, Hasher};

use crate::lang::op::Op;
use crate::lang::sym::Sym;
use crate::lang::ty::{Dtype, Ty};

// ── Values ───────────────────────────────────────────────────────────────────

/// A runtime value in the RMIL VM.
///
/// Kept deliberately flat — no recursive nesting except `Tuple`.
/// Tensor data is a raw byte buffer interpreted via its `Dtype` + shape.
///
/// # Examples
///
/// ```
/// use rmi::lang::expr::Val;
///
/// let v = Val::f64(3.14);
/// assert!((v.as_f64().unwrap() - 3.14).abs() < 1e-10);
///
/// let b = Val::Bool(true);
/// assert_eq!(b.as_bool(), Some(true));
/// ```
#[derive(Clone, Debug)]
pub enum Val {
    /// Unit / nil.
    Nil,
    /// Boolean.
    Bool(bool),
    /// 64-bit signed integer.
    I64(i64),
    /// 32-bit float (stored as bits for hashing).
    F32(u32),
    /// 64-bit float (stored as bits for hashing).
    F64(u64),
    /// Tensor: dtype + shape + raw bytes.
    Tensor {
        /// Element data type.
        dtype: Dtype,
        /// Shape (dimension sizes).
        shape: Vec<usize>,
        /// Raw byte data (length = numel × dtype.size()).
        data: Vec<u8>,
    },
    /// Interned symbol.
    Sym(Sym),
    /// Tuple of values (product).
    Tuple(Vec<Val>),
}

impl Val {
    /// Construct an F32 value.
    pub fn f32(v: f32) -> Self {
        Self::F32(v.to_bits())
    }

    /// Construct an F64 value.
    pub fn f64(v: f64) -> Self {
        Self::F64(v.to_bits())
    }

    /// Extract f32 if this is an F32 value.
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Self::F32(b) => Some(f32::from_bits(*b)),
            _ => None,
        }
    }

    /// Extract f64 if this is an F64 value.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(b) => Some(f64::from_bits(*b)),
            _ => None,
        }
    }

    /// Extract i64.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::I64(v) => Some(*v),
            _ => None,
        }
    }

    /// Extract bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Wire-format tag byte.
    pub(crate) fn tag(&self) -> u8 {
        match self {
            Self::Nil => 0,
            Self::Bool(_) => 1,
            Self::I64(_) => 2,
            Self::F32(_) => 3,
            Self::F64(_) => 4,
            Self::Tensor { .. } => 5,
            Self::Sym(_) => 6,
            Self::Tuple(_) => 7,
        }
    }
}

impl PartialEq for Val {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Nil, Self::Nil) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::I64(a), Self::I64(b)) => a == b,
            (Self::F32(a), Self::F32(b)) => a == b,
            (Self::F64(a), Self::F64(b)) => a == b,
            (
                Self::Tensor {
                    dtype: d1,
                    shape: s1,
                    data: v1,
                },
                Self::Tensor {
                    dtype: d2,
                    shape: s2,
                    data: v2,
                },
            ) => d1 == d2 && s1 == s2 && v1 == v2,
            (Self::Sym(a), Self::Sym(b)) => a == b,
            (Self::Tuple(a), Self::Tuple(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Val {}

impl Hash for Val {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.tag().hash(state);
        match self {
            Self::Nil => {}
            Self::Bool(v) => v.hash(state),
            Self::I64(v) => v.hash(state),
            Self::F32(v) => v.hash(state),
            Self::F64(v) => v.hash(state),
            Self::Tensor { dtype, shape, data } => {
                dtype.hash(state);
                shape.hash(state);
                data.hash(state);
            }
            Self::Sym(s) => s.hash(state),
            Self::Tuple(vs) => vs.hash(state),
        }
    }
}

// ── Expression AST ───────────────────────────────────────────────────────────

/// The core AST node of RMIL.
///
/// Programs are trees of `Expr` nodes composed via:
/// - `>>` (sequential / category morphism composition)
/// - `|`  (parallel / product)
/// - `.cond()`, `.repeat()`, `.residual()`
///
/// Every `Expr` can be hashed, serialised to binary, and sent between agents.
///
/// # Examples
///
/// ```
/// use rmi::lang::{Op, Expr, Val};
///
/// // Arithmetic: 2.0 + 3.0
/// let add = Expr::op2(Op::ADD, Expr::float(2.0), Expr::float(3.0));
///
/// // Sequential composition via >>
/// let pipeline = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Expr {
    /// Literal constant value.
    Lit(Val),

    /// Symbol reference (variable / binding name).
    Ref(Sym),

    /// Apply opcode to arguments: `op(args…)`.
    App(Op, Vec<Expr>),

    /// Sequential composition: `left >> right`.
    /// Categorical reading: morphism composition `right ∘ left`.
    Seq(Box<Expr>, Box<Expr>),

    /// Parallel composition: `left ‖ right`.
    /// Runs both on the same input, produces a tuple of outputs.
    Par(Box<Expr>, Box<Expr>),

    /// Conditional: `if pred then yes else no`.
    Cond {
        /// Predicate (must evaluate to Bool).
        pred: Box<Expr>,
        /// Then branch.
        yes: Box<Expr>,
        /// Else branch.
        no: Box<Expr>,
    },

    /// Let-binding: `let name = val in body`.
    Let {
        /// Bound name.
        name: Sym,
        /// Value expression.
        val: Box<Expr>,
        /// Body expression (may reference `name`).
        body: Box<Expr>,
    },

    /// Lambda abstraction: `λ(params) → body`.
    Lam {
        /// Parameter names with types.
        params: Vec<(Sym, Ty)>,
        /// Body.
        body: Box<Expr>,
    },

    /// Function call: `func(args…)`.
    Call(Box<Expr>, Vec<Expr>),

    /// Block: sequence of expressions, returns the last.
    Block(Vec<Expr>),
}

// ── Constructors ─────────────────────────────────────────────────────────────

impl Expr {
    /// Nullary op: `op()`.
    pub fn op0(op: Op) -> Self {
        Self::App(op, Vec::new())
    }

    /// Unary op: `op(a)` — used as a pipeline stage.
    /// When used with `>>`, arg is filled implicitly by the pipeline.
    pub fn op1(op: Op) -> Self {
        Self::App(op, Vec::new())
    }

    /// Binary op: `op(a, b)`.
    pub fn op2(op: Op, a: Expr, b: Expr) -> Self {
        Self::App(op, vec![a, b])
    }

    /// N-ary op: `op(args…)`.
    pub fn op(op: Op, args: Vec<Expr>) -> Self {
        Self::App(op, args)
    }

    /// Literal integer.
    pub fn int(v: i64) -> Self {
        Self::Lit(Val::I64(v))
    }

    /// Literal f32.
    pub fn float(v: f32) -> Self {
        Self::Lit(Val::f32(v))
    }

    /// Literal boolean.
    pub fn boolean(v: bool) -> Self {
        Self::Lit(Val::Bool(v))
    }

    /// Symbol reference.
    pub fn sym(s: Sym) -> Self {
        Self::Ref(s)
    }

    /// Identity (passthrough).
    pub fn id() -> Self {
        Self::op0(Op::IDENTITY)
    }

    // ── Composition methods ──────────────────────────────────────────────

    /// Sequential composition: `self >> next`.
    pub fn then(self, next: Expr) -> Self {
        Expr::Seq(Box::new(self), Box::new(next))
    }

    /// Parallel composition: `self ‖ other`.
    pub fn par(self, other: Expr) -> Self {
        Expr::Par(Box::new(self), Box::new(other))
    }

    /// Conditional: `if pred then self else other`.
    pub fn cond(self, pred: Expr, other: Expr) -> Self {
        Expr::Cond {
            pred: Box::new(pred),
            yes: Box::new(self),
            no: Box::new(other),
        }
    }

    /// Residual connection: `x + self(x)` (additive skip).
    pub fn residual(self) -> Self {
        Expr::App(Op::RES_ADD, vec![Expr::id(), self])
    }

    /// Repeat N times (loop unrolling).
    pub fn repeat(self, n: i64) -> Self {
        Expr::App(Op::REPEAT, vec![self, Expr::int(n)])
    }

    /// Map over collection.
    pub fn map_over(self, collection: Expr) -> Self {
        Expr::App(Op::MAP, vec![self, collection])
    }

    /// Let-binding shorthand.
    pub fn bind(name: Sym, val: Expr, body: Expr) -> Self {
        Expr::Let {
            name,
            val: Box::new(val),
            body: Box::new(body),
        }
    }

    /// Lambda shorthand.
    pub fn lam(params: Vec<(Sym, Ty)>, body: Expr) -> Self {
        Expr::Lam {
            params,
            body: Box::new(body),
        }
    }

    // ── Introspection ────────────────────────────────────────────────────

    /// Content hash (XXH3-64). Structurally identical expressions produce
    /// the same hash, enabling:
    /// - Deduplication of shared sub-expressions
    /// - Caching of evaluation results
    /// - Fast equality checks across agents
    pub fn content_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }

    /// Number of AST nodes in this expression tree.
    pub fn node_count(&self) -> usize {
        match self {
            Self::Lit(_) | Self::Ref(_) => 1,
            Self::App(_, args) => 1 + args.iter().map(|a| a.node_count()).sum::<usize>(),
            Self::Seq(a, b) | Self::Par(a, b) => 1 + a.node_count() + b.node_count(),
            Self::Cond { pred, yes, no } => {
                1 + pred.node_count() + yes.node_count() + no.node_count()
            }
            Self::Let { val, body, .. } => 1 + val.node_count() + body.node_count(),
            Self::Lam { body, .. } => 1 + body.node_count(),
            Self::Call(f, args) => {
                1 + f.node_count() + args.iter().map(|a| a.node_count()).sum::<usize>()
            }
            Self::Block(exprs) => 1 + exprs.iter().map(|e| e.node_count()).sum::<usize>(),
        }
    }

    /// Depth of the deepest node.
    pub fn depth(&self) -> usize {
        match self {
            Self::Lit(_) | Self::Ref(_) => 1,
            Self::App(_, args) => 1 + args.iter().map(|a| a.depth()).max().unwrap_or(0),
            Self::Seq(a, b) | Self::Par(a, b) => 1 + a.depth().max(b.depth()),
            Self::Cond { pred, yes, no } => 1 + pred.depth().max(yes.depth()).max(no.depth()),
            Self::Let { val, body, .. } => 1 + val.depth().max(body.depth()),
            Self::Lam { body, .. } => 1 + body.depth(),
            Self::Call(f, args) => {
                let max_arg = args.iter().map(|a| a.depth()).max().unwrap_or(0);
                1 + f.depth().max(max_arg)
            }
            Self::Block(exprs) => 1 + exprs.iter().map(|e| e.depth()).max().unwrap_or(0),
        }
    }

    /// Collect all unique opcodes used in this expression.
    pub fn opcodes(&self) -> Vec<Op> {
        let mut set = std::collections::HashSet::new();
        self.collect_ops(&mut set);
        let mut v: Vec<Op> = set.into_iter().collect();
        v.sort();
        v
    }

    fn collect_ops(&self, set: &mut std::collections::HashSet<Op>) {
        match self {
            Self::App(op, args) => {
                set.insert(*op);
                for a in args {
                    a.collect_ops(set);
                }
            }
            Self::Seq(a, b) | Self::Par(a, b) => {
                a.collect_ops(set);
                b.collect_ops(set);
            }
            Self::Cond { pred, yes, no } => {
                pred.collect_ops(set);
                yes.collect_ops(set);
                no.collect_ops(set);
            }
            Self::Let { val, body, .. } => {
                val.collect_ops(set);
                body.collect_ops(set);
            }
            Self::Lam { body, .. } => body.collect_ops(set),
            Self::Call(f, args) => {
                f.collect_ops(set);
                for a in args {
                    a.collect_ops(set);
                }
            }
            Self::Block(exprs) => {
                for e in exprs {
                    e.collect_ops(set);
                }
            }
            _ => {}
        }
    }

    /// Whether this expression tree is differentiable (all ops support gradients).
    pub fn is_differentiable(&self) -> bool {
        match self {
            Self::Lit(_) | Self::Ref(_) => true,
            Self::App(op, args) => {
                op.is_differentiable() && args.iter().all(|a| a.is_differentiable())
            }
            Self::Seq(a, b) | Self::Par(a, b) => a.is_differentiable() && b.is_differentiable(),
            Self::Cond { pred, yes, no } => {
                pred.is_differentiable() && yes.is_differentiable() && no.is_differentiable()
            }
            Self::Let { val, body, .. } => val.is_differentiable() && body.is_differentiable(),
            Self::Lam { body, .. } => body.is_differentiable(),
            Self::Call(f, args) => {
                f.is_differentiable() && args.iter().all(|a| a.is_differentiable())
            }
            Self::Block(exprs) => exprs.iter().all(|e| e.is_differentiable()),
        }
    }

    /// Wire-format tag byte.
    pub(crate) fn tag(&self) -> u8 {
        match self {
            Self::Lit(_) => 0,
            Self::Ref(_) => 1,
            Self::App(_, _) => 2,
            Self::Seq(_, _) => 3,
            Self::Par(_, _) => 4,
            Self::Cond { .. } => 5,
            Self::Let { .. } => 6,
            Self::Lam { .. } => 7,
            Self::Call(_, _) => 8,
            Self::Block(_) => 9,
        }
    }
}

// ── Operator overloading for ergonomic composition ───────────────────────────

/// `a >> b` = sequential composition.
impl std::ops::Shr for Expr {
    type Output = Expr;
    fn shr(self, rhs: Expr) -> Expr {
        Expr::Seq(Box::new(self), Box::new(rhs))
    }
}

/// `a | b` = parallel composition.
impl std::ops::BitOr for Expr {
    type Output = Expr;
    fn bitor(self, rhs: Expr) -> Expr {
        Expr::Par(Box::new(self), Box::new(rhs))
    }
}

// ── Common architecture patterns ─────────────────────────────────────────────

/// Pre-built RMIL expression templates for common architectures.
///
/// An agent can use these as starting points and mutate them.
pub mod patterns {
    use super::*;

    /// Transformer encoder block (pre-norm variant).
    ///
    /// ```text
    /// res_add(norm >> attn >> drop) >> res_add(norm >> linear >> gelu >> linear >> drop)
    /// ```
    pub fn transformer_block() -> Expr {
        let attn_path = Expr::op1(Op::LAYER_NORM) >> Expr::op1(Op::ATTN) >> Expr::op1(Op::DROP);

        let ffn_path = Expr::op1(Op::LAYER_NORM)
            >> Expr::op1(Op::LINEAR)
            >> Expr::op1(Op::GELU)
            >> Expr::op1(Op::LINEAR)
            >> Expr::op1(Op::DROP);

        attn_path.residual() >> ffn_path.residual()
    }

    /// MLP: linear >> act >> linear >> act >> linear.
    pub fn mlp(n_layers: usize) -> Expr {
        let mut e = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        for _ in 1..n_layers.saturating_sub(1) {
            e = e >> Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        }
        e >> Expr::op1(Op::LINEAR) // final layer, no activation
    }

    /// ResNet-style residual block.
    ///
    /// ```text
    /// res_add(conv >> bn >> relu >> conv >> bn) >> relu
    /// ```
    pub fn resnet_block() -> Expr {
        let path = Expr::op1(Op::CONV2D)
            >> Expr::op1(Op::BATCH_NORM)
            >> Expr::op1(Op::RELU)
            >> Expr::op1(Op::CONV2D)
            >> Expr::op1(Op::BATCH_NORM);
        path.residual() >> Expr::op1(Op::RELU)
    }

    /// RNN sequence model: embed >> lstm >> drop >> linear.
    pub fn rnn_model() -> Expr {
        Expr::op1(Op::EMBED) >> Expr::op1(Op::LSTM) >> Expr::op1(Op::DROP) >> Expr::op1(Op::LINEAR)
    }

    /// Classifier head: global_pool >> linear >> relu >> drop >> linear.
    pub fn classifier_head() -> Expr {
        Expr::op1(Op::GLOBAL_POOL)
            >> Expr::op1(Op::LINEAR)
            >> Expr::op1(Op::RELU)
            >> Expr::op1(Op::DROP)
            >> Expr::op1(Op::LINEAR)
    }

    /// Conv feature extractor: (conv >> bn >> relu >> pool) × N.
    pub fn conv_backbone(stages: usize) -> Expr {
        let stage = Expr::op1(Op::CONV2D)
            >> Expr::op1(Op::BATCH_NORM)
            >> Expr::op1(Op::RELU)
            >> Expr::op1(Op::MAX_POOL);
        stage.repeat(stages as i64)
    }

    /// Full image classifier: conv backbone >> classifier head.
    pub fn image_classifier(conv_stages: usize) -> Expr {
        conv_backbone(conv_stages) >> classifier_head()
    }

    /// Neurosymbolic hybrid: embed → neural path ‖ symbolic path → merge.
    pub fn neurosymbolic_hybrid() -> Expr {
        let neural = Expr::op1(Op::LINEAR) >> Expr::op1(Op::GELU) >> Expr::op1(Op::LINEAR);

        let symbolic = Expr::op1(Op::INFER) >> Expr::op1(Op::RESOLVE);

        Expr::op1(Op::EMBED) >> (neural | symbolic) >> Expr::op1(Op::CONCAT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::sym::Sym;

    #[test]
    fn sequential_composition() {
        let e = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        assert_eq!(e.node_count(), 5); // 2 Seq + 3 App
    }

    #[test]
    fn parallel_composition() {
        let e = Expr::op1(Op::LINEAR) | Expr::op1(Op::CONV2D);
        assert!(matches!(e, Expr::Par(_, _)));
    }

    #[test]
    fn content_hash_deterministic() {
        let a = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        let b = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        assert_eq!(a.content_hash(), b.content_hash());
    }

    #[test]
    fn content_hash_distinct() {
        let a = Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        let b = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        assert_ne!(a.content_hash(), b.content_hash());
    }

    #[test]
    fn transformer_block_shape() {
        let block = patterns::transformer_block();
        assert!(block.node_count() > 10);
        let ops = block.opcodes();
        assert!(ops.contains(&Op::ATTN));
        assert!(ops.contains(&Op::LAYER_NORM));
        assert!(ops.contains(&Op::LINEAR));
        assert!(ops.contains(&Op::GELU));
    }

    #[test]
    fn mlp_construction() {
        let mlp = patterns::mlp(3);
        assert!(mlp.node_count() >= 7);
        assert!(mlp.is_differentiable());
    }

    #[test]
    fn resnet_block() {
        let block = patterns::resnet_block();
        let ops = block.opcodes();
        assert!(ops.contains(&Op::CONV2D));
        assert!(ops.contains(&Op::BATCH_NORM));
        assert!(ops.contains(&Op::RES_ADD));
    }

    #[test]
    fn residual_wraps() {
        let e = Expr::op1(Op::LINEAR).residual();
        assert!(matches!(e, Expr::App(Op::RES_ADD, _)));
    }

    #[test]
    fn differentiable_check() {
        let good = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        assert!(good.is_differentiable());

        let bad = Expr::op1(Op::LINEAR) >> Expr::op1(Op::SEND);
        assert!(!bad.is_differentiable());
    }

    #[test]
    fn depth() {
        let flat = Expr::int(42);
        assert_eq!(flat.depth(), 1);

        let deep = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        assert!(deep.depth() >= 3);
    }

    #[test]
    fn val_f32_roundtrip() {
        let v = Val::f32(3.15);
        assert!((v.as_f32().unwrap() - 3.15).abs() < 1e-6);
    }

    #[test]
    fn neurosymbolic_pattern() {
        let hybrid = patterns::neurosymbolic_hybrid();
        let ops = hybrid.opcodes();
        // Has both neural and symbolic ops
        assert!(ops.contains(&Op::LINEAR));
        assert!(ops.contains(&Op::INFER));
        assert!(ops.contains(&Op::RESOLVE));
    }

    #[test]
    fn bind_and_ref() {
        let x = Sym(1);
        let e = Expr::bind(x, Expr::int(42), Expr::sym(x));
        assert!(matches!(e, Expr::Let { .. }));
        assert_eq!(e.node_count(), 3); // let + lit + ref
    }
}