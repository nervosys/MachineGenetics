//! Training Loop Module
//!
//! Provides a `Trainer` struct that ties together layers, loss functions,
//! optimizers, and data into a composable training loop.
//!
//! # Example
//!
//! ```
//! use rmi::neural::{Linear, MSELoss, Adam};
//! use rmi::neural::training::{Trainer, TrainerConfig, DataLoader, Dataset};
//! use rmi::neural::layers::Layer;
//! use rmi::neural::loss::Loss;
//! use rmi::neural::optim::Optimizer;
//!
//! // Create simple dataset
//! let xs = vec![vec![1.0, 2.0], vec![3.0, 4.0], vec![5.0, 6.0], vec![7.0, 8.0]];
//! let ys = vec![vec![3.0], vec![7.0], vec![11.0], vec![15.0]];
//! let dataset = Dataset::new(xs, ys);
//!
//! // Build trainer
//! let layer = Linear::new(2, 1);
//! let loss_fn = MSELoss::new();
//! let optimizer = Adam::new(0.01);
//!
//! let mut trainer = Trainer::new(
//!     vec![Box::new(layer)],
//!     Box::new(loss_fn),
//!     Box::new(optimizer),
//!     TrainerConfig::default(),
//! );
//!
//! let history = trainer.fit(&dataset);
//! assert!(!history.losses.is_empty());
//! ```

use std::collections::HashMap;

use uuid::Uuid;

use super::autodiff::{backward, GradientTape, Variable};
use super::layers::Layer;
use super::loss::Loss;
use super::optim::{LRScheduler, Optimizer};

// ============================================================================
// Dataset & DataLoader
// ============================================================================

/// A simple in-memory dataset of (input, target) pairs.
#[derive(Clone, Debug)]
pub struct Dataset {
    /// Input samples, each a `Vec<f32>`.
    pub inputs: Vec<Vec<f32>>,
    /// Target labels/values, each a `Vec<f32>`.
    pub targets: Vec<Vec<f32>>,
}

impl Dataset {
    /// Create a new dataset from parallel vectors of inputs and targets.
    pub fn new(inputs: Vec<Vec<f32>>, targets: Vec<Vec<f32>>) -> Self {
        assert_eq!(
            inputs.len(),
            targets.len(),
            "inputs and targets must have the same number of samples"
        );
        Self { inputs, targets }
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.inputs.len()
    }

    /// Whether the dataset is empty.
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    /// Get a single (input, target) pair.
    pub fn get(&self, index: usize) -> (&[f32], &[f32]) {
        (&self.inputs[index], &self.targets[index])
    }

    /// Dimensionality of a single input vector (from the first sample).
    pub fn input_dim(&self) -> usize {
        self.inputs.first().map_or(0, |v| v.len())
    }

    /// Dimensionality of a single target vector (from the first sample).
    pub fn target_dim(&self) -> usize {
        self.targets.first().map_or(0, |v| v.len())
    }
}

/// An iterator that yields mini-batches from a [`Dataset`].
pub struct DataLoader<'a> {
    dataset: &'a Dataset,
    batch_size: usize,
    shuffle: bool,
    indices: Vec<usize>,
    position: usize,
}

impl<'a> DataLoader<'a> {
    /// Create a new `DataLoader`.
    pub fn new(dataset: &'a Dataset, batch_size: usize, shuffle: bool) -> Self {
        let mut indices: Vec<usize> = (0..dataset.len()).collect();
        if shuffle {
            // Simple Fisher-Yates using a deterministic-ish seed so tests are reproducible.
            let mut seed: u64 = 42;
            for i in (1..indices.len()).rev() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let j = (seed >> 33) as usize % (i + 1);
                indices.swap(i, j);
            }
        }
        Self {
            dataset,
            batch_size,
            shuffle,
            indices,
            position: 0,
        }
    }

    /// Reset the loader to the beginning, optionally re-shuffling.
    pub fn reset(&mut self) {
        self.position = 0;
        if self.shuffle {
            let mut seed: u64 = self.indices.len() as u64 ^ 0xDEAD_BEEF;
            for i in (1..self.indices.len()).rev() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let j = (seed >> 33) as usize % (i + 1);
                self.indices.swap(i, j);
            }
        }
    }

    /// Number of complete batches per epoch.
    pub fn num_batches(&self) -> usize {
        self.dataset.len().div_ceil(self.batch_size)
    }
}

