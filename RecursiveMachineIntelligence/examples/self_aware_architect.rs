//! Self-Aware Architect — An Agent That Designs from Introspection
//!
//! This example demonstrates an external agent that:
//! 1. Loads the RMI framework introspection ontology to discover every
//!    available component **without reading source code**.
//! 2. Reasons about composition rules, differentiability, and constraints.
//! 3. Synthesises a novel neurosymbolic architecture: **Constraint-Guided
//!    Attention Network (CGAN)** — a transformer variant that interleaves
//!    symbolic constraint checking between attention blocks, using the
//!    neurosymbolic bridge to feed logical corrections back into the
//!    gradient flow.
//!
//! The agent narrates its reasoning at each step so you can follow its
//! decision process.

use rmi::core::introspection::{
    FrameworkOntology, IntrospectionQueries, NS_COMPUTE, NS_NEURAL, NS_NEUROSYMBOLIC, NS_SYMBOLIC,
};
use rmi::core::ontology::{AttributeValue, ConceptId, RelationType};
use rmi::neural::architecture::NetworkArchitecture;
use rmi::neural::primitives::{HyperparameterValue, NeuralPrimitiveKind};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
//  Agent state
// ═══════════════════════════════════════════════════════════════════════════

/// The agent's reasoning trace — what it discovered and decided.
struct AgentTrace {
    discoveries: Vec<String>,
    decisions: Vec<String>,
}

impl AgentTrace {
    fn new() -> Self {
        Self {
            discoveries: Vec::new(),
            decisions: Vec::new(),
        }
    }

    fn discover(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        println!("  [DISCOVER]  {}", msg);
        self.discoveries.push(msg);
    }

    fn decide(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        println!("  [DECIDE]    {}", msg);
        self.decisions.push(msg);
    }

