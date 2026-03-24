//! # std::neural — Neural Network Computation
//!
//! First-class neural network definitions, layers, activations, and training.
//! Networks are compiled, shape-checked, and hardware-targeted at compile time.

// ---------------------------------------------------------------------------
// Network trait
// ---------------------------------------------------------------------------

/// Trait implemented by all `net` definitions.
pub trait Network {
    /// Input tensor type.
    type Input;
    /// Output tensor type.
    type Output;

    /// Forward pass.
    pub fn forward(&self, input: Self::Input) -> Self::Output / gpu;

    /// Get all learnable parameters.
    pub fn params(&self) -> Vec<&Param<f32, _>>;

    /// Get mutable parameters for gradient updates.
    pub fn params_mut(&mut self) -> Vec<&mut Param<f32, _>>;

    /// Apply gradients with a learning rate.
    pub fn apply_grads(&mut self, grads: &[Tensor<f32, _>], lr: f64);

    /// Number of trainable parameters.
    pub fn param_count(&self) -> usize;

    /// Save model weights to a file.
    pub fn save(&self, path: &str) -> Result<(), IoError> / io;

    /// Load model weights from a file.
    pub fn load(&mut self, path: &str) -> Result<(), IoError> / io;
}

// ---------------------------------------------------------------------------
// Layer types
// ---------------------------------------------------------------------------

/// A dense (fully connected) layer.
pub struct Dense {
    weights: Param<f32, [_, _]>,
    bias: Param<f32, [_]>,
    activation: Activation,
}

/// A 2D convolutional layer.
pub struct Conv2d {
    kernel: Param<f32, [_, _, _, _]>,
    bias: Param<f32, [_]>,
    stride: usize,
    padding: usize,
}

/// A multi-head attention layer.
pub struct MultiHeadAttention {
    q_proj: Param<f32, [_, _]>,
    k_proj: Param<f32, [_, _]>,
    v_proj: Param<f32, [_, _]>,
    out_proj: Param<f32, [_, _]>,
    heads: usize,
}

/// An LSTM recurrent layer.
pub struct LSTM {
    W_ih: Param<f32, [_, _]>,
    W_hh: Param<f32, [_, _]>,
    bias: Param<f32, [_]>,
    hidden_size: usize,
}

/// Batch normalization layer.
pub struct BatchNorm {
    gamma: Param<f32, [_]>,
    beta: Param<f32, [_]>,
    running_mean: Tensor<f32, [_]>,
    running_var: Tensor<f32, [_]>,
}

/// Layer normalization.
pub struct LayerNorm {
    gamma: Param<f32, [_]>,
    beta: Param<f32, [_]>,
}

/// Dropout (training-time regularization).
pub struct Dropout {
    rate: f64,
}

/// Token embedding layer.
pub struct Embedding {
    table: Param<f32, [_, _]>,
    vocab_size: usize,
    embed_dim: usize,
}

// ---------------------------------------------------------------------------
// Activations
// ---------------------------------------------------------------------------

/// Activation functions.
pub enum Activation {
    ReLU,
    Sigmoid,
    Tanh,
    Softmax,
    GELU,
    Swish,
    LeakyReLU(f64),
    ELU(f64),
    SiLU,
    Mish,
    None,
}

// ---------------------------------------------------------------------------
// Optimizers
// ---------------------------------------------------------------------------

/// Adam optimizer.
pub struct Adam {
    pub lr: f64,
    pub betas: (f64, f64),
    pub eps: f64,
    pub weight_decay: f64,
}

/// SGD optimizer with optional momentum.
pub struct SGD {
    pub lr: f64,
    pub momentum: f64,
    pub dampening: f64,
}

/// AdamW optimizer (decoupled weight decay).
pub struct AdamW {
    pub lr: f64,
    pub betas: (f64, f64),
    pub eps: f64,
    pub weight_decay: f64,
}

// ---------------------------------------------------------------------------
// Loss functions
// ---------------------------------------------------------------------------

/// Compute cross-entropy loss.
pub fn cross_entropy<const B: usize, const C: usize>(
    logits: Tensor<f32, [B, C]>,
    targets: Tensor<i64, [B]>,
) -> Tensor<f32, []>;

/// Compute mean squared error loss.
pub fn mse_loss<const N: usize>(
    predictions: Tensor<f32, [N]>,
    targets: Tensor<f32, [N]>,
) -> Tensor<f32, []>;

/// Binary cross-entropy loss.
pub fn bce_loss<const N: usize>(
    predictions: Tensor<f32, [N]>,
    targets: Tensor<f32, [N]>,
) -> Tensor<f32, []>;

// ---------------------------------------------------------------------------
// Training metrics
// ---------------------------------------------------------------------------

/// Metrics reported during training.
pub struct Metrics {
    pub loss: f64,
    pub accuracy: f64,
    pub epoch: u32,
    pub batch: u32,
    pub lr: f64,
}

// ---------------------------------------------------------------------------
// Dataset
// ---------------------------------------------------------------------------

/// A training dataset.
pub struct Dataset<T> {
    data: Vec<T>,
    batch_size: usize,
}

impl<T> Dataset<T> {
    /// Load a named dataset.
    pub fn load(name: &str) -> Dataset<T> / io;

    /// Split into train/test sets.
    pub fn split(&self, ratio: f64) -> (Dataset<T>, Dataset<T>);

    /// Get test set.
    pub fn test(&self) -> &Dataset<T>;

    /// Iterate in batches.
    pub fn batches(&self, size: usize) -> impl Iterator<Item = &[T]>;
}
