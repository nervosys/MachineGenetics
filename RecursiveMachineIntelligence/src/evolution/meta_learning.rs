//! Meta-Learning Integration
//!
//! Learning-to-learn primitives for self-improving agents:
//!
//! - **TaskDistribution**: Tracks performance across task families
//! - **LearnerProfile**: Encodes strengths, weaknesses, and learning curves
//! - **ArchitectureSearchAgent**: Automated neural architecture discovery
//! - **HyperparameterOptimizer**: Bayesian and evolutionary hyperparameter tuning

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;


// ============================================================================
// Task Representation
// ============================================================================

/// A family of related tasks (e.g., "image classification", "time-series forecasting").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFamily {
    /// Unique ID
    pub id: Uuid,
    /// Family name
    pub name: String,
    /// Task dimensionality (feature count)
    pub input_dim: usize,
    /// Output dimensionality
    pub output_dim: usize,
    /// Observed difficulty (0.0 = trivial, 1.0 = impossible)
    pub difficulty: f64,
    /// Metadata tags
    pub tags: Vec<String>,
}

/// Performance record for a single task attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPerformance {
    /// Task family
    pub family_id: Uuid,
    /// Metric achieved (e.g., accuracy, loss)
    pub score: f64,
    /// Whether higher score is better
    pub higher_is_better: bool,
    /// Training iterations used
    pub iterations: u64,
    /// Wall-clock time
    pub duration_secs: f64,
    /// Configuration used
    pub config_hash: u64,
    /// Timestamp
    pub timestamp: f64,
}

// ============================================================================
// Learner Profile
// ============================================================================

/// Learning curve model: tracks how performance improves over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningCurve {
    /// Sampled (iteration, score) pairs
    pub points: Vec<(u64, f64)>,
    /// Estimated asymptotic performance
    pub asymptote: Option<f64>,
    /// Estimated half-life (iterations to reach 50% of asymptote)
    pub half_life: Option<f64>,
}

impl LearningCurve {
    /// Create a new empty learning curve.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            asymptote: None,
            half_life: None,
        }
    }

    /// Record a data point.
    pub fn record(&mut self, iteration: u64, score: f64) {
        self.points.push((iteration, score));
        self.points.sort_by_key(|&(it, _)| it);
        self.estimate_params();
    }

    /// Estimate asymptote and half-life from data.
    fn estimate_params(&mut self) {
        if self.points.len() < 3 {
            return;
        }

        // Simple heuristic: asymptote ≈ best score * 1.05
        let best = self
            .points
            .iter()
            .map(|&(_, s)| s)
            .fold(f64::NEG_INFINITY, f64::max);
        self.asymptote = Some(best * 1.05);

        // Half-life: iteration where we first exceeded 50% of asymptote
        if let Some(asym) = self.asymptote {
            let half_target = asym * 0.5;
            for &(it, s) in &self.points {
                if s >= half_target {
                    self.half_life = Some(it as f64);
                    break;
                }
            }
        }
    }

    /// Predict score at a given iteration using simple extrapolation.
    pub fn predict(&self, iteration: u64) -> Option<f64> {
        let asym = self.asymptote?;
        let hl = self.half_life?;
        if hl <= 0.0 {
            return Some(asym);
        }
        // Diminishing returns model: score = asym * (1 - e^(-it / hl))
        let predicted = asym * (1.0 - (-(iteration as f64) / hl).exp());
        Some(predicted.min(asym))
    }

    /// Get the latest score.
    pub fn latest_score(&self) -> Option<f64> {
        self.points.last().map(|&(_, s)| s)
    }
}

impl Default for LearningCurve {
    fn default() -> Self {
        Self::new()
    }
}

/// Profile of a learner's capabilities, tracking strengths and weaknesses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerProfile {
    /// Learner ID
    pub id: Uuid,
    /// Performance history per task family
    pub history: HashMap<Uuid, Vec<TaskPerformance>>,
    /// Learning curves per task family
    pub curves: HashMap<Uuid, LearningCurve>,
    /// Strength scores per tag (higher = more proficient)
    pub strengths: HashMap<String, f64>,
    /// Total tasks attempted
    pub total_attempts: u64,
    /// Total training time in seconds
    pub total_training_secs: f64,
}

