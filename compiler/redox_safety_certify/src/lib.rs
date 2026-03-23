// redox_safety_certify: Certification pipeline for safety-critical industries.
//
//  Provides an opt-in full-safety mode that runs a sequence of certification
//  stages (static analysis, formal verification, fuzz testing, MISRA/DO-178C
//  compliance checks) and produces an auditable SafetyCertificate.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Industry standards
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SafetyStandard {
    Do178C,       // avionics
    Iec61508,     // industrial
    Iso26262,     // automotive
    Iec62304,     // medical devices
    MisraC2023,   // coding rules
    CertC,        // CERT C secure coding
    AutoSar,      // automotive AUTOSAR C++14
    Generic,      // baseline
}

impl SafetyStandard {
    pub fn label(self) -> &'static str {
        match self {
            Self::Do178C => "DO-178C",
            Self::Iec61508 => "IEC 61508",
            Self::Iso26262 => "ISO 26262",
            Self::Iec62304 => "IEC 62304",
            Self::MisraC2023 => "MISRA C:2023",
            Self::CertC => "CERT C",
            Self::AutoSar => "AUTOSAR C++14",
            Self::Generic => "Generic Safety",
        }
    }
}

// ---------------------------------------------------------------------------
// Assurance levels
// ---------------------------------------------------------------------------

/// DAL (Design Assurance Level) for DO-178C or equivalent SIL/ASIL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssuranceLevel {
    DalE, // lowest
    DalD,
    DalC,
    DalB,
    DalA, // highest
}

impl AssuranceLevel {
    pub fn label(self) -> &'static str {
        match self {
            Self::DalE => "DAL-E",
            Self::DalD => "DAL-D",
            Self::DalC => "DAL-C",
            Self::DalB => "DAL-B",
            Self::DalA => "DAL-A",
        }
    }

    /// Required coverage percentage for the level.
    pub fn required_coverage(self) -> f64 {
        match self {
            Self::DalE => 0.0,
            Self::DalD => 50.0,
            Self::DalC => 80.0,
            Self::DalB => 95.0,
            Self::DalA => 100.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline stages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CertStage {
    StaticAnalysis,
    FormalVerification,
    FuzzTesting,
    CodingStandardCheck,
    CoverageAnalysis,
    TraceabilityAudit,
    DocumentGeneration,
}

impl CertStage {
    pub fn label(self) -> &'static str {
        match self {
            Self::StaticAnalysis => "Static Analysis",
            Self::FormalVerification => "Formal Verification",
            Self::FuzzTesting => "Fuzz Testing",
            Self::CodingStandardCheck => "Coding Standard Check",
            Self::CoverageAnalysis => "Coverage Analysis",
            Self::TraceabilityAudit => "Traceability Audit",
            Self::DocumentGeneration => "Document Generation",
        }
    }
}

// ---------------------------------------------------------------------------
// Stage result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum StageVerdict {
    Pass,
    PassWithWarnings(Vec<String>),
    Fail(Vec<String>),
    Skipped,
}

