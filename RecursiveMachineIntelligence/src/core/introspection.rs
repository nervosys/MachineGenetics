//! Framework Introspection - Self-Describing Ontology
//!
//! Makes RMI fully observable to external agents by exposing the framework's
//! own structure — modules, types, operations, composition rules, and
//! constraints — as a queryable [`Ontology`].
//!
//! An external agent receiving this ontology can:
//! 1. Discover every available neural primitive, symbolic operator, and
//!    neurosymbolic bridge without reading source code.
//! 2. Query composition rules (what connects to what) and constraints
//!    (shape compatibility, differentiability, hardware affinity).
//! 3. Reason about trade-offs (FLOPs, memory, parallelisability) and
//!    synthesise novel architectures from first principles.
//!
//! # Example
//!
//! ```
//! use rmi::core::introspection::{FrameworkOntology, IntrospectionQueries};
//!
//! let ontology = FrameworkOntology::build();
//! // An agent can now discover all composable components:
//! let composable = ontology.composable_components();
//! // Or query a specific namespace:
//! let neural = ontology.in_namespace("rmi.neural");
//! ```

use crate::core::ontology::{
    AttributeValue, Concept, ConceptId, ConceptType, Ontology, Relation, RelationType,
};

// ── Namespace constants ─────────────────────────────────────────────────────

/// Root namespace for framework introspection concepts.
pub const NS_ROOT: &str = "rmi";
/// Neural sub-namespace.
pub const NS_NEURAL: &str = "rmi.neural";
/// Symbolic sub-namespace.
pub const NS_SYMBOLIC: &str = "rmi.symbolic";
/// Neurosymbolic sub-namespace.
pub const NS_NEUROSYMBOLIC: &str = "rmi.neurosymbolic";
/// Compute sub-namespace.
pub const NS_COMPUTE: &str = "rmi.compute";
/// Core sub-namespace.
pub const NS_CORE: &str = "rmi.core";

// ── Public API ──────────────────────────────────────────────────────────────

/// The full self-describing ontology of the RMI framework.
///
/// Calling [`FrameworkOntology::build`] returns an [`Ontology`] populated with
/// every module, layer type, operation kind, composition rule, and constraint
/// that a foreign agent needs to reason about and use the framework.
pub struct FrameworkOntology;

impl FrameworkOntology {
    /// Build the complete framework introspection ontology.
    ///
    /// The returned [`Ontology`] contains ~100 concepts across six namespaces
    /// (`rmi`, `rmi.neural`, `rmi.symbolic`, `rmi.neurosymbolic`,
    /// `rmi.compute`, `rmi.core`) with full composition rules and constraints.
    pub fn build() -> Ontology {
        let ont = Ontology::new(NS_ROOT);

        // ── Top-level module concepts ────────────────────────────────────
        Self::add_module_hierarchy(&ont);

        // ── Neural primitives ────────────────────────────────────────────
        Self::add_neural_layers(&ont);
        Self::add_neural_activations(&ont);
        Self::add_neural_normalisations(&ont);
        Self::add_neural_attention(&ont);
        Self::add_neural_regularisation(&ont);
        Self::add_neural_positional(&ont);
        Self::add_neural_recurrent(&ont);
        Self::add_neural_pooling(&ont);

        // ── Symbolic operations ──────────────────────────────────────────
        Self::add_symbolic_ops(&ont);

        // ── Neurosymbolic bridges ────────────────────────────────────────
        Self::add_neurosymbolic_bridges(&ont);

        // ── Compute backends ─────────────────────────────────────────────
        Self::add_compute_backends(&ont);

        // ── Composition rules (edges) ────────────────────────────────────
        Self::add_composition_rules(&ont);

        ont
    }

    // ── helpers ──────────────────────────────────────────────────────────

    /// Convenience: create a concept with common introspection attributes.
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

    fn s(v: &str) -> AttributeValue {
        AttributeValue::String(v.to_string())
    }
    fn b(v: bool) -> AttributeValue {
        AttributeValue::Bool(v)
    }
    fn i(v: i64) -> AttributeValue {
        AttributeValue::Int(v)
    }
    fn _f(v: f64) -> AttributeValue {
        AttributeValue::Float(v)
    }
    fn list(v: &[&str]) -> AttributeValue {
        AttributeValue::List(
            v.iter()
                .map(|s| AttributeValue::String(s.to_string()))
                .collect(),
        )
    }