impl LearnerProfile {
    /// Create a new profile.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            history: HashMap::new(),
            curves: HashMap::new(),
            strengths: HashMap::new(),
            total_attempts: 0,
            total_training_secs: 0.0,
        }
    }

    /// Record a task performance result.
    pub fn record(&mut self, perf: TaskPerformance, tags: &[String]) {
        let family_id = perf.family_id;
        self.total_training_secs += perf.duration_secs;
        self.total_attempts += 1;

        // Update learning curve
        let curve = self
            .curves
            .entry(family_id)
            .or_default();
        curve.record(perf.iterations, perf.score);

        // Update strength estimates per tag
        for tag in tags {
            let entry = self.strengths.entry(tag.clone()).or_insert(0.5);
            // Exponential moving average
            *entry = *entry * 0.9 + perf.score * 0.1;
        }

        // Store history
        self.history.entry(family_id).or_default().push(perf);
    }

    /// Get best score for a task family.
    pub fn best_score(&self, family_id: Uuid) -> Option<f64> {
        self.history.get(&family_id).and_then(|h| {
            h.iter()
                .map(|p| p.score)
                .fold(None, |acc, s| Some(acc.map_or(s, |a: f64| a.max(s))))
        })
    }

    /// Get average score for a task family.
    pub fn avg_score(&self, family_id: Uuid) -> Option<f64> {
        self.history.get(&family_id).and_then(|h| {
            if h.is_empty() {
                None
            } else {
                Some(h.iter().map(|p| p.score).sum::<f64>() / h.len() as f64)
            }
        })
    }

    /// Predict performance on a task given its tags.
    pub fn predict_performance(&self, tags: &[String]) -> f64 {
        if tags.is_empty() {
            return 0.5;
        }
        let sum: f64 = tags
            .iter()
            .map(|t| self.strengths.get(t).copied().unwrap_or(0.5))
            .sum();
        sum / tags.len() as f64
    }

    /// Get ranked strengths.
    pub fn ranked_strengths(&self) -> Vec<(String, f64)> {
        let mut strengths: Vec<_> = self
            .strengths
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        strengths.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        strengths
    }
}

impl Default for LearnerProfile {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Architecture Search
// ============================================================================

/// Search space for neural architectures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSpace {
    /// Minimum layer count
    pub min_layers: usize,
    /// Maximum layer count
    pub max_layers: usize,
    /// Allowed layer types
    pub layer_types: Vec<LayerSpec>,
    /// Minimum hidden dimension
    pub min_hidden: usize,
    /// Maximum hidden dimension
    pub max_hidden: usize,
    /// Allowed activations
    pub activations: Vec<String>,
    /// Allow skip connections
    pub allow_skip: bool,
}

impl Default for SearchSpace {
    fn default() -> Self {
        Self {
            min_layers: 1,
            max_layers: 12,
            layer_types: vec![LayerSpec::Linear, LayerSpec::Conv2d, LayerSpec::Attention],
            min_hidden: 16,
            max_hidden: 2048,
            activations: vec!["relu".into(), "gelu".into(), "silu".into()],
            allow_skip: true,
        }
    }
}

/// Layer specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LayerSpec {
    /// Linear (fully connected) layer
    Linear,
    /// 2D convolutional layer
    Conv2d,
    /// Attention layer
    Attention,
    /// Normalization layer
    Normalization,
    /// Dropout layer
    Dropout,
    /// Pooling layer
    Pooling,
}

/// A candidate architecture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureCandidate {
    /// Candidate ID
    pub id: Uuid,
    /// Layer definitions
    pub layers: Vec<LayerDef>,
    /// Skip connections (from_idx, to_idx)
    pub skip_connections: Vec<(usize, usize)>,
    /// Fitness score (higher = better)
    pub fitness: f64,
    /// Parameter count
    pub param_count: u64,
    /// FLOPs estimate
    pub flops: u64,
    /// Generation number
    pub generation: u32,
    /// Parent architectures
    pub parents: Vec<Uuid>,
}

/// Concrete layer definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerDef {
    /// Layer type
    pub spec: LayerSpec,
    /// Input dimension
    pub in_dim: usize,
    /// Output dimension
    pub out_dim: usize,
    /// Activation function name
    pub activation: Option<String>,
    /// Extra config (e.g., kernel_size, heads)
    pub config: HashMap<String, f64>,
}

/// Architecture search agent.
pub struct ArchitectureSearchAgent {
    /// Search space
    pub search_space: SearchSpace,
    /// Population of candidates
    population: Vec<ArchitectureCandidate>,
    /// Best candidate so far
    best: Option<ArchitectureCandidate>,
    /// Generation counter
    generation: u32,
    /// Population size
    pop_size: usize,
    /// Mutation rate
    mutation_rate: f64,
    /// Tournament size for selection
    tournament_size: usize,
}