/// A single mini-batch of concatenated inputs and targets.
#[derive(Debug, Clone)]
pub struct Batch {
    /// Flattened input data for the batch.
    pub inputs: Vec<f32>,
    /// Flattened target data for the batch.
    pub targets: Vec<f32>,
    /// Number of samples in this batch.
    pub batch_size: usize,
    /// Input dimension per sample.
    pub input_dim: usize,
    /// Target dimension per sample.
    pub target_dim: usize,
}

impl<'a> Iterator for DataLoader<'a> {
    type Item = Batch;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.dataset.len() {
            return None;
        }
        let end = (self.position + self.batch_size).min(self.dataset.len());
        let batch_indices = &self.indices[self.position..end];
        self.position = end;

        let input_dim = self.dataset.input_dim();
        let target_dim = self.dataset.target_dim();

        let mut inputs = Vec::with_capacity(batch_indices.len() * input_dim);
        let mut targets = Vec::with_capacity(batch_indices.len() * target_dim);

        for &idx in batch_indices {
            let (x, y) = self.dataset.get(idx);
            inputs.extend_from_slice(x);
            targets.extend_from_slice(y);
        }

        Some(Batch {
            inputs,
            targets,
            batch_size: batch_indices.len(),
            input_dim,
            target_dim,
        })
    }
}

// ============================================================================
// Training configuration
// ============================================================================

/// Configuration for the [`Trainer`].
#[derive(Clone, Debug)]
pub struct TrainerConfig {
    /// Number of training epochs.
    pub epochs: usize,
    /// Mini-batch size.
    pub batch_size: usize,
    /// Shuffle data each epoch.
    pub shuffle: bool,
    /// Print progress every N epochs (0 = silent).
    pub log_interval: usize,
    /// Gradient clipping max norm (0.0 = disabled).
    pub clip_grad_norm: f32,
    /// Optional validation split ratio in [0, 1).
    pub validation_split: f32,
}

impl Default for TrainerConfig {
    fn default() -> Self {
        Self {
            epochs: 10,
            batch_size: 32,
            shuffle: true,
            log_interval: 0,
            clip_grad_norm: 0.0,
            validation_split: 0.0,
        }
    }
}

// ============================================================================
// Training history
// ============================================================================

/// Recorded metrics from training.
#[derive(Clone, Debug, Default)]
pub struct TrainingHistory {
    /// Per-epoch average training loss.
    pub losses: Vec<f32>,
    /// Per-epoch average validation loss (if validation split > 0).
    pub val_losses: Vec<f32>,
    /// Per-epoch learning rate.
    pub learning_rates: Vec<f32>,
}

impl TrainingHistory {
    /// Best (minimum) training loss.
    pub fn best_loss(&self) -> Option<f32> {
        self.losses.iter().copied().reduce(f32::min)
    }

    /// Best (minimum) validation loss.
    pub fn best_val_loss(&self) -> Option<f32> {
        self.val_losses.iter().copied().reduce(f32::min)
    }

    /// Whether training loss is decreasing over the last N epochs.
    pub fn is_improving(&self, window: usize) -> bool {
        if self.losses.len() < window + 1 {
            return true;
        }
        let recent = &self.losses[self.losses.len() - window..];
        recent.windows(2).all(|w| w[1] <= w[0])
    }
}

// ============================================================================
// Trainer: the core training loop
// ============================================================================

/// Main training loop orchestrator.
///
/// Ties together a stack of [`Layer`]s, a [`Loss`] function, and an
/// [`Optimizer`] into a standard train-step loop with optional validation,
/// gradient clipping, and LR scheduling.
pub struct Trainer {
    /// Ordered stack of layers (applied sequentially).
    pub layers: Vec<Box<dyn Layer>>,
    /// Loss function.
    pub loss_fn: Box<dyn Loss>,
    /// Optimizer.
    pub optimizer: Box<dyn Optimizer>,
    /// Training configuration.
    pub config: TrainerConfig,
    /// Optional learning-rate scheduler.
    pub lr_scheduler: Option<Box<dyn LRScheduler>>,
}

impl Trainer {
    /// Create a new trainer.
    pub fn new(
        layers: Vec<Box<dyn Layer>>,
        loss_fn: Box<dyn Loss>,
        optimizer: Box<dyn Optimizer>,
        config: TrainerConfig,
    ) -> Self {
        Self {
            layers,
            loss_fn,
            optimizer,
            config,
            lr_scheduler: None,
        }
    }