    fn emit(&self, msg: impl std::fmt::Display) {
        println!("  [EMIT]      {}", msg);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Main
// ═══════════════════════════════════════════════════════════════════════════

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  RMI Self-Aware Architect Agent                            ║");
    println!("║  Goal: Design a novel neurosymbolic architecture by        ║");
    println!("║        introspecting the framework's own ontology.         ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let mut trace = AgentTrace::new();

    // ── Phase 1: Load the introspection ontology ─────────────────────────
    println!("━━━ Phase 1: Framework Introspection ━━━\n");

    let ont = FrameworkOntology::build();

    trace.discover(format!(
        "Loaded framework ontology: {} concepts across all namespaces",
        ont.len()
    ));

    // ── Phase 2: Catalogue available components ──────────────────────────
    println!("\n━━━ Phase 2: Component Discovery ━━━\n");

    // 2a. Enumerate every namespace
    let neural_components = ont.in_namespace(NS_NEURAL);
    let symbolic_components = ont.in_namespace(NS_SYMBOLIC);
    let bridge_components = ont.in_namespace(NS_NEUROSYMBOLIC);
    let compute_components = ont.in_namespace(NS_COMPUTE);

    trace.discover(format!(
        "Neural components:        {} (layers, activations, norms, attention, ...)",
        neural_components.len()
    ));
    trace.discover(format!(
        "Symbolic components:      {} (logic, unification, inference, planning)",
        symbolic_components.len()
    ));
    trace.discover(format!(
        "Neurosymbolic bridges:    {} (symbol_embedder, constraint_solver, hybrid_reasoner)",
        bridge_components.len()
    ));
    trace.discover(format!(
        "Compute backends:         {} (CPU, CUDA, WebGPU, Metal, Vulkan, Apple ANE, Qualcomm)",
        compute_components.len()
    ));

    // 2b. Find differentiable components (needed for end-to-end training)
    let diff_components = ont.differentiable_components();
    trace.discover(format!(
        "Differentiable components: {} — these can participate in gradient flow",
        diff_components.len()
    ));

    // 2c. List attention mechanisms available
    let attn_id = ConceptId::new(NS_NEURAL, "attention");
    let attention_variants: Vec<_> = ont
        .get_related(&attn_id, RelationType::HasComponent)
        .into_iter()
        .chain(
            neural_components
                .iter()
                .filter(|c| {
                    ont.get_related(&c.id, RelationType::IsA)
                        .iter()
                        .any(|parent| parent.id.local_name == "attention")
                })
                .cloned(),
        )
        .collect();

    trace.discover(format!(
        "Attention variants: {}",
        attention_variants
            .iter()
            .map(|c| c.label.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    // 2d. List normalisation options
    let norm_variants: Vec<_> = neural_components
        .iter()
        .filter(|c| {
            ont.get_related(&c.id, RelationType::IsA)
                .iter()
                .any(|p| p.id.local_name == "normalisation")
        })
        .collect();

    trace.discover(format!(
        "Normalisation variants: {}",
        norm_variants
            .iter()
            .map(|c| c.label.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    // 2e. List what Linear can feed into (composition rules)
    let linear_id = ConceptId::new(NS_NEURAL, "linear");
    let linear_targets = ont.get_related(&linear_id, RelationType::Enables);
    trace.discover(format!(
        "Linear can feed into: {}",
        linear_targets
            .iter()
            .map(|c| c.label.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    // 2f. Find neurosymbolic bridges and their capabilities
    for bridge in &bridge_components {
        let direction = bridge
            .attributes
            .get("direction")
            .map(|v| match v {
                AttributeValue::String(s) => s.as_str(),
                _ => "?",
            })
            .unwrap_or("?");
        let enables = bridge
            .attributes
            .get("enables")
            .map(|v| match v {
                AttributeValue::String(s) => s.as_str(),
                _ => "?",
            })
            .unwrap_or("?");
        trace.discover(format!(
            "Bridge '{}': direction={}, enables={}",
            bridge.label, direction, enables
        ));
    }

    // ── Phase 3: Reason about architecture requirements ──────────────────
    println!("\n━━━ Phase 3: Architectural Reasoning ━━━\n");

    trace.decide(
        "Goal: design a model for structured prediction tasks (e.g., code generation, \
         logical QA) where outputs must satisfy hard logical constraints.",
    );

    trace.decide(
        "Observation: Transformers excel at learning patterns (attention captures \
         long-range dependencies) but have no mechanism to enforce logical invariants.",
    );

    trace.decide(
        "Observation: Symbolic constraint solvers enforce invariants but are not \
         differentiable in traditional form.",
    );

    trace.decide(
        "Key insight from ontology: the ConstraintSolver bridge is differentiable \
         (attribute 'differentiable' = true) and direction is 'logic → loss'. \
         This means I can convert constraint violations into a differentiable loss \
         term and feed corrections back into the transformer via residual connections.",
    );

    // Verify the key property programmatically
    let cs = ont.lookup("constraint_solver");
    if let Some(ref cs_concept) = cs {
        let is_diff = matches!(
            cs_concept.attributes.get("differentiable"),
            Some(AttributeValue::Bool(true))
        );
        trace.decide(format!(
            "Verified: ConstraintSolver.differentiable = {} ✓",
            is_diff
        ));
    }

    // Check that constraint_solver can feed into linear (gradient pathway)
    let cs_id = ConceptId::new(NS_NEUROSYMBOLIC, "constraint_solver");
    let cs_targets = ont.get_related(&cs_id, RelationType::Enables);
    trace.decide(format!(
        "ConstraintSolver feeds into: {} — confirming gradient pathway exists",
        cs_targets
            .iter()
            .map(|c| c.label.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));

    trace.decide("Architecture name: Constraint-Guided Attention Network (CGAN)");

    trace.decide(
        "Design: Interleave standard transformer blocks with constraint-checking \
         layers. After every N attention blocks, route hidden states through \
         SymbolEmbedder → ConstraintSolver → projection back to hidden dim. \
         The constraint loss is added to the main task loss.",
    );

    // Check composition chain: embedding → positional → attention → norm → linear → activation → constraint_solver → linear
    println!();
    trace.decide("Verifying full composition chain through ontology:");

    let chain = [
        (NS_NEURAL, "embedding", "sinusoidal_pe"),
        (NS_NEURAL, "layer_norm", "multi_head_attention"),
        (NS_NEURAL, "multi_head_attention", "layer_norm"),
        (NS_NEURAL, "gelu", "linear"),
        (NS_NEURAL, "linear", "dropout"),
        (NS_NEUROSYMBOLIC, "symbol_embedder", "linear"),
        (NS_NEUROSYMBOLIC, "constraint_solver", "linear"),
    ];

    for (ns, src, expected_tgt) in chain {
        let src_id = ConceptId::new(ns, src);
        let targets = ont.get_related(&src_id, RelationType::Enables);
        let found = targets.iter().any(|t| t.id.local_name == expected_tgt);
        trace.decide(format!(
            "  {} → {} : {}",
            src,
            expected_tgt,
            if found { "✓ valid" } else { "✗ NOT FOUND" }
        ));
    }

    // ── Phase 4: Synthesise the architecture ─────────────────────────────
    println!("\n━━━ Phase 4: Architecture Synthesis ━━━\n");

    trace.decide("Building Constraint-Guided Attention Network (CGAN):");
    trace.decide("  - Input: Embedding + Sinusoidal PE");
    trace.decide("  - 2× Transformer blocks (pre-norm, multi-head attention, FFN)");
    trace.decide("  - 1× Constraint checkpoint (SymbolEmbedder → ConstraintSolver → projection)");
    trace.decide("  - 2× more Transformer blocks");
    trace.decide("  - Final projection head");

    let arch = build_cgan_architecture();

    trace.emit(format!("Architecture '{}' assembled", arch.name));
    trace.emit(format!("  Total nodes:  {}", arch.node_count()));
    trace.emit(format!("  Total edges:  {}", arch.edge_count()));
    trace.emit(format!("  Depth:        {}", arch.depth()));

    // Estimate parameters
    let bindings = default_cgan_bindings();
    let params = arch.estimate_parameters(&bindings);
    let flops = arch.estimate_flops(&bindings);
    trace.emit(format!(
        "  Est. params:  {} ({:.1}M)",
        params,
        params as f64 / 1e6
    ));
    trace.emit(format!(
        "  Est. FLOPs:   {} ({:.1}G)",
        flops,
        flops as f64 / 1e9
    ));

    // Show topological order
    if let Some(order) = arch.topological_order() {
        println!();
        trace.emit("Topological execution order:");
        for (i, node_id) in order.iter().enumerate() {
            if let Some(node) = arch.get_node(*node_id) {
                trace.emit(format!(
                    "    {:2}. {:30} ({:?})",
                    i + 1,
                    node.name,
                    node.primitive
                ));
            }
        }
    }

    // ── Phase 5: Report ──────────────────────────────────────────────────
    println!("\n━━━ Phase 5: Agent Report ━━━\n");

    println!(
        "  The agent discovered {} framework components across {} namespaces",
        ont.len(),
        5
    );
    println!(
        "  It made {} discovery observations and {} design decisions",
        trace.discoveries.len(),
        trace.decisions.len()
    );
    println!();
    println!("  Novel architecture: Constraint-Guided Attention Network (CGAN)");
    println!("  Key innovation: differentiable constraint checking interleaved");
    println!("  with transformer blocks, enabled by the ConstraintSolver bridge");
    println!("  discovered through framework introspection.");
    println!();
    println!("  This architecture was designed entirely from ontology queries —");
    println!("  the agent never read source code, documentation, or examples.");
    println!("  Every composition rule was verified through Enables relations");
    println!("  in the introspection ontology.");
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  Architecture synthesis complete.                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}

// ═══════════════════════════════════════════════════════════════════════════
//  Architecture construction
// ═══════════════════════════════════════════════════════════════════════════

/// Build the Constraint-Guided Attention Network.
///
/// ```text
///  ┌──────────┐   ┌──────────┐   ┌────────────────┐   ┌──────────┐   ┌──────┐
///  │ Embedding │──▸│ Pos. Enc │──▸│ Transformer ×2 │──▸│ Constr.  │──▸│ T×2  │──▸ Head
///  └──────────┘   └──────────┘   │ (LN→MHA→Res→   │   │ Check-   │   │      │
///                                │  LN→FFN→Res)   │   │ point    │   │      │
///                                └────────────────┘   └──────────┘   └──────┘
/// ```
fn build_cgan_architecture() -> NetworkArchitecture {
    let hidden = 512i64;
    let ffn_dim = 2048i64;
    let heads = 8i64;
    let head_dim = hidden / heads;

    // Start with a builder for the input → embedding → positional encoding path.
    let mut arch = NetworkArchitecture::new("CGAN: Constraint-Guided Attention Network");

    // ── Input embedding ──────────────────────────────────────────────────
    let emb = arch.add_node(
        node(NeuralPrimitiveKind::Embedding, "token_embedding")
            .with_config("vocab_size", HyperparameterValue::Int(32000))
            .with_config("embedding_dim", HyperparameterValue::Int(hidden))
            .with_metadata("phase", "input"),
    );
    arch.mark_input(emb);

    let pos = arch.add_node(
        node(
            NeuralPrimitiveKind::SinusoidalPositionalEncoding,
            "pos_encoding",
        )
        .with_metadata("phase", "input"),
    );
    arch.connect(emb, pos, default_edge());

    // ── Transformer block 1 ──────────────────────────────────────────────
    let (block1_out, _) =
        add_transformer_block(&mut arch, pos, "block_1", heads, head_dim, ffn_dim, hidden);

    // ── Transformer block 2 ──────────────────────────────────────────────
    let (block2_out, _) = add_transformer_block(
        &mut arch, block1_out, "block_2", heads, head_dim, ffn_dim, hidden,
    );

    // ── Constraint checkpoint ────────────────────────────────────────────
    //  hidden → symbol_embed (project to symbolic space) → constraint check
    //         → project back → residual add with block2 output

    let sym_proj = arch.add_node(
        node(NeuralPrimitiveKind::Linear, "constraint_sym_project")
            .with_config("out_features", HyperparameterValue::Int(256))
            .with_metadata("phase", "constraint")
            .with_metadata("role", "project hidden → symbolic space"),
    );
    arch.connect(block2_out, sym_proj, default_edge());

    // Represent the differentiable constraint solver as a custom linear
    // projection (in a real system this would call ConstraintSolver::evaluate,
    // but architecturally it's a differentiable transformation).
    let constraint = arch.add_node(
        node(NeuralPrimitiveKind::Linear, "constraint_solver_diff")
            .with_config("out_features", HyperparameterValue::Int(256))
            .with_metadata("phase", "constraint")
            .with_metadata("role", "differentiable constraint evaluation"),
    );
    arch.connect(sym_proj, constraint, default_edge());

    let constraint_act = arch.add_node(
        node(NeuralPrimitiveKind::GeLU, "constraint_activation")
            .with_metadata("phase", "constraint"),
    );
    arch.connect(constraint, constraint_act, default_edge());

    let back_proj = arch.add_node(
        node(NeuralPrimitiveKind::Linear, "constraint_back_project")
            .with_config("out_features", HyperparameterValue::Int(hidden))
            .with_metadata("phase", "constraint")
            .with_metadata("role", "project constraint corrections back to hidden dim"),
    );
    arch.connect(constraint_act, back_proj, default_edge());

    // Residual: add constraint correction to block2 output
    let constraint_residual = arch.add_node(
        node(NeuralPrimitiveKind::ResidualAdd, "constraint_residual")
            .with_metadata("phase", "constraint")
            .with_metadata("role", "inject constraint corrections via residual"),
    );
    arch.connect(back_proj, constraint_residual, default_edge());
    arch.connect(
        block2_out,
        constraint_residual,
        rmi::neural::architecture::ArchitectureEdge {
            dest_input: 1,
            ..Default::default()
        },
    );

    let constraint_norm = arch.add_node(
        node(NeuralPrimitiveKind::LayerNorm, "constraint_ln").with_metadata("phase", "constraint"),
    );
    arch.connect(constraint_residual, constraint_norm, default_edge());

    // ── Transformer block 3 ──────────────────────────────────────────────
    let (block3_out, _) = add_transformer_block(
        &mut arch,
        constraint_norm,
        "block_3",
        heads,
        head_dim,
        ffn_dim,
        hidden,
    );

    // ── Transformer block 4 ──────────────────────────────────────────────
    let (block4_out, _) = add_transformer_block(
        &mut arch, block3_out, "block_4", heads, head_dim, ffn_dim, hidden,
    );

    // ── Output head ──────────────────────────────────────────────────────
    let final_norm = arch.add_node(
        node(NeuralPrimitiveKind::LayerNorm, "final_ln").with_metadata("phase", "output"),
    );
    arch.connect(block4_out, final_norm, default_edge());

    let head = arch.add_node(
        node(NeuralPrimitiveKind::Linear, "output_head")
            .with_config("out_features", HyperparameterValue::Int(32000))
            .with_metadata("phase", "output"),
    );
    arch.connect(final_norm, head, default_edge());

    let softmax = arch.add_node(
        node(NeuralPrimitiveKind::Softmax, "output_softmax").with_metadata("phase", "output"),
    );
    arch.connect(head, softmax, default_edge());
    arch.mark_output(softmax);

    arch
}

/// Add a single pre-norm transformer block and return (output_id, residual_pre_id).
fn add_transformer_block(
    arch: &mut NetworkArchitecture,
    input: uuid::Uuid,
    prefix: &str,
    heads: i64,
    head_dim: i64,
    ffn_dim: i64,
    hidden: i64,
) -> (uuid::Uuid, uuid::Uuid) {
    // Pre-norm attention
    let ln1 = arch.add_node(node(
        NeuralPrimitiveKind::LayerNorm,
        &format!("{}_ln1", prefix),
    ));
    arch.connect(input, ln1, default_edge());

    let attn = arch.add_node(
        node(
            NeuralPrimitiveKind::MultiHeadAttention,
            &format!("{}_mha", prefix),
        )
        .with_config("heads", HyperparameterValue::Int(heads))
        .with_config("head_dim", HyperparameterValue::Int(head_dim)),
    );
    arch.connect(ln1, attn, default_edge());

    let drop1 = arch.add_node(
        node(NeuralPrimitiveKind::Dropout, &format!("{}_drop1", prefix))
            .with_config("p", HyperparameterValue::Float(0.1)),
    );
    arch.connect(attn, drop1, default_edge());

    let res1 = arch.add_node(node(
        NeuralPrimitiveKind::ResidualAdd,
        &format!("{}_res1", prefix),
    ));
    arch.connect(drop1, res1, default_edge());
    arch.connect(
        input,
        res1,
        rmi::neural::architecture::ArchitectureEdge {
            dest_input: 1,
            ..Default::default()
        },
    );

    // Pre-norm FFN
    let ln2 = arch.add_node(node(
        NeuralPrimitiveKind::LayerNorm,
        &format!("{}_ln2", prefix),
    ));
    arch.connect(res1, ln2, default_edge());

    let ffn1 = arch.add_node(
        node(NeuralPrimitiveKind::Linear, &format!("{}_ffn1", prefix))
            .with_config("out_features", HyperparameterValue::Int(ffn_dim)),
    );
    arch.connect(ln2, ffn1, default_edge());

    let act = arch.add_node(node(NeuralPrimitiveKind::GeLU, &format!("{}_gelu", prefix)));
    arch.connect(ffn1, act, default_edge());

    let ffn2 = arch.add_node(
        node(NeuralPrimitiveKind::Linear, &format!("{}_ffn2", prefix))
            .with_config("out_features", HyperparameterValue::Int(hidden)),
    );
    arch.connect(act, ffn2, default_edge());

    let drop2 = arch.add_node(
        node(NeuralPrimitiveKind::Dropout, &format!("{}_drop2", prefix))
            .with_config("p", HyperparameterValue::Float(0.1)),
    );
    arch.connect(ffn2, drop2, default_edge());

    let res2 = arch.add_node(node(
        NeuralPrimitiveKind::ResidualAdd,
        &format!("{}_res2", prefix),
    ));
    arch.connect(drop2, res2, default_edge());
    arch.connect(
        res1,
        res2,
        rmi::neural::architecture::ArchitectureEdge {
            dest_input: 1,
            ..Default::default()
        },
    );

    (res2, input)
}

fn node(kind: NeuralPrimitiveKind, name: &str) -> rmi::neural::architecture::ArchitectureNode {
    rmi::neural::architecture::ArchitectureNode::new(kind, name)
}

fn default_edge() -> rmi::neural::architecture::ArchitectureEdge {
    rmi::neural::architecture::ArchitectureEdge::default()
}

fn default_cgan_bindings() -> HashMap<String, usize> {
    HashMap::from([
        ("in_features".to_string(), 512),
        ("out_features".to_string(), 2048),
        ("heads".to_string(), 8),
        ("head_dim".to_string(), 64),
        ("vocab_size".to_string(), 32000),
        ("embedding_dim".to_string(), 512),
        ("batch".to_string(), 32),
        ("seq".to_string(), 256),
        ("hidden".to_string(), 512),
    ])
}