    fn relate(ont: &Ontology, src_ns: &str, src: &str, rel: RelationType, tgt_ns: &str, tgt: &str) {
        let mut r = Relation::new(
            ConceptId::new(src_ns, src),
            rel,
            ConceptId::new(tgt_ns, tgt),
        );
        // Structural relations are always bidirectional so agents can traverse
        // in either direction (e.g. PartOf ↔ HasComponent, IsA both ways).
        r.bidirectional = matches!(
            rel,
            RelationType::PartOf | RelationType::IsA | RelationType::HasComponent
        );
        ont.add_relation(r);
    }

    // ── Module hierarchy ─────────────────────────────────────────────────

    fn add_module_hierarchy(ont: &Ontology) {
        // Root framework concept
        ont.add_concept(Self::concept(
            NS_ROOT,
            "framework",
            ConceptType::Entity,
            "Recursive Machine Intelligence Framework",
            vec![
                ("version", Self::s("0.1.0")),
                (
                    "paradigms",
                    Self::list(&["neural", "symbolic", "neurosymbolic"]),
                ),
                ("serialisation", Self::s("MessagePack + LZ4")),
                (
                    "agent_protocol",
                    Self::s("binary, 32-byte header, xxh64 checksum"),
                ),
            ],
        ));

        // Top-level modules
        for (name, label, desc) in [
            (
                "module.neural",
                "Neural Module",
                "Deep learning primitives with autodiff",
            ),
            (
                "module.symbolic",
                "Symbolic Module",
                "First-order logic, unification, planning",
            ),
            (
                "module.neurosymbolic",
                "Neurosymbolic Module",
                "Hybrid reasoning bridges",
            ),
            (
                "module.compute",
                "Compute Module",
                "Backend abstraction (CPU/CUDA)",
            ),
            (
                "module.core",
                "Core Module",
                "Agents, protocol, ontology, storage, codegen",
            ),
            (
                "module.knowledge",
                "Knowledge Module",
                "AI history & concept knowledge bases",
            ),
        ] {
            ont.add_concept(Self::concept(
                NS_ROOT,
                name,
                ConceptType::Schema,
                label,
                vec![("description", Self::s(desc))],
            ));
            Self::relate(
                ont,
                NS_ROOT,
                name,
                RelationType::PartOf,
                NS_ROOT,
                "framework",
            );
        }
    }

    // ── Neural layers ────────────────────────────────────────────────────

    fn add_neural_layers(ont: &Ontology) {
        // Abstract parent
        ont.add_concept(Self::concept(
            NS_NEURAL,
            "layer",
            ConceptType::Schema,
            "Neural Layer",
            vec![
                ("kind", Self::s("abstract")),
                (
                    "api",
                    Self::s("Layer::forward(&[&Variable], &mut GradientTape) -> Variable"),
                ),
                ("differentiable", Self::b(true)),
            ],
        ));
        Self::relate(
            ont,
            NS_NEURAL,
            "layer",
            RelationType::PartOf,
            NS_ROOT,
            "module.neural",
        );

        // Concrete layers
        #[allow(clippy::type_complexity)]
        let layers: Vec<(&str, &str, Vec<(&str, AttributeValue)>)> = vec![
            (
                "linear",
                "Linear (Dense)",
                vec![
                    ("rust_type", Self::s("Linear")),
                    ("params", Self::list(&["in_features", "out_features"])),
                    (
                        "param_count_formula",
                        Self::s("in_features * out_features + out_features"),
                    ),
                    (
                        "flops_formula",
                        Self::s("2 * batch * seq * in_features * out_features"),
                    ),
                    ("differentiable", Self::b(true)),
                    ("hardware_affinity", Self::s("GPU preferred for large dims")),
                ],
            ),
            (
                "conv2d",
                "Conv2d",
                vec![
                    ("rust_type", Self::s("Conv2d")),
                    (
                        "params",
                        Self::list(&["in_channels", "out_channels", "kernel_size"]),
                    ),
                    (
                        "param_count_formula",
                        Self::s("in_ch * out_ch * k * k + out_ch"),
                    ),
                    ("differentiable", Self::b(true)),
                    ("spatial", Self::b(true)),
                    ("translation_equivariant", Self::b(true)),
                ],
            ),
            (
                "embedding",
                "Embedding",
                vec![
                    ("rust_type", Self::s("Embedding")),
                    ("params", Self::list(&["vocab_size", "embedding_dim"])),
                    ("param_count_formula", Self::s("vocab_size * embedding_dim")),
                    ("differentiable", Self::b(true)),
                    ("input_type", Self::s("integer indices")),
                    ("output_type", Self::s("dense vectors")),
                ],
            ),
        ];

        for (name, label, attrs) in layers {
            ont.add_concept(Self::concept(
                NS_NEURAL,
                name,
                ConceptType::Entity,
                label,
                attrs,
            ));
            Self::relate(ont, NS_NEURAL, name, RelationType::IsA, NS_NEURAL, "layer");
        }
    }

