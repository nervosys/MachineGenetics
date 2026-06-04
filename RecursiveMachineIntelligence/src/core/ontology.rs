//! Ontology System - Machine-Readable Knowledge Representation
//!
//! The ontology system provides a formal, machine-native representation of
//! concepts, relations, and axioms that AI agents can reason over. Unlike
//! human-oriented ontologies (which prioritize readability), this system
//! optimizes for:
//!
//! 1. Fast binary serialization for inter-agent communication
//! 2. Efficient query patterns for AI reasoning
//! 3. Support for uncertain/probabilistic knowledge
//! 4. Integration with neural embeddings
//! 5. Automatic composition and inheritance reasoning

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use ndarray::Array1;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{RmiError, Result};

/// Types of concepts in the ontology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ConceptType {
    /// Concrete things
    Entity = 0x01,
    /// Actions/transformations
    Process = 0x02,
    /// Attributes
    Property = 0x03,
    /// Connections between concepts
    Relation = 0x04,
    /// Logical constraints
    Constraint = 0x05,
    /// Universal truths
    Axiom = 0x06,
    /// Structural templates
    Schema = 0x07,
    /// Quantifiable aspects
    Measure = 0x08,
}

/// Types of relations between concepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum RelationType {
    // Taxonomic
    /// Subsumption (is-a)
    IsA = 0x01,
    /// Type membership
    InstanceOf = 0x02,
    /// Mereological (part-of)
    PartOf = 0x03,

    // Causal
    /// Causation
    Causes = 0x10,
    /// Enablement
    Enables = 0x11,
    /// Prevention
    Prevents = 0x12,

    // Temporal
    /// Precedence
    Precedes = 0x20,
    /// Following
    Follows = 0x21,
    /// Concurrency
    Concurrent = 0x22,

    // Logical
    /// Implication
    Implies = 0x30,
    /// Equivalence
    Equivalent = 0x31,
    /// Contradiction
    Contradicts = 0x32,

    // Structural
    /// Component relationship
    HasComponent = 0x40,
    /// Composition
    ComposedOf = 0x41,
    /// Transformation
    TransformsTo = 0x42,

    // Functional
    /// Input relation
    InputOf = 0x50,
    /// Output relation
    OutputOf = 0x51,
    /// Parameter relation
    ParameterOf = 0x52,
}

/// Unique identifier for a concept.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConceptId {
    /// Namespace (e.g., "air.neural", "air.symbolic")
    pub namespace: String,
    /// Local name
    pub local_name: String,
    /// Version number
    pub version: u32,
}

impl ConceptId {
    /// Create a new concept ID.
    #[inline]
    pub fn new(namespace: &str, local_name: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            local_name: local_name.to_string(),
            version: 1,
        }
    }

    /// Get the full URI.
    #[inline]
    pub fn uri(&self) -> String {
        format!("air://{}/{}", self.namespace, self.local_name)
    }

    /// Serialize to binary.
    pub fn to_binary(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).unwrap_or_default()
    }

    /// Deserialize from binary.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        rmp_serde::from_slice(data).map_err(|e| RmiError::Serialization(e.to_string()))
    }
}

impl std::fmt::Display for ConceptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}@v{}",
            self.namespace, self.local_name, self.version
        )
    }
}

/// A concept in the ontology - the fundamental unit of knowledge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    /// Unique identifier
    pub id: ConceptId,
    /// Type of concept
    pub concept_type: ConceptType,
    /// Human-readable label (for debugging only)
    pub label: String,
    /// Formal definition in a formal language
    pub formal_definition: String,
    /// Attributes as key-value pairs
    pub attributes: HashMap<String, AttributeValue>,
    /// Constraints as strings in constraint language
    pub constraints: Vec<String>,
    /// Confidence/probability (for uncertain knowledge)
    pub confidence: f64,
    /// Source/provenance
    pub source: Option<String>,
    /// Creation timestamp
    pub created_at: Option<i64>,
    /// Optional neural embedding
    #[serde(skip)]
    pub embedding: Option<Array1<f32>>,
    /// Embedding model identifier
    pub embedding_model: Option<String>,
}