impl ArchitectureSearchAgent {
    /// Create a new search agent.
    pub fn new(search_space: SearchSpace, pop_size: usize) -> Self {
        Self {
            search_space,
            population: Vec::new(),
            best: None,
            generation: 0,
            pop_size,
            mutation_rate: 0.3,
            tournament_size: 3,
        }
    }

    /// Initialize population with random architectures.
    pub fn initialize(&mut self, input_dim: usize, output_dim: usize) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        self.population.clear();

        for _ in 0..self.pop_size {
            let num_layers =
                rng.gen_range(self.search_space.min_layers..=self.search_space.max_layers);
            let mut layers = Vec::new();

            for i in 0..num_layers {
                let in_d = if i == 0 {
                    input_dim
                } else {
                    layers
                        .last()
                        .map(|l: &LayerDef| l.out_dim)
                        .unwrap_or(input_dim)
                };
                let out_d = if i == num_layers - 1 {
                    output_dim
                } else {
                    rng.gen_range(self.search_space.min_hidden..=self.search_space.max_hidden)
                };

                let spec_idx = rng.gen_range(0..self.search_space.layer_types.len());
                let act_idx = rng.gen_range(0..self.search_space.activations.len());

                layers.push(LayerDef {
                    spec: self.search_space.layer_types[spec_idx].clone(),
                    in_dim: in_d,
                    out_dim: out_d,
                    activation: if i < num_layers - 1 {
                        Some(self.search_space.activations[act_idx].clone())
                    } else {
                        None
                    },
                    config: HashMap::new(),
                });
            }

            let candidate = ArchitectureCandidate {
                id: Uuid::new_v4(),
                layers,
                skip_connections: Vec::new(),
                fitness: 0.0,
                param_count: 0,
                flops: 0,
                generation: self.generation,
                parents: Vec::new(),
            };
            self.population.push(candidate);
        }
    }

    /// Set fitness for a candidate.
    pub fn set_fitness(&mut self, candidate_id: Uuid, fitness: f64) {
        if let Some(c) = self.population.iter_mut().find(|c| c.id == candidate_id) {
            c.fitness = fitness;
            if self.best.as_ref().map_or(true, |b| fitness > b.fitness) {
                self.best = Some(c.clone());
            }
        }
    }

    /// Tournament selection.
    fn tournament_select(&self) -> &ArchitectureCandidate {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut best: Option<&ArchitectureCandidate> = None;

        for _ in 0..self.tournament_size.min(self.population.len()) {
            let idx = rng.gen_range(0..self.population.len());
            let c = &self.population[idx];
            if best.map_or(true, |b| c.fitness > b.fitness) {
                best = Some(c);
            }
        }
        best.unwrap_or(&self.population[0])
    }

    /// Mutate a candidate.
    fn mutate(&self, candidate: &ArchitectureCandidate) -> ArchitectureCandidate {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut new = candidate.clone();
        new.id = Uuid::new_v4();
        new.generation = self.generation;
        new.parents = vec![candidate.id];
        new.fitness = 0.0;

        if !new.layers.is_empty() {
            let layer_idx = rng.gen_range(0..new.layers.len());
            let hidden = rng.gen_range(self.search_space.min_hidden..=self.search_space.max_hidden);

            // Mutate output dimension (skip last layer)
            if layer_idx < new.layers.len() - 1 {
                new.layers[layer_idx].out_dim = hidden;
                // Fix next layer's input
                if layer_idx + 1 < new.layers.len() {
                    new.layers[layer_idx + 1].in_dim = hidden;
                }
            }
        }

        new
    }

    /// Crossover two candidates.
    fn crossover(
        &self,
        parent_a: &ArchitectureCandidate,
        parent_b: &ArchitectureCandidate,
    ) -> ArchitectureCandidate {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let split_a = if parent_a.layers.is_empty() {
            0
        } else {
            rng.gen_range(0..parent_a.layers.len())
        };
        let split_b = if parent_b.layers.is_empty() {
            0
        } else {
            rng.gen_range(0..parent_b.layers.len())
        };

        let mut layers: Vec<LayerDef> = parent_a.layers[..split_a].to_vec();
        layers.extend_from_slice(&parent_b.layers[split_b..]);

        // Fix dimension mismatches at splice
        for i in 1..layers.len() {
            if layers[i].in_dim != layers[i - 1].out_dim {
                layers[i].in_dim = layers[i - 1].out_dim;
            }
        }

        ArchitectureCandidate {
            id: Uuid::new_v4(),
            layers,
            skip_connections: Vec::new(),
            fitness: 0.0,
            param_count: 0,
            flops: 0,
            generation: self.generation,
            parents: vec![parent_a.id, parent_b.id],
        }
    }

    /// Evolve to next generation.
    pub fn evolve(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        self.generation += 1;

        let mut next_gen = Vec::new();

        // Elitism: keep top 10%
        let mut sorted = self.population.clone();
        sorted.sort_by(|a, b| {
            b.fitness
                .partial_cmp(&a.fitness)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let elite = (self.pop_size as f64 * 0.1).ceil() as usize;
        for c in sorted.iter().take(elite) {
            let mut kept = c.clone();
            kept.generation = self.generation;
            next_gen.push(kept);
        }

        // Fill rest with crossover + mutation
        while next_gen.len() < self.pop_size {
            let parent_a = self.tournament_select();
            let parent_b = self.tournament_select();
            let mut child = self.crossover(parent_a, parent_b);

            if rng.gen::<f64>() < self.mutation_rate {
                child = self.mutate(&child);
            }
            next_gen.push(child);
        }

        self.population = next_gen;
    }

    /// Get best candidate.
    pub fn best(&self) -> Option<&ArchitectureCandidate> {
        self.best.as_ref()
    }

    /// Get current generation.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Get population.
    pub fn population(&self) -> &[ArchitectureCandidate] {
        &self.population
    }

    /// Get population size.
    pub fn pop_size(&self) -> usize {
        self.population.len()
    }
}

// ============================================================================
// Hyperparameter Optimization
// ============================================================================

/// A hyperparameter range definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HyperparamRange {
    /// Continuous range [low, high]
    Continuous {
        /// Lower bound (inclusive).
        low: f64,
        /// Upper bound (inclusive).
        high: f64,
        /// Sample in log space (for learning rates etc.).
        log_scale: bool,
    },
    /// Discrete integer range [low, high]
    Discrete {
        /// Lower bound (inclusive).
        low: i64,
        /// Upper bound (inclusive).
        high: i64,
    },
    /// Categorical choices
    Categorical {
        /// The candidate values to choose among.
        choices: Vec<String>,
    },
}

