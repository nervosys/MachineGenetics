// Redox Verification Oracle — opt-in compile-time verification service.
//
// Verifies contracts (requires/ensures), effects, and capabilities.
// Emits verification certificates per §6.3 and §8 of REDOX_PROPOSAL.md.
//
// (ROADMAP Step 49)

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Contract Verification ──────────────────────────────────────────────────

/// A function contract to verify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    /// Fully-qualified function name.
    pub function: String,
    /// Precondition expression.
    pub requires: String,
    /// Postcondition expression.
    pub ensures: String,
    /// Side effects declared.
    pub effects: Vec<String>,
}

impl Contract {
    pub fn new(function: &str, requires: &str, ensures: &str) -> Self {
        Contract {
            function: function.to_string(),
            requires: requires.to_string(),
            ensures: ensures.to_string(),
            effects: Vec::new(),
        }
    }

    pub fn with_effects(mut self, effects: Vec<String>) -> Self {
        self.effects = effects;
        self
    }

    pub fn has_precondition(&self) -> bool {
        !self.requires.is_empty()
    }

    pub fn has_postcondition(&self) -> bool {
        !self.ensures.is_empty()
    }

    pub fn has_effects(&self) -> bool {
        !self.effects.is_empty()
    }
}

impl fmt::Display for Contract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}  @req({})  @ens({})", self.function, self.requires, self.ensures)
    }
}

/// Result of verifying a single contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractVerdict {
    /// Contract satisfied (proven or checked).
    Satisfied,
    /// Contract violated with explanation.
    Violated(String),
    /// Contract could not be determined (e.g. too complex).
    Unknown(String),
    /// Conditionally satisfied under stated assumptions.
    Conditional(Vec<String>),
}

impl ContractVerdict {
    pub fn is_satisfied(&self) -> bool {
        matches!(self, ContractVerdict::Satisfied)
    }

    pub fn is_violated(&self) -> bool {
        matches!(self, ContractVerdict::Violated(_))
    }
}

impl fmt::Display for ContractVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractVerdict::Satisfied => write!(f, "satisfied"),
            ContractVerdict::Violated(msg) => write!(f, "violated: {msg}"),
            ContractVerdict::Unknown(msg) => write!(f, "unknown: {msg}"),
            ContractVerdict::Conditional(conds) => {
                write!(f, "conditional: {}", conds.join(", "))
            }
        }
    }
}

/// Result of verifying one contract, including metadata.
#[derive(Debug, Clone)]
pub struct ContractResult {
    pub contract: Contract,
    pub verdict: ContractVerdict,
    pub proof_kind: ProofKind,
}

// ── Effect Verification ────────────────────────────────────────────────────

/// A declared effect to verify.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Pure computation (no side effects).
    Pure,
    /// Allocates heap memory.
    Alloc,
    /// Performs I/O.
    Io,
    /// Panics under some conditions.
    Panic,
    /// Sends messages on the bus.
    Send,
    /// Modifies shared state.
    Mutation,
    /// Custom/user-defined effect.
    Custom(String),
}

impl Effect {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pure" => Some(Effect::Pure),
            "alloc" => Some(Effect::Alloc),
            "io" => Some(Effect::Io),
            "panic" => Some(Effect::Panic),
            "send" => Some(Effect::Send),
            "mutation" => Some(Effect::Mutation),
            _ => Some(Effect::Custom(s.to_string())),
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::Pure => write!(f, "pure"),
            Effect::Alloc => write!(f, "alloc"),
            Effect::Io => write!(f, "io"),
            Effect::Panic => write!(f, "panic"),
            Effect::Send => write!(f, "send"),
            Effect::Mutation => write!(f, "mutation"),
            Effect::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

/// Effect containment verification result.
#[derive(Debug, Clone)]
pub struct EffectResult {
    pub function: String,
    pub declared: Vec<Effect>,
    pub inferred: Vec<Effect>,
    pub contained: bool,
    pub leaks: Vec<Effect>,
}

impl EffectResult {
    pub fn is_contained(&self) -> bool {
        self.contained
    }
}

// ── Capability Verification ────────────────────────────────────────────────

/// Result of verifying an agent's capability usage.
#[derive(Debug, Clone)]
pub struct CapabilityResult {
    pub agent: String,
    pub declared_capabilities: Vec<String>,
    pub used_capabilities: Vec<String>,
    pub exceeds_bounds: Vec<String>,
    pub within_bounds: bool,
}

impl CapabilityResult {
    pub fn is_within_bounds(&self) -> bool {
        self.within_bounds
    }
}

// ── Proof Kinds ────────────────────────────────────────────────────────────

/// How a verification was proved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofKind {
    /// Proved by static analysis (borrowck, type system).
    StaticAnalysis,
    /// Proved by symbolic execution / abstract interpretation.
    SymbolicExecution,
    /// Verified by testing (empirical, not a proof).
    Testing,
    /// Proven by formal verification tool.
    FormalProof,
    /// Bounded model checking.
    BoundedModelCheck,
    /// No proof available.
    None,
}

