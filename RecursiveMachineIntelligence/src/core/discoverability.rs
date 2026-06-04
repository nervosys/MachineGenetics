//! Framework Discoverability — Unified Ontology for AI Agents
//!
//! Provides a single entry point ([`crate::core::discoverability::FrameworkCatalog`]) for an AI agent to
//! discover, reason about, and compose every capability in the RMI framework.
//!
//! This module solves three critical discoverability problems:
//!
//! 1. **Fragmentation** — The framework has separate ontology systems
//!    (`Ontology`, `AIConceptsOntology`, `PrimitiveRegistry`, `AIHistoryKB`)
//!    that are not cross-linked. `FrameworkCatalog` merges them into one
//!    queryable surface.
//!
//! 2. **Incomplete coverage** — The introspection ontology was missing the
//!    `evolution`, `runtime`, `distributed`, `codegen`, `collaboration`, and
//!    `training` modules. [`crate::core::discoverability::extend_ontology`] fills those gaps and adds API
//!    signatures (constructor, forward signature, shape metadata) to every
//!    composable component.
//!
//! 3. **No composition templates** — An agent knowing that "Linear → GELU →
//!    Linear" is valid still lacks *recipes* for common architectures.
//!    [`crate::core::discoverability::ArchitectureRecipe`] provides queryable templates for Transformer
//!    blocks, MLPs, ResNets, and more.
//!
//! # Example
//!
//! ```
//! use rmi::core::discoverability::{FrameworkCatalog, ArchitectureRecipe};
//!
//! let catalog = FrameworkCatalog::build();
//!
//! // Unified search across framework + domain knowledge
//! let hits = catalog.search("attention");
//! assert!(!hits.is_empty());
//!
//! // Find components for a task
//! let for_nlp = catalog.for_task("text_classification");
//! assert!(!for_nlp.is_empty());
//!
//! // Get architecture recipes
//! let recipes = catalog.recipes_for_task("sequence_modeling");
//! assert!(!recipes.is_empty());
//!
//! // List all available capabilities
//! let caps = catalog.capabilities();
//! assert!(caps.len() > 10);
//! ```

use crate::core::introspection::{
    FrameworkOntology, IntrospectionQueries, NS_COMPUTE, NS_NEURAL, NS_ROOT, NS_SYMBOLIC,
};
use crate::core::ontology::{
    AttributeValue, Concept, ConceptId, ConceptType, Ontology, OntologyQuery, Relation,
    RelationType,
};
use crate::knowledge::ai_concepts::AIConceptsOntology;
use crate::knowledge::history::AIHistoryKB;

// ── Namespace constants for new modules ──────────────────────────────────────

/// Evolution sub-namespace.
pub const NS_EVOLUTION: &str = "rmi.evolution";
/// Runtime sub-namespace.
pub const NS_RUNTIME: &str = "rmi.runtime";
/// Distributed sub-namespace.
pub const NS_DISTRIBUTED: &str = "rmi.distributed";
/// Codegen sub-namespace.
pub const NS_CODEGEN: &str = "rmi.codegen";
/// Collaboration sub-namespace.
pub const NS_COLLABORATION: &str = "rmi.collaboration";
/// Training sub-namespace.
pub const NS_TRAINING: &str = "rmi.training";

// ============================================================================
// Ontology Extension — fills in every missing module
// ============================================================================

/// Extend a [`FrameworkOntology`]-built ontology with the modules that were
/// previously missing: codegen, collaboration, training, evolution, runtime,
/// distributed, plus API signatures on all existing neural concepts.
pub fn extend_ontology(ont: &Ontology) {
    add_codegen_concepts(ont);
    add_training_concepts(ont);
    add_collaboration_concepts(ont);
    add_evolution_concepts(ont);
    add_runtime_concepts(ont);
    add_distributed_concepts(ont);
    add_loss_functions(ont);
    add_optimizers(ont);
    add_api_signatures(ont);
    add_extra_compute_backends(ont);
    add_extra_composition_rules(ont);
}

// ── Helper re-use from FrameworkOntology pattern ─────────────────────────────

fn s(v: &str) -> AttributeValue {
    AttributeValue::String(v.to_string())
}
fn b(v: bool) -> AttributeValue {
    AttributeValue::Bool(v)
}
fn _i(v: i64) -> AttributeValue {
    AttributeValue::Int(v)
}
fn list(v: &[&str]) -> AttributeValue {
    AttributeValue::List(
        v.iter()
            .map(|x| AttributeValue::String(x.to_string()))
            .collect(),
    )
}

fn concept(
    ns: &str,
    name: &str,
    ctype: ConceptType,
    label: &str,
    attrs: Vec<(&str, AttributeValue)>,
) -> Concept {
    let id = ConceptId::new(ns, name);
    let mut c = Concept::new(id, ctype).with_label(label);
    for (k, v) in attrs {
        c = c.with_attribute(k, v);
    }
    c
}

fn relate(ont: &Ontology, src_ns: &str, src: &str, rel: RelationType, tgt_ns: &str, tgt: &str) {
    let mut r = Relation::new(
        ConceptId::new(src_ns, src),
        rel,
        ConceptId::new(tgt_ns, tgt),
    );
    r.bidirectional = matches!(
        rel,
        RelationType::PartOf | RelationType::IsA | RelationType::HasComponent
    );
    ont.add_relation(r);
}

// ── Code Generation / IR ─────────────────────────────────────────────────────

fn add_codegen_concepts(ont: &Ontology) {
    // Module top-level
    ont.add_concept(concept(
        NS_ROOT,
        "module.codegen",
        ConceptType::Schema,
        "CodeGen Module",
        vec![(
            "description",
            s("Low-level IR, program synthesis, code emission, optimization, and verification"),
        )],
    ));
    relate(
        ont,
        NS_ROOT,
        "module.codegen",
        RelationType::PartOf,
        NS_ROOT,
        "framework",
    );

    // IR Builder
    ont.add_concept(concept(
        NS_CODEGEN,
        "ir_builder",
        ConceptType::Entity,
        "IR Function Builder",
        vec![
            ("rust_type", s("FunctionBuilder")),
            (
                "constructor",
                s("FunctionBuilder::new(name, params, return_type)"),
            ),
            (
                "description",
                s("Typed intermediate representation builder for machine code generation"),
            ),
            (
                "operations",
                list(&[
                    "matmul",
                    "activation",
                    "binary_op",
                    "unary_op",
                    "reduce",
                    "reshape",
                    "transpose",
                    "normalize",
                    "conv2d",
                    "pool",
                    "ret",
                ]),
            ),
        ],
    ));
    relate(
        ont,
        NS_CODEGEN,
        "ir_builder",
        RelationType::PartOf,
        NS_ROOT,
        "module.codegen",
    );

    // Program
    ont.add_concept(concept(
        NS_CODEGEN,
        "program",
        ConceptType::Entity,
        "IR Program",
        vec![
            ("rust_type", s("Program")),
            ("constructor", s("Program::new(name)")),
            (
                "description",
                s("Container for multiple IR functions with metadata and structural hashing"),
            ),
        ],
    ));
    relate(
        ont,
        NS_CODEGEN,
        "program",
        RelationType::PartOf,
        NS_ROOT,
        "module.codegen",
    );

    // Mutation operators
    ont.add_concept(concept(
        NS_CODEGEN,
        "mutator",
        ConceptType::Process,
        "Mutation Operators",
        vec![
            ("rust_type", s("Mutator")),
            ("constructor", s("Mutator::new(seed)")),
            (
                "mutations",
                list(&[
                    "InsertNode",
                    "DeleteNode",
                    "ReplaceNode",
                    "SwapNodes",
                    "RewireEdge",
                    "ChangeActivation",
                    "ChangeBinaryOp",
                    "ChangeUnaryOp",
                ]),
            ),
            (
                "description",
                s("Evolutionary mutation operators for program synthesis"),
            ),
        ],
    ));
    relate(
        ont,
        NS_CODEGEN,
        "mutator",
        RelationType::PartOf,
        NS_ROOT,
        "module.codegen",
    );

    // Crossover
    ont.add_concept(concept(
        NS_CODEGEN,
        "crossover",
        ConceptType::Process,
        "Crossover Operators",
        vec![
            ("rust_type", s("Crossover")),
            ("strategies", list(&["SinglePoint", "Uniform"])),
            (
                "description",
                s("Genetic crossover for combining two IR programs"),
            ),
        ],
    ));
    relate(
        ont,
        NS_CODEGEN,
        "crossover",
        RelationType::PartOf,
        NS_ROOT,
        "module.codegen",
    );

    // Code emitters
    for (name, label, target) in [
        ("rust_emitter", "Rust Emitter", "Rust"),
        ("cuda_emitter", "CUDA Emitter", "CUDA/PTX"),
        ("mlir_emitter", "MLIR Emitter", "MLIR dialect"),
        ("onnx_emitter", "ONNX Emitter", "ONNX graph"),
    ] {
        ont.add_concept(concept(
            NS_CODEGEN,
            name,
            ConceptType::Entity,
            label,
            vec![
                ("target", s(target)),
                ("trait", s("CodeEmitter")),
                ("api", s("fn emit(&self, &Program) -> Result<String>")),
            ],
        ));
        relate(
            ont,
            NS_CODEGEN,
            name,
            RelationType::PartOf,
            NS_ROOT,
            "module.codegen",
        );
    }

    // Optimization passes
    for (name, label, desc) in [
        (
            "dce_pass",
            "Dead Code Elimination",
            "Removes unreachable IR nodes",
        ),
        (
            "const_fold_pass",
            "Constant Folding",
            "Evaluates constant expressions at compile time",
        ),
        (
            "cse_pass",
            "Common Subexpression Elimination",
            "Deduplicates identical computations",
        ),
        (
            "fusion_pass",
            "Operator Fusion",
            "Fuses compatible operators to reduce memory traffic",
        ),
        (
            "strength_reduction_pass",
            "Strength Reduction",
            "Replaces expensive ops with cheaper equivalents",
        ),
        (
            "algebraic_simp_pass",
            "Algebraic Simplification",
            "Applies algebraic identities (x*1=x, x+0=x)",
        ),
    ] {
        ont.add_concept(concept(
            NS_CODEGEN,
            name,
            ConceptType::Process,
            label,
            vec![("description", s(desc)), ("trait", s("OptimizationPass"))],
        ));
        relate(
            ont,
            NS_CODEGEN,
            name,
            RelationType::PartOf,
            NS_ROOT,
            "module.codegen",
        );
    }

    // Verification
    ont.add_concept(concept(
        NS_CODEGEN,
        "verifier",
        ConceptType::Entity,
        "IR Verifier",
        vec![
            ("rust_type", s("Verifier")),
            ("description", s("Static analysis: type checking, shape inference, cycle detection, dead code, unused params, complexity analysis")),
            ("analyses", list(&[
                "type_check", "shape_inference", "cycle_detection",
                "dead_code", "unused_params", "complexity",
            ])),
        ],
    ));
    relate(
        ont,
        NS_CODEGEN,
        "verifier",
        RelationType::PartOf,
        NS_ROOT,
        "module.codegen",
    );
}