    // ── Activations ──────────────────────────────────────────────────────

    fn add_neural_activations(ont: &Ontology) {
        ont.add_concept(Self::concept(
            NS_NEURAL,
            "activation",
            ConceptType::Schema,
            "Activation Function",
            vec![
                ("kind", Self::s("abstract")),
                ("element_wise", Self::b(true)),
                ("differentiable", Self::b(true)),
                ("learnable_params", Self::i(0)),
            ],
        ));
        Self::relate(
            ont,
            NS_NEURAL,
            "activation",
            RelationType::PartOf,
            NS_ROOT,
            "module.neural",
        );

        for (name, label, smooth, recommended_for) in [
            ("relu", "ReLU", false, "general purpose, CNNs"),
            ("gelu", "GELU", true, "transformers, BERT, GPT"),
            ("silu", "SiLU / Swish", true, "EfficientNet, modern CNNs"),
            ("sigmoid", "Sigmoid", true, "binary output, gates"),
            ("tanh", "Tanh", true, "RNN hidden states, bounded output"),
            (
                "softmax",
                "Softmax",
                true,
                "probability distributions, attention",
            ),
            ("softplus", "Softplus", true, "positive output, smooth ReLU"),
            ("mish", "Mish", true, "image classification, smooth"),
        ] {
            ont.add_concept(Self::concept(
                NS_NEURAL,
                name,
                ConceptType::Process,
                label,
                vec![
                    (
                        "rust_type",
                        Self::s(&format!(
                            "NeuralPrimitiveKind::{}",
                            label.split_whitespace().next().unwrap_or(name)
                        )),
                    ),
                    ("smooth", Self::b(smooth)),
                    ("recommended_for", Self::s(recommended_for)),
                    ("learnable_params", Self::i(0)),
                    ("flops_per_element", Self::i(1)),
                ],
            ));
            Self::relate(
                ont,
                NS_NEURAL,
                name,
                RelationType::IsA,
                NS_NEURAL,
                "activation",
            );
        }
    }

    // ── Normalisations ───────────────────────────────────────────────────

    fn add_neural_normalisations(ont: &Ontology) {
        ont.add_concept(Self::concept(
            NS_NEURAL,
            "normalisation",
            ConceptType::Schema,
            "Normalisation Layer",
            vec![
                ("kind", Self::s("abstract")),
                ("differentiable", Self::b(true)),
                ("stabilises_training", Self::b(true)),
            ],
        ));
        Self::relate(
            ont,
            NS_NEURAL,
            "normalisation",
            RelationType::PartOf,
            NS_ROOT,
            "module.neural",
        );

        for (name, label, recommended) in [
            ("layer_norm", "LayerNorm", "transformers, RNNs"),
            ("batch_norm", "BatchNorm", "CNNs (large batch)"),
            ("group_norm", "GroupNorm", "CNNs (small batch)"),
            ("rms_norm", "RMSNorm", "modern transformers (LLaMA)"),
            ("instance_norm", "InstanceNorm", "style transfer"),
        ] {
            ont.add_concept(Self::concept(
                NS_NEURAL,
                name,
                ConceptType::Process,
                label,
                vec![
                    (
                        "rust_type",
                        Self::s(&format!(
                            "NeuralPrimitiveKind::{}",
                            label
                        )),
                    ),
                    ("recommended_for", Self::s(recommended)),
                    ("learnable_params_formula", Self::s("2 * normalised_dim")),
                ],
            ));
            Self::relate(
                ont,
                NS_NEURAL,
                name,
                RelationType::IsA,
                NS_NEURAL,
                "normalisation",
            );
        }
    }

    // ── Attention ────────────────────────────────────────────────────────

