//! Extended Neural Network Layers
//!
//! Additional neural network layers and architectures for air:
//! - Normalization: BatchNorm, GroupNorm, RMSNorm
//! - Recurrent: LSTM, GRU
//! - Modern architectures: TransformerBlock, ResidualBlock, MLP
//! - Regularization: Dropout, DropPath

use super::autodiff::{GradientTape, Variable};
use super::layers::{Layer, LayerNorm, Linear};

// ============================================================================
// Batch Normalization
// ============================================================================

/// Batch Normalization layer
///
/// Normalizes activations across the batch dimension, commonly used
/// in CNNs. Tracks running mean/variance for inference.
#[derive(Debug, Clone)]
pub struct BatchNorm {
    name: String,
    num_features: usize,
    eps: f32,
    momentum: f32,

    /// Learnable scale parameter (gamma)
    weight: Variable,
    /// Learnable shift parameter (beta)
    bias: Variable,
    /// Running mean for inference
    running_mean: Vec<f32>,
    /// Running variance for inference
    running_var: Vec<f32>,
    /// Whether in training mode
    training: bool,
}

impl BatchNorm {
    /// Create a new BatchNorm layer
    pub fn new(num_features: usize) -> Self {
        Self {
            name: format!("batch_norm_{}", num_features),
            num_features,
            eps: 1e-5,
            momentum: 0.1,
            weight: Variable::new(vec![1.0; num_features], vec![num_features], true),
            bias: Variable::new(vec![0.0; num_features], vec![num_features], true),
            running_mean: vec![0.0; num_features],
            running_var: vec![1.0; num_features],
            training: true,
        }
    }

    /// Set epsilon for numerical stability
    pub fn with_eps(mut self, eps: f32) -> Self {
        self.eps = eps;
        self
    }

    /// Set momentum for running stats
    pub fn with_momentum(mut self, momentum: f32) -> Self {
        self.momentum = momentum;
        self
    }

    /// Set training mode
    pub fn train(&mut self, mode: bool) {
        self.training = mode;
    }
}

impl Layer for BatchNorm {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], _tape: &mut GradientTape) -> Variable {
        let input = inputs[0];

        // Input can be [N, C] or [N, C, H, W]
        let batch_size = input.shape[0];
        let num_features = input.shape[1];

        assert_eq!(num_features, self.num_features);

        let spatial_size: usize = input.shape[2..].iter().product::<usize>().max(1);
        let total_elements = batch_size * spatial_size;

        let mut output = vec![0.0; input.data.len()];

        if self.training {
            // Compute batch statistics
            for c in 0..num_features {
                let mut mean = 0.0;
                let mut var = 0.0;

                // Compute mean
                for n in 0..batch_size {
                    for s in 0..spatial_size {
                        let idx = n * num_features * spatial_size + c * spatial_size + s;
                        mean += input.data[idx];
                    }
                }
                mean /= total_elements as f32;

                // Compute variance
                for n in 0..batch_size {
                    for s in 0..spatial_size {
                        let idx = n * num_features * spatial_size + c * spatial_size + s;
                        let diff = input.data[idx] - mean;
                        var += diff * diff;
                    }
                }
                var /= total_elements as f32;

                // Normalize and apply affine transform
                let std = (var + self.eps).sqrt();
                for n in 0..batch_size {
                    for s in 0..spatial_size {
                        let idx = n * num_features * spatial_size + c * spatial_size + s;
                        let normalized = (input.data[idx] - mean) / std;
                        output[idx] = self.weight.data[c] * normalized + self.bias.data[c];
                    }
                }
            }
        } else {
            // Use running statistics
            for c in 0..num_features {
                let mean = self.running_mean[c];
                let std = (self.running_var[c] + self.eps).sqrt();

                for n in 0..batch_size {
                    for s in 0..spatial_size {
                        let idx = n * num_features * spatial_size + c * spatial_size + s;
                        let normalized = (input.data[idx] - mean) / std;
                        output[idx] = self.weight.data[c] * normalized + self.bias.data[c];
                    }
                }
            }
        }

        Variable::new(output, input.shape.clone(), true)
    }

    fn parameters(&self) -> Vec<&Variable> {
        vec![&self.weight, &self.bias]
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        vec![&mut self.weight, &mut self.bias]
    }

    fn set_trainable(&mut self, trainable: bool) {
        self.weight.requires_grad = trainable;
        self.bias.requires_grad = trainable;
    }

    fn reset_parameters(&mut self) {
        self.weight.data = vec![1.0; self.num_features];
        self.bias.data = vec![0.0; self.num_features];
        self.running_mean = vec![0.0; self.num_features];
        self.running_var = vec![1.0; self.num_features];
    }
}

