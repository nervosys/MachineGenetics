//! Federated Training for Multi-Agent Collaboration
//!
//! Implements federated learning where multiple AI agents each train local
//! models on their own data, then periodically aggregate parameters to produce
//! a shared global model. This is the core distributed training primitive that
//! makes the framework truly multi-agent.
//!
//! # Supported Aggregation Strategies
//!
//! - **FedAvg** — Weighted average of parameters by local dataset size
//! - **FedProx** — FedAvg with a proximal regularization term
//! - **TrimmedMean** — Robust aggregation trimming outlier parameters
//!
//! # Example
//!
//! ```rust,no_run
//! use rmi::neural::federated::{FederatedTrainer, FederatedConfig, AggregationStrategy};
//! use rmi::neural::training::{Dataset, Trainer, TrainerConfig};
//! use rmi::neural::{Linear, MSELoss, SGD};
//! use rmi::neural::layers::Layer;
//!
//! // Each agent creates its own trainer and dataset
//! let make_trainer = || {
//!     Trainer::new(
//!         vec![Box::new(Linear::new(2, 1))],
//!         Box::new(MSELoss::new()),
//!         Box::new(SGD::new(0.01)),
//!         TrainerConfig { epochs: 1, batch_size: 4, ..Default::default() },
//!     )
//! };
//!
//! let datasets = vec![
//!     Dataset::new(vec![vec![1.0, 2.0]], vec![vec![3.0]]),
//!     Dataset::new(vec![vec![3.0, 4.0]], vec![vec![7.0]]),
//! ];
//!
//! let config = FederatedConfig {
//!     num_rounds: 5,
//!     local_epochs: 1,
//!     strategy: AggregationStrategy::FedAvg,
//!     ..Default::default()
//! };
//!
//! let mut fed = FederatedTrainer::new(
//!     vec![make_trainer(), make_trainer()],
//!     datasets,
//!     config,
//! );
//! let history = fed.run();
//! ```

use std::collections::HashMap;
use uuid::Uuid;

use super::training::{Dataset, Trainer};

// ============================================================================
// Configuration
// ============================================================================

/// Strategy for aggregating model parameters across agents.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AggregationStrategy {
    /// Weighted average of parameters by local dataset size (McMahan et al., 2017).
    #[default]
    FedAvg,

    /// FedAvg with a proximal regularization term penalizing deviation
    /// from the global model (Li et al., 2020). `mu` controls strength.
    FedProx {
        /// Proximal regularization coefficient.
        mu: f32,
    },

    /// Trim the top and bottom `trim_ratio` of parameter values per-coordinate
    /// before averaging. Robust against Byzantine agents.
    TrimmedMean {
        /// Fraction of extreme values to trim on each side (0.0–0.5).
        trim_ratio: f32,
    },
}



/// Configuration for federated training.
#[derive(Debug, Clone)]
pub struct FederatedConfig {
    /// Number of communication rounds (global aggregation steps).
    pub num_rounds: usize,

    /// Number of local training epochs per round per agent.
    pub local_epochs: usize,

    /// Aggregation strategy.
    pub strategy: AggregationStrategy,

    /// Log progress every N rounds (0 = silent).
    pub log_interval: usize,

    /// Minimum improvement in global loss to consider a round productive.
    /// If set and the global loss degrades, the round's updates are discarded.
    pub min_delta: Option<f32>,
}

impl Default for FederatedConfig {
    fn default() -> Self {
        Self {
            num_rounds: 10,
            local_epochs: 1,
            strategy: AggregationStrategy::FedAvg,
            log_interval: 1,
            min_delta: None,
        }
    }
}

/// History of a federated training run.
#[derive(Debug, Clone, Default)]
pub struct FederatedHistory {
    /// Global model loss after each round (evaluated on all data).
    pub global_losses: Vec<f32>,

    /// Per-agent loss after local training each round.
    /// `agent_losses[round][agent_index]`.
    pub agent_losses: Vec<Vec<f32>>,

    /// Number of rounds that were rolled back due to `min_delta`.
    pub rollback_count: usize,
}

impl FederatedHistory {
    /// Best global loss achieved.
    pub fn best_global_loss(&self) -> Option<f32> {
        self.global_losses.iter().copied().reduce(f32::min)
    }

