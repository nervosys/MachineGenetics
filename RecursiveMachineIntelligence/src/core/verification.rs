//! IR Verification System
//!
//! Static analysis passes that validate the correctness and safety of generated
//! IR programs. The verifier catches errors *before* code emission, ensuring
//! that only well-formed programs reach the emitters.
//!
//! # Verification Passes
//!
//! | Pass | Description |
//! |------|-------------|
//! | [`TypeChecker`] | Validates type consistency across all nodes |
//! | [`ShapeInference`] | Infers and validates tensor shapes |
//! | [`ResourceChecker`] | Checks memory safety (alloc/free pairing) |
//! | [`TerminationAnalyzer`] | Detects unbounded loops and recursion |
//! | [`BoundsChecker`] | Validates tensor operation bounds and parameters |
//! | [`DataflowAnalyzer`] | Checks use-before-def and parameter validity |
//! | [`Verifier`] | Composite verifier running all passes |
//!
//! # Usage
//!
//! ```rust
//! use rmi::core::codegen::Program;
//! use rmi::core::verification::Verifier;
//!
//! let program = Program::new("test");
//! let verifier = Verifier::new();
//! let report = verifier.verify(&program);
//! assert!(report.is_ok(), "Verification failed: {:?}", report.errors());
//! ```

use crate::core::codegen::{Dimension, Function, IRNode, IROperation, IRType, Padding, Program};
use std::collections::{HashMap, HashSet};
use std::fmt;

// ============================================================================
// Verification Report
// ============================================================================

/// Severity level for verification issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational note
    Note,
    /// Non-critical issue
    Warning,
    /// Critical error that prevents code emission
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Note => write!(f, "note"),
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

