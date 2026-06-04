//! Evolutionary Infrastructure
//!
//! Population management and evolutionary algorithms for self-improving agents:
//!
//! - **Individual**: A candidate solution (architecture, weights, or strategy)
//! - **Population**: Collection of individuals with fitness tracking
//! - **SelectionStrategy**: Tournament, roulette, rank-based selection
//! - **EvolutionEngine**: Orchestrates generations with crossover and mutation

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;


// ============================================================================
// Individual
// ============================================================================

/// Genome encoding types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Genome {
    /// Real-valued vector (weights, hyperparameters)
    RealValued(Vec<f64>),
    /// Binary string (feature selection, architecture choices)
    Binary(Vec<bool>),
    /// Permutation (scheduling, ordering)
    Permutation(Vec<usize>),
    /// Tree structure (expressions, programs)
    Tree(TreeNode),
    /// Custom serialized genome
    Custom(Vec<u8>),
}

/// A tree node for genetic programming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    /// Node label (operator or terminal)
    pub label: String,
    /// Children
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Create a leaf node.
    pub fn leaf(label: &str) -> Self {
        Self {
            label: label.to_string(),
            children: Vec::new(),
        }
    }

    /// Create an internal node.
    pub fn node(label: &str, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.to_string(),
            children,
        }
    }

    /// Count total nodes in tree.
    pub fn size(&self) -> usize {
        1 + self.children.iter().map(|c| c.size()).sum::<usize>()
    }

    /// Get depth of tree.
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }
}

/// A candidate solution in the evolutionary population.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Individual {
    /// Unique ID
    pub id: Uuid,
    /// Genome encoding
    pub genome: Genome,
    /// Fitness scores (multi-objective)
    pub fitness: Vec<f64>,
    /// Generation born
    pub generation: u32,
    /// Parent IDs
    pub parents: Vec<Uuid>,
    /// Age (number of generations survived)
    pub age: u32,
    /// Number of offspring produced
    pub offspring_count: u32,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl Individual {
    /// Create a new individual.
    pub fn new(genome: Genome, generation: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            genome,
            fitness: Vec::new(),
            generation,
            parents: Vec::new(),
            age: 0,
            offspring_count: 0,
            metadata: HashMap::new(),
        }
    }

    /// Get primary fitness (first objective).
    pub fn primary_fitness(&self) -> f64 {
        self.fitness.first().copied().unwrap_or(f64::NEG_INFINITY)
    }

    /// Set fitness (single-objective).
    pub fn set_fitness(&mut self, fitness: f64) {
        self.fitness = vec![fitness];
    }

    /// Set multi-objective fitness.
    pub fn set_multi_fitness(&mut self, fitness: Vec<f64>) {
        self.fitness = fitness;
    }

    /// Check Pareto dominance: self dominates other if ≥ in all objectives and > in at least one.
    pub fn dominates(&self, other: &Individual) -> bool {
        if self.fitness.len() != other.fitness.len() || self.fitness.is_empty() {
            return false;
        }

        let mut strictly_better = false;
        for (a, b) in self.fitness.iter().zip(other.fitness.iter()) {
            if a < b {
                return false;
            }
            if a > b {
                strictly_better = true;
            }
        }
        strictly_better
    }

    /// Increment age.
    pub fn age_one_generation(&mut self) {
        self.age += 1;
    }
}

// ============================================================================
// Selection Strategies
// ============================================================================

/// Selection strategy for choosing parents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SelectionStrategy {
    /// Tournament selection with given size
    Tournament(usize),
    /// Roulette wheel (fitness-proportionate)
    RouletteWheel,
    /// Rank-based selection
    RankBased,
    /// Truncation (top-k)
    Truncation(usize),
    /// Random uniform
    Random,
}

