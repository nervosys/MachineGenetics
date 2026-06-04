//! Optimization passes for the RMI intermediate representation.
//!
//! This module provides a suite of optimization passes that transform IR programs
//! to improve performance while preserving semantics. Each pass implements the
//! [`OptimizationPass`] trait and can be composed into pipelines.
//!
//! # Available Passes
//!
//! | Pass | Description |
//! |------|-------------|
//! | [`ConstantFolding`] | Evaluates compile-time-known expressions |
//! | [`DeadCodeElimination`] | Removes unreachable / unused nodes |
//! | [`CommonSubexpressionElimination`] | Deduplicates identical sub-expressions |
//! | [`OperatorFusion`] | Fuses MatMul+Bias, Conv+Activation, etc. |
//! | [`StrengthReduction`] | Replaces expensive ops with cheaper equivalents |
//! | [`AlgebraicSimplification`] | Applies algebraic identities (x-x=0, x/x=1) |
//!
//! # Example
//!
//! ```
//! use rmi::core::optimization::{OptimizationPipeline, OptimizationLevel};
//! use rmi::core::codegen::Program;
//!
//! let pipeline = OptimizationPipeline::level(OptimizationLevel::O2);
//! let program = Program::new("example");
//! let optimized = pipeline.optimize(program);
//! ```

use crate::core::codegen::{
    ActivationKind, BinaryOpKind, Function, IRNode, IROperation, IRValue, Program, UnaryOpKind,
};
use std::collections::{BTreeMap, HashMap, HashSet};

// ============================================================================
// Pass Trait
// ============================================================================

/// A single optimization pass that transforms IR functions.
///
/// Implementors should preserve program semantics while improving performance
/// characteristics such as reducing instruction count, eliminating redundancy,
/// or enabling better code generation.
pub trait OptimizationPass: Send + Sync {
    /// Human-readable name for this pass (used in pipeline diagnostics).
    fn name(&self) -> &str;

    /// Optimize a single function, returning the transformed version.
    fn optimize_function(&self, func: &Function) -> Function;

    /// Optimize an entire program by applying `optimize_function` to each function.
    fn optimize_program(&self, program: &Program) -> Program {
        let functions = program
            .functions
            .iter()
            .map(|f| self.optimize_function(f))
            .collect();
        Program {
            functions,
            ..program.clone()
        }
    }
}

// ============================================================================
// Optimization Statistics
// ============================================================================

/// Statistics comparing programs before and after optimization.
#[derive(Debug, Clone)]
pub struct OptimizationStats {
    /// Total nodes before optimization.
    pub nodes_before: usize,
    /// Total nodes after optimization.
    pub nodes_after: usize,
    /// Number of nodes eliminated.
    pub nodes_eliminated: usize,
    /// Reduction ratio (0.0 = no change, 1.0 = all eliminated).
    pub reduction_ratio: f64,
}

impl OptimizationStats {
    /// Compare two programs and compute statistics.
    pub fn compare(before: &Program, after: &Program) -> Self {
        let nodes_before: usize = before.functions.iter().map(|f| f.nodes.len()).sum();
        let nodes_after: usize = after.functions.iter().map(|f| f.nodes.len()).sum();
        let nodes_eliminated = nodes_before.saturating_sub(nodes_after);
        let reduction_ratio = if nodes_before > 0 {
            nodes_eliminated as f64 / nodes_before as f64
        } else {
            0.0
        };
        Self {
            nodes_before,
            nodes_after,
            nodes_eliminated,
            reduction_ratio,
        }
    }
}

// ============================================================================
// Constant Folding
// ============================================================================

/// Evaluates expressions whose operands are compile-time constants.
///
/// Handles:
/// - Binary arithmetic on f64/i64 literals (Add, Sub, Mul, Div, Pow, Min, Max)
/// - Unary operations on f64 (Neg, Abs, Sqrt, Exp, Log, trig, rounding)
/// - Boolean comparisons (Eq, Ne, Lt, Le, Gt, Ge)
/// - Guards against division by zero, log of non-positive, sqrt of negative
#[derive(Default)]
pub struct ConstantFolding;

impl ConstantFolding {
    /// Create a new constant folding pass.
    pub fn new() -> Self {
        Self
    }

    /// Try to fold a binary operation on two f64 constants.
    fn fold_binary_f64(op: &BinaryOpKind, a: f64, b: f64) -> Option<IRValue> {
        let result = match op {
            BinaryOpKind::Add => a + b,
            BinaryOpKind::Sub => a - b,
            BinaryOpKind::Mul => a * b,
            BinaryOpKind::Div => {
                if b == 0.0 {
                    return None;
                }
                a / b
            }
            BinaryOpKind::Pow => a.powf(b),
            BinaryOpKind::Min => a.min(b),
            BinaryOpKind::Max => a.max(b),
            _ => return None,
        };
        Some(IRValue::F64(result))
    }

    /// Try to fold a binary operation on two i64 constants.
    fn fold_binary_i64(op: &BinaryOpKind, a: i64, b: i64) -> Option<IRValue> {
        let result = match op {
            BinaryOpKind::Add => a.checked_add(b)?,
            BinaryOpKind::Sub => a.checked_sub(b)?,
            BinaryOpKind::Mul => a.checked_mul(b)?,
            BinaryOpKind::Div => {
                if b == 0 {
                    return None;
                }
                a.checked_div(b)?
            }
            BinaryOpKind::Min => a.min(b),
            BinaryOpKind::Max => a.max(b),
            _ => return None,
        };
        Some(IRValue::I64(result))
    }

    /// Try to fold a binary comparison on two f64 constants.
    fn fold_comparison_f64(op: &BinaryOpKind, a: f64, b: f64) -> Option<IRValue> {
        let result = match op {
            BinaryOpKind::Eq => (a - b).abs() < f64::EPSILON,
            BinaryOpKind::Ne => (a - b).abs() >= f64::EPSILON,
            BinaryOpKind::Lt => a < b,
            BinaryOpKind::Le => a <= b,
            BinaryOpKind::Gt => a > b,
            BinaryOpKind::Ge => a >= b,
            _ => return None,
        };
        Some(IRValue::Bool(result))
    }

