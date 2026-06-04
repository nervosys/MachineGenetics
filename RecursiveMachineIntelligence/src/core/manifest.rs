//! Token-compact progressive-disclosure manifest of the framework itself.
//!
//! Agentic-first discoverability means an agent's *first* contact with the
//! framework must be cheap: a root index costing a few hundred tokens, with
//! every entry expandable on demand. Building the full
//! [`FrameworkOntology`](crate::core::introspection::FrameworkOntology) gives
//! deep structure (relations, composition rules) but costs far more tokens
//! than a session that only needs "what can this do and where do I start?".
//!
//! This module is the cheap front door:
//!
//! - [`manifest`] — a deterministic, compact root index (namespaces, axes of
//!   capability, entry points). Read this first.
//! - [`describe`] — expand any manifest entry by name for the next level of
//!   detail (key types, functions, feature flags, safety notes).
//!
//! Both are plain `&'static`-backed strings: zero allocation surprises, no
//! map-iteration nondeterminism, and no need to construct the ontology.
//!
//! ```
//! let root = rmi::core::manifest::manifest();
//! assert!(root.contains("compute:") && root.contains("RecursiveMachineIntelligence"));
//! let compute = rmi::core::manifest::describe("compute").unwrap();
//! assert!(compute.contains("quantized_matmul"));
//! assert!(rmi::core::manifest::describe("nope").is_none());
//! ```

/// Crate version baked into the manifest so it can never drift from the build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One expandable manifest entry.
#[derive(Debug, Clone, Copy)]
struct Entry {
    name: &'static str,
    summary: &'static str,
    detail: &'static str,
}

/// Static entry table — fixed order, deterministic output.
const ENTRIES: &[Entry] = &[
    Entry {
        name: "compute",
        summary: "tensor Backend trait (CPU/CUDA): full op set, dtypes F32..I4, quantization, cast",
        detail: "namespace rmi.compute\n\
                 trait Backend (~40 ops): allocate/free/copy*, zeros/ones/rand/randn/from_slice_f32,\n\
                 add/sub/mul/div/matmul/scale, quantized_matmul{,_calibrated,_asym_calibrated,_w4_calibrated},\n\
                 cast, sum/sum_axis/mean/mean_axis/max/min, relu/gelu/sigmoid/tanh/softmax,\n\
                 conv2d(stride,padding,dilation), reshape/transpose/concat/split, synchronize.\n\
                 dtypes: F32 F64 F16 BF16 I32 I64 I8 I4(packed) U8 Bool.\n\
                 CpuBackend: always available; real INT8 (sym+asym) quantized_matmul; cast F32<->F16/BF16/F64.\n\
                 CudaBackend (production, in MechGen prototype via IronAccelerator, feature `cuda`):\n\
                 cuBLASLt F32 + tensor-core F16/BF16 + INT8 IMMA GEMM, NVRTC kernels, GPU-resident storage,\n\
                 quantization: per-tensor/per-channel, sym/asym zero-point, INT4 packed weights+acts,\n\
                 calibrated static scales (max/percentile/entropy), cached quantized weights.\n\
                 entry: rmi::compute::{Backend, CpuBackend, TensorHandle, DType, get_backend}",
    },
    Entry {
        name: "neural",
        summary: "differentiable layers, activations, attention, architectures",
        detail: "namespace rmi.neural\n\
                 layers: linear, conv2d, embedding, recurrent (RNN/LSTM/GRU), attention (MHA/GQA/flash),\n\
                 norm (layer/rms/batch/group), dropout, state_space (Mamba/S4), MoE, adapters (LoRA).\n\
                 architectures: transformer, cnn, sequence, generative (VAE/GAN/diffusion), graph_nets.\n\
                 composition rules live in the ontology (what Enables what).\n\
                 entry: rmi::neural; ontology: FrameworkOntology::build().in_namespace(\"rmi.neural\")",
    },
    Entry {
        name: "symbolic",
        summary: "logic, knowledge bases, planning, constraint/SAT/SMT solvers",
        detail: "namespace rmi.symbolic\n\
                 modules: logic (FOL/propositional), knowledge (KB/KG/triples), planning,\n\
                 probabilistic (Bayes nets), solvers (constraint/SAT/SMT).\n\
                 entry: rmi::symbolic",
    },
    Entry {
        name: "neurosymbolic",
        summary: "neural+symbolic bridges: differentiable logic, verification, reasoning",
        detail: "namespace rmi.neurosymbolic\n\
                 modules: differentiable_logic (soft rules), verification (provable safety),\n\
                 reasoning (concept bottlenecks).\n\
                 entry: rmi::neurosymbolic",
    },
    Entry {
        name: "core",
        summary: "agents, ontology, protocol, codegen/IR, optimization, introspection, manifest",
        detail: "namespace rmi.core\n\
                 agent: Agent/AgentBuilder/AgentCapability; collaboration: AgentRuntime/SharedWorkspace;\n\
                 ontology: Ontology/Concept/RelationType (machine-readable KR);\n\
                 protocol: binary-first inter-agent messages; storage; message_bus;\n\
                 codegen: Program/IR + emitters (CUDA/MLIR/ONNX); optimization: O0-O3 pipeline;\n\
                 verification: static IR checks; introspection: FrameworkOntology::build() full graph;\n\
                 discoverability: ComponentCatalog::search/for_task, ArchitectureRecipes;\n\
                 manifest (this module): cheap root index + describe().\n\
                 entry: rmi::core::*",
    },
    Entry {
        name: "lang",
        summary: "RMIL expression IR: Expr/Op/Val, codec, pattern matching, debugger",
        detail: "namespace rmi.lang\n\
                 Expr (Seq/Par/App/Let/Call/Lit), ~80 Ops (LINEAR, CONV2D, ATTN, RELU, SOFTMAX, ...),\n\
                 binary codec (RMIB container), pattern_match (Pat/Tmpl rewriting), debugger.\n\
                 Used by MechGen's rmil_compute pipeline: run_pipeline{,_with_precision,_quantized,_calibrated}.\n\
                 entry: rmi::lang::{Expr, Op, Val}",
    },
    Entry {
        name: "distributed",
        summary: "multi-node agents: transport, discovery, consensus, federation",
        detail: "namespace rmi.distributed\n\
                 modules: transport, discovery, consensus, federation.\n\
                 entry: rmi::distributed",
    },
    Entry {
        name: "evolution",
        summary: "self-improvement: meta-learning, self-modification, populations",
        detail: "namespace rmi.evolution\n\
                 modules: meta-learning, self-modification, population search.\n\
                 entry: rmi::evolution",
    },
    Entry {
        name: "knowledge",
        summary: "AI knowledge base: history, concepts",
        detail: "namespace rmi.knowledge\n\
                 curated AI/ML concept and history KB for agent grounding.\n\
                 entry: rmi::knowledge",
    },
    Entry {
        name: "runtime",
        summary: "execution runtime for agent programs",
        detail: "namespace rmi.runtime\n\
                 program execution host; pairs with core::codegen output.\n\
                 entry: rmi::runtime",
    },
    Entry {
        name: "safety",
        summary: "agent-facing safety posture: typed errors, F32 fallbacks, feature gating, effect map",
        detail: "design guarantees relevant to autonomous use:\n\
                 - every Backend op returns Result<_, RmiError> (typed; agent_diagnostic() emits\n\
                   `error code=.. recoverable=.. message=.. fix=..` — machine-parseable self-correction)\n\
                 - quantized/half paths fall back to exact F32 on unsupported dims/dtypes (never wrong-silently)\n\
                 - CUDA is feature-gated + driver-checked at construction (absent driver = clean Err, not crash)\n\
                 - ontology/manifest/query output are deterministic (id-sorted; no map-iteration leakage)\n\
                 - verification module statically checks generated IR before execution\n\
                 effect map (for external policy gating, agentic-eval Effect taxonomy):\n\
                 - compute/neural/symbolic/lang: pure (in-process tensors only; no ambient I/O)\n\
                 - core::storage: write_local | distributed::transport: network\n\
                 - evolution::self_modification: EXEC-equivalent — patches run through SandboxLimits\n\
                   with ResourceUsage checks; gate behind approval in agent deployments\n\
                 - core::codegen emitters: write_local (emit files); runtime execution of generated\n\
                   code: exec — verify (core::verification) before running",
    },
];