// ============================================================================
// Group Normalization
// ============================================================================

/// Group Normalization layer
///
/// Divides channels into groups and normalizes within each group.
/// Works well with small batch sizes unlike BatchNorm.
#[derive(Debug, Clone)]
pub struct GroupNorm {
    name: String,
    num_groups: usize,
    num_channels: usize,
    eps: f32,
    weight: Variable,
    bias: Variable,
}

impl GroupNorm {
    /// Create a new GroupNorm layer
    pub fn new(num_groups: usize, num_channels: usize) -> Self {
        assert!(
            num_channels % num_groups == 0,
            "num_channels must be divisible by num_groups"
        );

        Self {
            name: format!("group_norm_{}_{}", num_groups, num_channels),
            num_groups,
            num_channels,
            eps: 1e-5,
            weight: Variable::new(vec![1.0; num_channels], vec![num_channels], true),
            bias: Variable::new(vec![0.0; num_channels], vec![num_channels], true),
        }
    }

    /// Set epsilon
    pub fn with_eps(mut self, eps: f32) -> Self {
        self.eps = eps;
        self
    }
}

impl Layer for GroupNorm {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], _tape: &mut GradientTape) -> Variable {
        let input = inputs[0];
        let batch_size = input.shape[0];
        let num_channels = input.shape[1];
        let spatial_size: usize = input.shape[2..].iter().product::<usize>().max(1);

        let channels_per_group = num_channels / self.num_groups;
        let group_size = channels_per_group * spatial_size;

        let mut output = vec![0.0; input.data.len()];

        for n in 0..batch_size {
            for g in 0..self.num_groups {
                // Compute group mean
                let mut mean = 0.0;
                for c in 0..channels_per_group {
                    let channel = g * channels_per_group + c;
                    for s in 0..spatial_size {
                        let idx = n * num_channels * spatial_size + channel * spatial_size + s;
                        mean += input.data[idx];
                    }
                }
                mean /= group_size as f32;

                // Compute group variance
                let mut var = 0.0;
                for c in 0..channels_per_group {
                    let channel = g * channels_per_group + c;
                    for s in 0..spatial_size {
                        let idx = n * num_channels * spatial_size + channel * spatial_size + s;
                        let diff = input.data[idx] - mean;
                        var += diff * diff;
                    }
                }
                var /= group_size as f32;

                // Normalize and apply affine
                let std = (var + self.eps).sqrt();
                for c in 0..channels_per_group {
                    let channel = g * channels_per_group + c;
                    for s in 0..spatial_size {
                        let idx = n * num_channels * spatial_size + channel * spatial_size + s;
                        let normalized = (input.data[idx] - mean) / std;
                        output[idx] =
                            self.weight.data[channel] * normalized + self.bias.data[channel];
                    }
                }
            }
        }

        Variable::new(output, input.shape.clone(), true)
    }

    fn parameters(&self) -> Vec<&Variable> {
        vec![&self.weight, &self.bias]
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        vec![&mut self.weight, &mut self.bias]
    }

    fn set_trainable(&mut self, trainable: bool) {
        self.weight.requires_grad = trainable;
        self.bias.requires_grad = trainable;
    }

    fn reset_parameters(&mut self) {
        self.weight.data = vec![1.0; self.num_channels];
        self.bias.data = vec![0.0; self.num_channels];
    }
}

// ============================================================================
// RMS Normalization
// ============================================================================

/// RMS Normalization layer
///
/// A simpler normalization that only uses root mean square,
/// used in models like LLaMA. More efficient than LayerNorm.
#[derive(Debug, Clone)]
pub struct RMSNorm {
    name: String,
    dim: usize,
    eps: f32,
    weight: Variable,
}

impl RMSNorm {
    /// Create a new RMSNorm layer
    pub fn new(dim: usize) -> Self {
        Self {
            name: format!("rms_norm_{}", dim),
            dim,
            eps: 1e-6,
            weight: Variable::new(vec![1.0; dim], vec![dim], true),
        }
    }

    /// Set epsilon
    pub fn with_eps(mut self, eps: f32) -> Self {
        self.eps = eps;
        self
    }
}

impl Layer for RMSNorm {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], _tape: &mut GradientTape) -> Variable {
        let input = inputs[0];

        // Input shape: [..., dim]
        let last_dim = *input.shape.last().expect("RMSNorm input shape must have >= 1 dim");
        assert_eq!(last_dim, self.dim);

        let num_vectors = input.data.len() / self.dim;
        let mut output = vec![0.0; input.data.len()];

