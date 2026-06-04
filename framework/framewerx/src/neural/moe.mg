// framewerx::neural::moe — mixture-of-experts routing and gating.

// Expert: a single feed-forward sub-network the router can dispatch to.
S Expert { hidden_dim: usize, output_dim: usize }

// Top-K router: picks the K highest-scoring experts per token.
S TopKRouter {
    num_experts: usize,
    top_k: usize,
    capacity_factor: f32,
}

I TopKRouter {
    +f new(num_experts: usize, top_k: usize) -> TopKRouter {
        @TopKRouter { num_experts: num_experts, top_k: top_k, capacity_factor: 1.25 }
    }
}

// Switch Transformer (Fedus et al.): always top-1, capacity-limited.
S SwitchRouter {
    num_experts: usize,
    capacity_factor: f32,
}

// Expert-Choice routing (each expert picks K tokens, not vice versa).
S ExpertChoiceRouter {
    num_experts: usize,
    capacity_per_expert: usize,
}

// Sparse MoE block: router + N experts + load-balancing loss.
S SparseMoE {
    dim: usize,
    num_experts: usize,
    top_k: usize,
    hidden_mult: usize,
    load_balance_alpha: f32,
}

I SparseMoE {
    +f new(dim: usize, num_experts: usize, top_k: usize) -> SparseMoE {
        @SparseMoE {
            dim: dim,
            num_experts: num_experts,
            top_k: top_k,
            hidden_mult: 4,
            load_balance_alpha: 0.01,
        }
    }
}

// Mixture-of-Depths: token-level skip via per-block router.
S MixtureOfDepths {
    dim: usize,
    capacity_factor: f32,
}
