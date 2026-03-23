//! # Verification Certificate Emission Pipeline
//!
//! Opt-in pipeline for safety-critical code that emits machine-checkable proofs.
//! Produces verification certificates with proof obligations, evidence, and
//! chain-of-trust metadata.

use std::collections::HashMap;
use std::fmt;

// ── Proof Obligations ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObligationKind {
    Precondition,
    Postcondition,
    Invariant,
    TypeSafety,
    MemorySafety,
    Termination,
    Overflow,
    BoundsCheck,
    NullSafety,
    DataRace,
    Custom(String),
}

impl fmt::Display for ObligationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Precondition => write!(f, "precondition"),
            Self::Postcondition => write!(f, "postcondition"),
            Self::Invariant => write!(f, "invariant"),
            Self::TypeSafety => write!(f, "type_safety"),
            Self::MemorySafety => write!(f, "memory_safety"),
            Self::Termination => write!(f, "termination"),
            Self::Overflow => write!(f, "overflow"),
            Self::BoundsCheck => write!(f, "bounds_check"),
            Self::NullSafety => write!(f, "null_safety"),
            Self::DataRace => write!(f, "data_race"),
            Self::Custom(s) => write!(f, "custom({s})"),
        }
    }
}

/// A single proof obligation.
#[derive(Debug, Clone)]
pub struct ProofObligation {
    pub id: String,
    pub kind: ObligationKind,
    pub description: String,
    pub source_location: String,
    pub status: ProofStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofStatus {
    Pending,
    Verified,
    Discharged,
    Failed,
    Timeout,
    Skipped,
}

impl fmt::Display for ProofStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Verified => write!(f, "verified"),
            Self::Discharged => write!(f, "discharged"),
            Self::Failed => write!(f, "FAILED"),
            Self::Timeout => write!(f, "timeout"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

impl ProofObligation {
    pub fn new(id: impl Into<String>, kind: ObligationKind, desc: impl Into<String>, loc: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            description: desc.into(),
            source_location: loc.into(),
            status: ProofStatus::Pending,
        }
    }

    pub fn is_resolved(&self) -> bool {
        matches!(self.status, ProofStatus::Verified | ProofStatus::Discharged | ProofStatus::Skipped)
    }
}

impl fmt::Display for ProofObligation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} ({}) at {} — {}", self.status, self.id, self.kind, self.source_location, self.description)
    }
}

// ── Evidence ─────────────────────────────────────────────────────────

/// Evidence supporting a proof.
#[derive(Debug, Clone)]
pub enum Evidence {
    TypeCheck { type_name: String, constraint: String },
    BorrowCheck { region: String },
    SMTResult { solver: String, result: String },
    TestEvidence { test_name: String, passed: bool },
    ManualAudit { auditor: String, notes: String },
    Axiom { name: String },
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeCheck { type_name, constraint } =>
                write!(f, "TypeCheck({type_name}: {constraint})"),
            Self::BorrowCheck { region } =>
                write!(f, "BorrowCheck({region})"),
            Self::SMTResult { solver, result } =>
                write!(f, "SMT({solver}: {result})"),
            Self::TestEvidence { test_name, passed } =>
                write!(f, "Test({test_name}: {})", if *passed { "pass" } else { "fail" }),
            Self::ManualAudit { auditor, notes } =>
                write!(f, "Audit({auditor}: {notes})"),
            Self::Axiom { name } =>
                write!(f, "Axiom({name})"),
        }
    }
}

// ── Verification Certificate ─────────────────────────────────────────

/// A machine-checkable verification certificate.
#[derive(Debug, Clone)]
pub struct VerificationCertificate {
    pub id: String,
    pub subject: String,
    pub version: String,
    pub obligations: Vec<ProofObligation>,
    pub evidence: Vec<(String, Evidence)>,
    pub trust_chain: Vec<TrustLink>,
    pub safety_level: SafetyLevel,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    Standard,
    SafetyCritical,
    HighAssurance,
    FormallyVerified,
}

impl fmt::Display for SafetyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::SafetyCritical => write!(f, "safety-critical"),
            Self::HighAssurance => write!(f, "high-assurance"),
            Self::FormallyVerified => write!(f, "formally-verified"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrustLink {
    pub issuer: String,
    pub subject: String,
    pub evidence_refs: Vec<String>,
}

impl fmt::Display for TrustLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {} ({} evidence)", self.issuer, self.subject, self.evidence_refs.len())
    }
}