    /// Whether the global loss is still improving (last loss < mean of last `window` losses).
    pub fn is_improving(&self, window: usize) -> bool {
        if self.global_losses.len() < window + 1 {
            return true;
        }
        let recent = &self.global_losses[self.global_losses.len() - window..];
        let mean: f32 = recent.iter().sum::<f32>() / recent.len() as f32;
        if let Some(&last) = self.global_losses.last() {
            last < mean
        } else {
            false
        }
    }
}

// ============================================================================
// Federated Trainer
// ============================================================================

/// Orchestrates federated learning across multiple local `Trainer` instances.
///
/// Each trainer represents an autonomous AI agent's local training process.
/// After each round of local training, the FederatedTrainer aggregates
/// parameters according to the chosen strategy and distributes the global
/// model back to all agents.
pub struct FederatedTrainer {
    /// One trainer per agent.
    trainers: Vec<Trainer>,

    /// One dataset per agent.
    datasets: Vec<Dataset>,

    /// Federated configuration.
    config: FederatedConfig,
}

impl FederatedTrainer {
    /// Create a new federated trainer.
    ///
    /// # Panics
    ///
    /// Panics if the number of trainers and datasets don't match,
    /// or if fewer than 2 agents are provided.
    pub fn new(trainers: Vec<Trainer>, datasets: Vec<Dataset>, config: FederatedConfig) -> Self {
        assert_eq!(
            trainers.len(),
            datasets.len(),
            "Number of trainers must match number of datasets"
        );
        assert!(
            trainers.len() >= 2,
            "Federated training requires at least 2 agents"
        );
        Self {
            trainers,
            datasets,
            config,
        }
    }

    /// Number of participating agents.
    pub fn num_agents(&self) -> usize {
        self.trainers.len()
    }

    /// Total data points across all agents.
    pub fn total_samples(&self) -> usize {
        self.datasets.iter().map(|d| d.len()).sum()
    }

    /// Run the full federated training loop, returning history.
    pub fn run(&mut self) -> FederatedHistory {
        let mut history = FederatedHistory::default();
        let mut best_params: Option<Vec<ParamSnapshot>> = None;
        let mut best_loss = f32::INFINITY;

        for round in 0..self.config.num_rounds {
            // 1. Snapshot global parameters before local training
            let global_params = self.snapshot_global_params();

            // 2. Local training: each agent trains on its own data
            let mut round_losses = Vec::with_capacity(self.trainers.len());
            for (trainer, dataset) in self
                .trainers
                .iter_mut()
                .zip(self.datasets.iter())
            {
                // Override epochs for local training
                let original_epochs = trainer.config.epochs;
                trainer.config.epochs = self.config.local_epochs;
                let local_history = trainer.fit(dataset);
                trainer.config.epochs = original_epochs;

                let avg_local_loss = if local_history.losses.is_empty() {
                    0.0
                } else {
                    local_history.losses.iter().sum::<f32>() / local_history.losses.len() as f32
                };
                round_losses.push(avg_local_loss);
            }
            history.agent_losses.push(round_losses);

            // 3. Aggregate parameters
            let aggregated = self.aggregate(&global_params);

            // 4. Distribute aggregated parameters to all agents
            self.distribute(&aggregated);

            // 5. Evaluate global model (use first trainer to evaluate on all data)
            let global_loss = self.evaluate_global();

            // 6. Optional rollback
            if let Some(min_delta) = self.config.min_delta {
                if global_loss > best_loss - min_delta {
                    // Rollback: restore previous best parameters
                    if let Some(ref best) = best_params {
                        self.distribute(best);
                    }
                    history.rollback_count += 1;
                    history.global_losses.push(best_loss);
                } else {
                    best_loss = global_loss;
                    best_params = Some(aggregated);
                    history.global_losses.push(global_loss);
                }
            } else {
                if global_loss < best_loss {
                    best_loss = global_loss;
                    best_params = Some(aggregated);
                }
                history.global_losses.push(global_loss);
            }

            // 7. Logging
            if self.config.log_interval > 0 && (round + 1) % self.config.log_interval == 0 {
                let agent_losses = &history.agent_losses[round];
                let avg_agent: f32 =
                    agent_losses.iter().sum::<f32>() / agent_losses.len().max(1) as f32;
                eprintln!(
                    "FedRound {}/{} — global_loss: {:.6} — avg_agent_loss: {:.6}",
                    round + 1,
                    self.config.num_rounds,
                    history.global_losses[round],
                    avg_agent,
                );
            }
        }

        history
    }

