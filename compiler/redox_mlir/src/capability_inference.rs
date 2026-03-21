//! # Capability Inference Pass
//!
//! Analyzes MIR function bodies to determine what capabilities (effects) each
//! function requires: `alloc`, `io`, `panic`, `unsafe`, `async`, `ffi`.
//!
//! This is a forward dataflow-style analysis: scan statements and terminators
//! for patterns that indicate capability usage, then aggregate into a set.
//!
//! Reference: REDOX_PROPOSAL.md §4 (Compile-Time Safety — Effect System)

use crate::mir_to_mlir::{BinOp, MirBody, Operand, Projection, Rvalue, Statement, Terminator};
use std::collections::BTreeSet;
use std::fmt;

// ===========================================================================
// Capabilities
// ===========================================================================

/// A capability (effect) that a function body may require.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Heap allocation (`Box::new`, `Vec::new`, etc.).
    Alloc,
    /// I/O operations (filesystem, network, stdout).
    Io,
    /// Can panic (unwrap, expect, assert, index bounds, division).
    Panic,
    /// Uses unsafe operations (raw pointer deref, transmute).
    Unsafe,
    /// Async suspension points.
    Async,
    /// Foreign function interface calls.
    Ffi,
}

impl Capability {
    pub fn name(self) -> &'static str {
        match self {
            Self::Alloc => "alloc",
            Self::Io => "io",
            Self::Panic => "panic",
            Self::Unsafe => "unsafe",
            Self::Async => "async",
            Self::Ffi => "ffi",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ===========================================================================
// Inference result
// ===========================================================================

/// The result of capability inference for a single function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferredCapabilities {
    pub function_name: String,
    pub capabilities: BTreeSet<Capability>,
    /// Explanations for why each capability was inferred.
    pub evidence: Vec<CapabilityEvidence>,
}

/// Evidence for a single capability inference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEvidence {
    pub capability: Capability,
    pub reason: String,
    /// Block index where the evidence was found.
    pub block: usize,
}