/// Select an individual from the population using the given strategy.
pub fn select(pop: &[Individual], strategy: SelectionStrategy) -> Option<&Individual> {
    use rand::Rng;

    if pop.is_empty() {
        return None;
    }

    let mut rng = rand::thread_rng();

    match strategy {
        SelectionStrategy::Tournament(size) => {
            let mut best: Option<&Individual> = None;
            for _ in 0..size.min(pop.len()) {
                let idx = rng.gen_range(0..pop.len());
                let candidate = &pop[idx];
                if best.map_or(true, |b| candidate.primary_fitness() > b.primary_fitness()) {
                    best = Some(candidate);
                }
            }
            best
        }
        SelectionStrategy::RouletteWheel => {
            let min_fit = pop
                .iter()
                .map(|i| i.primary_fitness())
                .fold(f64::INFINITY, f64::min);
            let adjusted: Vec<f64> = pop
                .iter()
                .map(|i| i.primary_fitness() - min_fit + 1e-6)
                .collect();
            let total: f64 = adjusted.iter().sum();
            if total <= 0.0 {
                return pop.first();
            }

            let mut r = rng.gen::<f64>() * total;
            for (i, adj) in adjusted.iter().enumerate() {
                r -= adj;
                if r <= 0.0 {
                    return Some(&pop[i]);
                }
            }
            pop.last()
        }
        SelectionStrategy::RankBased => {
            let mut indices: Vec<usize> = (0..pop.len()).collect();
            indices.sort_by(|&a, &b| {
                pop[b]
                    .primary_fitness()
                    .partial_cmp(&pop[a].primary_fitness())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let n = pop.len();
            let total: usize = n * (n + 1) / 2;
            let mut r = rng.gen_range(0..total);
            for (rank_val, &idx) in indices.iter().enumerate() {
                let weight = n - rank_val;
                if r < weight {
                    return Some(&pop[idx]);
                }
                r -= weight;
            }
            pop.first()
        }
        SelectionStrategy::Truncation(k) => {
            let mut indices: Vec<usize> = (0..pop.len()).collect();
            indices.sort_by(|&a, &b| {
                pop[b]
                    .primary_fitness()
                    .partial_cmp(&pop[a].primary_fitness())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let top_k = k.min(pop.len());
            let chosen = rng.gen_range(0..top_k);
            Some(&pop[indices[chosen]])
        }
        SelectionStrategy::Random => {
            let idx = rng.gen_range(0..pop.len());
            Some(&pop[idx])
        }
    }
}

// ============================================================================
// Genetic Operators
// ============================================================================

/// Crossover two real-valued genomes.
pub fn crossover_real(a: &[f64], b: &[f64], blend_alpha: f64) -> Vec<f64> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let len = a.len().min(b.len());
    let mut child = Vec::with_capacity(len);

    for i in 0..len {
        let t = rng.gen::<f64>() * (1.0 + 2.0 * blend_alpha) - blend_alpha;
        child.push(a[i] + t * (b[i] - a[i]));
    }

    child
}

/// Mutate a real-valued genome with Gaussian noise.
pub fn mutate_real(genome: &mut [f64], mutation_rate: f64, sigma: f64) {
    use rand::Rng;
    use rand_distr::{Distribution, Normal};

    let mut rng = rand::thread_rng();
    let normal = Normal::new(0.0, sigma).unwrap_or_else(|_| Normal::new(0.0, 0.1).expect("fallback Normal(0,0.1) must succeed"));

    for gene in genome.iter_mut() {
        if rng.gen::<f64>() < mutation_rate {
            *gene += normal.sample(&mut rng);
        }
    }
}

/// Crossover two binary genomes (uniform crossover).
pub fn crossover_binary(a: &[bool], b: &[bool]) -> Vec<bool> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let len = a.len().min(b.len());
    let mut child = Vec::with_capacity(len);

    for i in 0..len {
        child.push(if rng.gen() { a[i] } else { b[i] });
    }

    child
}

/// Mutate a binary genome (bit flip).
pub fn mutate_binary(genome: &mut [bool], mutation_rate: f64) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for gene in genome.iter_mut() {
        if rng.gen::<f64>() < mutation_rate {
            *gene = !*gene;
        }
    }
}

// ============================================================================
// Population
// ============================================================================

/// Configuration for the evolution engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionConfig {
    /// Population size
    pub population_size: usize,
    /// Maximum generations
    pub max_generations: u32,
    /// Crossover probability
    pub crossover_rate: f64,
    /// Mutation rate
    pub mutation_rate: f64,
    /// Mutation standard deviation (for real-valued)
    pub mutation_sigma: f64,
    /// Elitism ratio (fraction of top individuals to keep)
    pub elitism_ratio: f64,
    /// Selection strategy
    pub selection: SelectionStrategy,
    /// Early stopping if no improvement for N generations
    pub stagnation_limit: u32,
    /// Blend crossover alpha
    pub blend_alpha: f64,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            population_size: 100,
            max_generations: 500,
            crossover_rate: 0.8,
            mutation_rate: 0.1,
            mutation_sigma: 0.1,
            elitism_ratio: 0.05,
            selection: SelectionStrategy::Tournament(3),
            stagnation_limit: 50,
            blend_alpha: 0.5,
        }
    }
}