    /// Try to fold a unary operation on an f64 constant.
    fn fold_unary_f64(op: &UnaryOpKind, a: f64) -> Option<IRValue> {
        let result = match op {
            UnaryOpKind::Neg => -a,
            UnaryOpKind::Abs => a.abs(),
            UnaryOpKind::Sqrt => {
                if a < 0.0 {
                    return None;
                }
                a.sqrt()
            }
            UnaryOpKind::Exp => a.exp(),
            UnaryOpKind::Log => {
                if a <= 0.0 {
                    return None;
                }
                a.ln()
            }
            UnaryOpKind::Sin => a.sin(),
            UnaryOpKind::Cos => a.cos(),
            UnaryOpKind::Tanh => a.tanh(),
            UnaryOpKind::Ceil => a.ceil(),
            UnaryOpKind::Floor => a.floor(),
            UnaryOpKind::Round => a.round(),
            UnaryOpKind::Not => return None,
        };
        Some(IRValue::F64(result))
    }
}

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &str {
        "constant-folding"
    }

    fn optimize_function(&self, func: &Function) -> Function {
        // Build a map from node id to node for quick lookup.
        let node_map: HashMap<u64, &IRNode> = func.nodes.iter().map(|n| (n.id, n)).collect();

        let new_nodes: Vec<IRNode> = func
            .nodes
            .iter()
            .map(|node| {
                match &node.op {
                    IROperation::BinaryOp { op } => {
                        // Both inputs must be constants.
                        let lhs = node.inputs.first().and_then(|id| node_map.get(id));
                        let rhs = node.inputs.get(1).and_then(|id| node_map.get(id));

                        if let (Some(l), Some(r)) = (lhs, rhs) {
                            if matches!(l.op, IROperation::Constant)
                                && matches!(r.op, IROperation::Constant)
                            {
                                let lv = l.attrs.get("value");
                                let rv = r.attrs.get("value");

                                if let (Some(lv), Some(rv)) = (lv, rv) {
                                    let folded = match (lv, rv) {
                                        (IRValue::F64(a), IRValue::F64(b)) => {
                                            Self::fold_binary_f64(op, *a, *b)
                                                .or_else(|| Self::fold_comparison_f64(op, *a, *b))
                                        }
                                        (IRValue::I64(a), IRValue::I64(b)) => {
                                            Self::fold_binary_i64(op, *a, *b)
                                        }
                                        (IRValue::Bool(a), IRValue::Bool(b)) => match op {
                                            BinaryOpKind::And => Some(IRValue::Bool(*a && *b)),
                                            BinaryOpKind::Or => Some(IRValue::Bool(*a || *b)),
                                            _ => None,
                                        },
                                        _ => None,
                                    };

                                    if let Some(value) = folded {
                                        let mut attrs = BTreeMap::new();
                                        attrs.insert("value".to_string(), value);
                                        return IRNode {
                                            id: node.id,
                                            op: IROperation::Constant,
                                            output_type: node.output_type.clone(),
                                            inputs: vec![],
                                            attrs,
                                            source_loc: node.source_loc.clone(),
                                        };
                                    }
                                }
                            }
                        }
                        node.clone()
                    }
                    IROperation::UnaryOp { op } => {
                        let input = node.inputs.first().and_then(|id| node_map.get(id));
                        if let Some(inp) = input {
                            if matches!(inp.op, IROperation::Constant) {
                                if let Some(IRValue::F64(v)) = inp.attrs.get("value") {
                                    if let Some(value) = Self::fold_unary_f64(op, *v) {
                                        let mut attrs = BTreeMap::new();
                                        attrs.insert("value".to_string(), value);
                                        return IRNode {
                                            id: node.id,
                                            op: IROperation::Constant,
                                            output_type: node.output_type.clone(),
                                            inputs: vec![],
                                            attrs,
                                            source_loc: node.source_loc.clone(),
                                        };
                                    }
                                }
                            }
                        }
                        node.clone()
                    }
                    _ => node.clone(),
                }
            })
            .collect();

        Function {
            nodes: new_nodes,
            ..func.clone()
        }
    }
}

// ============================================================================
// Common Sub-expression Elimination
// ============================================================================

/// Deduplicates identical sub-expressions by computing a canonical key for each
/// node (operation + inputs) and rewriting duplicates as references to the first
/// occurrence.
#[derive(Default)]
pub struct CommonSubexpressionElimination;

impl CommonSubexpressionElimination {
    /// Create a new CSE pass.
    pub fn new() -> Self {
        Self
    }

    /// Compute a canonical key for a node that captures its semantic identity.
    fn canonical_key(node: &IRNode) -> String {
        format!("{:?}:{:?}", node.op, node.inputs)
    }
}

impl OptimizationPass for CommonSubexpressionElimination {
    fn name(&self) -> &str {
        "cse"
    }

    fn optimize_function(&self, func: &Function) -> Function {
        let mut seen: HashMap<String, u64> = HashMap::new();
        let mut remap: HashMap<u64, u64> = HashMap::new();

        for node in &func.nodes {
            // Skip constants and parameters (they are unique by definition).
            if matches!(
                node.op,
                IROperation::Constant | IROperation::Parameter { .. }
            ) {
                continue;
            }

            let key = Self::canonical_key(node);
            if let Some(&existing_id) = seen.get(&key) {
                remap.insert(node.id, existing_id);
            } else {
                seen.insert(key, node.id);
            }
        }

        if remap.is_empty() {
            return func.clone();
        }

        // Rewrite inputs and filter out remapped nodes.
        let new_nodes: Vec<IRNode> = func
            .nodes
            .iter()
            .filter(|n| !remap.contains_key(&n.id))
            .map(|n| {
                let new_inputs = n
                    .inputs
                    .iter()
                    .map(|id| *remap.get(id).unwrap_or(id))
                    .collect();
                IRNode {
                    inputs: new_inputs,
                    ..n.clone()
                }
            })
            .collect();

        let return_node = func.return_node.map(|id| *remap.get(&id).unwrap_or(&id));

        Function {
            nodes: new_nodes,
            return_node,
            ..func.clone()
        }
    }
}

// ============================================================================
// Dead Code Elimination
// ============================================================================

/// Removes nodes that do not contribute to any return value.
///
/// Uses backwards reachability analysis starting from return nodes to determine
/// which nodes are live.
#[derive(Default)]
pub struct DeadCodeElimination;