    fn add_neural_attention(ont: &Ontology) {
        ont.add_concept(Self::concept(
            NS_NEURAL,
            "attention",
            ConceptType::Schema,
            "Attention Mechanism",
            vec![
                ("kind", Self::s("abstract")),
                ("differentiable", Self::b(true)),
                ("params", Self::list(&["num_heads", "head_dim"])),
                ("captures_global_context", Self::b(true)),
            ],
        ));
        Self::relate(
            ont,
            NS_NEURAL,
            "attention",
            RelationType::PartOf,
            NS_ROOT,
            "module.neural",
        );

        for (name, label, complexity, notes) in [
            (
                "multi_head_attention",
                "MultiHeadAttention",
                "O(n^2 * d)",
                "Standard transformer attention",
            ),
            (
                "self_attention",
                "SelfAttention",
                "O(n^2 * d)",
                "Q=K=V from same input",
            ),
            (
                "cross_attention",
                "CrossAttention",
                "O(nm * d)",
                "Q from decoder, KV from encoder",
            ),
            (
                "linear_attention",
                "LinearAttention",
                "O(n * d^2)",
                "Kernel trick for linear complexity",
            ),
            (
                "flash_attention",
                "FlashAttention",
                "O(n^2 * d) time, O(n) memory",
                "IO-aware, memory efficient",
            ),
        ] {
            ont.add_concept(Self::concept(
                NS_NEURAL,
                name,
                ConceptType::Entity,
                label,
                vec![
                    ("complexity", Self::s(complexity)),
                    ("notes", Self::s(notes)),
                    ("differentiable", Self::b(true)),
                ],
            ));
            Self::relate(
                ont,
                NS_NEURAL,
                name,
                RelationType::IsA,
                NS_NEURAL,
                "attention",
            );
        }
    }

    // ── Regularisation ───────────────────────────────────────────────────

    fn add_neural_regularisation(ont: &Ontology) {
        ont.add_concept(Self::concept(
            NS_NEURAL,
            "regularisation",
            ConceptType::Schema,
            "Regularisation",
            vec![
                ("kind", Self::s("abstract")),
                ("prevents_overfitting", Self::b(true)),
            ],
        ));
        Self::relate(
            ont,
            NS_NEURAL,
            "regularisation",
            RelationType::PartOf,
            NS_ROOT,
            "module.neural",
        );

        for (name, label, typical_range) in [
            ("dropout", "Dropout", "0.1 – 0.5"),
            ("drop_path", "DropPath (Stochastic Depth)", "0.0 – 0.3"),
        ] {
            ont.add_concept(Self::concept(
                NS_NEURAL,
                name,
                ConceptType::Process,
                label,
                vec![
                    ("typical_rate_range", Self::s(typical_range)),
                    ("train_only", Self::b(true)),
                ],
            ));
            Self::relate(
                ont,
                NS_NEURAL,
                name,
                RelationType::IsA,
                NS_NEURAL,
                "regularisation",
            );
        }
    }

    // ── Positional encoding ──────────────────────────────────────────────

    fn add_neural_positional(ont: &Ontology) {
        ont.add_concept(Self::concept(
            NS_NEURAL,
            "positional_encoding",
            ConceptType::Schema,
            "Positional Encoding",
            vec![
                ("kind", Self::s("abstract")),
                ("purpose", Self::s("inject sequence position information")),
            ],
        ));
        Self::relate(
            ont,
            NS_NEURAL,
            "positional_encoding",
            RelationType::PartOf,
            NS_ROOT,
            "module.neural",
        );

        for (name, label, learnable, supports_extrapolation) in [
            (
                "sinusoidal_pe",
                "Sinusoidal Positional Encoding",
                false,
                true,
            ),
            ("rope", "Rotary Positional Encoding (RoPE)", false, true),
            ("learned_pe", "Learned Positional Encoding", true, false),
            ("alibi", "ALiBi", false, true),
        ] {
            ont.add_concept(Self::concept(
                NS_NEURAL,
                name,
                ConceptType::Process,
                label,
                vec![
                    ("learnable", Self::b(learnable)),
                    (
                        "supports_length_extrapolation",
                        Self::b(supports_extrapolation),
                    ),
                ],
            ));
            Self::relate(
                ont,
                NS_NEURAL,
                name,
                RelationType::IsA,
                NS_NEURAL,
                "positional_encoding",
            );
        }
    }

    // ── Recurrent ────────────────────────────────────────────────────────

    fn add_neural_recurrent(ont: &Ontology) {
        ont.add_concept(Self::concept(
            NS_NEURAL,
            "recurrent",
            ConceptType::Schema,
            "Recurrent Layer",
            vec![
                ("kind", Self::s("abstract")),
                ("handles_sequences", Self::b(true)),
                ("has_hidden_state", Self::b(true)),
            ],
        ));
        Self::relate(
            ont,
            NS_NEURAL,
            "recurrent",
            RelationType::PartOf,
            NS_ROOT,
            "module.neural",
        );

        for (name, label, gate_count) in [
            ("rnn_cell", "RNNCell", 0i64),
            ("lstm_cell", "LSTMCell", 3),
            ("gru_cell", "GRUCell", 2),
        ] {
            ont.add_concept(Self::concept(
                NS_NEURAL,
                name,
                ConceptType::Entity,
                label,
                vec![
                    ("gate_count", Self::i(gate_count)),
                    ("differentiable", Self::b(true)),
                ],
            ));
            Self::relate(
                ont,
                NS_NEURAL,
                name,
                RelationType::IsA,
                NS_NEURAL,
                "recurrent",
            );
        }
    }