// ── Training & Federated Learning ────────────────────────────────────────────

fn add_training_concepts(ont: &Ontology) {
    // Module
    ont.add_concept(concept(
        NS_ROOT,
        "module.training",
        ConceptType::Schema,
        "Training Module",
        vec![(
            "description",
            s("Training loops, data loading, federated learning"),
        )],
    ));
    relate(
        ont,
        NS_ROOT,
        "module.training",
        RelationType::PartOf,
        NS_ROOT,
        "module.neural",
    );

    // Trainer
    ont.add_concept(concept(
        NS_TRAINING,
        "trainer",
        ConceptType::Entity,
        "Trainer",
        vec![
            ("rust_type", s("Trainer")),
            (
                "constructor",
                s("Trainer::new(layers, loss_fn, optimizer, config)"),
            ),
            (
                "api_fit",
                s("fn fit(&mut self, &Dataset) -> TrainingHistory"),
            ),
            ("api_evaluate", s("fn evaluate(&self, &Dataset) -> f32")),
            (
                "features",
                list(&[
                    "gradient_clipping",
                    "validation_split",
                    "lr_scheduling",
                    "epoch-based_training",
                    "batch_iteration",
                ]),
            ),
        ],
    ));
    relate(
        ont,
        NS_TRAINING,
        "trainer",
        RelationType::PartOf,
        NS_ROOT,
        "module.training",
    );

    // Dataset & DataLoader
    ont.add_concept(concept(
        NS_TRAINING,
        "dataset",
        ConceptType::Entity,
        "Dataset",
        vec![
            ("rust_type", s("Dataset")),
            (
                "constructor",
                s("Dataset::new(inputs: Vec<Vec<f32>>, targets: Vec<Vec<f32>>)"),
            ),
            (
                "api",
                s("fn len(), fn input_dim(), fn target_dim(), fn get(idx)"),
            ),
        ],
    ));
    relate(
        ont,
        NS_TRAINING,
        "dataset",
        RelationType::PartOf,
        NS_ROOT,
        "module.training",
    );

    ont.add_concept(concept(
        NS_TRAINING,
        "dataloader",
        ConceptType::Entity,
        "DataLoader",
        vec![
            ("rust_type", s("DataLoader")),
            (
                "constructor",
                s("DataLoader::new(&Dataset, batch_size, shuffle)"),
            ),
            ("description", s("Batched iterator with optional shuffling")),
        ],
    ));
    relate(
        ont,
        NS_TRAINING,
        "dataloader",
        RelationType::PartOf,
        NS_ROOT,
        "module.training",
    );

    // Federated Trainer
    ont.add_concept(concept(
        NS_TRAINING,
        "federated_trainer",
        ConceptType::Entity,
        "FederatedTrainer",
        vec![
            ("rust_type", s("FederatedTrainer")),
            (
                "constructor",
                s("FederatedTrainer::new(trainers, datasets, config)"),
            ),
            ("api", s("fn run(&mut self) -> FederatedHistory")),
            ("strategies", list(&["FedAvg", "FedProx", "TrimmedMean"])),
            ("multi_agent", b(true)),
            (
                "description",
                s("Federated learning across multiple AI agents with configurable aggregation"),
            ),
        ],
    ));
    relate(
        ont,
        NS_TRAINING,
        "federated_trainer",
        RelationType::PartOf,
        NS_ROOT,
        "module.training",
    );
    relate(
        ont,
        NS_TRAINING,
        "federated_trainer",
        RelationType::Enables,
        NS_COLLABORATION,
        "agent_runtime",
    );

    // Model serialization
    ont.add_concept(concept(
        NS_TRAINING,
        "model_serialization",
        ConceptType::Process,
        "Model Serialization",
        vec![
            ("rust_module", s("neural::serialization")),
            ("formats", list(&["binary (MessagePack + LZ4)", "JSON"])),
            (
                "api_save",
                s("fn save_model(path, &[Box<dyn Layer>], &TrainingHistory) -> Result<()>"),
            ),
            ("api_load", s("fn load_model(path) -> Result<SavedModel>")),
        ],
    ));
    relate(
        ont,
        NS_TRAINING,
        "model_serialization",
        RelationType::PartOf,
        NS_ROOT,
        "module.training",
    );
}

// ── Multi-Agent Collaboration ────────────────────────────────────────────────