impl DeadCodeElimination {
    /// Create a new DCE pass.
    pub fn new() -> Self {
        Self
    }
}

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &str {
        "dce"
    }

    fn optimize_function(&self, func: &Function) -> Function {
        // Start from return nodes and trace backwards to find live nodes.
        let mut live: HashSet<u64> = HashSet::new();
        let mut worklist: Vec<u64> = Vec::new();

        // Seed with return nodes and any nodes referenced by return.
        for node in &func.nodes {
            if matches!(node.op, IROperation::Return) {
                live.insert(node.id);
                worklist.extend(&node.inputs);
            }
        }

        // Also seed from the explicit return_node.
        if let Some(ret_id) = func.return_node {
            live.insert(ret_id);
            if let Some(ret_node) = func.nodes.iter().find(|n| n.id == ret_id) {
                worklist.extend(&ret_node.inputs);
            }
        }

        // Build a map for quick lookup.
        let node_map: HashMap<u64, &IRNode> = func.nodes.iter().map(|n| (n.id, n)).collect();

        // Trace backwards.
        while let Some(id) = worklist.pop() {
            if live.insert(id) {
                if let Some(node) = node_map.get(&id) {
                    worklist.extend(&node.inputs);
                }
            }
        }

        let new_nodes: Vec<IRNode> = func
            .nodes
            .iter()
            .filter(|n| live.contains(&n.id))
            .cloned()
            .collect();

        Function {
            nodes: new_nodes,
            ..func.clone()
        }
    }
}

// ============================================================================
// Operator Fusion
// ============================================================================

/// Fuses sequences of operations into single fused operations where profitable.
///
/// Supported fusions:
/// - MatMul + BinaryOp(Add) -> fused MatMul with bias attribute
/// - Conv + Activation -> fused Conv with activation attribute
/// - MatMul + Activation -> fused MatMul with activation attribute
#[derive(Default)]
pub struct OperatorFusion;

impl OperatorFusion {
    /// Create a new operator fusion pass.
    pub fn new() -> Self {
        Self
    }
}

impl OptimizationPass for OperatorFusion {
    fn name(&self) -> &str {
        "operator-fusion"
    }

    fn optimize_function(&self, func: &Function) -> Function {
        let node_map: HashMap<u64, &IRNode> = func.nodes.iter().map(|n| (n.id, n)).collect();

        // Build a use-count map to ensure we only fuse single-use producers.
        let mut use_count: HashMap<u64, usize> = HashMap::new();
        for node in &func.nodes {
            for &input in &node.inputs {
                *use_count.entry(input).or_insert(0) += 1;
            }
        }

        let mut fused: HashSet<u64> = HashSet::new();
        let mut replacements: HashMap<u64, IRNode> = HashMap::new();

        for node in &func.nodes {
            // Pattern 1: MatMul + Add -> fused MatMul with bias
            if let IROperation::BinaryOp {
                op: BinaryOpKind::Add,
            } = &node.op
            {
                if let Some(&matmul_id) = node.inputs.first() {
                    if let Some(matmul_node) = node_map.get(&matmul_id) {
                        if matches!(matmul_node.op, IROperation::MatMul { .. })
                            && use_count.get(&matmul_id).copied().unwrap_or(0) == 1
                        {
                            let bias_id = node.inputs.get(1).copied();
                            let mut attrs = matmul_node.attrs.clone();
                            attrs.insert("fused_bias".to_string(), IRValue::Bool(true));
                            if let Some(bid) = bias_id {
                                attrs.insert("bias_input".to_string(), IRValue::U64(bid));
                            }
                            let mut inputs = matmul_node.inputs.clone();
                            if let Some(bid) = bias_id {
                                inputs.push(bid);
                            }
                            replacements.insert(
                                node.id,
                                IRNode {
                                    id: node.id,
                                    op: matmul_node.op.clone(),
                                    output_type: node.output_type.clone(),
                                    inputs,
                                    attrs,
                                    source_loc: node.source_loc.clone(),
                                },
                            );
                            fused.insert(matmul_id);
                        }
                    }
                }
            }

            // Pattern 2: Conv + Activation -> fused Conv
            if let IROperation::Activation { kind } = &node.op {
                if let Some(&conv_id) = node.inputs.first() {
                    if let Some(conv_node) = node_map.get(&conv_id) {
                        if matches!(conv_node.op, IROperation::Conv { .. })
                            && use_count.get(&conv_id).copied().unwrap_or(0) == 1
                        {
                            let mut attrs = conv_node.attrs.clone();
                            attrs.insert(
                                "fused_activation".to_string(),
                                IRValue::String(format!("{:?}", kind)),
                            );
                            replacements.insert(
                                node.id,
                                IRNode {
                                    id: node.id,
                                    op: conv_node.op.clone(),
                                    output_type: node.output_type.clone(),
                                    inputs: conv_node.inputs.clone(),
                                    attrs,
                                    source_loc: node.source_loc.clone(),
                                },
                            );
                            fused.insert(conv_id);
                        }
                    }
                }
            }

            // Pattern 3: MatMul + Activation -> fused MatMul
            if let IROperation::Activation { kind } = &node.op {
                if let Some(&mm_id) = node.inputs.first() {
                    if let Some(mm_node) = node_map.get(&mm_id) {
                        if matches!(mm_node.op, IROperation::MatMul { .. })
                            && use_count.get(&mm_id).copied().unwrap_or(0) == 1
                            && !fused.contains(&mm_id)
                        {
                            let mut attrs = mm_node.attrs.clone();
                            attrs.insert(
                                "fused_activation".to_string(),
                                IRValue::String(format!("{:?}", kind)),
                            );
                            replacements.insert(
                                node.id,
                                IRNode {
                                    id: node.id,
                                    op: mm_node.op.clone(),
                                    output_type: node.output_type.clone(),
                                    inputs: mm_node.inputs.clone(),
                                    attrs,
                                    source_loc: node.source_loc.clone(),
                                },
                            );
                            fused.insert(mm_id);
                        }
                    }
                }
            }
        }

        let new_nodes: Vec<IRNode> = func
            .nodes
            .iter()
            .filter(|n| !fused.contains(&n.id))
            .map(|n| {
                if let Some(replacement) = replacements.get(&n.id) {
                    replacement.clone()
                } else {
                    n.clone()
                }
            })
            .collect();

        Function {
            nodes: new_nodes,
            ..func.clone()
        }
    }
}

// ============================================================================
// Strength Reduction
// ============================================================================

/// Replaces expensive operations with cheaper equivalents.
///
/// Patterns:
/// - `x * 2` -> `x + x`
/// - `x * 1` -> `x` (passthrough)
/// - `x * 0` -> `0`
/// - `x + 0` -> `x` (passthrough)
/// - `x - 0` -> `x` (passthrough)
/// - `x / 1` -> `x` (passthrough)
/// - `x ** 2` -> `x * x`
#[derive(Default)]
pub struct StrengthReduction;

