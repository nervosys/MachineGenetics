//! # Redox MLIR Dialect Definition
//!
//! Defines the Redox MLIR dialect: types, operations, and verification rules.
//! This corresponds to REDOX_PROPOSAL.md §14 — formal operation definitions
//! following MLIR's ODS conventions, implemented as Rust types for the mock
//! pipeline and as a specification for future TableGen codegen.
//!
//! ## Dialect Structure
//!
//! - **Types**: `OwnedType`, `RefType`, `RegionType`, `EffectType`, `CapabilityType`
//! - **Ownership ops**: `redox.move`, `redox.copy`, `redox.borrow`, `redox.drop`
//! - **Effect ops**: `redox.effect.decl`, `redox.effect.perform`, `redox.effect.handle`
//! - **Contract ops**: `redox.contract.require`, `redox.contract.ensure`, `redox.contract.invariant`
//! - **Performance ops**: `redox.perf.place`, `redox.perf.vectorize`,
//!   `redox.perf.no_bounds_check`, `redox.perf.autotune`, `redox.perf.cost_query`
//! - **Capability ops**: `redox.capability.decl`, `redox.capability.check`, `redox.capability.gate`

use std::fmt;

// ===========================================================================
// Dialect registration
// ===========================================================================

/// Metadata for the Redox MLIR dialect.
pub struct RedoxDialect {
    pub name: &'static str,
    pub cpp_namespace: &'static str,
    pub summary: &'static str,
}

/// The canonical Redox dialect descriptor.
pub const REDOX_DIALECT: RedoxDialect = RedoxDialect {
    name: "redox",
    cpp_namespace: "::redox",
    summary: "Redox agentic language dialect for MLIR",
};

// ===========================================================================
// Type definitions (§14.2)
// ===========================================================================

/// Borrow mode for references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorrowMode {
    Shared,
    Exclusive,
    Inferred,
}

impl fmt::Display for BorrowMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BorrowMode::Shared => write!(f, "shared"),
            BorrowMode::Exclusive => write!(f, "exclusive"),
            BorrowMode::Inferred => write!(f, "inferred"),
        }
    }
}

/// A type in the Redox MLIR dialect.
#[derive(Debug, Clone, PartialEq)]
pub enum RedoxType {
    /// `!redox.owned<T>` — an owned value with compiler-managed ownership.
    Owned(OwnedType),
    /// `!redox.ref<T, mode>` — a reference with borrow mode.
    Ref(RefType),
    /// `!redox.region` — a lifetime region variable.
    Region(RegionType),
    /// `!redox.effect` — an algebraic effect annotation.
    Effect(EffectType),
    /// `!redox.cap` — an agent capability token.
    Capability(CapabilityType),
}

/// `!redox.owned<elementType>` — compiler manages ownership transfer.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedType {
    pub element_type: String,
}

/// `!redox.ref<elementType, mode>` — reference with borrow mode.
#[derive(Debug, Clone, PartialEq)]
pub struct RefType {
    pub element_type: String,
    pub mode: BorrowMode,
}

/// `!redox.region` — lifetime region variable.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionType {
    pub name: String,
}

/// `!redox.effect` — algebraic effect annotation (IO, Async, Alloc, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct EffectType {
    pub effect_name: String,
}

/// `!redox.cap` — agent capability token for discovery.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityType {
    pub capabilities: Vec<String>,
}

impl fmt::Display for RedoxType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedoxType::Owned(t) => write!(f, "!redox.owned<{}>", t.element_type),
            RedoxType::Ref(t) => write!(f, "!redox.ref<{}, {}>", t.element_type, t.mode),
            RedoxType::Region(t) => write!(f, "!redox.region<{}>", t.name),
            RedoxType::Effect(t) => write!(f, "!redox.effect<{}>", t.effect_name),
            RedoxType::Capability(t) => {
                write!(f, "!redox.cap<[{}]>", t.capabilities.join(", "))
            }
        }
    }
}

// ===========================================================================
// Operation definitions
// ===========================================================================

/// All operations in the Redox MLIR dialect.
#[derive(Debug, Clone, PartialEq)]
pub enum RedoxOp {
    // Ownership operations (§14.3)
    Move(MoveOp),
    Copy(CopyOp),
    Borrow(BorrowOp),
    Drop(DropOp),

    // Effect operations (§14.4)
    EffectDecl(EffectDeclOp),
    EffectPerform(EffectPerformOp),
    EffectHandle(EffectHandleOp),