fn add_collaboration_concepts(ont: &Ontology) {
    // Module
    ont.add_concept(concept(
        NS_ROOT,
        "module.collaboration",
        ConceptType::Schema,
        "Collaboration Module",
        vec![(
            "description",
            s("Multi-agent collaboration runtime, workspace, delegation, pipelines"),
        )],
    ));
    relate(
        ont,
        NS_ROOT,
        "module.collaboration",
        RelationType::PartOf,
        NS_ROOT,
        "framework",
    );

    // AgentRuntime
    ont.add_concept(concept(
        NS_COLLABORATION,
        "agent_runtime",
        ConceptType::Entity,
        "AgentRuntime",
        vec![
            ("rust_type", s("AgentRuntime")),
            (
                "constructor",
                s("AgentRuntime::new(RuntimeConfig::default())"),
            ),
            (
                "api_spawn",
                s("async fn spawn(&self, Agent) -> Result<Uuid>"),
            ),
            ("api_stats", s("fn stats(&self) -> RuntimeStats")),
            (
                "description",
                s("Central hub wiring agents to MessageBus, ServiceRegistry, SharedWorkspace"),
            ),
            ("multi_agent", b(true)),
        ],
    ));
    relate(
        ont,
        NS_COLLABORATION,
        "agent_runtime",
        RelationType::PartOf,
        NS_ROOT,
        "module.collaboration",
    );

    // SharedWorkspace
    ont.add_concept(concept(
        NS_COLLABORATION,
        "shared_workspace",
        ConceptType::Entity,
        "SharedWorkspace",
        vec![
            ("rust_type", s("SharedWorkspace")),
            ("constructor", s("SharedWorkspace::new()")),
            ("api", s("put/get/delete/watch/put_tensor/get_tensor")),
            ("pattern", s("Blackboard pattern with versioned entries")),
            ("multi_agent", b(true)),
        ],
    ));
    relate(
        ont,
        NS_COLLABORATION,
        "shared_workspace",
        RelationType::PartOf,
        NS_ROOT,
        "module.collaboration",
    );

    // TaskDelegator
    ont.add_concept(concept(
        NS_COLLABORATION,
        "task_delegator",
        ConceptType::Entity,
        "TaskDelegator",
        vec![
            ("rust_type", s("TaskDelegator")),
            ("description", s("Capability-based task routing — finds least-loaded agent matching a capability")),
            ("api", s("async fn delegate(capability, payload) -> Result<Uuid>")),
            ("multi_agent", b(true)),
        ],
    ));
    relate(
        ont,
        NS_COLLABORATION,
        "task_delegator",
        RelationType::PartOf,
        NS_ROOT,
        "module.collaboration",
    );

    // AgentPipeline
    ont.add_concept(concept(
        NS_COLLABORATION,
        "agent_pipeline",
        ConceptType::Entity,
        "AgentPipeline",
        vec![
            ("rust_type", s("AgentPipeline")),
            ("constructor", s("AgentPipeline::new(name).then(stage).then(stage)")),
            ("api_validate", s("fn validate() -> Result<()>")),
            ("description", s("Composable multi-stage processing pipeline with input/output key chain validation")),
            ("multi_agent", b(true)),
        ],
    ));
    relate(
        ont,
        NS_COLLABORATION,
        "agent_pipeline",
        RelationType::PartOf,
        NS_ROOT,
        "module.collaboration",
    );

    // ModelRegistry
    ont.add_concept(concept(
        NS_COLLABORATION,
        "model_registry",
        ConceptType::Entity,
        "ModelRegistry",
        vec![
            ("rust_type", s("ModelRegistry")),
            ("constructor", s("ModelRegistry::new()")),
            (
                "api",
                s("register/get_latest/get_version/find_by_metric/find_by_tag"),
            ),
            (
                "description",
                s("Versioned model store for multi-agent model sharing"),
            ),
            ("multi_agent", b(true)),
        ],
    ));
    relate(
        ont,
        NS_COLLABORATION,
        "model_registry",
        RelationType::PartOf,
        NS_ROOT,
        "module.collaboration",
    );

    // MessageBus
    ont.add_concept(concept(
        NS_COLLABORATION,
        "message_bus",
        ConceptType::Entity,
        "MessageBus",
        vec![
            ("rust_type", s("MessageBus")),
            ("constructor", s("MessageBus::new()")),
            ("api", s("publish/subscribe/request/reply")),
            (
                "patterns",
                list(&["pub/sub", "request/reply", "dead_letter_queue"]),
            ),
            ("multi_agent", b(true)),
        ],
    ));
    relate(
        ont,
        NS_COLLABORATION,
        "message_bus",
        RelationType::PartOf,
        NS_ROOT,
        "module.collaboration",
    );

    // Swarm coordinator
    ont.add_concept(concept(
        NS_COLLABORATION,
        "swarm_coordinator",
        ConceptType::Entity,
        "SwarmCoordinator",
        vec![
            ("rust_type", s("SwarmCoordinator")),
            ("description", s("Multi-agent swarm coordination with voting, proposals, and collaborative workflows")),
            ("strategies", list(&["RoundRobin", "Hierarchical", "Consensus", "Competitive", "Stigmergic"])),
            ("multi_agent", b(true)),
        ],
    ));
    relate(
        ont,
        NS_COLLABORATION,
        "swarm_coordinator",
        RelationType::PartOf,
        NS_ROOT,
        "module.collaboration",
    );
}

// ── Evolution / Self-Improvement ─────────────────────────────────────────────

fn add_evolution_concepts(ont: &Ontology) {
    ont.add_concept(concept(
        NS_ROOT,
        "module.evolution",
        ConceptType::Schema,
        "Evolution Module",
        vec![(
            "description",
            s("Meta-learning, evolutionary search, self-modification"),
        )],
    ));
    relate(
        ont,
        NS_ROOT,
        "module.evolution",
        RelationType::PartOf,
        NS_ROOT,
        "framework",
    );

    // Meta-learner
    ont.add_concept(concept(
        NS_EVOLUTION,
        "meta_learner",
        ConceptType::Entity,
        "MetaLearner",
        vec![
            ("rust_type", s("MetaLearner")),
            (
                "description",
                s("Architecture search and hyperparameter optimization"),
            ),
            (
                "strategies",
                list(&[
                    "ArchitectureSearch",
                    "HyperparameterOptimization",
                    "NeuralArchitectureSearch",
                ]),
            ),
        ],
    ));
    relate(
        ont,
        NS_EVOLUTION,
        "meta_learner",
        RelationType::PartOf,
        NS_ROOT,
        "module.evolution",
    );

    // Population engine
    ont.add_concept(concept(
        NS_EVOLUTION,
        "population",
        ConceptType::Entity,
        "Population Engine",
        vec![
            ("rust_type", s("Population")),
            (
                "description",
                s("Multi-objective evolutionary optimization with Pareto fronts"),
            ),
            ("selection", list(&["Tournament", "Roulette", "NSGA-II"])),
        ],
    ));
    relate(
        ont,
        NS_EVOLUTION,
        "population",
        RelationType::PartOf,
        NS_ROOT,
        "module.evolution",
    );

    // Self-modification
    ont.add_concept(concept(
        NS_EVOLUTION,
        "self_modification",
        ConceptType::Process,
        "Self-Modification Engine",
        vec![
            ("rust_module", s("evolution::self_modification")),
            (
                "description",
                s("Sandboxed code patching with rollback support"),
            ),
            (
                "safety",
                list(&[
                    "sandboxed_execution",
                    "automatic_rollback",
                    "diff_generation",
                ]),
            ),
        ],
    ));
    relate(
        ont,
        NS_EVOLUTION,
        "self_modification",
        RelationType::PartOf,
        NS_ROOT,
        "module.evolution",
    );
}

// ── Runtime ──────────────────────────────────────────────────────────────────

fn add_runtime_concepts(ont: &Ontology) {
    ont.add_concept(concept(
        NS_ROOT,
        "module.runtime",
        ConceptType::Schema,
        "Runtime Module",
        vec![(
            "description",
            s("Production deployment, memory management, observability"),
        )],
    ));
    relate(
        ont,
        NS_ROOT,
        "module.runtime",
        RelationType::PartOf,
        NS_ROOT,
        "framework",
    );

    ont.add_concept(concept(
        NS_RUNTIME,
        "memory_pool",
        ConceptType::Entity,
        "Memory Pool",
        vec![
            ("rust_type", s("MemoryPool")),
            (
                "description",
                s("Arena-based allocation with zero-copy tensor buffers and slab allocator"),
            ),
            (
                "features",
                list(&["arena_allocation", "zero_copy_tensors", "slab_allocator"]),
            ),
        ],
    ));
    relate(
        ont,
        NS_RUNTIME,
        "memory_pool",
        RelationType::PartOf,
        NS_ROOT,
        "module.runtime",
    );

    ont.add_concept(concept(
        NS_RUNTIME,
        "observability",
        ConceptType::Entity,
        "Observability Stack",
        vec![
            ("rust_module", s("runtime::observability")),
            (
                "description",
                s("Metrics, distributed tracing, structured logging"),
            ),
            (
                "features",
                list(&[
                    "metrics_counter",
                    "histograms",
                    "span_tracing",
                    "log_levels",
                ]),
            ),
        ],
    ));
    relate(
        ont,
        NS_RUNTIME,
        "observability",
        RelationType::PartOf,
        NS_ROOT,
        "module.runtime",
    );

    ont.add_concept(concept(
        NS_RUNTIME,
        "deployment",
        ConceptType::Entity,
        "Deployment Engine",
        vec![
            ("rust_type", s("DeploymentSpec")),
            (
                "description",
                s("Declarative deployment specs with YAML/Docker Compose rendering"),
            ),
            ("outputs", list(&["YAML", "Docker Compose", "Kubernetes"])),
        ],
    ));
    relate(
        ont,
        NS_RUNTIME,
        "deployment",
        RelationType::PartOf,
        NS_ROOT,
        "module.runtime",
    );
}

