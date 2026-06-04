//! AI Concepts Ontology
//!
//! Machine-readable ontology of AI concepts, techniques, and paradigms
//! for AI agents to reason about when creating and evaluating architectures.

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Concept domain in AI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConceptDomain {
    /// Neural network related
    Neural,
    /// Symbolic AI related
    Symbolic,
    /// Hybrid approaches
    Neurosymbolic,
    /// Optimization and learning
    Learning,
    /// Architectures and models
    Architecture,
    /// Data and representations
    Representation,
    /// Inference and reasoning
    Reasoning,
    /// Computation and hardware
    Computation,
    /// Evaluation and metrics
    Evaluation,
    /// Safety and alignment
    Safety,
}

/// Relation types between concepts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConceptRelation {
    /// A is a subtype of B
    IsA,
    /// A is part of B
    PartOf,
    /// A requires B
    Requires,
    /// A enables B
    Enables,
    /// A is alternative to B
    AlternativeTo,
    /// A improves upon B
    Improves,
    /// A generalizes B
    Generalizes,
    /// A is instance of B
    InstanceOf,
    /// A uses B
    Uses,
    /// A computes B
    Computes,
    /// A optimizes B
    Optimizes,
}

/// An AI concept with machine-readable properties
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIConcept {
    /// Unique identifier
    pub id: Uuid,

    /// Concept name
    pub name: String,

    /// Domain
    pub domain: ConceptDomain,

    /// Machine-readable definition
    pub definition: String,

    /// Mathematical formulation (if applicable)
    pub math: Option<String>,

    /// Computational complexity
    pub complexity: Option<ComplexitySpec>,

    /// Properties and characteristics
    pub properties: HashMap<String, PropertyValue>,

    /// Applicable to these tasks
    pub applicable_tasks: Vec<String>,

    /// Contraindications (when not to use)
    pub contraindications: Vec<String>,

    /// Implementation hints for AI agents
    pub implementation_hints: Vec<String>,

    /// Tags for search
    pub tags: Vec<String>,
}

/// Computational complexity specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySpec {
    /// Time complexity (big-O notation)
    pub time: String,

    /// Space complexity
    pub space: String,

    /// Variables used in complexity expressions
    pub variables: HashMap<String, String>,
}

/// Property value types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PropertyValue {
    /// Boolean property
    Bool(bool),
    /// Integer property
    Int(i64),
    /// Float property
    Float(f64),
    /// String property
    String(String),
    /// List of strings
    List(Vec<String>),
    /// Range [min, max]
    Range(f64, f64),
}

impl AIConcept {
    /// Create a new concept
    pub fn new(name: impl Into<String>, domain: ConceptDomain) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            domain,
            definition: String::new(),
            math: None,
            complexity: None,
            properties: HashMap::new(),
            applicable_tasks: Vec::new(),
            contraindications: Vec::new(),
            implementation_hints: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Set definition
    pub fn with_definition(mut self, def: impl Into<String>) -> Self {
        self.definition = def.into();
        self
    }

    /// Set mathematical formulation
    pub fn with_math(mut self, math: impl Into<String>) -> Self {
        self.math = Some(math.into());
        self
    }

    /// Set complexity
    pub fn with_complexity(mut self, time: impl Into<String>, space: impl Into<String>) -> Self {
        self.complexity = Some(ComplexitySpec {
            time: time.into(),
            space: space.into(),
            variables: HashMap::new(),
        });
        self
    }

    /// Add property
    pub fn with_property(mut self, key: impl Into<String>, value: PropertyValue) -> Self {
        self.properties.insert(key.into(), value);
        self
    }

    /// Add applicable task
    pub fn with_task(mut self, task: impl Into<String>) -> Self {
        self.applicable_tasks.push(task.into());
        self
    }

    /// Add contraindication
    pub fn with_contraindication(mut self, contra: impl Into<String>) -> Self {
        self.contraindications.push(contra.into());
        self
    }

    /// Add implementation hint
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.implementation_hints.push(hint.into());
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.into_iter().map(|s| s.to_string()).collect();
        self
    }
}

/// The AI Concepts Ontology
pub struct AIConceptsOntology {
    /// All concepts indexed by ID
    concepts: HashMap<Uuid, AIConcept>,

    /// Name to ID mapping
    by_name: HashMap<String, Uuid>,

