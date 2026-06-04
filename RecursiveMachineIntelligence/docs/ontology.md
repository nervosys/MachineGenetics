# RecursiveMachineIntelligence AI Concepts Ontology

This document describes the AI concepts ontology embedded in RecursiveMachineIntelligence, providing machine-readable knowledge for agent reasoning.

---

## Overview

The ontology organizes AI concepts into a hierarchical graph with typed relationships. Agents use this for:

- **Semantic similarity**: Finding related concepts via graph distance
- **Concept grounding**: Mapping learned representations to symbolic knowledge
- **Knowledge transfer**: Identifying applicable techniques across domains

---

## Concept Domains

### Machine Learning (ML)

Core machine learning concepts and algorithms.

```
MachineLearning
├── SupervisedLearning
│   ├── Classification
│   │   ├── BinaryClassification
│   │   ├── MulticlassClassification
│   │   └── MultilabelClassification
│   └── Regression
│       ├── LinearRegression
│       ├── PolynomialRegression
│       └── LogisticRegression
├── UnsupervisedLearning
│   ├── Clustering
│   │   ├── KMeans
│   │   ├── DBSCAN
│   │   └── HierarchicalClustering
│   └── DimensionalityReduction
│       ├── PCA
│       ├── tSNE
│       └── UMAP
├── ReinforcementLearning
│   ├── ModelFree
│   │   ├── QLearning
│   │   ├── SARSA
│   │   ├── DQN
│   │   ├── PolicyGradient
│   │   └── ActorCritic
│   └── ModelBased
│       ├── MCTS
│       └── WorldModels
└── SemiSupervisedLearning
```

### Deep Learning (DL)

Neural network architectures and components.

```
DeepLearning
├── Architectures
│   ├── FeedForward
│   │   ├── MLP
│   │   └── ResNet
│   ├── Convolutional
│   │   ├── CNN
│   │   ├── LeNet
│   │   ├── AlexNet
│   │   ├── VGG
│   │   ├── GoogLeNet
│   │   └── DenseNet
│   ├── Recurrent
│   │   ├── RNN
│   │   ├── LSTM
│   │   ├── GRU
│   │   └── Bidirectional
│   ├── Attention
│   │   ├── SelfAttention
│   │   ├── CrossAttention
│   │   ├── MultiHeadAttention
│   │   └── SparseAttention
│   └── Transformer
│       ├── Encoder
│       ├── Decoder
│       ├── EncoderDecoder
│       ├── BERT
│       ├── GPT
│       └── T5
├── Components
│   ├── Layers
│   │   ├── Linear
│   │   ├── Conv2d
│   │   ├── MaxPool
│   │   ├── BatchNorm
│   │   ├── LayerNorm
│   │   ├── Dropout
│   │   └── Embedding
│   ├── Activations
│   │   ├── ReLU
│   │   ├── LeakyReLU
│   │   ├── GELU
│   │   ├── Sigmoid
│   │   ├── Tanh
│   │   ├── Softmax
│   │   └── Swish
│   └── Regularization
│       ├── L1
│       ├── L2
│       ├── Dropout
│       └── WeightDecay
└── Training
    ├── Optimization
    │   ├── SGD
    │   ├── Adam
    │   ├── AdamW
    │   ├── RMSprop
    │   └── LAMB
    ├── LearningRateSchedule
    │   ├── StepDecay
    │   ├── ExponentialDecay
    │   ├── CosineAnnealing
    │   └── WarmupLinear
    └── Techniques
        ├── BatchNormalization
        ├── ResidualConnections
        ├── GradientClipping
        └── MixedPrecision
```

### Symbolic AI (SYM)

Logic, knowledge representation, and reasoning.

```
SymbolicAI
├── Logic
│   ├── PropositionalLogic
│   │   ├── Conjunction
│   │   ├── Disjunction
│   │   ├── Negation
│   │   └── Implication
│   ├── FirstOrderLogic
│   │   ├── Quantifiers
│   │   ├── Predicates
│   │   ├── Functions
│   │   └── Unification
│   └── HigherOrderLogic
├── KnowledgeRepresentation
│   ├── SemanticNetworks
│   ├── Frames
│   ├── OntologyLanguages
│   │   ├── RDF
│   │   ├── OWL
│   │   └── KIF
│   └── KnowledgeGraphs
├── Reasoning
│   ├── ForwardChaining
│   ├── BackwardChaining
│   ├── Resolution
│   ├── Abduction
│   └── Analogy
└── Planning
    ├── STRIPS
    ├── PDDL
    ├── HTN
    └── MCTS
```

