// framewerx::neural::attention_variants — modern attention efficient
// implementations and structured variants.
//
// All lower to the RMI ATTN opcode (0x0004) plus masking metadata.

// FlashAttention: tiled exact attention with O(N) memory.
S FlashAttention {
    dim: usize,
    heads: usize,
    head_dim: usize,
    causal: bool,
}

I FlashAttention {
    +f new(dim: usize, heads: usize) -> FlashAttention {
        @FlashAttention { dim: dim, heads: heads, head_dim: dim / heads, causal: 0b }
    }
    +f causal(dim: usize, heads: usize) -> FlashAttention {
        @FlashAttention { dim: dim, heads: heads, head_dim: dim / heads, causal: 1b }
    }
}

// SlidingWindowAttention: each token sees only window_size tokens behind.
S SlidingWindowAttention {
    dim: usize,
    heads: usize,
    window: usize,
}

// LongformerAttention: sliding window + global tokens.
S LongformerAttention { dim: usize, heads: usize, window: usize, num_global: usize }

// LinearAttention: O(N) via kernel feature maps.
S LinearAttention { dim: usize, heads: usize, feature_map: s }

// PerformerAttention: FAVOR+ random feature approximation.
S PerformerAttention { dim: usize, heads: usize, num_features: usize }

// GroupedQueryAttention (Llama-2/3 style): query heads share KV heads.
S GroupedQueryAttention {
    dim: usize,
    query_heads: usize,
    kv_heads: usize,
}

I GroupedQueryAttention {
    +f new(dim: usize, query_heads: usize, kv_heads: usize) -> GroupedQueryAttention {
        @GroupedQueryAttention { dim: dim, query_heads: query_heads, kv_heads: kv_heads }
    }
}

// MultiQueryAttention (MQA, PaLM-style): one KV head shared across all queries.
S MultiQueryAttention { dim: usize, query_heads: usize }

// CrossAttention: query from one sequence, key/value from another.
S CrossAttention { dim: usize, heads: usize, context_dim: usize }

// KVCache: stateful per-layer key/value buffer for autoregressive decoding.
S KVCache {
    max_seq_len: usize,
    num_layers: usize,
    num_heads: usize,
    head_dim: usize,
}

I KVCache {
    +f new(max_seq_len: usize, num_layers: usize, num_heads: usize, head_dim: usize) -> KVCache {
        @KVCache {
            max_seq_len: max_seq_len,
            num_layers: num_layers,
            num_heads: num_heads,
            head_dim: head_dim,
        }
    }
}