    // Contract operations (§14.5)
    ContractRequire(RequireOp),
    ContractEnsure(EnsureOp),
    ContractInvariant(InvariantOp),

    // Performance annotation operations (§14.6)
    PerfPlace(PlaceOp),
    PerfVectorize(VectorizeOp),
    PerfNoBoundsCheck(NoBoundsCheckOp),
    PerfAutotune(AutotuneOp),
    PerfCostQuery(CostQueryOp),

    // Capability operations (§14.7)
    CapabilityDecl(CapabilityDeclOp),
    CapabilityCheck(CapabilityCheckOp),
    CapabilityGate(CapabilityGateOp),
}

impl RedoxOp {
    /// Returns the canonical operation name (e.g. `"redox.move"`).
    pub fn op_name(&self) -> &'static str {
        match self {
            RedoxOp::Move(_) => "redox.move",
            RedoxOp::Copy(_) => "redox.copy",
            RedoxOp::Borrow(_) => "redox.borrow",
            RedoxOp::Drop(_) => "redox.drop",
            RedoxOp::EffectDecl(_) => "redox.effect.decl",
            RedoxOp::EffectPerform(_) => "redox.effect.perform",
            RedoxOp::EffectHandle(_) => "redox.effect.handle",
            RedoxOp::ContractRequire(_) => "redox.contract.require",
            RedoxOp::ContractEnsure(_) => "redox.contract.ensure",
            RedoxOp::ContractInvariant(_) => "redox.contract.invariant",
            RedoxOp::PerfPlace(_) => "redox.perf.place",
            RedoxOp::PerfVectorize(_) => "redox.perf.vectorize",
            RedoxOp::PerfNoBoundsCheck(_) => "redox.perf.no_bounds_check",
            RedoxOp::PerfAutotune(_) => "redox.perf.autotune",
            RedoxOp::PerfCostQuery(_) => "redox.perf.cost_query",
            RedoxOp::CapabilityDecl(_) => "redox.capability.decl",
            RedoxOp::CapabilityCheck(_) => "redox.capability.check",
            RedoxOp::CapabilityGate(_) => "redox.capability.gate",
        }
    }
}

// ---------------------------------------------------------------------------
// Ownership operations (§14.3)
// ---------------------------------------------------------------------------

/// `redox.move` — transfer ownership from source to result.
/// After this op, the source SSA value is consumed and must not be used.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveOp {
    pub source_type: RedoxType,
    pub result_type: RedoxType,
}

/// `redox.copy` — duplicate a value (only valid for Copy types).
#[derive(Debug, Clone, PartialEq)]
pub struct CopyOp {
    pub source_type: RedoxType,
}

/// `redox.borrow` — create a reference to a value.
#[derive(Debug, Clone, PartialEq)]
pub struct BorrowOp {
    pub source_type: RedoxType,
    pub mode: BorrowMode,
    pub region: RegionType,
}

/// `redox.drop` — run destructor and release owned resources.
#[derive(Debug, Clone, PartialEq)]
pub struct DropOp {
    pub value_type: RedoxType,
}

// ---------------------------------------------------------------------------
// Effect operations (§14.4)
// ---------------------------------------------------------------------------

/// `redox.effect.decl` — declare algebraic effects on a function region.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectDeclOp {
    pub effects: Vec<String>,
    pub handlers: Vec<String>,
}

/// `redox.effect.perform` — perform an algebraic effect at runtime.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectPerformOp {
    pub effect: EffectType,
    pub arg_types: Vec<RedoxType>,
    pub result_type: Option<RedoxType>,
}

/// `redox.effect.handle` — install an effect handler for a region.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectHandleOp {
    pub effect: EffectType,
}

// ---------------------------------------------------------------------------
// Contract operations (§14.5)
// ---------------------------------------------------------------------------

/// `redox.contract.require` — assert a precondition (contract).
#[derive(Debug, Clone, PartialEq)]
pub struct RequireOp {
    pub message: String,
}

/// `redox.contract.ensure` — assert a postcondition.
#[derive(Debug, Clone, PartialEq)]
pub struct EnsureOp {
    pub message: String,
    pub has_return_value: bool,
}

/// `redox.contract.invariant` — assert a loop or type invariant.
#[derive(Debug, Clone, PartialEq)]
pub struct InvariantOp {
    pub message: String,
    pub kind: InvariantKind,
}

/// Kind of invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InvariantKind {
    Loop,
    Type,
    Module,
}

