// framewerx::neurosymbolic::differentiable_logic — soft / fuzzy logic
// for end-to-end differentiable reasoning.

// Logic Tensor Networks (LTN, Serafini & Garcez).
S LogicTensorNetwork {
    embed_dim: usize,
    aggregation: s,
    t_norm: s,
}

I LogicTensorNetwork {
    +f new(embed_dim: usize) -> LogicTensorNetwork {
        @LogicTensorNetwork {
            embed_dim: embed_dim,
            aggregation: "p_mean_error",
            t_norm: "product",
        }
    }
}

// DeepProbLog: ProbLog atoms parameterised by neural nets.
S DeepProbLog {
    nn_atoms: [s]~,
    semiring: s,
}

I DeepProbLog {
    +f new() -> DeepProbLog {
        @DeepProbLog { nn_atoms: [], semiring: "probability" }
    }
}

// Semantic Loss (Xu et al.): penalises probability mass on
// constraint-violating predictions.
S SemanticLoss { constraint: s, weight: f32 }

// Differentiable SAT solver (NeuroSAT, SATNet).
S DifferentiableSAT { num_vars: usize, num_clauses: usize, t_norm: s }

// Differentiable Theorem Prover (NTP, Rocktaschel & Riedel).
S NeuralTheoremProver {
    embed_dim: usize,
    max_depth: usize,
    aggregation: s,
}

// Real-valued t-norms / t-conorms for fuzzy logic.
S TNorm { variant: s }
I TNorm {
    +f product() -> TNorm { @TNorm { variant: "product" } }
    +f godel() -> TNorm { @TNorm { variant: "godel" } }
    +f lukasiewicz() -> TNorm { @TNorm { variant: "lukasiewicz" } }
    +f nilpotent_min() -> TNorm { @TNorm { variant: "nilpotent_min" } }
}

// Markov Logic Networks: weighted first-order formulae.
S MarkovLogicNetwork { num_predicates: usize, num_formulae: usize }