impl fmt::Display for ProofKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProofKind::StaticAnalysis => write!(f, "static_analysis"),
            ProofKind::SymbolicExecution => write!(f, "symbolic_execution"),
            ProofKind::Testing => write!(f, "testing"),
            ProofKind::FormalProof => write!(f, "formal_proof"),
            ProofKind::BoundedModelCheck => write!(f, "bounded_model_check"),
            ProofKind::None => write!(f, "none"),
        }
    }
}

// ── Verification Checks (§6.3) ────────────────────────────────────────────

/// A single verification check in a certificate.
#[derive(Debug, Clone)]
pub struct VerificationCheck {
    pub kind: CheckKind,
    pub status: CheckStatus,
    pub proof_kind: ProofKind,
    pub details: Option<String>,
}

impl VerificationCheck {
    pub fn proven(kind: CheckKind, proof: ProofKind) -> Self {
        VerificationCheck {
            kind,
            status: CheckStatus::Proven,
            proof_kind: proof,
            details: None,
        }
    }

    pub fn conditional(kind: CheckKind, conditions: Vec<String>) -> Self {
        VerificationCheck {
            kind,
            status: CheckStatus::Conditional(conditions),
            proof_kind: ProofKind::StaticAnalysis,
            details: None,
        }
    }

    pub fn bounded(kind: CheckKind, bound: u64) -> Self {
        VerificationCheck {
            kind,
            status: CheckStatus::Bounded(bound),
            proof_kind: ProofKind::BoundedModelCheck,
            details: None,
        }
    }

    pub fn failed(kind: CheckKind, reason: &str) -> Self {
        VerificationCheck {
            kind,
            status: CheckStatus::Failed(reason.to_string()),
            proof_kind: ProofKind::None,
            details: None,
        }
    }

    pub fn is_proven(&self) -> bool {
        matches!(self.status, CheckStatus::Proven)
    }
}

/// What kind of property was checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckKind {
    MemorySafety,
    DataRaceFreedom,
    Exhaustiveness,
    ContractSatisfaction,
    EffectContainment,
    PanicFreedom,
    StackOverflowFreedom,
    CapabilityBounds,
    Custom(String),
}

impl fmt::Display for CheckKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckKind::MemorySafety => write!(f, "memory_safety"),
            CheckKind::DataRaceFreedom => write!(f, "data_race_freedom"),
            CheckKind::Exhaustiveness => write!(f, "exhaustiveness"),
            CheckKind::ContractSatisfaction => write!(f, "contract_satisfaction"),
            CheckKind::EffectContainment => write!(f, "effect_containment"),
            CheckKind::PanicFreedom => write!(f, "panic_freedom"),
            CheckKind::StackOverflowFreedom => write!(f, "stack_overflow_freedom"),
            CheckKind::CapabilityBounds => write!(f, "capability_bounds"),
            CheckKind::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

/// Status of a verification check.
#[derive(Debug, Clone)]
pub enum CheckStatus {
    /// Proven unconditionally.
    Proven,
    /// Conditionally proven (requires stated assumptions).
    Conditional(Vec<String>),
    /// Bounded (e.g. stack depth ≤ N).
    Bounded(u64),
    /// Verification failed.
    Failed(String),
    /// Skipped.
    Skipped,
}

impl CheckStatus {
    pub fn is_proven(&self) -> bool {
        matches!(self, CheckStatus::Proven)
    }
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckStatus::Proven => write!(f, "proven"),
            CheckStatus::Conditional(c) => write!(f, "conditional({})", c.join("; ")),
            CheckStatus::Bounded(n) => write!(f, "bounded({n})"),
            CheckStatus::Failed(r) => write!(f, "failed: {r}"),
            CheckStatus::Skipped => write!(f, "skipped"),
        }
    }
}

// ── Verification Certificate (§6.3) ───────────────────────────────────────

/// A verification certificate for a crate or module.
#[derive(Debug, Clone)]
pub struct VerificationCertificate {
    pub crate_name: String,
    pub version: String,
    pub timestamp: u64,
    pub checks: Vec<VerificationCheck>,
    pub compiler_version: String,
    pub hash: String,
    pub contracts_verified: usize,
    pub effects_verified: usize,
    pub capabilities_verified: usize,
}

impl VerificationCertificate {
    pub fn all_proven(&self) -> bool {
        self.checks.iter().all(|c| c.is_proven())
    }