impl VerificationCertificate {
    pub fn new(id: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            subject: subject.into(),
            version: "1.0".into(),
            obligations: Vec::new(),
            evidence: Vec::new(),
            trust_chain: Vec::new(),
            safety_level: SafetyLevel::Standard,
            metadata: HashMap::new(),
        }
    }

    pub fn with_safety_level(mut self, level: SafetyLevel) -> Self {
        self.safety_level = level;
        self
    }

    pub fn add_obligation(&mut self, obligation: ProofObligation) {
        self.obligations.push(obligation);
    }

    pub fn add_evidence(&mut self, obligation_id: impl Into<String>, evidence: Evidence) {
        self.evidence.push((obligation_id.into(), evidence));
    }

    pub fn add_trust_link(&mut self, link: TrustLink) {
        self.trust_chain.push(link);
    }

    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn total_obligations(&self) -> usize {
        self.obligations.len()
    }

    pub fn verified_count(&self) -> usize {
        self.obligations.iter().filter(|o| o.status == ProofStatus::Verified).count()
    }

    pub fn failed_count(&self) -> usize {
        self.obligations.iter().filter(|o| o.status == ProofStatus::Failed).count()
    }

    pub fn pending_count(&self) -> usize {
        self.obligations.iter().filter(|o| o.status == ProofStatus::Pending).count()
    }

    pub fn is_fully_verified(&self) -> bool {
        !self.obligations.is_empty() && self.obligations.iter().all(|o| o.is_resolved())
    }

    pub fn verification_ratio(&self) -> f64 {
        if self.obligations.is_empty() { return 0.0; }
        self.verified_count() as f64 / self.obligations.len() as f64
    }
}

impl fmt::Display for VerificationCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== Verification Certificate {} ===", self.id)?;
        writeln!(f, "Subject: {}", self.subject)?;
        writeln!(f, "Safety Level: {}", self.safety_level)?;
        writeln!(f, "Obligations: {}/{} verified", self.verified_count(), self.total_obligations())?;
        for ob in &self.obligations {
            writeln!(f, "  {ob}")?;
        }
        if !self.trust_chain.is_empty() {
            writeln!(f, "Trust Chain:")?;
            for link in &self.trust_chain {
                writeln!(f, "  {link}")?;
            }
        }
        Ok(())
    }
}

// ── Certificate Pipeline ─────────────────────────────────────────────

/// Pipeline stages for certificate emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    ObligationGeneration,
    StaticAnalysis,
    TypeChecking,
    BorrowChecking,
    SMTSolving,
    TestVerification,
    CertificateAssembly,
    SignatureGeneration,
}

impl fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObligationGeneration => write!(f, "obligation_gen"),
            Self::StaticAnalysis => write!(f, "static_analysis"),
            Self::TypeChecking => write!(f, "type_check"),
            Self::BorrowChecking => write!(f, "borrow_check"),
            Self::SMTSolving => write!(f, "smt_solve"),
            Self::TestVerification => write!(f, "test_verify"),
            Self::CertificateAssembly => write!(f, "cert_assembly"),
            Self::SignatureGeneration => write!(f, "signature_gen"),
        }
    }
}

/// Result from running a pipeline stage.
#[derive(Debug)]
pub struct StageResult {
    pub stage: PipelineStage,
    pub success: bool,
    pub obligations_resolved: usize,
    pub evidence_produced: Vec<Evidence>,
    pub message: String,
}