### Neurosymbolic AI (NS)

Integration of neural and symbolic approaches.

```
NeurosymbolicAI
├── Architecture
│   ├── SymbolicEmbedding
│   ├── NeuralKnowledgeGraph
│   ├── DifferentiableLogic
│   │   ├── FuzzyLogic
│   │   ├── ProbabilisticLogic
│   │   └── TensorLogic
│   └── NeuralTheoremProving
├── Integration
│   ├── SymbolicRegularization
│   ├── LogicLoss
│   ├── ConstraintSatisfaction
│   └── KnowledgeDistillation
└── Applications
    ├── VisualQuestionAnswering
    ├── KnowledgeGraphCompletion
    ├── NeuralProgramSynthesis
    └── ExplainableAI
```

### Multi-Agent Systems (MAS)

Agent architectures and coordination.

```
MultiAgentSystems
├── Architecture
│   ├── ReactiveAgents
│   ├── DeliberativeAgents
│   ├── HybridAgents
│   └── BDIAgents
├── Communication
│   ├── MessagePassing
│   ├── Blackboard
│   ├── PublishSubscribe
│   └── ContractNet
├── Coordination
│   ├── Cooperation
│   ├── Competition
│   ├── Negotiation
│   └── Coalition
└── Learning
    ├── IndependentLearners
    ├── JointActionLearners
    └── CommunicationLearning
```

---

## Relationships

### Hierarchy

- `is_a`: Subsumption (e.g., LSTM is_a RNN)
- `part_of`: Composition (e.g., MultiHeadAttention part_of Transformer)

### Semantic

- `related_to`: Conceptual similarity (e.g., BatchNorm related_to LayerNorm)
- `alternative_to`: Functionally interchangeable (e.g., ReLU alternative_to GELU)
- `extends`: Enhancement (e.g., ResNet extends CNN)

### Functional

- `used_for`: Application (e.g., CNN used_for ImageClassification)
- `requires`: Dependency (e.g., Backpropagation requires DifferentiableFunction)
- `improves`: Enhancement (e.g., ResidualConnections improves GradientFlow)

### Historical

- `builds_on`: Intellectual lineage (e.g., Transformer builds_on Attention)
- `introduced_by`: Attribution (e.g., Backpropagation introduced_by Rumelhart)
- `superseded_by`: Replacement (e.g., RNN superseded_by Transformer)

---

## Concept Properties

Each concept has:

| Property             | Type           | Description                           |
| -------------------- | -------------- | ------------------------------------- |
| name                 | String         | Unique identifier                     |
| domain               | Domain         | Primary domain (ML, DL, SYM, NS, MAS) |
| description          | String         | Natural language description          |
| math_notation        | Option<String> | LaTeX mathematical definition         |
| complexity           | Option<String> | Big-O complexity                      |
| implementation_hints | Vec<String>    | Code implementation notes             |

---

## Example Concepts

### Attention

```yaml
name: Attention
domain: DeepLearning
description: |
  Mechanism that computes weighted combinations of values based on 
  query-key similarity, enabling models to focus on relevant parts of input.
math_notation: |
  $\text{Attention}(Q, K, V) = \text{softmax}\left(\frac{QK^T}{\sqrt{d_k}}\right)V$
complexity: O(n² d)
implementation_hints:
  - Scale dot products by sqrt(d_k) for stable gradients
  - Use causal masking for autoregressive models
  - Flash attention for memory-efficient computation
relations:
  is_a: [Mechanism]
  part_of: [Transformer, MultiHeadAttention]
  builds_on: [SoftmaxFunction, DotProduct]
  used_for: [SequenceModeling, MachineTranslation]
```

### BackwardChaining

```yaml
name: BackwardChaining
domain: SymbolicAI
description: |
  Goal-directed inference that works backward from a goal, 
  recursively proving subgoals until reaching known facts.
math_notation: null
complexity: O(b^d) where b=branching, d=depth
implementation_hints:
  - Use occur check to prevent infinite loops
  - Implement memoization for repeated subgoals
  - Consider iterative deepening for completeness
relations:
  is_a: [InferenceMethod]
  alternative_to: [ForwardChaining]
  requires: [Unification, KnowledgeBase]
  used_for: [QueryAnswering, TheoremProving]
```

### ResidualConnection

