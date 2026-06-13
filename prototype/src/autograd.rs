/// MAGE Autograd Compilation — reverse-mode automatic differentiation.
///
/// Compiles `grad(loss, params)` expressions into backward-pass MLIR ops.
/// The algorithm:
///   1. Build a computation graph (Wengert list) from the forward expression
///   2. Topologically sort the graph in reverse
///   3. Generate adjoint (gradient) operations for each node
///   4. Emit MAGE.grad.* MLIR dialect operations
///
/// Supported differentiable operations:
///   - Arithmetic: add, sub, mul, div
///   - Tensor: matmul (⊗), transpose, sum, relu, sigmoid, tanh, softmax
///   - Reductions: mean, sum, norm
use crate::ast;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity};
use std::collections::HashMap;

// ── Computation graph ───────────────────────────────────────────────

/// A node in the computation graph for AD.
#[derive(Debug, Clone)]
pub struct GradNode {
    pub id: usize,
    pub op: GradOp,
    /// Indices of input nodes.
    pub inputs: Vec<usize>,
    /// The symbolic name (for parameters and intermediates).
    pub name: Option<String>,
    /// Shape annotation (if known).
    pub shape: Vec<u64>,
}

/// Differentiable operations in the computation graph.
#[derive(Debug, Clone, PartialEq)]
pub enum GradOp {
    /// A named parameter (leaf).
    Param(String),
    /// A constant (no gradient).
    Const,
    /// Input data (no gradient).
    Input,
    /// Element-wise addition.
    Add,
    /// Element-wise subtraction.
    Sub,
    /// Element-wise multiplication.
    Mul,
    /// Element-wise division.
    Div,
    /// Matrix multiplication (⊗).
    MatMul,
    /// Transpose.
    Transpose,
    /// Sum reduction (over all or specified axes).
    Sum,
    /// Mean reduction.
    Mean,
    /// ReLU activation.
    ReLU,
    /// Sigmoid activation.
    Sigmoid,
    /// Tanh activation.
    Tanh,
    /// Softmax.
    Softmax,
    /// Log.
    Log,
    /// Exp.
    Exp,
    /// Negation.
    Neg,
    /// Power.
    Pow,
    /// Cross-entropy loss.
    CrossEntropy,
    /// Mean squared error loss.
    MSE,
    /// A generic function call (opaque — grad is identity or zero).
    Call(String),
}

// ── Gradient tape ───────────────────────────────────────────────────

/// The computation graph built during forward pass analysis.
pub struct GradTape {
    pub nodes: Vec<GradNode>,
    name_to_id: HashMap<String, usize>,
}

impl GradTape {
    pub fn new() -> Self {
        GradTape {
            nodes: Vec::new(),
            name_to_id: HashMap::new(),
        }
    }

    fn add_node(&mut self, op: GradOp, inputs: Vec<usize>, name: Option<String>) -> usize {
        let id = self.nodes.len();
        if let Some(ref n) = name {
            self.name_to_id.insert(n.clone(), id);
        }
        self.nodes.push(GradNode {
            id,
            op,
            inputs,
            name,
            shape: Vec::new(),
        });
        id
    }

    pub fn param(&mut self, name: &str) -> usize {
        if let Some(&id) = self.name_to_id.get(name) {
            return id;
        }
        self.add_node(GradOp::Param(name.to_string()), vec![], Some(name.to_string()))
    }

    pub fn input(&mut self, name: &str) -> usize {
        self.add_node(GradOp::Input, vec![], Some(name.to_string()))
    }

    pub fn constant(&mut self) -> usize {
        self.add_node(GradOp::Const, vec![], None)
    }

    pub fn binary(&mut self, op: GradOp, lhs: usize, rhs: usize) -> usize {
        self.add_node(op, vec![lhs, rhs], None)
    }

    pub fn unary(&mut self, op: GradOp, x: usize) -> usize {
        self.add_node(op, vec![x], None)
    }

    pub fn lookup(&self, name: &str) -> Option<usize> {
        self.name_to_id.get(name).copied()
    }
}

// ── Backward pass generation ────────────────────────────────────────

