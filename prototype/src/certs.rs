// ── Verification Certificates ──────────────────────────────────────
//
// Machine-checkable proofs attesting that verified properties hold.
//
// Certificate kinds:
//   - MemorySafety     — no dangling pointers, no use-after-free
//   - DataRaceFreedom  — no unsynchronised shared mutable access
//   - ContractSatisfaction — all @req/@ens/@inv hold
//   - EffectContainment — all effects declared and bounded
//
// Each certificate carries evidence (proof steps), a verifier id,
// and can be serialised/checked independently.

use std::collections::BTreeMap;

// ── Certificate Kind ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CertKind {
    MemorySafety,
    DataRaceFreedom,
    ContractSatisfaction,
    EffectContainment,
}

impl std::fmt::Display for CertKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CertKind::MemorySafety => write!(f, "memory-safety"),
            CertKind::DataRaceFreedom => write!(f, "data-race-freedom"),
            CertKind::ContractSatisfaction => write!(f, "contract-satisfaction"),
            CertKind::EffectContainment => write!(f, "effect-containment"),
        }
    }
}

// ── Proof Step ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofStep {
    /// Axiom: something assumed true.
    Axiom(String),
    /// Derived from preceding steps by a named rule.
    Derivation { rule: String, premises: Vec<usize>, conclusion: String },
    /// External witness (e.g., borrow-checker pass result).
    Witness { source: String, claim: String },
}

impl ProofStep {
    pub fn conclusion(&self) -> &str {
        match self {
            ProofStep::Axiom(s) => s,
            ProofStep::Derivation { conclusion, .. } => conclusion,
            ProofStep::Witness { claim, .. } => claim,
        }
    }
}

// ── Certificate ────────────────────────────────────────────────────

pub type CertId = u64;

#[derive(Debug, Clone)]
pub struct Certificate {
    pub id: CertId,
    pub kind: CertKind,
    pub target: String,          // function/module name
    pub verifier: String,        // which pass produced this
    pub steps: Vec<ProofStep>,
    pub timestamp: u64,
    pub valid: bool,
}

impl Certificate {
    /// Number of proof steps.
    pub fn proof_depth(&self) -> usize {
        self.steps.len()
    }

    /// Final conclusion string.
    pub fn final_conclusion(&self) -> Option<&str> {
        self.steps.last().map(|s| s.conclusion())
    }

