//! Automatic Differentiation
//!
//! Provides gradient computation capabilities for AI agents to
//! analyze and optimize neural architectures programmatically.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// A differentiable variable in the computation graph
#[derive(Debug, Clone)]
pub struct Variable {
    /// Unique identifier
    pub id: Uuid,

    /// The value tensor (flattened for simplicity)
    pub data: Vec<f32>,

    /// Shape of the tensor
    pub shape: Vec<usize>,

    /// Gradient tensor (same shape as data)
    pub grad: Option<Vec<f32>>,

    /// Whether this variable requires gradient
    pub requires_grad: bool,

    /// Name for debugging
    pub name: Option<String>,
}

impl Variable {
    /// Create a new variable
    pub fn new(data: Vec<f32>, shape: Vec<usize>, requires_grad: bool) -> Self {
        assert_eq!(data.len(), shape.iter().product::<usize>());
        Self {
            id: Uuid::new_v4(),
            data,
            shape,
            grad: None,
            requires_grad,
            name: None,
        }
    }

    /// Create a variable with a name
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Create zeros tensor
    pub fn zeros(shape: &[usize], requires_grad: bool) -> Self {
        let numel: usize = shape.iter().product();
        Self::new(vec![0.0; numel], shape.to_vec(), requires_grad)
    }

    /// Create ones tensor
    pub fn ones(shape: &[usize], requires_grad: bool) -> Self {
        let numel: usize = shape.iter().product();
        Self::new(vec![1.0; numel], shape.to_vec(), requires_grad)
    }

    /// Create from scalar
    pub fn scalar(value: f32, requires_grad: bool) -> Self {
        Self::new(vec![value], vec![1], requires_grad)
    }

    /// Number of elements
    pub fn numel(&self) -> usize {
        self.data.len()
    }

    /// Zero the gradient
    pub fn zero_grad(&mut self) {
        if self.requires_grad {
            self.grad = Some(vec![0.0; self.data.len()]);
        }
    }

    /// Accumulate gradient
    pub fn accumulate_grad(&mut self, grad: &[f32]) {
        if let Some(ref mut g) = self.grad {
            for (a, b) in g.iter_mut().zip(grad.iter()) {
                *a += b;
            }
        } else {
            self.grad = Some(grad.to_vec());
        }
    }
}

/// Types of operations in the computation graph for automatic differentiation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OpType {
    // === Binary Operations ===
    /// Element-wise addition
    Add,
    /// Element-wise subtraction
    Sub,
    /// Element-wise multiplication
    Mul,
    /// Element-wise division
    Div,
    /// Matrix multiplication
    MatMul,

    // === Unary Operations ===
    /// Negation (-x)
    Neg,
    /// Exponential (e^x)
    Exp,
    /// Natural logarithm
    Log,
    /// Square root
    Sqrt,
    /// Absolute value
    Abs,

    // === Activations ===
    /// Rectified Linear Unit
    ReLU,
    /// Sigmoid function
    Sigmoid,
    /// Hyperbolic tangent
    Tanh,
    /// Gaussian Error Linear Unit
    GeLU,
    /// Softmax activation
    Softmax {
        /// Dimension to apply softmax over
        dim: i32,
    },

    // === Reductions ===
    /// Sum reduction
    Sum {
        /// Dimension to reduce (None for global)
        dim: Option<i32>,
        /// Keep reduced dimension
        keepdim: bool,
    },
    /// Mean reduction
    Mean {
        /// Dimension to reduce (None for global)
        dim: Option<i32>,
        /// Keep reduced dimension
        keepdim: bool,
    },
    /// Max reduction
    Max {
        /// Dimension to reduce (None for global)
        dim: Option<i32>,
        /// Keep reduced dimension
        keepdim: bool,
    },

    // === Shape Operations ===
    /// Reshape tensor
    Reshape {
        /// New tensor shape
        new_shape: Vec<usize>,
    },
    /// Transpose dimensions
    Transpose {
        /// First dimension to swap
        dim0: usize,
        /// Second dimension to swap
        dim1: usize,
    },
    /// Remove singleton dimension
    Squeeze {
        /// Dimension to squeeze (None for all)
        dim: Option<i32>,
    },
    /// Add singleton dimension
    Unsqueeze {
        /// Position to insert dimension
        dim: i32,
    },

    // === Other Operations ===
    /// Clone tensor
    Clone,
    /// Power operation (x^exp)
    Pow {
        /// Exponent value
        exponent: f32,
    },

    // === Neural Network Operations ===
    /// Linear/dense layer
    Linear {
        /// Whether layer has bias term
        has_bias: bool,
    },
    /// 2D convolution
    Conv2d {
        /// Kernel size [height, width]
        kernel_size: [usize; 2],
        /// Stride [height, width]
        stride: [usize; 2],
        /// Padding [height, width]
        padding: [usize; 2],
    },
    /// Batch normalization
    BatchNorm {
        /// Epsilon for numerical stability
        eps: f32,
        /// Momentum for running stats
        momentum: f32,
    },
    /// Layer normalization
    LayerNorm {
        /// Epsilon for numerical stability
        eps: f32,
    },
    /// Dropout regularization
    Dropout {
        /// Dropout probability
        p: f32,
    },
}