/// A hyperparameter trial.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trial {
    /// Trial ID
    pub id: Uuid,
    /// Parameter values
    pub params: HashMap<String, f64>,
    /// Categorical parameter values
    pub categorical_params: HashMap<String, String>,
    /// Objective value (score)
    pub score: Option<f64>,
    /// Trial status
    pub status: TrialStatus,
    /// Duration
    pub duration_secs: Option<f64>,
}

/// Trial status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrialStatus {
    /// Trial is pending execution
    Pending,
    /// Trial is currently running
    Running,
    /// Trial completed successfully
    Completed,
    /// Trial failed
    Failed,
    /// Trial was pruned early
    Pruned,
}

/// Optimization strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OptStrategy {
    /// Random search
    Random,
    /// Grid search
    Grid,
    /// TPE-like Bayesian optimization (simplified)
    Bayesian,
    /// Evolutionary
    Evolutionary,
}

/// Hyperparameter optimizer.
pub struct HyperparameterOptimizer {
    /// Parameter space
    space: HashMap<String, HyperparamRange>,
    /// All trials
    trials: Vec<Trial>,
    /// Strategy
    strategy: OptStrategy,
    /// Whether to maximize (true) or minimize (false)
    maximize: bool,
    /// Maximum trials
    max_trials: usize,
}

impl HyperparameterOptimizer {
    /// Create a new optimizer.
    pub fn new(strategy: OptStrategy, maximize: bool, max_trials: usize) -> Self {
        Self {
            space: HashMap::new(),
            trials: Vec::new(),
            strategy,
            maximize,
            max_trials,
        }
    }

    /// Add a parameter to the search space.
    pub fn add_param(&mut self, name: &str, range: HyperparamRange) {
        self.space.insert(name.to_string(), range);
    }

