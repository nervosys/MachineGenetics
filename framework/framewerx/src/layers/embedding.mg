// framewerx::layers::embedding — token + positional embeddings
//
// Maps to RMI opcode 0x0005 (EMBED). The bridge already lowers
// `layer e: Embed(vocab, dim)` to that opcode. Higher-level wrappers
// here let agents declare positional / rotary variants without
// inventing new Machine Language opcodes.

S Embedding {
    vocab_size: usize,
    dim: usize,
}

I Embedding {
    +f new(vocab_size: usize, dim: usize) -> Embedding {
        @Embedding { vocab_size: vocab_size, dim: dim }
    }
}

// Sinusoidal positional embedding. Frozen at init; not trainable.
S PositionalEmbedding {
    max_len: usize,
    dim: usize,
}

I PositionalEmbedding {
    +f new(max_len: usize, dim: usize) -> PositionalEmbedding {
        @PositionalEmbedding { max_len: max_len, dim: dim }
    }
}

// Learned positional embedding. Same shape as PositionalEmbedding
// but the table is a trainable parameter.
S LearnedPositionEmbedding {
    max_len: usize,
    dim: usize,
}

// Rotary positional embedding (RoPE). Applied to query/key tensors
// inside attention; doesn't have its own opcode, the bridge inlines
// the rotation into the attention op.
S RotaryEmbedding {
    dim: usize,
    base: f32,
}

I RotaryEmbedding {
    +f new(dim: usize) -> RotaryEmbedding {
        @RotaryEmbedding { dim: dim, base: 10000.0 }
    }
}