    /// Graph of concept relations
    graph: DiGraph<Uuid, ConceptRelation>,

    /// Node indices
    node_indices: HashMap<Uuid, NodeIndex>,

    /// Concepts by domain
    by_domain: HashMap<ConceptDomain, Vec<Uuid>>,
}

impl AIConceptsOntology {
    /// Create a new empty ontology
    pub fn new() -> Self {
        Self {
            concepts: HashMap::new(),
            by_name: HashMap::new(),
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            by_domain: HashMap::new(),
        }
    }

    /// Create ontology with core AI concepts
    pub fn with_core_concepts() -> Self {
        let mut ont = Self::new();
        ont.populate_core_concepts();
        ont
    }

    /// Add a concept
    pub fn add_concept(&mut self, concept: AIConcept) -> Uuid {
        let id = concept.id;
        let name = concept.name.to_lowercase();
        let domain = concept.domain;

        let idx = self.graph.add_node(id);
        self.node_indices.insert(id, idx);

        self.by_name.insert(name, id);
        self.by_domain.entry(domain).or_default().push(id);
        self.concepts.insert(id, concept);

        id
    }

    /// Add a relation between concepts
    pub fn add_relation(&mut self, from: Uuid, to: Uuid, relation: ConceptRelation) {
        if let (Some(&from_idx), Some(&to_idx)) =
            (self.node_indices.get(&from), self.node_indices.get(&to))
        {
            self.graph.add_edge(from_idx, to_idx, relation);
        }
    }

    /// Get concept by ID
    pub fn get(&self, id: &Uuid) -> Option<&AIConcept> {
        self.concepts.get(id)
    }

    /// Get concept by name
    pub fn get_by_name(&self, name: &str) -> Option<&AIConcept> {
        self.by_name
            .get(&name.to_lowercase())
            .and_then(|id| self.concepts.get(id))
    }

    /// Get ID by name
    pub fn get_id_by_name(&self, name: &str) -> Option<Uuid> {
        self.by_name.get(&name.to_lowercase()).copied()
    }

