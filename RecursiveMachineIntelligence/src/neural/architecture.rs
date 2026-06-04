//! Neural Network Architectures
//!
//! Provides architecture representation and manipulation for AI agents
//! to design, analyze, and optimize neural network topologies.

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use super::primitives::{HyperparameterValue, NeuralPrimitiveKind, ShapeSpec, TensorDType};

/// A node in a neural network architecture graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureNode {
    /// Unique identifier
    pub id: Uuid,

    /// The primitive operation at this node
    pub primitive: NeuralPrimitiveKind,

    /// Node name for identification
    pub name: String,

    /// Configuration for this node
    pub config: HashMap<String, HyperparameterValue>,

    /// Output shape specification
    pub output_shape: Option<ShapeSpec>,

    /// Output data type
    pub output_dtype: TensorDType,

    /// Whether this node's parameters are frozen
    pub frozen: bool,

    /// Agent-readable metadata for reasoning
    pub metadata: HashMap<String, String>,
}

impl ArchitectureNode {
    /// Create a new architecture node with the given primitive and name
    pub fn new(primitive: NeuralPrimitiveKind, name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            primitive,
            name: name.into(),
            config: HashMap::new(),
            output_shape: None,
            output_dtype: TensorDType::F32,
            frozen: false,
            metadata: HashMap::new(),
        }
    }

    /// Set a configuration value
    pub fn with_config(mut self, key: impl Into<String>, value: HyperparameterValue) -> Self {
        self.config.insert(key.into(), value);
        self
    }

    /// Set output shape
    pub fn with_shape(mut self, shape: ShapeSpec) -> Self {
        self.output_shape = Some(shape);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Freeze parameters
    pub fn freeze(mut self) -> Self {
        self.frozen = true;
        self
    }
}

/// An edge in the architecture graph (data flow)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArchitectureEdge {
    /// Source output index (for multi-output nodes)
    pub source_output: usize,

    /// Destination input index (for multi-input nodes)
    pub dest_input: usize,

    /// Optional transformation applied during connection
    pub transform: Option<EdgeTransform>,
}

/// Transformations that can be applied to architecture edges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeTransform {
    /// Reshape tensor to new shape
    Reshape(ShapeSpec),

    /// Transpose dimensions (permutation)
    Transpose(Vec<usize>),

    /// Slice tensor by range
    Slice {
        /// Start indices for each dimension
        start: Vec<usize>,
        /// End indices for each dimension
        end: Vec<usize>,
    },

    /// Cast to different data type
    Cast(TensorDType),

    /// Broadcast/repeat to new shape
    Broadcast(ShapeSpec),
}

/// A complete neural network architecture
#[derive(Debug)]
pub struct NetworkArchitecture {
    /// Unique identifier
    pub id: Uuid,

    /// Architecture name
    pub name: String,

    /// Version for tracking changes
    pub version: u64,

    /// The computation graph
    graph: DiGraph<ArchitectureNode, ArchitectureEdge>,

    /// Node ID to graph index mapping
    node_indices: HashMap<Uuid, NodeIndex>,

    /// Input nodes
    inputs: Vec<Uuid>,

    /// Output nodes
    outputs: Vec<Uuid>,

    /// Architecture-level metadata
    metadata: HashMap<String, String>,
}

impl NetworkArchitecture {
    /// Create a new empty architecture
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            version: 1,
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a node to the architecture
    pub fn add_node(&mut self, node: ArchitectureNode) -> Uuid {
        let node_id = node.id;
        let idx = self.graph.add_node(node);
        self.node_indices.insert(node_id, idx);
        self.version += 1;
        node_id
    }

    /// Connect two nodes
    pub fn connect(&mut self, from: Uuid, to: Uuid, edge: ArchitectureEdge) -> bool {
        if let (Some(&from_idx), Some(&to_idx)) =
            (self.node_indices.get(&from), self.node_indices.get(&to))
        {
            self.graph.add_edge(from_idx, to_idx, edge);
            self.version += 1;
            true
        } else {
            false
        }
    }

