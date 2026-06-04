// framewerx::neural::energy_based — energy-based models and flows.

// Energy-Based Model: scalar energy E(x) defines unnormalised density.
S EBM { input_dim: usize, energy_net_hidden: usize, mcmc_steps: usize }

// Restricted Boltzmann Machine.
S RBM { visible: usize, hidden: usize, k_gibbs: usize }

// Score-Based Generative Model (NCSN / VESDE).
S ScoreModel {
    input_dim: usize,
    sigma_min: f32,
    sigma_max: f32,
    num_scales: usize,
}

// Normalizing Flow base.
S NormalizingFlow { input_dim: usize, num_layers: usize, hidden_dim: usize }

// Real NVP coupling layer.
S RealNVPCoupling { dim: usize, hidden: usize, mask_pattern: s }

// Inverse Autoregressive Flow.
S IAF { dim: usize, hidden: usize, num_layers: usize }

// Continuous Normalising Flow (uses Neural ODE under the hood).
S CNF { dim: usize, hidden: usize, solver: s }