/// Evolution engine driving a population through generations.
pub struct EvolutionEngine {
    /// Configuration
    pub config: EvolutionConfig,
    /// Current population
    population: Vec<Individual>,
    /// Generation counter
    generation: u32,
    /// Best fitness seen
    best_fitness: f64,
    /// Generations since last improvement
    stagnation_counter: u32,
    /// Hall of fame (top-N all-time)
    hall_of_fame: Vec<Individual>,
    /// Statistics per generation
    gen_stats: Vec<GenerationStats>,
}

/// Statistics for a generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationStats {
    /// Generation number
    pub generation: u32,
    /// Best fitness in this generation
    pub best_fitness: f64,
    /// Average fitness in this generation
    pub avg_fitness: f64,
    /// Worst fitness in this generation
    pub worst_fitness: f64,
    /// Population diversity metric
    pub diversity: f64,
    /// Population size
    pub population_size: usize,
}

impl EvolutionEngine {
    /// Create a new evolution engine.
    pub fn new(config: EvolutionConfig) -> Self {
        Self {
            config,
            population: Vec::new(),
            generation: 0,
            best_fitness: f64::NEG_INFINITY,
            stagnation_counter: 0,
            hall_of_fame: Vec::new(),
            gen_stats: Vec::new(),
        }
    }

    /// Initialize with a population of individuals.
    pub fn initialize(&mut self, individuals: Vec<Individual>) {
        self.population = individuals;
        self.generation = 0;
        self.best_fitness = f64::NEG_INFINITY;
        self.stagnation_counter = 0;
    }

    /// Initialize with random real-valued genomes.
    pub fn initialize_real(&mut self, genome_len: usize, min_val: f64, max_val: f64) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut pop = Vec::new();

        for _ in 0..self.config.population_size {
            let genome: Vec<f64> = (0..genome_len)
                .map(|_| rng.gen::<f64>() * (max_val - min_val) + min_val)
                .collect();
            pop.push(Individual::new(Genome::RealValued(genome), 0));
        }

