// framewerx::architectures::graph_nets — full graph models
//
// Composes graph-layer primitives (GCN, GAT, SAGE) into trainable
// network specs. Each lowers to a chain of node-feature updates
// followed by a readout.

// Graph Convolutional Network (Kipf & Welling).
S GCN {
    input_dim: usize,
    hidden_dim: usize,
    output_dim: usize,
    num_layers: usize,
}

I GCN {
    +f new(input_dim: usize, hidden_dim: usize, output_dim: usize) -> GCN {
        @GCN {
            input_dim: input_dim,
            hidden_dim: hidden_dim,
            output_dim: output_dim,
            num_layers: 2,
        }
    }
}

// Graph Attention Network.
S GAT {
    input_dim: usize,
    hidden_dim: usize,
    output_dim: usize,
    heads: usize,
    num_layers: usize,
}

I GAT {
    +f new(input_dim: usize, hidden_dim: usize, output_dim: usize, heads: usize) -> GAT {
        @GAT {
            input_dim: input_dim,
            hidden_dim: hidden_dim,
            output_dim: output_dim,
            heads: heads,
            num_layers: 2,
        }
    }
}

// GraphSAGE: sample neighbours, aggregate, project.
S GraphSAGE {
    input_dim: usize,
    hidden_dim: usize,
    output_dim: usize,
    aggregator: s,
    num_layers: usize,
}

I GraphSAGE {
    +f mean(input_dim: usize, hidden_dim: usize, output_dim: usize) -> GraphSAGE {
        @GraphSAGE {
            input_dim: input_dim,
            hidden_dim: hidden_dim,
            output_dim: output_dim,
            aggregator: "mean",
            num_layers: 2,
        }
    }
}

// Message Passing Neural Network base.
S MPNN {
    node_dim: usize,
    edge_dim: usize,
    hidden_dim: usize,
    num_steps: usize,
}

I MPNN {
    +f new(node_dim: usize, edge_dim: usize, hidden_dim: usize) -> MPNN {
        @MPNN {
            node_dim: node_dim,
            edge_dim: edge_dim,
            hidden_dim: hidden_dim,
            num_steps: 3,
        }
    }
}