    /// Mark a node as an input
    pub fn mark_input(&mut self, node_id: Uuid) {
        if !self.inputs.contains(&node_id) {
            self.inputs.push(node_id);
        }
    }

    /// Mark a node as an output
    pub fn mark_output(&mut self, node_id: Uuid) {
        if !self.outputs.contains(&node_id) {
            self.outputs.push(node_id);
        }
    }

    /// Get a node by ID
    pub fn get_node(&self, id: Uuid) -> Option<&ArchitectureNode> {
        self.node_indices
            .get(&id)
            .and_then(|&idx| self.graph.node_weight(idx))
    }

    /// Get a mutable node by ID
    pub fn get_node_mut(&mut self, id: Uuid) -> Option<&mut ArchitectureNode> {
        if let Some(&idx) = self.node_indices.get(&id) {
            self.graph.node_weight_mut(idx)
        } else {
            None
        }
    }

    /// Remove a node
    pub fn remove_node(&mut self, id: Uuid) -> Option<ArchitectureNode> {
        if let Some(idx) = self.node_indices.remove(&id) {
            self.inputs.retain(|&x| x != id);
            self.outputs.retain(|&x| x != id);
            self.version += 1;
            self.graph.remove_node(idx)
        } else {
            None
        }
    }

    /// Get all nodes in topological order
    pub fn topological_order(&self) -> Option<Vec<Uuid>> {
        toposort(&self.graph, None).ok().map(|indices| {
            indices
                .into_iter()
                .filter_map(|idx| self.graph.node_weight(idx).map(|n| n.id))
                .collect()
        })
    }