    // ── Pooling ──────────────────────────────────────────────────────────

    fn add_neural_pooling(ont: &Ontology) {
        ont.add_concept(Self::concept(
            NS_NEURAL,
            "pooling",
            ConceptType::Schema,
            "Pooling Operation",
            vec![
                ("kind", Self::s("abstract")),
                ("reduces_spatial_dims", Self::b(true)),
            ],
        ));
        Self::relate(
            ont,
            NS_NEURAL,
            "pooling",
            RelationType::PartOf,
            NS_ROOT,
            "module.neural",
        );

        for (name, label) in [
            ("max_pool_2d", "MaxPool2d"),
            ("avg_pool_2d", "AvgPool2d"),
            ("adaptive_avg_pool", "AdaptiveAvgPool"),
            ("global_avg_pool", "GlobalAvgPool"),
        ] {
            ont.add_concept(Self::concept(
                NS_NEURAL,
                name,
                ConceptType::Process,
                label,
                vec![],
            ));
            Self::relate(
                ont,
                NS_NEURAL,
                name,
                RelationType::IsA,
                NS_NEURAL,
                "pooling",
            );
        }
    }

    // ── Symbolic ─────────────────────────────────────────────────────────

    fn add_symbolic_ops(ont: &Ontology) {
        #[allow(clippy::type_complexity)]
        let ops: Vec<(&str, &str, &str, Vec<(&str, AttributeValue)>)> = vec![
            (
                "first_order_logic",
                "First-Order Logic",
                "rmi.symbolic",
                vec![
                    ("rust_module", Self::s("symbolic::logic")),
                    (
                        "types",
                        Self::list(&["Term", "Predicate", "Clause", "Formula", "KnowledgeBase"]),
                    ),
                    ("supports_cnf", Self::b(true)),
                    ("supports_dnf", Self::b(true)),
                ],
            ),
            (
                "unification",
                "Unification Engine",
                "rmi.symbolic",
                vec![
                    ("rust_module", Self::s("symbolic::unification")),
                    ("algorithm", Self::s("Robinson with occurs check")),
                    ("supports_anti_unification", Self::b(true)),
                    ("complexity", Self::s("O(n) with union-find")),
                ],
            ),
            (
                "inference_engine",
                "Inference Engine",
                "rmi.symbolic",
                vec![
                    ("rust_module", Self::s("symbolic::inference")),
                    (
                        "strategies",
                        Self::list(&["forward_chaining", "backward_chaining", "resolution"]),
                    ),
                ],
            ),
            (
                "planner",
                "STRIPS Planner",
                "rmi.symbolic",
                vec![
                    ("rust_module", Self::s("symbolic::planner")),
                    ("formalism", Self::s("STRIPS")),
                    ("types", Self::list(&["Action", "Domain", "Plan"])),
                ],
            ),
        ];

        for (name, label, ns, attrs) in ops {
            ont.add_concept(Self::concept(ns, name, ConceptType::Entity, label, attrs));
            Self::relate(
                ont,
                ns,
                name,
                RelationType::PartOf,
                NS_ROOT,
                "module.symbolic",
            );
        }
    }

    // ── Neurosymbolic bridges ────────────────────────────────────────────

    fn add_neurosymbolic_bridges(ont: &Ontology) {
        #[allow(clippy::type_complexity)]
        let bridges: Vec<(&str, &str, Vec<(&str, AttributeValue)>)> = vec![
            (
                "symbol_embedder",
                "Symbol Embedder",
                vec![
                    ("rust_type", Self::s("SymbolEmbedder")),
                    ("direction", Self::s("symbol → vector")),
                    ("differentiable", Self::b(true)),
                    (
                        "enables",
                        Self::s("gradient flow through symbolic structures"),
                    ),
                ],
            ),
            (
                "constraint_solver",
                "Differentiable Constraint Solver",
                vec![
                    ("rust_type", Self::s("ConstraintSolver")),
                    ("direction", Self::s("logic → loss")),
                    ("differentiable", Self::b(true)),
                    (
                        "enables",
                        Self::s("enforce logical rules via gradient descent"),
                    ),
                ],
            ),
            (
                "hybrid_reasoner",
                "Hybrid Reasoner",
                vec![
                    ("rust_type", Self::s("HybridReasoner")),
                    ("direction", Self::s("bidirectional")),
                    (
                        "modes",
                        Self::list(&["neural_only", "symbolic_only", "hybrid", "adaptive"]),
                    ),
                    ("temperature_controlled", Self::b(true)),
                    (
                        "enables",
                        Self::s("seamless switching between neural and symbolic reasoning"),
                    ),
                ],
            ),
        ];

        for (name, label, attrs) in bridges {
            ont.add_concept(Self::concept(
                NS_NEUROSYMBOLIC,
                name,
                ConceptType::Entity,
                label,
                attrs,
            ));
            Self::relate(
                ont,
                NS_NEUROSYMBOLIC,
                name,
                RelationType::PartOf,
                NS_ROOT,
                "module.neurosymbolic",
            );
        }
    }