```yaml
name: ResidualConnection
domain: DeepLearning
description: |
  Skip connection that adds input to layer output, enabling 
  training of very deep networks by providing gradient shortcuts.
math_notation: |
  $y = F(x) + x$
complexity: O(1) additional
implementation_hints:
  - Ensure input/output dimensions match
  - Use projection layer for dimension mismatch
  - Pre-norm variant often more stable
relations:
  is_a: [Connection]
  part_of: [ResNet, Transformer]
  improves: [GradientFlow, DeepNetworkTraining]
  introduced_by: [HeKaiming]
```

---

## Ontology API

### Rust Interface

```rust
use framewerx::core::ontology::{Ontology, Concept, Relation};

// Load ontology
let ontology = Ontology::new();

// Get concept
let attention = ontology.get_concept("Attention").unwrap();
println!("{}", attention.description);

// Find related concepts
let related = ontology.related_concepts("Attention", Relation::BuildsOn);
for concept in related {
    println!("- {}: {}", concept.name, concept.description);
}

// Compute similarity (graph-based)
let sim = ontology.similarity("LSTM", "GRU");
println!("LSTM-GRU similarity: {:.3}", sim);  // High, both RNNs

// Get all concepts in domain
let dl_concepts = ontology.by_domain(ConceptDomain::DeepLearning);
```

### Graph Queries

```rust
// Find path between concepts
let path = ontology.shortest_path("BackPropagation", "Transformer");
// [BackPropagation, NeuralNetwork, DeepLearning, Transformer]

// Get ancestors (transitive is_a)
let ancestors = ontology.ancestors("LSTM");
// [RNN, RecurrentArchitecture, NeuralNetwork, ...]

// Get descendants
let descendants = ontology.descendants("Attention");
// [SelfAttention, CrossAttention, MultiHeadAttention, ...]
```

---

## Using Ontology for Reasoning

### Symbol Grounding

Map neural representations to symbolic concepts:

```rust
use framewerx::neurosymbolic::SymbolEmbedding;

let mut embedder = SymbolEmbedding::new(config);

// Get embedding for ontology concept
let attn_vec = embedder.embed("Attention");
let lstm_vec = embedder.embed("LSTM");

// Similarity in embedding space
let sim = cosine_similarity(&attn_vec, &lstm_vec);
```

### Knowledge-Guided Inference

Use ontology to constrain neural inference:

```rust
let kb = KnowledgeBase::new();

// Add ontology facts
for concept in ontology.all_concepts() {
    for (related, relation) in concept.relations {
        let clause = Clause::fact(Predicate::new(
            relation.name(),
            vec![Term::symbol(&concept.name), Term::symbol(&related)],
        ));
        kb.add_fact(clause);
    }
}

// Query with ontology knowledge
let similar = kb.query(&Predicate::new("related_to", vec![
    Term::symbol("BatchNorm"),
    Term::variable("X"),
]));
```

---

## Extending the Ontology

### Adding Concepts

```rust
ontology.add_concept(Concept {
    name: "MoE".into(),
    domain: ConceptDomain::DeepLearning,
    description: "Mixture of Experts with sparse routing".into(),
    math_notation: Some("$y = \\sum_i g_i(x) E_i(x)$".into()),
    complexity: Some("O(n * k)".into()),
    implementation_hints: vec![
        "Use top-k routing for efficiency".into(),
        "Load balance with auxiliary loss".into(),
    ],
});
```

### Adding Relations

```rust
ontology.add_relation("MoE", "Transformer", Relation::UsedIn);
ontology.add_relation("MoE", "SparseGating", Relation::Uses);
ontology.add_relation("MoE", "Shazeer", Relation::IntroducedBy);
```

---

## Concept Index

Quick reference of key concepts:

| Concept             | Domain | Key Relation                    |
| ------------------- | ------ | ------------------------------- |
| Attention           | DL     | builds_on Softmax               |
| Transformer         | DL     | uses MultiHeadAttention         |
| BERT                | DL     | is_a Transformer                |
| GPT                 | DL     | is_a Transformer                |
| LSTM                | DL     | is_a RNN                        |
| BatchNorm           | DL     | improves Training               |
| BackwardChaining    | SYM    | requires Unification            |
| ForwardChaining     | SYM    | alternative_to BackwardChaining |
| KnowledgeGraph      | SYM    | used_for Reasoning              |
| SymbolicEmbedding   | NS     | integrates DL, SYM              |
| DifferentiableLogic | NS     | extends Logic                   |
| BDIAgent            | MAS    | uses Reasoning                  |

---

*RecursiveMachineIntelligence Ontology Reference v0.1.0*
