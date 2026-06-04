//! Neural Primitives Module
//!
//! Provides machine-native building blocks for neural computation.
//! These primitives are designed for AI agents to compose, analyze,
//! and optimize neural architectures programmatically.
//!
//! # Modules
//!
//! - `primitives` - Core mathematical primitives with algebraic properties
//! - `autodiff` - Automatic differentiation engine with gradient tape
//! - `architecture` - DAG-based neural architecture representation
//! - `layers` - Standard neural network layers (Linear, Conv2d, Attention)
//! - `extended_layers` - Advanced layers (LSTM, GRU, BatchNorm, Embedding)
//! - `loss` - Loss functions (MSE, CrossEntropy, BCE, etc.)
//! - `optim` - Optimizers (SGD, Adam, AdamW, RMSprop)
//!
//! # Example
//!
//! ```
//! use rmi::neural::{Linear, GradientTape, Variable};
//! use rmi::neural::layers::Layer;
//!
//! // Create a simple network
//! let layer = Linear::new(784, 256);
//! let input = Variable::new(vec![0.1; 784], vec![1, 784], false);
//! let mut tape = GradientTape::new();
//!
//! let output = layer.forward(&[&input], &mut tape);
//! ```

pub mod architecture;
pub mod autodiff;
pub mod extended_layers;
pub mod layers;
pub mod loss;
pub mod optim;
pub mod primitives;
pub mod serialization;
pub mod federated;
pub mod training;

pub use architecture::{
    ArchitectureBuilder, ArchitectureEdge, ArchitectureNode, NetworkArchitecture,
};
pub use autodiff::{backward, grad, GradientTape, Variable};
pub use extended_layers::{
    Activation, BatchNorm, Dropout, Embedding, FeedForward, GRUCell, GroupNorm, LSTMCell, RMSNorm,
    ResidualBlock,
};
pub use layers::{Attention, Conv2d, Layer, LayerNorm, Linear, MultiHeadAttention};
pub use loss::{
    BCELoss, BCEWithLogitsLoss, CrossEntropyLoss, KLDivLoss, L1Loss, Loss, MSELoss, NLLLoss,
    Reduction, SmoothL1Loss,
};
pub use optim::{Adam, AdamW, CosineAnnealingLR, LRScheduler, Optimizer, RMSprop, StepLR, SGD};
pub use primitives::*;
pub use federated::{
    AggregationStrategy, FederatedConfig, FederatedHistory, FederatedTrainer, ParamSnapshot,
};
pub use training::{
    clip_grad_norm, Batch, DataLoader, Dataset, Trainer, TrainerConfig, TrainingHistory,
};
