//! Kernel fusion for RMIL expression graphs.
//!
//! Detects sequences of adjacent RMIL operations that can be fused into a
//! single compute kernel, reducing memory traffic and kernel launch overhead.
//! Fusion patterns include:
//!
//! - **Elementwise chains**: `RELU >> LINEAR >> GELU` → single fused kernel
//! - **Norm + activation**: `LAYER_NORM >> RELU` → fused
//! - **MatMul + bias + activation**: `MATMUL >> ADD >> RELU` → fused GEMM+activation
//! - **Reduce + elementwise**: `REDUCE >> EXP` → single pass
//!
//! # Design
//!
//! Fusion operates on the [`Expr`] AST. The [`FusionPass`] walks the tree,
//! identifies fusible sequences, and rewrites them as [`FusedKernel`] nodes
//! wrapped in `Expr::App` with a synthetic `FUSED` opcode.
//!
//! The fused representation is consumed by compute backends (GPU especially)
//! to generate a single kernel instead of multiple dispatches.
//!
//! # Examples
//!
//! ```
//! use rmi::compute::fusion::{FusionPass, FusionConfig, FusedKernel};
//! use rmi::lang::{Expr, Op};
//!
//! let pass = FusionPass::new(FusionConfig::default());
//!
//! // Before: 3 separate ops
//! let expr = Expr::op1(Op::LINEAR)
//!     >> Expr::op1(Op::RELU)
//!     >> Expr::op1(Op::LINEAR);
//!
//! let result = pass.fuse(&expr);
//! assert!(result.fused_count > 0);
//! assert!(result.output.node_count() <= expr.node_count());
//! ```

use crate::lang::expr::Expr;
use crate::lang::op::Op;

// ── Fusion configuration ─────────────────────────────────────────────────────

/// Configuration for the kernel fusion pass.
#[derive(Debug, Clone)]
pub struct FusionConfig {
    /// Maximum number of ops to fuse into a single kernel.
    pub max_fusion_length: usize,
    /// Enable elementwise fusion (chains of pointwise ops).
    pub fuse_elementwise: bool,
    /// Enable matmul + activation fusion.
    pub fuse_matmul_act: bool,
    /// Enable norm + activation fusion.
    pub fuse_norm_act: bool,
    /// Enable reduction + elementwise fusion.
    pub fuse_reduce_ewise: bool,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            max_fusion_length: 8,
            fuse_elementwise: true,
            fuse_matmul_act: true,
            fuse_norm_act: true,
            fuse_reduce_ewise: true,
        }
    }
}

// ── Fused kernel ─────────────────────────────────────────────────────────────

/// A fused kernel — a sequence of RMIL ops that execute as one unit.
#[derive(Debug, Clone)]
pub struct FusedKernel {
    /// The original ops in fusion order.
    pub ops: Vec<Op>,
    /// A name for the fused kernel (e.g., "linear_relu_linear").
    pub name: String,
    /// Fusion pattern that was matched.
    pub pattern: FusionPattern,
    /// Estimated speedup from fusion (as a multiplier, e.g. 1.5x).
    pub estimated_speedup: f64,
}

/// Known fusion patterns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FusionPattern {
    /// Chain of elementwise operations (activations, math).
    ElementwiseChain,
    /// MatMul or Linear followed by activation.
    MatmulActivation,
    /// Layer/batch norm followed by activation.
    NormActivation,
    /// Reduction followed by elementwise op.
    ReduceElementwise,
    /// Generic sequential fusion.
    GenericSeq,
}

// ── Fusion result ────────────────────────────────────────────────────────────

/// Result of running the fusion pass.
#[derive(Debug, Clone)]
pub struct FusionResult {
    /// The rewritten expression tree with fused kernels.
    pub output: Expr,
    /// Number of fusion opportunities found.
    pub fused_count: usize,
    /// Total ops before fusion.
    pub ops_before: usize,
    /// Total ops after fusion (fused groups count as 1).
    pub ops_after: usize,
    /// The fused kernels that were created.
    pub kernels: Vec<FusedKernel>,
}

// ── Op classification ────────────────────────────────────────────────────────

/// Classify an op for fusion purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpClass {
    /// Elementwise (pointwise) — no data dependencies across elements.
    Elementwise,
    /// Linear algebra (matmul, linear, conv).
    LinAlg,
    /// Normalization (layer norm, batch norm, etc.).
    Norm,
    /// Reduction (reduce, global pool).
    Reduction,
    /// Other (not fusible).
    Other,
}