/// Attribute value types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttributeValue {
    /// String value
    String(String),
    /// Integer value
    Int(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Bool(bool),
    /// List of values
    List(Vec<AttributeValue>),
    /// Nested map
    Map(HashMap<String, AttributeValue>),
    /// Binary data
    Binary(Vec<u8>),
}

impl Concept {
    /// Create a new concept.
    pub fn new(id: ConceptId, concept_type: ConceptType) -> Self {
        Self {
            id,
            concept_type,
            label: String::new(),
            formal_definition: String::new(),
            attributes: HashMap::new(),
            constraints: Vec::new(),
            confidence: 1.0,
            source: None,
            created_at: Some(chrono::Utc::now().timestamp()),
            embedding: None,
            embedding_model: None,
        }
    }

    /// Builder-style: set label.
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Builder-style: set formal definition.
    pub fn with_definition(mut self, definition: &str) -> Self {
        self.formal_definition = definition.to_string();
        self
    }

    /// Builder-style: set confidence.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Builder-style: add attribute.
    pub fn with_attribute(mut self, key: &str, value: AttributeValue) -> Self {
        self.attributes.insert(key.to_string(), value);
        self
    }

    /// Builder-style: add constraint.
    pub fn with_constraint(mut self, constraint: &str) -> Self {
        self.constraints.push(constraint.to_string());
        self
    }

    /// Builder-style: set embedding.
    pub fn with_embedding(mut self, embedding: Array1<f32>, model: &str) -> Self {
        self.embedding = Some(embedding);
        self.embedding_model = Some(model.to_string());
        self
    }

    /// Serialize to binary format.
    pub fn to_binary(&self) -> Vec<u8> {
        let packed = rmp_serde::to_vec(self).unwrap_or_default();

        // Append embedding if present
        if let Some(ref emb) = self.embedding {
            let emb_bytes: Vec<u8> = emb.iter().flat_map(|f| f.to_le_bytes()).collect();

            let mut result = Vec::with_capacity(4 + packed.len() + emb_bytes.len());
            result.extend_from_slice(&(packed.len() as u32).to_le_bytes());
            result.extend_from_slice(&packed);
            result.extend_from_slice(&emb_bytes);
            result
        } else {
            packed
        }
    }

    /// Content-addressable hash.
    pub fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.to_binary());
        let result = hasher.finalize();
        hex::encode(&result[..8])
    }
}

/// A relation instance between two concepts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    /// Source concept
    pub source: ConceptId,
    /// Relation type
    pub relation_type: RelationType,
    /// Target concept
    pub target: ConceptId,
    /// Confidence score
    pub confidence: f64,
    /// Is this relation bidirectional?
    pub bidirectional: bool,
    /// Constraints specific to this relation
    pub constraints: Vec<String>,
    /// Valid from timestamp
    pub valid_from: Option<i64>,
    /// Valid until timestamp
    pub valid_until: Option<i64>,
}

impl Relation {
    /// Create a new relation.
    pub fn new(source: ConceptId, relation_type: RelationType, target: ConceptId) -> Self {
        Self {
            source,
            relation_type,
            target,
            confidence: 1.0,
            bidirectional: false,
            constraints: Vec::new(),
            valid_from: None,
            valid_until: None,
        }
    }

    /// Serialize to binary.
    pub fn to_binary(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).unwrap_or_default()
    }
}

/// Query builder for the ontology.
pub struct OntologyQuery {
    type_filter: Option<ConceptType>,
    relation_filters: Vec<(RelationType, Option<ConceptId>)>,
    attribute_filters: Vec<(String, AttributeValue)>,
    min_confidence: Option<f64>,
    limit: Option<usize>,
    traversal_depth: Option<usize>,
}

impl OntologyQuery {
    /// Create a new query.
    pub fn new() -> Self {
        Self {
            type_filter: None,
            relation_filters: Vec::new(),
            attribute_filters: Vec::new(),
            min_confidence: None,
            limit: None,
            traversal_depth: None,
        }
    }

    /// Filter by concept type.
    pub fn filter_type(mut self, concept_type: ConceptType) -> Self {
        self.type_filter = Some(concept_type);
        self
    }