impl InferredCapabilities {
    pub fn has(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    pub fn is_pure(&self) -> bool {
        self.capabilities.is_empty()
    }

    pub fn count(&self) -> usize {
        self.capabilities.len()
    }
}

// ===========================================================================
// Pattern matchers  — function name classification
// ===========================================================================

/// Known heap-allocating function patterns.
fn is_alloc_fn(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("box::new")
        || lower.contains("vec::new")
        || lower.contains("vec::with_capacity")
        || lower.contains("string::new")
        || lower.contains("string::from")
        || lower.contains("hashmap::new")
        || lower.contains("btreemap::new")
        || lower.contains("rc::new")
        || lower.contains("arc::new")
        || lower.contains("alloc::alloc")
        || lower.contains("__rust_alloc")
        || lower.contains("heap::allocate")
}

/// Known I/O function patterns.
fn is_io_fn(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("std::io::")
        || lower.starts_with("std::fs::")
        || lower.starts_with("std::net::")
        || lower.contains("println")
        || lower.contains("print!")
        || lower.contains("eprintln")
        || lower.contains("stdin")
        || lower.contains("stdout")
        || lower.contains("stderr")
        || lower.contains("file::open")
        || lower.contains("file::create")
        || lower.contains("tcpstream")
        || lower.contains("udpsocket")
}

/// Known panic-related function patterns.
fn is_panic_fn(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("::unwrap")
        || lower.contains("::expect")
        || lower == "panic"
        || lower.contains("core::panicking::panic")
        || lower.contains("std::panicking::begin_panic")
        || lower.contains("assert!")
        || lower.contains("assert_eq!")
        || lower.contains("assert_ne!")
        || lower.contains("unreachable!")
        || lower.contains("todo!")
        || lower.contains("unimplemented!")
}

/// Known async-related function patterns.
fn is_async_fn(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("::poll")
        || lower.contains("::await")
        || lower.contains("future::join")
        || lower.contains("tokio::")
        || lower.contains("async_std::")
        || lower.contains("spawn_blocking")
        || lower.contains("spawn_local")
}

/// Known FFI function patterns.
fn is_ffi_fn(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("extern ")
        || lower.starts_with("ffi::")
        || lower.contains("libc::")
        || lower.contains("winapi::")
        || lower.contains("c_str")
        || lower.contains("dlsym")
        || lower.contains("dlopen")
}

// ===========================================================================
// Inference engine
// ===========================================================================

/// Infer capabilities from a MIR function body.
pub fn infer_capabilities(body: &MirBody) -> InferredCapabilities {
    let mut caps = BTreeSet::new();
    let mut evidence = Vec::new();

    for (block_idx, block) in body.blocks.iter().enumerate() {
        // Analyze statements.
        for stmt in &block.statements {
            match stmt {
                Statement::Assign(place, rvalue) => {
                    analyze_rvalue(rvalue, block_idx, &mut caps, &mut evidence);
                    // Raw pointer deref in the destination.
                    if has_deref_projection(place) {
                        add_cap(
                            Capability::Unsafe,
                            format!("raw pointer write to {place}"),
                            block_idx,
                            &mut caps,
                            &mut evidence,
                        );
                    }
                }
                Statement::StorageLive(_) | Statement::StorageDead(_) | Statement::Nop => {}
            }
        }

        // Analyze terminator.
        match &block.terminator {
            Terminator::Call { func, args, .. } => {
                analyze_call(func, args, block_idx, &mut caps, &mut evidence);
            }
            Terminator::Drop { .. } => {
                // Drop itself doesn't require a capability — it's part of ownership.
            }
            Terminator::Unreachable => {
                add_cap(
                    Capability::Panic,
                    "unreachable terminator".into(),
                    block_idx,
                    &mut caps,
                    &mut evidence,
                );
            }
            Terminator::Goto { .. } | Terminator::SwitchInt { .. } | Terminator::Return => {}
        }
    }

    InferredCapabilities { function_name: body.name.clone(), capabilities: caps, evidence }
}

fn analyze_call(
    func: &str,
    _args: &[Operand],
    block_idx: usize,
    caps: &mut BTreeSet<Capability>,
    evidence: &mut Vec<CapabilityEvidence>,
) {
    if is_alloc_fn(func) {
        add_cap(Capability::Alloc, format!("call to {func}"), block_idx, caps, evidence);
    }
    if is_io_fn(func) {
        add_cap(Capability::Io, format!("call to {func}"), block_idx, caps, evidence);
    }
    if is_panic_fn(func) {
        add_cap(Capability::Panic, format!("call to {func}"), block_idx, caps, evidence);
    }
    if is_async_fn(func) {
        add_cap(Capability::Async, format!("call to {func}"), block_idx, caps, evidence);
    }
    if is_ffi_fn(func) {
        add_cap(Capability::Ffi, format!("call to {func}"), block_idx, caps, evidence);
    }
}

fn analyze_rvalue(
    rvalue: &Rvalue,
    block_idx: usize,
    caps: &mut BTreeSet<Capability>,
    evidence: &mut Vec<CapabilityEvidence>,
) {
    match rvalue {
        Rvalue::Use(Operand::Copy(place)) | Rvalue::Use(Operand::Move(place)) => {
            if has_deref_projection(place) {
                add_cap(
                    Capability::Unsafe,
                    format!("pointer deref read from {place}"),
                    block_idx,
                    caps,
                    evidence,
                );
            }
        }
        Rvalue::Ref { place, .. } => {
            if has_deref_projection(place) {
                add_cap(
                    Capability::Unsafe,
                    format!("ref through raw pointer {place}"),
                    block_idx,
                    caps,
                    evidence,
                );
            }
        }
        Rvalue::BinaryOp(BinOp::Div, _, _) | Rvalue::BinaryOp(BinOp::Rem, _, _) => {
            add_cap(
                Capability::Panic,
                "division/remainder may panic on zero".into(),
                block_idx,
                caps,
                evidence,
            );
        }
        _ => {}
    }
}

fn has_deref_projection(place: &crate::mir_to_mlir::Place) -> bool {
    place.projections.iter().any(|p| matches!(p, Projection::Deref))
}

fn add_cap(
    cap: Capability,
    reason: String,
    block: usize,
    caps: &mut BTreeSet<Capability>,
    evidence: &mut Vec<CapabilityEvidence>,
) {
    caps.insert(cap);
    evidence.push(CapabilityEvidence { capability: cap, reason, block });
}

/// Infer capabilities for multiple MIR bodies.
pub fn infer_all(bodies: &[MirBody]) -> Vec<InferredCapabilities> {
    bodies.iter().map(infer_capabilities).collect()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir_to_mlir::*;

    fn simple_body(name: &str, stmts: Vec<Statement>, term: Terminator) -> MirBody {
        MirBody {
            name: name.into(),
            blocks: vec![BasicBlock { label: "bb0".into(), statements: stmts, terminator: term }],
            local_types: vec![],
        }
    }

    fn call_term(func: &str) -> Terminator {
        Terminator::Call {
            func: func.into(),
            args: vec![],
            destination: Place::local(0),
            target: Some(1),
        }
    }

    // -- Pure function (no capabilities) -----------------------------------

    #[test]
    fn pure_function() {
        let body = simple_body(
            "add",
            vec![Statement::Assign(
                Place::local(0),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::Copy(Place::local(1)),
                    Operand::Copy(Place::local(2)),
                ),
            )],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(result.is_pure());
        assert_eq!(result.count(), 0);
        assert_eq!(result.function_name, "add");
    }

    // -- Alloc capability --------------------------------------------------

    #[test]
    fn alloc_from_box_new() {
        let body = simple_body("make_box", vec![], call_term("Box::new"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Alloc));
    }

    #[test]
    fn alloc_from_vec_new() {
        let body = simple_body("make_vec", vec![], call_term("Vec::new"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Alloc));
    }

    #[test]
    fn alloc_from_arc() {
        let body = simple_body("make_arc", vec![], call_term("Arc::new"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Alloc));
    }

    // -- IO capability -----------------------------------------------------

    #[test]
    fn io_from_file_open() {
        let body = simple_body("open", vec![], call_term("std::fs::File::open"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Io));
    }

    #[test]
    fn io_from_println() {
        let body = simple_body("greet", vec![], call_term("println"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Io));
    }

    #[test]
    fn io_from_tcp() {
        let body = simple_body("connect", vec![], call_term("TcpStream::connect"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Io));
    }

    // -- Panic capability --------------------------------------------------

    #[test]
    fn panic_from_unwrap() {
        let body = simple_body("parse", vec![], call_term("Option::unwrap"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Panic));
    }

    #[test]
    fn panic_from_expect() {
        let body = simple_body("get", vec![], call_term("Result::expect"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Panic));
    }

    #[test]
    fn panic_from_division() {
        let body = simple_body(
            "div",
            vec![Statement::Assign(
                Place::local(0),
                Rvalue::BinaryOp(
                    BinOp::Div,
                    Operand::Copy(Place::local(1)),
                    Operand::Copy(Place::local(2)),
                ),
            )],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Panic));
    }