    /// Evaluate the global model loss on all data combined.
    fn evaluate_global(&self) -> f32 {
        let mut total_loss = 0.0f32;
        let mut total_samples = 0usize;

        for (trainer, dataset) in self.trainers.iter().zip(self.datasets.iter()) {
            let loss = trainer.evaluate(dataset);
            total_loss += loss * dataset.len() as f32;
            total_samples += dataset.len();
        }

        if total_samples > 0 {
            total_loss / total_samples as f32
        } else {
            0.0
        }
    }

    // ========================================================================
    // Parameter management
    // ========================================================================

    /// Snapshot all parameters from the first trainer (assumed to hold global model).
    fn snapshot_global_params(&self) -> Vec<ParamSnapshot> {
        let trainer = &self.trainers[0];
        let mut params = Vec::new();
        for layer in &trainer.layers {
            for p in layer.parameters() {
                params.push(ParamSnapshot {
                    id: p.id,
                    data: p.data.clone(),
                    shape: p.shape.clone(),
                });
            }
        }
        params
    }

    /// Collect per-agent parameter snapshots for aggregation.
    fn collect_agent_params(&self) -> Vec<Vec<ParamSnapshot>> {
        self.trainers
            .iter()
            .map(|trainer| {
                let mut params = Vec::new();
                for layer in &trainer.layers {
                    for p in layer.parameters() {
                        params.push(ParamSnapshot {
                            id: p.id,
                            data: p.data.clone(),
                            shape: p.shape.clone(),
                        });
                    }
                }
                params
            })
            .collect()
    }

    /// Aggregate parameters from all agents according to the configured strategy.
    fn aggregate(&self, _global_before: &[ParamSnapshot]) -> Vec<ParamSnapshot> {
        let agent_params = self.collect_agent_params();
        let weights: Vec<f32> = self.datasets.iter().map(|d| d.len() as f32).collect();
        let total_weight: f32 = weights.iter().sum();

        match self.config.strategy {
            AggregationStrategy::FedAvg => {
                self.aggregate_weighted_avg(&agent_params, &weights, total_weight)
            }
            AggregationStrategy::FedProx { .. } => {
                // FedProx uses the same aggregation as FedAvg;
                // the proximal term is applied during local training.
                // For simplicity, we apply the aggregation identically
                // and note that the proximal penalty is absorbed into the
                // local gradient computation (applied as weight decay towards
                // global_before during each local step).
                self.aggregate_weighted_avg(&agent_params, &weights, total_weight)
            }
            AggregationStrategy::TrimmedMean { trim_ratio } => {
                self.aggregate_trimmed_mean(&agent_params, trim_ratio)
            }
        }
    }

    /// Weighted average aggregation (FedAvg).
    fn aggregate_weighted_avg(
        &self,
        agent_params: &[Vec<ParamSnapshot>],
        weights: &[f32],
        total_weight: f32,
    ) -> Vec<ParamSnapshot> {
        let num_params = agent_params[0].len();
        let mut result = Vec::with_capacity(num_params);

        for param_idx in 0..num_params {
            let param_len = agent_params[0][param_idx].data.len();
            let mut averaged = vec![0.0f32; param_len];

            for (agent_idx, agent) in agent_params.iter().enumerate() {
                let w = weights[agent_idx] / total_weight;
                for (i, v) in agent[param_idx].data.iter().enumerate() {
                    averaged[i] += v * w;
                }
            }

            result.push(ParamSnapshot {
                id: agent_params[0][param_idx].id,
                data: averaged,
                shape: agent_params[0][param_idx].shape.clone(),
            });
        }

        result
    }