impl fmt::Display for InvariantKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvariantKind::Loop => write!(f, "loop"),
            InvariantKind::Type => write!(f, "type"),
            InvariantKind::Module => write!(f, "module"),
        }
    }
}

// ---------------------------------------------------------------------------
// Performance annotation operations (§14.6)
// ---------------------------------------------------------------------------

/// Target device for placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaceTarget {
    Cpu,
    Gpu,
    Npu,
    Auto,
}

impl fmt::Display for PlaceTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlaceTarget::Cpu => write!(f, "cpu"),
            PlaceTarget::Gpu => write!(f, "gpu"),
            PlaceTarget::Npu => write!(f, "npu"),
            PlaceTarget::Auto => write!(f, "auto"),
        }
    }
}

/// `redox.perf.place` — hint target device for a computation region.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceOp {
    pub target: PlaceTarget,
    pub priority: Option<u64>,
}

/// `redox.perf.vectorize` — hint vectorization width for a loop region.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorizeOp {
    pub width: u64,
}

/// `redox.perf.no_bounds_check` — disable bounds checking (agent-trusted).
#[derive(Debug, Clone, PartialEq)]
pub struct NoBoundsCheckOp;

/// Autotuning metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutotuneMetric {
    Latency,
    Throughput,
    Energy,
}

impl fmt::Display for AutotuneMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutotuneMetric::Latency => write!(f, "latency"),
            AutotuneMetric::Throughput => write!(f, "throughput"),
            AutotuneMetric::Energy => write!(f, "energy"),
        }
    }
}

/// `redox.perf.autotune` — generate N optimization variants for autotuning.
#[derive(Debug, Clone, PartialEq)]
pub struct AutotuneOp {
    pub variants: u64,
    pub metric: Option<AutotuneMetric>,
}

/// Cost query metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CostMetric {
    LatencyNs,
    MemoryBytes,
    Allocs,
    EnergyPj,
}

impl fmt::Display for CostMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CostMetric::LatencyNs => write!(f, "latency_ns"),
            CostMetric::MemoryBytes => write!(f, "memory_bytes"),
            CostMetric::Allocs => write!(f, "allocs"),
            CostMetric::EnergyPj => write!(f, "energy_pj"),
        }
    }
}

/// `redox.perf.cost_query` — query the cost model for an expression.
#[derive(Debug, Clone, PartialEq)]
pub struct CostQueryOp {
    pub target_hw: String,
    pub metric: CostMetric,
}

// ---------------------------------------------------------------------------
// Capability operations (§14.7)
// ---------------------------------------------------------------------------

/// `redox.capability.decl` — declare capabilities provided by a module.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityDeclOp {
    pub name: String,
    pub provides: Vec<String>,
    pub requires: Vec<String>,
    pub version: Option<String>,
}

/// `redox.capability.check` — verify a capability is available (compile-time).
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityCheckOp {
    pub capability: String,
}

/// `redox.capability.gate` — gate a region on a capability token.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityGateOp {
    pub token: CapabilityType,
}

// ===========================================================================
// Dialect registration into Context
// ===========================================================================

/// Registers all Redox dialect types and operations with a Context.
/// Returns a `DialectInfo` summarizing what was registered.
pub fn register_redox_dialect(ctx: &mut crate::Context) -> Result<DialectInfo, crate::MlirError> {
    ctx.register_dialect(REDOX_DIALECT.name)?;
    Ok(DialectInfo {
        name: REDOX_DIALECT.name.to_string(),
        num_types: 5, // Owned, Ref, Region, Effect, Capability
        num_ops: 18,  // All ops defined above
    })
}

/// Summary of a registered dialect.
#[derive(Debug, Clone, PartialEq)]
pub struct DialectInfo {
    pub name: String,
    pub num_types: usize,
    pub num_ops: usize,
}

// ===========================================================================
// Verification
// ===========================================================================