// ── Distributed ──────────────────────────────────────────────────────────────

fn add_distributed_concepts(ont: &Ontology) {
    ont.add_concept(concept(
        NS_ROOT,
        "module.distributed",
        ConceptType::Schema,
        "Distributed Module",
        vec![(
            "description",
            s("Network transport, service discovery, consensus, federation"),
        )],
    ));
    relate(
        ont,
        NS_ROOT,
        "module.distributed",
        RelationType::PartOf,
        NS_ROOT,
        "framework",
    );

    ont.add_concept(concept(
        NS_DISTRIBUTED,
        "transport",
        ConceptType::Entity,
        "Transport Layer",
        vec![
            ("rust_module", s("distributed::transport")),
            ("protocols", list(&["TCP", "QUIC", "InProcess"])),
            (
                "features",
                list(&["connection_pooling", "load_balancing", "keepalive"]),
            ),
        ],
    ));
    relate(
        ont,
        NS_DISTRIBUTED,
        "transport",
        RelationType::PartOf,
        NS_ROOT,
        "module.distributed",
    );

    ont.add_concept(concept(
        NS_DISTRIBUTED,
        "service_registry",
        ConceptType::Entity,
        "Service Registry",
        vec![
            ("rust_type", s("ServiceRegistry")),
            (
                "constructor",
                s("ServiceRegistry::new(DiscoveryConfig::default())"),
            ),
            (
                "description",
                s("Capability-indexed service discovery with health monitoring"),
            ),
            (
                "discovery_methods",
                list(&["Static", "Registry", "Gossip", "Multicast"]),
            ),
        ],
    ));
    relate(
        ont,
        NS_DISTRIBUTED,
        "service_registry",
        RelationType::PartOf,
        NS_ROOT,
        "module.distributed",
    );

    ont.add_concept(concept(
        NS_DISTRIBUTED,
        "consensus",
        ConceptType::Process,
        "Consensus Protocol",
        vec![
            ("rust_module", s("distributed::consensus")),
            (
                "algorithms",
                list(&["Raft", "BFT (Byzantine Fault Tolerance)"]),
            ),
            ("description", s("Multi-agent agreement mechanisms")),
        ],
    ));
    relate(
        ont,
        NS_DISTRIBUTED,
        "consensus",
        RelationType::PartOf,
        NS_ROOT,
        "module.distributed",
    );

    ont.add_concept(concept(
        NS_DISTRIBUTED,
        "federation",
        ConceptType::Process,
        "Federation",
        vec![
            ("rust_module", s("distributed::federation")),
            (
                "description",
                s("Cross-cluster federation for multi-site agent coordination"),
            ),
        ],
    ));
    relate(
        ont,
        NS_DISTRIBUTED,
        "federation",
        RelationType::PartOf,
        NS_ROOT,
        "module.distributed",
    );
}

// ── Loss Functions ───────────────────────────────────────────────────────────

fn add_loss_functions(ont: &Ontology) {
    ont.add_concept(concept(
        NS_NEURAL,
        "loss",
        ConceptType::Schema,
        "Loss Function",
        vec![
            ("kind", s("abstract")),
            ("trait", s("Loss")),
            (
                "api",
                s("fn forward(&[f32], &[f32]) -> Vec<f32> + fn backward(...) -> Vec<f32>"),
            ),
            ("differentiable", b(true)),
        ],
    ));
    relate(
        ont,
        NS_NEURAL,
        "loss",
        RelationType::PartOf,
        NS_ROOT,
        "module.neural",
    );

    for (name, label, task, formula) in [
        ("mse_loss", "MSE Loss", "regression", "mean((y - ŷ)²)"),
        (
            "cross_entropy_loss",
            "Cross-Entropy Loss",
            "multi-class classification",
            "-Σ y·log(ŷ)",
        ),
        (
            "bce_loss",
            "BCE Loss",
            "binary classification",
            "-[y·log(ŷ) + (1-y)·log(1-ŷ)]",
        ),
        (
            "bce_logits_loss",
            "BCE With Logits",
            "binary classification",
            "BCE + built-in sigmoid",
        ),
        ("nll_loss", "NLL Loss", "classification", "-Σ log(p(y))"),
        (
            "l1_loss",
            "L1 / MAE Loss",
            "regression (robust)",
            "mean(|y - ŷ|)",
        ),
        (
            "smooth_l1_loss",
            "Smooth L1 (Huber)",
            "regression (robust)",
            "piecewise L1+L2",
        ),
        (
            "kl_div_loss",
            "KL Divergence",
            "distribution matching",
            "Σ p·log(p/q)",
        ),
    ] {
        ont.add_concept(concept(
            NS_NEURAL,
            name,
            ConceptType::Entity,
            label,
            vec![
                ("rust_type", s(&label.replace([' ', '/'], ""))),
                ("recommended_task", s(task)),
                ("formula", s(formula)),
                ("differentiable", b(true)),
            ],
        ));
        relate(ont, NS_NEURAL, name, RelationType::IsA, NS_NEURAL, "loss");
    }
}

// ── Optimizers ───────────────────────────────────────────────────────────────

fn add_optimizers(ont: &Ontology) {
    ont.add_concept(concept(
        NS_NEURAL,
        "optimizer",
        ConceptType::Schema,
        "Optimizer",
        vec![
            ("kind", s("abstract")),
            ("trait", s("Optimizer")),
            (
                "api",
                s("fn step(&mut self, params, grads) + fn get_lr() + fn set_lr()"),
            ),
        ],
    ));
    relate(
        ont,
        NS_NEURAL,
        "optimizer",
        RelationType::PartOf,
        NS_ROOT,
        "module.neural",
    );

    for (name, label, adaptive, hyperparams) in [
        ("sgd", "SGD", false, "lr, momentum"),
        ("adam", "Adam", true, "lr, beta1, beta2, epsilon"),
        (
            "adamw",
            "AdamW",
            true,
            "lr, beta1, beta2, epsilon, weight_decay",
        ),
        ("rmsprop", "RMSprop", true, "lr, alpha, epsilon"),
    ] {
        ont.add_concept(concept(
            NS_NEURAL,
            name,
            ConceptType::Entity,
            label,
            vec![
                ("rust_type", s(label)),
                ("adaptive", b(adaptive)),
                ("hyperparameters", s(hyperparams)),
                ("constructor", s(&format!("{}::new(lr)", label))),
            ],
        ));
        relate(
            ont,
            NS_NEURAL,
            name,
            RelationType::IsA,
            NS_NEURAL,
            "optimizer",
        );
    }

    // LR schedulers
    ont.add_concept(concept(
        NS_NEURAL,
        "lr_scheduler",
        ConceptType::Schema,
        "Learning Rate Scheduler",
        vec![
            ("kind", s("abstract")),
            ("trait", s("LRScheduler")),
            ("implementations", list(&["StepLR", "CosineAnnealingLR"])),
        ],
    ));
    relate(
        ont,
        NS_NEURAL,
        "lr_scheduler",
        RelationType::PartOf,
        NS_ROOT,
        "module.neural",
    );
}

// ── API Signatures on existing neural components ─────────────────────────────