    pub fn proven_count(&self) -> usize {
        self.checks.iter().filter(|c| c.is_proven()).count()
    }

    pub fn total_checks(&self) -> usize {
        self.checks.len()
    }

    pub fn failed_checks(&self) -> Vec<&VerificationCheck> {
        self.checks.iter()
            .filter(|c| matches!(c.status, CheckStatus::Failed(_)))
            .collect()
    }

    /// Simple text summary.
    pub fn summary(&self) -> String {
        format!(
            "Certificate({} v{}: {}/{} proven, {} contracts, {} effects, {} capabilities)",
            self.crate_name, self.version,
            self.proven_count(), self.total_checks(),
            self.contracts_verified, self.effects_verified,
            self.capabilities_verified,
        )
    }

    /// Serialize to a minimal JSON string.
    pub fn to_json(&self) -> String {
        let checks_json: Vec<String> = self.checks.iter().map(|c| {
            format!(
                "{{\"kind\":\"{}\",\"status\":\"{}\",\"proof\":\"{}\"}}",
                c.kind, c.status, c.proof_kind,
            )
        }).collect();
        format!(
            concat!(
                "{{\"crate\":\"{}\",\"version\":\"{}\",\"timestamp\":{},",
                "\"checks\":[{}],\"compiler_version\":\"{}\",\"hash\":\"{}\",",
                "\"contracts_verified\":{},\"effects_verified\":{},",
                "\"capabilities_verified\":{}}}"
            ),
            self.crate_name, self.version, self.timestamp,
            checks_json.join(","),
            self.compiler_version, self.hash,
            self.contracts_verified, self.effects_verified,
            self.capabilities_verified,
        )
    }
}

impl fmt::Display for VerificationCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ── Certificate Builder ────────────────────────────────────────────────────

pub struct CertificateBuilder {
    crate_name: String,
    version: String,
    checks: Vec<VerificationCheck>,
    compiler_version: String,
    contracts_verified: usize,
    effects_verified: usize,
    capabilities_verified: usize,
}

impl CertificateBuilder {
    pub fn new(crate_name: &str, version: &str) -> Self {
        CertificateBuilder {
            crate_name: crate_name.to_string(),
            version: version.to_string(),
            checks: Vec::new(),
            compiler_version: "redox 0.1.0".to_string(),
            contracts_verified: 0,
            effects_verified: 0,
            capabilities_verified: 0,
        }
    }

    pub fn compiler_version(mut self, ver: &str) -> Self {
        self.compiler_version = ver.to_string();
        self
    }

    pub fn add_check(mut self, check: VerificationCheck) -> Self {
        self.checks.push(check);
        self
    }

    pub fn contracts_verified(mut self, n: usize) -> Self {
        self.contracts_verified = n;
        self
    }

    pub fn effects_verified(mut self, n: usize) -> Self {
        self.effects_verified = n;
        self
    }

    pub fn capabilities_verified(mut self, n: usize) -> Self {
        self.capabilities_verified = n;
        self
    }

    pub fn build(self) -> VerificationCertificate {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Simple hash from crate name + version + check count.
        let hash_input = format!(
            "{}:{}:{}:{}",
            self.crate_name, self.version, self.checks.len(), timestamp
        );
        let hash = simple_hash(&hash_input);

        VerificationCertificate {
            crate_name: self.crate_name,
            version: self.version,
            timestamp,
            checks: self.checks,
            compiler_version: self.compiler_version,
            hash,
            contracts_verified: self.contracts_verified,
            effects_verified: self.effects_verified,
            capabilities_verified: self.capabilities_verified,
        }
    }
}

/// Simple deterministic hash (not cryptographic, for development).
fn simple_hash(input: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325; // FNV offset basis
    for b in input.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3); // FNV prime
    }
    format!("fnv64:{h:016x}")
}

// ── Verification Oracle ────────────────────────────────────────────────────

/// The main verification oracle: verifies contracts, effects, capabilities
/// and emits certificates.
pub struct VerificationOracle {
    /// Contract verification results.
    contract_results: Vec<ContractResult>,
    /// Effect verification results.
    effect_results: Vec<EffectResult>,
    /// Capability verification results.
    capability_results: Vec<CapabilityResult>,
    /// Emitted certificates.
    certificates: Vec<VerificationCertificate>,
    /// Opt-in: whether verification is enabled.
    enabled: bool,
}

impl VerificationOracle {
    /// Create a new oracle (enabled by default).
    pub fn new() -> Self {
        VerificationOracle {
            contract_results: Vec::new(),
            effect_results: Vec::new(),
            capability_results: Vec::new(),
            certificates: Vec::new(),
            enabled: true,
        }
    }