        for i in 0..num_vectors {
            let start = i * self.dim;
            let end = start + self.dim;

            // Compute RMS
            let sum_sq: f32 = input.data[start..end].iter().map(|x| x * x).sum();
            let rms = (sum_sq / self.dim as f32 + self.eps).sqrt();

            // Normalize and scale
            for j in 0..self.dim {
                output[start + j] = input.data[start + j] / rms * self.weight.data[j];
            }
        }

        Variable::new(output, input.shape.clone(), true)
    }

    fn parameters(&self) -> Vec<&Variable> {
        vec![&self.weight]
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        vec![&mut self.weight]
    }

    fn set_trainable(&mut self, trainable: bool) {
        self.weight.requires_grad = trainable;
    }

    fn reset_parameters(&mut self) {
        self.weight.data = vec![1.0; self.dim];
    }
}

// ============================================================================
// Dropout
// ============================================================================

/// Dropout layer for regularization
#[derive(Debug, Clone)]
pub struct Dropout {
    name: String,
    p: f32,
    training: bool,
    seed: u64,
}

impl Dropout {
    /// Create a new Dropout layer
    pub fn new(p: f32) -> Self {
        assert!(
            (0.0..1.0).contains(&p),
            "Dropout probability must be in [0, 1)"
        );
        Self {
            name: format!("dropout_{}", (p * 100.0) as u32),
            p,
            training: true,
            seed: 42,
        }
    }

    /// Set training mode
    pub fn train(&mut self, mode: bool) {
        self.training = mode;
    }
}

impl Layer for Dropout {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], _tape: &mut GradientTape) -> Variable {
        let input = inputs[0];

        if !self.training || self.p == 0.0 {
            return input.clone();
        }

        let scale = 1.0 / (1.0 - self.p);
        let mut output = vec![0.0; input.data.len()];

        // Simple PRNG for deterministic dropout
        let mut seed = self.seed;
        for (out, &inp) in output.iter_mut().zip(input.data.iter()) {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let rand = (seed >> 33) as f32 / (1u64 << 31) as f32;

            if rand >= self.p {
                *out = inp * scale;
            }
        }

        Variable::new(output, input.shape.clone(), input.requires_grad)
    }

    fn parameters(&self) -> Vec<&Variable> {
        vec![]
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        vec![]
    }

    fn set_trainable(&mut self, _trainable: bool) {}

    fn reset_parameters(&mut self) {}
}

// ============================================================================
// LSTM Cell
// ============================================================================

/// LSTM (Long Short-Term Memory) cell
///
/// Processes sequences while maintaining long-term dependencies
/// through cell state and hidden state.
#[derive(Debug, Clone)]
pub struct LSTMCell {
    name: String,
    input_size: usize,
    hidden_size: usize,

    /// Input-hidden weights [4*hidden, input]
    weight_ih: Variable,
    /// Hidden-hidden weights [4*hidden, hidden]
    weight_hh: Variable,
    /// Input-hidden bias [4*hidden]
    bias_ih: Variable,
    /// Hidden-hidden bias [4*hidden]
    bias_hh: Variable,
}

impl LSTMCell {
    /// Create a new LSTM cell
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        let gate_size = 4 * hidden_size; // i, f, g, o gates

        // Xavier initialization
        let bound_ih = (6.0 / (input_size + hidden_size) as f32).sqrt();
        let bound_hh = (6.0 / (hidden_size + hidden_size) as f32).sqrt();