    // ── Compute backends ─────────────────────────────────────────────────

    fn add_compute_backends(ont: &Ontology) {
        ont.add_concept(Self::concept(
            NS_COMPUTE,
            "backend",
            ConceptType::Schema,
            "Compute Backend",
            vec![
                ("kind", Self::s("abstract")),
                ("trait", Self::s("Backend")),
                (
                    "operations",
                    Self::list(&[
                        // memory / creation
                        "allocate", "free", "copy_to_device", "copy_to_host", "copy",
                        "zeros", "ones", "rand", "randn", "from_slice_f32",
                        // arithmetic
                        "add", "sub", "mul", "div", "matmul", "scale",
                        // quantized matmul family (INT8/INT4; exact F32 fallback)
                        "quantized_matmul", "quantized_matmul_calibrated",
                        "quantized_matmul_asym_calibrated", "quantized_matmul_w4_calibrated",
                        // dtype conversion
                        "cast",
                        // reductions
                        "sum", "sum_axis", "mean", "mean_axis", "max", "min",
                        // activations
                        "relu", "gelu", "sigmoid", "tanh", "softmax",
                        // convolution (stride / padding / dilation)
                        "conv2d",
                        // shape
                        "reshape", "transpose", "concat", "split",
                        // sync
                        "synchronize",
                    ]),
                ),
                (
                    "dtypes",
                    Self::list(&["F32", "F64", "F16", "BF16", "I32", "I64", "I8", "I4", "U8", "Bool"]),
                ),
            ],
        ));
        Self::relate(
            ont,
            NS_COMPUTE,
            "backend",
            RelationType::PartOf,
            NS_ROOT,
            "module.compute",
        );

        ont.add_concept(Self::concept(
            NS_COMPUTE,
            "cpu_backend",
            ConceptType::Entity,
            "CPU Backend",
            vec![
                ("rust_type", Self::s("CpuBackend")),
                ("libraries", Self::list(&["ndarray", "rayon", "half"])),
                ("simd", Self::b(true)),
                ("always_available", Self::b(true)),
                (
                    "quantization",
                    Self::list(&["int8_symmetric", "int8_asymmetric_calibrated"]),
                ),
                ("cast_dtypes", Self::list(&["F32", "F16", "BF16", "F64"])),
            ],
        ));
        Self::relate(
            ont,
            NS_COMPUTE,
            "cpu_backend",
            RelationType::IsA,
            NS_COMPUTE,
            "backend",
        );

        // NOTE: the production CUDA path lives in the MAGE prototype's
        // CudaBackend (IronAccelerator / CUDA 13.2), NOT the legacy
        // cudarc-0.10 module in this crate. The ontology describes the
        // production surface so agents discover the real capabilities.
        ont.add_concept(Self::concept(
            NS_COMPUTE,
            "cuda_backend",
            ConceptType::Entity,
            "CUDA Backend",
            vec![
                ("rust_type", Self::s("CudaBackend")),
                (
                    "libraries",
                    Self::list(&["ironaccelerator-cuda", "cuBLASLt", "NVRTC", "cuRAND"]),
                ),
                ("feature_gated", Self::b(true)),
                ("feature_flag", Self::s("cuda")),
                ("cuda_version", Self::s("13.2")),
                (
                    "acceleration",
                    Self::list(&[
                        "f32_cublaslt_matmul",
                        "f16_bf16_tensor_core_matmul",
                        "int8_imma_tensor_core_gemm",
                        "nvrtc_elementwise_activations_reductions",
                        "gpu_resident_storage",
                        "im2col_gemm_conv2d",
                    ]),
                ),
                (
                    "quantization",
                    Self::list(&[
                        "int8_per_tensor",
                        "int8_per_channel",
                        "int8_asymmetric_zero_point",
                        "int4_packed_weights",
                        "int4_packed_activations",
                        "calibrated_static_scales",
                        "weight_cache",
                    ]),
                ),
            ],
        ));
        Self::relate(
            ont,
            NS_COMPUTE,
            "cuda_backend",
            RelationType::IsA,
            NS_COMPUTE,
            "backend",
        );

        // Quantization as a first-class discoverable concept: the modes
        // and calibration methods an agent can request via the pipeline.
        ont.add_concept(Self::concept(
            NS_COMPUTE,
            "quantization",
            ConceptType::Schema,
            "Quantization (PTQ)",
            vec![
                ("modes", Self::list(&["off", "dynamic", "calibrate", "calibrated"])),
                (
                    "calibration_methods",
                    Self::list(&["max", "percentile", "entropy_kl"]),
                ),
                ("schemes", Self::list(&["symmetric", "asymmetric_zero_point"])),
                ("granularity", Self::list(&["per_tensor", "per_channel"])),
                ("weight_bits", Self::list(&["8", "4"])),
                ("accumulator", Self::s("int32")),
                ("fallback", Self::s("exact_f32_on_unsupported")),
            ],
        ));
        Self::relate(
            ont,
            NS_COMPUTE,
            "quantization",
            RelationType::PartOf,
            NS_COMPUTE,
            "backend",
        );
    }