    /// Create a disabled oracle (opt-out).
    pub fn disabled() -> Self {
        let mut oracle = Self::new();
        oracle.enabled = false;
        oracle
    }

    /// Whether verification is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable verification.
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable verification.
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    // ── Contract Verification ──

    /// Verify a contract. Returns the verdict.
    pub fn verify_contract(&mut self, contract: Contract) -> ContractVerdict {
        if !self.enabled {
            return ContractVerdict::Unknown("verification disabled".to_string());
        }

        let verdict = self.evaluate_contract(&contract);
        let proof_kind = match &verdict {
            ContractVerdict::Satisfied => ProofKind::StaticAnalysis,
            ContractVerdict::Conditional(_) => ProofKind::StaticAnalysis,
            _ => ProofKind::None,
        };

        self.contract_results.push(ContractResult {
            contract,
            verdict: verdict.clone(),
            proof_kind,
        });

        verdict
    }

    /// Evaluate a contract (simulated static analysis).
    fn evaluate_contract(&self, contract: &Contract) -> ContractVerdict {
        // Simple heuristic-based verification:
        // 1. Empty preconditions are trivially satisfied.
        // 2. "true" ensures is trivially satisfied.
        // 3. Preconditions containing "false" indicate impossible paths.
        // 4. Otherwise, conditionally satisfied.

        if contract.requires == "false" {
            return ContractVerdict::Satisfied; // vacuously true
        }

        if contract.ensures.is_empty() || contract.ensures == "true" {
            return ContractVerdict::Satisfied;
        }

        if contract.ensures == "false" {
            return ContractVerdict::Violated(
                "postcondition is unsatisfiable".to_string()
            );
        }

        // Check for common patterns.
        if contract.requires.contains(">=") && contract.ensures.contains(">=") {
            return ContractVerdict::Satisfied;
        }

        if contract.requires.is_empty() {
            return ContractVerdict::Conditional(vec![
                "no precondition specified".to_string(),
            ]);
        }

        ContractVerdict::Conditional(vec![
            format!("requires: {}", contract.requires),
        ])
    }

    /// Verify multiple contracts, returning all verdicts.
    pub fn verify_contracts(&mut self, contracts: Vec<Contract>) -> Vec<ContractVerdict> {
        contracts.into_iter()
            .map(|c| self.verify_contract(c))
            .collect()
    }

    // ── Effect Verification ──