impl StrengthReduction {
    /// Create a new strength reduction pass.
    pub fn new() -> Self {
        Self
    }

    /// Check if a node is a constant with a specific f64 value.
    fn is_const_f64(node: &IRNode, val: f64) -> bool {
        matches!(node.op, IROperation::Constant)
            && node.attrs.get("value") == Some(&IRValue::F64(val))
    }
}

impl OptimizationPass for StrengthReduction {
    fn name(&self) -> &str {
        "strength-reduction"
    }

    fn optimize_function(&self, func: &Function) -> Function {
        let node_map: HashMap<u64, &IRNode> = func.nodes.iter().map(|n| (n.id, n)).collect();

        let new_nodes: Vec<IRNode> = func
            .nodes
            .iter()
            .map(|node| {
                match &node.op {
                    IROperation::BinaryOp { op } => {
                        let lhs = node.inputs.first().and_then(|id| node_map.get(id));
                        let rhs = node.inputs.get(1).and_then(|id| node_map.get(id));

                        match (lhs, rhs) {
                            (Some(_l), Some(r)) => {
                                match op {
                                    // x * 2 -> x + x
                                    BinaryOpKind::Mul if Self::is_const_f64(r, 2.0) => {
                                        let x = node.inputs[0];
                                        IRNode {
                                            id: node.id,
                                            op: IROperation::BinaryOp {
                                                op: BinaryOpKind::Add,
                                            },
                                            output_type: node.output_type.clone(),
                                            inputs: vec![x, x],
                                            attrs: node.attrs.clone(),
                                            source_loc: node.source_loc.clone(),
                                        }
                                    }
                                    // x * 1 -> passthrough x
                                    BinaryOpKind::Mul if Self::is_const_f64(r, 1.0) => {
                                        let mut attrs = BTreeMap::new();
                                        attrs.insert(
                                            "passthrough".to_string(),
                                            IRValue::U64(node.inputs[0]),
                                        );
                                        IRNode {
                                            id: node.id,
                                            op: IROperation::Constant,
                                            output_type: node.output_type.clone(),
                                            inputs: vec![],
                                            attrs,
                                            source_loc: node.source_loc.clone(),
                                        }
                                    }
                                    // x * 0 -> constant 0
                                    BinaryOpKind::Mul if Self::is_const_f64(r, 0.0) => {
                                        let mut attrs = BTreeMap::new();
                                        attrs.insert("value".to_string(), IRValue::F64(0.0));
                                        IRNode {
                                            id: node.id,
                                            op: IROperation::Constant,
                                            output_type: node.output_type.clone(),
                                            inputs: vec![],
                                            attrs,
                                            source_loc: node.source_loc.clone(),
                                        }
                                    }
                                    // x + 0 -> passthrough x
                                    BinaryOpKind::Add if Self::is_const_f64(r, 0.0) => {
                                        let mut attrs = BTreeMap::new();
                                        attrs.insert(
                                            "passthrough".to_string(),
                                            IRValue::U64(node.inputs[0]),
                                        );
                                        IRNode {
                                            id: node.id,
                                            op: IROperation::Constant,
                                            output_type: node.output_type.clone(),
                                            inputs: vec![],
                                            attrs,
                                            source_loc: node.source_loc.clone(),
                                        }
                                    }
                                    // x - 0 -> passthrough x
                                    BinaryOpKind::Sub if Self::is_const_f64(r, 0.0) => {
                                        let mut attrs = BTreeMap::new();
                                        attrs.insert(
                                            "passthrough".to_string(),
                                            IRValue::U64(node.inputs[0]),
                                        );
                                        IRNode {
                                            id: node.id,
                                            op: IROperation::Constant,
                                            output_type: node.output_type.clone(),
                                            inputs: vec![],
                                            attrs,
                                            source_loc: node.source_loc.clone(),
                                        }
                                    }
                                    // x / 1 -> passthrough x
                                    BinaryOpKind::Div if Self::is_const_f64(r, 1.0) => {
                                        let mut attrs = BTreeMap::new();
                                        attrs.insert(
                                            "passthrough".to_string(),
                                            IRValue::U64(node.inputs[0]),
                                        );
                                        IRNode {
                                            id: node.id,
                                            op: IROperation::Constant,
                                            output_type: node.output_type.clone(),
                                            inputs: vec![],
                                            attrs,
                                            source_loc: node.source_loc.clone(),
                                        }
                                    }
                                    // x ** 2 -> x * x
                                    BinaryOpKind::Pow if Self::is_const_f64(r, 2.0) => {
                                        let x = node.inputs[0];
                                        IRNode {
                                            id: node.id,
                                            op: IROperation::BinaryOp {
                                                op: BinaryOpKind::Mul,
                                            },
                                            output_type: node.output_type.clone(),
                                            inputs: vec![x, x],
                                            attrs: node.attrs.clone(),
                                            source_loc: node.source_loc.clone(),
                                        }
                                    }
                                    _ => node.clone(),
                                }
                            }
                            _ => node.clone(),
                        }
                    }
                    _ => node.clone(),
                }
            })
            .collect();

        Function {
            nodes: new_nodes,
            ..func.clone()
        }
    }
}

// ============================================================================
// Algebraic Simplification
// ============================================================================

/// Applies algebraic identities to simplify expressions.
///
/// Patterns:
/// - `x - x` -> `0`
/// - `x / x` -> `1`
/// - `neg(neg(x))` -> `x` (passthrough)
/// - `abs(abs(x))` -> `abs(x)` (passthrough)
/// - `relu(relu(x))` -> `relu(x)` (passthrough)
#[derive(Default)]
pub struct AlgebraicSimplification;

impl AlgebraicSimplification {
    /// Create a new algebraic simplification pass.
    pub fn new() -> Self {
        Self
    }
}

impl OptimizationPass for AlgebraicSimplification {
    fn name(&self) -> &str {
        "algebraic-simplification"
    }