    // ── Composition rules ────────────────────────────────────────────────

    fn add_composition_rules(ont: &Ontology) {
        // Neural composition patterns: what canonically follows what.
        //   layer → activation, layer → normalisation
        //   normalisation → activation, activation → layer
        //   attention feeds into normalisation (pre-norm) or residual
        let neural_compositions: Vec<(&str, &str, RelationType)> = vec![
            // Linear → Activation
            ("linear", "relu", RelationType::Enables),
            ("linear", "gelu", RelationType::Enables),
            ("linear", "silu", RelationType::Enables),
            // Linear → Normalisation
            ("linear", "layer_norm", RelationType::Enables),
            ("linear", "batch_norm", RelationType::Enables),
            // Conv2d → Activation / Norm
            ("conv2d", "relu", RelationType::Enables),
            ("conv2d", "batch_norm", RelationType::Enables),
            ("conv2d", "group_norm", RelationType::Enables),
            // Normalisation → Attention (pre-norm pattern)
            ("layer_norm", "multi_head_attention", RelationType::Enables),
            ("rms_norm", "multi_head_attention", RelationType::Enables),
            // Attention → Normalisation (post-attn)
            ("multi_head_attention", "layer_norm", RelationType::Enables),
            // Activation → Linear (MLP second layer)
            ("gelu", "linear", RelationType::Enables),
            ("relu", "linear", RelationType::Enables),
            ("silu", "linear", RelationType::Enables),
            // Dropout after any layer
            ("linear", "dropout", RelationType::Enables),
            ("multi_head_attention", "dropout", RelationType::Enables),
            // Embedding feeds into positional + attention
            ("embedding", "sinusoidal_pe", RelationType::Enables),
            ("embedding", "rope", RelationType::Enables),
            ("embedding", "learned_pe", RelationType::Enables),
            // Symbolic → Neurosymbolic
            (
                "first_order_logic",
                "symbol_embedder",
                RelationType::Enables,
            ),
            ("inference_engine", "hybrid_reasoner", RelationType::Enables),
            // Neurosymbolic → Neural
            ("symbol_embedder", "linear", RelationType::Enables),
            ("hybrid_reasoner", "linear", RelationType::Enables),
            ("constraint_solver", "linear", RelationType::Enables),
        ];

        for (src, tgt, rel) in neural_compositions {
            // Source could be in any sub-namespace; resolve by trying known ones.
            let src_ns = Self::resolve_ns(src);
            let tgt_ns = Self::resolve_ns(tgt);
            Self::relate(ont, src_ns, src, rel, tgt_ns, tgt);
        }
    }

    /// Resolve a concept name to its namespace.
    fn resolve_ns(name: &str) -> &'static str {
        match name {
            "symbol_embedder" | "constraint_solver" | "hybrid_reasoner" => NS_NEUROSYMBOLIC,
            "first_order_logic" | "unification" | "inference_engine" | "planner" => NS_SYMBOLIC,
            "cpu_backend" | "cuda_backend" | "backend" => NS_COMPUTE,
            _ => NS_NEURAL,
        }
    }
}

// ── Query helpers for agents ─────────────────────────────────────────────────

/// Extension trait that adds introspection-specific queries to an [`Ontology`].
pub trait IntrospectionQueries {
    /// List every concept that is composable (has at least one `Enables` relation).
    fn composable_components(&self) -> Vec<Concept>;