        self.initialize(pop);
    }

    /// Get current population.
    pub fn population(&self) -> &[Individual] {
        &self.population
    }

    /// Get current population mutably (for setting fitness).
    pub fn population_mut(&mut self) -> &mut [Individual] {
        &mut self.population
    }

    /// Get current generation.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Get best individual in current population.
    pub fn best(&self) -> Option<&Individual> {
        self.population.iter().max_by(|a, b| {
            a.primary_fitness()
                .partial_cmp(&b.primary_fitness())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Compute generation statistics.
    fn compute_stats(&self) -> GenerationStats {
        let fitnesses: Vec<f64> = self
            .population
            .iter()
            .map(|i| i.primary_fitness())
            .collect();

        let best = fitnesses.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let worst = fitnesses.iter().copied().fold(f64::INFINITY, f64::min);
        let avg = if fitnesses.is_empty() {
            0.0
        } else {
            fitnesses.iter().sum::<f64>() / fitnesses.len() as f64
        };

        // Diversity: standard deviation of fitness
        let variance = if fitnesses.len() > 1 {
            fitnesses.iter().map(|f| (f - avg).powi(2)).sum::<f64>() / (fitnesses.len() - 1) as f64
        } else {
            0.0
        };

        GenerationStats {
            generation: self.generation,
            best_fitness: best,
            avg_fitness: avg,
            worst_fitness: worst,
            diversity: variance.sqrt(),
            population_size: self.population.len(),
        }
    }

    /// Evolve one generation (assumes fitness has been set on all individuals).
    pub fn step(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Compute and record stats
        let stats = self.compute_stats();
        self.gen_stats.push(stats.clone());

        // Check for stagnation
        if stats.best_fitness > self.best_fitness {
            self.best_fitness = stats.best_fitness;
            self.stagnation_counter = 0;

            // Update hall of fame
            if let Some(best) = self.best() {
                self.hall_of_fame.push(best.clone());
                self.hall_of_fame.sort_by(|a, b| {
                    b.primary_fitness()
                        .partial_cmp(&a.primary_fitness())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                self.hall_of_fame.truncate(10);
            }
        } else {
            self.stagnation_counter += 1;
        }

        self.generation += 1;

        // Sort by fitness
        self.population.sort_by(|a, b| {
            b.primary_fitness()
                .partial_cmp(&a.primary_fitness())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let elite_count =
            (self.config.population_size as f64 * self.config.elitism_ratio).ceil() as usize;
        let mut next_gen: Vec<Individual> = Vec::new();

        // Elitism
        for ind in self.population.iter().take(elite_count) {
            let mut kept = ind.clone();
            kept.age_one_generation();
            next_gen.push(kept);
        }

        // Fill the rest with offspring
        while next_gen.len() < self.config.population_size {
            let parent_a = select(&self.population, self.config.selection).expect("parent selection failed — population empty");
            let parent_b = select(&self.population, self.config.selection).expect("parent selection failed — population empty");

            let child_genome = if rng.gen::<f64>() < self.config.crossover_rate {
                match (&parent_a.genome, &parent_b.genome) {
                    (Genome::RealValued(a), Genome::RealValued(b)) => {
                        let mut child = crossover_real(a, b, self.config.blend_alpha);
                        mutate_real(
                            &mut child,
                            self.config.mutation_rate,
                            self.config.mutation_sigma,
                        );
                        Genome::RealValued(child)
                    }
                    (Genome::Binary(a), Genome::Binary(b)) => {
                        let mut child = crossover_binary(a, b);
                        mutate_binary(&mut child, self.config.mutation_rate);
                        Genome::Binary(child)
                    }
                    _ => parent_a.genome.clone(),
                }
            } else {
                let mut g = parent_a.genome.clone();
                match &mut g {
                    Genome::RealValued(v) => {
                        mutate_real(v, self.config.mutation_rate, self.config.mutation_sigma);
                    }
                    Genome::Binary(v) => {
                        mutate_binary(v, self.config.mutation_rate);
                    }
                    _ => {}
                }
                g
            };

            let mut child = Individual::new(child_genome, self.generation);
            child.parents = vec![parent_a.id, parent_b.id];
            next_gen.push(child);
        }

        self.population = next_gen;
    }

    /// Check if evolution should stop.
    pub fn should_stop(&self) -> bool {
        self.generation >= self.config.max_generations
            || self.stagnation_counter >= self.config.stagnation_limit
    }

    /// Get stagnation counter.
    pub fn stagnation(&self) -> u32 {
        self.stagnation_counter
    }

    /// Get hall of fame.
    pub fn hall_of_fame(&self) -> &[Individual] {
        &self.hall_of_fame
    }

    /// Get generation statistics history.
    pub fn statistics(&self) -> &[GenerationStats] {
        &self.gen_stats
    }

    /// Get best all-time fitness.
    pub fn best_fitness(&self) -> f64 {
        self.best_fitness
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_individual_creation() {
        let ind = Individual::new(Genome::RealValued(vec![1.0, 2.0, 3.0]), 0);
        assert_eq!(ind.generation, 0);
        assert_eq!(ind.primary_fitness(), f64::NEG_INFINITY);
    }

    #[test]
    fn test_individual_fitness() {
        let mut ind = Individual::new(Genome::RealValued(vec![1.0]), 0);
        ind.set_fitness(0.95);
        assert_eq!(ind.primary_fitness(), 0.95);
    }

    #[test]
    fn test_pareto_dominance() {
        let mut a = Individual::new(Genome::RealValued(vec![]), 0);
        a.set_multi_fitness(vec![0.9, 0.8]);

        let mut b = Individual::new(Genome::RealValued(vec![]), 0);
        b.set_multi_fitness(vec![0.8, 0.7]);

        assert!(a.dominates(&b));
        assert!(!b.dominates(&a));
    }

    #[test]
    fn test_pareto_non_domination() {
        let mut a = Individual::new(Genome::RealValued(vec![]), 0);
        a.set_multi_fitness(vec![0.9, 0.5]);

        let mut b = Individual::new(Genome::RealValued(vec![]), 0);
        b.set_multi_fitness(vec![0.5, 0.9]);

        assert!(!a.dominates(&b));
        assert!(!b.dominates(&a));
    }

    #[test]
    fn test_tree_node() {
        let tree = TreeNode::node(
            "+",
            vec![
                TreeNode::leaf("x"),
                TreeNode::node("*", vec![TreeNode::leaf("2"), TreeNode::leaf("y")]),
            ],
        );
        assert_eq!(tree.size(), 5);
        assert_eq!(tree.depth(), 3);
    }

    #[test]
    fn test_tournament_selection() {
        let mut pop = Vec::new();
        for i in 0..10 {
            let mut ind = Individual::new(Genome::RealValued(vec![i as f64]), 0);
            ind.set_fitness(i as f64);
            pop.push(ind);
        }

        let selected = select(&pop, SelectionStrategy::Tournament(5)).unwrap();
        assert!(selected.primary_fitness() >= 0.0);
    }

    #[test]
    fn test_roulette_selection() {
        let mut pop = Vec::new();
        for i in 1..=5 {
            let mut ind = Individual::new(Genome::RealValued(vec![]), 0);
            ind.set_fitness(i as f64);
            pop.push(ind);
        }

        let selected = select(&pop, SelectionStrategy::RouletteWheel).unwrap();
        assert!(selected.primary_fitness() >= 1.0);
    }

    #[test]
    fn test_crossover_real() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let child = crossover_real(&a, &b, 0.5);
        assert_eq!(child.len(), 3);
    }

    #[test]
    fn test_crossover_binary() {
        let a = vec![true, true, true, true];
        let b = vec![false, false, false, false];
        let child = crossover_binary(&a, &b);
        assert_eq!(child.len(), 4);
    }

    #[test]
    fn test_mutate_real() {
        let mut genome = vec![1.0; 10];
        mutate_real(&mut genome, 1.0, 0.01); // Mutate all
                                             // At least some should have changed
        let unchanged = genome.iter().filter(|&&g| (g - 1.0).abs() < 1e-10).count();
        assert!(unchanged < 10);
    }

    #[test]
    fn test_evolution_engine() {
        let config = EvolutionConfig {
            population_size: 20,
            max_generations: 10,
            crossover_rate: 0.8,
            mutation_rate: 0.2,
            mutation_sigma: 0.1,
            elitism_ratio: 0.1,
            selection: SelectionStrategy::Tournament(3),
            stagnation_limit: 50,
            blend_alpha: 0.5,
        };

        let mut engine = EvolutionEngine::new(config);
        engine.initialize_real(5, -1.0, 1.0);
        assert_eq!(engine.population().len(), 20);

        // Set fitness: sphere function (minimize sum of squares, negate for maximization)
        for ind in engine.population_mut().iter_mut() {
            if let Genome::RealValued(ref v) = ind.genome {
                let fitness = -v.iter().map(|x| x * x).sum::<f64>();
                ind.set_fitness(fitness);
            }
        }

        // Evolve
        engine.step();
        assert_eq!(engine.generation(), 1);
        assert_eq!(engine.population().len(), 20);
        assert!(!engine.statistics().is_empty());
    }

    #[test]
    fn test_evolution_convergence() {
        let config = EvolutionConfig {
            population_size: 30,
            max_generations: 20,
            crossover_rate: 0.9,
            mutation_rate: 0.3,
            mutation_sigma: 0.5,
            elitism_ratio: 0.1,
            selection: SelectionStrategy::Tournament(3),
            stagnation_limit: 100,
            blend_alpha: 0.5,
        };

        let mut engine = EvolutionEngine::new(config);
        engine.initialize_real(3, -5.0, 5.0);

        for _ in 0..20 {
            for ind in engine.population_mut().iter_mut() {
                if let Genome::RealValued(ref v) = ind.genome {
                    let fitness = -v.iter().map(|x| x * x).sum::<f64>();
                    ind.set_fitness(fitness);
                }
            }
            engine.step();
        }

        // Best should have improved from random initialization
        assert!(engine.best_fitness() > -75.0); // 3 * 5^2 = 75 (worst case)
    }

    #[test]
    fn test_should_stop_max_gen() {
        let config = EvolutionConfig {
            max_generations: 2,
            ..Default::default()
        };
        let mut engine = EvolutionEngine::new(config);
        engine.initialize_real(1, 0.0, 1.0);

        for _ in 0..2 {
            for ind in engine.population_mut().iter_mut() {
                ind.set_fitness(0.5);
            }
            engine.step();
        }

        assert!(engine.should_stop());
    }

    #[test]
    fn test_select_empty_population() {
        let pop: Vec<Individual> = Vec::new();
        assert!(select(&pop, SelectionStrategy::Tournament(3)).is_none());
    }
}