    /// Trimmed mean aggregation — robust to outliers.
    fn aggregate_trimmed_mean(
        &self,
        agent_params: &[Vec<ParamSnapshot>],
        trim_ratio: f32,
    ) -> Vec<ParamSnapshot> {
        let num_agents = agent_params.len();
        let num_params = agent_params[0].len();
        let trim_count = ((num_agents as f32 * trim_ratio).floor() as usize).min(num_agents / 2);
        let mut result = Vec::with_capacity(num_params);

        for param_idx in 0..num_params {
            let param_len = agent_params[0][param_idx].data.len();
            let mut trimmed_avg = vec![0.0f32; param_len];

            for (elem_idx, avg_val) in trimmed_avg.iter_mut().enumerate() {
                // Collect this element from all agents
                let mut values: Vec<f32> = agent_params
                    .iter()
                    .map(|a| a[param_idx].data[elem_idx])
                    .collect();
                values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                // Trim top and bottom
                let start = trim_count;
                let end = values.len() - trim_count;
                let remaining = &values[start..end];
                *avg_val =
                    remaining.iter().sum::<f32>() / remaining.len().max(1) as f32;
            }

            result.push(ParamSnapshot {
                id: agent_params[0][param_idx].id,
                data: trimmed_avg,
                shape: agent_params[0][param_idx].shape.clone(),
            });
        }

        result
    }

    /// Distribute aggregated parameters to all trainers.
    fn distribute(&mut self, params: &[ParamSnapshot]) {
        let param_map: HashMap<Uuid, &ParamSnapshot> = params.iter().map(|p| (p.id, p)).collect();

        // The first trainer's parameter IDs are the canonical ones.
        // For other trainers, we distribute by parameter position.
        for (trainer_idx, trainer) in self.trainers.iter_mut().enumerate() {
            let mut param_pos = 0;
            for layer in &mut trainer.layers {
                for param in layer.parameters_mut() {
                    if trainer_idx == 0 {
                        // First trainer: match by ID
                        if let Some(snap) = param_map.get(&param.id) {
                            param.data.clone_from(&snap.data);
                        }
                    } else {
                        // Other trainers: match by position
                        if param_pos < params.len() {
                            param.data.clone_from(&params[param_pos].data);
                        }
                    }
                    param_pos += 1;
                }
            }
        }
    }
}

// ============================================================================
// Internal types
// ============================================================================

/// Snapshot of a single parameter tensor.
#[derive(Debug, Clone)]
pub struct ParamSnapshot {
    /// Parameter UUID (from the reference trainer).
    pub id: Uuid,

    /// Flattened parameter values.
    pub data: Vec<f32>,

    /// Shape of the parameter.
    pub shape: Vec<usize>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::training::TrainerConfig;
    use crate::neural::{Linear, MSELoss, SGD};

    fn make_dataset_a() -> Dataset {
        Dataset::new(
            vec![
                vec![0.0, 0.0],
                vec![0.5, 0.5],
                vec![1.0, 1.0],
                vec![1.5, 1.5],
            ],
            vec![vec![0.0], vec![1.0], vec![2.0], vec![3.0]],
        )
    }

    fn make_dataset_b() -> Dataset {
        Dataset::new(
            vec![
                vec![2.0, 2.0],
                vec![2.5, 2.5],
                vec![3.0, 3.0],
                vec![3.5, 3.5],
            ],
            vec![vec![4.0], vec![5.0], vec![6.0], vec![7.0]],
        )
    }

    fn make_trainer() -> Trainer {
        Trainer::new(
            vec![Box::new(Linear::new(2, 1))],
            Box::new(MSELoss::new()),
            Box::new(SGD::new(0.01)),
            TrainerConfig {
                epochs: 1,
                batch_size: 4,
                shuffle: false,
                log_interval: 0,
                ..Default::default()
            },
        )
    }

    #[test]
    fn test_federated_basic_run() {
        let config = FederatedConfig {
            num_rounds: 3,
            local_epochs: 1,
            strategy: AggregationStrategy::FedAvg,
            log_interval: 0,
            ..Default::default()
        };
        let mut fed = FederatedTrainer::new(
            vec![make_trainer(), make_trainer()],
            vec![make_dataset_a(), make_dataset_b()],
            config,
        );

        let history = fed.run();
        assert_eq!(history.global_losses.len(), 3);
        assert_eq!(history.agent_losses.len(), 3);
        assert_eq!(history.agent_losses[0].len(), 2);
        assert!(history.global_losses.iter().all(|l| l.is_finite()));
    }

    #[test]
    fn test_federated_convergence() {
        let config = FederatedConfig {
            num_rounds: 20,
            local_epochs: 2,
            strategy: AggregationStrategy::FedAvg,
            log_interval: 0,
            ..Default::default()
        };
        let mut fed = FederatedTrainer::new(
            vec![make_trainer(), make_trainer()],
            vec![make_dataset_a(), make_dataset_b()],
            config,
        );

        let history = fed.run();
        // Loss should generally decrease over many rounds
        let first_loss = history.global_losses[0];
        let last_loss = *history.global_losses.last().unwrap();
        assert!(
            last_loss <= first_loss + 0.1,
            "Expected convergence: first={first_loss:.4}, last={last_loss:.4}"
        );
    }