/// A node in the computation graph
#[derive(Debug)]
pub struct ComputeNode {
    /// Unique identifier
    pub id: Uuid,

    /// Operation type
    pub op: OpType,

    /// Input variable IDs
    pub inputs: Vec<Uuid>,

    /// Output variable ID
    pub output: Uuid,

    /// Cached values for backward pass
    pub saved_tensors: Vec<Vec<f32>>,
}

/// Gradient tape for recording operations
pub struct GradientTape {
    /// Computation graph nodes
    nodes: Vec<ComputeNode>,

    /// Variable storage
    variables: HashMap<Uuid, Variable>,

    /// Whether tape is recording
    recording: bool,
}

impl GradientTape {
    /// Create a new gradient tape
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            variables: HashMap::new(),
            recording: true,
        }
    }

    /// Start recording
    pub fn start(&mut self) {
        self.recording = true;
    }

    /// Stop recording
    pub fn stop(&mut self) {
        self.recording = false;
    }

    /// Check if recording
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Register a variable
    pub fn register(&mut self, var: Variable) -> Uuid {
        let id = var.id;
        self.variables.insert(id, var);
        id
    }

    /// Get a variable by ID
    pub fn get(&self, id: Uuid) -> Option<&Variable> {
        self.variables.get(&id)
    }

    /// Get a mutable variable by ID
    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut Variable> {
        self.variables.get_mut(&id)
    }

    /// Record an operation
    pub fn record(
        &mut self,
        op: OpType,
        inputs: Vec<Uuid>,
        output: Variable,
        saved: Vec<Vec<f32>>,
    ) -> Uuid {
        let output_id = output.id;

        if self.recording {
            self.nodes.push(ComputeNode {
                id: Uuid::new_v4(),
                op,
                inputs,
                output: output_id,
                saved_tensors: saved,
            });
        }

        self.variables.insert(output_id, output);
        output_id
    }

    /// Add operation
    pub fn add(&mut self, a: Uuid, b: Uuid) -> Uuid {
        let var_a = self.variables.get(&a).expect("Variable not found");
        let var_b = self.variables.get(&b).expect("Variable not found");

        assert_eq!(var_a.shape, var_b.shape, "Shape mismatch for add");

        let result_data: Vec<f32> = var_a
            .data
            .iter()
            .zip(var_b.data.iter())
            .map(|(x, y)| x + y)
            .collect();

        let requires_grad = var_a.requires_grad || var_b.requires_grad;
        let output = Variable::new(result_data, var_a.shape.clone(), requires_grad);

        self.record(OpType::Add, vec![a, b], output, vec![])
    }

    /// Multiply operation
    pub fn mul(&mut self, a: Uuid, b: Uuid) -> Uuid {
        let var_a = self.variables.get(&a).expect("Variable not found");
        let var_b = self.variables.get(&b).expect("Variable not found");

        assert_eq!(var_a.shape, var_b.shape, "Shape mismatch for mul");

        let result_data: Vec<f32> = var_a
            .data
            .iter()
            .zip(var_b.data.iter())
            .map(|(x, y)| x * y)
            .collect();

        let requires_grad = var_a.requires_grad || var_b.requires_grad;
        let output = Variable::new(result_data, var_a.shape.clone(), requires_grad);

        // Save inputs for backward
        let saved = vec![var_a.data.clone(), var_b.data.clone()];
        self.record(OpType::Mul, vec![a, b], output, saved)
    }

    /// Matrix multiplication
    pub fn matmul(&mut self, a: Uuid, b: Uuid) -> Uuid {
        let var_a = self.variables.get(&a).expect("Variable not found");
        let var_b = self.variables.get(&b).expect("Variable not found");

        // Handle 2D case: [M, K] @ [K, N] -> [M, N]
        assert_eq!(var_a.shape.len(), 2, "matmul requires 2D tensors");
        assert_eq!(var_b.shape.len(), 2, "matmul requires 2D tensors");
        assert_eq!(
            var_a.shape[1], var_b.shape[0],
            "Inner dimensions must match"
        );

        let m = var_a.shape[0];
        let k = var_a.shape[1];
        let n = var_b.shape[1];

        let mut result_data = vec![0.0; m * n];

        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0;
                for l in 0..k {
                    sum += var_a.data[i * k + l] * var_b.data[l * n + j];
                }
                result_data[i * n + j] = sum;
            }
        }

        let requires_grad = var_a.requires_grad || var_b.requires_grad;
        let output = Variable::new(result_data, vec![m, n], requires_grad);

        // Save for backward: need original tensors and shapes
        let saved = vec![
            var_a.data.clone(),
            var_b.data.clone(),
            vec![m as f32, k as f32, n as f32], // shapes
        ];
        self.record(OpType::MatMul, vec![a, b], output, saved)
    }

    /// ReLU activation
    pub fn relu(&mut self, x: Uuid) -> Uuid {
        let var_x = self.variables.get(&x).expect("Variable not found");

        let result_data: Vec<f32> = var_x.data.iter().map(|&v| v.max(0.0)).collect();

        let output = Variable::new(result_data, var_x.shape.clone(), var_x.requires_grad);

        // Save mask for backward
        let mask: Vec<f32> = var_x
            .data
            .iter()
            .map(|&v| if v > 0.0 { 1.0 } else { 0.0 })
            .collect();
        self.record(OpType::ReLU, vec![x], output, vec![mask])
    }

    /// Sigmoid activation
    pub fn sigmoid(&mut self, x: Uuid) -> Uuid {
        let var_x = self.variables.get(&x).expect("Variable not found");

        let result_data: Vec<f32> = var_x
            .data
            .iter()
            .map(|&v| 1.0 / (1.0 + (-v).exp()))
            .collect();

        let output = Variable::new(
            result_data.clone(),
            var_x.shape.clone(),
            var_x.requires_grad,
        );

        // Save output for backward (sigmoid' = sigmoid * (1 - sigmoid))
        self.record(OpType::Sigmoid, vec![x], output, vec![result_data])
    }

    /// Sum reduction
    pub fn sum(&mut self, x: Uuid, dim: Option<i32>, keepdim: bool) -> Uuid {
        let var_x = self.variables.get(&x).expect("Variable not found");

        match dim {
            None => {
                // Sum all elements
                let sum: f32 = var_x.data.iter().sum();
                let shape = if keepdim {
                    vec![1; var_x.shape.len()]
                } else {
                    vec![1]
                };
                let output = Variable::new(vec![sum], shape, var_x.requires_grad);
                let saved = vec![vec![var_x.data.len() as f32]];
                self.record(OpType::Sum { dim: None, keepdim }, vec![x], output, saved)
            }
            Some(d) => {
                // Sum along dimension
                let dim = if d < 0 {
                    (var_x.shape.len() as i32 + d) as usize
                } else {
                    d as usize
                };

                // Simplified: just handle the case of reducing last dim
                // Full implementation would handle all cases
                let reduction_size = var_x.shape[dim];
                let outer_size: usize = var_x.shape[..dim].iter().product();
                let inner_size: usize = var_x.shape[dim + 1..].iter().product();

                let mut result = vec![0.0; outer_size * inner_size];

                for i in 0..outer_size {
                    for j in 0..inner_size {
                        let mut sum = 0.0;
                        for k in 0..reduction_size {
                            sum += var_x.data[i * reduction_size * inner_size + k * inner_size + j];
                        }
                        result[i * inner_size + j] = sum;
                    }
                }

                let mut new_shape = var_x.shape.clone();
                if keepdim {
                    new_shape[dim] = 1;
                } else {
                    new_shape.remove(dim);
                }

                let output = Variable::new(result, new_shape, var_x.requires_grad);
                let saved = vec![var_x.shape.iter().map(|&s| s as f32).collect()];
                self.record(
                    OpType::Sum {
                        dim: Some(d),
                        keepdim,
                    },
                    vec![x],
                    output,
                    saved,
                )
            }
        }
    }

    /// Mean reduction
    pub fn mean(&mut self, x: Uuid, dim: Option<i32>, keepdim: bool) -> Uuid {
        let var_x = self.variables.get(&x).expect("Variable not found");

        match dim {
            None => {
                let n = var_x.data.len() as f32;
                let mean: f32 = var_x.data.iter().sum::<f32>() / n;
                let shape = if keepdim {
                    vec![1; var_x.shape.len()]
                } else {
                    vec![1]
                };
                let output = Variable::new(vec![mean], shape, var_x.requires_grad);
                let saved = vec![vec![n]];
                self.record(OpType::Mean { dim: None, keepdim }, vec![x], output, saved)
            }
            Some(d) => {
                // Extract shape info before calling self.sum() to avoid borrow issues
                let shape_len = var_x.shape.len();
                let dim_idx = if d < 0 {
                    (shape_len as i32 + d) as usize
                } else {
                    d as usize
                };
                let n = var_x.shape[dim_idx] as f32;

                // Now we can call self.sum() since var_x is no longer borrowed
                let sum_id = self.sum(x, Some(d), keepdim);

                // Scale down
                let var_sum = self
                    .variables
                    .get(&sum_id)
                    .expect("Variable not found")
                    .clone();
                let result_data: Vec<f32> = var_sum.data.iter().map(|&v| v / n).collect();
                let output =
                    Variable::new(result_data, var_sum.shape.clone(), var_sum.requires_grad);
                let saved = vec![vec![n]];
                self.record(
                    OpType::Mean {
                        dim: Some(d),
                        keepdim,
                    },
                    vec![x],
                    output,
                    saved,
                )
            }
        }
    }

    /// Exp operation
    pub fn exp(&mut self, x: Uuid) -> Uuid {
        let var_x = self.variables.get(&x).expect("Variable not found");

        let result_data: Vec<f32> = var_x.data.iter().map(|&v| v.exp()).collect();

        let output = Variable::new(
            result_data.clone(),
            var_x.shape.clone(),
            var_x.requires_grad,
        );
        self.record(OpType::Exp, vec![x], output, vec![result_data])
    }

    /// Log operation
    pub fn log(&mut self, x: Uuid) -> Uuid {
        let var_x = self.variables.get(&x).expect("Variable not found");

        let result_data: Vec<f32> = var_x.data.iter().map(|&v| v.ln()).collect();

        let output = Variable::new(result_data, var_x.shape.clone(), var_x.requires_grad);
        self.record(OpType::Log, vec![x], output, vec![var_x.data.clone()])
    }

    /// Perform backward pass from a scalar output
    pub fn backward(&mut self, output_id: Uuid) {
        // Initialize gradient of output as 1
        if let Some(var) = self.variables.get_mut(&output_id) {
            var.grad = Some(vec![1.0; var.data.len()]);
        }

        // Process nodes in reverse order
        for node in self.nodes.iter().rev() {
            let output_grad = self
                .variables
                .get(&node.output)
                .and_then(|v| v.grad.clone())
                .unwrap_or_else(|| {
                    vec![
                        0.0;
                        self.variables
                            .get(&node.output)
                            .map(|v| v.numel())
                            .unwrap_or(1)
                    ]
                });

            match &node.op {
                OpType::Add => {
                    // Gradient flows through to both inputs
                    for &input_id in &node.inputs {
                        if let Some(var) = self.variables.get_mut(&input_id) {
                            if var.requires_grad {
                                var.accumulate_grad(&output_grad);
                            }
                        }
                    }
                }

                OpType::Mul
                    // d(a*b)/da = b, d(a*b)/db = a
                    if node.saved_tensors.len() >= 2 => {
                        let a_data = &node.saved_tensors[0];
                        let b_data = &node.saved_tensors[1];

                        if let Some(var_a) = self.variables.get_mut(&node.inputs[0]) {
                            if var_a.requires_grad {
                                let grad_a: Vec<f32> = output_grad
                                    .iter()
                                    .zip(b_data.iter())
                                    .map(|(g, b)| g * b)
                                    .collect();
                                var_a.accumulate_grad(&grad_a);
                            }
                        }

                        if let Some(var_b) = self.variables.get_mut(&node.inputs[1]) {
                            if var_b.requires_grad {
                                let grad_b: Vec<f32> = output_grad
                                    .iter()
                                    .zip(a_data.iter())
                                    .map(|(g, a)| g * a)
                                    .collect();
                                var_b.accumulate_grad(&grad_b);
                            }
                        }
                    }

                OpType::MatMul
                    // d(A@B)/dA = G @ B^T, d(A@B)/dB = A^T @ G
                    if node.saved_tensors.len() >= 3 => {
                        let a_data = &node.saved_tensors[0];
                        let b_data = &node.saved_tensors[1];
                        let shapes = &node.saved_tensors[2];

                        let m = shapes[0] as usize;
                        let k = shapes[1] as usize;
                        let n = shapes[2] as usize;

                        // Grad w.r.t. A: G @ B^T
                        if let Some(var_a) = self.variables.get_mut(&node.inputs[0]) {
                            if var_a.requires_grad {
                                let mut grad_a = vec![0.0; m * k];
                                for i in 0..m {
                                    for j in 0..k {
                                        let mut sum = 0.0;
                                        for l in 0..n {
                                            sum += output_grad[i * n + l] * b_data[j * n + l];
                                        }
                                        grad_a[i * k + j] = sum;
                                    }
                                }
                                var_a.accumulate_grad(&grad_a);
                            }
                        }

                        // Grad w.r.t. B: A^T @ G
                        if let Some(var_b) = self.variables.get_mut(&node.inputs[1]) {
                            if var_b.requires_grad {
                                let mut grad_b = vec![0.0; k * n];
                                for i in 0..k {
                                    for j in 0..n {
                                        let mut sum = 0.0;
                                        for l in 0..m {
                                            sum += a_data[l * k + i] * output_grad[l * n + j];
                                        }
                                        grad_b[i * n + j] = sum;
                                    }
                                }
                                var_b.accumulate_grad(&grad_b);
                            }
                        }
                    }

                OpType::ReLU => {
                    if let Some(mask) = node.saved_tensors.first() {
                        if let Some(var) = self.variables.get_mut(&node.inputs[0]) {
                            if var.requires_grad {
                                let grad: Vec<f32> = output_grad
                                    .iter()
                                    .zip(mask.iter())
                                    .map(|(g, m)| g * m)
                                    .collect();
                                var.accumulate_grad(&grad);
                            }
                        }
                    }
                }

                OpType::Sigmoid => {
                    // sigmoid' = sigmoid * (1 - sigmoid)
                    if let Some(sigmoid_out) = node.saved_tensors.first() {
                        if let Some(var) = self.variables.get_mut(&node.inputs[0]) {
                            if var.requires_grad {
                                let grad: Vec<f32> = output_grad
                                    .iter()
                                    .zip(sigmoid_out.iter())
                                    .map(|(g, s)| g * s * (1.0 - s))
                                    .collect();
                                var.accumulate_grad(&grad);
                            }
                        }
                    }
                }

                OpType::Exp => {
                    // d(exp(x))/dx = exp(x)
                    if let Some(exp_out) = node.saved_tensors.first() {
                        if let Some(var) = self.variables.get_mut(&node.inputs[0]) {
                            if var.requires_grad {
                                let grad: Vec<f32> = output_grad
                                    .iter()
                                    .zip(exp_out.iter())
                                    .map(|(g, e)| g * e)
                                    .collect();
                                var.accumulate_grad(&grad);
                            }
                        }
                    }
                }

                OpType::Log => {
                    // d(log(x))/dx = 1/x
                    if let Some(input) = node.saved_tensors.first() {
                        if let Some(var) = self.variables.get_mut(&node.inputs[0]) {
                            if var.requires_grad {
                                let grad: Vec<f32> = output_grad
                                    .iter()
                                    .zip(input.iter())
                                    .map(|(g, x)| g / x)
                                    .collect();
                                var.accumulate_grad(&grad);
                            }
                        }
                    }
                }

                OpType::Sum { dim, keepdim: _ } => {
                    // Gradient broadcasts back
                    if let Some(var) = self.variables.get_mut(&node.inputs[0]) {
                        if var.requires_grad {
                            match dim {
                                None => {
                                    // Scalar output - broadcast gradient to all elements
                                    let grad = vec![output_grad[0]; var.data.len()];
                                    var.accumulate_grad(&grad);
                                }
                                Some(_) => {
                                    // Dimension reduction - broadcast along reduced dim
                                    // Simplified: just repeat the gradient
                                    let grad = vec![output_grad[0]; var.data.len()];
                                    var.accumulate_grad(&grad);
                                }
                            }
                        }
                    }
                }

                OpType::Mean { dim: _, keepdim: _ } => {
                    if let Some(n) = node.saved_tensors.first().and_then(|v| v.first()) {
                        if let Some(var) = self.variables.get_mut(&node.inputs[0]) {
                            if var.requires_grad {
                                let grad = vec![output_grad[0] / n; var.data.len()];
                                var.accumulate_grad(&grad);
                            }
                        }
                    }
                }

                _ => {
                    // Other ops not yet implemented
                }
            }
        }
    }

    /// Clear the tape
    pub fn clear(&mut self) {
        self.nodes.clear();
    }

    /// Get number of operations recorded
    pub fn num_ops(&self) -> usize {
        self.nodes.len()
    }
}

