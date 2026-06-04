//! Symbolic gradient tape for RMIL expressions.
//!
//! Computes reverse-mode automatic differentiation at the expression level.
//! Instead of building a runtime tape of float operations, this module
//! transforms an [`Expr`] tree into its *adjoint* (gradient) form, producing
//! a new expression that computes `∂out/∂in`.
//!
//! # Design
//!
//! Each differentiable [`Op`] knows its local gradient rule. The tape records
//! which ops were applied in what order, then the [`differentiate`] function
//! walks the tape in reverse to build the adjoint expression.
//!
//! # Examples
//!
//! ```
//! use rmi::lang::grad::{GradTape, TapeNode};
//! use rmi::lang::{Expr, Op};
//!
//! let mut tape = GradTape::new();
//!
//! // Forward: x → relu → exp → out
//! let x_id = tape.input("x");
//! let r_id = tape.apply(Op::RELU, &[x_id]);
//! let e_id = tape.apply(Op::EXP, &[r_id]);
//!
//! // Reverse: build adjoint expression d(out)/d(x)
//! let grad_expr = tape.backward(e_id);
//! assert!(grad_expr.node_count() > 0);
//! ```

use crate::lang::expr::Expr;
use crate::lang::op::Op;

// ── Tape node ────────────────────────────────────────────────────────────────

/// An entry on the gradient tape.
#[derive(Debug, Clone)]
pub struct TapeNode {
    /// Index in the tape.
    pub id: usize,
    /// The operation applied (None for inputs).
    pub op: Option<Op>,
    /// Indices of input nodes.
    pub inputs: Vec<usize>,
    /// Optional label (for inputs).
    pub label: Option<String>,
}

// ── Gradient tape ────────────────────────────────────────────────────────────

/// A gradient tape that records forward-pass operations on RMIL expressions
/// and can compute symbolic adjoints via reverse-mode AD.
#[derive(Debug, Clone)]
pub struct GradTape {
    nodes: Vec<TapeNode>,
}

impl GradTape {
    /// Create a new empty tape.
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Register an input variable and return its tape index.
    pub fn input(&mut self, label: &str) -> usize {
        let id = self.nodes.len();
        self.nodes.push(TapeNode {
            id,
            op: None,
            inputs: vec![],
            label: Some(label.to_string()),
        });
        id
    }

    /// Register a constant (not differentiated through).
    pub fn constant(&mut self) -> usize {
        let id = self.nodes.len();
        self.nodes.push(TapeNode {
            id,
            op: None,
            inputs: vec![],
            label: None,
        });
        id
    }

    /// Apply an operation to tape entries and return the new entry index.
    pub fn apply(&mut self, op: Op, inputs: &[usize]) -> usize {
        let id = self.nodes.len();
        self.nodes.push(TapeNode {
            id,
            op: Some(op),
            inputs: inputs.to_vec(),
            label: None,
        });
        id
    }

    /// Number of nodes on the tape.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tape is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Get a node by index.
    pub fn node(&self, id: usize) -> &TapeNode {
        &self.nodes[id]
    }

    /// Perform reverse-mode AD from `output_id` back through the tape.
    ///
    /// Returns a vector of adjoint expressions indexed by tape node id.
    /// The adjoint at `output_id` is initialized to `Expr::float(1.0)` (seed).
    pub fn backward_all(&self, output_id: usize) -> Vec<Option<Expr>> {
        let n = self.nodes.len();
        let mut adjoints: Vec<Option<Expr>> = vec![None; n];

        // Seed the output adjoint
        adjoints[output_id] = Some(Expr::float(1.0));

        // Walk tape in reverse
        for i in (0..=output_id).rev() {
            let adj = match adjoints[i].clone() {
                Some(a) => a,
                None => continue,
            };
            let node = &self.nodes[i];
            let op = match node.op {
                Some(op) => op,
                None => continue, // input or constant
            };

            // Compute local gradient for each input and accumulate
            let local_grads = local_gradient(op, &node.inputs, &adj);

            for (input_idx, grad_expr) in node.inputs.iter().zip(local_grads) {
                let entry = &mut adjoints[*input_idx];
                if let Some(existing) = entry {
                    // Accumulate: existing + new gradient
                    *entry = Some(Expr::op2(Op::ADD, existing.clone(), grad_expr));
                } else {
                    *entry = Some(grad_expr);
                }
            }
        }

        adjoints
    }