fn add_api_signatures(ont: &Ontology) {
    // Update existing concepts with richer metadata via new linked concepts.
    // (We can't mutate existing concepts through the Ontology API, so we add
    //  companion "api_spec" concepts linked via ParameterOf.)

    let api_specs: Vec<(&str, &str, &str, &str, &str, &str)> = vec![
        // (parent_ns, parent_name, constructor, forward_sig, input_shape, output_shape)
        (
            NS_NEURAL,
            "linear",
            "Linear::new(in_features: usize, out_features: usize)",
            "fn forward(&[&Variable], &mut GradientTape) -> Variable",
            "[batch, *, in_features]",
            "[batch, *, out_features]",
        ),
        (
            NS_NEURAL,
            "conv2d",
            "Conv2d::new(in_channels, out_channels, kernel_size)",
            "fn forward(&[&Variable], &mut GradientTape) -> Variable",
            "[batch, in_channels, height, width]",
            "[batch, out_channels, height', width']",
        ),
        (
            NS_NEURAL,
            "embedding",
            "Embedding::new(vocab_size: usize, embedding_dim: usize)",
            "fn forward(&[&Variable], &mut GradientTape) -> Variable",
            "[batch, seq_len] (integer indices)",
            "[batch, seq_len, embedding_dim]",
        ),
        (
            NS_NEURAL,
            "multi_head_attention",
            "MultiHeadAttention::new(embed_dim, num_heads)",
            "fn forward(&[&Variable], &mut GradientTape) -> Variable",
            "[batch, seq_len, embed_dim]",
            "[batch, seq_len, embed_dim]",
        ),
        (
            NS_NEURAL,
            "layer_norm",
            "LayerNorm::new(normalized_shape: usize)",
            "fn forward(&[&Variable], &mut GradientTape) -> Variable",
            "[batch, *, normalized_shape]",
            "[batch, *, normalized_shape]",
        ),
        (
            NS_NEURAL,
            "batch_norm",
            "BatchNorm::new(num_features: usize)",
            "fn forward(&[&Variable], &mut GradientTape) -> Variable",
            "[batch, num_features, *]",
            "[batch, num_features, *]",
        ),
    ];

    for (ns, parent, constructor, forward, in_shape, out_shape) in api_specs {
        let spec_name = format!("{}_api", parent);
        ont.add_concept(concept(
            ns,
            &spec_name,
            ConceptType::Property,
            &format!("{} API Spec", parent),
            vec![
                ("constructor", s(constructor)),
                ("forward_signature", s(forward)),
                ("input_shape", s(in_shape)),
                ("output_shape", s(out_shape)),
            ],
        ));
        relate(ont, ns, &spec_name, RelationType::ParameterOf, ns, parent);
    }
}

// ── Extra Compute Backends ───────────────────────────────────────────────────

fn add_extra_compute_backends(ont: &Ontology) {
    for (name, label, tech, feature_flag) in [
        (
            "webgpu_backend",
            "WebGPU Backend",
            "wgpu compute shaders",
            "gpu",
        ),
        ("metal_backend", "Metal Backend", "Apple Metal MSL", "gpu"),
        ("vulkan_backend", "Vulkan Backend", "Vulkan SPIR-V", "gpu"),
        ("wasm_backend", "WASM Backend", "WebAssembly SIMD", "wasm"),
    ] {
        ont.add_concept(concept(
            NS_COMPUTE,
            name,
            ConceptType::Entity,
            label,
            vec![
                ("technology", s(tech)),
                ("feature_gated", b(true)),
                ("feature_flag", s(feature_flag)),
            ],
        ));
        relate(
            ont,
            NS_COMPUTE,
            name,
            RelationType::IsA,
            NS_COMPUTE,
            "backend",
        );
    }
}

// ── Extra Composition Rules ──────────────────────────────────────────────────

fn add_extra_composition_rules(ont: &Ontology) {
    // Training compositions
    relate(
        ont,
        NS_TRAINING,
        "trainer",
        RelationType::InputOf,
        NS_TRAINING,
        "federated_trainer",
    );
    relate(
        ont,
        NS_TRAINING,
        "dataset",
        RelationType::InputOf,
        NS_TRAINING,
        "trainer",
    );
    relate(
        ont,
        NS_TRAINING,
        "model_serialization",
        RelationType::Follows,
        NS_TRAINING,
        "trainer",
    );

    // Collaboration compositions
    relate(
        ont,
        NS_COLLABORATION,
        "shared_workspace",
        RelationType::PartOf,
        NS_COLLABORATION,
        "agent_runtime",
    );
    relate(
        ont,
        NS_COLLABORATION,
        "task_delegator",
        RelationType::PartOf,
        NS_COLLABORATION,
        "agent_runtime",
    );
    relate(
        ont,
        NS_COLLABORATION,
        "message_bus",
        RelationType::PartOf,
        NS_COLLABORATION,
        "agent_runtime",
    );
    relate(
        ont,
        NS_COLLABORATION,
        "model_registry",
        RelationType::PartOf,
        NS_COLLABORATION,
        "agent_runtime",
    );

    // Evolution → Codegen
    relate(
        ont,
        NS_CODEGEN,
        "mutator",
        RelationType::Enables,
        NS_EVOLUTION,
        "population",
    );
    relate(
        ont,
        NS_CODEGEN,
        "crossover",
        RelationType::Enables,
        NS_EVOLUTION,
        "population",
    );

    // Compute → Neural
    relate(
        ont,
        NS_COMPUTE,
        "backend",
        RelationType::Enables,
        NS_NEURAL,
        "loss",
    );
    relate(
        ont,
        NS_COMPUTE,
        "backend",
        RelationType::Enables,
        NS_NEURAL,
        "optimizer",
    );
}

// ============================================================================
// Architecture Recipes
// ============================================================================

/// A step in an architecture recipe — which component to use and how.
#[derive(Debug, Clone)]
pub struct CompositionStep {
    /// Component name (matches ontology concept local_name).
    pub component: String,
    /// Human-readable description of what this step does.
    pub description: String,
    /// Namespace of the component.
    pub namespace: String,
}

/// A composable architecture template that agents can instantiate.
#[derive(Debug, Clone)]
pub struct ArchitectureRecipe {
    /// Recipe name (e.g., "transformer_encoder_block").
    pub name: String,
    /// Human description.
    pub label: String,
    /// Tasks this recipe is good for.
    pub tasks: Vec<String>,
    /// Ordered composition steps.
    pub steps: Vec<CompositionStep>,
    /// Rough parameter count formula.
    pub param_formula: String,
    /// Example Rust snippet showing instantiation.
    pub example_code: String,
}

/// Registry of architecture recipes.
pub struct RecipeRegistry {
    recipes: Vec<ArchitectureRecipe>,
}

impl RecipeRegistry {
    /// Build the standard recipe registry with canonical architecture templates.
    pub fn build() -> Self {
        let recipes = vec![
            Self::transformer_encoder_block(),
            Self::mlp_block(),
            Self::resnet_block(),
            Self::rnn_sequence_model(),
            Self::classifier_head(),
            Self::conv_feature_extractor(),
        ];
        Self { recipes }
    }

    /// All recipes.
    pub fn all(&self) -> &[ArchitectureRecipe] {
        &self.recipes
    }

    /// Find recipes suitable for a given task.
    pub fn for_task(&self, task: &str) -> Vec<&ArchitectureRecipe> {
        let task_lower = task.to_lowercase();
        self.recipes
            .iter()
            .filter(|r| {
                r.tasks
                    .iter()
                    .any(|t| t.to_lowercase().contains(&task_lower))
            })
            .collect()
    }

    /// Find a recipe by name.
    pub fn by_name(&self, name: &str) -> Option<&ArchitectureRecipe> {
        self.recipes.iter().find(|r| r.name == name)
    }

    // ── Canonical recipes ────────────────────────────────────────────────

    fn transformer_encoder_block() -> ArchitectureRecipe {
        ArchitectureRecipe {
            name: "transformer_encoder_block".into(),
            label: "Transformer Encoder Block (Pre-Norm)".into(),
            tasks: vec![
                "sequence_modeling".into(),
                "text_classification".into(),
                "language_modeling".into(),
                "machine_translation".into(),
            ],
            steps: vec![
                CompositionStep {
                    component: "layer_norm".into(),
                    description: "Pre-norm before attention".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "multi_head_attention".into(),
                    description: "Self-attention over sequence".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "dropout".into(),
                    description: "Attention dropout".into(),
                    namespace: NS_NEURAL.into(),
                },
                // Residual connection (implicit — add input)
                CompositionStep {
                    component: "layer_norm".into(),
                    description: "Pre-norm before FFN".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "linear".into(),
                    description: "FFN up-projection (d_model → 4*d_model)".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "gelu".into(),
                    description: "Activation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "linear".into(),
                    description: "FFN down-projection (4*d_model → d_model)".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "dropout".into(),
                    description: "FFN dropout".into(),
                    namespace: NS_NEURAL.into(),
                },
            ],
            param_formula: "12 * d_model^2 + 13 * d_model (per block)".into(),
            example_code: concat!(
                "let block = vec![\n",
                "    Box::new(LayerNorm::new(d_model)),\n",
                "    Box::new(MultiHeadAttention::new(d_model, num_heads)),\n",
                "    Box::new(Dropout::new(0.1)),\n",
                "    Box::new(LayerNorm::new(d_model)),\n",
                "    Box::new(Linear::new(d_model, 4 * d_model)),\n",
                "    // GELU activation applied in forward pass\n",
                "    Box::new(Linear::new(4 * d_model, d_model)),\n",
                "    Box::new(Dropout::new(0.1)),\n",
                "];\n",
            )
            .into(),
        }
    }

