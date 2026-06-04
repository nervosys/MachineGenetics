// framewerx::symbolic::probabilistic — Bayesian networks, MRFs, causal.

// Bayesian Network: DAG of random variables with conditional probability tables.
S BayesianNetwork {
    num_nodes: usize,
    discrete: bool,
}

// Variable elimination inference.
S VariableElimination { ordering: s }

// Belief propagation / sum-product.
S BeliefPropagation { schedule: s, max_iters: usize }

// Junction tree algorithm.
S JunctionTree {}

// Markov Random Field (undirected graphical model).
S MarkovRandomField { num_nodes: usize, num_factors: usize }

// Hidden Markov Model.
S HMM { num_states: usize, num_observations: usize }

// Particle filter / sequential Monte Carlo.
S ParticleFilter { num_particles: usize, resample_threshold: f32 }

// Gibbs / Metropolis-Hastings MCMC.
S MCMCSampler { method: s, chains: usize, warmup: usize, samples: usize }

I MCMCSampler {
    +f nuts(samples: usize) -> MCMCSampler {
        @MCMCSampler { method: "NUTS", chains: 4, warmup: 1000, samples: samples }
    }
    +f hmc(samples: usize) -> MCMCSampler {
        @MCMCSampler { method: "HMC", chains: 4, warmup: 1000, samples: samples }
    }
}

// Variational Inference.
S VariationalInference { family: s, max_iters: usize, learning_rate: f32 }

// Structural Causal Model (Pearl).
S StructuralCausalModel {
    variables: [s]~,
    exogenous: [s]~,
    structural_equations: [s]~,
}

// do-calculus for causal queries.
S DoCalculus {}

// Probabilistic Programming runtime (handler-style).
S PPLRuntime { backend: s, autodiff: bool }