        let mut seed: u64 = 42;
        let mut rand_uniform = |bound: f32, size: usize| -> Vec<f32> {
            (0..size)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let u = (seed >> 33) as f32 / (1u64 << 31) as f32;
                    u * 2.0 * bound - bound
                })
                .collect()
        };

        Self {
            name: format!("lstm_cell_{}_{}", input_size, hidden_size),
            input_size,
            hidden_size,
            weight_ih: Variable::new(
                rand_uniform(bound_ih, gate_size * input_size),
                vec![gate_size, input_size],
                true,
            ),
            weight_hh: Variable::new(
                rand_uniform(bound_hh, gate_size * hidden_size),
                vec![gate_size, hidden_size],
                true,
            ),
            bias_ih: Variable::new(vec![0.0; gate_size], vec![gate_size], true),
            bias_hh: Variable::new(vec![0.0; gate_size], vec![gate_size], true),
        }
    }

    /// Forward pass for a single timestep
    ///
    /// # Arguments
    /// * `x` - Input [batch, input_size]
    /// * `hx` - Previous hidden state [batch, hidden_size]
    /// * `cx` - Previous cell state [batch, hidden_size]
    ///
    /// # Returns
    /// (new_hidden, new_cell)
    pub fn forward_step(&self, x: &Variable, hx: &Variable, cx: &Variable) -> (Variable, Variable) {
        let batch = x.shape[0];

        // gates = x @ W_ih^T + h @ W_hh^T + b_ih + b_hh
        let mut gates = vec![0.0; batch * 4 * self.hidden_size];

        for b in 0..batch {
            for g in 0..(4 * self.hidden_size) {
                let mut sum = self.bias_ih.data[g] + self.bias_hh.data[g];

                // x @ W_ih^T
                for i in 0..self.input_size {
                    sum += x.data[b * self.input_size + i]
                        * self.weight_ih.data[g * self.input_size + i];
                }

                // h @ W_hh^T
                for h in 0..self.hidden_size {
                    sum += hx.data[b * self.hidden_size + h]
                        * self.weight_hh.data[g * self.hidden_size + h];
                }

                gates[b * 4 * self.hidden_size + g] = sum;
            }
        }

        // Split gates and apply activations
        let mut new_h = vec![0.0; batch * self.hidden_size];
        let mut new_c = vec![0.0; batch * self.hidden_size];

        for b in 0..batch {
            for h in 0..self.hidden_size {
                let base = b * 4 * self.hidden_size;

                // Input gate (sigmoid)
                let i_gate = 1.0 / (1.0 + (-gates[base + h]).exp());
                // Forget gate (sigmoid)
                let f_gate = 1.0 / (1.0 + (-gates[base + self.hidden_size + h]).exp());
                // Cell gate (tanh)
                let g_gate = gates[base + 2 * self.hidden_size + h].tanh();
                // Output gate (sigmoid)
                let o_gate = 1.0 / (1.0 + (-gates[base + 3 * self.hidden_size + h]).exp());

                // New cell state
                let old_c = cx.data[b * self.hidden_size + h];
                let c = f_gate * old_c + i_gate * g_gate;

                // New hidden state
                let h_new = o_gate * c.tanh();

                new_c[b * self.hidden_size + h] = c;
                new_h[b * self.hidden_size + h] = h_new;
            }
        }

        (
            Variable::new(new_h, vec![batch, self.hidden_size], true),
            Variable::new(new_c, vec![batch, self.hidden_size], true),
        )
    }

    /// Get hidden size
    pub fn hidden_size(&self) -> usize {
        self.hidden_size
    }
}

impl Layer for LSTMCell {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], _tape: &mut GradientTape) -> Variable {
        // Expects [x, h, c] or just [x] with zero initial state
        let x = inputs[0];
        let batch = x.shape[0];

        let (hx, cx) = if inputs.len() >= 3 {
            (inputs[1].clone(), inputs[2].clone())
        } else {
            (
                Variable::new(
                    vec![0.0; batch * self.hidden_size],
                    vec![batch, self.hidden_size],
                    false,
                ),
                Variable::new(
                    vec![0.0; batch * self.hidden_size],
                    vec![batch, self.hidden_size],
                    false,
                ),
            )
        };

        let (new_h, _new_c) = self.forward_step(x, &hx, &cx);
        new_h
    }

    fn parameters(&self) -> Vec<&Variable> {
        vec![
            &self.weight_ih,
            &self.weight_hh,
            &self.bias_ih,
            &self.bias_hh,
        ]
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        vec![
            &mut self.weight_ih,
            &mut self.weight_hh,
            &mut self.bias_ih,
            &mut self.bias_hh,
        ]
    }

    fn set_trainable(&mut self, trainable: bool) {
        self.weight_ih.requires_grad = trainable;
        self.weight_hh.requires_grad = trainable;
        self.bias_ih.requires_grad = trainable;
        self.bias_hh.requires_grad = trainable;
    }

    fn reset_parameters(&mut self) {
        // Re-initialize with Xavier
        let gate_size = 4 * self.hidden_size;
        let bound_ih = (6.0 / (self.input_size + self.hidden_size) as f32).sqrt();
        let bound_hh = (6.0 / (2 * self.hidden_size) as f32).sqrt();

        let mut seed: u64 = 42;
        let mut rand = |bound: f32| -> f32 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u = (seed >> 33) as f32 / (1u64 << 31) as f32;
            u * 2.0 * bound - bound
        };

        for i in 0..self.weight_ih.data.len() {
            self.weight_ih.data[i] = rand(bound_ih);
        }
        for i in 0..self.weight_hh.data.len() {
            self.weight_hh.data[i] = rand(bound_hh);
        }
        self.bias_ih.data = vec![0.0; gate_size];
        self.bias_hh.data = vec![0.0; gate_size];
    }
}

// ============================================================================
// GRU Cell
// ============================================================================