impl Default for GradientTape {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute gradient of output w.r.t. inputs
pub fn grad(
    output_id: Uuid,
    input_ids: &[Uuid],
    tape: &mut GradientTape,
) -> HashMap<Uuid, Vec<f32>> {
    tape.backward(output_id);

    input_ids
        .iter()
        .filter_map(|&id| tape.get(id).and_then(|v| v.grad.clone()).map(|g| (id, g)))
        .collect()
}

/// Perform backward pass starting from output
pub fn backward(output_id: Uuid, tape: &mut GradientTape) {
    tape.backward(output_id);
}

/// Thread-safe gradient tape for concurrent autodiff
pub struct ThreadSafeTape {
    /// Inner thread-safe tape storage
    inner: Arc<RwLock<GradientTape>>,
}

impl ThreadSafeTape {
    /// Create a new thread-safe gradient tape
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(GradientTape::new())),
        }
    }

    /// Execute a function with mutable access to the tape
    pub fn with_tape<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut GradientTape) -> R,
    {
        let mut tape = self.inner.write().unwrap();
        f(&mut tape)
    }
}

impl Default for ThreadSafeTape {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ThreadSafeTape {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_forward() {
        let mut tape = GradientTape::new();

        let x = Variable::new(vec![1.0, 2.0, 3.0], vec![3], true);
        let y = Variable::new(vec![4.0, 5.0, 6.0], vec![3], true);