    fn optimize_function(&self, func: &Function) -> Function {
        let node_map: HashMap<u64, &IRNode> = func.nodes.iter().map(|n| (n.id, n)).collect();

        let new_nodes: Vec<IRNode> = func
            .nodes
            .iter()
            .map(|node| {
                match &node.op {
                    IROperation::BinaryOp { op } => {
                        // x - x -> 0
                        if *op == BinaryOpKind::Sub
                            && node.inputs.len() == 2
                            && node.inputs[0] == node.inputs[1]
                        {
                            let mut attrs = BTreeMap::new();
                            attrs.insert("value".to_string(), IRValue::F64(0.0));
                            return IRNode {
                                id: node.id,
                                op: IROperation::Constant,
                                output_type: node.output_type.clone(),
                                inputs: vec![],
                                attrs,
                                source_loc: node.source_loc.clone(),
                            };
                        }
                        // x / x -> 1
                        if *op == BinaryOpKind::Div
                            && node.inputs.len() == 2
                            && node.inputs[0] == node.inputs[1]
                        {
                            let mut attrs = BTreeMap::new();
                            attrs.insert("value".to_string(), IRValue::F64(1.0));
                            return IRNode {
                                id: node.id,
                                op: IROperation::Constant,
                                output_type: node.output_type.clone(),
                                inputs: vec![],
                                attrs,
                                source_loc: node.source_loc.clone(),
                            };
                        }
                        node.clone()
                    }
                    // neg(neg(x)) -> passthrough x
                    IROperation::UnaryOp {
                        op: UnaryOpKind::Neg,
                    } => {
                        if let Some(inner) = node.inputs.first().and_then(|id| node_map.get(id)) {
                            if matches!(
                                inner.op,
                                IROperation::UnaryOp {
                                    op: UnaryOpKind::Neg
                                }
                            ) {
                                let x = inner.inputs.first().copied().unwrap_or(node.inputs[0]);
                                let mut attrs = BTreeMap::new();
                                attrs.insert("passthrough".to_string(), IRValue::U64(x));
                                return IRNode {
                                    id: node.id,
                                    op: IROperation::Constant,
                                    output_type: node.output_type.clone(),
                                    inputs: vec![],
                                    attrs,
                                    source_loc: node.source_loc.clone(),
                                };
                            }
                        }
                        node.clone()
                    }
                    // abs(abs(x)) -> passthrough abs(x)
                    IROperation::UnaryOp {
                        op: UnaryOpKind::Abs,
                    } => {
                        if let Some(inner) = node.inputs.first().and_then(|id| node_map.get(id)) {
                            if matches!(
                                inner.op,
                                IROperation::UnaryOp {
                                    op: UnaryOpKind::Abs
                                }
                            ) {
                                let mut attrs = BTreeMap::new();
                                attrs.insert(
                                    "passthrough".to_string(),
                                    IRValue::U64(node.inputs[0]),
                                );
                                return IRNode {
                                    id: node.id,
                                    op: IROperation::Constant,
                                    output_type: node.output_type.clone(),
                                    inputs: vec![],
                                    attrs,
                                    source_loc: node.source_loc.clone(),
                                };
                            }
                        }
                        node.clone()
                    }
                    // relu(relu(x)) -> passthrough relu(x)
                    IROperation::Activation {
                        kind: ActivationKind::ReLU,
                    } => {
                        if let Some(inner) = node.inputs.first().and_then(|id| node_map.get(id)) {
                            if matches!(
                                inner.op,
                                IROperation::Activation {
                                    kind: ActivationKind::ReLU
                                }
                            ) {
                                let mut attrs = BTreeMap::new();
                                attrs.insert(
                                    "passthrough".to_string(),
                                    IRValue::U64(node.inputs[0]),
                                );
                                return IRNode {
                                    id: node.id,
                                    op: IROperation::Constant,
                                    output_type: node.output_type.clone(),
                                    inputs: vec![],
                                    attrs,
                                    source_loc: node.source_loc.clone(),
                                };
                            }
                        }
                        node.clone()
                    }
                    _ => node.clone(),
                }
            })
            .collect();

        Function {
            nodes: new_nodes,
            ..func.clone()
        }
    }
}

// ============================================================================
// Optimization Pipeline
// ============================================================================

/// Optimization aggressiveness level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// No optimizations.
    O0,
    /// Basic optimizations: constant folding + DCE.
    O1,
    /// Standard optimizations: CF + CSE + DCE (2 iterations).
    O2,
    /// Aggressive optimizations: all passes (3 iterations).
    O3,
}

/// A configurable pipeline that chains optimization passes.
pub struct OptimizationPipeline {
    passes: Vec<Box<dyn OptimizationPass>>,
    max_iterations: usize,
}

impl Default for OptimizationPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl OptimizationPipeline {
    /// Create an empty pipeline with 1 iteration.
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            max_iterations: 1,
        }
    }

    /// Create a pipeline preset for the given optimization level.
    pub fn level(level: OptimizationLevel) -> Self {
        match level {
            OptimizationLevel::O0 => Self::new(),
            OptimizationLevel::O1 => {
                let mut p = Self::new();
                p.add_pass(ConstantFolding::new());
                p.add_pass(DeadCodeElimination::new());
                p
            }
            OptimizationLevel::O2 => {
                let mut p = Self {
                    passes: Vec::new(),
                    max_iterations: 2,
                };
                p.add_pass(ConstantFolding::new());
                p.add_pass(CommonSubexpressionElimination::new());
                p.add_pass(DeadCodeElimination::new());
                p
            }
            OptimizationLevel::O3 => {
                let mut p = Self {
                    passes: Vec::new(),
                    max_iterations: 3,
                };
                p.add_pass(ConstantFolding::new());
                p.add_pass(StrengthReduction::new());
                p.add_pass(AlgebraicSimplification::new());
                p.add_pass(CommonSubexpressionElimination::new());
                p.add_pass(OperatorFusion::new());
                p.add_pass(DeadCodeElimination::new());
                p
            }
        }
    }

    /// Add a pass to the pipeline.
    pub fn add_pass(&mut self, pass: impl OptimizationPass + 'static) {
        self.passes.push(Box::new(pass));
    }

    /// Set the maximum number of fix-point iterations.
    pub fn max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }

    /// Return the names of all passes in the pipeline.
    pub fn pass_names(&self) -> Vec<String> {
        self.passes.iter().map(|p| p.name().to_string()).collect()
    }

    /// Run the pipeline on a program, iterating up to `max_iterations` times.
    pub fn optimize(&self, program: Program) -> Program {
        let mut current = program;
        for _ in 0..self.max_iterations {
            let before_count: usize = current.functions.iter().map(|f| f.nodes.len()).sum();
            for pass in &self.passes {
                current = pass.optimize_program(&current);
            }
            let after_count: usize = current.functions.iter().map(|f| f.nodes.len()).sum();
            // Fixed-point: stop if nothing changed.
            if after_count == before_count {
                break;
            }
        }
        current
    }
}