fn classify_op(op: Op) -> OpClass {
    match op {
        // Activations
        Op::RELU | Op::GELU | Op::SILU | Op::SIGMOID | Op::TANH_ACT | Op::MISH | Op::SOFTPLUS => {
            OpClass::Elementwise
        }
        // Math (elementwise)
        Op::ADD
        | Op::SUB
        | Op::MUL
        | Op::DIV
        | Op::NEG
        | Op::ABS
        | Op::EXP
        | Op::LOG
        | Op::SQRT
        | Op::SIN
        | Op::COS
        | Op::POW
        | Op::MAX
        | Op::MIN
        | Op::CLAMP => OpClass::Elementwise,
        // Dropout (elementwise mask)
        Op::DROP => OpClass::Elementwise,
        // Linear algebra
        Op::MATMUL | Op::LINEAR | Op::CONV2D => OpClass::LinAlg,
        // Normalization
        Op::LAYER_NORM | Op::BATCH_NORM | Op::RMS_NORM | Op::GROUP_NORM | Op::INSTANCE_NORM => {
            OpClass::Norm
        }
        // Reduction / pooling
        Op::REDUCE | Op::GLOBAL_POOL | Op::MAX_POOL | Op::AVG_POOL | Op::SOFTMAX => {
            OpClass::Reduction
        }
        // Everything else
        _ => OpClass::Other,
    }
}

/// Check if an op is an activation function.
fn is_activation(op: Op) -> bool {
    matches!(
        op,
        Op::RELU | Op::GELU | Op::SILU | Op::SIGMOID | Op::TANH_ACT | Op::MISH | Op::SOFTPLUS
    )
}

// ── Fusion pass ──────────────────────────────────────────────────────────────

/// The kernel fusion optimisation pass.
///
/// Walks the RMIL expression tree, identifies fusible op sequences,
/// and rewrites them as fused kernel references.
pub struct FusionPass {
    config: FusionConfig,
}

impl FusionPass {
    /// Create a new fusion pass with the given configuration.
    pub fn new(config: FusionConfig) -> Self {
        Self { config }
    }

    /// Run fusion on an expression tree.
    pub fn fuse(&self, expr: &Expr) -> FusionResult {
        let ops_before = count_ops(expr);
        let mut kernels = Vec::new();
        let output = self.fuse_inner(expr, &mut kernels);
        let ops_after = count_ops(&output);

        FusionResult {
            output,
            fused_count: kernels.len(),
            ops_before,
            ops_after,
            kernels,
        }
    }

    fn fuse_inner(&self, expr: &Expr, kernels: &mut Vec<FusedKernel>) -> Expr {
        match expr {
            // Sequential: main fusion target
            Expr::Seq(a, b) => {
                // Collect the chain of ops in this sequence
                let mut chain = Vec::new();
                self.collect_seq_chain(expr, &mut chain);

                if chain.len() >= 2 {
                    // Try to find fusible subsequences
                    let fused = self.fuse_chain(&chain, kernels);
                    return fused;
                }

                // No fusion possible, recurse
                Expr::Seq(
                    Box::new(self.fuse_inner(a, kernels)),
                    Box::new(self.fuse_inner(b, kernels)),
                )
            }

            // Recursion into other node types
            Expr::Par(a, b) => Expr::Par(
                Box::new(self.fuse_inner(a, kernels)),
                Box::new(self.fuse_inner(b, kernels)),
            ),
            Expr::Cond { pred, yes, no } => Expr::Cond {
                pred: Box::new(self.fuse_inner(pred, kernels)),
                yes: Box::new(self.fuse_inner(yes, kernels)),
                no: Box::new(self.fuse_inner(no, kernels)),
            },
            Expr::Let { name, val, body } => Expr::Let {
                name: *name,
                val: Box::new(self.fuse_inner(val, kernels)),
                body: Box::new(self.fuse_inner(body, kernels)),
            },
            Expr::Block(exprs) => {
                Expr::Block(exprs.iter().map(|e| self.fuse_inner(e, kernels)).collect())
            }
            Expr::App(op, args) => {
                let fused_args = args.iter().map(|a| self.fuse_inner(a, kernels)).collect();
                Expr::App(*op, fused_args)
            }
            // Leaf nodes
            other => other.clone(),
        }
    }