        let x_id = tape.register(x);
        let y_id = tape.register(y);

        let z_id = tape.add(x_id, y_id);

        let z = tape.get(z_id).unwrap();
        assert_eq!(z.data, vec![5.0, 7.0, 9.0]);
    }

    #[test]
    fn test_backward_add() {
        let mut tape = GradientTape::new();

        let x = Variable::new(vec![1.0, 2.0, 3.0], vec![3], true);
        let y = Variable::new(vec![4.0, 5.0, 6.0], vec![3], true);

        let x_id = tape.register(x);
        let y_id = tape.register(y);

        let z_id = tape.add(x_id, y_id);
        let loss_id = tape.sum(z_id, None, false);

        tape.backward(loss_id);

        let x_grad = tape.get(x_id).unwrap().grad.as_ref().unwrap();
        let y_grad = tape.get(y_id).unwrap().grad.as_ref().unwrap();

        // Gradient of sum is all ones
        assert_eq!(x_grad, &vec![1.0, 1.0, 1.0]);
        assert_eq!(y_grad, &vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_backward_mul() {
        let mut tape = GradientTape::new();

        let x = Variable::new(vec![2.0, 3.0], vec![2], true);
        let y = Variable::new(vec![4.0, 5.0], vec![2], true);

        let x_id = tape.register(x);
        let y_id = tape.register(y);

        let z_id = tape.mul(x_id, y_id); // z = [8, 15]
        let loss_id = tape.sum(z_id, None, false); // loss = 23

        tape.backward(loss_id);

        let x_grad = tape.get(x_id).unwrap().grad.as_ref().unwrap();
        let y_grad = tape.get(y_id).unwrap().grad.as_ref().unwrap();

        // d(xy)/dx = y, d(xy)/dy = x
        assert_eq!(x_grad, &vec![4.0, 5.0]);
        assert_eq!(y_grad, &vec![2.0, 3.0]);
    }

    #[test]
    fn test_matmul_forward() {
        let mut tape = GradientTape::new();

        // [2, 3] @ [3, 2] -> [2, 2]
        let a = Variable::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3], true);
        let b = Variable::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![3, 2], true);