/// Result of compiling a grad() expression.
pub struct GradResult {
    /// MLIR operations for the backward pass.
    pub mlir_ops: Vec<String>,
    /// Parameter name → gradient SSA variable name.
    pub param_grads: HashMap<String, String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Generate the backward pass for a computation graph.
/// `loss_id` is the output (scalar loss) node.
/// `param_names` are the parameters we want gradients for.
pub fn backward(tape: &GradTape, loss_id: usize, param_names: &[String]) -> GradResult {
    let n = tape.nodes.len();
    let mut adjoints: Vec<Option<String>> = vec![None; n];
    let mut ops: Vec<String> = Vec::new();
    let mut ssa = 0usize;
    let mut diagnostics = Vec::new();

    let fresh = |ssa: &mut usize| -> String {
        let v = format!("%grad_{ssa}");
        *ssa += 1;
        v
    };

    // Seed: d(loss)/d(loss) = 1.0
    let seed = fresh(&mut ssa);
    ops.push(format!(
        "{seed} = MAGE.grad.const 1.0 : f32  // d(loss)/d(loss)"
    ));
    adjoints[loss_id] = Some(seed);

    // Reverse topological order.
    for i in (0..=loss_id).rev() {
        let node = &tape.nodes[i];
        let adj = match &adjoints[i] {
            Some(a) => a.clone(),
            None => continue, // No gradient flows to this node.
        };

        match &node.op {
            GradOp::Param(_) | GradOp::Const | GradOp::Input => {
                // Leaf nodes: gradient accumulation stops here.
            }
            GradOp::Add => {
                // d(a+b)/da = 1, d(a+b)/db = 1
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &adj);
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[1], &adj);
            }
            GradOp::Sub => {
                // d(a-b)/da = 1, d(a-b)/db = -1
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &adj);
                let neg = fresh(&mut ssa);
                ops.push(format!(
                    "{neg} = MAGE.grad.neg {adj} : f32"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[1], &neg);
            }
            GradOp::Mul => {
                // d(a*b)/da = b, d(a*b)/db = a
                let fwd_a = format!("%fwd_{}", node.inputs[0]);
                let fwd_b = format!("%fwd_{}", node.inputs[1]);
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.mul {adj}, {fwd_b} : f32  // d/da of mul"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
                let db = fresh(&mut ssa);
                ops.push(format!(
                    "{db} = MAGE.grad.mul {adj}, {fwd_a} : f32  // d/db of mul"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[1], &db);
            }
            GradOp::Div => {
                // d(a/b)/da = 1/b, d(a/b)/db = -a/b^2
                let fwd_a = format!("%fwd_{}", node.inputs[0]);
                let fwd_b = format!("%fwd_{}", node.inputs[1]);
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.div {adj}, {fwd_b} : f32  // d/da of div"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
                let db = fresh(&mut ssa);
                ops.push(format!(
                    "{db} = MAGE.grad.div_rhs {adj}, {fwd_a}, {fwd_b} : f32  // d/db of div"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[1], &db);
            }
            GradOp::MatMul => {
                // d(A⊗B)/dA = adj ⊗ B^T, d(A⊗B)/dB = A^T ⊗ adj
                let fwd_a = format!("%fwd_{}", node.inputs[0]);
                let fwd_b = format!("%fwd_{}", node.inputs[1]);
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.matmul_lhs {adj}, {fwd_b} : tensor<*xf32>"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
                let db = fresh(&mut ssa);
                ops.push(format!(
                    "{db} = MAGE.grad.matmul_rhs {fwd_a}, {adj} : tensor<*xf32>"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[1], &db);
            }
            GradOp::Transpose => {
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.transpose {adj} : tensor<*xf32>"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::ReLU => {
                let fwd = format!("%fwd_{}", node.inputs[0]);
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.relu {adj}, {fwd} : f32  // adj * (x > 0)"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Sigmoid => {
                let fwd_out = format!("%fwd_{i}");
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.sigmoid {adj}, {fwd_out} : f32  // adj * σ * (1-σ)"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Tanh => {
                let fwd_out = format!("%fwd_{i}");
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.tanh {adj}, {fwd_out} : f32  // adj * (1-tanh²)"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Softmax => {
                let fwd_out = format!("%fwd_{i}");
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.softmax {adj}, {fwd_out} : tensor<*xf32>"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Log => {
                let fwd = format!("%fwd_{}", node.inputs[0]);
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.div {adj}, {fwd} : f32  // d/dx of log = 1/x"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Exp => {
                let fwd_out = format!("%fwd_{i}");
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.mul {adj}, {fwd_out} : f32  // d/dx of exp = exp"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Neg => {
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.neg {adj} : f32"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Sum | GradOp::Mean => {
                // Gradient broadcasts back.
                let da = fresh(&mut ssa);
                let kind = if node.op == GradOp::Mean { "mean" } else { "sum" };
                ops.push(format!(
                    "{da} = MAGE.grad.broadcast_{kind} {adj} : tensor<*xf32>"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::CrossEntropy => {
                let fwd_pred = format!("%fwd_{}", node.inputs[0]);
                let fwd_target = format!("%fwd_{}", node.inputs[1]);
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.cross_entropy {adj}, {fwd_pred}, {fwd_target} : tensor<*xf32>"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
                // No gradient for targets.
            }
            GradOp::MSE => {
                let fwd_pred = format!("%fwd_{}", node.inputs[0]);
                let fwd_target = format!("%fwd_{}", node.inputs[1]);
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.mse {adj}, {fwd_pred}, {fwd_target} : tensor<*xf32>"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Pow => {
                let fwd_base = format!("%fwd_{}", node.inputs[0]);
                let fwd_exp = format!("%fwd_{}", node.inputs[1]);
                let da = fresh(&mut ssa);
                ops.push(format!(
                    "{da} = MAGE.grad.pow_base {adj}, {fwd_base}, {fwd_exp} : f32"
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &da);
            }
            GradOp::Call(name) => {
                diagnostics.push(Diagnostic::categorized(
                    Severity::Warning,
                    format!("no derivative rule for call `{name}` — treating as identity"),
                    DiagnosticCategory::TypeMismatch,
                    None,
                ));
                accumulate(&mut adjoints, &mut ops, &mut ssa, node.inputs[0], &adj);
            }
        }
    }

    // Collect parameter gradients.
    let mut param_grads = HashMap::new();
    for pname in param_names {
        if let Some(&id) = tape.name_to_id.get(pname) {
            if let Some(ref grad_var) = adjoints[id] {
                param_grads.insert(pname.clone(), grad_var.clone());
            } else {
                diagnostics.push(Diagnostic::categorized(
                    Severity::Warning,
                    format!("parameter `{pname}` has no gradient (not in computation path)"),
                    DiagnosticCategory::TypeMismatch,
                    None,
                ));
            }
        }
    }

    GradResult {
        mlir_ops: ops,
        param_grads,
        diagnostics,
    }
}

/// Accumulate gradient into node's adjoint (sum if already has one).
fn accumulate(
    adjoints: &mut [Option<String>],
    ops: &mut Vec<String>,
    ssa: &mut usize,
    node_id: usize,
    incoming: &str,
) {
    match &adjoints[node_id] {
        None => {
            adjoints[node_id] = Some(incoming.to_string());
        }
        Some(existing) => {
            let sum = format!("%grad_{ssa}");
            *ssa += 1;
            ops.push(format!(
                "{sum} = MAGE.grad.add {existing}, {incoming} : f32  // accumulate"
            ));
            adjoints[node_id] = Some(sum);
        }
    }
}

// ── AST → GradTape builder ─────────────────────────────────────────

/// Build a computation graph from a training block's loss expression.
pub fn build_tape_from_train(train: &ast::TrainDef) -> GradTape {
    let mut tape = GradTape::new();

    // Register the net name as a parameter source.
    tape.param(&train.net);

    // Walk the body to build the tape (simplified — handles common patterns).
    build_tape_from_block(&mut tape, &train.body);
    tape
}

fn build_tape_from_block(tape: &mut GradTape, block: &ast::Block) {
    for stmt in &block.stmts {
        if let ast::Stmt::Let { pattern, value, .. } = stmt {
            if let ast::Pattern::Ident { name } = pattern {
                let id = build_tape_expr(tape, value);
                tape.name_to_id.insert(name.clone(), id);
            }
        }
    }
    if let Some(tail) = &block.tail_expr {
        build_tape_expr(tape, tail);
    }
}

fn build_tape_expr(tape: &mut GradTape, expr: &ast::Expr) -> usize {
    match expr {
        ast::Expr::Ident { name } => {
            tape.name_to_id.get(name).copied().unwrap_or_else(|| tape.input(name))
        }
        ast::Expr::Literal { .. } => tape.constant(),
        ast::Expr::Binary { op, left, right } => {
            let l = build_tape_expr(tape, left);
            let r = build_tape_expr(tape, right);
            let grad_op = match op.as_str() {
                "+" => GradOp::Add,
                "-" => GradOp::Sub,
                "*" => GradOp::Mul,
                "/" => GradOp::Div,
                "⊗" => GradOp::MatMul,
                _ => GradOp::Call(op.clone()),
            };
            tape.binary(grad_op, l, r)
        }
        ast::Expr::Unary { op, operand } => {
            let x = build_tape_expr(tape, operand);
            match op.as_str() {
                "-" => tape.unary(GradOp::Neg, x),
                _ => x,
            }
        }
        ast::Expr::Call { func, args } => {
            let name = match func.as_ref() {
                ast::Expr::Ident { name } => name.clone(),
                _ => "unknown".into(),
            };
            let arg_ids: Vec<usize> = args.iter().map(|a| build_tape_expr(tape, a)).collect();
            let grad_op = match name.as_str() {
                "relu" | "ReLU" => GradOp::ReLU,
                "sigmoid" => GradOp::Sigmoid,
                "tanh" => GradOp::Tanh,
                "softmax" => GradOp::Softmax,
                "sum" => GradOp::Sum,
                "mean" => GradOp::Mean,
                "log" => GradOp::Log,
                "exp" => GradOp::Exp,
                "cross_entropy" => GradOp::CrossEntropy,
                "mse" | "mse_loss" => GradOp::MSE,
                _ => GradOp::Call(name),
            };
            match arg_ids.len() {
                0 => tape.constant(),
                1 => tape.unary(grad_op, arg_ids[0]),
                _ => tape.binary(grad_op, arg_ids[0], arg_ids[1]),
            }
        }
        ast::Expr::MethodCall { receiver, method, args, .. } => {
            let recv = build_tape_expr(tape, receiver);
            let grad_op = match method.as_str() {
                "relu" => GradOp::ReLU,
                "sigmoid" => GradOp::Sigmoid,
                "tanh" => GradOp::Tanh,
                "softmax" => GradOp::Softmax,
                "sum" => GradOp::Sum,
                "mean" => GradOp::Mean,
                "log" => GradOp::Log,
                "exp" => GradOp::Exp,
                "t" | "T" | "transpose" => GradOp::Transpose,
                _ => GradOp::Call(method.clone()),
            };
            if args.is_empty() {
                tape.unary(grad_op, recv)
            } else {
                let arg = build_tape_expr(tape, &args[0]);
                tape.binary(grad_op, recv, arg)
            }
        }
        _ => tape.constant(),
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tape_simple_linear() {
        let mut tape = GradTape::new();
        let w = tape.param("w");
        let x = tape.input("x");
        let wx = tape.binary(GradOp::MatMul, w, x);
        let b = tape.param("b");
        let out = tape.binary(GradOp::Add, wx, b);
        let loss = tape.unary(GradOp::Sum, out);

        let result = backward(&tape, loss, &["w".into(), "b".into()]);
        assert!(result.param_grads.contains_key("w"));
        assert!(result.param_grads.contains_key("b"));
        assert!(!result.mlir_ops.is_empty());
    }

    #[test]
    fn tape_chain_rule() {
        let mut tape = GradTape::new();
        let x = tape.param("x");
        let a = tape.binary(GradOp::Mul, x, x);  // x^2
        let b = tape.unary(GradOp::Sum, a);

        let result = backward(&tape, b, &["x".into()]);
        assert!(result.param_grads.contains_key("x"));
        // Should have mul gradients and accumulation.
        let has_mul = result.mlir_ops.iter().any(|op| op.contains("grad.mul"));
        assert!(has_mul, "expected grad.mul ops for d/dx of x*x");
    }

    #[test]
    fn tape_relu_grad() {
        let mut tape = GradTape::new();
        let x = tape.param("x");
        let r = tape.unary(GradOp::ReLU, x);
        let loss = tape.unary(GradOp::Sum, r);

        let result = backward(&tape, loss, &["x".into()]);
        let has_relu = result.mlir_ops.iter().any(|op| op.contains("grad.relu"));
        assert!(has_relu, "expected grad.relu op");
    }

    #[test]
    fn tape_no_gradient_warning() {
        let mut tape = GradTape::new();
        let _x = tape.param("x");
        let c = tape.constant();  // Not connected to loss.
        let loss = tape.unary(GradOp::Sum, c);

        let result = backward(&tape, loss, &["x".into()]);
        assert!(result.diagnostics.iter().any(|d| d.message.contains("no gradient")));
    }

    #[test]
    fn tape_matmul_grad() {
        let mut tape = GradTape::new();
        let a = tape.param("A");
        let b = tape.param("B");
        let c = tape.binary(GradOp::MatMul, a, b);
        let loss = tape.unary(GradOp::Mean, c);

        let result = backward(&tape, loss, &["A".into(), "B".into()]);
        assert!(result.param_grads.contains_key("A"));
        assert!(result.param_grads.contains_key("B"));
        let has_matmul = result.mlir_ops.iter().any(|op| op.contains("grad.matmul"));
        assert!(has_matmul);
    }

    #[test]
    fn tape_cross_entropy_grad() {
        let mut tape = GradTape::new();
        let pred = tape.param("pred");
        let target = tape.input("target");
        let loss = tape.binary(GradOp::CrossEntropy, pred, target);

        let result = backward(&tape, loss, &["pred".into()]);
        assert!(result.param_grads.contains_key("pred"));
    }

    #[test]
    fn backward_ops_are_ordered() {
        let mut tape = GradTape::new();
        let w = tape.param("w");
        let x = tape.input("x");
        let wx = tape.binary(GradOp::Mul, w, x);
        let loss = tape.unary(GradOp::Sum, wx);

        let result = backward(&tape, loss, &["w".into()]);
        // First op should be the seed constant.
        assert!(result.mlir_ops[0].contains("1.0"));
    }
}