/// Run the obligation generation stage.
pub fn generate_obligations(subject: &str, safety_level: SafetyLevel) -> Vec<ProofObligation> {
    let mut obligations = Vec::new();
    let mut id_counter = 0u32;

    let mut add = |kind: ObligationKind, desc: &str| {
        obligations.push(ProofObligation::new(
            format!("OB-{id_counter:04}"),
            kind,
            desc,
            subject,
        ));
        id_counter += 1;
    };

    // Always: type safety and memory safety
    add(ObligationKind::TypeSafety, "All types are well-formed");
    add(ObligationKind::MemorySafety, "No use-after-free or dangling references");

    match safety_level {
        SafetyLevel::Standard => {}
        SafetyLevel::SafetyCritical => {
            add(ObligationKind::BoundsCheck, "All array accesses are within bounds");
            add(ObligationKind::Overflow, "No arithmetic overflow");
            add(ObligationKind::NullSafety, "No null pointer dereferences");
        }
        SafetyLevel::HighAssurance => {
            add(ObligationKind::BoundsCheck, "All array accesses are within bounds");
            add(ObligationKind::Overflow, "No arithmetic overflow");
            add(ObligationKind::NullSafety, "No null pointer dereferences");
            add(ObligationKind::DataRace, "No data races");
            add(ObligationKind::Termination, "All loops terminate");
        }
        SafetyLevel::FormallyVerified => {
            add(ObligationKind::BoundsCheck, "All array accesses are within bounds");
            add(ObligationKind::Overflow, "No arithmetic overflow");
            add(ObligationKind::NullSafety, "No null pointer dereferences");
            add(ObligationKind::DataRace, "No data races");
            add(ObligationKind::Termination, "All loops terminate");
            add(ObligationKind::Precondition, "All preconditions hold at call sites");
            add(ObligationKind::Postcondition, "All postconditions hold after returns");
            add(ObligationKind::Invariant, "All invariants are maintained");

        }
    }

    obligations
}

/// Simulate type-checking stage: resolves TypeSafety obligations.
pub fn run_type_check(cert: &mut VerificationCertificate) -> StageResult {
    let mut resolved = 0;
    let mut evidence = Vec::new();

    for ob in &mut cert.obligations {
        if ob.kind == ObligationKind::TypeSafety && ob.status == ProofStatus::Pending {
            ob.status = ProofStatus::Verified;
            resolved += 1;
            evidence.push(Evidence::TypeCheck {
                type_name: "all".into(),
                constraint: "well-typed".into(),
            });
        }
    }

    StageResult {
        stage: PipelineStage::TypeChecking,
        success: true,
        obligations_resolved: resolved,
        evidence_produced: evidence,
        message: format!("{resolved} type obligations verified"),
    }
}

/// Simulate borrow-checking stage: resolves MemorySafety obligations.
pub fn run_borrow_check(cert: &mut VerificationCertificate) -> StageResult {
    let mut resolved = 0;
    let mut evidence = Vec::new();

    for ob in &mut cert.obligations {
        if ob.kind == ObligationKind::MemorySafety && ob.status == ProofStatus::Pending {
            ob.status = ProofStatus::Verified;
            resolved += 1;
            evidence.push(Evidence::BorrowCheck {
                region: "all".into(),
            });
        }
    }

    StageResult {
        stage: PipelineStage::BorrowChecking,
        success: true,
        obligations_resolved: resolved,
        evidence_produced: evidence,
        message: format!("{resolved} memory obligations verified"),
    }
}

/// Simulate SMT solving: resolves remaining formal obligations.
pub fn run_smt_solver(cert: &mut VerificationCertificate) -> StageResult {
    let mut resolved = 0;
    let mut evidence = Vec::new();
    let solvable = [
        ObligationKind::BoundsCheck,
        ObligationKind::Overflow,
        ObligationKind::NullSafety,
        ObligationKind::DataRace,
        ObligationKind::Precondition,
        ObligationKind::Postcondition,
        ObligationKind::Invariant,
        ObligationKind::Termination,
    ];

    for ob in &mut cert.obligations {
        if solvable.contains(&ob.kind) && ob.status == ProofStatus::Pending {
            ob.status = ProofStatus::Verified;
            resolved += 1;
            evidence.push(Evidence::SMTResult {
                solver: "z3".into(),
                result: "sat".into(),
            });
        }
    }

    StageResult {
        stage: PipelineStage::SMTSolving,
        success: true,
        obligations_resolved: resolved,
        evidence_produced: evidence,
        message: format!("{resolved} obligations solved by SMT"),
    }
}