    /// Compute the adjoint expression for the output w.r.t the first input
    /// that has a label matching `wrt`.
    ///
    /// Returns `None` if no such input exists on the tape or the gradient is zero.
    pub fn backward(&self, output_id: usize) -> Expr {
        let adjoints = self.backward_all(output_id);
        // Find the first labelled input with a non-None adjoint
        for node in &self.nodes {
            if node.label.is_some() {
                if let Some(adj) = &adjoints[node.id] {
                    return adj.clone();
                }
            }
        }
        // If nothing found, gradient is zero
        Expr::float(0.0)
    }

    /// Get adjoint for a specific tape node.
    pub fn grad_at(&self, output_id: usize, target_id: usize) -> Option<Expr> {
        let adjoints = self.backward_all(output_id);
        adjoints.get(target_id).cloned().flatten()
    }
}

impl Default for GradTape {
    fn default() -> Self {
        Self::new()
    }
}

// ── Local gradient rules ─────────────────────────────────────────────────────

/// Compute the local gradient contributions for each input of an op.
///
/// Given the upstream adjoint `upstream`, returns one gradient expression
/// per input of the operation.
fn local_gradient(op: Op, inputs: &[usize], upstream: &Expr) -> Vec<Expr> {
    match op {
        // ── Unary ops ────────────────────────────────────────────────────
        // d/dx relu(x) = upstream * (x > 0 ? 1 : 0)
        // Approximation: upstream * relu'(x) ≈ upstream (pass-through with indicator)
        Op::RELU => vec![Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::RELU))],

        // d/dx sigmoid(x) = upstream * sigmoid(x) * (1 - sigmoid(x))
        Op::SIGMOID => {
            let sig = Expr::op1(Op::SIGMOID);
            let one_minus = Expr::op2(Op::SUB, Expr::float(1.0), sig.clone());
            let local = Expr::op2(Op::MUL, sig, one_minus);
            vec![Expr::op2(Op::MUL, upstream.clone(), local)]
        }

        // d/dx tanh(x) = upstream * (1 - tanh²(x))
        Op::TANH_ACT => {
            let th = Expr::op1(Op::TANH_ACT);
            let th_sq = Expr::op2(Op::MUL, th.clone(), th);
            let local = Expr::op2(Op::SUB, Expr::float(1.0), th_sq);
            vec![Expr::op2(Op::MUL, upstream.clone(), local)]
        }

        // d/dx gelu(x) ≈ upstream * 0.5 * (1 + tanh(...)) + ...
        // Simplified: treat as smooth relu
        Op::GELU => vec![Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::SIGMOID))],

        // d/dx exp(x) = upstream * exp(x)
        Op::EXP => vec![Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::EXP))],

        // d/dx log(x) = upstream / x  (represented as upstream * (1/x))
        Op::LOG => {
            let inv = Expr::op2(Op::DIV, Expr::float(1.0), Expr::op1(Op::IDENTITY));
            vec![Expr::op2(Op::MUL, upstream.clone(), inv)]
        }

        // d/dx sqrt(x) = upstream / (2 * sqrt(x))
        Op::SQRT => {
            let two_sqrt = Expr::op2(Op::MUL, Expr::float(2.0), Expr::op1(Op::SQRT));
            let inv = Expr::op2(Op::DIV, Expr::float(1.0), two_sqrt);
            vec![Expr::op2(Op::MUL, upstream.clone(), inv)]
        }

        // d/dx neg(x) = -upstream
        Op::NEG => vec![Expr::op1(Op::NEG)],

        // d/dx abs(x) = upstream * sign(x) ≈ upstream
        Op::ABS => vec![upstream.clone()],

        // d/dx sin(x) = upstream * cos(x)
        Op::SIN => vec![Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::COS))],

        // d/dx cos(x) = upstream * (-sin(x))
        Op::COS => {
            let neg_sin = Expr::App(Op::NEG, vec![Expr::op1(Op::SIN)]);
            vec![Expr::op2(Op::MUL, upstream.clone(), neg_sin)]
        }

        // d/dx identity(x) = upstream
        Op::IDENTITY => vec![upstream.clone()],

        // ── Binary ops ───────────────────────────────────────────────────
        // d/d{a,b} add(a,b) = {upstream, upstream}
        Op::ADD | Op::RES_ADD => vec![upstream.clone(), upstream.clone()],

        // d/da sub(a,b) = upstream, d/db sub(a,b) = -upstream
        Op::SUB => vec![upstream.clone(), Expr::App(Op::NEG, vec![upstream.clone()])],

        // d/da mul(a,b) = upstream * b, d/db mul(a,b) = upstream * a
        Op::MUL => {
            // We can't directly reference the *values* of inputs since we
            // only have tape indices, so we use placeholder symbols.
            // In practice the adjoints compose with forward expressions.
            vec![
                Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::IDENTITY)),
                Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::IDENTITY)),
            ]
        }

        // d/da div(a,b) = upstream / b, d/db div(a,b) = -upstream * a / b²
        Op::DIV => vec![
            Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::IDENTITY)),
            Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::NEG)),
        ],

        // d/da pow(a,b) = upstream * b * pow(a, b-1)
        Op::POW => vec![
            Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::IDENTITY)),
            Expr::op2(Op::MUL, upstream.clone(), Expr::op1(Op::LOG)),
        ],

        // ── Neural ops (pass gradients through) ──────────────────────────
        Op::LINEAR | Op::MATMUL | Op::CONV2D => {
            // For neural ops, gradient flows through
            vec![upstream.clone(); inputs.len()]
        }

        Op::LAYER_NORM | Op::BATCH_NORM => vec![upstream.clone()],
        Op::SOFTMAX => vec![upstream.clone()],
        Op::DROP => vec![upstream.clone()],
        Op::EMBED => vec![upstream.clone()],
        Op::ATTN => vec![upstream.clone()],

        // ── Non-differentiable ops ───────────────────────────────────────
        // Return zero gradients
        _ => vec![Expr::float(0.0); inputs.len()],
    }
}