impl StageVerdict {
    pub fn is_pass(&self) -> bool {
        matches!(self, Self::Pass | Self::PassWithWarnings(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StageResult {
    pub stage: CertStage,
    pub verdict: StageVerdict,
    pub coverage_pct: Option<f64>,
    pub findings_count: usize,
}

// ---------------------------------------------------------------------------
// Certification profile
// ---------------------------------------------------------------------------

/// A certification profile selects which stages are required and at what
/// assurance level, for a given standard.
#[derive(Debug, Clone, PartialEq)]
pub struct CertProfile {
    pub standard: SafetyStandard,
    pub level: AssuranceLevel,
    pub required_stages: Vec<CertStage>,
}

pub fn profile_for(standard: SafetyStandard, level: AssuranceLevel) -> CertProfile {
    let base = vec![
        CertStage::StaticAnalysis,
        CertStage::CodingStandardCheck,
    ];

    let mut stages = base;

    if level >= AssuranceLevel::DalC {
        stages.push(CertStage::CoverageAnalysis);
        stages.push(CertStage::FuzzTesting);
    }
    if level >= AssuranceLevel::DalB {
        stages.push(CertStage::FormalVerification);
        stages.push(CertStage::TraceabilityAudit);
    }
    if level >= AssuranceLevel::DalA {
        stages.push(CertStage::DocumentGeneration);
    }

    CertProfile {
        standard,
        level,
        required_stages: stages,
    }
}

// ---------------------------------------------------------------------------
// Safety certificate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct SafetyCertificate {
    pub crate_name: String,
    pub profile: CertProfile,
    pub results: Vec<StageResult>,
    pub overall_pass: bool,
    pub coverage_pct: f64,
}

impl SafetyCertificate {
    pub fn failed_stages(&self) -> Vec<&StageResult> {
        self.results.iter().filter(|r| !r.verdict.is_pass() && r.verdict != StageVerdict::Skipped).collect()
    }

    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        for r in &self.results {
            if let StageVerdict::PassWithWarnings(ws) = &r.verdict {
                for w in ws {
                    out.push(format!("[{}] {}", r.stage.label(), w));
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Pipeline runner (simulated)
// ---------------------------------------------------------------------------

fn run_static_analysis(crate_name: &str) -> StageResult {
    // Simulate: pass with a warning about unsafe
    let _ = crate_name;
    StageResult {
        stage: CertStage::StaticAnalysis,
        verdict: StageVerdict::PassWithWarnings(vec![
            "1 unsafe block found — requires manual justification".into(),
        ]),
        coverage_pct: None,
        findings_count: 1,
    }
}

fn run_formal_verification(_crate_name: &str) -> StageResult {
    StageResult {
        stage: CertStage::FormalVerification,
        verdict: StageVerdict::Pass,
        coverage_pct: None,
        findings_count: 0,
    }
}

fn run_fuzz_testing(_crate_name: &str) -> StageResult {
    StageResult {
        stage: CertStage::FuzzTesting,
        verdict: StageVerdict::Pass,
        coverage_pct: None,
        findings_count: 0,
    }
}

fn run_coding_standard_check(standard: SafetyStandard) -> StageResult {
    let _ = standard;
    StageResult {
        stage: CertStage::CodingStandardCheck,
        verdict: StageVerdict::Pass,
        coverage_pct: None,
        findings_count: 0,
    }
}

fn run_coverage_analysis(level: AssuranceLevel) -> StageResult {
    // Simulate measured coverage
    let measured = match level {
        AssuranceLevel::DalA => 100.0,
        AssuranceLevel::DalB => 97.0,
        AssuranceLevel::DalC => 85.0,
        _ => 60.0,
    };
    let required = level.required_coverage();
    let pass = measured >= required;
    StageResult {
        stage: CertStage::CoverageAnalysis,
        verdict: if pass { StageVerdict::Pass } else {
            StageVerdict::Fail(vec![format!("Coverage {measured:.1}% < required {required:.1}%")])
        },
        coverage_pct: Some(measured),
        findings_count: if pass { 0 } else { 1 },
    }
}

fn run_traceability_audit(_crate_name: &str) -> StageResult {
    StageResult {
        stage: CertStage::TraceabilityAudit,
        verdict: StageVerdict::Pass,
        coverage_pct: None,
        findings_count: 0,
    }
}

fn run_document_generation(_crate_name: &str) -> StageResult {
    StageResult {
        stage: CertStage::DocumentGeneration,
        verdict: StageVerdict::Pass,
        coverage_pct: None,
        findings_count: 0,
    }
}

fn run_stage(crate_name: &str, stage: CertStage, profile: &CertProfile) -> StageResult {
    match stage {
        CertStage::StaticAnalysis => run_static_analysis(crate_name),
        CertStage::FormalVerification => run_formal_verification(crate_name),
        CertStage::FuzzTesting => run_fuzz_testing(crate_name),
        CertStage::CodingStandardCheck => run_coding_standard_check(profile.standard),
        CertStage::CoverageAnalysis => run_coverage_analysis(profile.level),
        CertStage::TraceabilityAudit => run_traceability_audit(crate_name),
        CertStage::DocumentGeneration => run_document_generation(crate_name),
    }
}

/// Run the full certification pipeline for a crate and return a certificate.
pub fn certify(crate_name: &str, standard: SafetyStandard, level: AssuranceLevel) -> SafetyCertificate {
    let profile = profile_for(standard, level);
    let mut results = Vec::new();
    for &stage in &profile.required_stages {
        results.push(run_stage(crate_name, stage, &profile));
    }
    let overall_pass = results.iter().all(|r| r.verdict.is_pass());
    let coverage_pct = results.iter()
        .filter_map(|r| r.coverage_pct)
        .last()
        .unwrap_or(0.0);
    SafetyCertificate {
        crate_name: crate_name.to_string(),
        profile,
        results,
        overall_pass,
        coverage_pct,
    }
}

// ---------------------------------------------------------------------------
// Batch certification
// ---------------------------------------------------------------------------

pub fn certify_batch(
    crates: &[&str],
    standard: SafetyStandard,
    level: AssuranceLevel,
) -> Vec<SafetyCertificate> {
    crates.iter().map(|c| certify(c, standard, level)).collect()
}

// ---------------------------------------------------------------------------
// Summary report
// ---------------------------------------------------------------------------

pub fn summary_report(certs: &[SafetyCertificate]) -> String {
    let mut out = String::new();
    out.push_str("=== Safety Certification Summary ===\n");
    for c in certs {
        let status = if c.overall_pass { "PASS" } else { "FAIL" };
        out.push_str(&format!(
            "  {} [{}] {} — {}\n",
            c.crate_name,
            c.profile.profile_label(),
            status,
            c.coverage_pct,
        ));
    }
    let total = certs.len();
    let passed = certs.iter().filter(|c| c.overall_pass).count();
    out.push_str(&format!("Total: {passed}/{total} passed\n"));
    out
}

impl CertProfile {
    pub fn profile_label(&self) -> String {
        format!("{} {}", self.standard.label(), self.level.label())
    }
}

// ---------------------------------------------------------------------------
// Compliance matrix
// ---------------------------------------------------------------------------

pub struct ComplianceMatrix {
    entries: HashMap<(String, SafetyStandard), SafetyCertificate>,
}

impl ComplianceMatrix {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    pub fn insert(&mut self, cert: SafetyCertificate) {
        let key = (cert.crate_name.clone(), cert.profile.standard);
        self.entries.insert(key, cert);
    }

    pub fn get(&self, crate_name: &str, standard: SafetyStandard) -> Option<&SafetyCertificate> {
        self.entries.get(&(crate_name.to_string(), standard))
    }

    pub fn all_passing(&self) -> bool {
        self.entries.values().all(|c| c.overall_pass)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn by_standard(&self, standard: SafetyStandard) -> Vec<&SafetyCertificate> {
        self.entries.values().filter(|c| c.profile.standard == standard).collect()
    }
}

impl Default for ComplianceMatrix {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- SafetyStandard --
    #[test]
    fn test_standard_labels() {
        assert_eq!(SafetyStandard::Do178C.label(), "DO-178C");
        assert_eq!(SafetyStandard::Iso26262.label(), "ISO 26262");
        assert_eq!(SafetyStandard::MisraC2023.label(), "MISRA C:2023");
        assert_eq!(SafetyStandard::Generic.label(), "Generic Safety");
    }

    // -- AssuranceLevel --
    #[test]
    fn test_assurance_ordering() {
        assert!(AssuranceLevel::DalA > AssuranceLevel::DalB);
        assert!(AssuranceLevel::DalB > AssuranceLevel::DalC);
        assert!(AssuranceLevel::DalC > AssuranceLevel::DalD);
        assert!(AssuranceLevel::DalD > AssuranceLevel::DalE);
    }

    #[test]
    fn test_assurance_coverage() {
        assert_eq!(AssuranceLevel::DalA.required_coverage(), 100.0);
        assert_eq!(AssuranceLevel::DalE.required_coverage(), 0.0);
    }

    // -- profile_for --
    #[test]
    fn test_profile_dal_e_minimal() {
        let p = profile_for(SafetyStandard::Generic, AssuranceLevel::DalE);
        assert_eq!(p.required_stages.len(), 2); // static + coding std
    }

    #[test]
    fn test_profile_dal_c_adds_coverage_fuzz() {
        let p = profile_for(SafetyStandard::Iso26262, AssuranceLevel::DalC);
        assert!(p.required_stages.contains(&CertStage::CoverageAnalysis));
        assert!(p.required_stages.contains(&CertStage::FuzzTesting));
    }

    #[test]
    fn test_profile_dal_b_adds_formal_trace() {
        let p = profile_for(SafetyStandard::Do178C, AssuranceLevel::DalB);
        assert!(p.required_stages.contains(&CertStage::FormalVerification));
        assert!(p.required_stages.contains(&CertStage::TraceabilityAudit));
    }

    #[test]
    fn test_profile_dal_a_full() {
        let p = profile_for(SafetyStandard::Do178C, AssuranceLevel::DalA);
        assert_eq!(p.required_stages.len(), 7);
        assert!(p.required_stages.contains(&CertStage::DocumentGeneration));
    }

    #[test]
    fn test_profile_label() {
        let p = profile_for(SafetyStandard::Do178C, AssuranceLevel::DalA);
        assert_eq!(p.profile_label(), "DO-178C DAL-A");
    }

    // -- StageVerdict --
    #[test]
    fn test_verdict_is_pass() {
        assert!(StageVerdict::Pass.is_pass());
        assert!(StageVerdict::PassWithWarnings(vec!["w".into()]).is_pass());
        assert!(!StageVerdict::Fail(vec!["e".into()]).is_pass());
        assert!(!StageVerdict::Skipped.is_pass());
    }

    // -- certify --
    #[test]
    fn test_certify_dal_e_passes() {
        let cert = certify("mylib", SafetyStandard::Generic, AssuranceLevel::DalE);
        assert!(cert.overall_pass);
        assert_eq!(cert.results.len(), 2);
    }

    #[test]
    fn test_certify_dal_a_passes() {
        let cert = certify("mylib", SafetyStandard::Do178C, AssuranceLevel::DalA);
        assert!(cert.overall_pass);
        assert_eq!(cert.results.len(), 7);
        assert_eq!(cert.coverage_pct, 100.0);
    }

    #[test]
    fn test_certify_crate_name_recorded() {
        let cert = certify("safety_core", SafetyStandard::Iec61508, AssuranceLevel::DalC);
        assert_eq!(cert.crate_name, "safety_core");
    }

    #[test]
    fn test_certify_warnings_collected() {
        let cert = certify("x", SafetyStandard::Generic, AssuranceLevel::DalE);
        let ws = cert.warnings();
        assert_eq!(ws.len(), 1);
        assert!(ws[0].contains("unsafe"));
    }

    #[test]
    fn test_certify_no_failed_stages_dal_a() {
        let cert = certify("x", SafetyStandard::Do178C, AssuranceLevel::DalA);
        assert!(cert.failed_stages().is_empty());
    }

    // -- certify_batch --
    #[test]
    fn test_certify_batch() {
        let certs = certify_batch(&["a", "b", "c"], SafetyStandard::Generic, AssuranceLevel::DalE);
        assert_eq!(certs.len(), 3);
        assert!(certs.iter().all(|c| c.overall_pass));
    }

    // -- summary_report --
    #[test]
    fn test_summary_report_format() {
        let certs = certify_batch(&["alpha"], SafetyStandard::Do178C, AssuranceLevel::DalA);
        let report = summary_report(&certs);
        assert!(report.contains("alpha"));
        assert!(report.contains("PASS"));
        assert!(report.contains("1/1 passed"));
    }

    // -- ComplianceMatrix --
    #[test]
    fn test_matrix_empty() {
        let m = ComplianceMatrix::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
        assert!(m.all_passing()); // vacuously true
    }

    #[test]
    fn test_matrix_insert_get() {
        let mut m = ComplianceMatrix::new();
        let cert = certify("x", SafetyStandard::Iso26262, AssuranceLevel::DalC);
        m.insert(cert);
        assert_eq!(m.len(), 1);
        assert!(m.get("x", SafetyStandard::Iso26262).is_some());
        assert!(m.get("x", SafetyStandard::Do178C).is_none());
    }

    #[test]
    fn test_matrix_all_passing() {
        let mut m = ComplianceMatrix::new();
        m.insert(certify("a", SafetyStandard::Generic, AssuranceLevel::DalE));
        m.insert(certify("b", SafetyStandard::Do178C, AssuranceLevel::DalA));
        assert!(m.all_passing());
    }

    #[test]
    fn test_matrix_by_standard() {
        let mut m = ComplianceMatrix::new();
        m.insert(certify("a", SafetyStandard::Do178C, AssuranceLevel::DalA));
        m.insert(certify("b", SafetyStandard::Iso26262, AssuranceLevel::DalC));
        m.insert(certify("c", SafetyStandard::Do178C, AssuranceLevel::DalB));
        assert_eq!(m.by_standard(SafetyStandard::Do178C).len(), 2);
        assert_eq!(m.by_standard(SafetyStandard::Iso26262).len(), 1);
    }

    #[test]
    fn test_matrix_default() {
        let m = ComplianceMatrix::default();
        assert!(m.is_empty());
    }

    // -- individual stage runners --
    #[test]
    fn test_static_analysis_warns() {
        let r = run_static_analysis("x");
        assert!(r.verdict.is_pass());
        assert_eq!(r.findings_count, 1);
    }

    #[test]
    fn test_coverage_dal_d_passes() {
        let r = run_coverage_analysis(AssuranceLevel::DalD);
        // measured 60% >= required 50%
        assert!(r.verdict.is_pass());
    }

    #[test]
    fn test_coverage_dal_a_passes() {
        let r = run_coverage_analysis(AssuranceLevel::DalA);
        assert!(r.verdict.is_pass());
        assert_eq!(r.coverage_pct, Some(100.0));
    }

    #[test]
    fn test_formal_verification_passes() {
        let r = run_formal_verification("x");
        assert_eq!(r.verdict, StageVerdict::Pass);
    }

    #[test]
    fn test_fuzz_testing_passes() {
        let r = run_fuzz_testing("x");
        assert_eq!(r.verdict, StageVerdict::Pass);
    }

    #[test]
    fn test_traceability_audit_passes() {
        let r = run_traceability_audit("x");
        assert_eq!(r.verdict, StageVerdict::Pass);
    }

    #[test]
    fn test_document_generation_passes() {
        let r = run_document_generation("x");
        assert_eq!(r.verdict, StageVerdict::Pass);
    }

    #[test]
    fn test_cert_stage_labels() {
        assert_eq!(CertStage::StaticAnalysis.label(), "Static Analysis");
        assert_eq!(CertStage::DocumentGeneration.label(), "Document Generation");
    }
}