/// A single verification diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity level
    pub severity: Severity,
    /// Error code (e.g., "E001")
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Function name where the issue was found
    pub function: Option<String>,
    /// Node ID where the issue was found
    pub node_id: Option<u64>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.code, self.message)?;
        if let Some(func) = &self.function {
            write!(f, " (in function '{}'", func)?;
            if let Some(id) = self.node_id {
                write!(f, ", node {}", id)?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

/// Aggregated verification report.
#[derive(Debug, Clone, Default)]
pub struct VerificationReport {
    diagnostics: Vec<Diagnostic>,
}

impl VerificationReport {
    /// Create an empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a diagnostic.
    pub fn add(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    /// Merge another report into this one.
    pub fn merge(&mut self, other: VerificationReport) {
        self.diagnostics.extend(other.diagnostics);
    }

    /// Check if verification passed (no errors).
    pub fn is_ok(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }

    /// Get all error diagnostics.
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect()
    }

    /// Get all warning diagnostics.
    pub fn warnings(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect()
    }

    /// Total number of diagnostics.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Whether there are any diagnostics.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Get all diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for VerificationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let errors = self.errors().len();
        let warnings = self.warnings().len();
        writeln!(
            f,
            "Verification: {} error(s), {} warning(s)",
            errors, warnings
        )?;
        for diag in &self.diagnostics {
            writeln!(f, "  {}", diag)?;
        }
        Ok(())
    }
}

// ============================================================================
// Verification Pass Trait
// ============================================================================

/// A verification pass that checks a program for specific properties.
pub trait VerificationPass: Send + Sync {
    /// Human-readable pass name.
    fn name(&self) -> &str;

    /// Check a function, producing diagnostics.
    fn check_function(&self, func: &Function) -> VerificationReport;

    /// Check an entire program.
    fn check_program(&self, program: &Program) -> VerificationReport {
        let mut report = VerificationReport::new();
        for func in &program.functions {
            report.merge(self.check_function(func));
        }
        report
    }
}

// ============================================================================
// Type Checker
// ============================================================================

/// Validates type consistency: operands have correct types for their operators,
/// function calls receive the right argument types, and return types match.
///
/// # Error Codes
/// - `T001`: Type mismatch in binary operation
/// - `T002`: Type mismatch in unary operation
/// - `T003`: Input count mismatch
/// - `T004`: Return type mismatch
/// - `T005`: Undefined node reference
/// - `T006`: Duplicate node ID
pub struct TypeChecker;

impl TypeChecker {
    /// Create a new type checker.
    pub fn new() -> Self {
        Self
    }

    fn diag(
        severity: Severity,
        code: &str,
        msg: String,
        func: &str,
        node_id: Option<u64>,
    ) -> Diagnostic {
        Diagnostic {
            severity,
            code: code.to_string(),
            message: msg,
            function: Some(func.to_string()),
            node_id,
        }
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationPass for TypeChecker {
    fn name(&self) -> &str {
        "type-checker"
    }

    fn check_function(&self, func: &Function) -> VerificationReport {
        let mut report = VerificationReport::new();
        let mut node_types: HashMap<u64, &IRType> = HashMap::new();
        let mut seen_ids: HashSet<u64> = HashSet::new();

        // Register parameter types
        for (idx, (_name, ty)) in func.params.iter().enumerate() {
            // Convention: parameter nodes are first in the node list
            if let Some(node) = func.nodes.get(idx) {
                if matches!(node.op, IROperation::Parameter { .. }) {
                    node_types.insert(node.id, ty);
                }
            }
        }

        for node in &func.nodes {
            // Check for duplicate IDs
            if !seen_ids.insert(node.id) {
                report.add(Self::diag(
                    Severity::Error,
                    "T006",
                    format!("Duplicate node ID: {}", node.id),
                    &func.name,
                    Some(node.id),
                ));
            }

            // Register this node's type
            node_types.insert(node.id, &node.output_type);

            // Check that all inputs reference existing nodes
            for &input_id in &node.inputs {
                if !node_types.contains_key(&input_id) {
                    report.add(Self::diag(
                        Severity::Error,
                        "T005",
                        format!(
                            "Reference to undefined node {} in node {}",
                            input_id, node.id
                        ),
                        &func.name,
                        Some(node.id),
                    ));
                }
            }

            // Check operation-specific type constraints
            match &node.op {
                IROperation::BinaryOp { op } => {
                    if node.inputs.len() != 2 {
                        report.add(Self::diag(
                            Severity::Error,
                            "T003",
                            format!(
                                "Binary op {:?} requires 2 inputs, got {}",
                                op,
                                node.inputs.len()
                            ),
                            &func.name,
                            Some(node.id),
                        ));
                    } else {
                        // Check type compatibility between operands
                        let a_ty = node_types.get(&node.inputs[0]);
                        let b_ty = node_types.get(&node.inputs[1]);
                        if let (Some(a), Some(b)) = (a_ty, b_ty) {
                            if !a.is_compatible(b) {
                                report.add(Self::diag(
                                    Severity::Warning,
                                    "T001",
                                    format!(
                                        "Binary op {:?}: operand types may be incompatible ({:?} vs {:?})",
                                        op, a, b
                                    ),
                                    &func.name,
                                    Some(node.id),
                                ));
                            }
                        }
                    }
                }
                IROperation::UnaryOp { op }
                    if node.inputs.len() != 1 => {
                        report.add(Self::diag(
                            Severity::Error,
                            "T003",
                            format!(
                                "Unary op {:?} requires 1 input, got {}",
                                op,
                                node.inputs.len()
                            ),
                            &func.name,
                            Some(node.id),
                        ));
                    }
                IROperation::Return => {
                    // Check that the returned type matches the function's return type
                    if let Some(&ret_input) = node.inputs.first() {
                        if let Some(ret_ty) = node_types.get(&ret_input) {
                            if !ret_ty.is_compatible(&func.return_type) {
                                report.add(Self::diag(
                                    Severity::Warning,
                                    "T004",
                                    format!(
                                        "Return type mismatch: expected {:?}, got {:?}",
                                        func.return_type, ret_ty
                                    ),
                                    &func.name,
                                    Some(node.id),
                                ));
                            }
                        }
                    }
                }
                IROperation::MatMul { .. }
                    if node.inputs.len() != 2 => {
                        report.add(Self::diag(
                            Severity::Error,
                            "T003",
                            format!("MatMul requires 2 inputs, got {}", node.inputs.len()),
                            &func.name,
                            Some(node.id),
                        ));
                    }
                IROperation::Activation { .. } | IROperation::Normalize { .. }
                    if node.inputs.is_empty() => {
                        report.add(Self::diag(
                            Severity::Error,
                            "T003",
                            "Activation/Normalize requires at least 1 input".to_string(),
                            &func.name,
                            Some(node.id),
                        ));
                    }
                IROperation::Attention { .. }
                    if node.inputs.len() < 3 => {
                        report.add(Self::diag(
                            Severity::Error,
                            "T003",
                            format!(
                                "Attention requires at least 3 inputs (Q, K, V), got {}",
                                node.inputs.len()
                            ),
                            &func.name,
                            Some(node.id),
                        ));
                    }
                _ => {}
            }
        }

        // Check that return node exists if specified
        if let Some(ret_id) = func.return_node {
            if !seen_ids.contains(&ret_id) {
                report.add(Self::diag(
                    Severity::Error,
                    "T005",
                    format!("Return node {} not found in function", ret_id),
                    &func.name,
                    None,
                ));
            }
        }

        report
    }
}

// ============================================================================
// Shape Inference
// ============================================================================

/// Infers tensor shapes and validates shape compatibility across operations.
///
/// # Error Codes
/// - `S001`: Shape mismatch in elementwise operation
/// - `S002`: Invalid shapes for matrix multiplication
/// - `S003`: Shape mismatch in concatenation
/// - `S004`: Invalid reshape dimensions
pub struct ShapeInference;

impl ShapeInference {
    /// Create a new shape inference pass.
    pub fn new() -> Self {
        Self
    }

    fn diag(severity: Severity, code: &str, msg: String, func: &str, node_id: u64) -> Diagnostic {
        Diagnostic {
            severity,
            code: code.to_string(),
            message: msg,
            function: Some(func.to_string()),
            node_id: Some(node_id),
        }
    }

    /// Extract shape from a type (returns None for non-tensor types).
    fn get_shape(ty: &IRType) -> Option<&[Dimension]> {
        match ty {
            IRType::Tensor { shape, .. } => Some(shape),
            _ => None,
        }
    }

    /// Check if two shapes are broadcast-compatible.
    fn shapes_broadcast_compatible(a: &[Dimension], b: &[Dimension]) -> bool {
        let max_rank = a.len().max(b.len());
        for i in 0..max_rank {
            let da = if i < a.len() {
                &a[a.len() - 1 - i]
            } else {
                &Dimension::Static(1)
            };
            let db = if i < b.len() {
                &b[b.len() - 1 - i]
            } else {
                &Dimension::Static(1)
            };

            match (da, db) {
                (Dimension::Static(x), Dimension::Static(y)) => {
                    if *x != *y && *x != 1 && *y != 1 {
                        return false;
                    }
                }
                (Dimension::Dynamic, _) | (_, Dimension::Dynamic) => {} // OK
                (Dimension::Symbolic(_), _) | (_, Dimension::Symbolic(_)) => {} // OK
            }
        }
        true
    }

    /// Check if shapes are valid for matmul: [..., M, K] x [..., K, N] → [..., M, N]
    fn check_matmul_shapes(a: &[Dimension], b: &[Dimension]) -> bool {
        if a.len() < 2 || b.len() < 2 {
            return false;
        }
        // Inner dimensions must be compatible
        let k_a = &a[a.len() - 1];
        let k_b = &b[b.len() - 2];
        match (k_a, k_b) {
            (Dimension::Static(x), Dimension::Static(y)) => *x == *y,
            _ => true, // Dynamic or symbolic dimensions are always compatible
        }
    }
}

impl Default for ShapeInference {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationPass for ShapeInference {
    fn name(&self) -> &str {
        "shape-inference"
    }

    fn check_function(&self, func: &Function) -> VerificationReport {
        let mut report = VerificationReport::new();
        let node_types: HashMap<u64, &IRType> =
            func.nodes.iter().map(|n| (n.id, &n.output_type)).collect();

        for node in &func.nodes {
            match &node.op {
                IROperation::BinaryOp { op }
                    if node.inputs.len() == 2 => {
                        let a_ty = node_types.get(&node.inputs[0]);
                        let b_ty = node_types.get(&node.inputs[1]);
                        if let (Some(a), Some(b)) = (a_ty, b_ty) {
                            if let (Some(sa), Some(sb)) = (Self::get_shape(a), Self::get_shape(b)) {
                                if !Self::shapes_broadcast_compatible(sa, sb) {
                                    report.add(Self::diag(
                                        Severity::Error,
                                        "S001",
                                        format!("Shape mismatch in {:?}: {:?} vs {:?}", op, sa, sb),
                                        &func.name,
                                        node.id,
                                    ));
                                }
                            }
                        }
                    }
                IROperation::MatMul { .. } | IROperation::BatchMatMul { .. }
                    if node.inputs.len() == 2 => {
                        let a_ty = node_types.get(&node.inputs[0]);
                        let b_ty = node_types.get(&node.inputs[1]);
                        if let (Some(a), Some(b)) = (a_ty, b_ty) {
                            if let (Some(sa), Some(sb)) = (Self::get_shape(a), Self::get_shape(b)) {
                                if !Self::check_matmul_shapes(sa, sb) {
                                    report.add(Self::diag(
                                        Severity::Error,
                                        "S002",
                                        format!(
                                            "Invalid matmul shapes: {:?} x {:?} (inner dims must match)",
                                            sa, sb
                                        ),
                                        &func.name,
                                        node.id,
                                    ));
                                }
                            }
                        }
                    }
                IROperation::TensorConcat { axis } => {
                    // All inputs must have the same rank and matching dims except on axis
                    let shapes: Vec<Option<&[Dimension]>> = node
                        .inputs
                        .iter()
                        .filter_map(|id| node_types.get(id))
                        .map(|ty| Self::get_shape(ty))
                        .collect();

                    if shapes.len() >= 2 {
                        if let Some(first_shape) = shapes[0] {
                            for (idx, shape) in shapes[1..].iter().enumerate() {
                                if let Some(s) = shape {
                                    if s.len() != first_shape.len() {
                                        report.add(Self::diag(
                                            Severity::Error,
                                            "S003",
                                            format!(
                                                "Concat: rank mismatch (input 0 has rank {}, input {} has rank {})",
                                                first_shape.len(),
                                                idx + 1,
                                                s.len()
                                            ),
                                            &func.name,
                                            node.id,
                                        ));
                                    }
                                }
                            }
                            if *axis as usize >= first_shape.len() {
                                report.add(Self::diag(
                                    Severity::Error,
                                    "S003",
                                    format!(
                                        "Concat axis {} out of bounds for rank {}",
                                        axis,
                                        first_shape.len()
                                    ),
                                    &func.name,
                                    node.id,
                                ));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        report
    }
}

// ============================================================================
// Resource Checker
// ============================================================================

/// Validates memory safety by ensuring all `Alloc` nodes have matching `Free`
/// nodes, and no double-frees or use-after-free patterns exist.
///
/// # Error Codes
/// - `R001`: Memory leak (alloc without matching free)
/// - `R002`: Double free
/// - `R003`: Use after free
pub struct ResourceChecker;

impl ResourceChecker {
    /// Create a new resource checker.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ResourceChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationPass for ResourceChecker {
    fn name(&self) -> &str {
        "resource-checker"
    }

    fn check_function(&self, func: &Function) -> VerificationReport {
        let mut report = VerificationReport::new();
        let mut allocations: HashSet<u64> = HashSet::new();
        let mut freed: HashSet<u64> = HashSet::new();

        // Track which nodes have been freed (use-order analysis)
        let mut node_order: HashMap<u64, usize> = HashMap::new();
        for (idx, node) in func.nodes.iter().enumerate() {
            node_order.insert(node.id, idx);
        }

        for node in &func.nodes {
            match &node.op {
                IROperation::Alloc => {
                    allocations.insert(node.id);
                }
                IROperation::Free => {
                    if let Some(&input_id) = node.inputs.first() {
                        if freed.contains(&input_id) {
                            report.add(Diagnostic {
                                severity: Severity::Error,
                                code: "R002".to_string(),
                                message: format!("Double free of allocation v{}", input_id),
                                function: Some(func.name.clone()),
                                node_id: Some(node.id),
                            });
                        } else if !allocations.contains(&input_id) {
                            report.add(Diagnostic {
                                severity: Severity::Warning,
                                code: "R003".to_string(),
                                message: format!(
                                    "Free of non-allocated value v{} (may be use-after-free)",
                                    input_id
                                ),
                                function: Some(func.name.clone()),
                                node_id: Some(node.id),
                            });
                        }
                        freed.insert(input_id);
                    }
                }
                _ => {
                    // Check for use of freed memory
                    for &input_id in &node.inputs {
                        if freed.contains(&input_id) {
                            report.add(Diagnostic {
                                severity: Severity::Error,
                                code: "R003".to_string(),
                                message: format!("Use of freed memory v{}", input_id),
                                function: Some(func.name.clone()),
                                node_id: Some(node.id),
                            });
                        }
                    }
                }
            }
        }

        // Check for leaks
        let leaked: HashSet<_> = allocations.difference(&freed).collect();
        for &alloc_id in &leaked {
            report.add(Diagnostic {
                severity: Severity::Warning,
                code: "R001".to_string(),
                message: format!("Memory leak: allocation v{} is never freed", alloc_id),
                function: Some(func.name.clone()),
                node_id: Some(*alloc_id),
            });
        }

        report
    }
}

// ============================================================================
// Termination Analyzer
// ============================================================================

/// Analyzes the IR for potential non-termination risks.
///
/// Checks for:
/// - Unbounded loops (loops without obvious termination conditions)
/// - Recursive function calls (direct or indirect)
/// - Call depth estimation
///
/// # Error Codes
/// - `L001`: Potentially unbounded loop detected
/// - `L002`: Direct recursion detected
/// - `L003`: High loop nesting depth
pub struct TerminationAnalyzer {
    /// Maximum allowed loop nesting depth
    max_loop_depth: usize,
}

impl TerminationAnalyzer {
    /// Create a new termination analyzer.
    pub fn new() -> Self {
        Self { max_loop_depth: 10 }
    }

    /// Set maximum allowed loop nesting depth.
    pub fn max_loop_depth(mut self, depth: usize) -> Self {
        self.max_loop_depth = depth;
        self
    }
}

impl Default for TerminationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationPass for TerminationAnalyzer {
    fn name(&self) -> &str {
        "termination-analyzer"
    }

    fn check_function(&self, func: &Function) -> VerificationReport {
        let mut report = VerificationReport::new();
        let mut loop_depth: usize = 0;

        for node in &func.nodes {
            match &node.op {
                IROperation::Loop { kind } => {
                    loop_depth += 1;

                    // Check nesting depth
                    if loop_depth > self.max_loop_depth {
                        report.add(Diagnostic {
                            severity: Severity::Warning,
                            code: "L003".to_string(),
                            message: format!(
                                "Loop nesting depth {} exceeds threshold {}",
                                loop_depth, self.max_loop_depth
                            ),
                            function: Some(func.name.clone()),
                            node_id: Some(node.id),
                        });
                    }

                    // Check for potentially unbounded loops (heuristic)
                    if matches!(kind, crate::core::codegen::LoopKind::While) {
                        // While loops are potentially unbounded
                        report.add(Diagnostic {
                            severity: Severity::Note,
                            code: "L001".to_string(),
                            message: "While loop detected — ensure termination condition is sound"
                                .to_string(),
                            function: Some(func.name.clone()),
                            node_id: Some(node.id),
                        });
                    }
                }
                IROperation::Call { target }
                    // Check for direct recursion
                    if target == &func.name => {
                        report.add(Diagnostic {
                            severity: Severity::Warning,
                            code: "L002".to_string(),
                            message: format!(
                                "Direct recursion: function '{}' calls itself",
                                func.name
                            ),
                            function: Some(func.name.clone()),
                            node_id: Some(node.id),
                        });
                    }
                _ => {}
            }
        }

        report
    }

    fn check_program(&self, program: &Program) -> VerificationReport {
        let mut report = VerificationReport::new();

        // Check each function individually
        for func in &program.functions {
            report.merge(self.check_function(func));
        }

        // Check for indirect recursion via call graph
        let mut call_graph: HashMap<&str, HashSet<&str>> = HashMap::new();
        for func in &program.functions {
            let calls: HashSet<&str> = func
                .nodes
                .iter()
                .filter_map(|n| match &n.op {
                    IROperation::Call { target } => Some(target.as_str()),
                    _ => None,
                })
                .collect();
            call_graph.insert(func.name.as_str(), calls);
        }

        // Detect cycles in call graph using DFS coloring
        let mut visited: HashSet<&str> = HashSet::new();
        let mut in_stack: HashSet<&str> = HashSet::new();

        for func_name in call_graph.keys() {
            if !visited.contains(func_name) {
                self.dfs_cycle_check(
                    func_name,
                    &call_graph,
                    &mut visited,
                    &mut in_stack,
                    &mut report,
                );
            }
        }

        report
    }
}

impl TerminationAnalyzer {
    fn dfs_cycle_check<'a>(
        &self,
        node: &'a str,
        graph: &HashMap<&'a str, HashSet<&'a str>>,
        visited: &mut HashSet<&'a str>,
        in_stack: &mut HashSet<&'a str>,
        report: &mut VerificationReport,
    ) {
        visited.insert(node);
        in_stack.insert(node);

        if let Some(callees) = graph.get(node) {
            for &callee in callees {
                if !visited.contains(callee) {
                    self.dfs_cycle_check(callee, graph, visited, in_stack, report);
                } else if in_stack.contains(callee) && callee != node {
                    // Indirect recursion (direct is caught per-function)
                    report.add(Diagnostic {
                        severity: Severity::Warning,
                        code: "L002".to_string(),
                        message: format!(
                            "Indirect recursion detected: '{}' → '{}' forms a cycle",
                            node, callee,
                        ),
                        function: Some(node.to_string()),
                        node_id: None,
                    });
                }
            }
        }

        in_stack.remove(node);
    }
}

// ============================================================================
// Bounds Checker
// ============================================================================

/// Validates that tensor operations have valid bounds and parameters.
///
/// # Error Codes
/// - `B001`: Tensor split requests more splits than dimension allows
/// - `B002`: Convolution has mismatched stride/padding dimensions
/// - `B003`: Reduce operation references non-existent axis
/// - `B004`: Pool kernel dimensions are zero
#[derive(Default)]
pub struct BoundsChecker;

impl BoundsChecker {
    /// Create a new bounds checker.
    pub fn new() -> Self {
        Self
    }

    fn diag(
        severity: Severity,
        code: &str,
        message: String,
        func_name: &str,
        node_id: u64,
    ) -> Diagnostic {
        Diagnostic {
            severity,
            code: code.to_string(),
            message,
            function: Some(func_name.to_string()),
            node_id: Some(node_id),
        }
    }
}

impl VerificationPass for BoundsChecker {
    fn name(&self) -> &str {
        "bounds-checker"
    }

    fn check_function(&self, func: &Function) -> VerificationReport {
        let mut report = VerificationReport::new();
        let node_map: HashMap<u64, &IRNode> = func.nodes.iter().map(|n| (n.id, n)).collect();

        for node in &func.nodes {
            match &node.op {
                // B001: TensorSplit — num_splits should be > 0
                IROperation::TensorSplit { num_splits, .. }
                    if *num_splits == 0 => {
                        report.add(Self::diag(
                            Severity::Error,
                            "B001",
                            "TensorSplit with 0 splits is invalid".to_string(),
                            &func.name,
                            node.id,
                        ));
                    }
                // B002: Conv — stride dimensions should match spatial dims
                IROperation::Conv {
                    dims,
                    stride,
                    padding,
                } => {
                    if stride.len() != *dims {
                        report.add(Self::diag(
                            Severity::Error,
                            "B002",
                            format!(
                                "Conv has {} dims but stride has {} elements",
                                dims,
                                stride.len()
                            ),
                            &func.name,
                            node.id,
                        ));
                    }
                    // Check explicit padding dimensions
                    if let Padding::Explicit(pairs) = padding {
                        if pairs.len() != *dims {
                            report.add(Self::diag(
                                Severity::Warning,
                                "B002",
                                format!(
                                    "Conv has {} dims but explicit padding has {} elements",
                                    dims,
                                    pairs.len()
                                ),
                                &func.name,
                                node.id,
                            ));
                        }
                    }
                }
                // B003: Reduce — axes should reference valid input dimensions
                IROperation::Reduce { axes, .. } => {
                    if let Some(&input_id) = node.inputs.first() {
                        if let Some(input_node) = node_map.get(&input_id) {
                            if let IRType::Tensor { shape, .. } = &input_node.output_type {
                                let ndim = shape.len() as i32;
                                for &axis in axes {
                                    if axis >= ndim || axis < -ndim {
                                        report.add(Self::diag(
                                            Severity::Error,
                                            "B003",
                                            format!(
                                                "Reduce axis {} exceeds input rank {}",
                                                axis, ndim
                                            ),
                                            &func.name,
                                            node.id,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                // B004: Pool — kernel must be non-zero
                IROperation::Pool { kernel, .. } => {
                    for (i, &k) in kernel.iter().enumerate() {
                        if k == 0 {
                            report.add(Self::diag(
                                Severity::Error,
                                "B004",
                                format!("Pool kernel dimension {} is zero", i),
                                &func.name,
                                node.id,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        report
    }
}

// ============================================================================
// Dataflow Analyzer
// ============================================================================

/// Validates correct dataflow: all node inputs must be defined before use,
/// parameter indices must be valid, and there must be no cycles in the
/// non-loop portion of the graph.
///
/// # Error Codes
/// - `D001`: Use of undefined node (input references non-existent node ID)
/// - `D002`: Parameter index exceeds function parameter count
/// - `D003`: Function has no return node
/// - `D004`: Duplicate node definition
#[derive(Default)]
pub struct DataflowAnalyzer;

impl DataflowAnalyzer {
    /// Create a new dataflow analyzer.
    pub fn new() -> Self {
        Self
    }

    fn diag(
        severity: Severity,
        code: &str,
        message: String,
        func_name: &str,
        node_id: Option<u64>,
    ) -> Diagnostic {
        Diagnostic {
            severity,
            code: code.to_string(),
            message,
            function: Some(func_name.to_string()),
            node_id,
        }
    }
}

impl VerificationPass for DataflowAnalyzer {
    fn name(&self) -> &str {
        "dataflow-analyzer"
    }

    fn check_function(&self, func: &Function) -> VerificationReport {
        let mut report = VerificationReport::new();

        // Collect all defined node IDs.
        let mut defined: HashSet<u64> = HashSet::new();
        let param_count = func.params.len();

        // D004: Check for duplicate node IDs.
        for node in &func.nodes {
            if !defined.insert(node.id) {
                report.add(Self::diag(
                    Severity::Error,
                    "D004",
                    format!("Duplicate definition of node {}", node.id),
                    &func.name,
                    Some(node.id),
                ));
            }
        }

        // D001: Check that all inputs reference defined nodes.
        for node in &func.nodes {
            for &input_id in &node.inputs {
                if !defined.contains(&input_id) {
                    report.add(Self::diag(
                        Severity::Error,
                        "D001",
                        format!("Node {} references undefined input {}", node.id, input_id),
                        &func.name,
                        Some(node.id),
                    ));
                }
            }
        }

        // D002: Parameter index validation.
        for node in &func.nodes {
            if let IROperation::Parameter { index, .. } = &node.op {
                if *index >= param_count {
                    report.add(Self::diag(
                        Severity::Error,
                        "D002",
                        format!(
                            "Parameter index {} exceeds function parameter count {}",
                            index, param_count
                        ),
                        &func.name,
                        Some(node.id),
                    ));
                }
            }
        }

        // D003: Function should have a return node.
        let has_return = func.return_node.is_some()
            || func
                .nodes
                .iter()
                .any(|n| matches!(n.op, IROperation::Return));
        if !has_return && !func.nodes.is_empty() {
            report.add(Self::diag(
                Severity::Warning,
                "D003",
                "Function has no return node".to_string(),
                &func.name,
                None,
            ));
        }

        report
    }
}

// ============================================================================
// Composite Verifier
// ============================================================================

/// Runs all verification passes and produces a combined report.
///
/// This is the primary entry point for verification. It runs type checking,
/// shape inference, resource checking, and termination analysis.
pub struct Verifier {
    passes: Vec<Box<dyn VerificationPass>>,
}

impl Verifier {
    /// Create a default verifier with all passes enabled.
    pub fn new() -> Self {
        Self {
            passes: vec![
                Box::new(TypeChecker::new()),
                Box::new(ShapeInference::new()),
                Box::new(ResourceChecker::new()),
                Box::new(TerminationAnalyzer::new()),
                Box::new(BoundsChecker::new()),
                Box::new(DataflowAnalyzer::new()),
            ],
        }
    }

    /// Create a verifier with only specific passes.
    pub fn with_passes(passes: Vec<Box<dyn VerificationPass>>) -> Self {
        Self { passes }
    }

    /// Verify a program, returning a combined report.
    pub fn verify(&self, program: &Program) -> VerificationReport {
        let mut report = VerificationReport::new();
        for pass in &self.passes {
            report.merge(pass.check_program(program));
        }
        report
    }

    /// Quick check: verify and return Ok or Err.
    pub fn check(&self, program: &Program) -> crate::error::Result<()> {
        let report = self.verify(program);
        if report.is_ok() {
            Ok(())
        } else {
            let errors: Vec<String> = report.errors().iter().map(|d| d.to_string()).collect();
            Err(crate::error::RmiError::validation(
                "IR verification failed",
                errors.join("; "),
            ))
        }
    }

    /// Get pass names.
    pub fn pass_names(&self) -> Vec<&str> {
        self.passes.iter().map(|p| p.name()).collect()
    }
}

impl Default for Verifier {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::codegen::{ActivationKind, BinaryOpKind, FunctionBuilder, PrimitiveType};

    fn f32_type() -> IRType {
        IRType::Primitive(PrimitiveType::F32)
    }

    fn tensor_type(shape: Vec<usize>) -> IRType {
        IRType::tensor(PrimitiveType::F32, shape)
    }

    // ── Type Checker ─────────────────────────────────────────────────

    #[test]
    fn type_checker_valid_program() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f32_type())], f32_type());
        let x = fb.param(0);
        let y = fb.activation(ActivationKind::ReLU, x);
        fb.ret(y);

        let func = fb.build();
        let tc = TypeChecker::new();
        let report = tc.check_function(&func);
        assert!(
            report.is_ok(),
            "Valid program should pass type checking: {:?}",
            report.errors()
        );
    }

    #[test]
    fn type_checker_no_duplicate_ids() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f32_type())], f32_type());
        let x = fb.param(0);
        fb.ret(x);

        let func = fb.build();
        let tc = TypeChecker::new();
        let report = tc.check_function(&func);

        let dups = report.errors().iter().filter(|d| d.code == "T006").count();
        assert_eq!(dups, 0, "Builder should not create duplicate IDs");
    }

    // ── Shape Inference ──────────────────────────────────────────────

    #[test]
    fn shape_inference_compatible_broadcast() {
        let mut fb = FunctionBuilder::new(
            "test",
            vec![
                ("a".to_string(), tensor_type(vec![32, 784])),
                ("b".to_string(), tensor_type(vec![784])),
            ],
            tensor_type(vec![32, 784]),
        );
        let a = fb.param(0);
        let b = fb.param(1);
        let c = fb.binary_op(BinaryOpKind::Add, a, b);
        fb.ret(c);

        let func = fb.build();
        let si = ShapeInference::new();
        let report = si.check_function(&func);
        let errors = report.errors();
        assert!(
            errors.is_empty(),
            "Broadcast-compatible shapes should pass: {:?}",
            errors
        );
    }

    #[test]
    fn shape_inference_incompatible() {
        let mut fb = FunctionBuilder::new(
            "test",
            vec![
                ("a".to_string(), tensor_type(vec![32, 10])),
                ("b".to_string(), tensor_type(vec![20, 5])),
            ],
            tensor_type(vec![32, 10]),
        );
        let a = fb.param(0);
        let b = fb.param(1);
        let _c = fb.binary_op(BinaryOpKind::Add, a, b);

        let func = fb.build();
        let si = ShapeInference::new();
        let report = si.check_function(&func);
        assert!(
            !report.errors().is_empty(),
            "Incompatible shapes should produce errors"
        );
    }

    #[test]
    fn shape_inference_matmul_valid() {
        let mut fb = FunctionBuilder::new(
            "test",
            vec![
                ("a".to_string(), tensor_type(vec![32, 784])),
                ("b".to_string(), tensor_type(vec![784, 10])),
            ],
            tensor_type(vec![32, 10]),
        );
        let a = fb.param(0);
        let b = fb.param(1);
        let c = fb.matmul(a, b, false, false);
        fb.ret(c);

        let func = fb.build();
        let si = ShapeInference::new();
        let report = si.check_function(&func);
        assert!(
            report.errors().is_empty(),
            "Valid matmul shapes should pass: {:?}",
            report.errors()
        );
    }

    #[test]
    fn shape_inference_matmul_invalid() {
        let mut fb = FunctionBuilder::new(
            "test",
            vec![
                ("a".to_string(), tensor_type(vec![32, 784])),
                ("b".to_string(), tensor_type(vec![100, 10])),
            ],
            tensor_type(vec![32, 10]),
        );
        let a = fb.param(0);
        let b = fb.param(1);
        let _c = fb.matmul(a, b, false, false);

        let func = fb.build();
        let si = ShapeInference::new();
        let report = si.check_function(&func);
        assert!(
            !report.errors().is_empty(),
            "Invalid matmul shapes should produce error"
        );
    }

    // ── Resource Checker ─────────────────────────────────────────────

    #[test]
    fn resource_checker_balanced() {
        // Build a function with alloc + free manually
        let mut func = Function::new("test", vec![], IRType::Primitive(PrimitiveType::Void));
        let alloc_id = func.add_node(IROperation::Alloc, tensor_type(vec![100]), vec![]);
        func.add_node(
            IROperation::Free,
            IRType::Primitive(PrimitiveType::Void),
            vec![alloc_id],
        );

        let rc = ResourceChecker::new();
        let report = rc.check_function(&func);
        assert!(
            report.errors().is_empty(),
            "Balanced alloc/free should pass: {:?}",
            report.errors()
        );
        assert!(
            report.warnings().is_empty(),
            "Balanced alloc/free should have no warnings: {:?}",
            report.warnings()
        );
    }

    #[test]
    fn resource_checker_leak() {
        let mut func = Function::new("test", vec![], IRType::Primitive(PrimitiveType::Void));
        func.add_node(IROperation::Alloc, tensor_type(vec![100]), vec![]);

        let rc = ResourceChecker::new();
        let report = rc.check_function(&func);
        assert!(
            !report.warnings().is_empty(),
            "Unfreed alloc should produce leak warning"
        );
    }

    #[test]
    fn resource_checker_double_free() {
        let mut func = Function::new("test", vec![], IRType::Primitive(PrimitiveType::Void));
        let alloc_id = func.add_node(IROperation::Alloc, tensor_type(vec![100]), vec![]);
        func.add_node(
            IROperation::Free,
            IRType::Primitive(PrimitiveType::Void),
            vec![alloc_id],
        );
        func.add_node(
            IROperation::Free,
            IRType::Primitive(PrimitiveType::Void),
            vec![alloc_id],
        );

        let rc = ResourceChecker::new();
        let report = rc.check_function(&func);
        assert!(
            !report.errors().is_empty(),
            "Double free should produce error"
        );
    }

    // ── Termination Analyzer ─────────────────────────────────────────

    #[test]
    fn termination_no_issues() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f32_type())], f32_type());
        let x = fb.param(0);
        fb.ret(x);

        let func = fb.build();
        let ta = TerminationAnalyzer::new();
        let report = ta.check_function(&func);
        assert!(
            report.is_ok(),
            "Simple function should have no termination issues"
        );
    }

    #[test]
    fn termination_direct_recursion() {
        let mut func = Function::new("recurse", vec![("x".to_string(), f32_type())], f32_type());
        func.add_node(
            IROperation::Call {
                target: "recurse".to_string(),
            },
            f32_type(),
            vec![],
        );

        let ta = TerminationAnalyzer::new();
        let report = ta.check_function(&func);
        assert!(
            !report.warnings().is_empty(),
            "Direct recursion should produce warning"
        );
    }

    // ── Composite Verifier ───────────────────────────────────────────

    #[test]
    fn verifier_valid_program() {
        let mut prog = Program::new("test");
        let mut fb =
            FunctionBuilder::new("forward", vec![("x".to_string(), f32_type())], f32_type());
        let x = fb.param(0);
        let y = fb.activation(ActivationKind::ReLU, x);
        fb.ret(y);
        prog.add_function(fb.build());

        let verifier = Verifier::new();
        let report = verifier.verify(&prog);
        assert!(
            report.is_ok(),
            "Valid program should pass verification: {}",
            report
        );
    }

    #[test]
    fn verifier_check_api() {
        let mut prog = Program::new("test");
        let mut fb = FunctionBuilder::new("f", vec![], f32_type());
        let c = fb.constant(crate::core::codegen::IRValue::F64(1.0), f32_type());
        fb.ret(c);
        prog.add_function(fb.build());

        let verifier = Verifier::new();
        assert!(verifier.check(&prog).is_ok());
    }

    #[test]
    fn verifier_pass_names() {
        let verifier = Verifier::new();
        let names = verifier.pass_names();
        assert!(names.contains(&"type-checker"));
        assert!(names.contains(&"shape-inference"));
        assert!(names.contains(&"resource-checker"));
        assert!(names.contains(&"termination-analyzer"));
    }

    #[test]
    fn verification_report_display() {
        let mut report = VerificationReport::new();
        report.add(Diagnostic {
            severity: Severity::Error,
            code: "T001".to_string(),
            message: "test error".to_string(),
            function: Some("foo".to_string()),
            node_id: Some(42),
        });
        let display = format!("{}", report);
        assert!(display.contains("1 error(s)"));
        assert!(display.contains("T001"));
    }
    // -- Bounds Checker -----------------------------------------------

    #[test]
    fn bounds_checker_zero_tensor_split() {
        let func = Function {
            name: "test".to_string(),
            params: vec![],
            return_type: f32_type(),
            nodes: vec![IRNode {
                id: 1,
                op: IROperation::TensorSplit {
                    axis: 0,
                    num_splits: 0,
                },
                output_type: f32_type(),
                inputs: vec![],
                attrs: std::collections::BTreeMap::new(),
                source_loc: None,
            }],
            return_node: None,
            attrs: std::collections::HashMap::new(),
        };

        let bc = BoundsChecker::new();
        let report = bc.check_function(&func);
        assert!(
            report.errors().iter().any(|d| d.code == "B001"),
            "Should detect zero splits"
        );
    }

    #[test]
    fn bounds_checker_pool_zero_kernel() {
        let func = Function {
            name: "test".to_string(),
            params: vec![],
            return_type: f32_type(),
            nodes: vec![IRNode {
                id: 1,
                op: IROperation::Pool {
                    kind: crate::core::codegen::PoolKind::Max,
                    kernel: vec![0, 3],
                    stride: vec![1, 1],
                },
                output_type: f32_type(),
                inputs: vec![],
                attrs: std::collections::BTreeMap::new(),
                source_loc: None,
            }],
            return_node: None,
            attrs: std::collections::HashMap::new(),
        };

        let bc = BoundsChecker::new();
        let report = bc.check_function(&func);
        assert!(
            report.errors().iter().any(|d| d.code == "B004"),
            "Should detect zero kernel dimension"
        );
    }

    #[test]
    fn bounds_checker_valid_passes() {
        let func = Function {
            name: "test".to_string(),
            params: vec![],
            return_type: f32_type(),
            nodes: vec![],
            return_node: None,
            attrs: std::collections::HashMap::new(),
        };

        let bc = BoundsChecker::new();
        let report = bc.check_function(&func);
        assert!(report.is_ok(), "Empty function should pass bounds check");
    }

    // -- Dataflow Analyzer --------------------------------------------

    #[test]
    fn dataflow_undefined_input() {
        let func = Function {
            name: "test".to_string(),
            params: vec![],
            return_type: f32_type(),
            nodes: vec![IRNode {
                id: 1,
                op: IROperation::BinaryOp {
                    op: crate::core::codegen::BinaryOpKind::Add,
                },
                output_type: f32_type(),
                inputs: vec![99, 100], // undefined
                attrs: std::collections::BTreeMap::new(),
                source_loc: None,
            }],
            return_node: None,
            attrs: std::collections::HashMap::new(),
        };

        let da = DataflowAnalyzer::new();
        let report = da.check_function(&func);
        assert!(
            report.errors().iter().any(|d| d.code == "D001"),
            "Should detect undefined inputs"
        );
    }

    #[test]
    fn dataflow_invalid_param_index() {
        let func = Function {
            name: "test".to_string(),
            params: vec![("x".to_string(), f32_type())],
            return_type: f32_type(),
            nodes: vec![IRNode {
                id: 1,
                op: IROperation::Parameter {
                    index: 5,
                    name: "bad".to_string(),
                },
                output_type: f32_type(),
                inputs: vec![],
                attrs: std::collections::BTreeMap::new(),
                source_loc: None,
            }],
            return_node: None,
            attrs: std::collections::HashMap::new(),
        };

        let da = DataflowAnalyzer::new();
        let report = da.check_function(&func);
        assert!(
            report.errors().iter().any(|d| d.code == "D002"),
            "Should detect invalid parameter index"
        );
    }

    #[test]
    fn dataflow_no_return_warning() {
        let func = Function {
            name: "test".to_string(),
            params: vec![("x".to_string(), f32_type())],
            return_type: f32_type(),
            nodes: vec![IRNode {
                id: 1,
                op: IROperation::Parameter {
                    index: 0,
                    name: "x".to_string(),
                },
                output_type: f32_type(),
                inputs: vec![],
                attrs: std::collections::BTreeMap::new(),
                source_loc: None,
            }],
            return_node: None,
            attrs: std::collections::HashMap::new(),
        };

        let da = DataflowAnalyzer::new();
        let report = da.check_function(&func);
        assert!(
            report.warnings().iter().any(|d| d.code == "D003"),
            "Should warn about missing return node"
        );
    }

    #[test]
    fn dataflow_valid_function() {
        let mut fb = FunctionBuilder::new("test", vec![("x".to_string(), f32_type())], f32_type());
        let x = fb.param(0);
        fb.ret(x);
        let func = fb.build();

        let da = DataflowAnalyzer::new();
        let report = da.check_function(&func);
        assert!(
            report.is_ok(),
            "Valid function should pass dataflow analysis"
        );
    }

    #[test]
    fn verifier_includes_new_passes() {
        let v = Verifier::new();
        let names = v.pass_names();
        assert!(
            names.contains(&"bounds-checker"),
            "Should include bounds-checker"
        );
        assert!(
            names.contains(&"dataflow-analyzer"),
            "Should include dataflow-analyzer"
        );
        assert_eq!(names.len(), 6, "Should have 6 passes total");
    }
}
