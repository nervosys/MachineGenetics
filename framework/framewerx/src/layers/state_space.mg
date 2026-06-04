// framewerx::layers::state_space — Mamba / S4 / S5 selective SSMs
//
// State-space models compose: discretised continuous-time dynamics
// (A, B, C, D matrices) + selective gating (Mamba's input-dependent
// step size). The bridge lowers each variant to a Control::Scan over
// Linear+pointwise ops.

// S4: structured state-space sequence model (HiPPO initialisation).
S S4Layer {
    dim: usize,
    state_dim: usize,
}

I S4Layer {
    +f new(dim: usize, state_dim: usize) -> S4Layer {
        @S4Layer { dim: dim, state_dim: state_dim }
    }
}

// S5: parallel scan-based variant, multi-input multi-output.
S S5Layer {
    dim: usize,
    state_dim: usize,
    blocks: usize,
}

I S5Layer {
    +f new(dim: usize, state_dim: usize) -> S5Layer {
        @S5Layer { dim: dim, state_dim: state_dim, blocks: 1 }
    }
}

// Mamba: selective state-space with input-dependent dt/B/C.
// dim       — model dim (residual-stream width)
// state_dim — SSM hidden state width (typically 16)
// conv_kernel — 1-D causal conv width before the SSM (typically 4)
// expand    — inner expansion factor (typically 2)
S MambaBlock {
    dim: usize,
    state_dim: usize,
    conv_kernel: usize,
    expand: usize,
}

I MambaBlock {
    +f new(dim: usize) -> MambaBlock {
        @MambaBlock {
            dim: dim,
            state_dim: 16,
            conv_kernel: 4,
            expand: 2,
        }
    }
}

// H3: hybrid that interleaves SSM and attention layers.
S H3Layer {
    dim: usize,
    state_dim: usize,
    head_dim: usize,
}

I H3Layer {
    +f new(dim: usize) -> H3Layer {
        @H3Layer { dim: dim, state_dim: 16, head_dim: 64 }
    }
}