    /// Given a component name, return the concepts it can feed into.
    fn can_feed_into(&self, component: &str) -> Vec<Concept>;

    /// Given a component name, return the concepts that can feed into it.
    fn can_receive_from(&self, component: &str) -> Vec<Concept>;

    /// List all concepts in a given sub-namespace.
    fn in_namespace(&self, ns: &str) -> Vec<Concept>;

    /// Get all differentiable components.
    fn differentiable_components(&self) -> Vec<Concept>;
}

impl IntrospectionQueries for Ontology {
    fn composable_components(&self) -> Vec<Concept> {
        use crate::core::ontology::OntologyQuery;
        // Return concepts that have at least one outgoing Enables relation
        let all = self.query(&OntologyQuery::new());
        all.into_iter()
            .filter(|c| !self.get_related(&c.id, RelationType::Enables).is_empty())
            .collect()
    }

    fn can_feed_into(&self, component: &str) -> Vec<Concept> {
        // The component is a *target* of Enables edges; we need concepts
        // whose Enables edges point to it.  Because Ontology stores directed
        // edges source→target, we search all concepts for Enables targets
        // matching `component`.
        use crate::core::ontology::OntologyQuery;
        let all = self.query(&OntologyQuery::new());
        all.into_iter()
            .filter(|c| {
                self.get_related(&c.id, RelationType::Enables)
                    .iter()
                    .any(|t| t.id.local_name == component)
            })
            .collect()
    }

    fn can_receive_from(&self, component: &str) -> Vec<Concept> {
        // Find the concept, then follow its Enables edges.
        if let Some(c) = self.lookup(component) {
            self.get_related(&c.id, RelationType::Enables)
        } else {
            Vec::new()
        }
    }

    fn in_namespace(&self, ns: &str) -> Vec<Concept> {
        use crate::core::ontology::OntologyQuery;
        let all = self.query(&OntologyQuery::new());
        all.into_iter().filter(|c| c.id.namespace == ns).collect()
    }

    fn differentiable_components(&self) -> Vec<Concept> {
        use crate::core::ontology::OntologyQuery;
        let all = self.query(&OntologyQuery::new());
        all.into_iter()
            .filter(|c| {
                matches!(
                    c.attributes.get("differentiable"),
                    Some(AttributeValue::Bool(true))
                )
            })
            .collect()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_framework_ontology_non_empty() {
        let ont = FrameworkOntology::build();
        // Should contain the framework root + modules + neural + symbolic + …
        assert!(ont.len() > 40, "Expected > 40 concepts, got {}", ont.len());
    }

    #[test]
    fn modules_are_part_of_framework() {
        let ont = FrameworkOntology::build();
        let fw_id = ConceptId::new(NS_ROOT, "framework");
        // Every top-level module should be reachable via PartOf→framework
        // (we check the inverse: framework should have HasComponent edges to modules)
        // Because Ontology stores bidirectional PartOf ↔ HasComponent.
        let components = ont.get_related(&fw_id, RelationType::HasComponent);
        assert!(
            components.len() >= 6,
            "Framework should have >= 6 module components, got {}",
            components.len()
        );
    }

    #[test]
    fn neural_layers_are_composable() {
        let ont = FrameworkOntology::build();
        let linear_id = ConceptId::new(NS_NEURAL, "linear");
        let targets = ont.get_related(&linear_id, RelationType::Enables);
        assert!(
            targets.len() >= 3,
            "Linear should enable >= 3 downstream ops, got {}",
            targets.len()
        );
    }

    #[test]
    fn symbolic_bridges_to_neural() {
        let ont = FrameworkOntology::build();
        let sym_emb_id = ConceptId::new(NS_NEUROSYMBOLIC, "symbol_embedder");
        let targets = ont.get_related(&sym_emb_id, RelationType::Enables);
        assert!(
            !targets.is_empty(),
            "SymbolEmbedder should enable at least one neural op"
        );
    }

    #[test]
    fn differentiable_query_works() {
        let ont = FrameworkOntology::build();
        let diff = ont.differentiable_components();
        assert!(
            diff.len() > 10,
            "Should have > 10 differentiable components, got {}",
            diff.len()
        );
    }

    #[test]
    fn introspection_namespace_filter() {
        let ont = FrameworkOntology::build();
        let neural = ont.in_namespace(NS_NEURAL);
        assert!(
            neural.len() > 15,
            "Neural namespace should have > 15 concepts"
        );
        let symbolic = ont.in_namespace(NS_SYMBOLIC);
        assert!(
            !symbolic.is_empty(),
            "Symbolic namespace should not be empty"
        );
    }
}