    /// Filter by having a specific relation type.
    pub fn filter_has_relation(mut self, rel_type: RelationType) -> Self {
        self.relation_filters.push((rel_type, None));
        self
    }

    /// Filter by attribute value.
    pub fn filter_attribute(mut self, key: &str, value: AttributeValue) -> Self {
        self.attribute_filters.push((key.to_string(), value));
        self
    }

    /// Filter by minimum confidence.
    pub fn filter_confidence(mut self, min_conf: f64) -> Self {
        self.min_confidence = Some(min_conf);
        self
    }

    /// Limit number of results.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Set traversal depth for relation queries.
    pub fn traverse_depth(mut self, depth: usize) -> Self {
        self.traversal_depth = Some(depth);
        self
    }

    /// Serialize query for remote execution.
    pub fn to_binary(&self) -> Vec<u8> {
        rmp_serde::to_vec(&QuerySpec {
            type_filter: self.type_filter,
            min_confidence: self.min_confidence,
            limit: self.limit,
            traversal_depth: self.traversal_depth,
        })
        .unwrap_or_default()
    }
}

impl Default for OntologyQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize)]
struct QuerySpec {
    type_filter: Option<ConceptType>,
    min_confidence: Option<f64>,
    limit: Option<usize>,
    traversal_depth: Option<usize>,
}

/// The main ontology container.
pub struct Ontology {
    /// Namespace for this ontology
    pub namespace: String,
    /// Concepts by ID
    concepts: RwLock<HashMap<ConceptId, Concept>>,
    /// Relations as a graph
    graph: RwLock<DiGraph<ConceptId, RelationType>>,
    /// Node index mapping
    node_indices: RwLock<HashMap<ConceptId, NodeIndex>>,
    /// Index by concept type
    by_type: RwLock<HashMap<ConceptType, HashSet<ConceptId>>>,
    /// Index by local name
    by_name: RwLock<HashMap<String, ConceptId>>,
    /// Embedding index for similarity search
    embedding_index: RwLock<Option<EmbeddingIndex>>,
}

struct EmbeddingIndex {
    embeddings: Vec<Array1<f32>>,
    ids: Vec<ConceptId>,
}

