# RecursiveMachineIntelligence AI Concepts Ontology

This document describes the AI concepts ontology embedded in
RecursiveMachineIntelligence, providing machine-readable knowledge for agent
reasoning.

> **Read the domain taxonomies as a design sketch, not an inventory.** The
> relation set, the domain set and the concept-property table below were
> checked against `knowledge/ai_concepts.rs` on 2026-08-18 and each was wrong;
> they are corrected. The per-domain concept trees were *measured* rather than
> corrected — see the note under "Concept Domains" for the numbers.

---

## Overview

The ontology organizes AI concepts into a hierarchical graph with typed relationships. Agents use this for:

- **Semantic similarity**: Finding related concepts via graph distance
- **Concept grounding**: Mapping learned representations to symbolic knowledge
- **Knowledge transfer**: Identifying applicable techniques across domains

---

## Concept Domains

`ConceptDomain` has **ten** variants: `Neural`, `Symbolic`, `Neurosymbolic`,
`Learning`, `Architecture`, `Representation`, `Reasoning`, `Computation`,
`Evaluation`, `Safety`.

> **Corrected 2026-08-18.** The five headings below — ML, DL, SYM, NS, MAS —
> are not the domain set the code uses, and only `Symbolic`/`Neurosymbolic`
> have a counterpart at all. There is no `MultiAgentSystems` domain, and
> `Learning`, `Architecture`, `Representation`, `Reasoning`, `Computation`,
> `Evaluation` and `Safety` have no heading here.
>
> **The taxonomies below are aspirational, and now measured as such.** They
> hold **166 nodes**; `with_core_concepts()` populates **28** concepts, of
> which one is a test fixture; and **12** of the 166 correspond to something
> the crate ships (Adam, BatchNormalization, Dropout, FirstOrderLogic, GELU,
> NeurosymbolicAI, ReLU, Regularization, SymbolicAI, Transformer, Unification,
> WeightDecay). The other 154 are a survey of the field, which is a reasonable
> thing for this document to contain and a misleading thing for it to present
> as the contents of a crate. Generating this section from
> `populate_core_concepts` would make the two agree; that is not done.

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

`ConceptRelation` has **eleven** variants (`knowledge/ai_concepts.rs`):

### Hierarchy

- `IsA`: Subsumption (e.g., LSTM is-a RNN)
- `PartOf`: Composition (e.g., MultiHeadAttention part-of Transformer)
- `InstanceOf`: Membership, as distinct from subsumption
- `Generalizes`: The inverse direction of `IsA`

### Functional

- `Requires`: Dependency (e.g., Backpropagation requires a differentiable function)
- `Enables`: The inverse of `Requires`
- `Uses`: Employs without depending on
- `Computes`: Produces a quantity
- `Optimizes`: Improves an objective

### Comparative

- `AlternativeTo`: Functionally interchangeable (e.g., ReLU / GELU)
- `Improves`: Enhancement (e.g., residual connections improve gradient flow)

> **Corrected 2026-08-18.** Six of the eleven relations listed here did not
> exist — `related_to`, `extends`, `used_for`, `builds_on`, `introduced_by`,
> `superseded_by` — and six real ones were missing: `Enables`, `Generalizes`,
> `InstanceOf`, `Uses`, `Computes`, `Optimizes`. The "Historical" group in
> particular described an attribution model (`introduced_by`, `superseded_by`)
> that the concept graph does not have; lineage lives in `AIHistoryKB`
> instead. Variants are Rust identifiers, not the snake_case shown before.

---

## Concept Properties

Each concept has:

| Property            | Type                          | Description                          |
| ------------------- | ----------------------------- | ------------------------------------ |
| id                  | Uuid                          | Identity — **lookups key on this**   |
| name                | String                        | Human-readable name                  |
| domain              | ConceptDomain                 | Primary domain (ten of them, below)  |
| definition          | String                        | Natural language definition          |
| math                | Option\<String\>              | LaTeX mathematical definition        |
| complexity          | Option\<ComplexitySpec\>      | Structured complexity, not a string  |
| properties          | HashMap\<String, PropertyValue\> | Open attribute bag                |
| applicable_tasks    | Vec\<String\>                 | Where the concept applies            |
| contraindications   | Vec\<String\>                 | Where it does not                    |

> **Corrected 2026-08-18.** `name` is not the unique identifier — `id: Uuid`
> is, which is why every lookup in the API takes one. `description` is
> `definition`, `math_notation` is `math`, `complexity` is a structured
> `ComplexitySpec` rather than a string, and `implementation_hints` does not
> exist; `properties`, `applicable_tasks` and `contraindications` were
> missing.

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
