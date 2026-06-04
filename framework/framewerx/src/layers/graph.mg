// framewerx::layers::graph — graph neural network layers
//
// Graph layers operate on (node_features, adjacency) tuples. The
// bridge lowers them as a Linear + scatter-gather composition over
// RMI's tensor primitives.

S GCNLayer {
    in_features: usize,
    out_features: usize,
}

I GCNLayer {
    +f new(in_features: usize, out_features: usize) -> GCNLayer {
        @GCNLayer { in_features: in_features, out_features: out_features }
    }
}

// Graph Attention layer (Velickovic et al.).
S GATLayer {
    in_features: usize,
    out_features: usize,
    heads: usize,
}

I GATLayer {
    +f new(in_features: usize, out_features: usize, heads: usize) -> GATLayer {
        @GATLayer {
            in_features: in_features,
            out_features: out_features,
            heads: heads,
        }
    }
}

// Graph SAGE: sampled neighbour aggregation.
S GraphSAGELayer {
    in_features: usize,
    out_features: usize,
    aggregator: s,
}

I GraphSAGELayer {
    +f mean(in_features: usize, out_features: usize) -> GraphSAGELayer {
        @GraphSAGELayer {
            in_features: in_features,
            out_features: out_features,
            aggregator: "mean",
        }
    }

    +f max(in_features: usize, out_features: usize) -> GraphSAGELayer {
        @GraphSAGELayer {
            in_features: in_features,
            out_features: out_features,
            aggregator: "max",
        }
    }
}

// Edge convolution (used in PointNet, DGCNN).
S EdgeConv {
    in_features: usize,
    out_features: usize,
    k: usize,
}

I EdgeConv {
    +f new(in_features: usize, out_features: usize, k: usize) -> EdgeConv {
        @EdgeConv {
            in_features: in_features,
            out_features: out_features,
            k: k,
        }
    }
}

// Global readout: pool node features into a graph-level embedding.
S GlobalMeanPool {}
S GlobalMaxPool {}
S GlobalSumPool {}