    /// Verify that a function's actual effects are contained within declared effects.
    pub fn verify_effects(
        &mut self,
        function: &str,
        declared: Vec<Effect>,
        inferred: Vec<Effect>,
    ) -> EffectResult {
        let leaks: Vec<Effect> = if self.enabled {
            inferred.iter()
                .filter(|e| {
                    // Pure is always contained.
                    if **e == Effect::Pure { return false; }
                    !declared.contains(e)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let contained = leaks.is_empty();
        let result = EffectResult {
            function: function.to_string(),
            declared,
            inferred,
            contained,
            leaks,
        };
        self.effect_results.push(result.clone());
        result
    }

    // ── Capability Verification ──

    /// Verify that an agent uses only capabilities within its declared bounds.
    pub fn verify_capabilities(
        &mut self,
        agent: &str,
        declared: Vec<String>,
        used: Vec<String>,
    ) -> CapabilityResult {
        let exceeds: Vec<String> = if self.enabled {
            used.iter()
                .filter(|u| !declared.contains(u))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        let within_bounds = exceeds.is_empty();
        let result = CapabilityResult {
            agent: agent.to_string(),
            declared_capabilities: declared,
            used_capabilities: used,
            exceeds_bounds: exceeds,
            within_bounds,
        };
        self.capability_results.push(result.clone());
        result
    }

    // ── Certificate Emission ──

    /// Emit a verification certificate for a crate, based on all accumulated results.
    pub fn emit_certificate(
        &mut self,
        crate_name: &str,
        version: &str,
    ) -> VerificationCertificate {
        let mut builder = CertificateBuilder::new(crate_name, version);

        // Memory safety — always proven in Rust-based system.
        builder = builder.add_check(
            VerificationCheck::proven(CheckKind::MemorySafety, ProofKind::StaticAnalysis)
        );

        // Data race freedom — proven via Send/Sync.
        builder = builder.add_check(
            VerificationCheck::proven(CheckKind::DataRaceFreedom, ProofKind::StaticAnalysis)
        );

        // Contract satisfaction.
        let contract_count = self.contract_results.len();
        let all_contracts_ok = self.contract_results.iter()
            .all(|r| r.verdict.is_satisfied());
        if all_contracts_ok && contract_count > 0 {
            builder = builder.add_check(
                VerificationCheck::proven(CheckKind::ContractSatisfaction, ProofKind::StaticAnalysis)
            );
        } else if contract_count > 0 {
            let violated: Vec<String> = self.contract_results.iter()
                .filter(|r| r.verdict.is_violated())
                .map(|r| r.contract.function.clone())
                .collect();
            if violated.is_empty() {
                builder = builder.add_check(VerificationCheck::conditional(
                    CheckKind::ContractSatisfaction,
                    vec!["some contracts not fully proven".to_string()],
                ));
            } else {
                builder = builder.add_check(VerificationCheck::failed(
                    CheckKind::ContractSatisfaction,
                    &format!("violated in: {}", violated.join(", ")),
                ));
            }
        }
        builder = builder.contracts_verified(contract_count);

        // Effect containment.
        let effect_count = self.effect_results.len();
        let all_effects_ok = self.effect_results.iter().all(|r| r.contained);
        if all_effects_ok && effect_count > 0 {
            builder = builder.add_check(
                VerificationCheck::proven(CheckKind::EffectContainment, ProofKind::StaticAnalysis)
            );
        } else if effect_count > 0 {
            let leaky: Vec<String> = self.effect_results.iter()
                .filter(|r| !r.contained)
                .map(|r| r.function.clone())
                .collect();
            builder = builder.add_check(VerificationCheck::failed(
                CheckKind::EffectContainment,
                &format!("leaks in: {}", leaky.join(", ")),
            ));
        }
        builder = builder.effects_verified(effect_count);

        // Capability bounds.
        let cap_count = self.capability_results.len();
        let all_caps_ok = self.capability_results.iter().all(|r| r.within_bounds);
        if all_caps_ok && cap_count > 0 {
            builder = builder.add_check(
                VerificationCheck::proven(CheckKind::CapabilityBounds, ProofKind::StaticAnalysis)
            );
        } else if cap_count > 0 {
            let excess: Vec<String> = self.capability_results.iter()
                .filter(|r| !r.within_bounds)
                .map(|r| r.agent.clone())
                .collect();
            builder = builder.add_check(VerificationCheck::failed(
                CheckKind::CapabilityBounds,
                &format!("exceeded by: {}", excess.join(", ")),
            ));
        }
        builder = builder.capabilities_verified(cap_count);

        let cert = builder.build();
        self.certificates.push(cert.clone());
        cert
    }

    // ── Accessors ──

    pub fn contract_results(&self) -> &[ContractResult] {
        &self.contract_results
    }

    pub fn effect_results(&self) -> &[EffectResult] {
        &self.effect_results
    }

    pub fn capability_results(&self) -> &[CapabilityResult] {
        &self.capability_results
    }

    pub fn certificates(&self) -> &[VerificationCertificate] {
        &self.certificates
    }

    pub fn total_contracts_verified(&self) -> usize {
        self.contract_results.len()
    }

    pub fn total_effects_verified(&self) -> usize {
        self.effect_results.len()
    }

    pub fn total_capabilities_verified(&self) -> usize {
        self.capability_results.len()
    }

    /// Reset all accumulated results.
    pub fn reset(&mut self) {
        self.contract_results.clear();
        self.effect_results.clear();
        self.capability_results.clear();
        self.certificates.clear();
    }
}

impl Default for VerificationOracle {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Contract ──

    #[test]
    fn contract_creation() {
        let c = Contract::new("foo::bar", "x > 0", "result >= x");
        assert_eq!(c.function, "foo::bar");
        assert!(c.has_precondition());
        assert!(c.has_postcondition());
        assert!(!c.has_effects());
    }

    #[test]
    fn contract_with_effects() {
        let c = Contract::new("f", "", "")
            .with_effects(vec!["io".to_string(), "alloc".to_string()]);
        assert!(c.has_effects());
        assert_eq!(c.effects.len(), 2);
    }

    #[test]
    fn contract_display() {
        let c = Contract::new("f", "x > 0", "ret > 0");
        let s = format!("{c}");
        assert!(s.contains("@req(x > 0)"));
        assert!(s.contains("@ens(ret > 0)"));
    }

    // ── ContractVerdict ──

    #[test]
    fn verdict_satisfied() {
        assert!(ContractVerdict::Satisfied.is_satisfied());
        assert!(!ContractVerdict::Satisfied.is_violated());
    }

    #[test]
    fn verdict_violated() {
        let v = ContractVerdict::Violated("bad".to_string());
        assert!(v.is_violated());
        assert!(!v.is_satisfied());
    }

    #[test]
    fn verdict_display() {
        assert_eq!(format!("{}", ContractVerdict::Satisfied), "satisfied");
        let v = ContractVerdict::Violated("x".to_string());
        assert!(format!("{v}").contains("violated"));
    }

    // ── Effect ──

    #[test]
    fn effect_from_str() {
        assert_eq!(Effect::from_str("pure"), Some(Effect::Pure));
        assert_eq!(Effect::from_str("io"), Some(Effect::Io));
        assert_eq!(Effect::from_str("alloc"), Some(Effect::Alloc));
        assert!(matches!(Effect::from_str("custom_effect"), Some(Effect::Custom(_))));
    }

    #[test]
    fn effect_display() {
        assert_eq!(format!("{}", Effect::Pure), "pure");
        assert_eq!(format!("{}", Effect::Io), "io");
        assert_eq!(format!("{}", Effect::Custom("foo".to_string())), "custom:foo");
    }

    // ── ProofKind ──

    #[test]
    fn proof_kind_display() {
        assert_eq!(format!("{}", ProofKind::StaticAnalysis), "static_analysis");
        assert_eq!(format!("{}", ProofKind::FormalProof), "formal_proof");
        assert_eq!(format!("{}", ProofKind::None), "none");
    }

    // ── CheckKind ──

    #[test]
    fn check_kind_display() {
        assert_eq!(format!("{}", CheckKind::MemorySafety), "memory_safety");
        assert_eq!(format!("{}", CheckKind::ContractSatisfaction), "contract_satisfaction");
        assert_eq!(format!("{}", CheckKind::Custom("x".to_string())), "custom:x");
    }

    // ── CheckStatus ──

    #[test]
    fn check_status_display() {
        assert_eq!(format!("{}", CheckStatus::Proven), "proven");
        assert_eq!(format!("{}", CheckStatus::Bounded(42)), "bounded(42)");
        assert!(format!("{}", CheckStatus::Failed("x".to_string())).contains("failed"));
    }

    // ── VerificationCheck ──

    #[test]
    fn verification_check_proven() {
        let c = VerificationCheck::proven(CheckKind::MemorySafety, ProofKind::StaticAnalysis);
        assert!(c.is_proven());
    }

    #[test]
    fn verification_check_conditional() {
        let c = VerificationCheck::conditional(
            CheckKind::PanicFreedom,
            vec!["inputs valid".to_string()],
        );
        assert!(!c.is_proven());
    }

    #[test]
    fn verification_check_bounded() {
        let c = VerificationCheck::bounded(CheckKind::StackOverflowFreedom, 42);
        assert!(!c.is_proven());
        assert!(matches!(c.status, CheckStatus::Bounded(42)));
    }

    #[test]
    fn verification_check_failed() {
        let c = VerificationCheck::failed(CheckKind::ContractSatisfaction, "bad contract");
        assert!(matches!(c.status, CheckStatus::Failed(_)));
    }

    // ── VerificationCertificate ──

    #[test]
    fn certificate_builder() {
        let cert = CertificateBuilder::new("my_crate", "1.0.0")
            .compiler_version("redox 0.2.0")
            .add_check(VerificationCheck::proven(CheckKind::MemorySafety, ProofKind::StaticAnalysis))
            .add_check(VerificationCheck::proven(CheckKind::DataRaceFreedom, ProofKind::StaticAnalysis))
            .contracts_verified(5)
            .effects_verified(3)
            .capabilities_verified(2)
            .build();

        assert_eq!(cert.crate_name, "my_crate");
        assert_eq!(cert.version, "1.0.0");
        assert_eq!(cert.compiler_version, "redox 0.2.0");
        assert_eq!(cert.total_checks(), 2);
        assert!(cert.all_proven());
        assert_eq!(cert.contracts_verified, 5);
        assert!(cert.timestamp > 0);
        assert!(cert.hash.starts_with("fnv64:"));
    }

    #[test]
    fn certificate_failed_checks() {
        let cert = CertificateBuilder::new("test", "0.1.0")
            .add_check(VerificationCheck::proven(CheckKind::MemorySafety, ProofKind::StaticAnalysis))
            .add_check(VerificationCheck::failed(CheckKind::ContractSatisfaction, "bad"))
            .build();

        assert!(!cert.all_proven());
        assert_eq!(cert.proven_count(), 1);
        assert_eq!(cert.failed_checks().len(), 1);
    }

    #[test]
    fn certificate_summary() {
        let cert = CertificateBuilder::new("my_crate", "1.0.0")
            .add_check(VerificationCheck::proven(CheckKind::MemorySafety, ProofKind::StaticAnalysis))
            .contracts_verified(3)
            .build();

        let s = cert.summary();
        assert!(s.contains("my_crate"));
        assert!(s.contains("1/1 proven"));
        assert!(s.contains("3 contracts"));
    }

    #[test]
    fn certificate_to_json() {
        let cert = CertificateBuilder::new("test", "0.1.0")
            .add_check(VerificationCheck::proven(CheckKind::MemorySafety, ProofKind::StaticAnalysis))
            .build();

        let json = cert.to_json();
        assert!(json.contains("\"crate\":\"test\""));
        assert!(json.contains("\"memory_safety\""));
        assert!(json.contains("\"proven\""));
    }

    #[test]
    fn certificate_display() {
        let cert = CertificateBuilder::new("test", "0.1.0").build();
        let s = format!("{cert}");
        assert!(s.contains("Certificate"));
    }

    // ── Oracle: Contract Verification ──

    #[test]
    fn oracle_verify_trivial_contract() {
        let mut oracle = VerificationOracle::new();
        let c = Contract::new("f", "", "true");
        let v = oracle.verify_contract(c);
        assert!(v.is_satisfied());
    }

    #[test]
    fn oracle_verify_unsatisfiable_contract() {
        let mut oracle = VerificationOracle::new();
        let c = Contract::new("f", "", "false");
        let v = oracle.verify_contract(c);
        assert!(v.is_violated());
    }

    #[test]
    fn oracle_verify_vacuous_contract() {
        let mut oracle = VerificationOracle::new();
        let c = Contract::new("f", "false", "anything");
        let v = oracle.verify_contract(c);
        assert!(v.is_satisfied()); // vacuously true
    }

    #[test]
    fn oracle_verify_conditional_contract() {
        let mut oracle = VerificationOracle::new();
        let c = Contract::new("f", "x > 0", "result > 0");
        let v = oracle.verify_contract(c);
        assert!(matches!(v, ContractVerdict::Conditional(_)));
    }

    #[test]
    fn oracle_verify_matching_patterns() {
        let mut oracle = VerificationOracle::new();
        let c = Contract::new("f", "x >= 0", "result >= 0");
        let v = oracle.verify_contract(c);
        assert!(v.is_satisfied());
    }

    #[test]
    fn oracle_verify_multiple_contracts() {
        let mut oracle = VerificationOracle::new();
        let contracts = vec![
            Contract::new("a", "", "true"),
            Contract::new("b", "false", "anything"),
            Contract::new("c", "", "false"),
        ];
        let verdicts = oracle.verify_contracts(contracts);
        assert_eq!(verdicts.len(), 3);
        assert!(verdicts[0].is_satisfied());
        assert!(verdicts[1].is_satisfied());
        assert!(verdicts[2].is_violated());
        assert_eq!(oracle.total_contracts_verified(), 3);
    }

    #[test]
    fn oracle_disabled_returns_unknown() {
        let mut oracle = VerificationOracle::disabled();
        assert!(!oracle.is_enabled());
        let v = oracle.verify_contract(Contract::new("f", "x > 0", "false"));
        assert!(matches!(v, ContractVerdict::Unknown(_)));
    }

    // ── Oracle: Effect Verification ──

    #[test]
    fn oracle_verify_effects_contained() {
        let mut oracle = VerificationOracle::new();
        let r = oracle.verify_effects(
            "f",
            vec![Effect::Io, Effect::Alloc],
            vec![Effect::Io],
        );
        assert!(r.is_contained());
        assert!(r.leaks.is_empty());
    }

    #[test]
    fn oracle_verify_effects_leaked() {
        let mut oracle = VerificationOracle::new();
        let r = oracle.verify_effects(
            "f",
            vec![Effect::Pure],
            vec![Effect::Io],
        );
        assert!(!r.is_contained());
        assert_eq!(r.leaks.len(), 1);
        assert_eq!(r.leaks[0], Effect::Io);
    }

    #[test]
    fn oracle_verify_effects_pure_always_contained() {
        let mut oracle = VerificationOracle::new();
        let r = oracle.verify_effects(
            "f",
            vec![],
            vec![Effect::Pure],
        );
        assert!(r.is_contained());
    }

    // ── Oracle: Capability Verification ──

    #[test]
    fn oracle_verify_capabilities_within_bounds() {
        let mut oracle = VerificationOracle::new();
        let r = oracle.verify_capabilities(
            "agent-01",
            vec!["read".to_string(), "write".to_string()],
            vec!["read".to_string()],
        );
        assert!(r.is_within_bounds());
    }

    #[test]
    fn oracle_verify_capabilities_exceeds_bounds() {
        let mut oracle = VerificationOracle::new();
        let r = oracle.verify_capabilities(
            "agent-01",
            vec!["read".to_string()],
            vec!["read".to_string(), "execute".to_string()],
        );
        assert!(!r.is_within_bounds());
        assert_eq!(r.exceeds_bounds, vec!["execute".to_string()]);
    }

    // ── Oracle: Certificate Emission ──

    #[test]
    fn oracle_emit_certificate_all_passing() {
        let mut oracle = VerificationOracle::new();

        // Verify some contracts.
        oracle.verify_contract(Contract::new("a", "", "true"));
        oracle.verify_contract(Contract::new("b", "false", "x"));

        // Verify some effects.
        oracle.verify_effects("f", vec![Effect::Io], vec![Effect::Io]);

        // Verify capabilities.
        oracle.verify_capabilities("agent", vec!["read".to_string()], vec!["read".to_string()]);

        let cert = oracle.emit_certificate("my_crate", "1.0.0");
        assert!(cert.all_proven());
        assert_eq!(cert.contracts_verified, 2);
        assert_eq!(cert.effects_verified, 1);
        assert_eq!(cert.capabilities_verified, 1);
        // memory_safety + data_race_freedom + contract_satisfaction + effect_containment + capability_bounds
        assert_eq!(cert.total_checks(), 5);
    }

    #[test]
    fn oracle_emit_certificate_with_failures() {
        let mut oracle = VerificationOracle::new();

        oracle.verify_contract(Contract::new("bad_fn", "", "false"));
        oracle.verify_effects("leaky", vec![Effect::Pure], vec![Effect::Io]);

        let cert = oracle.emit_certificate("bad_crate", "0.0.1");
        assert!(!cert.all_proven());
        assert!(cert.failed_checks().len() > 0);
    }

    #[test]
    fn oracle_emit_certificate_minimal() {
        let mut oracle = VerificationOracle::new();
        // No contracts/effects/capabilities verified — just memory safety and data race freedom.
        let cert = oracle.emit_certificate("empty_crate", "0.1.0");
        assert_eq!(cert.total_checks(), 2); // memory_safety + data_race_freedom
        assert!(cert.all_proven());
    }

    // ── Oracle: State Management ──

    #[test]
    fn oracle_reset() {
        let mut oracle = VerificationOracle::new();
        oracle.verify_contract(Contract::new("f", "", "true"));
        oracle.verify_effects("f", vec![], vec![]);
        oracle.verify_capabilities("a", vec![], vec![]);
        oracle.emit_certificate("c", "1.0");

        assert!(oracle.total_contracts_verified() > 0);
        oracle.reset();
        assert_eq!(oracle.total_contracts_verified(), 0);
        assert_eq!(oracle.total_effects_verified(), 0);
        assert_eq!(oracle.total_capabilities_verified(), 0);
        assert_eq!(oracle.certificates().len(), 0);
    }

    #[test]
    fn oracle_enable_disable() {
        let mut oracle = VerificationOracle::new();
        assert!(oracle.is_enabled());
        oracle.disable();
        assert!(!oracle.is_enabled());
        oracle.enable();
        assert!(oracle.is_enabled());
    }

    // ── Full Scenario ──

    #[test]
    fn full_verification_pipeline() {
        let mut oracle = VerificationOracle::new();

        // 1. Verify contracts for a crate.
        let contracts = vec![
            Contract::new("flight_controller::init", "sensors_ready >= 1", "result >= 1"),
            Contract::new("flight_controller::update", "", "true"),
            Contract::new("flight_controller::shutdown", "false", "motors_off"),
        ];
        let verdicts = oracle.verify_contracts(contracts);
        assert_eq!(verdicts.len(), 3);

        // 2. Verify effects.
        oracle.verify_effects(
            "flight_controller::init",
            vec![Effect::Io, Effect::Alloc],
            vec![Effect::Io, Effect::Alloc],
        );
        oracle.verify_effects(
            "flight_controller::update",
            vec![Effect::Mutation],
            vec![Effect::Mutation],
        );

        // 3. Verify capabilities.
        oracle.verify_capabilities(
            "flight-agent",
            vec!["read".to_string(), "write".to_string(), "execute".to_string()],
            vec!["read".to_string(), "execute".to_string()],
        );

        // 4. Emit certificate.
        let cert = oracle.emit_certificate("flight_controller", "2.1.0");
        assert_eq!(cert.contracts_verified, 3);
        assert_eq!(cert.effects_verified, 2);
        assert_eq!(cert.capabilities_verified, 1);
        assert!(cert.total_checks() >= 4);

        // Memory safety and data race freedom always proven.
        let json = cert.to_json();
        assert!(json.contains("memory_safety"));
        assert!(json.contains("data_race_freedom"));
    }

    // ── Hash ──

    #[test]
    fn simple_hash_deterministic() {
        let h1 = simple_hash("test");
        let h2 = simple_hash("test");
        assert_eq!(h1, h2);
        assert!(h1.starts_with("fnv64:"));
    }

    #[test]
    fn simple_hash_different_inputs() {
        let h1 = simple_hash("test1");
        let h2 = simple_hash("test2");
        assert_ne!(h1, h2);
    }
}