/// GRU (Gated Recurrent Unit) cell
///
/// A simpler alternative to LSTM with fewer parameters.
/// Uses reset and update gates.
#[derive(Debug, Clone)]
pub struct GRUCell {
    name: String,
    input_size: usize,
    hidden_size: usize,

    /// Input-hidden weights [3*hidden, input]
    weight_ih: Variable,
    /// Hidden-hidden weights [3*hidden, hidden]
    weight_hh: Variable,
    /// Input-hidden bias [3*hidden]
    bias_ih: Variable,
    /// Hidden-hidden bias [3*hidden]
    bias_hh: Variable,
}

impl GRUCell {
    /// Create a new GRU cell
    pub fn new(input_size: usize, hidden_size: usize) -> Self {
        let gate_size = 3 * hidden_size; // r, z, n gates

        let bound_ih = (6.0 / (input_size + hidden_size) as f32).sqrt();
        let bound_hh = (6.0 / (2 * hidden_size) as f32).sqrt();

        let mut seed: u64 = 123;
        let mut rand_uniform = |bound: f32, size: usize| -> Vec<f32> {
            (0..size)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let u = (seed >> 33) as f32 / (1u64 << 31) as f32;
                    u * 2.0 * bound - bound
                })
                .collect()
        };

        Self {
            name: format!("gru_cell_{}_{}", input_size, hidden_size),
            input_size,
            hidden_size,
            weight_ih: Variable::new(
                rand_uniform(bound_ih, gate_size * input_size),
                vec![gate_size, input_size],
                true,
            ),
            weight_hh: Variable::new(
                rand_uniform(bound_hh, gate_size * hidden_size),
                vec![gate_size, hidden_size],
                true,
            ),
            bias_ih: Variable::new(vec![0.0; gate_size], vec![gate_size], true),
            bias_hh: Variable::new(vec![0.0; gate_size], vec![gate_size], true),
        }
    }

    /// Forward pass for a single timestep
    pub fn forward_step(&self, x: &Variable, hx: &Variable) -> Variable {
        let batch = x.shape[0];
        let h = self.hidden_size;

        // Compute gates
        let mut gates_ih = vec![0.0; batch * 3 * h];
        let mut gates_hh = vec![0.0; batch * 3 * h];

        for b in 0..batch {
            for g in 0..(3 * h) {
                // x @ W_ih^T + b_ih
                let mut sum_ih = self.bias_ih.data[g];
                for i in 0..self.input_size {
                    sum_ih += x.data[b * self.input_size + i]
                        * self.weight_ih.data[g * self.input_size + i];
                }
                gates_ih[b * 3 * h + g] = sum_ih;

                // h @ W_hh^T + b_hh
                let mut sum_hh = self.bias_hh.data[g];
                for hi in 0..h {
                    sum_hh += hx.data[b * h + hi] * self.weight_hh.data[g * h + hi];
                }
                gates_hh[b * 3 * h + g] = sum_hh;
            }
        }

        let mut new_h = vec![0.0; batch * h];

        for b in 0..batch {
            for hi in 0..h {
                let base_ih = b * 3 * h;
                let base_hh = b * 3 * h;

                // Reset gate: r = sigmoid(x @ W_ir + b_ir + h @ W_hr + b_hr)
                let r = 1.0 / (1.0 + (-(gates_ih[base_ih + hi] + gates_hh[base_hh + hi])).exp());

                // Update gate: z = sigmoid(x @ W_iz + b_iz + h @ W_hz + b_hz)
                let z = 1.0
                    / (1.0 + (-(gates_ih[base_ih + h + hi] + gates_hh[base_hh + h + hi])).exp());

                // New gate: n = tanh(x @ W_in + b_in + r * (h @ W_hn + b_hn))
                let n =
                    (gates_ih[base_ih + 2 * h + hi] + r * gates_hh[base_hh + 2 * h + hi]).tanh();

                // New hidden: h' = (1 - z) * n + z * h
                new_h[b * h + hi] = (1.0 - z) * n + z * hx.data[b * h + hi];
            }
        }

        Variable::new(new_h, vec![batch, h], true)
    }

    /// Get hidden size
    pub fn hidden_size(&self) -> usize {
        self.hidden_size
    }
}

impl Layer for GRUCell {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], _tape: &mut GradientTape) -> Variable {
        let x = inputs[0];
        let batch = x.shape[0];

        let hx = if inputs.len() >= 2 {
            inputs[1].clone()
        } else {
            Variable::new(
                vec![0.0; batch * self.hidden_size],
                vec![batch, self.hidden_size],
                false,
            )
        };