    #[test]
    fn panic_from_remainder() {
        let body = simple_body(
            "rem",
            vec![Statement::Assign(
                Place::local(0),
                Rvalue::BinaryOp(
                    BinOp::Rem,
                    Operand::Copy(Place::local(1)),
                    Operand::Copy(Place::local(2)),
                ),
            )],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Panic));
    }

    #[test]
    fn panic_from_unreachable() {
        let body = simple_body("diverge", vec![], Terminator::Unreachable);
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Panic));
    }

    // -- Unsafe capability -------------------------------------------------

    #[test]
    fn unsafe_from_deref_read() {
        let body = simple_body(
            "deref_read",
            vec![Statement::Assign(Place::local(0), Rvalue::Use(Operand::Copy(Place::deref(1))))],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Unsafe));
    }

    #[test]
    fn unsafe_from_deref_write() {
        let body = simple_body(
            "deref_write",
            vec![Statement::Assign(Place::deref(0), Rvalue::Use(Operand::Constant("42".into())))],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Unsafe));
    }

    #[test]
    fn unsafe_from_ref_through_deref() {
        let body = simple_body(
            "ref_deref",
            vec![Statement::Assign(
                Place::local(0),
                Rvalue::Ref { mutable: false, place: Place::deref(1) },
            )],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Unsafe));
    }

    // -- Async capability --------------------------------------------------

    #[test]
    fn async_from_poll() {
        let body = simple_body("poll_fut", vec![], call_term("Future::poll"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Async));
    }

    #[test]
    fn async_from_tokio() {
        let body = simple_body("spawn", vec![], call_term("tokio::spawn"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Async));
    }

    // -- FFI capability ----------------------------------------------------

    #[test]
    fn ffi_from_libc() {
        let body = simple_body("syscall", vec![], call_term("libc::write"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Ffi));
    }

    #[test]
    fn ffi_from_extern() {
        let body = simple_body("extern_call", vec![], call_term("extern c_function"));
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Ffi));
    }

    // -- Multiple capabilities ---------------------------------------------

    #[test]
    fn multiple_capabilities() {
        let body = MirBody {
            name: "complex".into(),
            blocks: vec![
                BasicBlock {
                    label: "bb0".into(),
                    statements: vec![Statement::Assign(
                        Place::deref(0),
                        Rvalue::Use(Operand::Constant("1".into())),
                    )],
                    terminator: Terminator::Goto { target: 1 },
                },
                BasicBlock {
                    label: "bb1".into(),
                    statements: vec![],
                    terminator: call_term("Vec::new"),
                },
                BasicBlock {
                    label: "bb2".into(),
                    statements: vec![],
                    terminator: call_term("std::fs::read"),
                },
            ],
            local_types: vec![],
        };
        let result = infer_capabilities(&body);
        assert!(result.has(Capability::Unsafe));
        assert!(result.has(Capability::Alloc));
        assert!(result.has(Capability::Io));
        assert_eq!(result.count(), 3);
    }

    // -- Evidence ----------------------------------------------------------

    #[test]
    fn evidence_records_block_and_reason() {
        let body = simple_body("f", vec![], call_term("Box::new"));
        let result = infer_capabilities(&body);
        assert_eq!(result.evidence.len(), 1);
        assert_eq!(result.evidence[0].capability, Capability::Alloc);
        assert!(result.evidence[0].reason.contains("Box::new"));
        assert_eq!(result.evidence[0].block, 0);
    }

    // -- Batch inference ---------------------------------------------------

    #[test]
    fn infer_all_batch() {
        let bodies = vec![
            simple_body("pure", vec![], Terminator::Return),
            simple_body("io", vec![], call_term("println")),
        ];
        let results = infer_all(&bodies);
        assert_eq!(results.len(), 2);
        assert!(results[0].is_pure());
        assert!(results[1].has(Capability::Io));
    }

    // -- Display -----------------------------------------------------------

    #[test]
    fn capability_display() {
        assert_eq!(format!("{}", Capability::Alloc), "alloc");
        assert_eq!(format!("{}", Capability::Io), "io");
        assert_eq!(format!("{}", Capability::Panic), "panic");
        assert_eq!(format!("{}", Capability::Unsafe), "unsafe");
        assert_eq!(format!("{}", Capability::Async), "async");
        assert_eq!(format!("{}", Capability::Ffi), "ffi");
    }

    // -- No false positives ------------------------------------------------

    #[test]
    fn normal_ref_not_unsafe() {
        let body = simple_body(
            "borrow",
            vec![Statement::Assign(
                Place::local(0),
                Rvalue::Ref { mutable: true, place: Place::local(1) },
            )],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(!result.has(Capability::Unsafe));
    }

    #[test]
    fn field_access_not_unsafe() {
        let body = simple_body(
            "field",
            vec![Statement::Assign(
                Place::local(0),
                Rvalue::Use(Operand::Copy(Place::field(1, 0))),
            )],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(!result.has(Capability::Unsafe));
    }

    #[test]
    fn addition_not_panic() {
        let body = simple_body(
            "add",
            vec![Statement::Assign(
                Place::local(0),
                Rvalue::BinaryOp(
                    BinOp::Add,
                    Operand::Copy(Place::local(1)),
                    Operand::Copy(Place::local(2)),
                ),
            )],
            Terminator::Return,
        );
        let result = infer_capabilities(&body);
        assert!(!result.has(Capability::Panic));
    }

    #[test]
    fn unknown_call_no_capabilities() {
        let body = simple_body("custom", vec![], call_term("my_crate::helper"));
        let result = infer_capabilities(&body);
        assert!(result.is_pure());
    }
}