    fn mlp_block() -> ArchitectureRecipe {
        ArchitectureRecipe {
            name: "mlp_block".into(),
            label: "Multi-Layer Perceptron".into(),
            tasks: vec![
                "tabular_regression".into(),
                "tabular_classification".into(),
                "function_approximation".into(),
            ],
            steps: vec![
                CompositionStep {
                    component: "linear".into(),
                    description: "Input projection".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "relu".into(),
                    description: "Activation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "dropout".into(),
                    description: "Regularisation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "linear".into(),
                    description: "Hidden layer".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "relu".into(),
                    description: "Activation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "linear".into(),
                    description: "Output projection".into(),
                    namespace: NS_NEURAL.into(),
                },
            ],
            param_formula: "in*h + h + h*h + h + h*out + out".into(),
            example_code: concat!(
                "let mlp = vec![\n",
                "    Box::new(Linear::new(input_dim, hidden)),\n",
                "    // ReLU activation applied in forward\n",
                "    Box::new(Linear::new(hidden, hidden)),\n",
                "    Box::new(Linear::new(hidden, output_dim)),\n",
                "];\n",
            )
            .into(),
        }
    }

    fn resnet_block() -> ArchitectureRecipe {
        ArchitectureRecipe {
            name: "resnet_block".into(),
            label: "Residual Block (ResNet-style)".into(),
            tasks: vec![
                "image_classification".into(),
                "object_detection".into(),
                "feature_extraction".into(),
            ],
            steps: vec![
                CompositionStep {
                    component: "conv2d".into(),
                    description: "3x3 convolution".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "batch_norm".into(),
                    description: "Batch normalisation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "relu".into(),
                    description: "Activation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "conv2d".into(),
                    description: "3x3 convolution".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "batch_norm".into(),
                    description: "Batch normalisation".into(),
                    namespace: NS_NEURAL.into(),
                },
                // Residual add (implicit — add input)
                CompositionStep {
                    component: "relu".into(),
                    description: "Post-residual activation".into(),
                    namespace: NS_NEURAL.into(),
                },
            ],
            param_formula: "2 * (in_ch * out_ch * 9 + out_ch)".into(),
            example_code: concat!(
                "// Use the built-in ResidualBlock from extended_layers\n",
                "let block = ResidualBlock::new(channels, channels, 3);\n",
            )
            .into(),
        }
    }

    fn rnn_sequence_model() -> ArchitectureRecipe {
        ArchitectureRecipe {
            name: "rnn_sequence_model".into(),
            label: "Recurrent Sequence Model".into(),
            tasks: vec![
                "sequence_modeling".into(),
                "time_series".into(),
                "language_modeling".into(),
            ],
            steps: vec![
                CompositionStep {
                    component: "embedding".into(),
                    description: "Token embedding".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "lstm_cell".into(),
                    description: "LSTM recurrent layer".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "dropout".into(),
                    description: "Recurrent dropout".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "linear".into(),
                    description: "Output projection".into(),
                    namespace: NS_NEURAL.into(),
                },
            ],
            param_formula: "vocab*d + 4*(d*d + d*d + d) + d*out + out".into(),
            example_code: concat!(
                "let rnn = vec![\n",
                "    Box::new(Embedding::new(vocab_size, embed_dim)),\n",
                "    Box::new(LSTMCell::new(embed_dim, hidden_dim)),\n",
                "    Box::new(Dropout::new(0.3)),\n",
                "    Box::new(Linear::new(hidden_dim, output_dim)),\n",
                "];\n",
            )
            .into(),
        }
    }

    fn classifier_head() -> ArchitectureRecipe {
        ArchitectureRecipe {
            name: "classifier_head".into(),
            label: "Classification Head".into(),
            tasks: vec![
                "image_classification".into(),
                "text_classification".into(),
                "multi_class_classification".into(),
            ],
            steps: vec![
                CompositionStep {
                    component: "global_avg_pool".into(),
                    description: "Global average pooling (for spatial inputs)".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "linear".into(),
                    description: "Feature projection".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "relu".into(),
                    description: "Activation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "dropout".into(),
                    description: "Regularisation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "linear".into(),
                    description: "Logits layer (output_dim = num_classes)".into(),
                    namespace: NS_NEURAL.into(),
                },
            ],
            param_formula: "in*h + h + h*num_classes + num_classes".into(),
            example_code: concat!(
                "let head = vec![\n",
                "    // GlobalAvgPool applied before this\n",
                "    Box::new(Linear::new(feature_dim, 256)),\n",
                "    // ReLU activation\n",
                "    Box::new(Dropout::new(0.5)),\n",
                "    Box::new(Linear::new(256, num_classes)),\n",
                "];\n",
            )
            .into(),
        }
    }

    fn conv_feature_extractor() -> ArchitectureRecipe {
        ArchitectureRecipe {
            name: "conv_feature_extractor".into(),
            label: "Convolutional Feature Extractor".into(),
            tasks: vec![
                "image_classification".into(),
                "object_detection".into(),
                "image_segmentation".into(),
            ],
            steps: vec![
                CompositionStep {
                    component: "conv2d".into(),
                    description: "Initial 3x3 conv, stride 2".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "batch_norm".into(),
                    description: "Normalisation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "relu".into(),
                    description: "Activation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "max_pool_2d".into(),
                    description: "Spatial downsampling".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "conv2d".into(),
                    description: "Deeper conv, more channels".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "batch_norm".into(),
                    description: "Normalisation".into(),
                    namespace: NS_NEURAL.into(),
                },
                CompositionStep {
                    component: "relu".into(),
                    description: "Activation".into(),
                    namespace: NS_NEURAL.into(),
                },
            ],
            param_formula: "dependent on channel progression".into(),
            example_code: concat!(
                "let features = vec![\n",
                "    Box::new(Conv2d::new(3, 64, 3)),\n",
                "    Box::new(BatchNorm::new(64)),\n",
                "    // ReLU + MaxPool applied in forward\n",
                "    Box::new(Conv2d::new(64, 128, 3)),\n",
                "    Box::new(BatchNorm::new(128)),\n",
                "];\n",
            )
            .into(),
        }
    }
}

// ============================================================================
// CatalogEntry — unified search result
// ============================================================================

/// A unified search result that can reference a framework concept, AI domain
/// concept, or historical contribution.
#[derive(Debug, Clone)]
pub enum CatalogEntry {
    /// A concept from the framework ontology (layers, ops, modules, etc.).
    FrameworkConcept(Concept),
    /// A concept from the AI domain knowledge base.
    DomainConcept {
        /// Concept name.
        name: String,
        /// Domain.
        domain: String,
        /// Brief definition.
        definition: String,
    },
    /// A historical contribution.
    HistoricalContribution {
        /// Title.
        title: String,
        /// Year.
        year: u32,
        /// Authors.
        authors: Vec<String>,
    },
    /// An architecture recipe.
    Recipe(ArchitectureRecipe),
}

impl CatalogEntry {
    /// Display name for the entry.
    pub fn name(&self) -> &str {
        match self {
            Self::FrameworkConcept(c) => {
                if c.label.is_empty() {
                    &c.id.local_name
                } else {
                    &c.label
                }
            }
            Self::DomainConcept { name, .. } => name,
            Self::HistoricalContribution { title, .. } => title,
            Self::Recipe(r) => &r.name,
        }
    }
}

// ============================================================================
// CapabilityDescriptor — structured capability listing
// ============================================================================

/// Describes a single framework capability with enough detail for an agent
/// to decide whether and how to use it.
#[derive(Debug, Clone)]
pub struct CapabilityDescriptor {
    /// Unique name (matches ontology concept).
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Human label.
    pub label: String,
    /// Rust type name (if applicable).
    pub rust_type: Option<String>,
    /// Constructor pattern (if applicable).
    pub constructor: Option<String>,
    /// Whether this component is differentiable.
    pub differentiable: bool,
    /// Whether this is a multi-agent component.
    pub multi_agent: bool,
}

// ============================================================================
// FrameworkCatalog — the unified facade
// ============================================================================

