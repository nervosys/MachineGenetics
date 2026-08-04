//! # `mage-prototype` — the MAGE compiler, evaluator, and agent runtime
//!
//! This crate is a **library first and a binary second**. The modules below are
//! the language's reference surface: the full AST/HIR, the ontology's operation
//! catalogue, and the subsystems an agent drives over RAP (leases, CRDTs,
//! certificates, sandbox, hot-reload, swarm).
//!
//! Much of that surface has no in-tree caller — it exists so `--build=schema`
//! and `MAGE_ONTOLOGY.json` can describe it, and so agents can call it. When it
//! all lived inside `main.rs` that made ~85 items look like dead code, which was
//! silenced with a crate-wide `allow(dead_code)` and a comment promising this
//! refactor. Declared `pub` from a library, the same items are simply public
//! API, so visibility carries the meaning and the lint stays on to catch things
//! that are genuinely unreachable.
//!
//! The `mage-parse` binary is a thin CLI over this library.
pub mod aci;
pub mod agent_runtime;
pub mod ast;
pub mod autograd;
pub mod backends;
pub mod bench;
pub mod builder;
#[cfg(feature = "cuda")]
pub mod cuda_backend;
pub mod certs;
pub mod cli_manifest;
pub mod codegen_bridge;
pub mod consensus;
pub mod cost;
pub mod cost_calibration;
pub mod crdt;
pub mod decompose;
pub mod effects;
pub mod eval;
pub mod elision;
pub mod evolve_gen;
pub mod ffi_gen;
pub mod fmt;
#[cfg(test)]
pub mod fuzz;
pub mod forge;
pub mod grammar;
pub mod heal;
pub mod hir;
pub mod hot_reload;
pub mod lease;
pub mod legacy;
pub mod lexer;
pub mod logic;
pub mod manifest;
pub mod mlir;
pub mod nl_engine;
pub mod ontology;
pub mod parser;
pub mod perf_annot;
pub mod perf_measure;
pub mod rain;
pub mod rap;
pub mod recover;
pub mod resolve;
pub mod rmi_ontology_adapter;
pub mod rmi_runtime_adapter;
pub mod abl;
pub mod abl_bridge;
pub mod abl_compute;
pub mod abl_shape;
pub mod sandbox;
pub mod semantic_vcs;
pub mod shape;
pub mod spine_bridge;
pub mod skb;
pub mod stdlib_ext;
pub mod swarm_bus;
pub mod swarm_sdk;
pub mod synthesis;
pub mod token_budget;
pub mod token_canonical;
pub mod types;
pub mod verify;