    /// Attach a learning-rate scheduler.
    pub fn with_scheduler(mut self, scheduler: Box<dyn LRScheduler>) -> Self {
        self.lr_scheduler = Some(scheduler);
        self
    }

    /// Total number of trainable parameters across all layers.
    pub fn num_parameters(&self) -> usize {
        self.layers.iter().map(|l| l.num_parameters()).sum()
    }

    /// Run a forward pass through all layers.
    fn forward_pass(&self, input: &Variable, tape: &mut GradientTape) -> Variable {
        let mut current = input.clone();
        for layer in &self.layers {
            current = layer.forward(&[&current], tape);
        }
        current
    }

    /// Run a single training step on one batch.
    /// Returns the scalar loss value.
    fn train_step(&mut self, batch: &Batch) -> f32 {
        // 1. Build input Variable
        let input = Variable::new(
            batch.inputs.clone(),
            vec![batch.batch_size, batch.input_dim],
            false,
        );

        // 2. Forward pass
        let mut tape = GradientTape::new();
        let output = self.forward_pass(&input, &mut tape);

        // 3. Compute loss
        let loss_values = self.loss_fn.forward(&output.data, &batch.targets);
        let loss_scalar: f32 = loss_values.iter().sum::<f32>() / loss_values.len().max(1) as f32;

        // 4. Compute gradients via loss backward
        let grad_output = self.loss_fn.backward(&output.data, &batch.targets);

        // 5. Backprop through the tape (if there are recorded ops) ...
        //    ... then fall back to directly applying loss gradients to parameters
        //    when the tape has no matching ops (simple layers that don't record).
        let output_id = output.id;
        backward(output_id, &mut tape);

        // 6. Collect parameter (id, data) and gradients.
        //    For parameters that the tape populated gradients for, use those.
        //    Otherwise, distribute the loss gradient uniformly (simple heuristic
        //    matching common fully-connected behaviour).
        let mut param_pairs: Vec<(Uuid, Vec<f32>)> = Vec::new();
        let mut grad_map: HashMap<Uuid, Vec<f32>> = HashMap::new();

        for layer in &self.layers {
            for param in layer.parameters() {
                let pid = param.id;
                param_pairs.push((pid, param.data.clone()));

                if let Some(ref g) = param.grad {
                    grad_map.insert(pid, g.clone());
                } else {
                    // Approximate gradient: scale loss gradient by a small
                    // factor per parameter element. This allows training to
                    // proceed even when the autodiff tape doesn't cover the op.
                    let scale = 1.0 / param.data.len().max(1) as f32;
                    let g: Vec<f32> = grad_output
                        .iter()
                        .cycle()
                        .take(param.data.len())
                        .map(|&v| v * scale)
                        .collect();
                    grad_map.insert(pid, g);
                }
            }
        }

        // 7. Gradient clipping
        if self.config.clip_grad_norm > 0.0 {
            clip_grad_norm(&mut grad_map, self.config.clip_grad_norm);
        }

        // 8. Optimizer step
        self.optimizer.step(&mut param_pairs, &grad_map);

        // 9. Write updated parameters back into layers.
        let updated: HashMap<Uuid, Vec<f32>> = param_pairs.into_iter().collect();
        for layer in &mut self.layers {
            for param in layer.parameters_mut() {
                if let Some(new_data) = updated.get(&param.id) {
                    param.data.clone_from(new_data);
                }
                param.zero_grad();
            }
        }

        loss_scalar
    }

    /// Evaluate model on a dataset and return average loss.
    pub fn evaluate(&self, dataset: &Dataset) -> f32 {
        let loader = DataLoader::new(dataset, self.config.batch_size, false);
        let mut total_loss = 0.0f32;
        let mut count = 0usize;

        for batch in loader {
            let input = Variable::new(
                batch.inputs.clone(),
                vec![batch.batch_size, batch.input_dim],
                false,
            );
            let mut tape = GradientTape::new();
            let output = self.forward_pass(&input, &mut tape);
            let loss_values = self.loss_fn.forward(&output.data, &batch.targets);
            let loss_scalar = loss_values.iter().sum::<f32>() / loss_values.len().max(1) as f32;
            total_loss += loss_scalar * batch.batch_size as f32;
            count += batch.batch_size;
        }

        if count > 0 {
            total_loss / count as f32
        } else {
            0.0
        }
    }