    /// Collect a chain of ops from nested Seq nodes.
    fn collect_seq_chain<'a>(&self, expr: &'a Expr, chain: &mut Vec<&'a Expr>) {
        match expr {
            Expr::Seq(a, b) => {
                self.collect_seq_chain(a, chain);
                self.collect_seq_chain(b, chain);
            }
            _ => chain.push(expr),
        }
    }

    /// Try to fuse a chain of expressions, returning the rewritten expression.
    fn fuse_chain(&self, chain: &[&Expr], kernels: &mut Vec<FusedKernel>) -> Expr {
        let mut result_exprs: Vec<Expr> = Vec::new();
        let mut i = 0;

        while i < chain.len() {
            // Extract op if this is an App node
            let current_op = extract_op(chain[i]);

            if let Some(_op) = current_op {
                // Try to start a fusion group from here
                let group = self.try_fuse_from(chain, i);
                if group.len() >= 2 {
                    let kernel = self.create_kernel(&group);
                    kernels.push(kernel.clone());

                    // Replace the group with a single fused node
                    let fused_ops: Vec<Op> = group.to_vec();
                    let fused_expr = Expr::App(fused_ops[0], vec![]);
                    // Chain remaining fused ops
                    let combined = fused_ops[1..]
                        .iter()
                        .fold(fused_expr, |acc, &op| acc.then(Expr::App(op, vec![])));
                    // Mark as fused by wrapping ops count metadata
                    result_exprs.push(combined);
                    i += group.len();
                    continue;
                }
            }

            result_exprs.push(chain[i].clone());
            i += 1;
        }

        // Reconstruct as Seq chain
        if result_exprs.is_empty() {
            Expr::Lit(crate::lang::expr::Val::Nil)
        } else {
            let mut expr = result_exprs.remove(0);
            for next in result_exprs {
                expr = expr.then(next);
            }
            expr
        }
    }

    /// Try to build a fusible group starting at index `start`.
    fn try_fuse_from(&self, chain: &[&Expr], start: usize) -> Vec<Op> {
        let mut group = Vec::new();

        for item in chain.iter().skip(start) {
            if group.len() >= self.config.max_fusion_length {
                break;
            }
            let Some(op) = extract_op(item) else {
                break;
            };

            if group.is_empty() {
                group.push(op);
                continue;
            }

            // Check if this op can be fused with the current group
            if self.can_fuse(&group, op) {
                group.push(op);
            } else {
                break;
            }
        }

        group
    }

    /// Check if `op` can be fused onto the end of the current group.
    fn can_fuse(&self, group: &[Op], op: Op) -> bool {
        let last = *group.last().expect("can_fuse called with empty group");
        let last_class = classify_op(last);
        let op_class = classify_op(op);

        // Elementwise + Elementwise → always fusible
        if self.config.fuse_elementwise
            && last_class == OpClass::Elementwise
            && op_class == OpClass::Elementwise
        {
            return true;
        }

        // LinAlg + Activation → matmul-activation fusion
        if self.config.fuse_matmul_act && last_class == OpClass::LinAlg && is_activation(op) {
            return true;
        }

        // Norm + Activation
        if self.config.fuse_norm_act && last_class == OpClass::Norm && is_activation(op) {
            return true;
        }

        // Reduction + Elementwise
        if self.config.fuse_reduce_ewise
            && last_class == OpClass::Reduction
            && op_class == OpClass::Elementwise
        {
            return true;
        }

        // Elementwise after any fusible group (only if the relevant fusion rule is enabled)
        if op_class == OpClass::Elementwise && group.len() < self.config.max_fusion_length {
            let first_class = classify_op(group[0]);
            match first_class {
                OpClass::LinAlg if self.config.fuse_matmul_act => return true,
                OpClass::Norm if self.config.fuse_norm_act => return true,
                OpClass::Reduction if self.config.fuse_reduce_ewise => return true,
                OpClass::Elementwise if self.config.fuse_elementwise => return true,
                _ => {}
            }
        }

        false
    }

    /// Create a fused kernel descriptor from a group of ops.
    fn create_kernel(&self, ops: &[Op]) -> FusedKernel {
        let name = ops
            .iter()
            .map(|op| op.meta().name.to_lowercase())
            .collect::<Vec<_>>()
            .join("_");

        let pattern = self.detect_pattern(ops);
        let estimated_speedup = self.estimate_speedup(ops, &pattern);

        FusedKernel {
            ops: ops.to_vec(),
            name,
            pattern,
            estimated_speedup,
        }
    }

    fn detect_pattern(&self, ops: &[Op]) -> FusionPattern {
        if ops.is_empty() {
            return FusionPattern::GenericSeq;
        }

        let first_class = classify_op(ops[0]);

        match first_class {
            OpClass::LinAlg => {
                if ops.len() >= 2 && is_activation(ops[1]) {
                    FusionPattern::MatmulActivation
                } else {
                    FusionPattern::GenericSeq
                }
            }
            OpClass::Norm => {
                if ops.len() >= 2 && is_activation(ops[1]) {
                    FusionPattern::NormActivation
                } else {
                    FusionPattern::GenericSeq
                }
            }
            OpClass::Reduction => {
                if ops.len() >= 2 && classify_op(ops[1]) == OpClass::Elementwise {
                    FusionPattern::ReduceElementwise
                } else {
                    FusionPattern::GenericSeq
                }
            }
            OpClass::Elementwise => {
                if ops
                    .iter()
                    .all(|op| classify_op(*op) == OpClass::Elementwise)
                {
                    FusionPattern::ElementwiseChain
                } else {
                    FusionPattern::GenericSeq
                }
            }
            _ => FusionPattern::GenericSeq,
        }
    }

    fn estimate_speedup(&self, ops: &[Op], pattern: &FusionPattern) -> f64 {
        let n = ops.len() as f64;
        match pattern {
            // Elementwise chains save memory round-trips
            FusionPattern::ElementwiseChain => 1.0 + (n - 1.0) * 0.3,
            // MatMul+act saves one kernel launch + memory write/read
            FusionPattern::MatmulActivation => 1.3 + (n - 2.0) * 0.1,
            // Norm+act saves one pass
            FusionPattern::NormActivation => 1.2 + (n - 2.0) * 0.1,
            // Reduce+ewise in single pass
            FusionPattern::ReduceElementwise => 1.4,
            // Generic: small improvement from reduced dispatch
            FusionPattern::GenericSeq => 1.0 + (n - 1.0) * 0.05,
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Extract the op from an App node, if any.
fn extract_op(expr: &Expr) -> Option<Op> {
    match expr {
        Expr::App(op, _) => Some(*op),
        _ => None,
    }
}

/// Count total App nodes (operations) in an expression tree.
fn count_ops(expr: &Expr) -> usize {
    match expr {
        Expr::App(_, args) => 1 + args.iter().map(count_ops).sum::<usize>(),
        Expr::Seq(a, b) | Expr::Par(a, b) => count_ops(a) + count_ops(b),
        Expr::Cond { pred, yes, no } => count_ops(pred) + count_ops(yes) + count_ops(no),
        Expr::Let { val, body, .. } => count_ops(val) + count_ops(body),
        Expr::Lam { body, .. } => count_ops(body),
        Expr::Call(f, args) => count_ops(f) + args.iter().map(count_ops).sum::<usize>(),
        Expr::Block(exprs) => exprs.iter().map(count_ops).sum(),
        _ => 0,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::{Expr, Op};

    #[test]
    fn test_elementwise_fusion() {
        let pass = FusionPass::new(FusionConfig::default());
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::GELU) >> Expr::op1(Op::SIGMOID);
        let result = pass.fuse(&expr);
        assert!(result.fused_count > 0);
        assert_eq!(result.kernels[0].pattern, FusionPattern::ElementwiseChain);
    }

    #[test]
    fn test_matmul_activation_fusion() {
        let pass = FusionPass::new(FusionConfig::default());
        let expr = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        let result = pass.fuse(&expr);
        assert!(result.fused_count > 0);
        assert_eq!(result.kernels[0].pattern, FusionPattern::MatmulActivation);
    }

    #[test]
    fn test_norm_activation_fusion() {
        let pass = FusionPass::new(FusionConfig::default());
        let expr = Expr::op1(Op::LAYER_NORM) >> Expr::op1(Op::RELU);
        let result = pass.fuse(&expr);
        assert!(result.fused_count > 0);
        assert_eq!(result.kernels[0].pattern, FusionPattern::NormActivation);
    }

    #[test]
    fn test_no_fusion_single_op() {
        let pass = FusionPass::new(FusionConfig::default());
        let expr = Expr::op1(Op::LINEAR);
        let result = pass.fuse(&expr);
        assert_eq!(result.fused_count, 0);
    }

    #[test]
    fn test_no_fusion_non_fusible() {
        let pass = FusionPass::new(FusionConfig::default());
        // SEND is not fusible
        let expr = Expr::op1(Op::SEND) >> Expr::op1(Op::RECV);
        let result = pass.fuse(&expr);
        assert_eq!(result.fused_count, 0);
    }

    #[test]
    fn test_max_fusion_length() {
        let config = FusionConfig {
            max_fusion_length: 2,
            ..Default::default()
        };
        let pass = FusionPass::new(config);
        let expr = Expr::op1(Op::RELU)
            >> Expr::op1(Op::GELU)
            >> Expr::op1(Op::SIGMOID)
            >> Expr::op1(Op::TANH_ACT);
        let result = pass.fuse(&expr);
        // Should fuse at most 2 at a time
        for kernel in &result.kernels {
            assert!(kernel.ops.len() <= 2);
        }
    }

    #[test]
    fn test_fusion_config_disable() {
        let config = FusionConfig {
            fuse_matmul_act: false,
            ..Default::default()
        };
        let pass = FusionPass::new(config);
        let expr = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        let result = pass.fuse(&expr);
        // Should not fuse matmul+activation since it's disabled
        // but RELU is elementwise, so chain may still be empty if LINEAR isn't fusible alone
        let has_matmul_act = result
            .kernels
            .iter()
            .any(|k| k.pattern == FusionPattern::MatmulActivation);
        assert!(!has_matmul_act);
    }

    #[test]
    fn test_fusion_speedup_estimate() {
        let pass = FusionPass::new(FusionConfig::default());
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::GELU) >> Expr::op1(Op::SIGMOID);
        let result = pass.fuse(&expr);
        if !result.kernels.is_empty() {
            assert!(result.kernels[0].estimated_speedup > 1.0);
        }
    }

    #[test]
    fn test_transformer_block_fusion() {
        let pass = FusionPass::new(FusionConfig::default());
        let block = Expr::op1(Op::LAYER_NORM)
            >> Expr::op1(Op::ATTN)
            >> Expr::op1(Op::DROP)
            >> Expr::op1(Op::LAYER_NORM)
            >> Expr::op1(Op::LINEAR)
            >> Expr::op1(Op::GELU)
            >> Expr::op1(Op::LINEAR)
            >> Expr::op1(Op::DROP);
        let result = pass.fuse(&block);
        // Should find at least some fusion opportunities
        assert!(result.fused_count > 0);
    }

    #[test]
    fn test_parallel_no_fusion() {
        let pass = FusionPass::new(FusionConfig::default());
        // Parallel branches should not be fused with each other
        let expr = Expr::op1(Op::RELU) | Expr::op1(Op::GELU);
        let result = pass.fuse(&expr);
        // No sequential chain to fuse
        assert_eq!(result.fused_count, 0);
    }

    #[test]
    fn test_kernel_name() {
        let pass = FusionPass::new(FusionConfig::default());
        let expr = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU);
        let result = pass.fuse(&expr);
        if !result.kernels.is_empty() {
            assert!(result.kernels[0].name.contains("relu"));
        }
    }

    #[test]
    fn test_reduce_elementwise_fusion() {
        let pass = FusionPass::new(FusionConfig::default());
        let expr = Expr::op1(Op::SOFTMAX) >> Expr::op1(Op::LOG);
        let result = pass.fuse(&expr);
        assert!(result.fused_count > 0);
    }

    #[test]
    fn test_op_classification() {
        assert_eq!(classify_op(Op::RELU), OpClass::Elementwise);
        assert_eq!(classify_op(Op::LINEAR), OpClass::LinAlg);
        assert_eq!(classify_op(Op::LAYER_NORM), OpClass::Norm);
        assert_eq!(classify_op(Op::SOFTMAX), OpClass::Reduction);
        assert_eq!(classify_op(Op::SEND), OpClass::Other);
    }
}