/// Token-compact root index. Read this first; expand entries with
/// [`describe`]. Deterministic: fixed entry order, no map iteration.
pub fn manifest() -> String {
    let mut s = String::with_capacity(1024);
    s.push_str("rmi v");
    s.push_str(VERSION);
    s.push_str(" — RecursiveMachineIntelligence, the built-in agentic-first AI framework of MachineGenetics (MechGen), NERVOSYS. namespaces:\n");
    for e in ENTRIES {
        s.push_str("  ");
        s.push_str(e.name);
        s.push_str(": ");
        s.push_str(e.summary);
        s.push('\n');
    }
    s.push_str("expand: rmi::core::manifest::describe(name). full graph: FrameworkOntology::build().\n");
    s.push_str("search: ComponentCatalog::build().search(query) / .for_task(task).\n");
    s
}

/// Expand one manifest entry by name. Returns `None` for unknown names —
/// callers should surface [`manifest`] (the index) on miss.
pub fn describe(name: &str) -> Option<String> {
    ENTRIES
        .iter()
        .find(|e| e.name == name)
        .map(|e| format!("{} — {}\n{}", e.name, e.summary, e.detail))
}

/// Names of every expandable entry, in manifest order (for tooling).
pub fn entry_names() -> Vec<&'static str> {
    ENTRIES.iter().map(|e| e.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_compact_and_deterministic() {
        let m1 = manifest();
        let m2 = manifest();
        assert_eq!(m1, m2, "manifest must be byte-stable");
        // Token-efficiency budget: a root index should stay well under
        // ~1000 tokens (~4 chars/token heuristic → ~4 KB).
        assert!(m1.len() < 4096, "manifest too large: {} bytes", m1.len());
        assert!(m1.contains("rmi.compute") || m1.contains("compute:"));
        assert!(m1.contains("describe"));
    }

    #[test]
    fn every_entry_describes() {
        for name in entry_names() {
            let d = describe(name).unwrap_or_else(|| panic!("describe({name}) missing"));
            assert!(d.len() > 40, "{name} detail too thin");
        }
        assert!(describe("definitely_not_an_entry").is_none());
    }

    #[test]
    fn compute_entry_covers_quantization_surface() {
        let c = describe("compute").unwrap();
        for needle in [
            "quantized_matmul",
            "cast",
            "conv2d",
            "I4",
            "IMMA",
            "calibrated",
            "entropy",
        ] {
            assert!(c.contains(needle), "compute detail missing `{needle}`");
        }
    }

    #[test]
    fn safety_entry_documents_fallback_guarantees() {
        let s = describe("safety").unwrap();
        assert!(s.contains("Result"));
        assert!(s.contains("F32"));
        assert!(s.contains("feature-gated"));
        // The effect map names the dangerous surfaces and their gates.
        assert!(s.contains("effect map"));
        assert!(s.contains("self_modification"));
        assert!(s.contains("agent_diagnostic"));
    }
}