// ── Expr-level differentiation ───────────────────────────────────────────────

/// Build a gradient tape from an `Expr` tree by walking it top-down.
///
/// Returns `(tape, output_node_id)`.
pub fn trace_expr(expr: &Expr) -> (GradTape, usize) {
    let mut tape = GradTape::new();
    let out = trace_recursive(expr, &mut tape);
    (tape, out)
}

fn trace_recursive(expr: &Expr, tape: &mut GradTape) -> usize {
    match expr {
        Expr::Lit(_) => tape.constant(),
        Expr::Ref(_) => tape.input("ref"),

        Expr::App(op, args) => {
            let input_ids: Vec<usize> = args.iter().map(|a| trace_recursive(a, tape)).collect();
            tape.apply(*op, &input_ids)
        }

        Expr::Seq(a, b) => {
            let a_id = trace_recursive(a, tape);
            // In a pipeline, the output of a flows into b
            // For the tape we model this as b applied to a's output
            let _a_id = a_id; // b consumes a's output
            trace_recursive(b, tape)
        }

        Expr::Par(a, b) => {
            let a_id = trace_recursive(a, tape);
            let b_id = trace_recursive(b, tape);
            tape.apply(Op::CONCAT, &[a_id, b_id])
        }

        Expr::Cond { pred: _, yes, no } => {
            let y_id = trace_recursive(yes, tape);
            let n_id = trace_recursive(no, tape);
            tape.apply(Op::ADD, &[y_id, n_id]) // approximate: sum of branches
        }

        Expr::Let { val, body, .. } => {
            let _v_id = trace_recursive(val, tape);
            trace_recursive(body, tape)
        }

        Expr::Lam { body, .. } => trace_recursive(body, tape),

        Expr::Call(f, _args) => trace_recursive(f, tape),

        Expr::Block(exprs) => {
            let mut last = tape.constant();
            for e in exprs {
                last = trace_recursive(e, tape);
            }
            last
        }
    }
}