        self.forward_step(x, &hx)
    }

    fn parameters(&self) -> Vec<&Variable> {
        vec![
            &self.weight_ih,
            &self.weight_hh,
            &self.bias_ih,
            &self.bias_hh,
        ]
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        vec![
            &mut self.weight_ih,
            &mut self.weight_hh,
            &mut self.bias_ih,
            &mut self.bias_hh,
        ]
    }

    fn set_trainable(&mut self, trainable: bool) {
        self.weight_ih.requires_grad = trainable;
        self.weight_hh.requires_grad = trainable;
        self.bias_ih.requires_grad = trainable;
        self.bias_hh.requires_grad = trainable;
    }

    fn reset_parameters(&mut self) {
        // Similar to LSTM
    }
}

// ============================================================================
// Transformer Building Blocks
// ============================================================================

/// Feed-Forward Network (MLP) used in Transformers
#[derive(Debug, Clone)]
pub struct FeedForward {
    name: String,
    linear1: Linear,
    linear2: Linear,
    /// Dropout layer
    dropout: Dropout,
    /// Activation function
    activation: Activation,
}

/// Activation function enum for feed-forward networks
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Activation {
    /// Rectified Linear Unit
    ReLU,
    /// Gaussian Error Linear Unit
    GELU,
    /// Sigmoid Linear Unit
    SiLU,
    /// Hyperbolic tangent
    Tanh,
}

impl FeedForward {
    /// Create a new feed-forward network
    ///
    /// # Arguments
    /// * `d_model` - Input/output dimension
    /// * `d_ff` - Hidden dimension (typically 4 * d_model)
    /// * `dropout` - Dropout probability
    pub fn new(d_model: usize, d_ff: usize, dropout: f32) -> Self {
        Self {
            name: format!("ffn_{}_{}", d_model, d_ff),
            linear1: Linear::new(d_model, d_ff),
            linear2: Linear::new(d_ff, d_model),
            dropout: Dropout::new(dropout),
            activation: Activation::GELU,
        }
    }

    /// Set activation function
    pub fn with_activation(mut self, activation: Activation) -> Self {
        self.activation = activation;
        self
    }

    /// Apply activation function
    fn apply_activation(&self, x: &Variable) -> Variable {
        let mut output = vec![0.0; x.data.len()];

        match self.activation {
            Activation::ReLU => {
                for (out, &inp) in output.iter_mut().zip(x.data.iter()) {
                    *out = inp.max(0.0);
                }
            }
            Activation::GELU => {
                // Approximate GELU
                for (out, &x_val) in output.iter_mut().zip(x.data.iter()) {
                    *out = 0.5
                        * x_val
                        * (1.0 + (0.797_884_6 * (x_val + 0.044715 * x_val.powi(3))).tanh());
                }
            }
            Activation::SiLU => {
                // SiLU (Swish): x * sigmoid(x)
                for (out, &x_val) in output.iter_mut().zip(x.data.iter()) {
                    *out = x_val / (1.0 + (-x_val).exp());
                }
            }
            Activation::Tanh => {
                for (out, &inp) in output.iter_mut().zip(x.data.iter()) {
                    *out = inp.tanh();
                }
            }
        }

        Variable::new(output, x.shape.clone(), x.requires_grad)
    }
}

impl Layer for FeedForward {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], tape: &mut GradientTape) -> Variable {
        let x = inputs[0];

        // x -> Linear1 -> Activation -> Dropout -> Linear2
        let h = self.linear1.forward(&[x], tape);
        let h = self.apply_activation(&h);
        let h = self.dropout.forward(&[&h], tape);
        self.linear2.forward(&[&h], tape)
    }

    fn parameters(&self) -> Vec<&Variable> {
        let mut params = Vec::new();
        params.extend(self.linear1.parameters());
        params.extend(self.linear2.parameters());
        params
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        let mut params = Vec::new();
        params.extend(self.linear1.parameters_mut());
        params.extend(self.linear2.parameters_mut());
        params
    }

    fn set_trainable(&mut self, trainable: bool) {
        self.linear1.set_trainable(trainable);
        self.linear2.set_trainable(trainable);
    }

    fn reset_parameters(&mut self) {
        self.linear1.reset_parameters();
        self.linear2.reset_parameters();
    }
}

// ============================================================================
// Residual Block
// ============================================================================

/// Residual connection block
///
/// Applies a sublayer with residual connection and layer normalization.
/// Supports both Pre-LN and Post-LN configurations.
#[derive(Debug, Clone)]
pub struct ResidualBlock {
    name: String,
    #[allow(dead_code)]
    dim: usize,
    norm: LayerNorm,
    dropout: Dropout,
    pre_norm: bool,
}