    /// Get predecessors of a node
    pub fn predecessors(&self, id: Uuid) -> Vec<Uuid> {
        if let Some(&idx) = self.node_indices.get(&id) {
            self.graph
                .neighbors_directed(idx, Direction::Incoming)
                .filter_map(|i| self.graph.node_weight(i).map(|n| n.id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get successors of a node
    pub fn successors(&self, id: Uuid) -> Vec<Uuid> {
        if let Some(&idx) = self.node_indices.get(&id) {
            self.graph
                .neighbors_directed(idx, Direction::Outgoing)
                .filter_map(|i| self.graph.node_weight(i).map(|n| n.id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Count total nodes
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Count total edges
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Get all nodes
    pub fn nodes(&self) -> impl Iterator<Item = &ArchitectureNode> {
        self.graph.node_weights()
    }

    /// Get input nodes
    pub fn input_nodes(&self) -> Vec<&ArchitectureNode> {
        self.inputs
            .iter()
            .filter_map(|id| self.get_node(*id))
            .collect()
    }

    /// Get output nodes
    pub fn output_nodes(&self) -> Vec<&ArchitectureNode> {
        self.outputs
            .iter()
            .filter_map(|id| self.get_node(*id))
            .collect()
    }

    /// Find nodes by primitive type
    pub fn find_by_primitive(&self, primitive: NeuralPrimitiveKind) -> Vec<&ArchitectureNode> {
        self.graph
            .node_weights()
            .filter(|n| n.primitive == primitive)
            .collect()
    }

    /// Count parameters (estimated)
    pub fn estimate_parameters(&self, bindings: &HashMap<String, usize>) -> usize {
        // This is a simplified estimation
        // A full implementation would use the primitive registry
        self.graph
            .node_weights()
            .map(|node| {
                match node.primitive {
                    NeuralPrimitiveKind::Linear => {
                        let in_f = bindings.get("in_features").copied().unwrap_or(768);
                        let out_f = bindings.get("out_features").copied().unwrap_or(768);
                        in_f * out_f + out_f // weight + bias
                    }
                    NeuralPrimitiveKind::Conv2d => {
                        let in_c = bindings.get("in_channels").copied().unwrap_or(64);
                        let out_c = bindings.get("out_channels").copied().unwrap_or(64);
                        let k = bindings.get("kernel_size").copied().unwrap_or(3);
                        in_c * out_c * k * k + out_c
                    }
                    NeuralPrimitiveKind::LayerNorm | NeuralPrimitiveKind::BatchNorm => {
                        let hidden = bindings.get("hidden").copied().unwrap_or(768);
                        hidden * 2 // weight + bias
                    }
                    NeuralPrimitiveKind::Embedding => {
                        let vocab = bindings.get("vocab_size").copied().unwrap_or(50000);
                        let dim = bindings.get("embedding_dim").copied().unwrap_or(768);
                        vocab * dim
                    }
                    _ => 0,
                }
            })
            .sum()
    }

    /// Estimate FLOPs for forward pass
    pub fn estimate_flops(&self, bindings: &HashMap<String, usize>) -> usize {
        let batch = bindings.get("batch").copied().unwrap_or(1);
        let seq = bindings.get("seq").copied().unwrap_or(512);

        self.graph
            .node_weights()
            .map(|node| match node.primitive {
                NeuralPrimitiveKind::Linear => {
                    let in_f = bindings.get("in_features").copied().unwrap_or(768);
                    let out_f = bindings.get("out_features").copied().unwrap_or(768);
                    batch * seq * in_f * out_f * 2
                }
                NeuralPrimitiveKind::ScaledDotProductAttention
                | NeuralPrimitiveKind::MultiHeadAttention => {
                    let heads = bindings.get("heads").copied().unwrap_or(12);
                    let head_dim = bindings.get("head_dim").copied().unwrap_or(64);
                    batch * heads * seq * seq * head_dim * 4
                }
                NeuralPrimitiveKind::LayerNorm => {
                    let hidden = bindings.get("hidden").copied().unwrap_or(768);
                    batch * seq * hidden * 5
                }
                _ => 0,
            })
            .sum()
    }

    /// Get architecture depth (longest path)
    pub fn depth(&self) -> usize {
        if let Some(order) = self.topological_order() {
            let mut depths: HashMap<Uuid, usize> = HashMap::new();

            for id in order {
                let pred_depth = self
                    .predecessors(id)
                    .iter()
                    .filter_map(|p| depths.get(p))
                    .max()
                    .copied()
                    .unwrap_or(0);
                depths.insert(id, pred_depth + 1);
            }

            depths.values().max().copied().unwrap_or(0)
        } else {
            0
        }
    }

    /// Clone a subgraph
    pub fn clone_subgraph(&self, node_ids: &[Uuid]) -> NetworkArchitecture {
        let mut new_arch = NetworkArchitecture::new(format!("{}_subgraph", self.name));
        let mut id_map: HashMap<Uuid, Uuid> = HashMap::new();

        // Clone nodes
        for id in node_ids {
            if let Some(node) = self.get_node(*id) {
                let new_node = node.clone();
                let new_id = new_arch.add_node(new_node);
                id_map.insert(*id, new_id);
            }
        }

        // Clone edges between selected nodes
        for id in node_ids {
            if let Some(&idx) = self.node_indices.get(id) {
                for edge_ref in self.graph.edges_directed(idx, Direction::Outgoing) {
                    let target = self.graph.node_weight(edge_ref.target()).map(|n| n.id);

                    if let Some(target_id) = target {
                        if let (Some(&new_from), Some(&new_to)) =
                            (id_map.get(id), id_map.get(&target_id))
                        {
                            new_arch.connect(new_from, new_to, edge_ref.weight().clone());
                        }
                    }
                }
            }
        }

        new_arch
    }

    /// Set metadata
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }
}

/// Builder for constructing architectures programmatically
pub struct ArchitectureBuilder {
    arch: NetworkArchitecture,
    current_node: Option<Uuid>,
}

impl ArchitectureBuilder {
    /// Start building a new architecture
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            arch: NetworkArchitecture::new(name),
            current_node: None,
        }
    }

    /// Add an input node
    pub fn input(mut self, name: impl Into<String>, shape: ShapeSpec) -> Self {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::Embedding, name).with_shape(shape);
        let id = self.arch.add_node(node);
        self.arch.mark_input(id);
        self.current_node = Some(id);
        self
    }

    /// Add a linear layer
    pub fn linear(mut self, name: impl Into<String>, out_features: i64) -> Self {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::Linear, name)
            .with_config("out_features", HyperparameterValue::Int(out_features));
        let id = self.arch.add_node(node);

        if let Some(prev_id) = self.current_node {
            self.arch.connect(prev_id, id, ArchitectureEdge::default());
        }
        self.current_node = Some(id);
        self
    }

    /// Add a layer norm
    pub fn layer_norm(mut self, name: impl Into<String>) -> Self {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::LayerNorm, name);
        let id = self.arch.add_node(node);

        if let Some(prev_id) = self.current_node {
            self.arch.connect(prev_id, id, ArchitectureEdge::default());
        }
        self.current_node = Some(id);
        self
    }

    /// Add ReLU activation
    pub fn relu(mut self, name: impl Into<String>) -> Self {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::ReLU, name);
        let id = self.arch.add_node(node);

        if let Some(prev_id) = self.current_node {
            self.arch.connect(prev_id, id, ArchitectureEdge::default());
        }
        self.current_node = Some(id);
        self
    }

    /// Add GELU activation
    pub fn gelu(mut self, name: impl Into<String>) -> Self {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::GeLU, name);
        let id = self.arch.add_node(node);

        if let Some(prev_id) = self.current_node {
            self.arch.connect(prev_id, id, ArchitectureEdge::default());
        }
        self.current_node = Some(id);
        self
    }