    /// Suggest next trial parameters.
    pub fn suggest(&self) -> Trial {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut params = HashMap::new();
        let mut categorical_params = HashMap::new();

        match self.strategy {
            OptStrategy::Random | OptStrategy::Grid => {
                for (name, range) in &self.space {
                    match range {
                        HyperparamRange::Continuous {
                            low,
                            high,
                            log_scale,
                        } => {
                            let val = if *log_scale {
                                let log_lo = low.ln();
                                let log_hi = high.ln();
                                (rng.gen::<f64>() * (log_hi - log_lo) + log_lo).exp()
                            } else {
                                rng.gen::<f64>() * (high - low) + low
                            };
                            params.insert(name.clone(), val);
                        }
                        HyperparamRange::Discrete { low, high } => {
                            let val = rng.gen_range(*low..=*high);
                            params.insert(name.clone(), val as f64);
                        }
                        HyperparamRange::Categorical { choices } => {
                            if !choices.is_empty() {
                                let idx = rng.gen_range(0..choices.len());
                                categorical_params.insert(name.clone(), choices[idx].clone());
                            }
                        }
                    }
                }
            }
            OptStrategy::Bayesian => {
                // Simplified: sample near best known point with perturbation
                let best_trial = self.best_trial();
                for (name, range) in &self.space {
                    match range {
                        HyperparamRange::Continuous { low, high, .. } => {
                            let center = best_trial
                                .as_ref()
                                .and_then(|t| t.params.get(name).copied())
                                .unwrap_or((*low + *high) / 2.0);
                            let spread = (high - low) * 0.2;
                            let val = (center + rng.gen::<f64>() * 2.0 * spread - spread)
                                .clamp(*low, *high);
                            params.insert(name.clone(), val);
                        }
                        HyperparamRange::Discrete { low, high } => {
                            let val = rng.gen_range(*low..=*high);
                            params.insert(name.clone(), val as f64);
                        }
                        HyperparamRange::Categorical { choices } => {
                            if !choices.is_empty() {
                                let idx = rng.gen_range(0..choices.len());
                                categorical_params.insert(name.clone(), choices[idx].clone());
                            }
                        }
                    }
                }
            }
            OptStrategy::Evolutionary => {
                // Simplified: mutate best known
                let best_trial = self.best_trial();
                for (name, range) in &self.space {
                    match range {
                        HyperparamRange::Continuous { low, high, .. } => {
                            let center = best_trial
                                .as_ref()
                                .and_then(|t| t.params.get(name).copied())
                                .unwrap_or((*low + *high) / 2.0);
                            let perturbation = (high - low) * 0.1 * rng.gen::<f64>();
                            let val = if rng.gen() {
                                center + perturbation
                            } else {
                                center - perturbation
                            }
                            .clamp(*low, *high);
                            params.insert(name.clone(), val);
                        }
                        HyperparamRange::Discrete { low, high } => {
                            let val = rng.gen_range(*low..=*high);
                            params.insert(name.clone(), val as f64);
                        }
                        HyperparamRange::Categorical { choices } => {
                            if !choices.is_empty() {
                                let idx = rng.gen_range(0..choices.len());
                                categorical_params.insert(name.clone(), choices[idx].clone());
                            }
                        }
                    }
                }
            }
        }

        Trial {
            id: Uuid::new_v4(),
            params,
            categorical_params,
            score: None,
            status: TrialStatus::Pending,
            duration_secs: None,
        }
    }

    /// Report trial result.
    pub fn report(&mut self, mut trial: Trial, score: f64, duration_secs: f64) {
        trial.score = Some(score);
        trial.status = TrialStatus::Completed;
        trial.duration_secs = Some(duration_secs);
        self.trials.push(trial);
    }

    /// Get best trial.
    pub fn best_trial(&self) -> Option<&Trial> {
        self.trials
            .iter()
            .filter(|t| t.score.is_some())
            .max_by(|a, b| {
                let sa = a.score.unwrap_or(f64::NEG_INFINITY);
                let sb = b.score.unwrap_or(f64::NEG_INFINITY);
                if self.maximize {
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                } else {
                    sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
                }
            })
    }

    /// Check if search budget is exhausted.
    pub fn is_done(&self) -> bool {
        self.trials.len() >= self.max_trials
    }

    /// Get completed trial count.
    pub fn completed_count(&self) -> usize {
        self.trials
            .iter()
            .filter(|t| t.status == TrialStatus::Completed)
            .count()
    }