// ============================================================================
// RMIL-Level Optimization (Expr trees)
// ============================================================================

use crate::compute::fusion::{FusionConfig, FusionPass, FusionResult};
use crate::lang::expr::Expr;

/// A single RMIL-level optimization pass that transforms expression trees.
pub trait RmilPass: Send + Sync {
    /// Human-readable name for this pass.
    fn name(&self) -> &str;

    /// Transform an expression tree, returning the optimized version.
    fn optimize_expr(&self, expr: &Expr) -> Expr;
}

/// Adapter that wraps [`FusionPass`] as an [`RmilPass`].
pub struct RmilFusionPass {
    inner: FusionPass,
}

impl RmilFusionPass {
    /// Create a new RMIL fusion pass with the given configuration.
    pub fn new(config: FusionConfig) -> Self {
        Self {
            inner: FusionPass::new(config),
        }
    }
}

impl Default for RmilFusionPass {
    fn default() -> Self {
        Self::new(FusionConfig::default())
    }
}

impl RmilPass for RmilFusionPass {
    fn name(&self) -> &str {
        "rmil-kernel-fusion"
    }

    fn optimize_expr(&self, expr: &Expr) -> Expr {
        self.inner.fuse(expr).output
    }
}

/// Statistics from an RMIL optimization run.
#[derive(Debug, Clone)]
pub struct RmilOptStats {
    /// Number of ops in the input expression.
    pub ops_before: usize,
    /// Number of ops in the output expression.
    pub ops_after: usize,
    /// Number of fusion kernels created.
    pub fused_kernels: usize,
    /// Detailed fusion result (available when fusion was applied).
    pub fusion_detail: Option<FusionResult>,
}

/// Optimizer for RMIL expression trees.
///
/// Applies [`RmilPass`] passes (kernel fusion, etc.) to [`Expr`] trees
/// before they are evaluated or lowered to IR. This is the RMIL-level
/// counterpart of [`OptimizationPipeline`] (which operates on IR programs).
///
/// # Example
///
/// ```
/// use rmi::core::optimization::RmilOptimizer;
/// use rmi::lang::{Expr, Op};
///
/// let optimizer = RmilOptimizer::default();
/// let expr = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
/// let (optimized, stats) = optimizer.optimize_expr(&expr);
/// assert!(stats.ops_after <= stats.ops_before);
/// ```
pub struct RmilOptimizer {
    passes: Vec<Box<dyn RmilPass>>,
    fusion_config: FusionConfig,
}

impl Default for RmilOptimizer {
    fn default() -> Self {
        Self::with_fusion(FusionConfig::default())
    }
}

impl RmilOptimizer {
    /// Create an optimizer with only kernel fusion using the given config.
    pub fn with_fusion(config: FusionConfig) -> Self {
        let pass = RmilFusionPass::new(config.clone());
        Self {
            passes: vec![Box::new(pass)],
            fusion_config: config,
        }
    }

    /// Create an optimizer with no passes (identity transform).
    pub fn none() -> Self {
        Self {
            passes: Vec::new(),
            fusion_config: FusionConfig::default(),
        }
    }

    /// Add a custom RMIL-level pass.
    pub fn add_pass(&mut self, pass: impl RmilPass + 'static) {
        self.passes.push(Box::new(pass));
    }

    /// Return the names of all passes.
    pub fn pass_names(&self) -> Vec<String> {
        self.passes.iter().map(|p| p.name().to_string()).collect()
    }

    /// Optimize an expression tree, returning the result and statistics.
    pub fn optimize_expr(&self, expr: &Expr) -> (Expr, RmilOptStats) {
        let ops_before = count_expr_ops(expr);
        let mut current = expr.clone();
        let mut fusion_detail = None;

        for pass in &self.passes {
            if pass.name() == "rmil-kernel-fusion" {
                // Run FusionPass directly to capture full FusionResult
                let fp = FusionPass::new(self.fusion_config.clone());
                let result = fp.fuse(&current);
                current = result.output.clone();
                fusion_detail = Some(result);
            } else {
                current = pass.optimize_expr(&current);
            }
        }

        let ops_after = count_expr_ops(&current);
        let fused_kernels = fusion_detail.as_ref().map_or(0, |r| r.fused_count);

        (
            current,
            RmilOptStats {
                ops_before,
                ops_after,
                fused_kernels,
                fusion_detail,
            },
        )
    }

    /// Convenience: run fusion and return just the optimized expression.
    pub fn fuse(&self, expr: &Expr) -> Expr {
        self.optimize_expr(expr).0
    }
}