    /// Add attention
    pub fn attention(mut self, name: impl Into<String>, heads: i64, head_dim: i64) -> Self {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::MultiHeadAttention, name)
            .with_config("heads", HyperparameterValue::Int(heads))
            .with_config("head_dim", HyperparameterValue::Int(head_dim));
        let id = self.arch.add_node(node);

        if let Some(prev_id) = self.current_node {
            self.arch.connect(prev_id, id, ArchitectureEdge::default());
        }
        self.current_node = Some(id);
        self
    }

    /// Add dropout
    pub fn dropout(mut self, name: impl Into<String>, p: f64) -> Self {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::Dropout, name)
            .with_config("p", HyperparameterValue::Float(p));
        let id = self.arch.add_node(node);

        if let Some(prev_id) = self.current_node {
            self.arch.connect(prev_id, id, ArchitectureEdge::default());
        }
        self.current_node = Some(id);
        self
    }

    /// Add a residual connection
    pub fn residual_add(mut self, name: impl Into<String>, skip_from: Uuid) -> Self {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::ResidualAdd, name);
        let id = self.arch.add_node(node);

        if let Some(prev_id) = self.current_node {
            self.arch.connect(prev_id, id, ArchitectureEdge::default());
        }
        self.arch.connect(
            skip_from,
            id,
            ArchitectureEdge {
                dest_input: 1,
                ..Default::default()
            },
        );
        self.current_node = Some(id);
        self
    }

    /// Mark current node as output
    pub fn output(mut self) -> Self {
        if let Some(id) = self.current_node {
            self.arch.mark_output(id);
        }
        self
    }

    /// Get the current node ID
    pub fn current(&self) -> Option<Uuid> {
        self.current_node
    }

    /// Fork the builder to create parallel branches
    pub fn fork(&self) -> Self {
        Self {
            arch: NetworkArchitecture::new("fork"),
            current_node: self.current_node,
        }
    }

    /// Build the architecture
    pub fn build(self) -> NetworkArchitecture {
        self.arch
    }
}

/// Create a standard transformer block architecture
pub fn transformer_block(hidden_dim: i64, ffn_dim: i64, heads: i64) -> NetworkArchitecture {
    let head_dim = hidden_dim / heads;

    let builder = ArchitectureBuilder::new("transformer_block");
    let builder = builder.input("input", ShapeSpec::new(vec![]));
    let input_id = builder.current().unwrap();

    let builder = builder
        .layer_norm("ln1")
        .attention("attn", heads, head_dim)
        .residual_add("residual1", input_id);

    let post_attn_id = builder.current().unwrap();

    builder
        .layer_norm("ln2")
        .linear("ffn1", ffn_dim)
        .gelu("activation")
        .linear("ffn2", hidden_dim)
        .residual_add("residual2", post_attn_id)
        .output()
        .build()
}