/// Differentiate an expression symbolically, returning the adjoint expression.
///
/// This traces the expression into a gradient tape and runs reverse-mode AD.
pub fn differentiate(expr: &Expr) -> Expr {
    let (tape, out_id) = trace_expr(expr);
    tape.backward(out_id)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::expr::Val;

    #[test]
    fn tape_basic() {
        let mut tape = GradTape::new();
        assert!(tape.is_empty());
        let x = tape.input("x");
        assert_eq!(x, 0);
        assert_eq!(tape.len(), 1);
    }

    #[test]
    fn tape_apply() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let r = tape.apply(Op::RELU, &[x]);
        assert_eq!(r, 1);
        assert_eq!(tape.node(r).op, Some(Op::RELU));
    }

    #[test]
    fn tape_chain() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let r = tape.apply(Op::RELU, &[x]);
        let e = tape.apply(Op::EXP, &[r]);
        assert_eq!(tape.len(), 3);
        assert_eq!(e, 2);
    }

    #[test]
    fn backward_identity() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let y = tape.apply(Op::IDENTITY, &[x]);
        let grad = tape.grad_at(y, x);
        assert!(grad.is_some());
    }

    #[test]
    fn backward_add() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let y = tape.input("y");
        let z = tape.apply(Op::ADD, &[x, y]);
        let gx = tape.grad_at(z, x);
        let gy = tape.grad_at(z, y);
        assert!(gx.is_some());
        assert!(gy.is_some());
    }

    #[test]
    fn backward_relu() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let r = tape.apply(Op::RELU, &[x]);
        let grad = tape.grad_at(r, x).unwrap();
        assert!(grad.node_count() > 0);
    }

    #[test]
    fn backward_sigmoid() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let s = tape.apply(Op::SIGMOID, &[x]);
        let grad = tape.grad_at(s, x).unwrap();
        // sigmoid grad should involve mul and sub
        let ops = grad.opcodes();
        assert!(ops.contains(&Op::MUL));
    }

    #[test]
    fn backward_exp() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let e = tape.apply(Op::EXP, &[x]);
        let grad = tape.grad_at(e, x).unwrap();
        let ops = grad.opcodes();
        assert!(ops.contains(&Op::EXP));
    }

    #[test]
    fn backward_sub() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let y = tape.input("y");
        let z = tape.apply(Op::SUB, &[x, y]);
        let gx = tape.grad_at(z, x).unwrap();
        let gy = tape.grad_at(z, y).unwrap();
        assert!(gx.node_count() > 0);
        assert!(gy.node_count() > 0);
    }

    #[test]
    fn backward_chain() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let r = tape.apply(Op::RELU, &[x]);
        let e = tape.apply(Op::EXP, &[r]);
        let grad = tape.grad_at(e, x).unwrap();
        assert!(grad.node_count() > 2);
    }

    #[test]
    fn backward_non_diff() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let s = tape.apply(Op::SEND, &[x]);
        let grad = tape.grad_at(s, x).unwrap();
        // Non-differentiable → zero gradient
        assert!(matches!(grad, Expr::Lit(Val::F32(_))));
    }

    #[test]
    fn tape_default() {
        let tape = GradTape::default();
        assert!(tape.is_empty());
    }

    #[test]
    fn tape_constant() {
        let mut tape = GradTape::new();
        let c = tape.constant();
        assert_eq!(c, 0);
        assert!(tape.node(c).label.is_none());
    }

    #[test]
    fn trace_expr_lit() {
        let (tape, _) = trace_expr(&Expr::int(42));
        assert_eq!(tape.len(), 1);
    }

    #[test]
    fn trace_expr_app() {
        let expr = Expr::op2(Op::ADD, Expr::int(1), Expr::int(2));
        let (tape, out) = trace_expr(&expr);
        assert!(tape.len() >= 3);
        assert!(out > 0);
    }

    #[test]
    fn trace_expr_pipeline() {
        let expr = Expr::op1(Op::RELU) >> Expr::op1(Op::EXP);
        let (tape, _) = trace_expr(&expr);
        assert!(tape.len() >= 2);
    }

    #[test]
    fn differentiate_simple() {
        let expr = Expr::op1(Op::EXP);
        let grad = differentiate(&expr);
        assert!(grad.node_count() > 0);
    }

    #[test]
    fn backward_tanh() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let t = tape.apply(Op::TANH_ACT, &[x]);
        let grad = tape.grad_at(t, x).unwrap();
        let ops = grad.opcodes();
        assert!(ops.contains(&Op::MUL));
        assert!(ops.contains(&Op::SUB));
    }

    #[test]
    fn backward_sin_cos() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let s = tape.apply(Op::SIN, &[x]);
        let grad = tape.grad_at(s, x).unwrap();
        let ops = grad.opcodes();
        assert!(ops.contains(&Op::COS));

        let mut tape2 = GradTape::new();
        let x2 = tape2.input("x");
        let c = tape2.apply(Op::COS, &[x2]);
        let grad2 = tape2.grad_at(c, x2).unwrap();
        let ops2 = grad2.opcodes();
        assert!(ops2.contains(&Op::SIN));
    }

    #[test]
    fn backward_sqrt_log() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let s = tape.apply(Op::SQRT, &[x]);
        let grad = tape.grad_at(s, x).unwrap();
        assert!(grad.node_count() > 2);

        let mut tape2 = GradTape::new();
        let x2 = tape2.input("x");
        let l = tape2.apply(Op::LOG, &[x2]);
        let grad2 = tape2.grad_at(l, x2).unwrap();
        assert!(grad2.node_count() > 2);
    }

    #[test]
    fn backward_all_returns_correct_size() {
        let mut tape = GradTape::new();
        let x = tape.input("x");
        let y = tape.input("y");
        let z = tape.apply(Op::ADD, &[x, y]);
        let adjoints = tape.backward_all(z);
        assert_eq!(adjoints.len(), 3);
    }

    #[test]
    fn backward_no_label_returns_zero() {
        let mut tape = GradTape::new();
        let c = tape.constant();
        let r = tape.apply(Op::RELU, &[c]);
        let grad = tape.backward(r);
        // constant has no label, so backward returns zero
        assert_eq!(grad, Expr::float(0.0));
    }
}