/// Errors that can occur during dialect operation verification.
#[derive(Debug, Clone, PartialEq)]
pub enum VerifyError {
    /// Move op source and result types must match.
    MoveTypeMismatch { source: String, result: String },
    /// Move op requires an owned source type.
    MoveRequiresOwned,
    /// Copy op requires an owned source type.
    CopyRequiresOwned,
    /// Borrow op requires an owned source type.
    BorrowRequiresOwned,
    /// Drop op requires an owned type.
    DropRequiresOwned,
    /// Effect declaration must list at least one effect.
    EmptyEffectDecl,
    /// Vectorize width must be a power of two.
    VectorizeWidthNotPowerOfTwo(u64),
    /// Autotune variants must be ≥ 1.
    AutotuneZeroVariants,
    /// Capability declaration must provide at least one capability.
    EmptyCapabilityProvides,
    /// Contract message must not be empty.
    EmptyContractMessage,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::MoveTypeMismatch { source, result } => {
                write!(f, "redox.move: source type `{source}` != result type `{result}`")
            }
            VerifyError::MoveRequiresOwned => {
                write!(f, "redox.move: source must be !redox.owned<T>")
            }
            VerifyError::CopyRequiresOwned => {
                write!(f, "redox.copy: source must be !redox.owned<T>")
            }
            VerifyError::BorrowRequiresOwned => {
                write!(f, "redox.borrow: source must be !redox.owned<T>")
            }
            VerifyError::DropRequiresOwned => {
                write!(f, "redox.drop: value must be !redox.owned<T>")
            }
            VerifyError::EmptyEffectDecl => {
                write!(f, "redox.effect.decl: must declare at least one effect")
            }
            VerifyError::VectorizeWidthNotPowerOfTwo(w) => {
                write!(f, "redox.perf.vectorize: width {w} is not a power of two")
            }
            VerifyError::AutotuneZeroVariants => {
                write!(f, "redox.perf.autotune: variants must be >= 1")
            }
            VerifyError::EmptyCapabilityProvides => {
                write!(f, "redox.capability.decl: must provide at least one capability")
            }
            VerifyError::EmptyContractMessage => {
                write!(f, "contract op: message must not be empty")
            }
        }
    }
}

impl std::error::Error for VerifyError {}

/// Verify a single Redox dialect operation.
pub fn verify_op(op: &RedoxOp) -> Result<(), VerifyError> {
    match op {
        RedoxOp::Move(m) => {
            // Source must be owned
            if !matches!(m.source_type, RedoxType::Owned(_)) {
                return Err(VerifyError::MoveRequiresOwned);
            }
            // Source and result must match
            if m.source_type != m.result_type {
                return Err(VerifyError::MoveTypeMismatch {
                    source: m.source_type.to_string(),
                    result: m.result_type.to_string(),
                });
            }
            Ok(())
        }
        RedoxOp::Copy(c) => {
            if !matches!(c.source_type, RedoxType::Owned(_)) {
                return Err(VerifyError::CopyRequiresOwned);
            }
            Ok(())
        }
        RedoxOp::Borrow(b) => {
            if !matches!(b.source_type, RedoxType::Owned(_)) {
                return Err(VerifyError::BorrowRequiresOwned);
            }
            Ok(())
        }
        RedoxOp::Drop(d) => {
            if !matches!(d.value_type, RedoxType::Owned(_)) {
                return Err(VerifyError::DropRequiresOwned);
            }
            Ok(())
        }
        RedoxOp::EffectDecl(e) => {
            if e.effects.is_empty() {
                return Err(VerifyError::EmptyEffectDecl);
            }
            Ok(())
        }
        RedoxOp::EffectPerform(_) => Ok(()),
        RedoxOp::EffectHandle(_) => Ok(()),
        RedoxOp::ContractRequire(r) => {
            if r.message.is_empty() {
                return Err(VerifyError::EmptyContractMessage);
            }
            Ok(())
        }
        RedoxOp::ContractEnsure(e) => {
            if e.message.is_empty() {
                return Err(VerifyError::EmptyContractMessage);
            }
            Ok(())
        }
        RedoxOp::ContractInvariant(i) => {
            if i.message.is_empty() {
                return Err(VerifyError::EmptyContractMessage);
            }
            Ok(())
        }
        RedoxOp::PerfPlace(_) => Ok(()),
        RedoxOp::PerfVectorize(v) => {
            if v.width == 0 || (v.width & (v.width - 1)) != 0 {
                return Err(VerifyError::VectorizeWidthNotPowerOfTwo(v.width));
            }
            Ok(())
        }
        RedoxOp::PerfNoBoundsCheck(_) => Ok(()),
        RedoxOp::PerfAutotune(a) => {
            if a.variants == 0 {
                return Err(VerifyError::AutotuneZeroVariants);
            }
            Ok(())
        }
        RedoxOp::PerfCostQuery(_) => Ok(()),
        RedoxOp::CapabilityDecl(c) => {
            if c.provides.is_empty() {
                return Err(VerifyError::EmptyCapabilityProvides);
            }
            Ok(())
        }
        RedoxOp::CapabilityCheck(_) => Ok(()),
        RedoxOp::CapabilityGate(_) => Ok(()),
    }
}