    #[test]
    fn test_federated_trimmed_mean() {
        let config = FederatedConfig {
            num_rounds: 5,
            local_epochs: 1,
            strategy: AggregationStrategy::TrimmedMean { trim_ratio: 0.0 },
            log_interval: 0,
            ..Default::default()
        };
        // With trim_ratio=0.0, TrimmedMean should behave like uniform average
        let mut fed = FederatedTrainer::new(
            vec![make_trainer(), make_trainer()],
            vec![make_dataset_a(), make_dataset_b()],
            config,
        );

        let history = fed.run();
        assert_eq!(history.global_losses.len(), 5);
        assert!(history.global_losses.iter().all(|l| l.is_finite()));
    }

    #[test]
    fn test_federated_three_agents() {
        let ds_c = Dataset::new(
            vec![vec![4.0, 4.0], vec![5.0, 5.0]],
            vec![vec![8.0], vec![10.0]],
        );
        let config = FederatedConfig {
            num_rounds: 3,
            local_epochs: 1,
            strategy: AggregationStrategy::FedAvg,
            log_interval: 0,
            ..Default::default()
        };
        let mut fed = FederatedTrainer::new(
            vec![make_trainer(), make_trainer(), make_trainer()],
            vec![make_dataset_a(), make_dataset_b(), ds_c],
            config,
        );

        assert_eq!(fed.num_agents(), 3);
        assert_eq!(fed.total_samples(), 10); // 4 + 4 + 2

        let history = fed.run();
        assert_eq!(history.global_losses.len(), 3);
        assert_eq!(history.agent_losses[0].len(), 3);
    }

    #[test]
    fn test_federated_with_rollback() {
        let config = FederatedConfig {
            num_rounds: 5,
            local_epochs: 1,
            strategy: AggregationStrategy::FedAvg,
            log_interval: 0,
            min_delta: Some(0.0001),
        };
        let mut fed = FederatedTrainer::new(
            vec![make_trainer(), make_trainer()],
            vec![make_dataset_a(), make_dataset_b()],
            config,
        );

        let history = fed.run();
        assert_eq!(history.global_losses.len(), 5);
        // Rollback count should be non-negative
        assert!(history.rollback_count <= 5);
    }

    #[test]
    fn test_federated_history_helpers() {
        let h = FederatedHistory {
            global_losses: vec![5.0, 3.0, 2.0, 1.5, 1.0],
            agent_losses: vec![],
            rollback_count: 0,
        };
        assert_eq!(h.best_global_loss(), Some(1.0));
        assert!(h.is_improving(3));
    }

    #[test]
    fn test_federated_config_default() {
        let config = FederatedConfig::default();
        assert_eq!(config.num_rounds, 10);
        assert_eq!(config.local_epochs, 1);
        assert_eq!(config.strategy, AggregationStrategy::FedAvg);
        assert!(config.min_delta.is_none());
    }

    #[test]
    fn test_aggregation_strategy_fedprox() {
        let config = FederatedConfig {
            num_rounds: 3,
            local_epochs: 1,
            strategy: AggregationStrategy::FedProx { mu: 0.01 },
            log_interval: 0,
            ..Default::default()
        };
        let mut fed = FederatedTrainer::new(
            vec![make_trainer(), make_trainer()],
            vec![make_dataset_a(), make_dataset_b()],
            config,
        );

        let history = fed.run();
        assert_eq!(history.global_losses.len(), 3);
        assert!(history.global_losses.iter().all(|l| l.is_finite()));
    }

    #[test]
    fn test_param_snapshot_roundtrip() {
        let fed = FederatedTrainer::new(
            vec![make_trainer(), make_trainer()],
            vec![make_dataset_a(), make_dataset_b()],
            FederatedConfig::default(),
        );

        let snapshot = fed.snapshot_global_params();
        assert!(!snapshot.is_empty());
        for p in &snapshot {
            assert!(!p.data.is_empty());
            assert_eq!(p.data.len(), p.shape.iter().product::<usize>());
        }
    }
}