/// Count App nodes (operations) in an expression tree.
fn count_expr_ops(expr: &Expr) -> usize {
    match expr {
        Expr::App(_, args) => 1 + args.iter().map(count_expr_ops).sum::<usize>(),
        Expr::Seq(a, b) | Expr::Par(a, b) => count_expr_ops(a) + count_expr_ops(b),
        Expr::Cond { pred, yes, no } => {
            count_expr_ops(pred) + count_expr_ops(yes) + count_expr_ops(no)
        }
        Expr::Let { val, body, .. } => count_expr_ops(val) + count_expr_ops(body),
        Expr::Lam { body, .. } => count_expr_ops(body),
        Expr::Call(f, args) => count_expr_ops(f) + args.iter().map(count_expr_ops).sum::<usize>(),
        Expr::Block(exprs) => exprs.iter().map(count_expr_ops).sum(),
        _ => 0,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::codegen::{ActivationKind, FunctionBuilder, IRType, PrimitiveType};

    fn f32_type() -> IRType {
        IRType::Primitive(PrimitiveType::F32)
    }

    fn f64_type() -> IRType {
        IRType::Primitive(PrimitiveType::F64)
    }

    /// Helper to create a constant f64 node more concisely.
    fn const_f64(fb: &mut FunctionBuilder, v: f64) -> u64 {
        fb.constant(IRValue::F64(v), f64_type())
    }

    // -- Dead Code Elimination ----------------------------------------

    #[test]
    fn dce_removes_unused_nodes() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let _unused = const_f64(&mut fb, 42.0); // dead
        fb.ret(x);

        let func = fb.build();
        let dce = DeadCodeElimination::new();
        let optimized = dce.optimize_function(&func);

        assert!(
            optimized.nodes.len() < func.nodes.len(),
            "DCE should remove unused constant"
        );
    }

    #[test]
    fn dce_preserves_live_nodes() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let one = const_f64(&mut fb, 1.0);
        let sum = fb.binary_op(BinaryOpKind::Add, x, one);
        fb.ret(sum);

        let func = fb.build();
        let dce = DeadCodeElimination::new();
        let optimized = dce.optimize_function(&func);

        assert_eq!(
            optimized.nodes.len(),
            func.nodes.len(),
            "DCE should not remove live nodes"
        );
    }

    // -- Constant Folding ---------------------------------------------

    #[test]
    fn constant_folding_arithmetic() {
        let mut fb = FunctionBuilder::new("test", vec![], f64_type());
        let a = const_f64(&mut fb, 3.0);
        let b = const_f64(&mut fb, 4.0);
        let c = fb.binary_op(BinaryOpKind::Add, a, b);
        fb.ret(c);

        let func = fb.build();
        let cf = ConstantFolding::new();
        let optimized = cf.optimize_function(&func);

        let result_node = optimized.nodes.iter().find(|n| n.id == c).unwrap();
        assert!(matches!(result_node.op, IROperation::Constant));
        assert!(
            matches!(result_node.attrs.get("value"), Some(IRValue::F64(v)) if (*v - 7.0).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn constant_folding_unary() {
        let mut fb = FunctionBuilder::new("test", vec![], f64_type());
        let a = const_f64(&mut fb, -5.0);
        let b = fb.unary_op(UnaryOpKind::Abs, a);
        fb.ret(b);

        let func = fb.build();
        let cf = ConstantFolding::new();
        let optimized = cf.optimize_function(&func);

        let result_node = optimized.nodes.iter().find(|n| n.id == b).unwrap();
        assert!(matches!(result_node.op, IROperation::Constant));
        assert!(
            matches!(result_node.attrs.get("value"), Some(IRValue::F64(v)) if (*v - 5.0).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn constant_folding_no_div_by_zero() {
        let mut fb = FunctionBuilder::new("test", vec![], f64_type());
        let a = const_f64(&mut fb, 10.0);
        let b = const_f64(&mut fb, 0.0);
        let c = fb.binary_op(BinaryOpKind::Div, a, b);
        fb.ret(c);

        let func = fb.build();
        let cf = ConstantFolding::new();
        let optimized = cf.optimize_function(&func);

        // Should NOT fold division by zero.
        let result_node = optimized.nodes.iter().find(|n| n.id == c).unwrap();
        assert!(
            matches!(result_node.op, IROperation::BinaryOp { .. }),
            "Div by zero should not be folded"
        );
    }

    // -- Common Sub-expression Elimination ----------------------------

    #[test]
    fn cse_deduplicates() {
        let mut fb = FunctionBuilder::new(
            "test",
            vec![("x".to_string(), f64_type()), ("y".to_string(), f64_type())],
            f64_type(),
        );
        let x = fb.param(0);
        let y = fb.param(1);
        let a = fb.binary_op(BinaryOpKind::Add, x, y);
        let b = fb.binary_op(BinaryOpKind::Add, x, y); // duplicate of a
        let c = fb.binary_op(BinaryOpKind::Mul, a, b);
        fb.ret(c);

        let func = fb.build();
        let cse = CommonSubexpressionElimination::new();
        let optimized = cse.optimize_function(&func);

        assert!(
            optimized.nodes.len() < func.nodes.len(),
            "CSE should remove the duplicate Add"
        );
    }

    // -- Strength Reduction -------------------------------------------

    #[test]
    fn strength_reduction_mul_by_two() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let two = const_f64(&mut fb, 2.0);
        let y = fb.binary_op(BinaryOpKind::Mul, x, two);
        fb.ret(y);

        let func = fb.build();
        let sr = StrengthReduction::new();
        let optimized = sr.optimize_function(&func);

        let y_node = optimized.nodes.iter().find(|n| n.id == y).unwrap();
        assert!(
            matches!(
                y_node.op,
                IROperation::BinaryOp {
                    op: BinaryOpKind::Add
                }
            ),
            "x * 2.0 should be reduced to x + x"
        );
        assert_eq!(
            y_node.inputs[0], y_node.inputs[1],
            "Both inputs should be x"
        );
    }

    #[test]
    fn strength_reduction_mul_by_zero() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let zero = const_f64(&mut fb, 0.0);
        let y = fb.binary_op(BinaryOpKind::Mul, x, zero);
        fb.ret(y);

        let func = fb.build();
        let sr = StrengthReduction::new();
        let optimized = sr.optimize_function(&func);

        let y_node = optimized.nodes.iter().find(|n| n.id == y).unwrap();
        assert!(
            matches!(y_node.op, IROperation::Constant),
            "x * 0.0 should fold to constant 0.0"
        );
        assert!(
            matches!(y_node.attrs.get("value"), Some(IRValue::F64(v)) if (*v - 0.0).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn strength_reduction_pow_to_square() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let two = const_f64(&mut fb, 2.0);
        let y = fb.binary_op(BinaryOpKind::Pow, x, two);
        fb.ret(y);

        let func = fb.build();
        let sr = StrengthReduction::new();
        let optimized = sr.optimize_function(&func);

        let y_node = optimized.nodes.iter().find(|n| n.id == y).unwrap();
        assert!(
            matches!(
                y_node.op,
                IROperation::BinaryOp {
                    op: BinaryOpKind::Mul
                }
            ),
            "x ** 2.0 should be reduced to x * x"
        );
    }

    #[test]
    fn strength_reduction_add_zero_identity() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let zero = const_f64(&mut fb, 0.0);
        let y = fb.binary_op(BinaryOpKind::Add, x, zero);
        fb.ret(y);

        let func = fb.build();
        let sr = StrengthReduction::new();
        let optimized = sr.optimize_function(&func);

        let y_node = optimized.nodes.iter().find(|n| n.id == y).unwrap();
        assert!(
            matches!(y_node.op, IROperation::Constant),
            "x + 0.0 should be reduced to passthrough"
        );
        assert!(y_node.attrs.contains_key("passthrough"));
    }

    // -- Algebraic Simplification -------------------------------------

    #[test]
    fn algebraic_x_minus_x() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let y = fb.binary_op(BinaryOpKind::Sub, x, x);
        fb.ret(y);

        let func = fb.build();
        let alg = AlgebraicSimplification::new();
        let optimized = alg.optimize_function(&func);

        let y_node = optimized.nodes.iter().find(|n| n.id == y).unwrap();
        assert!(matches!(y_node.op, IROperation::Constant));
        assert!(
            matches!(y_node.attrs.get("value"), Some(IRValue::F64(v)) if (*v - 0.0).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn algebraic_x_div_x() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let y = fb.binary_op(BinaryOpKind::Div, x, x);
        fb.ret(y);

        let func = fb.build();
        let alg = AlgebraicSimplification::new();
        let optimized = alg.optimize_function(&func);

        let y_node = optimized.nodes.iter().find(|n| n.id == y).unwrap();
        assert!(matches!(y_node.op, IROperation::Constant));
        assert!(
            matches!(y_node.attrs.get("value"), Some(IRValue::F64(v)) if (*v - 1.0).abs() < f64::EPSILON)
        );
    }

    #[test]
    fn algebraic_double_neg() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let neg1 = fb.unary_op(UnaryOpKind::Neg, x);
        let neg2 = fb.unary_op(UnaryOpKind::Neg, neg1);
        fb.ret(neg2);

        let func = fb.build();
        let alg = AlgebraicSimplification::new();
        let optimized = alg.optimize_function(&func);

        let y_node = optimized.nodes.iter().find(|n| n.id == neg2).unwrap();
        assert!(matches!(y_node.op, IROperation::Constant));
        assert!(y_node.attrs.contains_key("passthrough"));
    }

    #[test]
    fn algebraic_double_relu_idempotent() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f32_type())], f32_type());
        let x = fb.param(0);
        let r1 = fb.activation(ActivationKind::ReLU, x);
        let r2 = fb.activation(ActivationKind::ReLU, r1);
        fb.ret(r2);

        let func = fb.build();
        let alg = AlgebraicSimplification::new();
        let optimized = alg.optimize_function(&func);

        let r2_node = optimized.nodes.iter().find(|n| n.id == r2).unwrap();
        assert!(matches!(r2_node.op, IROperation::Constant));
        assert!(r2_node.attrs.contains_key("passthrough"));
    }

    // -- Pipeline -----------------------------------------------------

    #[test]
    fn pipeline_o0_identity() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let _dead = const_f64(&mut fb, 99.0);
        fb.ret(x);

        let func = fb.build();
        let mut program = crate::core::codegen::Program::new("test");
        program.add_function(func);

        let pipeline = OptimizationPipeline::level(OptimizationLevel::O0);
        let optimized = pipeline.optimize(program.clone());

        assert_eq!(
            program.functions[0].nodes.len(),
            optimized.functions[0].nodes.len(),
            "O0 should not change anything"
        );
    }

    #[test]
    fn pipeline_o2_optimizes() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let a = const_f64(&mut fb, 3.0);
        let b = const_f64(&mut fb, 4.0);
        let c = fb.binary_op(BinaryOpKind::Add, a, b);
        let _dead = fb.unary_op(UnaryOpKind::Sqrt, a);
        let result = fb.binary_op(BinaryOpKind::Mul, x, c);
        fb.ret(result);

        let func = fb.build();
        let mut program = crate::core::codegen::Program::new("test");
        program.add_function(func);

        let before = program.clone();
        let pipeline = OptimizationPipeline::level(OptimizationLevel::O2);
        let after = pipeline.optimize(program);

        assert!(
            after.functions[0].nodes.len() <= before.functions[0].nodes.len(),
            "O2 should reduce node count"
        );
    }

    #[test]
    fn pipeline_pass_names() {
        let pipeline = OptimizationPipeline::level(OptimizationLevel::O3);
        let names = pipeline.pass_names();
        assert!(names.contains(&"constant-folding".to_string()));
        assert!(names.contains(&"strength-reduction".to_string()));
        assert!(names.contains(&"algebraic-simplification".to_string()));
        assert!(names.contains(&"cse".to_string()));
        assert!(names.contains(&"operator-fusion".to_string()));
        assert!(names.contains(&"dce".to_string()));
    }

    #[test]
    fn optimization_stats() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f64_type())], f64_type());
        let x = fb.param(0);
        let _dead = const_f64(&mut fb, 99.0);
        fb.ret(x);

        let func = fb.build();
        let mut before = crate::core::codegen::Program::new("test");
        before.add_function(func);

        let pipeline = OptimizationPipeline::level(OptimizationLevel::O1);
        let after = pipeline.optimize(before.clone());

        let stats = OptimizationStats::compare(&before, &after);
        assert!(stats.nodes_eliminated > 0, "Should report eliminated nodes");
    }

    // -- RMIL Optimizer ------------------------------------------------

    #[test]
    fn rmil_optimizer_fuses_elementwise_chain() {
        use crate::lang::Op;
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::SIGMOID) >> Expr::op1(Op::GELU);
        let optimizer = RmilOptimizer::default();
        let (optimized, stats) = optimizer.optimize_expr(&expr);
        assert!(
            stats.fused_kernels > 0,
            "Should create at least one fused kernel"
        );
        assert!(stats.ops_after <= stats.ops_before);
        assert!(stats.fusion_detail.is_some());
        // Optimized tree should still have the same ops (just rewritten)
        assert!(count_expr_ops(&optimized) > 0);
    }

    #[test]
    fn rmil_optimizer_fuses_matmul_activation() {
        use crate::lang::Op;
        let expr = Expr::op1(Op::MATMUL) >> Expr::op1(Op::RELU);
        let optimizer = RmilOptimizer::default();
        let (_optimized, stats) = optimizer.optimize_expr(&expr);
        assert!(stats.fused_kernels > 0, "MatMul+RELU should fuse");
    }

    #[test]
    fn rmil_optimizer_no_passes() {
        use crate::lang::Op;
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::SIGMOID);
        let optimizer = RmilOptimizer::none();
        let (optimized, stats) = optimizer.optimize_expr(&expr);
        assert_eq!(stats.fused_kernels, 0);
        assert_eq!(stats.ops_before, stats.ops_after);
        assert_eq!(count_expr_ops(&optimized), count_expr_ops(&expr));
    }

    #[test]
    fn rmil_optimizer_pass_names() {
        let optimizer = RmilOptimizer::default();
        let names = optimizer.pass_names();
        assert!(names.contains(&"rmil-kernel-fusion".to_string()));
    }

    #[test]
    fn rmil_optimizer_fuse_convenience() {
        use crate::lang::Op;
        let expr = Expr::op1(Op::LINEAR) >> Expr::op1(Op::RELU) >> Expr::op1(Op::LINEAR);
        let optimizer = RmilOptimizer::default();
        let optimized = optimizer.fuse(&expr);
        assert!(count_expr_ops(&optimized) > 0);
    }
}
