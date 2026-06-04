// framewerx::neural::memory — memory-augmented and external-store networks.

// Neural Turing Machine: controller + read/write heads over a memory matrix.
S NeuralTuringMachine {
    controller_dim: usize,
    memory_rows: usize,
    memory_cols: usize,
    num_read_heads: usize,
    num_write_heads: usize,
}

I NeuralTuringMachine {
    +f new(controller_dim: usize, memory_rows: usize, memory_cols: usize) -> NeuralTuringMachine {
        @NeuralTuringMachine {
            controller_dim: controller_dim,
            memory_rows: memory_rows,
            memory_cols: memory_cols,
            num_read_heads: 1,
            num_write_heads: 1,
        }
    }
}

// Differentiable Neural Computer (DeepMind, 2016).
S DNC {
    controller_dim: usize,
    memory_rows: usize,
    memory_cols: usize,
    num_read_heads: usize,
}

// End-to-End Memory Network: multi-hop attention over a memory bank.
S MemoryNetwork {
    memory_size: usize,
    embed_dim: usize,
    num_hops: usize,
}

// Pointer Network: attention used as a pointer to input positions.
S PointerNetwork { input_dim: usize, hidden_dim: usize }

// Vector Symbolic Architecture binding ops (HRR, MAP, FHRR, BSC).
S VSABinder { dim: usize, scheme: s }

I VSABinder {
    +f hrr(dim: usize) -> VSABinder { @VSABinder { dim: dim, scheme: "HRR" } }
    +f map(dim: usize) -> VSABinder { @VSABinder { dim: dim, scheme: "MAP" } }
    +f fhrr(dim: usize) -> VSABinder { @VSABinder { dim: dim, scheme: "FHRR" } }
    +f bsc(dim: usize) -> VSABinder { @VSABinder { dim: dim, scheme: "BSC" } }
}
