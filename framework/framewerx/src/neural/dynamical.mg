// framewerx::neural::dynamical — continuous-time and biological models.

// Neural ODE: dh/dt = f(h, t); integrate via numeric ODE solver.
S NeuralODE { hidden_dim: usize, solver: s, rtol: f32, atol: f32 }

I NeuralODE {
    +f new(hidden_dim: usize) -> NeuralODE {
        @NeuralODE { hidden_dim: hidden_dim, solver: "dopri5", rtol: 0.001, atol: 0.0001 }
    }
}

// Neural SDE: stochastic continuous-time model.
S NeuralSDE { hidden_dim: usize, noise_dim: usize, solver: s }

// Liquid Neural Network: closed-form continuous-time (CfC) cell.
S LiquidCell { input_size: usize, hidden_size: usize, tau: f32 }

// Spiking Neural Network cells (Leaky Integrate-and-Fire, ALIF).
S LIF { hidden_dim: usize, tau_mem: f32, tau_syn: f32, threshold: f32 }
S ALIF { hidden_dim: usize, tau_mem: f32, tau_adp: f32, threshold: f32 }

// Hopfield network (modern, dense associative memory variant).
S ModernHopfield { dim: usize, num_patterns: usize, beta: f32 }

// Spiking Transformer (snntorch-style, surrogate gradient).
S SpikingTransformer { dim: usize, heads: usize, layers: usize, timesteps: usize }
