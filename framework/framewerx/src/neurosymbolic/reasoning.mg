// framewerx::neurosymbolic::reasoning — concept bottlenecks, neural
// algorithmic reasoning, retrieval, tool use.

// Concept Bottleneck Model: features -> concepts -> task.
S ConceptBottleneckModel {
    input_dim: usize,
    num_concepts: usize,
    num_classes: usize,
    intervention: bool,
}

I ConceptBottleneckModel {
    +f new(input_dim: usize, num_concepts: usize, num_classes: usize) -> ConceptBottleneckModel {
        @ConceptBottleneckModel {
            input_dim: input_dim,
            num_concepts: num_concepts,
            num_classes: num_classes,
            intervention: 1b,
        }
    }
}

// Neural Algorithmic Reasoning: GNN trained to mimic a classical algorithm.
S NeuralAlgorithmicReasoner {
    target_algorithm: s,
    node_dim: usize,
    edge_dim: usize,
    num_steps: usize,
}

// Differentiable indexing into a knowledge store.
S DifferentiableIndex { keys_dim: usize, values_dim: usize, num_entries: usize }

// Retrieval-Augmented Generation: encoder + retriever + generator.
S RAG {
    query_encoder_dim: usize,
    doc_encoder_dim: usize,
    top_k: usize,
    rerank: bool,
}

I RAG {
    +f new(encoder_dim: usize, top_k: usize) -> RAG {
        @RAG {
            query_encoder_dim: encoder_dim,
            doc_encoder_dim: encoder_dim,
            top_k: top_k,
            rerank: 0b,
        }
    }
}

// Atlas / REALM: end-to-end-trained retrieval-augmented language model.
S Atlas { retriever_dim: usize, generator_dim: usize, num_passages: usize }

// FAISS-style vector index spec.
S VectorIndex { metric: s, index_type: s, dim: usize }

I VectorIndex {
    +f hnsw(dim: usize) -> VectorIndex {
        @VectorIndex { metric: "cosine", index_type: "HNSW", dim: dim }
    }
    +f ivfpq(dim: usize) -> VectorIndex {
        @VectorIndex { metric: "l2", index_type: "IVF_PQ", dim: dim }
    }
}

// Tool use / function calling wrapper.
S ToolCall {
    name: s,
    schema: s,
}

S ToolUsingAgent {
    base_model: s,
    available_tools: [s]~,
    max_calls_per_turn: usize,
}