impl ResidualBlock {
    /// Create a new residual block
    pub fn new(dim: usize, dropout: f32) -> Self {
        Self {
            name: format!("residual_{}", dim),
            dim,
            norm: LayerNorm::new(vec![dim]),
            dropout: Dropout::new(dropout),
            pre_norm: true, // Pre-LN by default (more stable)
        }
    }

    /// Use Post-LN instead of Pre-LN
    pub fn with_post_norm(mut self) -> Self {
        self.pre_norm = false;
        self
    }

    /// Forward with a sublayer function
    /// Returns: x + dropout(sublayer(norm(x))) for Pre-LN
    /// Returns: norm(x + dropout(sublayer(x))) for Post-LN
    pub fn forward_with<F>(&self, x: &Variable, sublayer: F, tape: &mut GradientTape) -> Variable
    where
        F: FnOnce(&Variable, &mut GradientTape) -> Variable,
    {
        if self.pre_norm {
            // Pre-LN: x + dropout(sublayer(norm(x)))
            let normalized = self.norm.forward(&[x], tape);
            let sublayer_out = sublayer(&normalized, tape);
            let dropped = self.dropout.forward(&[&sublayer_out], tape);

            // Add residual
            let mut output = vec![0.0; x.data.len()];
            for ((out, &xv), &dv) in output
                .iter_mut()
                .zip(x.data.iter())
                .zip(dropped.data.iter())
            {
                *out = xv + dv;
            }
            Variable::new(output, x.shape.clone(), true)
        } else {
            // Post-LN: norm(x + dropout(sublayer(x)))
            let sublayer_out = sublayer(x, tape);
            let dropped = self.dropout.forward(&[&sublayer_out], tape);

            // Add residual
            let mut sum = vec![0.0; x.data.len()];
            for ((s, &xv), &dv) in sum.iter_mut().zip(x.data.iter()).zip(dropped.data.iter()) {
                *s = xv + dv;
            }
            let sum_var = Variable::new(sum, x.shape.clone(), true);

            self.norm.forward(&[&sum_var], tape)
        }
    }
}

impl Layer for ResidualBlock {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], tape: &mut GradientTape) -> Variable {
        // Identity sublayer for basic forward
        self.forward_with(inputs[0], |x, _| x.clone(), tape)
    }

    fn parameters(&self) -> Vec<&Variable> {
        self.norm.parameters()
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        self.norm.parameters_mut()
    }

    fn set_trainable(&mut self, trainable: bool) {
        self.norm.set_trainable(trainable);
    }

    fn reset_parameters(&mut self) {
        self.norm.reset_parameters();
    }
}

// ============================================================================
// Embedding Layer
// ============================================================================

/// Embedding layer for discrete tokens
#[derive(Debug, Clone)]
pub struct Embedding {
    name: String,
    num_embeddings: usize,
    embedding_dim: usize,
    weight: Variable,
    padding_idx: Option<usize>,
}

impl Embedding {
    /// Create a new embedding layer
    pub fn new(num_embeddings: usize, embedding_dim: usize) -> Self {
        // Initialize with normal distribution
        let mut seed: u64 = 42;
        let weight_data: Vec<f32> = (0..num_embeddings * embedding_dim)
            .map(|_| {
                // Box-Muller transform for normal distribution
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let u1 = ((seed >> 33) as f32 / (1u64 << 31) as f32).max(1e-10);
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let u2 = (seed >> 33) as f32 / (1u64 << 31) as f32;
                (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
            })
            .collect();

        Self {
            name: format!("embedding_{}_{}", num_embeddings, embedding_dim),
            num_embeddings,
            embedding_dim,
            weight: Variable::new(weight_data, vec![num_embeddings, embedding_dim], true),
            padding_idx: None,
        }
    }

    /// Set padding index (embedding will be zeros)
    pub fn with_padding_idx(mut self, idx: usize) -> Self {
        self.padding_idx = Some(idx);
        // Zero out padding embedding
        for i in 0..self.embedding_dim {
            self.weight.data[idx * self.embedding_dim + i] = 0.0;
        }
        self
    }

    /// Lookup embeddings for indices
    pub fn lookup(&self, indices: &[usize]) -> Variable {
        let batch_size = indices.len();
        let mut output = vec![0.0; batch_size * self.embedding_dim];

        for (i, &idx) in indices.iter().enumerate() {
            assert!(idx < self.num_embeddings, "Index {} out of bounds", idx);
            for d in 0..self.embedding_dim {
                output[i * self.embedding_dim + d] = self.weight.data[idx * self.embedding_dim + d];
            }
        }

        Variable::new(output, vec![batch_size, self.embedding_dim], true)
    }
}

impl Layer for Embedding {
    fn name(&self) -> &str {
        &self.name
    }