impl Ontology {
    /// Create a new ontology.
    pub fn new(namespace: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            concepts: RwLock::new(HashMap::new()),
            graph: RwLock::new(DiGraph::new()),
            node_indices: RwLock::new(HashMap::new()),
            by_type: RwLock::new(HashMap::new()),
            by_name: RwLock::new(HashMap::new()),
            embedding_index: RwLock::new(None),
        }
    }

    /// Add a concept to the ontology.
    pub fn add_concept(&self, concept: Concept) {
        let id = concept.id.clone();
        let ctype = concept.concept_type;
        let name = concept.id.local_name.clone();
        let embedding = concept.embedding.clone();

        // Add to graph
        let node_idx = {
            let mut graph = self.graph.write().unwrap();
            graph.add_node(id.clone())
        };
        self.node_indices
            .write()
            .unwrap()
            .insert(id.clone(), node_idx);

        // Store concept
        self.concepts.write().unwrap().insert(id.clone(), concept);

        // Index by type
        self.by_type
            .write()
            .unwrap()
            .entry(ctype)
            .or_default()
            .insert(id.clone());

        // Index by name
        self.by_name.write().unwrap().insert(name, id.clone());

        // Update embedding index if present
        if let Some(emb) = embedding {
            self.update_embedding_index(id, emb);
        }
    }

    /// Batch-add concepts (amortizes lock acquisition over N concepts).
    pub fn add_concepts(&self, concepts: Vec<Concept>) {
        let mut graph = self.graph.write().unwrap();
        let mut node_indices = self.node_indices.write().unwrap();
        let mut concepts_map = self.concepts.write().unwrap();
        let mut by_type = self.by_type.write().unwrap();
        let mut by_name = self.by_name.write().unwrap();

        for concept in concepts {
            let id = concept.id.clone();
            let ctype = concept.concept_type;
            let name = concept.id.local_name.clone();
            let embedding = concept.embedding.clone();

            let node_idx = graph.add_node(id.clone());
            node_indices.insert(id.clone(), node_idx);
            concepts_map.insert(id.clone(), concept);
            by_type
                .entry(ctype)
                .or_default()
                .insert(id.clone());
            by_name.insert(name, id.clone());

            if let Some(emb) = embedding {
                // Inline embedding index update to avoid re-acquiring locks
                drop(by_name);
                drop(by_type);
                drop(concepts_map);
                drop(node_indices);
                drop(graph);
                self.update_embedding_index(id, emb);
                graph = self.graph.write().unwrap();
                node_indices = self.node_indices.write().unwrap();
                concepts_map = self.concepts.write().unwrap();
                by_type = self.by_type.write().unwrap();
                by_name = self.by_name.write().unwrap();
            }
        }
    }

    /// Batch-get multiple concepts by ID (single lock acquisition).
    pub fn get_many(&self, ids: &[ConceptId]) -> Vec<Option<Concept>> {
        let concepts = self.concepts.read().unwrap();
        ids.iter()
            .map(|id| concepts.get(id).cloned())
            .collect()
    }

    fn update_embedding_index(&self, id: ConceptId, embedding: Array1<f32>) {
        let mut index = self.embedding_index.write().unwrap();
        if let Some(ref mut idx) = *index {
            idx.embeddings.push(embedding);
            idx.ids.push(id);
        } else {
            *index = Some(EmbeddingIndex {
                embeddings: vec![embedding],
                ids: vec![id],
            });
        }
    }

    /// Add a relation between concepts.
    pub fn add_relation(&self, relation: Relation) {
        let node_indices = self.node_indices.read().unwrap();

        if let (Some(&src_idx), Some(&tgt_idx)) = (
            node_indices.get(&relation.source),
            node_indices.get(&relation.target),
        ) {
            let mut graph = self.graph.write().unwrap();
            graph.add_edge(src_idx, tgt_idx, relation.relation_type);

            if relation.bidirectional {
                // Add inverse relation
                let inverse_type = Self::inverse_relation_type(relation.relation_type);
                graph.add_edge(tgt_idx, src_idx, inverse_type);
            }
        }
    }

    fn inverse_relation_type(rel_type: RelationType) -> RelationType {
        match rel_type {
            RelationType::IsA => RelationType::IsA, // Special case
            RelationType::Precedes => RelationType::Follows,
            RelationType::Follows => RelationType::Precedes,
            RelationType::Causes => RelationType::Causes, // Causation doesn't have simple inverse
            RelationType::PartOf => RelationType::HasComponent,
            RelationType::HasComponent => RelationType::PartOf,
            RelationType::InputOf => RelationType::OutputOf,
            RelationType::OutputOf => RelationType::InputOf,
            other => other,
        }
    }

    /// Get a concept by ID.
    #[inline]
    pub fn get(&self, id: &ConceptId) -> Option<Concept> {
        self.concepts.read().unwrap().get(id).cloned()
    }

    /// Lookup a concept by local name.
    pub fn lookup(&self, name: &str) -> Option<Concept> {
        let by_name = self.by_name.read().unwrap();
        by_name
            .get(name)
            .and_then(|id| self.concepts.read().unwrap().get(id).cloned())
    }

    /// Execute a query against the ontology.
    pub fn query(&self, q: &OntologyQuery) -> Vec<Concept> {
        let concepts = self.concepts.read().unwrap();
        let mut results: Vec<&Concept> = concepts.values().collect();

        // Apply type filter
        if let Some(ctype) = q.type_filter {
            results.retain(|c| c.concept_type == ctype);
        }

        // Apply confidence filter
        if let Some(min_conf) = q.min_confidence {
            results.retain(|c| c.confidence >= min_conf);
        }

        // Apply attribute filters
        for (key, value) in &q.attribute_filters {
            results.retain(|c| {
                c.attributes.get(key).is_some_and(|v| match (v, value) {
                    (AttributeValue::String(a), AttributeValue::String(b)) => a == b,
                    (AttributeValue::Int(a), AttributeValue::Int(b)) => a == b,
                    (AttributeValue::Float(a), AttributeValue::Float(b)) => {
                        (a - b).abs() < f64::EPSILON
                    }
                    (AttributeValue::Bool(a), AttributeValue::Bool(b)) => a == b,
                    _ => false,
                })
            });
        }

        // Deterministic order: HashMap iteration is arbitrary, but agents
        // cache/diff query results (and `limit` would otherwise select a
        // *random subset*). Sort by id before applying the limit.
        results.sort_by(|a, b| {
            (&a.id.namespace, &a.id.local_name, a.id.version)
                .cmp(&(&b.id.namespace, &b.id.local_name, b.id.version))
        });

        // Apply limit
        let limit = q.limit.unwrap_or(usize::MAX);
        results.into_iter().take(limit).cloned().collect()
    }

    /// Find concepts with similar embeddings using cosine similarity.
    pub fn find_similar(
        &self,
        query_embedding: &Array1<f32>,
        k: usize,
        threshold: f64,
    ) -> Vec<(Concept, f64)> {
        let index = self.embedding_index.read().unwrap();
        let concepts = self.concepts.read().unwrap();

        if let Some(ref idx) = *index {
            let query_norm = l2_norm(query_embedding);
            if query_norm < f32::EPSILON {
                return Vec::new();
            }

            let mut similarities: Vec<(usize, f64)> = idx
                .embeddings
                .iter()
                .enumerate()
                .map(|(i, emb)| {
                    let emb_norm = l2_norm(emb);
                    if emb_norm < f32::EPSILON {
                        (i, 0.0)
                    } else {
                        let dot: f32 = query_embedding
                            .iter()
                            .zip(emb.iter())
                            .map(|(a, b)| a * b)
                            .sum();
                        (i, (dot / (query_norm * emb_norm)) as f64)
                    }
                })
                .filter(|(_, sim)| *sim >= threshold)
                .collect();

            similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            similarities.truncate(k);

            similarities
                .into_iter()
                .filter_map(|(i, sim)| {
                    let id = &idx.ids[i];
                    concepts.get(id).map(|c| (c.clone(), sim))
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get related concepts by following relations.
    pub fn get_related(&self, id: &ConceptId, rel_type: RelationType) -> Vec<Concept> {
        let node_indices = self.node_indices.read().unwrap();
        let graph = self.graph.read().unwrap();
        let concepts = self.concepts.read().unwrap();

        if let Some(&node_idx) = node_indices.get(id) {
            let mut v: Vec<Concept> = graph
                .edges(node_idx)
                .filter(|e| *e.weight() == rel_type)
                .filter_map(|e| {
                    let target_idx = e.target();
                    graph
                        .node_weight(target_idx)
                        .and_then(|tid| concepts.get(tid).cloned())
                })
                .collect();
            // Deterministic order (contractual, not just petgraph insertion
            // order): agents diff relation lists like any other query output.
            v.sort_by(|a, b| {
                (&a.id.namespace, &a.id.local_name, a.id.version)
                    .cmp(&(&b.id.namespace, &b.id.local_name, b.id.version))
            });
            v
        } else {
            Vec::new()
        }
    }

    /// Extract a subgraph rooted at a concept.
    pub fn get_subgraph(
        &self,
        root: &ConceptId,
        max_depth: usize,
        relation_types: Option<&HashSet<RelationType>>,
    ) -> Ontology {
        let subgraph = Ontology::new(&format!("{}.subgraph", self.namespace));

        let node_indices = self.node_indices.read().unwrap();
        let graph = self.graph.read().unwrap();
        let concepts = self.concepts.read().unwrap();

        let mut visited = HashSet::new();
        let mut frontier: Vec<ConceptId> = vec![root.clone()];

        for _depth in 0..max_depth {
            if frontier.is_empty() {
                break;
            }

            let mut new_frontier = Vec::new();

            for id in frontier {
                if visited.contains(&id) {
                    continue;
                }
                visited.insert(id.clone());

                // Add concept to subgraph
                if let Some(concept) = concepts.get(&id) {
                    subgraph.add_concept(concept.clone());
                }

                // Find neighbors
                if let Some(&node_idx) = node_indices.get(&id) {
                    for edge in graph.edges(node_idx) {
                        let rel_type = *edge.weight();

                        // Filter by relation types if specified
                        if let Some(types) = relation_types {
                            if !types.contains(&rel_type) {
                                continue;
                            }
                        }

                        if let Some(target_id) = graph.node_weight(edge.target()) {
                            if !visited.contains(target_id) {
                                new_frontier.push(target_id.clone());
                            }
                        }
                    }
                }
            }

            frontier = new_frontier;
        }

        subgraph
    }

    /// Merge another ontology into this one.
    pub fn merge(&self, other: &Ontology, strategy: MergeStrategy) {
        let other_concepts = other.concepts.read().unwrap();

        for (id, concept) in other_concepts.iter() {
            let should_add = {
                let concepts = self.concepts.read().unwrap();
                if let Some(existing) = concepts.get(id) {
                    match strategy {
                        MergeStrategy::KeepNewer => id.version > existing.id.version,
                        MergeStrategy::KeepHigherConfidence => {
                            concept.confidence > existing.confidence
                        }
                        MergeStrategy::MergeRelations => true,
                        MergeStrategy::Replace => true,
                    }
                } else {
                    true
                }
            };

            if should_add {
                self.add_concept(concept.clone());
            }
        }
    }

    /// Serialize entire ontology to binary.
    pub fn to_binary(&self) -> Vec<u8> {
        let concepts = self.concepts.read().unwrap();
        let graph = self.graph.read().unwrap();

        // Extract relations from graph
        let relations: Vec<Relation> = graph
            .edge_indices()
            .filter_map(|e| {
                let (src, tgt) = graph.edge_endpoints(e)?;
                let rel_type = *graph.edge_weight(e)?;
                let src_id = graph.node_weight(src)?;
                let tgt_id = graph.node_weight(tgt)?;
                Some(Relation::new(src_id.clone(), rel_type, tgt_id.clone()))
            })
            .collect();

        let data = OntologyData {
            namespace: self.namespace.clone(),
            concepts: concepts.values().cloned().collect(),
            relations,
        };

        let packed = rmp_serde::to_vec(&data).unwrap_or_default();
        lz4_flex::compress_prepend_size(&packed)
    }

    /// Deserialize from binary format.
    pub fn from_binary(data: &[u8]) -> Result<Self> {
        let decompressed = lz4_flex::decompress_size_prepended(data)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;

        let ont_data: OntologyData = rmp_serde::from_slice(&decompressed)
            .map_err(|e| RmiError::Serialization(e.to_string()))?;

        let ontology = Ontology::new(&ont_data.namespace);

        for concept in ont_data.concepts {
            ontology.add_concept(concept);
        }

        for relation in ont_data.relations {
            ontology.add_relation(relation);
        }

        Ok(ontology)
    }

    /// Load an ontology from a URI.
    pub fn load(uri: &str) -> Result<Self> {
        if uri.starts_with("air://") {
            // Built-in ontologies - return default for now
            // In future, will load from embedded resources
            Ok(Ontology::new("builtin"))
        } else if let Some(path) = uri.strip_prefix("file://") {
            let data = std::fs::read(path)?;
            Self::from_binary(&data)
        } else {
            Err(RmiError::ontology_simple(format!(
                "Unsupported URI scheme: {}",
                uri
            )))
        }
    }

    /// Save ontology to a file.
    pub fn save(&self, path: &str) -> Result<()> {
        let data = self.to_binary();
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Get the number of concepts.
    #[inline]
    pub fn len(&self) -> usize {
        self.concepts.read().unwrap().len()
    }

    /// Check if empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.concepts.read().unwrap().is_empty()
    }
}

/// Strategy for merging ontologies.
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    /// Keep concept with higher version
    KeepNewer,
    /// Keep concept with higher confidence
    KeepHigherConfidence,
    /// Merge relations from both
    MergeRelations,
    /// Replace existing with new
    Replace,
}

#[derive(Serialize, Deserialize)]
struct OntologyData {
    namespace: String,
    concepts: Vec<Concept>,
    relations: Vec<Relation>,
}

#[inline]
fn l2_norm(arr: &Array1<f32>) -> f32 {
    arr.iter().map(|x| x * x).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Agents cache/diff query output — it must be byte-stable across
    /// repeated queries AND across independently-built ontologies with the
    /// same content (HashMap iteration order must never leak through).
    #[test]
    fn query_order_is_deterministic() {
        let build = || {
            let ont = Ontology::new("det-test");
            // Insert in scrambled order on purpose.
            for name in ["zeta", "alpha", "mid", "beta", "omega"] {
                ont.add_concept(Concept::new(
                    ConceptId::new("test.det", name),
                    ConceptType::Entity,
                ));
            }
            ont
        };
        let names = |ont: &Ontology| -> Vec<String> {
            ont.query(&OntologyQuery::new())
                .into_iter()
                .map(|c| c.id.local_name)
                .collect()
        };
        let o1 = build();
        let o2 = build();
        assert_eq!(names(&o1), names(&o2), "same content → same order");
        assert_eq!(names(&o1), names(&o1), "repeat query → same order");
        // Sorted by id, so the order is the lexicographic one.
        let n = names(&o1);
        let mut sorted = n.clone();
        sorted.sort();
        assert_eq!(n, sorted, "query output is id-sorted");
        // And `limit` now selects a *deterministic* prefix, not a random subset.
        let first2 = o1.query(&OntologyQuery::new().limit(2));
        assert_eq!(first2.len(), 2);
        assert_eq!(first2[0].id.local_name, "alpha");
        assert_eq!(first2[1].id.local_name, "beta");
    }

    #[test]
    fn test_concept_creation() {
        let id = ConceptId::new("test", "relu");
        let concept = Concept::new(id.clone(), ConceptType::Process)
            .with_label("ReLU Activation")
            .with_confidence(0.95)
            .with_attribute("monotonic", AttributeValue::Bool(true));

        assert_eq!(concept.id, id);
        assert_eq!(concept.concept_type, ConceptType::Process);
        assert!(concept.confidence > 0.9);
    }

    #[test]
    fn test_ontology_basic() {
        let ont = Ontology::new("test");

        let id1 = ConceptId::new("test", "neural_network");
        let concept1 = Concept::new(id1.clone(), ConceptType::Entity).with_label("Neural Network");

        let id2 = ConceptId::new("test", "transformer");
        let concept2 = Concept::new(id2.clone(), ConceptType::Entity).with_label("Transformer");

        ont.add_concept(concept1);
        ont.add_concept(concept2);
        ont.add_relation(Relation::new(id2.clone(), RelationType::IsA, id1.clone()));

        assert_eq!(ont.len(), 2);

        let related = ont.get_related(&id2, RelationType::IsA);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].id, id1);
    }

    #[test]
    fn test_concept_id_uri() {
        let id = ConceptId::new("air.neural", "transformer");
        assert_eq!(id.uri(), "air://air.neural/transformer");
    }

    #[test]
    fn test_concept_id_binary_roundtrip() {
        let id = ConceptId::new("air.symbolic", "modus_ponens");
        let binary = id.to_binary();
        let restored = ConceptId::from_binary(&binary).unwrap();
        assert_eq!(id, restored);
    }

    #[test]
    fn test_concept_attributes() {
        let id = ConceptId::new("test", "attention");
        let concept = Concept::new(id, ConceptType::Process)
            .with_label("Multi-Head Attention")
            .with_definition("Scaled dot-product attention with multiple heads")
            .with_confidence(0.99)
            .with_attribute("num_heads", AttributeValue::Int(8))
            .with_attribute("dropout", AttributeValue::Float(0.1))
            .with_constraint("num_heads > 0");

        assert_eq!(concept.label, "Multi-Head Attention");
        assert!(concept.confidence > 0.98);
        assert_eq!(concept.constraints.len(), 1);
        assert!(concept.attributes.contains_key("num_heads"));
    }

    #[test]
    fn test_concept_content_hash_deterministic() {
        let id = ConceptId::new("test", "relu");
        let c1 = Concept::new(id.clone(), ConceptType::Process).with_label("ReLU");
        let c2 = Concept::new(id, ConceptType::Process).with_label("ReLU");

        assert_eq!(c1.content_hash(), c2.content_hash());
    }

    #[test]
    fn test_ontology_lookup_by_name() {
        let ont = Ontology::new("test");
        let id = ConceptId::new("test", "softmax");
        ont.add_concept(Concept::new(id, ConceptType::Process).with_label("Softmax"));

        let found = ont.lookup("softmax");
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "Softmax");

        assert!(ont.lookup("nonexistent").is_none());
    }

    #[test]
    fn test_ontology_get_many() {
        let ont = Ontology::new("test");
        let id1 = ConceptId::new("test", "a");
        let id2 = ConceptId::new("test", "b");
        let id3 = ConceptId::new("test", "c");

        ont.add_concept(Concept::new(id1.clone(), ConceptType::Entity));
        ont.add_concept(Concept::new(id2.clone(), ConceptType::Entity));

        let results = ont.get_many(&[id1, id3, id2]);
        assert!(results[0].is_some());
        assert!(results[1].is_none());
        assert!(results[2].is_some());
    }

    #[test]
    fn test_ontology_batch_add() {
        let ont = Ontology::new("test");
        let concepts = vec![
            Concept::new(ConceptId::new("test", "x"), ConceptType::Entity),
            Concept::new(ConceptId::new("test", "y"), ConceptType::Process),
            Concept::new(ConceptId::new("test", "z"), ConceptType::Property),
        ];
        ont.add_concepts(concepts);
        assert_eq!(ont.len(), 3);
    }

    #[test]
    fn test_ontology_query_by_type() {
        let ont = Ontology::new("test");
        ont.add_concept(Concept::new(ConceptId::new("t", "a"), ConceptType::Entity));
        ont.add_concept(Concept::new(ConceptId::new("t", "b"), ConceptType::Process));
        ont.add_concept(Concept::new(ConceptId::new("t", "c"), ConceptType::Entity));

        let q = OntologyQuery::new().filter_type(ConceptType::Entity);
        let results = ont.query(&q);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_ontology_query_by_confidence() {
        let ont = Ontology::new("test");
        ont.add_concept(Concept::new(ConceptId::new("t", "a"), ConceptType::Entity).with_confidence(0.9));
        ont.add_concept(Concept::new(ConceptId::new("t", "b"), ConceptType::Entity).with_confidence(0.5));
        ont.add_concept(Concept::new(ConceptId::new("t", "c"), ConceptType::Entity).with_confidence(0.95));

        let q = OntologyQuery::new().filter_confidence(0.85);
        let results = ont.query(&q);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_ontology_query_with_limit() {
        let ont = Ontology::new("test");
        for i in 0..10 {
            ont.add_concept(Concept::new(ConceptId::new("t", &format!("c{}", i)), ConceptType::Entity));
        }

        let q = OntologyQuery::new().limit(3);
        let results = ont.query(&q);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_ontology_relations() {
        let ont = Ontology::new("test");
        let parent = ConceptId::new("t", "nn");
        let child = ConceptId::new("t", "cnn");

        ont.add_concept(Concept::new(parent.clone(), ConceptType::Entity).with_label("Neural Net"));
        ont.add_concept(Concept::new(child.clone(), ConceptType::Entity).with_label("CNN"));
        ont.add_relation(Relation::new(child.clone(), RelationType::IsA, parent.clone()));

        let related = ont.get_related(&child, RelationType::IsA);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].label, "Neural Net");
    }

    #[test]
    fn test_ontology_binary_roundtrip() {
        let ont = Ontology::new("test");
        ont.add_concept(Concept::new(ConceptId::new("t", "a"), ConceptType::Entity).with_label("A"));
        ont.add_concept(Concept::new(ConceptId::new("t", "b"), ConceptType::Process).with_label("B"));

        let binary = ont.to_binary();
        let restored = Ontology::from_binary(&binary).unwrap();
        assert_eq!(restored.len(), 2);
        assert!(restored.lookup("a").is_some());
    }

    #[test]
    fn test_ontology_empty() {
        let ont = Ontology::new("empty");
        assert!(ont.is_empty());
        assert_eq!(ont.len(), 0);
        assert!(ont.get(&ConceptId::new("x", "y")).is_none());
    }
}