/// Unified entry point for discovering everything the RMI framework provides.
///
/// Merges the framework structure ontology, AI domain concepts, AI history,
/// and architecture recipes into a single queryable surface.
pub struct FrameworkCatalog {
    /// The unified framework ontology (extended with all modules).
    pub ontology: Ontology,
    /// AI domain concepts (backpropagation, transformers, etc.).
    pub concepts: AIConceptsOntology,
    /// AI history knowledge base.
    pub history: AIHistoryKB,
    /// Architecture composition recipes.
    pub recipes: RecipeRegistry,
}

impl FrameworkCatalog {
    /// Build the complete catalog — merges all knowledge systems and
    /// cross-links AI domain concepts to framework implementation concepts.
    pub fn build() -> Self {
        // 1. Build the base framework ontology
        let ontology = FrameworkOntology::build();

        // 2. Extend with all missing modules
        extend_ontology(&ontology);

        // 3. Build domain knowledge
        let concepts = AIConceptsOntology::with_core_concepts();
        let history = AIHistoryKB::with_history();

        // 4. Cross-link domain concepts → framework concepts
        Self::cross_link(&ontology, &concepts);

        // 5. Build recipes
        let recipes = RecipeRegistry::build();

        Self {
            ontology,
            concepts,
            history,
            recipes,
        }
    }

    /// Search across all knowledge systems for a query string.
    ///
    /// Returns matching entries from the framework ontology, domain concepts,
    /// historical contributions, and recipes.
    pub fn search(&self, query: &str) -> Vec<CatalogEntry> {
        let q = query.to_lowercase();
        let mut results = Vec::new();

        // 1. Search framework ontology
        let all_concepts = self.ontology.query(&OntologyQuery::new());
        for c in all_concepts {
            let name_match = c.id.local_name.to_lowercase().contains(&q);
            let label_match = c.label.to_lowercase().contains(&q);
            let attr_match = c.attributes.values().any(|v| {
                if let AttributeValue::String(s) = v {
                    s.to_lowercase().contains(&q)
                } else {
                    false
                }
            });
            if name_match || label_match || attr_match {
                results.push(CatalogEntry::FrameworkConcept(c));
            }
        }

        // 2. Search AI domain concepts by tag + task (union of matches)
        {
            let mut seen_names = std::collections::HashSet::new();
            // Search by tag match
            for c in self.concepts.by_tag(&q) {
                if seen_names.insert(c.name.clone()) {
                    results.push(CatalogEntry::DomainConcept {
                        name: c.name.clone(),
                        domain: format!("{:?}", c.domain),
                        definition: c.definition.clone(),
                    });
                }
            }
            // Search by task match
            for c in self.concepts.for_task(&q) {
                if seen_names.insert(c.name.clone()) {
                    results.push(CatalogEntry::DomainConcept {
                        name: c.name.clone(),
                        domain: format!("{:?}", c.domain),
                        definition: c.definition.clone(),
                    });
                }
            }
            // Search by name match via get_by_name
            if let Some(c) = self.concepts.get_by_name(&q) {
                if seen_names.insert(c.name.clone()) {
                    results.push(CatalogEntry::DomainConcept {
                        name: c.name.clone(),
                        domain: format!("{:?}", c.domain),
                        definition: c.definition.clone(),
                    });
                }
            }
        }

        // 3. Search history
        for c in self.history.search_concept(query) {
            results.push(CatalogEntry::HistoricalContribution {
                title: c.title.clone(),
                year: c.year as u32,
                authors: c.authors.clone(),
            });
        }

        // 4. Search recipes
        for r in &self.recipes.recipes {
            if r.name.to_lowercase().contains(&q)
                || r.label.to_lowercase().contains(&q)
                || r.tasks.iter().any(|t| t.to_lowercase().contains(&q))
            {
                results.push(CatalogEntry::Recipe(r.clone()));
            }
        }

        results
    }

    /// Find components suitable for a given task.
    pub fn for_task(&self, task: &str) -> Vec<CatalogEntry> {
        let task_lower = task.to_lowercase();
        let mut results = Vec::new();

        // Check AI concepts ontology
        for c in self.concepts.for_task(task) {
            results.push(CatalogEntry::DomainConcept {
                name: c.name.clone(),
                domain: format!("{:?}", c.domain),
                definition: c.definition.clone(),
            });
        }

        // Check ontology attributes for recommended_task / recommended_for
        let all = self.ontology.query(&OntologyQuery::new());
        for c in all {
            let matches = c.attributes.values().any(|v| {
                if let AttributeValue::String(s) = v {
                    s.to_lowercase().contains(&task_lower)
                } else {
                    false
                }
            });
            if matches {
                results.push(CatalogEntry::FrameworkConcept(c));
            }
        }

        // Check recipes
        for r in self.recipes.for_task(task) {
            results.push(CatalogEntry::Recipe(r.clone()));
        }

        results
    }