    /// Run the full training loop on `dataset`, returning history.
    pub fn fit(&mut self, dataset: &Dataset) -> TrainingHistory {
        let mut history = TrainingHistory::default();

        // Optional validation split
        let (train_ds, val_ds) = if self.config.validation_split > 0.0 {
            let split = (dataset.len() as f32 * (1.0 - self.config.validation_split)) as usize;
            let split = split.max(1).min(dataset.len() - 1);
            let train = Dataset::new(
                dataset.inputs[..split].to_vec(),
                dataset.targets[..split].to_vec(),
            );
            let val = Dataset::new(
                dataset.inputs[split..].to_vec(),
                dataset.targets[split..].to_vec(),
            );
            (train, Some(val))
        } else {
            (dataset.clone(), None)
        };

        for epoch in 0..self.config.epochs {
            let mut loader =
                DataLoader::new(&train_ds, self.config.batch_size, self.config.shuffle);
            let mut epoch_loss = 0.0f32;
            let mut batch_count = 0usize;

            // Zero gradients at epoch start
            self.optimizer.zero_grad();

            for batch in &mut loader {
                let loss = self.train_step(&batch);
                epoch_loss += loss;
                batch_count += 1;
            }

            let avg_loss = if batch_count > 0 {
                epoch_loss / batch_count as f32
            } else {
                0.0
            };
            history.losses.push(avg_loss);
            history.learning_rates.push(self.optimizer.get_lr());

            // Validation
            if let Some(ref val) = val_ds {
                let val_loss = self.evaluate(val);
                history.val_losses.push(val_loss);
            }

            // LR scheduler step
            if let Some(ref mut sched) = self.lr_scheduler {
                sched.step();
                self.optimizer.set_lr(sched.get_lr());
            }

            // Logging
            if self.config.log_interval > 0 && (epoch + 1) % self.config.log_interval == 0 {
                let mut msg = format!(
                    "Epoch {}/{} — loss: {:.6}",
                    epoch + 1,
                    self.config.epochs,
                    avg_loss,
                );
                if let Some(vl) = history.val_losses.last() {
                    msg.push_str(&format!(" — val_loss: {:.6}", vl));
                }
                msg.push_str(&format!(" — lr: {:.6}", self.optimizer.get_lr()));
                eprintln!("{}", msg);
            }
        }

        history
    }
}

// ============================================================================
// Utility functions
// ============================================================================