    fn forward(&self, inputs: &[&Variable], _tape: &mut GradientTape) -> Variable {
        // Expect indices as f32 (will be cast to usize)
        let indices: Vec<usize> = inputs[0].data.iter().map(|&x| x as usize).collect();
        self.lookup(&indices)
    }

    fn parameters(&self) -> Vec<&Variable> {
        vec![&self.weight]
    }

    fn parameters_mut(&mut self) -> Vec<&mut Variable> {
        vec![&mut self.weight]
    }

    fn set_trainable(&mut self, trainable: bool) {
        self.weight.requires_grad = trainable;
    }

    fn reset_parameters(&mut self) {
        // Re-initialize
        let mut seed: u64 = 42;
        for i in 0..self.weight.data.len() {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u1 = ((seed >> 33) as f32 / (1u64 << 31) as f32).max(1e-10);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u2 = (seed >> 33) as f32 / (1u64 << 31) as f32;
            self.weight.data[i] = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos();
        }

        // Zero padding if set
        if let Some(idx) = self.padding_idx {
            for i in 0..self.embedding_dim {
                self.weight.data[idx * self.embedding_dim + i] = 0.0;
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_norm() {
        let layer = BatchNorm::new(4);
        let input = Variable::new(
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            vec![2, 4],
            false,
        );
        let mut tape = GradientTape::new();

        let output = layer.forward(&[&input], &mut tape);
        assert_eq!(output.shape, vec![2, 4]);
    }

    #[test]
    fn test_group_norm() {
        let layer = GroupNorm::new(2, 4);
        let input = Variable::new(
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0],
            vec![2, 4],
            false,
        );
        let mut tape = GradientTape::new();

        let output = layer.forward(&[&input], &mut tape);
        assert_eq!(output.shape, vec![2, 4]);
    }

    #[test]
    fn test_rms_norm() {
        let layer = RMSNorm::new(4);
        let input = Variable::new(vec![1.0, 2.0, 3.0, 4.0], vec![1, 4], false);
        let mut tape = GradientTape::new();

        let output = layer.forward(&[&input], &mut tape);
        assert_eq!(output.shape, vec![1, 4]);
    }

    #[test]
    fn test_dropout() {
        let mut layer = Dropout::new(0.5);
        let input = Variable::new(vec![1.0; 100], vec![10, 10], false);
        let mut tape = GradientTape::new();

        // In training mode, some values should be dropped
        let output = layer.forward(&[&input], &mut tape);
        let zeros: usize = output.data.iter().filter(|&&x| x == 0.0).count();
        assert!(zeros > 0 && zeros < 100);

        // In eval mode, no values should be dropped
        layer.train(false);
        let output = layer.forward(&[&input], &mut tape);
        assert!(output.data.iter().all(|&x| x == 1.0));
    }

    #[test]
    fn test_lstm_cell() {
        let cell = LSTMCell::new(10, 20);
        let batch = 2;

        let x = Variable::new(vec![0.1; batch * 10], vec![batch, 10], false);
        let h = Variable::new(vec![0.0; batch * 20], vec![batch, 20], false);
        let c = Variable::new(vec![0.0; batch * 20], vec![batch, 20], false);

        let (new_h, new_c) = cell.forward_step(&x, &h, &c);

        assert_eq!(new_h.shape, vec![batch, 20]);
        assert_eq!(new_c.shape, vec![batch, 20]);
    }

    #[test]
    fn test_gru_cell() {
        let cell = GRUCell::new(10, 20);
        let batch = 2;

        let x = Variable::new(vec![0.1; batch * 10], vec![batch, 10], false);
        let h = Variable::new(vec![0.0; batch * 20], vec![batch, 20], false);

        let new_h = cell.forward_step(&x, &h);

        assert_eq!(new_h.shape, vec![batch, 20]);
    }

    #[test]
    fn test_feed_forward() {
        let ffn = FeedForward::new(64, 256, 0.0);
        let input = Variable::new(vec![0.1; 2 * 64], vec![2, 64], false);
        let mut tape = GradientTape::new();

        let output = ffn.forward(&[&input], &mut tape);
        assert_eq!(output.shape, vec![2, 64]);
    }

    #[test]
    fn test_embedding() {
        let emb = Embedding::new(100, 64);
        let indices = vec![1, 5, 10, 50];

        let output = emb.lookup(&indices);
        assert_eq!(output.shape, vec![4, 64]);
    }

    #[test]
    fn test_embedding_padding() {
        let emb = Embedding::new(100, 64).with_padding_idx(0);
        let indices = vec![0, 1, 2];

        let output = emb.lookup(&indices);

        // Padding embedding should be zeros
        for i in 0..64 {
            assert_eq!(output.data[i], 0.0);
        }
    }
}