/// Create an MLP architecture
pub fn mlp(layer_dims: &[i64]) -> NetworkArchitecture {
    let mut builder = ArchitectureBuilder::new("mlp");
    builder = builder.input("input", ShapeSpec::new(vec![]));

    for (i, &dim) in layer_dims.iter().enumerate() {
        let name = format!("linear_{}", i);
        builder = builder.linear(&name, dim);

        if i < layer_dims.len() - 1 {
            builder = builder.relu(format!("relu_{}", i));
        }
    }

    builder.output().build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_architecture_creation() {
        let arch = ArchitectureBuilder::new("test")
            .input("input", ShapeSpec::new(vec![]))
            .linear("fc1", 256)
            .relu("relu1")
            .linear("fc2", 10)
            .output()
            .build();

        assert_eq!(arch.node_count(), 4);
        assert_eq!(arch.edge_count(), 3);
    }

    #[test]
    fn test_topological_order() {
        let arch = ArchitectureBuilder::new("test")
            .input("input", ShapeSpec::new(vec![]))
            .linear("fc1", 256)
            .linear("fc2", 10)
            .output()
            .build();

        let order = arch.topological_order().unwrap();
        assert_eq!(order.len(), 3);
    }

    #[test]
    fn test_transformer_block() {
        let block = transformer_block(768, 3072, 12);

        // Should have: input, ln1, attn, residual1, ln2, ffn1, gelu, ffn2, residual2
        assert!(block.node_count() >= 8);
        assert!(block.depth() > 1);
    }

    #[test]
    fn test_mlp() {
        let net = mlp(&[256, 128, 10]);

        // input + 3 linear + 2 relu = 6 nodes
        assert_eq!(net.node_count(), 6);
    }

    #[test]
    fn test_parameter_estimation() {
        let mut bindings = HashMap::new();
        bindings.insert("in_features".to_string(), 768);
        bindings.insert("out_features".to_string(), 3072);

        let arch = ArchitectureBuilder::new("test")
            .input("input", ShapeSpec::new(vec![]))
            .linear("fc1", 3072)
            .build();

        let params = arch.estimate_parameters(&bindings);
        // 768 * 3072 + 3072 = 2,362,368
        assert!(params > 2_000_000);
    }

    #[test]
    fn test_node_with_config() {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::Linear, "fc")
            .with_config("bias", HyperparameterValue::Bool(false));
        assert_eq!(node.name, "fc");
        assert!(!node.config.is_empty());
    }

    #[test]
    fn test_node_with_metadata() {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::ReLU, "act")
            .with_metadata("source", "test");
        assert_eq!(node.metadata.get("source").unwrap(), "test");
    }

    #[test]
    fn test_node_freeze() {
        let node = ArchitectureNode::new(NeuralPrimitiveKind::Linear, "fc").freeze();
        assert!(node.frozen);
    }

    #[test]
    fn test_arch_add_and_get_node() {
        let mut arch = NetworkArchitecture::new("test");
        let node = ArchitectureNode::new(NeuralPrimitiveKind::Linear, "fc");
        let id = arch.add_node(node);
        assert!(arch.get_node(id).is_some());
        assert_eq!(arch.get_node(id).unwrap().name, "fc");
    }

    #[test]
    fn test_arch_remove_node() {
        let mut arch = NetworkArchitecture::new("test");
        let node = ArchitectureNode::new(NeuralPrimitiveKind::Linear, "fc");
        let id = arch.add_node(node);
        assert_eq!(arch.node_count(), 1);
        let removed = arch.remove_node(id);
        assert!(removed.is_some());
        assert_eq!(arch.node_count(), 0);
    }

    #[test]
    fn test_arch_connect() {
        let mut arch = NetworkArchitecture::new("test");
        let n1 = arch.add_node(ArchitectureNode::new(NeuralPrimitiveKind::Linear, "a"));
        let n2 = arch.add_node(ArchitectureNode::new(NeuralPrimitiveKind::ReLU, "b"));
        assert!(arch.connect(n1, n2, ArchitectureEdge::default()));
        assert_eq!(arch.edge_count(), 1);
    }

    #[test]
    fn test_arch_predecessors_successors() {
        let mut arch = NetworkArchitecture::new("test");
        let n1 = arch.add_node(ArchitectureNode::new(NeuralPrimitiveKind::Linear, "a"));
        let n2 = arch.add_node(ArchitectureNode::new(NeuralPrimitiveKind::ReLU, "b"));
        arch.connect(n1, n2, ArchitectureEdge::default());
        assert_eq!(arch.successors(n1), vec![n2]);
        assert_eq!(arch.predecessors(n2), vec![n1]);
    }

    #[test]
    fn test_arch_input_output_nodes() {
        let mut arch = NetworkArchitecture::new("test");
        let n1 = arch.add_node(ArchitectureNode::new(NeuralPrimitiveKind::Linear, "in"));
        let n2 = arch.add_node(ArchitectureNode::new(NeuralPrimitiveKind::Linear, "out"));
        arch.mark_input(n1);
        arch.mark_output(n2);
        assert_eq!(arch.input_nodes().len(), 1);
        assert_eq!(arch.output_nodes().len(), 1);
    }

    #[test]
    fn test_arch_find_by_primitive() {
        let arch = ArchitectureBuilder::new("test")
            .input("input", ShapeSpec::new(vec![]))
            .linear("fc1", 128)
            .relu("act1")
            .linear("fc2", 64)
            .relu("act2")
            .output()
            .build();
        let relus = arch.find_by_primitive(NeuralPrimitiveKind::ReLU);
        assert_eq!(relus.len(), 2);
    }

    #[test]
    fn test_arch_depth() {
        let arch = ArchitectureBuilder::new("test")
            .input("input", ShapeSpec::new(vec![]))
            .linear("fc1", 128)
            .linear("fc2", 64)
            .output()
            .build();
        assert!(arch.depth() >= 2);
    }

    #[test]
    fn test_arch_metadata() {
        let mut arch = NetworkArchitecture::new("test");
        arch.set_metadata("version", "1.0");
        assert_eq!(arch.get_metadata("version").unwrap(), "1.0");
        assert!(arch.get_metadata("missing").is_none());
    }

    #[test]
    fn test_builder_fork() {
        let builder = ArchitectureBuilder::new("test")
            .input("input", ShapeSpec::new(vec![]))
            .linear("fc1", 128);
        let fork = builder.fork();
        assert_eq!(fork.current(), builder.current());
    }

    #[test]
    fn test_builder_dropout() {
        let arch = ArchitectureBuilder::new("test")
            .input("input", ShapeSpec::new(vec![]))
            .linear("fc1", 128)
            .dropout("drop1", 0.5)
            .output()
            .build();
        assert_eq!(arch.node_count(), 3);
    }

    #[test]
    fn test_builder_gelu() {
        let arch = ArchitectureBuilder::new("test")
            .input("input", ShapeSpec::new(vec![]))
            .linear("fc1", 128)
            .gelu("gelu1")
            .output()
            .build();
        let gelus = arch.find_by_primitive(NeuralPrimitiveKind::GeLU);
        assert_eq!(gelus.len(), 1);
    }

    #[test]
    fn test_arch_clone_subgraph() {
        let mut arch = NetworkArchitecture::new("test");
        let n1 = arch.add_node(ArchitectureNode::new(NeuralPrimitiveKind::Linear, "a"));
        let n2 = arch.add_node(ArchitectureNode::new(NeuralPrimitiveKind::ReLU, "b"));
        arch.connect(n1, n2, ArchitectureEdge::default());
        let sub = arch.clone_subgraph(&[n1, n2]);
        assert_eq!(sub.node_count(), 2);
    }

}