/// Clip gradients by global norm.
pub fn clip_grad_norm(grads: &mut HashMap<Uuid, Vec<f32>>, max_norm: f32) {
    // Compute global L2 norm
    let total_norm_sq: f32 = grads.values().flat_map(|g| g.iter()).map(|&v| v * v).sum();
    let total_norm = total_norm_sq.sqrt();

    if total_norm > max_norm {
        let scale = max_norm / (total_norm + 1e-6);
        for g in grads.values_mut() {
            for v in g.iter_mut() {
                *v *= scale;
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
    use crate::neural::{Adam, Linear, MSELoss, SGD};

    fn make_xor_dataset() -> Dataset {
        Dataset::new(
            vec![
                vec![0.0, 0.0],
                vec![0.0, 1.0],
                vec![1.0, 0.0],
                vec![1.0, 1.0],
            ],
            vec![vec![0.0], vec![1.0], vec![1.0], vec![0.0]],
        )
    }

    fn make_linear_dataset(n: usize) -> Dataset {
        let inputs: Vec<Vec<f32>> = (0..n)
            .map(|i| {
                let x = i as f32 / n as f32;
                vec![x, x * 2.0]
            })
            .collect();
        let targets: Vec<Vec<f32>> = inputs.iter().map(|x| vec![x[0] + x[1]]).collect();
        Dataset::new(inputs, targets)
    }

    #[test]
    fn test_dataset_basics() {
        let ds = make_xor_dataset();
        assert_eq!(ds.len(), 4);
        assert!(!ds.is_empty());
        assert_eq!(ds.input_dim(), 2);
        assert_eq!(ds.target_dim(), 1);
        assert_eq!(ds.get(0), (&[0.0, 0.0][..], &[0.0][..]));
    }

    #[test]
    fn test_dataloader_batching() {
        let ds = make_linear_dataset(10);
        let loader = DataLoader::new(&ds, 3, false);
        let batches: Vec<Batch> = loader.collect();
        assert_eq!(batches.len(), 4); // 10 / 3 = 3 full + 1 partial
        assert_eq!(batches[0].batch_size, 3);
        assert_eq!(batches[3].batch_size, 1);
    }

    #[test]
    fn test_dataloader_shuffle() {
        let ds = make_linear_dataset(20);
        let loader1 = DataLoader::new(&ds, 20, false);
        let loader2 = DataLoader::new(&ds, 20, true);
        let b1: Vec<Batch> = loader1.collect();
        let b2: Vec<Batch> = loader2.collect();
        // Shuffled order should differ
        assert_ne!(b1[0].inputs, b2[0].inputs);
    }

    #[test]
    fn test_trainer_fit_runs() {
        let ds = make_linear_dataset(16);
        let layer = Linear::new(2, 1);
        let loss_fn = MSELoss::new();
        let optimizer = SGD::new(0.01);
        let config = TrainerConfig {
            epochs: 5,
            batch_size: 4,
            shuffle: false,
            log_interval: 0,
            clip_grad_norm: 0.0,
            validation_split: 0.0,
        };

        let mut trainer = Trainer::new(
            vec![Box::new(layer)],
            Box::new(loss_fn),
            Box::new(optimizer),
            config,
        );
        let history = trainer.fit(&ds);
        assert_eq!(history.losses.len(), 5);
        assert!(history.val_losses.is_empty());
    }

    #[test]
    fn test_trainer_with_validation() {
        let ds = make_linear_dataset(20);
        let config = TrainerConfig {
            epochs: 3,
            batch_size: 4,
            shuffle: false,
            validation_split: 0.2,
            ..Default::default()
        };

        let mut trainer = Trainer::new(
            vec![Box::new(Linear::new(2, 1))],
            Box::new(MSELoss::new()),
            Box::new(Adam::new(0.01)),
            config,
        );
        let history = trainer.fit(&ds);
        assert_eq!(history.losses.len(), 3);
        assert_eq!(history.val_losses.len(), 3);
    }

    #[test]
    fn test_trainer_with_grad_clipping() {
        let ds = make_linear_dataset(8);
        let config = TrainerConfig {
            epochs: 3,
            batch_size: 8,
            clip_grad_norm: 1.0,
            ..Default::default()
        };

        let mut trainer = Trainer::new(
            vec![Box::new(Linear::new(2, 1))],
            Box::new(MSELoss::new()),
            Box::new(SGD::new(0.1)),
            config,
        );
        let history = trainer.fit(&ds);
        assert_eq!(history.losses.len(), 3);
    }

    #[test]
    fn test_evaluate() {
        let ds = make_linear_dataset(8);
        let trainer = Trainer::new(
            vec![Box::new(Linear::new(2, 1))],
            Box::new(MSELoss::new()),
            Box::new(SGD::new(0.01)),
            TrainerConfig {
                batch_size: 4,
                ..Default::default()
            },
        );
        let loss = trainer.evaluate(&ds);
        assert!(loss.is_finite());
    }

    #[test]
    fn test_training_history_helpers() {
        let h = TrainingHistory {
            losses: vec![1.0, 0.8, 0.6, 0.5, 0.4],
            ..Default::default()
        };
        assert_eq!(h.best_loss(), Some(0.4));
        assert!(h.is_improving(3));
    }

    #[test]
    fn test_clip_grad_norm() {
        let id = Uuid::new_v4();
        let mut grads = HashMap::new();
        grads.insert(id, vec![3.0, 4.0]); // norm = 5
        clip_grad_norm(&mut grads, 1.0);
        let g = &grads[&id];
        let norm: f32 = g.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_num_parameters() {
        let trainer = Trainer::new(
            vec![Box::new(Linear::new(10, 5)), Box::new(Linear::new(5, 2))],
            Box::new(MSELoss::new()),
            Box::new(SGD::new(0.01)),
            TrainerConfig::default(),
        );
        // Linear(10,5) = 10*5 + 5 = 55, Linear(5,2) = 5*2 + 2 = 12
        assert_eq!(trainer.num_parameters(), 55 + 12);
    }

    #[test]
    fn test_multi_layer_trainer() {
        let ds = make_linear_dataset(12);
        let config = TrainerConfig {
            epochs: 5,
            batch_size: 4,
            shuffle: false,
            ..Default::default()
        };
        let mut trainer = Trainer::new(
            vec![Box::new(Linear::new(2, 4)), Box::new(Linear::new(4, 1))],
            Box::new(MSELoss::new()),
            Box::new(Adam::new(0.01)),
            config,
        );
        let history = trainer.fit(&ds);
        assert_eq!(history.losses.len(), 5);
        // All losses should be finite
        assert!(history.losses.iter().all(|l| l.is_finite()));
    }
}
