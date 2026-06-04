// framewerx::agentic::decoding — LLM decoding strategies and efficiency.

S GreedyDecode {}
S BeamSearch { beam_width: usize, length_penalty: f32 }
S TopKSampling { k: usize, temperature: f32 }
S TopPSampling { p: f32, temperature: f32 }
S MinPSampling { min_p: f32, temperature: f32 }
S TypicalSampling { tau: f32 }
S MirostatSampling { tau: f32, eta: f32 }

// Speculative decoding: draft model proposes, target model verifies.
S SpeculativeDecoding {
    target_model: s,
    draft_model: s,
    num_speculative_tokens: usize,
}

// Medusa: multi-head speculative decoding.
S Medusa {
    base_model: s,
    num_heads: usize,
    tree_size: usize,
}

// Constrained decoding: enforce grammar / regex / JSON-schema.
S ConstrainedDecode { grammar: s, format: s }

// Guidance / outlines-style structured generation.
S StructuredGenerator { schema: s, backend: s }

// Self-speculative decoding (one model is both draft and target).
S SelfSpeculative { num_heads: usize, num_speculative_tokens: usize }