    /// Get all trials.
    pub fn trials(&self) -> &[Trial] {
        &self.trials
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_curve() {
        let mut lc = LearningCurve::new();
        lc.record(10, 0.3);
        lc.record(50, 0.5);
        lc.record(100, 0.7);
        lc.record(200, 0.85);

        assert_eq!(lc.latest_score(), Some(0.85));
        assert!(lc.asymptote.is_some());
        assert!(lc.predict(300).is_some());
    }

    #[test]
    fn test_learner_profile() {
        let mut profile = LearnerProfile::new();
        let family_id = Uuid::new_v4();

        let perf = TaskPerformance {
            family_id,
            score: 0.9,
            higher_is_better: true,
            iterations: 100,
            duration_secs: 10.0,
            config_hash: 42,
            timestamp: 0.0,
        };

        profile.record(perf, &["classification".to_string(), "vision".to_string()]);

        assert_eq!(profile.total_attempts, 1);
        assert_eq!(profile.best_score(family_id), Some(0.9));
        assert!(profile.strengths.contains_key("classification"));
    }

    #[test]
    fn test_learner_predict_performance() {
        let mut profile = LearnerProfile::new();
        let family_id = Uuid::new_v4();

        for score in [0.7, 0.8, 0.85, 0.9] {
            let perf = TaskPerformance {
                family_id,
                score,
                higher_is_better: true,
                iterations: 100,
                duration_secs: 5.0,
                config_hash: 0,
                timestamp: 0.0,
            };
            profile.record(perf, &["nlp".to_string()]);
        }

        let predicted = profile.predict_performance(&["nlp".to_string()]);
        assert!(predicted > 0.0);
        assert!(predicted < 1.0);
    }

    #[test]
    fn test_architecture_search() {
        let space = SearchSpace {
            min_layers: 1,
            max_layers: 4,
            layer_types: vec![LayerSpec::Linear],
            min_hidden: 16,
            max_hidden: 128,
            activations: vec!["relu".into()],
            allow_skip: false,
        };

        let mut agent = ArchitectureSearchAgent::new(space, 10);
        agent.initialize(784, 10);
        assert_eq!(agent.pop_size(), 10);

        // Set fitness
        for c in agent.population().to_vec() {
            agent.set_fitness(c.id, rand::random::<f64>());
        }

        assert!(agent.best().is_some());

        // Evolve
        agent.evolve();
        assert_eq!(agent.generation(), 1);
        assert_eq!(agent.pop_size(), 10);
    }

    #[test]
    fn test_hyperparameter_optimizer() {
        let mut opt = HyperparameterOptimizer::new(OptStrategy::Random, false, 5);
        opt.add_param(
            "lr",
            HyperparamRange::Continuous {
                low: 1e-5,
                high: 1e-1,
                log_scale: true,
            },
        );
        opt.add_param(
            "batch_size",
            HyperparamRange::Discrete { low: 8, high: 128 },
        );

        assert!(!opt.is_done());

        for _ in 0..5 {
            let trial = opt.suggest();
            assert!(trial.params.contains_key("lr"));
            assert!(trial.params.contains_key("batch_size"));
            opt.report(trial, rand::random::<f64>(), 1.0);
        }

        assert!(opt.is_done());
        assert!(opt.best_trial().is_some());
        assert_eq!(opt.completed_count(), 5);
    }

    #[test]
    fn test_bayesian_optimizer() {
        let mut opt = HyperparameterOptimizer::new(OptStrategy::Bayesian, true, 10);
        opt.add_param(
            "lr",
            HyperparamRange::Continuous {
                low: 0.001,
                high: 0.1,
                log_scale: false,
            },
        );

        // First trial is random-ish, rest should be near-best
        let t1 = opt.suggest();
        opt.report(t1, 0.8, 1.0);

        let t2 = opt.suggest();
        assert!(t2.params.contains_key("lr"));
    }

    #[test]
    fn test_ranked_strengths() {
        let mut profile = LearnerProfile::new();
        profile.strengths.insert("nlp".to_string(), 0.9);
        profile.strengths.insert("vision".to_string(), 0.6);
        profile.strengths.insert("rl".to_string(), 0.3);

        let ranked = profile.ranked_strengths();
        assert_eq!(ranked[0].0, "nlp");
        assert_eq!(ranked[2].0, "rl");
    }

    #[test]
    fn test_task_family() {
        let family = TaskFamily {
            id: Uuid::new_v4(),
            name: "MNIST".to_string(),
            input_dim: 784,
            output_dim: 10,
            difficulty: 0.2,
            tags: vec!["classification".into(), "vision".into()],
        };
        assert_eq!(family.name, "MNIST");
        assert_eq!(family.difficulty, 0.2);
    }
}