/// Run the full pipeline on a certificate.
pub fn run_pipeline(cert: &mut VerificationCertificate) -> Vec<StageResult> {
    let mut results = Vec::new();
    results.push(run_type_check(cert));
    results.push(run_borrow_check(cert));
    results.push(run_smt_solver(cert));
    results
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obligation_kind_display() {
        assert_eq!(format!("{}", ObligationKind::Precondition), "precondition");
        assert_eq!(format!("{}", ObligationKind::Custom("x".into())), "custom(x)");
    }

    #[test]
    fn test_proof_status_display() {
        assert_eq!(format!("{}", ProofStatus::Verified), "verified");
        assert_eq!(format!("{}", ProofStatus::Failed), "FAILED");
    }

    #[test]
    fn test_proof_obligation_new() {
        let ob = ProofObligation::new("OB-0001", ObligationKind::TypeSafety, "safe", "main.rs:10");
        assert_eq!(ob.status, ProofStatus::Pending);
        assert!(!ob.is_resolved());
    }

    #[test]
    fn test_proof_obligation_resolved() {
        let mut ob = ProofObligation::new("OB-0001", ObligationKind::TypeSafety, "safe", "main.rs");
        ob.status = ProofStatus::Verified;
        assert!(ob.is_resolved());
    }

    #[test]
    fn test_proof_obligation_display() {
        let ob = ProofObligation::new("OB-0001", ObligationKind::MemorySafety, "no uaf", "main.rs");
        let s = format!("{ob}");
        assert!(s.contains("OB-0001"));
        assert!(s.contains("memory_safety"));
    }

    #[test]
    fn test_evidence_display() {
        let e = Evidence::TypeCheck { type_name: "i32".into(), constraint: "well-typed".into() };
        assert!(format!("{e}").contains("TypeCheck"));
    }

    #[test]
    fn test_evidence_smt() {
        let e = Evidence::SMTResult { solver: "z3".into(), result: "sat".into() };
        assert!(format!("{e}").contains("z3"));
    }

    #[test]
    fn test_evidence_test() {
        let e = Evidence::TestEvidence { test_name: "t1".into(), passed: true };
        assert!(format!("{e}").contains("pass"));
    }

    #[test]
    fn test_evidence_audit() {
        let e = Evidence::ManualAudit { auditor: "bob".into(), notes: "ok".into() };
        assert!(format!("{e}").contains("bob"));
    }

    #[test]
    fn test_evidence_axiom() {
        let e = Evidence::Axiom { name: "refl".into() };
        assert!(format!("{e}").contains("refl"));
    }

    #[test]
    fn test_safety_level_display() {
        assert_eq!(format!("{}", SafetyLevel::FormallyVerified), "formally-verified");
    }

    #[test]
    fn test_trust_link_display() {
        let link = TrustLink { issuer: "A".into(), subject: "B".into(), evidence_refs: vec!["e1".into()] };
        assert!(format!("{link}").contains("A -> B"));
    }

    #[test]
    fn test_certificate_new() {
        let cert = VerificationCertificate::new("CERT-001", "my_module");
        assert_eq!(cert.total_obligations(), 0);
        assert_eq!(cert.safety_level, SafetyLevel::Standard);
    }

    #[test]
    fn test_certificate_add_obligation() {
        let mut cert = VerificationCertificate::new("CERT-001", "m");
        cert.add_obligation(ProofObligation::new("OB-0001", ObligationKind::TypeSafety, "d", "l"));
        assert_eq!(cert.total_obligations(), 1);
        assert_eq!(cert.pending_count(), 1);
    }

    #[test]
    fn test_certificate_verified_count() {
        let mut cert = VerificationCertificate::new("C", "m");
        let mut ob = ProofObligation::new("O", ObligationKind::TypeSafety, "d", "l");
        ob.status = ProofStatus::Verified;
        cert.add_obligation(ob);
        assert_eq!(cert.verified_count(), 1);
    }

    #[test]
    fn test_certificate_is_fully_verified() {
        let mut cert = VerificationCertificate::new("C", "m");
        let mut ob = ProofObligation::new("O", ObligationKind::TypeSafety, "d", "l");
        ob.status = ProofStatus::Verified;
        cert.add_obligation(ob);
        assert!(cert.is_fully_verified());
    }

    #[test]
    fn test_certificate_not_fully_verified() {
        let mut cert = VerificationCertificate::new("C", "m");
        cert.add_obligation(ProofObligation::new("O", ObligationKind::TypeSafety, "d", "l"));
        assert!(!cert.is_fully_verified());
    }

    #[test]
    fn test_certificate_ratio() {
        let mut cert = VerificationCertificate::new("C", "m");
        let mut ob1 = ProofObligation::new("O1", ObligationKind::TypeSafety, "d", "l");
        ob1.status = ProofStatus::Verified;
        cert.add_obligation(ob1);
        cert.add_obligation(ProofObligation::new("O2", ObligationKind::MemorySafety, "d", "l"));
        assert!((cert.verification_ratio() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_certificate_empty_ratio() {
        let cert = VerificationCertificate::new("C", "m");
        assert_eq!(cert.verification_ratio(), 0.0);
    }

    #[test]
    fn test_certificate_display() {
        let cert = VerificationCertificate::new("CERT-001", "my_mod")
            .with_safety_level(SafetyLevel::HighAssurance);
        let s = format!("{cert}");
        assert!(s.contains("CERT-001"));
        assert!(s.contains("high-assurance"));
    }

    #[test]
    fn test_pipeline_stage_display() {
        assert_eq!(format!("{}", PipelineStage::SMTSolving), "smt_solve");
    }

    #[test]
    fn test_generate_obligations_standard() {
        let obs = generate_obligations("main.rs", SafetyLevel::Standard);
        assert_eq!(obs.len(), 2); // type + memory only
    }

    #[test]
    fn test_generate_obligations_safety_critical() {
        let obs = generate_obligations("main.rs", SafetyLevel::SafetyCritical);
        assert_eq!(obs.len(), 5);
    }

    #[test]
    fn test_generate_obligations_high_assurance() {
        let obs = generate_obligations("main.rs", SafetyLevel::HighAssurance);
        assert_eq!(obs.len(), 7);
    }

    #[test]
    fn test_generate_obligations_formally_verified() {
        let obs = generate_obligations("main.rs", SafetyLevel::FormallyVerified);
        assert_eq!(obs.len(), 10);
    }

    #[test]
    fn test_run_type_check() {
        let mut cert = VerificationCertificate::new("C", "m");
        for ob in generate_obligations("m", SafetyLevel::Standard) {
            cert.add_obligation(ob);
        }
        let result = run_type_check(&mut cert);
        assert!(result.success);
        assert_eq!(result.obligations_resolved, 1);
    }

    #[test]
    fn test_run_borrow_check() {
        let mut cert = VerificationCertificate::new("C", "m");
        for ob in generate_obligations("m", SafetyLevel::Standard) {
            cert.add_obligation(ob);
        }
        let result = run_borrow_check(&mut cert);
        assert!(result.success);
        assert_eq!(result.obligations_resolved, 1);
    }

    #[test]
    fn test_run_pipeline_standard() {
        let mut cert = VerificationCertificate::new("C", "m");
        for ob in generate_obligations("m", SafetyLevel::Standard) {
            cert.add_obligation(ob);
        }
        let results = run_pipeline(&mut cert);
        assert_eq!(results.len(), 3);
        assert!(cert.is_fully_verified());
    }

    #[test]
    fn test_run_pipeline_formally_verified() {
        let mut cert = VerificationCertificate::new("C", "m")
            .with_safety_level(SafetyLevel::FormallyVerified);
        for ob in generate_obligations("m", SafetyLevel::FormallyVerified) {
            cert.add_obligation(ob);
        }
        let _results = run_pipeline(&mut cert);
        assert!(cert.is_fully_verified());
    }

    #[test]
    fn test_certificate_metadata() {
        let mut cert = VerificationCertificate::new("C", "m");
        cert.set_metadata("compiler", "redox-2026");
        assert_eq!(cert.metadata.get("compiler").unwrap(), "redox-2026");
    }

    #[test]
    fn test_certificate_trust_chain() {
        let mut cert = VerificationCertificate::new("C", "m");
        cert.add_trust_link(TrustLink {
            issuer: "type_checker".into(),
            subject: "m".into(),
            evidence_refs: vec!["e1".into()],
        });
        assert_eq!(cert.trust_chain.len(), 1);
    }

    #[test]
    fn test_certificate_evidence() {
        let mut cert = VerificationCertificate::new("C", "m");
        cert.add_evidence("OB-0001", Evidence::Axiom { name: "refl".into() });
        assert_eq!(cert.evidence.len(), 1);
    }
}