/// Verify all operations in a sequence (e.g., a module body).
pub fn verify_ops(ops: &[RedoxOp]) -> Vec<VerifyError> {
    ops.iter().filter_map(|op| verify_op(op).err()).collect()
}

// ===========================================================================
// Builder helpers
// ===========================================================================

/// Build an `!redox.owned<T>` type.
pub fn owned_type(element: impl Into<String>) -> RedoxType {
    RedoxType::Owned(OwnedType { element_type: element.into() })
}

/// Build an `!redox.ref<T, mode>` type.
pub fn ref_type(element: impl Into<String>, mode: BorrowMode) -> RedoxType {
    RedoxType::Ref(RefType { element_type: element.into(), mode })
}

/// Build an `!redox.region` type.
pub fn region_type(name: impl Into<String>) -> RegionType {
    RegionType { name: name.into() }
}

/// Build an `!redox.effect` type.
pub fn effect_type(name: impl Into<String>) -> EffectType {
    EffectType { effect_name: name.into() }
}

/// Build an `!redox.cap` type.
pub fn cap_type(caps: Vec<String>) -> RedoxType {
    RedoxType::Capability(CapabilityType { capabilities: caps })
}

// ===========================================================================
// Lowering table (§14.9) — reference mapping
// ===========================================================================

/// The dialect that a Redox operation lowers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweringTarget {
    Std,
    MemRef,
    Func,
    Cf,
    Gpu,
    Vector,
    Arith,
    Scf,
    /// Stays in Redox dialect (e.g. metadata-only ops).
    Redox,
}