    /// Get concepts by domain, name-sorted (deterministic — agents cache
    /// and diff query results, so HashMap/HashSet order must not leak).
    pub fn by_domain(&self, domain: ConceptDomain) -> Vec<&AIConcept> {
        let mut v: Vec<&AIConcept> = self
            .by_domain
            .get(&domain)
            .map(|ids| ids.iter().filter_map(|id| self.concepts.get(id)).collect())
            .unwrap_or_default();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Get related concepts
    pub fn related(&self, id: &Uuid, relation: ConceptRelation) -> Vec<&AIConcept> {
        if let Some(&idx) = self.node_indices.get(id) {
            self.graph
                .edges(idx)
                .filter(|e| *e.weight() == relation)
                .filter_map(|e| {
                    let target_id = self.graph[e.target()];
                    self.concepts.get(&target_id)
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get parents (incoming relations)
    pub fn parents(&self, id: &Uuid, relation: ConceptRelation) -> Vec<&AIConcept> {
        if let Some(&idx) = self.node_indices.get(id) {
            self.graph
                .edges_directed(idx, Direction::Incoming)
                .filter(|e| *e.weight() == relation)
                .filter_map(|e| {
                    let source_id = self.graph[e.source()];
                    self.concepts.get(&source_id)
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all subtypes (transitive closure of IsA)
    pub fn all_subtypes(&self, id: &Uuid) -> Vec<&AIConcept> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        self.collect_subtypes(id, &mut result, &mut visited);
        result
    }

    fn collect_subtypes<'a>(
        &'a self,
        id: &Uuid,
        result: &mut Vec<&'a AIConcept>,
        visited: &mut HashSet<Uuid>,
    ) {
        if visited.contains(id) {
            return;
        }
        visited.insert(*id);

        for child in self.parents(id, ConceptRelation::IsA) {
            result.push(child);
            self.collect_subtypes(&child.id, result, visited);
        }
    }

    /// Find concepts applicable to a task, name-sorted (deterministic).
    pub fn for_task(&self, task: &str) -> Vec<&AIConcept> {
        let task_lower = task.to_lowercase();
        let mut v: Vec<&AIConcept> = self
            .concepts
            .values()
            .filter(|c| {
                c.applicable_tasks
                    .iter()
                    .any(|t| t.to_lowercase().contains(&task_lower))
            })
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Search concepts by tag, name-sorted (deterministic).
    pub fn by_tag(&self, tag: &str) -> Vec<&AIConcept> {
        let tag_lower = tag.to_lowercase();
        let mut v: Vec<&AIConcept> = self
            .concepts
            .values()
            .filter(|c| c.tags.iter().any(|t| t.to_lowercase() == tag_lower))
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    /// Populate with core AI concepts
    fn populate_core_concepts(&mut self) {
        // === NEURAL NETWORK CONCEPTS ===

        let neural_network = AIConcept::new("Neural Network", ConceptDomain::Neural)
            .with_definition("Computational model inspired by biological neurons, composed of layers of connected processing units")
            .with_property("type", PropertyValue::String("model".to_string()))
            .with_task("function approximation")
            .with_task("pattern recognition")
            .with_task("feature learning")
            .with_tags(vec!["neural", "model", "learning"]);
        let nn_id = self.add_concept(neural_network);

        let mlp = AIConcept::new("Multi-Layer Perceptron", ConceptDomain::Neural)
            .with_definition("Feedforward neural network with one or more hidden layers")
            .with_math(r"y = f(W_n f(W_{n-1} ... f(W_1 x + b_1) ... + b_{n-1}) + b_n)")
            .with_complexity("O(n²) per layer", "O(n²) for weights")
            .with_task("classification")
            .with_task("regression")
            .with_contraindication("Sequential data with long-range dependencies")
            .with_hint("Use ReLU activation for hidden layers to avoid vanishing gradients")
            .with_tags(vec!["feedforward", "dense", "mlp"]);
        let mlp_id = self.add_concept(mlp);
        self.add_relation(mlp_id, nn_id, ConceptRelation::IsA);

        let cnn = AIConcept::new("Convolutional Neural Network", ConceptDomain::Neural)
            .with_definition(
                "Neural network using convolution operations for translation equivariance",
            )
            .with_math(r"(f * g)(x) = \sum_k f(k) g(x-k)")
            .with_complexity("O(k² * c_in * c_out * h * w)", "O(k² * c_in * c_out)")
            .with_property("translation_equivariant", PropertyValue::Bool(true))
            .with_property("parameter_sharing", PropertyValue::Bool(true))
            .with_task("image classification")
            .with_task("object detection")
            .with_task("semantic segmentation")
            .with_hint("Use batch normalization between conv layers")
            .with_hint("Consider residual connections for deep networks")
            .with_tags(vec!["convolution", "vision", "cnn"]);
        let cnn_id = self.add_concept(cnn);
        self.add_relation(cnn_id, nn_id, ConceptRelation::IsA);

        let rnn = AIConcept::new("Recurrent Neural Network", ConceptDomain::Neural)
            .with_definition("Neural network with recurrent connections for sequential processing")
            .with_math(r"h_t = f(W_h h_{t-1} + W_x x_t + b)")
            .with_complexity("O(h² + hd) per timestep", "O(h² + hd)")
            .with_property("handles_sequences", PropertyValue::Bool(true))
            .with_task("sequence modeling")
            .with_task("time series")
            .with_contraindication("Very long sequences (>100 steps) due to vanishing gradients")
            .with_hint("Use LSTM or GRU for longer sequences")
            .with_tags(vec!["recurrent", "sequence", "rnn"]);
        let rnn_id = self.add_concept(rnn);
        self.add_relation(rnn_id, nn_id, ConceptRelation::IsA);

        let lstm = AIConcept::new("Long Short-Term Memory", ConceptDomain::Neural)
            .with_definition("RNN with gated memory cells for learning long-range dependencies")
            .with_math(r"c_t = f_t \odot c_{t-1} + i_t \odot \tilde{c}_t")
            .with_complexity("O(4h² + 4hd) per timestep", "O(4h² + 4hd)")
            .with_property("handles_long_sequences", PropertyValue::Bool(true))
            .with_property("gate_count", PropertyValue::Int(3))
            .with_task("language modeling")
            .with_task("speech recognition")
            .with_task("machine translation")
            .with_hint("Initialize forget gate bias to 1 for better gradient flow")
            .with_tags(vec!["lstm", "gated", "memory"]);
        let lstm_id = self.add_concept(lstm);
        self.add_relation(lstm_id, rnn_id, ConceptRelation::Improves);

        let transformer = AIConcept::new("Transformer", ConceptDomain::Neural)
            .with_definition("Architecture based purely on attention mechanisms without recurrence")
            .with_math(r"\text{Attention}(Q,K,V) = \text{softmax}(QK^T/\sqrt{d_k})V")
            .with_complexity("O(n² * d)", "O(n² + nd)")
            .with_property("parallelizable", PropertyValue::Bool(true))
            .with_property("handles_long_range", PropertyValue::Bool(true))
            .with_task("language modeling")
            .with_task("machine translation")
            .with_task("question answering")
            .with_task("image classification")
            .with_contraindication("Very long sequences due to O(n²) complexity")
            .with_hint("Use positional encoding for sequence order")
            .with_hint("Layer normalization before attention")
            .with_tags(vec!["transformer", "attention", "self-attention"]);
        let transformer_id = self.add_concept(transformer);
        self.add_relation(transformer_id, nn_id, ConceptRelation::IsA);
        self.add_relation(transformer_id, lstm_id, ConceptRelation::Improves);

        // === ACTIVATION FUNCTIONS ===

        let activation = AIConcept::new("Activation Function", ConceptDomain::Neural)
            .with_definition("Nonlinear function applied element-wise to introduce nonlinearity")
            .with_property("type", PropertyValue::String("function".to_string()))
            .with_tags(vec!["activation", "nonlinearity"]);
        let act_id = self.add_concept(activation);

        let relu = AIConcept::new("ReLU", ConceptDomain::Neural)
            .with_definition("Rectified Linear Unit: max(0, x)")
            .with_math(r"f(x) = \max(0, x)")
            .with_complexity("O(1)", "O(1)")
            .with_property(
                "derivative_at_zero",
                PropertyValue::String("undefined".to_string()),
            )
            .with_property("can_die", PropertyValue::Bool(true))
            .with_hint("Use Leaky ReLU or ELU if many dead neurons")
            .with_tags(vec!["relu", "activation"]);
        let relu_id = self.add_concept(relu);
        self.add_relation(relu_id, act_id, ConceptRelation::IsA);

        let gelu = AIConcept::new("GELU", ConceptDomain::Neural)
            .with_definition("Gaussian Error Linear Unit: x * Φ(x)")
            .with_math(
                r"f(x) = x \cdot \Phi(x) \approx 0.5x(1 + \tanh(\sqrt{2/\pi}(x + 0.044715x^3)))",
            )
            .with_property("smooth", PropertyValue::Bool(true))
            .with_task("transformer models")
            .with_hint("Default choice for transformers")
            .with_tags(vec!["gelu", "activation", "smooth"]);
        let gelu_id = self.add_concept(gelu);
        self.add_relation(gelu_id, act_id, ConceptRelation::IsA);
        self.add_relation(gelu_id, relu_id, ConceptRelation::Improves);

        // === OPTIMIZATION ===

        let optimizer = AIConcept::new("Optimizer", ConceptDomain::Learning)
            .with_definition("Algorithm for updating model parameters to minimize loss")
            .with_property("type", PropertyValue::String("algorithm".to_string()))
            .with_tags(vec!["optimizer", "learning", "training"]);
        let opt_id = self.add_concept(optimizer);

        let sgd = AIConcept::new("Stochastic Gradient Descent", ConceptDomain::Learning)
            .with_definition("Update parameters using gradient of mini-batch loss")
            .with_math(r"\theta_{t+1} = \theta_t - \eta \nabla_\theta L(\theta_t)")
            .with_property(
                "hyperparameters",
                PropertyValue::List(vec!["learning_rate".to_string()]),
            )
            .with_hint("Use learning rate warmup and decay")
            .with_tags(vec!["sgd", "gradient", "optimization"]);
        let sgd_id = self.add_concept(sgd);
        self.add_relation(sgd_id, opt_id, ConceptRelation::IsA);

        let adam = AIConcept::new("Adam", ConceptDomain::Learning)
            .with_definition("Adaptive moment estimation optimizer combining momentum and RMSprop")
            .with_math(
                r"m_t = \beta_1 m_{t-1} + (1-\beta_1)g_t, v_t = \beta_2 v_{t-1} + (1-\beta_2)g_t^2",
            )
            .with_property(
                "hyperparameters",
                PropertyValue::List(vec![
                    "learning_rate".to_string(),
                    "beta1".to_string(),
                    "beta2".to_string(),
                    "epsilon".to_string(),
                ]),
            )
            .with_property("default_beta1", PropertyValue::Float(0.9))
            .with_property("default_beta2", PropertyValue::Float(0.999))
            .with_hint("Good default optimizer for most tasks")
            .with_hint("May need weight decay (AdamW) for transformers")
            .with_tags(vec!["adam", "adaptive", "momentum"]);
        let adam_id = self.add_concept(adam);
        self.add_relation(adam_id, opt_id, ConceptRelation::IsA);
        self.add_relation(adam_id, sgd_id, ConceptRelation::Improves);

        // === REGULARIZATION ===

        let regularization = AIConcept::new("Regularization", ConceptDomain::Learning)
            .with_definition("Techniques to prevent overfitting and improve generalization")
            .with_property("type", PropertyValue::String("technique".to_string()))
            .with_tags(vec!["regularization", "generalization"]);
        let reg_id = self.add_concept(regularization);

        let dropout = AIConcept::new("Dropout", ConceptDomain::Learning)
            .with_definition("Randomly zero out activations during training")
            .with_math(r"\tilde{y} = y \odot m, m \sim \text{Bernoulli}(p)")
            .with_property("typical_rate", PropertyValue::Range(0.1, 0.5))
            .with_hint("Use higher dropout for larger networks")
            .with_hint("Don't use with batch normalization")
            .with_tags(vec!["dropout", "regularization"]);
        let dropout_id = self.add_concept(dropout);
        self.add_relation(dropout_id, reg_id, ConceptRelation::IsA);

        let weight_decay = AIConcept::new("Weight Decay", ConceptDomain::Learning)
            .with_definition("L2 penalty on weights to encourage smaller values")
            .with_math(r"L_{reg} = L + \lambda ||w||_2^2")
            .with_property("typical_lambda", PropertyValue::Range(1e-5, 1e-2))
            .with_tags(vec!["l2", "regularization", "weight-decay"]);
        let wd_id = self.add_concept(weight_decay);
        self.add_relation(wd_id, reg_id, ConceptRelation::IsA);

        // === NORMALIZATION ===

        let normalization = AIConcept::new("Normalization", ConceptDomain::Neural)
            .with_definition("Techniques to normalize activations for stable training")
            .with_property("type", PropertyValue::String("technique".to_string()))
            .with_tags(vec!["normalization", "training"]);
        let norm_id = self.add_concept(normalization);

        let batchnorm = AIConcept::new("Batch Normalization", ConceptDomain::Neural)
            .with_definition("Normalize activations across batch dimension")
            .with_math(r"\hat{x} = \frac{x - \mu_B}{\sqrt{\sigma_B^2 + \epsilon}}, y = \gamma\hat{x} + \beta")
            .with_property("requires_batch", PropertyValue::Bool(true))
            .with_task("CNN training")
            .with_contraindication("Small batch sizes")
            .with_contraindication("Transformers (use LayerNorm)")
            .with_hint("Place after convolution, before activation")
            .with_tags(vec!["batchnorm", "normalization"]);
        let bn_id = self.add_concept(batchnorm);
        self.add_relation(bn_id, norm_id, ConceptRelation::IsA);

        let layernorm = AIConcept::new("Layer Normalization", ConceptDomain::Neural)
            .with_definition("Normalize activations across feature dimension")
            .with_math(
                r"\hat{x} = \frac{x - \mu}{\sqrt{\sigma^2 + \epsilon}}, y = \gamma\hat{x} + \beta",
            )
            .with_property("batch_independent", PropertyValue::Bool(true))
            .with_task("Transformer training")
            .with_task("RNN training")
            .with_hint("Default choice for transformers")
            .with_tags(vec!["layernorm", "normalization"]);
        let ln_id = self.add_concept(layernorm);
        self.add_relation(ln_id, norm_id, ConceptRelation::IsA);

        // === SYMBOLIC AI ===

        let symbolic = AIConcept::new("Symbolic AI", ConceptDomain::Symbolic)
            .with_definition("AI based on manipulation of symbols and logical rules")
            .with_property("interpretable", PropertyValue::Bool(true))
            .with_property("requires_knowledge", PropertyValue::Bool(true))
            .with_task("reasoning")
            .with_task("planning")
            .with_task("expert systems")
            .with_tags(vec!["symbolic", "logic", "reasoning"]);
        let sym_id = self.add_concept(symbolic);

        let fol = AIConcept::new("First-Order Logic", ConceptDomain::Symbolic)
            .with_definition("Predicate logic with quantifiers over objects")
            .with_property("decidable", PropertyValue::Bool(false))
            .with_property("sound", PropertyValue::Bool(true))
            .with_property("complete", PropertyValue::Bool(true))
            .with_task("knowledge representation")
            .with_task("theorem proving")
            .with_tags(vec!["logic", "fol", "predicate"]);
        let fol_id = self.add_concept(fol);
        self.add_relation(fol_id, sym_id, ConceptRelation::PartOf);

        let unification = AIConcept::new("Unification", ConceptDomain::Symbolic)
            .with_definition("Finding substitution that makes two terms identical")
            .with_complexity("O(n²) naive, O(n) with union-find", "O(n)")
            .with_task("logic programming")
            .with_task("type inference")
            .with_tags(vec!["unification", "substitution", "matching"]);
        let unif_id = self.add_concept(unification);
        self.add_relation(unif_id, fol_id, ConceptRelation::Uses);

        // === NEUROSYMBOLIC ===

        let neurosymbolic = AIConcept::new("Neurosymbolic AI", ConceptDomain::Neurosymbolic)
            .with_definition("Integration of neural networks with symbolic reasoning")
            .with_property("combines_paradigms", PropertyValue::Bool(true))
            .with_task("reasoning with perception")
            .with_task("explainable AI")
            .with_task("knowledge-guided learning")
            .with_tags(vec!["neurosymbolic", "hybrid", "integration"]);
        let ns_id = self.add_concept(neurosymbolic);
        self.add_relation(ns_id, nn_id, ConceptRelation::Uses);
        self.add_relation(ns_id, sym_id, ConceptRelation::Uses);

        let symbol_embedding = AIConcept::new("Symbol Embedding", ConceptDomain::Neurosymbolic)
            .with_definition("Mapping symbolic structures to continuous vector spaces")
            .with_task("bridging neural and symbolic")
            .with_task("differentiable reasoning")
            .with_tags(vec!["embedding", "symbols", "vectors"]);
        let se_id = self.add_concept(symbol_embedding);
        self.add_relation(se_id, ns_id, ConceptRelation::PartOf);

        let soft_constraint = AIConcept::new("Soft Constraint", ConceptDomain::Neurosymbolic)
            .with_definition("Differentiable constraint for neural-symbolic optimization")
            .with_task("constrained optimization")
            .with_task("logical regularization")
            .with_tags(vec!["constraint", "soft", "differentiable"]);
        let sc_id = self.add_concept(soft_constraint);
        self.add_relation(sc_id, ns_id, ConceptRelation::PartOf);

        // === LOSS FUNCTIONS ===

        let loss = AIConcept::new("Loss Function", ConceptDomain::Learning)
            .with_definition("Function measuring model prediction error to minimize")
            .with_property("type", PropertyValue::String("function".to_string()))
            .with_tags(vec!["loss", "objective", "training"]);
        let loss_id = self.add_concept(loss);

        let cross_entropy = AIConcept::new("Cross-Entropy Loss", ConceptDomain::Learning)
            .with_definition("Loss for classification based on KL divergence")
            .with_math(r"L = -\sum_i y_i \log(\hat{y}_i)")
            .with_task("classification")
            .with_tags(vec!["cross-entropy", "classification"]);
        let ce_id = self.add_concept(cross_entropy);
        self.add_relation(ce_id, loss_id, ConceptRelation::IsA);

        let mse = AIConcept::new("Mean Squared Error", ConceptDomain::Learning)
            .with_definition("Average squared difference between predictions and targets")
            .with_math(r"L = \frac{1}{n}\sum_i (y_i - \hat{y}_i)^2")
            .with_task("regression")
            .with_tags(vec!["mse", "regression", "l2"]);
        let mse_id = self.add_concept(mse);
        self.add_relation(mse_id, loss_id, ConceptRelation::IsA);
    }
}

impl Default for AIConceptsOntology {
    fn default() -> Self {
        Self::with_core_concepts()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ontology_creation() {
        let ont = AIConceptsOntology::with_core_concepts();
        assert!(!ont.concepts.is_empty());
    }

    /// Agents cache/diff KB query output — by_domain/for_task/by_tag must
    /// not leak HashMap iteration order (they're name-sorted).
    #[test]
    fn kb_queries_are_name_sorted_and_stable() {
        let ont = AIConceptsOntology::with_core_concepts();
        let check_sorted = |v: &[&AIConcept], label: &str| {
            for w in v.windows(2) {
                assert!(w[0].name <= w[1].name, "{label} not sorted");
            }
        };
        let d1 = ont.by_domain(ConceptDomain::Neural);
        let d2 = ont.by_domain(ConceptDomain::Neural);
        assert_eq!(
            d1.iter().map(|c| &c.name).collect::<Vec<_>>(),
            d2.iter().map(|c| &c.name).collect::<Vec<_>>(),
            "by_domain stable across calls"
        );
        check_sorted(&d1, "by_domain");
        check_sorted(&ont.for_task("classification"), "for_task");
        check_sorted(&ont.by_tag("attention"), "by_tag");
    }

    #[test]
    fn test_get_by_name() {
        let ont = AIConceptsOntology::with_core_concepts();

        let transformer = ont.get_by_name("transformer");
        assert!(transformer.is_some());
        assert!(transformer.unwrap().name == "Transformer");
    }

    #[test]
    fn test_by_domain() {
        let ont = AIConceptsOntology::with_core_concepts();

        let neural = ont.by_domain(ConceptDomain::Neural);
        assert!(!neural.is_empty());
    }

    #[test]
    fn test_for_task() {
        let ont = AIConceptsOntology::with_core_concepts();

        let classification = ont.for_task("classification");
        assert!(!classification.is_empty());
    }

    #[test]
    fn test_related() {
        let ont = AIConceptsOntology::with_core_concepts();

        if let Some(id) = ont.get_id_by_name("multi-layer perceptron") {
            let parents = ont.related(&id, ConceptRelation::IsA);
            assert!(!parents.is_empty());
        }
    }

    #[test]
    fn test_concept_builder_chain() {
        let concept = AIConcept::new("TestConcept", ConceptDomain::Neural)
            .with_definition("A test concept")
            .with_math("f(x) = x^2")
            .with_complexity("O(n)", "O(1)")
            .with_task("testing")
            .with_contraindication("not for production")
            .with_hint("just a test")
            .with_tags(vec!["test", "demo"]);

        assert_eq!(concept.name, "TestConcept");
        assert_eq!(concept.domain, ConceptDomain::Neural);
        assert!(!concept.definition.is_empty());
        assert!(concept.math.is_some());
        assert!(concept.applicable_tasks.contains(&"testing".to_string()));
        assert!(concept
            .contraindications
            .contains(&"not for production".to_string()));
        assert!(concept
            .implementation_hints
            .contains(&"just a test".to_string()));
        assert!(concept.tags.contains(&"test".to_string()));
    }

    #[test]
    fn test_ontology_by_tag() {
        let ontology = AIConceptsOntology::with_core_concepts();
        // Core concepts should have tags
        let neural = ontology.by_tag("foundational");
        // by_tag should return a valid vec (possibly empty)
        let _ = neural.len();
    }

    #[test]
    fn test_ontology_get_id_by_name() {
        let ontology = AIConceptsOntology::with_core_concepts();
        let id = ontology.get_id_by_name("Neural Network");
        assert!(id.is_some(), "Core concepts should include Neural Network");
        let concept = ontology.get(&id.unwrap());
        assert!(concept.is_some());
    }

    #[test]
    fn test_property_value_variants() {
        let b = PropertyValue::Bool(true);
        let i = PropertyValue::Int(42);
        let f = PropertyValue::Float(std::f64::consts::PI);
        let s = PropertyValue::String("hello".to_string());
        let l = PropertyValue::List(vec!["a".to_string(), "b".to_string()]);
        let r = PropertyValue::Range(0.0, 1.0);

        // Just verify construction doesn't panic and debug prints work
        assert!(format!("{:?}", b).contains("true"));
        assert!(format!("{:?}", i).contains("42"));
        assert!(format!("{:?}", f).contains("3.14159"));
        assert!(format!("{:?}", s).contains("hello"));
        assert!(!format!("{:?}", l).is_empty());
        assert!(!format!("{:?}", r).is_empty());
    }
}