    /// Check internal consistency: derivation premises must reference earlier steps.
    pub fn check_consistency(&self) -> Result<(), String> {
        for (i, step) in self.steps.iter().enumerate() {
            if let ProofStep::Derivation { premises, rule, .. } = step {
                for &p in premises {
                    if p >= i {
                        return Err(format!(
                            "step {} ({}) references non-preceding step {}",
                            i, rule, p
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

// ── Certificate Store ──────────────────────────────────────────────

pub struct CertificateStore {
    certs: BTreeMap<CertId, Certificate>,
    next_id: CertId,
    /// Index: (target, kind) → cert ids.
    index: BTreeMap<(String, CertKind), Vec<CertId>>,
}

impl CertificateStore {
    pub fn new() -> Self {
        Self {
            certs: BTreeMap::new(),
            next_id: 1,
            index: BTreeMap::new(),
        }
    }

    /// Issue a new certificate. Checks consistency before storing.
    pub fn issue(&mut self, kind: CertKind, target: &str, verifier: &str, steps: Vec<ProofStep>, timestamp: u64) -> Result<CertId, String> {
        let id = self.next_id;
        let cert = Certificate {
            id,
            kind,
            target: target.into(),
            verifier: verifier.into(),
            steps,
            timestamp,
            valid: true,
        };
        cert.check_consistency()?;
        self.next_id += 1;
        self.index.entry((target.into(), kind)).or_default().push(id);
        self.certs.insert(id, cert);
        Ok(id)
    }

    /// Revoke a certificate.
    pub fn revoke(&mut self, id: CertId) -> Result<(), String> {
        let cert = self.certs.get_mut(&id).ok_or("certificate not found")?;
        cert.valid = false;
        Ok(())
    }

    /// Get a certificate by id.
    pub fn get(&self, id: CertId) -> Option<&Certificate> {
        self.certs.get(&id)
    }

    /// Find all valid certificates for a target.
    pub fn certs_for(&self, target: &str) -> Vec<&Certificate> {
        self.certs.values()
            .filter(|c| c.target == target && c.valid)
            .collect()
    }

    /// Find certificates by kind.
    pub fn certs_by_kind(&self, kind: CertKind) -> Vec<&Certificate> {
        self.certs.values()
            .filter(|c| c.kind == kind && c.valid)
            .collect()
    }

    /// Check if a target has all four certificate kinds.
    pub fn fully_certified(&self, target: &str) -> bool {
        let kinds = [
            CertKind::MemorySafety,
            CertKind::DataRaceFreedom,
            CertKind::ContractSatisfaction,
            CertKind::EffectContainment,
        ];
        kinds.iter().all(|k| {
            self.index.get(&(target.into(), *k))
                .map(|ids| ids.iter().any(|id| self.certs.get(id).map_or(false, |c| c.valid)))
                .unwrap_or(false)
        })
    }

    /// Total valid certificates.
    pub fn valid_count(&self) -> usize {
        self.certs.values().filter(|c| c.valid).count()
    }

    /// Total certificates (including revoked).
    pub fn total_count(&self) -> usize {
        self.certs.len()
    }

    /// JSON summary.
    pub fn to_json(&self) -> String {
        let by_kind: BTreeMap<String, usize> = self.certs.values()
            .filter(|c| c.valid)
            .fold(BTreeMap::new(), |mut m, c| {
                *m.entry(c.kind.to_string()).or_insert(0) += 1;
                m
            });
        let entries: Vec<String> = by_kind.iter()
            .map(|(k, v)| format!("\"{}\":{}", k, v))
            .collect();
        format!("{{\"total\":{},\"valid\":{},\"by_kind\":{{{}}}}}", self.total_count(), self.valid_count(), entries.join(","))
    }
}

// ── Certificate Builder (convenience) ──────────────────────────────

pub struct CertBuilder {
    kind: CertKind,
    target: String,
    verifier: String,
    steps: Vec<ProofStep>,
}

impl CertBuilder {
    pub fn new(kind: CertKind, target: &str, verifier: &str) -> Self {
        Self {
            kind,
            target: target.into(),
            verifier: verifier.into(),
            steps: Vec::new(),
        }
    }

    pub fn axiom(mut self, claim: &str) -> Self {
        self.steps.push(ProofStep::Axiom(claim.into()));
        self
    }

    pub fn witness(mut self, source: &str, claim: &str) -> Self {
        self.steps.push(ProofStep::Witness { source: source.into(), claim: claim.into() });
        self
    }

    pub fn derive(mut self, rule: &str, premises: &[usize], conclusion: &str) -> Self {
        self.steps.push(ProofStep::Derivation {
            rule: rule.into(),
            premises: premises.to_vec(),
            conclusion: conclusion.into(),
        });
        self
    }

    pub fn issue_into(self, store: &mut CertificateStore, timestamp: u64) -> Result<CertId, String> {
        store.issue(self.kind, &self.target, &self.verifier, self.steps, timestamp)
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> CertificateStore { CertificateStore::new() }

    fn simple_steps() -> Vec<ProofStep> {
        vec![
            ProofStep::Axiom("all borrows tracked".into()),
            ProofStep::Witness { source: "borrow_check".into(), claim: "no dangling refs".into() },
            ProofStep::Derivation {
                rule: "combine".into(),
                premises: vec![0, 1],
                conclusion: "memory safe".into(),
            },
        ]
    }

    // ── Issue & retrieve ──────────────────────────────────────────

    #[test]
    fn issue_certificate() {
        let mut s = store();
        let id = s.issue(CertKind::MemorySafety, "fn foo", "borrow_check", simple_steps(), 1).unwrap();
        assert_eq!(id, 1);
        assert_eq!(s.valid_count(), 1);
    }

    #[test]
    fn get_certificate() {
        let mut s = store();
        let id = s.issue(CertKind::MemorySafety, "fn foo", "borrow_check", simple_steps(), 1).unwrap();
        let cert = s.get(id).unwrap();
        assert_eq!(cert.target, "fn foo");
        assert!(cert.valid);
    }

    // ── Revocation ────────────────────────────────────────────────

    #[test]
    fn revoke_certificate() {
        let mut s = store();
        let id = s.issue(CertKind::MemorySafety, "fn foo", "bc", simple_steps(), 1).unwrap();
        s.revoke(id).unwrap();
        assert!(!s.get(id).unwrap().valid);
        assert_eq!(s.valid_count(), 0);
    }

    #[test]
    fn revoke_nonexistent() {
        let mut s = store();
        assert!(s.revoke(999).is_err());
    }

    // ── Query ─────────────────────────────────────────────────────

    #[test]
    fn certs_for_target() {
        let mut s = store();
        s.issue(CertKind::MemorySafety, "fn foo", "bc", simple_steps(), 1).unwrap();
        s.issue(CertKind::DataRaceFreedom, "fn foo", "race_check", vec![ProofStep::Axiom("no races".into())], 2).unwrap();
        s.issue(CertKind::MemorySafety, "fn bar", "bc", simple_steps(), 3).unwrap();
        assert_eq!(s.certs_for("fn foo").len(), 2);
    }

    #[test]
    fn certs_by_kind() {
        let mut s = store();
        s.issue(CertKind::MemorySafety, "fn foo", "bc", simple_steps(), 1).unwrap();
        s.issue(CertKind::MemorySafety, "fn bar", "bc", simple_steps(), 2).unwrap();
        assert_eq!(s.certs_by_kind(CertKind::MemorySafety).len(), 2);
        assert_eq!(s.certs_by_kind(CertKind::EffectContainment).len(), 0);
    }

    // ── Fully certified ───────────────────────────────────────────

    #[test]
    fn fully_certified() {
        let mut s = store();
        s.issue(CertKind::MemorySafety, "fn foo", "bc", simple_steps(), 1).unwrap();
        assert!(!s.fully_certified("fn foo")); // only 1 of 4
        s.issue(CertKind::DataRaceFreedom, "fn foo", "rc", vec![ProofStep::Axiom("ok".into())], 2).unwrap();
        s.issue(CertKind::ContractSatisfaction, "fn foo", "cv", vec![ProofStep::Axiom("ok".into())], 3).unwrap();
        s.issue(CertKind::EffectContainment, "fn foo", "ec", vec![ProofStep::Axiom("ok".into())], 4).unwrap();
        assert!(s.fully_certified("fn foo"));
    }

    #[test]
    fn not_fully_certified_after_revoke() {
        let mut s = store();
        let id1 = s.issue(CertKind::MemorySafety, "fn foo", "bc", simple_steps(), 1).unwrap();
        s.issue(CertKind::DataRaceFreedom, "fn foo", "rc", vec![ProofStep::Axiom("ok".into())], 2).unwrap();
        s.issue(CertKind::ContractSatisfaction, "fn foo", "cv", vec![ProofStep::Axiom("ok".into())], 3).unwrap();
        s.issue(CertKind::EffectContainment, "fn foo", "ec", vec![ProofStep::Axiom("ok".into())], 4).unwrap();
        s.revoke(id1).unwrap();
        assert!(!s.fully_certified("fn foo"));
    }

    // ── Consistency check ─────────────────────────────────────────

    #[test]
    fn consistency_valid() {
        let steps = simple_steps();
        let cert = Certificate { id: 0, kind: CertKind::MemorySafety, target: "t".into(), verifier: "v".into(), steps, timestamp: 0, valid: true };
        assert!(cert.check_consistency().is_ok());
    }

    #[test]
    fn consistency_invalid_forward_ref() {
        let steps = vec![
            ProofStep::Derivation { rule: "bad".into(), premises: vec![1], conclusion: "???".into() },
            ProofStep::Axiom("late".into()),
        ];
        let cert = Certificate { id: 0, kind: CertKind::MemorySafety, target: "t".into(), verifier: "v".into(), steps, timestamp: 0, valid: true };
        assert!(cert.check_consistency().is_err());
    }

    #[test]
    fn issue_rejects_invalid() {
        let mut s = store();
        let steps = vec![
            ProofStep::Derivation { rule: "bad".into(), premises: vec![1], conclusion: "???".into() },
            ProofStep::Axiom("late".into()),
        ];
        assert!(s.issue(CertKind::MemorySafety, "fn bad", "v", steps, 1).is_err());
    }

    // ── Certificate properties ────────────────────────────────────

    #[test]
    fn proof_depth() {
        let steps = simple_steps();
        let cert = Certificate { id: 0, kind: CertKind::MemorySafety, target: "t".into(), verifier: "v".into(), steps, timestamp: 0, valid: true };
        assert_eq!(cert.proof_depth(), 3);
    }

    #[test]
    fn final_conclusion() {
        let steps = simple_steps();
        let cert = Certificate { id: 0, kind: CertKind::MemorySafety, target: "t".into(), verifier: "v".into(), steps, timestamp: 0, valid: true };
        assert_eq!(cert.final_conclusion(), Some("memory safe"));
    }

    // ── Builder ───────────────────────────────────────────────────

    #[test]
    fn builder_flow() {
        let mut s = store();
        let id = CertBuilder::new(CertKind::EffectContainment, "fn pure", "effect_check")
            .axiom("no IO calls")
            .witness("static_analysis", "all effects declared")
            .derive("combine", &[0, 1], "effects contained")
            .issue_into(&mut s, 1)
            .unwrap();
        let cert = s.get(id).unwrap();
        assert_eq!(cert.proof_depth(), 3);
        assert_eq!(cert.final_conclusion(), Some("effects contained"));
    }

    // ── JSON ──────────────────────────────────────────────────────

    #[test]
    fn json_summary() {
        let mut s = store();
        s.issue(CertKind::MemorySafety, "fn foo", "bc", simple_steps(), 1).unwrap();
        s.issue(CertKind::DataRaceFreedom, "fn foo", "rc", vec![ProofStep::Axiom("ok".into())], 2).unwrap();
        let json = s.to_json();
        assert!(json.contains("\"total\":2"));
        assert!(json.contains("\"valid\":2"));
        assert!(json.contains("\"memory-safety\":1"));
    }

    // ── CertKind display ──────────────────────────────────────────

    #[test]
    fn cert_kind_display() {
        assert_eq!(format!("{}", CertKind::MemorySafety), "memory-safety");
        assert_eq!(format!("{}", CertKind::EffectContainment), "effect-containment");
    }

    // ── ProofStep conclusion ──────────────────────────────────────

    #[test]
    fn proof_step_conclusions() {
        assert_eq!(ProofStep::Axiom("a".into()).conclusion(), "a");
        assert_eq!(ProofStep::Witness { source: "s".into(), claim: "c".into() }.conclusion(), "c");
        let d = ProofStep::Derivation { rule: "r".into(), premises: vec![], conclusion: "d".into() };
        assert_eq!(d.conclusion(), "d");
    }
}