    /// List all available capabilities as structured descriptors.
    pub fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        let all = self.ontology.query(&OntologyQuery::new());
        all.into_iter()
            .filter(|c| matches!(c.concept_type, ConceptType::Entity | ConceptType::Process))
            .map(|c| {
                let rust_type = c.attributes.get("rust_type").and_then(|v| {
                    if let AttributeValue::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                });
                let constructor = c.attributes.get("constructor").and_then(|v| {
                    if let AttributeValue::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                });
                let differentiable = matches!(
                    c.attributes.get("differentiable"),
                    Some(AttributeValue::Bool(true))
                );
                let multi_agent = matches!(
                    c.attributes.get("multi_agent"),
                    Some(AttributeValue::Bool(true))
                );

                CapabilityDescriptor {
                    name: c.id.local_name.clone(),
                    namespace: c.id.namespace.clone(),
                    label: if c.label.is_empty() {
                        c.id.local_name.clone()
                    } else {
                        c.label.clone()
                    },
                    rust_type,
                    constructor,
                    differentiable,
                    multi_agent,
                }
            })
            .collect()
    }

    /// Get architecture recipes suitable for a task.
    pub fn recipes_for_task(&self, task: &str) -> Vec<&ArchitectureRecipe> {
        self.recipes.for_task(task)
    }

    /// Get all composable components (delegates to IntrospectionQueries).
    pub fn composable_components(&self) -> Vec<Concept> {
        self.ontology.composable_components()
    }

    /// Get all differentiable components.
    pub fn differentiable_components(&self) -> Vec<Concept> {
        self.ontology.differentiable_components()
    }

    /// Get all multi-agent components.
    pub fn multi_agent_components(&self) -> Vec<Concept> {
        let all = self.ontology.query(&OntologyQuery::new());
        all.into_iter()
            .filter(|c| {
                matches!(
                    c.attributes.get("multi_agent"),
                    Some(AttributeValue::Bool(true))
                )
            })
            .collect()
    }

    /// Query the composition graph: what can follow `component`?
    pub fn can_follow(&self, component: &str) -> Vec<Concept> {
        self.ontology.can_receive_from(component)
    }

    /// Query the composition graph: what can precede `component`?
    pub fn can_precede(&self, component: &str) -> Vec<Concept> {
        self.ontology.can_feed_into(component)
    }

    // ── Cross-linking ────────────────────────────────────────────────────

    /// Cross-link AI domain concepts to framework implementation concepts.
    ///
    /// For example: the domain concept "Transformer" maps to framework
    /// concepts `multi_head_attention`, `layer_norm`, `linear` (attention
    /// blocks), and the `transformer_encoder_block` recipe.
    fn cross_link(ont: &Ontology, concepts: &AIConceptsOntology) {
        // Map well-known AI concept names → framework ontology concept names
        let cross_links: Vec<(&str, &str, &str)> = vec![
            // (ai_concept_name, framework_ns, framework_concept_name)
            ("neural_network", NS_NEURAL, "layer"),
            ("convolutional_neural_network", NS_NEURAL, "conv2d"),
            ("recurrent_neural_network", NS_NEURAL, "recurrent"),
            ("transformer", NS_NEURAL, "multi_head_attention"),
            ("attention_mechanism", NS_NEURAL, "attention"),
            ("batch_normalization", NS_NEURAL, "batch_norm"),
            ("layer_normalization", NS_NEURAL, "layer_norm"),
            ("dropout", NS_NEURAL, "dropout"),
            ("relu", NS_NEURAL, "relu"),
            ("sigmoid", NS_NEURAL, "sigmoid"),
            ("softmax", NS_NEURAL, "softmax"),
            ("gelu", NS_NEURAL, "gelu"),
            ("embedding", NS_NEURAL, "embedding"),
            ("sgd", NS_NEURAL, "sgd"),
            ("adam", NS_NEURAL, "adam"),
            ("cross_entropy", NS_NEURAL, "cross_entropy_loss"),
            ("mean_squared_error", NS_NEURAL, "mse_loss"),
            ("backpropagation", NS_NEURAL, "loss"),
            ("knowledge_representation", NS_SYMBOLIC, "first_order_logic"),
            ("automated_reasoning", NS_SYMBOLIC, "inference_engine"),
            ("planning", NS_SYMBOLIC, "planner"),
            ("federated_learning", NS_TRAINING, "federated_trainer"),
        ];

        for (concept_name, fw_ns, fw_name) in cross_links {
            if concepts.get_by_name(concept_name).is_some() {
                // Add an ImplementedBy relation from a bridge concept
                let bridge_name = format!("domain_link_{}", concept_name);
                ont.add_concept(crate::core::discoverability::concept(
                    "rmi.domain",
                    &bridge_name,
                    ConceptType::Relation,
                    &format!("Domain link: {}", concept_name),
                    vec![
                        ("ai_concept", s(concept_name)),
                        ("framework_concept", s(fw_name)),
                        ("framework_namespace", s(fw_ns)),
                    ],
                ));
                relate(
                    ont,
                    "rmi.domain",
                    &bridge_name,
                    RelationType::InstanceOf,
                    fw_ns,
                    fw_name,
                );
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extend_ontology_adds_modules() {
        let ont = FrameworkOntology::build();
        let before = ont.len();
        extend_ontology(&ont);
        let after = ont.len();
        // Should add significantly more concepts
        assert!(
            after > before + 30,
            "Expected > 30 new concepts, got {} (before={before}, after={after})",
            after - before
        );
    }

    #[test]
    fn test_catalog_build_non_empty() {
        let catalog = FrameworkCatalog::build();
        assert!(
            catalog.ontology.len() > 80,
            "Expected > 80 concepts in unified ontology"
        );
        assert!(catalog.recipes.all().len() >= 6, "Expected >= 6 recipes");
    }

    #[test]
    fn test_catalog_search_attention() {
        let catalog = FrameworkCatalog::build();
        let hits = catalog.search("attention");
        assert!(!hits.is_empty(), "Should find attention-related entries");
        // Should find both framework and domain entries
        let has_framework = hits
            .iter()
            .any(|e| matches!(e, CatalogEntry::FrameworkConcept(_)));
        assert!(has_framework, "Should include framework concepts");
    }

    #[test]
    fn test_catalog_search_transformer() {
        let catalog = FrameworkCatalog::build();
        let hits = catalog.search("transformer");
        assert!(!hits.is_empty(), "Should find transformer-related entries");
    }

    #[test]
    fn test_catalog_for_task() {
        let catalog = FrameworkCatalog::build();
        let hits = catalog.for_task("classification");
        assert!(
            !hits.is_empty(),
            "Should find components for classification"
        );
    }

    #[test]
    fn test_catalog_capabilities() {
        let catalog = FrameworkCatalog::build();
        let caps = catalog.capabilities();
        assert!(
            caps.len() > 20,
            "Expected > 20 capabilities, got {}",
            caps.len()
        );

        // Should include both neural and multi-agent components
        let has_linear = caps.iter().any(|c| c.name == "linear");
        let has_runtime = caps.iter().any(|c| c.multi_agent);
        assert!(has_linear, "Should include Linear");
        assert!(has_runtime, "Should include multi-agent components");
    }

    #[test]
    fn test_catalog_recipes_for_task() {
        let catalog = FrameworkCatalog::build();
        let recipes = catalog.recipes_for_task("sequence_modeling");
        assert!(
            !recipes.is_empty(),
            "Should find recipes for sequence modeling"
        );
        assert!(recipes
            .iter()
            .any(|r| r.name == "transformer_encoder_block"));
    }

    #[test]
    fn test_catalog_multi_agent_components() {
        let catalog = FrameworkCatalog::build();
        let multi = catalog.multi_agent_components();
        assert!(
            multi.len() >= 5,
            "Expected >= 5 multi-agent components, got {}",
            multi.len()
        );
    }

    #[test]
    fn test_catalog_composition_queries() {
        let catalog = FrameworkCatalog::build();
        let after_linear = catalog.can_follow("linear");
        assert!(
            !after_linear.is_empty(),
            "Linear should connect to downstream components"
        );

        let before_attention = catalog.can_precede("multi_head_attention");
        assert!(
            !before_attention.is_empty(),
            "Something should feed into attention"
        );
    }

    #[test]
    fn test_recipe_registry_by_name() {
        let recipes = RecipeRegistry::build();
        assert!(recipes.by_name("transformer_encoder_block").is_some());
        assert!(recipes.by_name("mlp_block").is_some());
        assert!(recipes.by_name("nonexistent").is_none());
    }

    #[test]
    fn test_recipe_steps_non_empty() {
        let recipes = RecipeRegistry::build();
        for recipe in recipes.all() {
            assert!(
                !recipe.steps.is_empty(),
                "Recipe '{}' should have steps",
                recipe.name
            );
            assert!(
                !recipe.tasks.is_empty(),
                "Recipe '{}' should have tasks",
                recipe.name
            );
            assert!(
                !recipe.example_code.is_empty(),
                "Recipe '{}' should have code",
                recipe.name
            );
        }
    }

    #[test]
    fn test_codegen_concepts_present() {
        let ont = FrameworkOntology::build();
        extend_ontology(&ont);
        let codegen = ont.in_namespace(NS_CODEGEN);
        assert!(
            codegen.len() >= 8,
            "Expected >= 8 codegen concepts, got {}",
            codegen.len()
        );
    }

    #[test]
    fn test_training_concepts_present() {
        let ont = FrameworkOntology::build();
        extend_ontology(&ont);
        let training = ont.in_namespace(NS_TRAINING);
        assert!(
            training.len() >= 4,
            "Expected >= 4 training concepts, got {}",
            training.len()
        );
    }

    #[test]
    fn test_collaboration_concepts_present() {
        let ont = FrameworkOntology::build();
        extend_ontology(&ont);
        let collab = ont.in_namespace(NS_COLLABORATION);
        assert!(
            collab.len() >= 5,
            "Expected >= 5 collaboration concepts, got {}",
            collab.len()
        );
    }

    #[test]
    fn test_loss_and_optimizer_concepts() {
        let ont = FrameworkOntology::build();
        extend_ontology(&ont);

        // Losses
        let loss_concept = ont.lookup("mse_loss");
        assert!(loss_concept.is_some(), "Should have MSE loss concept");

        // Optimizers
        let adam_concept = ont.lookup("adam");
        assert!(adam_concept.is_some(), "Should have Adam optimizer concept");
    }

    #[test]
    fn test_api_signatures_linked() {
        let ont = FrameworkOntology::build();
        extend_ontology(&ont);

        // Check that API spec concepts exist
        let linear_api = ont.lookup("linear_api");
        assert!(linear_api.is_some(), "Should have linear API spec");
        let spec = linear_api.unwrap();
        assert!(spec.attributes.contains_key("constructor"));
        assert!(spec.attributes.contains_key("input_shape"));
        assert!(spec.attributes.contains_key("output_shape"));
    }

    #[test]
    fn test_cross_links_created() {
        let catalog = FrameworkCatalog::build();
        // Domain links should exist
        let links = catalog.search("domain_link");
        assert!(!links.is_empty(), "Should have cross-link bridge concepts");
    }

    #[test]
    fn test_extra_compute_backends() {
        let ont = FrameworkOntology::build();
        extend_ontology(&ont);
        let backends = ont.in_namespace(NS_COMPUTE);
        // Original: backend, cpu, cuda. New: webgpu, metal, vulkan, wasm
        assert!(
            backends.len() >= 6,
            "Expected >= 6 compute concepts, got {}",
            backends.len()
        );
    }

    #[test]
    fn test_catalog_entry_name() {
        let entry = CatalogEntry::DomainConcept {
            name: "Transformer".into(),
            domain: "Neural".into(),
            definition: "Self-attention based architecture".into(),
        };
        assert_eq!(entry.name(), "Transformer");
    }
}