        let a_id = tape.register(a);
        let b_id = tape.register(b);

        let c_id = tape.matmul(a_id, b_id);

        let c = tape.get(c_id).unwrap();
        assert_eq!(c.shape, vec![2, 2]);
        // [[1,2,3], [4,5,6]] @ [[1,2], [3,4], [5,6]]
        // = [[1*1+2*3+3*5, 1*2+2*4+3*6], [4*1+5*3+6*5, 4*2+5*4+6*6]]
        // = [[22, 28], [49, 64]]
        assert_eq!(c.data, vec![22.0, 28.0, 49.0, 64.0]);
    }

    #[test]
    fn test_relu_backward() {
        let mut tape = GradientTape::new();

        let x = Variable::new(vec![-1.0, 0.0, 1.0, 2.0], vec![4], true);
        let x_id = tape.register(x);

        let y_id = tape.relu(x_id);
        let loss_id = tape.sum(y_id, None, false);

        tape.backward(loss_id);

        let x_grad = tape.get(x_id).unwrap().grad.as_ref().unwrap();
        // ReLU gradient: 0 where x <= 0, 1 where x > 0
        assert_eq!(x_grad, &vec![0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn test_variable_zeros() {
        let v = Variable::zeros(&[2, 3], true);
        assert_eq!(v.data.len(), 6);
        assert!(v.data.iter().all(|&x| x == 0.0));
        assert_eq!(v.shape, vec![2, 3]);
        assert!(v.requires_grad);
    }

    #[test]
    fn test_variable_ones() {
        let v = Variable::ones(&[4], false);
        assert_eq!(v.data, vec![1.0, 1.0, 1.0, 1.0]);
        assert!(!v.requires_grad);
    }

    #[test]
    fn test_variable_scalar() {
        let v = Variable::scalar(2.71, true);
        assert_eq!(v.data, vec![2.71]);
        assert_eq!(v.shape, vec![1]);
        assert_eq!(v.numel(), 1);
    }

    #[test]
    fn test_variable_named() {
        let v = Variable::scalar(1.0, false).named("weight");
        assert_eq!(v.name.as_deref(), Some("weight"));
    }

    #[test]
    fn test_zero_grad() {
        let mut v = Variable::new(vec![1.0, 2.0], vec![2], true);
        v.accumulate_grad(&[5.0, 6.0]);
        assert_eq!(v.grad.as_deref(), Some(&[5.0, 6.0][..]));

        v.zero_grad();
        assert_eq!(v.grad.as_deref(), Some(&[0.0, 0.0][..]));
    }

    #[test]
    fn test_accumulate_grad_twice() {
        let mut v = Variable::new(vec![1.0, 2.0], vec![2], true);
        v.accumulate_grad(&[1.0, 1.0]);
        v.accumulate_grad(&[2.0, 3.0]);
        assert_eq!(v.grad.as_deref(), Some(&[3.0, 4.0][..]));
    }

    #[test]
    fn test_tape_start_stop() {
        let mut tape = GradientTape::new();
        assert!(tape.is_recording());
        tape.stop();
        assert!(!tape.is_recording());
        tape.start();
        assert!(tape.is_recording());
    }

    #[test]
    fn test_tape_register_and_get() {
        let mut tape = GradientTape::new();
        let v = Variable::scalar(42.0, false);
        let id = v.id;
        tape.register(v);

        assert!(tape.get(id).is_some());
        assert_eq!(tape.get(id).unwrap().data, vec![42.0]);
        assert!(tape.get(Uuid::new_v4()).is_none());
    }

    #[test]
    fn test_sigmoid_forward() {
        let mut tape = GradientTape::new();
        let x = Variable::new(vec![0.0], vec![1], true);
        let x_id = tape.register(x);

        let y_id = tape.sigmoid(x_id);
        let y = tape.get(y_id).unwrap();
        assert!((y.data[0] - 0.5).abs() < 1e-6); // sigmoid(0) = 0.5
    }

    #[test]
    fn test_sigmoid_backward() {
        let mut tape = GradientTape::new();
        let x = Variable::new(vec![0.0], vec![1], true);
        let x_id = tape.register(x);

        let y_id = tape.sigmoid(x_id);
        tape.backward(y_id);

        let x_grad = tape.get(x_id).unwrap().grad.as_ref().unwrap();
        // sigmoid'(0) = sigmoid(0) * (1 - sigmoid(0)) = 0.5 * 0.5 = 0.25
        assert!((x_grad[0] - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_exp_forward() {
        let mut tape = GradientTape::new();
        let x = Variable::new(vec![0.0, 1.0], vec![2], true);
        let x_id = tape.register(x);

        let y_id = tape.exp(x_id);
        let y = tape.get(y_id).unwrap();
        assert!((y.data[0] - 1.0).abs() < 1e-6);
        assert!((y.data[1] - std::f32::consts::E).abs() < 1e-4);
    }

    #[test]
    fn test_log_forward() {
        let mut tape = GradientTape::new();
        let x = Variable::new(vec![1.0, std::f32::consts::E], vec![2], true);
        let x_id = tape.register(x);

        let y_id = tape.log(x_id);
        let y = tape.get(y_id).unwrap();
        assert!((y.data[0]).abs() < 1e-6); // ln(1) = 0
        assert!((y.data[1] - 1.0).abs() < 1e-4); // ln(e) = 1
    }

    #[test]
    fn test_mean_forward() {
        let mut tape = GradientTape::new();
        let x = Variable::new(vec![1.0, 2.0, 3.0, 4.0], vec![4], true);
        let x_id = tape.register(x);

        let y_id = tape.mean(x_id, None, false);
        let y = tape.get(y_id).unwrap();
        assert!((y.data[0] - 2.5).abs() < 1e-6);
    }

    #[test]
    fn test_sum_along_dim() {
        let mut tape = GradientTape::new();
        // [[1, 2, 3], [4, 5, 6]] shape [2, 3]
        let x = Variable::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3], true);
        let x_id = tape.register(x);

        // Sum along dim 1 -> [6, 15] shape [2]
        let y_id = tape.sum(x_id, Some(1), false);
        let y = tape.get(y_id).unwrap();
        assert_eq!(y.shape, vec![2]);
        assert!((y.data[0] - 6.0).abs() < 1e-6);
        assert!((y.data[1] - 15.0).abs() < 1e-6);
    }

    #[test]
    fn test_clear() {
        let mut tape = GradientTape::new();
        let x = Variable::scalar(1.0, true);
        let x_id = tape.register(x);
        let _ = tape.relu(x_id);
        assert!(tape.num_ops() > 0);

        tape.clear();
        assert_eq!(tape.num_ops(), 0);
    }

    #[test]
    fn test_no_record_when_stopped() {
        let mut tape = GradientTape::new();
        tape.stop();

        let x = Variable::new(vec![1.0, 2.0], vec![2], true);
        let y = Variable::new(vec![3.0, 4.0], vec![2], true);
        let x_id = tape.register(x);
        let y_id = tape.register(y);

        let _ = tape.add(x_id, y_id);
        // Operation still computed, but not recorded on tape
        assert_eq!(tape.num_ops(), 0);
    }
}