/// Returns the lowering target for a given operation.
pub fn lowering_target(op: &RedoxOp) -> LoweringTarget {
    match op {
        RedoxOp::Move(_) => LoweringTarget::Std,
        RedoxOp::Copy(_) => LoweringTarget::MemRef,
        RedoxOp::Borrow(_) => LoweringTarget::MemRef,
        RedoxOp::Drop(_) => LoweringTarget::Func,
        RedoxOp::EffectDecl(_) => LoweringTarget::Redox,
        RedoxOp::EffectPerform(_) => LoweringTarget::Func,
        RedoxOp::EffectHandle(_) => LoweringTarget::Func,
        RedoxOp::ContractRequire(_) => LoweringTarget::Cf,
        RedoxOp::ContractEnsure(_) => LoweringTarget::Cf,
        RedoxOp::ContractInvariant(_) => LoweringTarget::Cf,
        RedoxOp::PerfPlace(p) => match p.target {
            PlaceTarget::Gpu => LoweringTarget::Gpu,
            _ => LoweringTarget::Func,
        },
        RedoxOp::PerfVectorize(_) => LoweringTarget::Vector,
        RedoxOp::PerfNoBoundsCheck(_) => LoweringTarget::Redox,
        RedoxOp::PerfAutotune(_) => LoweringTarget::Redox,
        RedoxOp::PerfCostQuery(_) => LoweringTarget::Arith,
        RedoxOp::CapabilityDecl(_) => LoweringTarget::Redox,
        RedoxOp::CapabilityCheck(_) => LoweringTarget::Redox,
        RedoxOp::CapabilityGate(_) => LoweringTarget::Scf,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Type display --------------------------------------------------------

    #[test]
    fn display_owned_type() {
        let t = owned_type("tensor<4xf32>");
        assert_eq!(t.to_string(), "!redox.owned<tensor<4xf32>>");
    }

    #[test]
    fn display_ref_type() {
        let t = ref_type("i64", BorrowMode::Shared);
        assert_eq!(t.to_string(), "!redox.ref<i64, shared>");
    }

    #[test]
    fn display_region_type() {
        let t = RedoxType::Region(region_type("'a"));
        assert_eq!(t.to_string(), "!redox.region<'a>");
    }

    #[test]
    fn display_effect_type() {
        let t = RedoxType::Effect(effect_type("IO"));
        assert_eq!(t.to_string(), "!redox.effect<IO>");
    }

    #[test]
    fn display_capability_type() {
        let t = cap_type(vec!["http_client".into(), "json_parse".into()]);
        assert_eq!(t.to_string(), "!redox.cap<[http_client, json_parse]>");
    }

    // -- Dialect registration ------------------------------------------------

    #[test]
    fn register_redox_dialect_succeeds() {
        let mut ctx = crate::Context::new().unwrap();
        let info = register_redox_dialect(&mut ctx).unwrap();
        assert_eq!(info.name, "redox");
        assert_eq!(info.num_types, 5);
        assert_eq!(info.num_ops, 18);
        assert_eq!(ctx.num_registered_dialects(), 1);
    }

    // -- Op names ------------------------------------------------------------

    #[test]
    fn op_names_ownership() {
        let owned = owned_type("i32");
        assert_eq!(
            RedoxOp::Move(MoveOp { source_type: owned.clone(), result_type: owned.clone() })
                .op_name(),
            "redox.move"
        );
        assert_eq!(RedoxOp::Copy(CopyOp { source_type: owned.clone() }).op_name(), "redox.copy");
        assert_eq!(
            RedoxOp::Borrow(BorrowOp {
                source_type: owned.clone(),
                mode: BorrowMode::Shared,
                region: region_type("'a"),
            })
            .op_name(),
            "redox.borrow"
        );
        assert_eq!(RedoxOp::Drop(DropOp { value_type: owned }).op_name(), "redox.drop");
    }

    #[test]
    fn op_names_effects() {
        assert_eq!(
            RedoxOp::EffectDecl(EffectDeclOp { effects: vec!["IO".into()], handlers: vec![] })
                .op_name(),
            "redox.effect.decl"
        );
        assert_eq!(
            RedoxOp::EffectPerform(EffectPerformOp {
                effect: effect_type("IO"),
                arg_types: vec![],
                result_type: None,
            })
            .op_name(),
            "redox.effect.perform"
        );
        assert_eq!(
            RedoxOp::EffectHandle(EffectHandleOp { effect: effect_type("Async") }).op_name(),
            "redox.effect.handle"
        );
    }

    #[test]
    fn op_names_contracts() {
        assert_eq!(
            RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() }).op_name(),
            "redox.contract.require"
        );
        assert_eq!(
            RedoxOp::ContractEnsure(EnsureOp { message: "sorted".into(), has_return_value: true })
                .op_name(),
            "redox.contract.ensure"
        );
        assert_eq!(
            RedoxOp::ContractInvariant(InvariantOp {
                message: "len > 0".into(),
                kind: InvariantKind::Loop,
            })
            .op_name(),
            "redox.contract.invariant"
        );
    }

    #[test]
    fn op_names_perf() {
        assert_eq!(
            RedoxOp::PerfPlace(PlaceOp { target: PlaceTarget::Auto, priority: None }).op_name(),
            "redox.perf.place"
        );
        assert_eq!(
            RedoxOp::PerfVectorize(VectorizeOp { width: 8 }).op_name(),
            "redox.perf.vectorize"
        );
        assert_eq!(
            RedoxOp::PerfNoBoundsCheck(NoBoundsCheckOp).op_name(),
            "redox.perf.no_bounds_check"
        );
        assert_eq!(
            RedoxOp::PerfAutotune(AutotuneOp { variants: 4, metric: None }).op_name(),
            "redox.perf.autotune"
        );
        assert_eq!(
            RedoxOp::PerfCostQuery(CostQueryOp {
                target_hw: "x86_64".into(),
                metric: CostMetric::LatencyNs,
            })
            .op_name(),
            "redox.perf.cost_query"
        );
    }

    #[test]
    fn op_names_capability() {
        assert_eq!(
            RedoxOp::CapabilityDecl(CapabilityDeclOp {
                name: "http".into(),
                provides: vec!["http_client".into()],
                requires: vec![],
                version: None,
            })
            .op_name(),
            "redox.capability.decl"
        );
        assert_eq!(
            RedoxOp::CapabilityCheck(CapabilityCheckOp { capability: "http_client".into() })
                .op_name(),
            "redox.capability.check"
        );
        assert_eq!(
            RedoxOp::CapabilityGate(CapabilityGateOp {
                token: CapabilityType { capabilities: vec!["http_client".into()] },
            })
            .op_name(),
            "redox.capability.gate"
        );
    }

    // -- Verification: valid ops ---------------------------------------------

    #[test]
    fn verify_valid_move() {
        let t = owned_type("i64");
        let op = RedoxOp::Move(MoveOp { source_type: t.clone(), result_type: t });
        assert!(verify_op(&op).is_ok());
    }

    #[test]
    fn verify_valid_copy() {
        let op = RedoxOp::Copy(CopyOp { source_type: owned_type("i32") });
        assert!(verify_op(&op).is_ok());
    }

    #[test]
    fn verify_valid_borrow() {
        let op = RedoxOp::Borrow(BorrowOp {
            source_type: owned_type("String"),
            mode: BorrowMode::Exclusive,
            region: region_type("'a"),
        });
        assert!(verify_op(&op).is_ok());
    }

    #[test]
    fn verify_valid_drop() {
        let op = RedoxOp::Drop(DropOp { value_type: owned_type("Vec<u8>") });
        assert!(verify_op(&op).is_ok());
    }

    #[test]
    fn verify_valid_effect_decl() {
        let op = RedoxOp::EffectDecl(EffectDeclOp {
            effects: vec!["IO".into(), "Async".into()],
            handlers: vec![],
        });
        assert!(verify_op(&op).is_ok());
    }

    #[test]
    fn verify_valid_contract_require() {
        let op = RedoxOp::ContractRequire(RequireOp { message: "index must be in bounds".into() });
        assert!(verify_op(&op).is_ok());
    }

    #[test]
    fn verify_valid_vectorize() {
        for w in [1, 2, 4, 8, 16] {
            let op = RedoxOp::PerfVectorize(VectorizeOp { width: w });
            assert!(verify_op(&op).is_ok(), "width {w} should be valid");
        }
    }

    #[test]
    fn verify_valid_autotune() {
        let op = RedoxOp::PerfAutotune(AutotuneOp {
            variants: 8,
            metric: Some(AutotuneMetric::Latency),
        });
        assert!(verify_op(&op).is_ok());
    }

    #[test]
    fn verify_valid_capability_decl() {
        let op = RedoxOp::CapabilityDecl(CapabilityDeclOp {
            name: "http".into(),
            provides: vec!["http_client".into()],
            requires: vec!["net".into()],
            version: Some("1.0".into()),
        });
        assert!(verify_op(&op).is_ok());
    }

    // -- Verification: invalid ops -------------------------------------------

    #[test]
    fn verify_move_type_mismatch() {
        let op = RedoxOp::Move(MoveOp {
            source_type: owned_type("i32"),
            result_type: owned_type("i64"),
        });
        assert_eq!(
            verify_op(&op).unwrap_err(),
            VerifyError::MoveTypeMismatch {
                source: "!redox.owned<i32>".into(),
                result: "!redox.owned<i64>".into(),
            }
        );
    }

    #[test]
    fn verify_move_requires_owned() {
        let op = RedoxOp::Move(MoveOp {
            source_type: ref_type("i32", BorrowMode::Shared),
            result_type: ref_type("i32", BorrowMode::Shared),
        });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::MoveRequiresOwned);
    }

    #[test]
    fn verify_copy_requires_owned() {
        let op = RedoxOp::Copy(CopyOp { source_type: RedoxType::Effect(effect_type("IO")) });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::CopyRequiresOwned);
    }

    #[test]
    fn verify_borrow_requires_owned() {
        let op = RedoxOp::Borrow(BorrowOp {
            source_type: ref_type("i32", BorrowMode::Shared),
            mode: BorrowMode::Shared,
            region: region_type("'a"),
        });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::BorrowRequiresOwned);
    }

    #[test]
    fn verify_drop_requires_owned() {
        let op = RedoxOp::Drop(DropOp { value_type: RedoxType::Region(region_type("'a")) });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::DropRequiresOwned);
    }

    #[test]
    fn verify_empty_effect_decl() {
        let op = RedoxOp::EffectDecl(EffectDeclOp { effects: vec![], handlers: vec![] });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::EmptyEffectDecl);
    }

    #[test]
    fn verify_vectorize_not_power_of_two() {
        for w in [0, 3, 5, 6, 7, 9] {
            let op = RedoxOp::PerfVectorize(VectorizeOp { width: w });
            assert_eq!(
                verify_op(&op).unwrap_err(),
                VerifyError::VectorizeWidthNotPowerOfTwo(w),
                "width {w} should be rejected"
            );
        }
    }

    #[test]
    fn verify_autotune_zero_variants() {
        let op = RedoxOp::PerfAutotune(AutotuneOp { variants: 0, metric: None });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::AutotuneZeroVariants);
    }

    #[test]
    fn verify_empty_capability_provides() {
        let op = RedoxOp::CapabilityDecl(CapabilityDeclOp {
            name: "empty".into(),
            provides: vec![],
            requires: vec![],
            version: None,
        });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::EmptyCapabilityProvides);
    }

    #[test]
    fn verify_empty_contract_message() {
        let op = RedoxOp::ContractRequire(RequireOp { message: String::new() });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::EmptyContractMessage);

        let op =
            RedoxOp::ContractEnsure(EnsureOp { message: String::new(), has_return_value: false });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::EmptyContractMessage);

        let op = RedoxOp::ContractInvariant(InvariantOp {
            message: String::new(),
            kind: InvariantKind::Loop,
        });
        assert_eq!(verify_op(&op).unwrap_err(), VerifyError::EmptyContractMessage);
    }

    // -- Batch verification --------------------------------------------------

    #[test]
    fn verify_ops_batch_mixed() {
        let ops = vec![
            RedoxOp::Move(MoveOp {
                source_type: owned_type("i32"),
                result_type: owned_type("i32"),
            }),
            // Invalid: move on ref
            RedoxOp::Move(MoveOp {
                source_type: ref_type("i32", BorrowMode::Shared),
                result_type: ref_type("i32", BorrowMode::Shared),
            }),
            // Valid drop
            RedoxOp::Drop(DropOp { value_type: owned_type("Vec<u8>") }),
            // Invalid: empty effect decl
            RedoxOp::EffectDecl(EffectDeclOp { effects: vec![], handlers: vec![] }),
        ];
        let errors = verify_ops(&ops);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0], VerifyError::MoveRequiresOwned);
        assert_eq!(errors[1], VerifyError::EmptyEffectDecl);
    }

    // -- Lowering target table -----------------------------------------------

    #[test]
    fn lowering_targets() {
        let owned = owned_type("i32");
        assert_eq!(
            lowering_target(&RedoxOp::Move(MoveOp {
                source_type: owned.clone(),
                result_type: owned.clone(),
            })),
            LoweringTarget::Std
        );
        assert_eq!(
            lowering_target(&RedoxOp::Copy(CopyOp { source_type: owned.clone() })),
            LoweringTarget::MemRef
        );
        assert_eq!(
            lowering_target(&RedoxOp::Drop(DropOp { value_type: owned.clone() })),
            LoweringTarget::Func
        );
        assert_eq!(
            lowering_target(&RedoxOp::ContractRequire(RequireOp { message: "x > 0".into() })),
            LoweringTarget::Cf
        );
        assert_eq!(
            lowering_target(&RedoxOp::PerfPlace(PlaceOp {
                target: PlaceTarget::Gpu,
                priority: None,
            })),
            LoweringTarget::Gpu
        );
        assert_eq!(
            lowering_target(&RedoxOp::PerfVectorize(VectorizeOp { width: 4 })),
            LoweringTarget::Vector
        );
        assert_eq!(
            lowering_target(&RedoxOp::PerfCostQuery(CostQueryOp {
                target_hw: "x86_64".into(),
                metric: CostMetric::LatencyNs,
            })),
            LoweringTarget::Arith
        );
        assert_eq!(
            lowering_target(&RedoxOp::CapabilityGate(CapabilityGateOp {
                token: CapabilityType { capabilities: vec!["net".into()] },
            })),
            LoweringTarget::Scf
        );
    }

    // -- Integration: register + create ops + verify -------------------------

    #[test]
    fn integration_register_and_verify_program() {
        let mut ctx = crate::Context::new().unwrap();
        let info = register_redox_dialect(&mut ctx).unwrap();
        assert_eq!(info.name, "redox");

        // Build a small program
        let owned_i32 = owned_type("i32");
        let ops = vec![
            // Effect declaration
            RedoxOp::EffectDecl(EffectDeclOp { effects: vec!["IO".into()], handlers: vec![] }),
            // Precondition
            RedoxOp::ContractRequire(RequireOp { message: "n > 0".into() }),
            // Move
            RedoxOp::Move(MoveOp {
                source_type: owned_i32.clone(),
                result_type: owned_i32.clone(),
            }),
            // Borrow
            RedoxOp::Borrow(BorrowOp {
                source_type: owned_i32.clone(),
                mode: BorrowMode::Shared,
                region: region_type("'a"),
            }),
            // Postcondition
            RedoxOp::ContractEnsure(EnsureOp {
                message: "result is valid".into(),
                has_return_value: true,
            }),
            // Drop
            RedoxOp::Drop(DropOp { value_type: owned_i32 }),
        ];

        let errors = verify_ops(&ops);
        assert!(errors.is_empty(), "all ops should verify: {errors:?}");

        // Also add them to a module
        let loc = crate::Location::file_line_col("test.mg", 1, 0);
        let mut module = crate::Module::new_empty(&ctx, &loc).unwrap();
        for op in &ops {
            module.add_operation(op.op_name());
        }
        assert_eq!(module.num_operations(), 6);
    }
}
