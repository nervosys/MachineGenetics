//! Attestation: proof that a verdict came from the evaluator it names.
//!
//! [`super::lineage`] records *which* evaluator issued a verdict. That is a
//! claim. This module makes it checkable.
//!
//! The gap matters because the whole succession design rests on evaluator
//! independence, and an unauthenticated `evaluator: "independent-harness"` field
//! is a string anything can write. Every other invariant is enforced against a
//! record that, until now, could simply assert the answer it wanted.
//!
//! ## HMAC, and what it does not give you
//!
//! This is HMAC-SHA256 over a canonical encoding of the verdict: symmetric, so
//! anyone able to *verify* an attestation is also able to *forge* one. That is
//! adequate when the verifier is the same trust domain as the signer — a
//! supervisor checking its own harness's output — and inadequate the moment
//! attestations cross a trust boundary, which is exactly what happens once
//! evaluation is distributed across a fleet.
//!
//! The honest statement is therefore: **this closes the "the record could just
//! assert it" gap and does not close the "a compromised worker could mint
//! verdicts" gap.** Asymmetric signatures (ed25519) are the production answer
//! and are one dependency away; the interface here — [`Attestor::attest`] /
//! [`Attestation::verify_with`] — does not change when they replace HMAC.
//!
//! It is written this way rather than left as a TODO because a `TODO: sign this`
//! and a working-but-limited mechanism have very different failure modes: the
//! first ships unsigned.

use super::gate::Verdict;
use ribosome::mac::{absorb, ct_eq, hex_decode, hex_encode, hmac_sha256};
use ribosome::Digest;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// A verdict plus proof of its origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    /// Digest of the canonical verdict encoding — what was actually signed.
    pub subject: Digest,
    /// The identity claiming authorship.
    pub evaluator: String,
    /// Hex MAC.
    pub mac: String,
}

impl Attestation {
    /// Verify against a key. Returns false for a wrong key, a tampered verdict,
    /// or a mismatched evaluator name — all three are the same answer to the
    /// caller: do not act on this.
    pub fn verify_with(&self, key: &[u8], verdict: &Verdict) -> bool {
        let subject = canonical_digest(verdict, &self.evaluator);
        if subject != self.subject {
            return false;
        }
        let expect = hmac_sha256(key, subject.0.as_bytes());
        let Ok(got) = hex_decode(&self.mac) else { return false };
        ct_eq(&expect, &got)
    }
}

/// Canonical digest of `(verdict, evaluator)`.
///
/// The evaluator name is inside the signed material, not merely alongside it.
/// Otherwise an attestation could be lifted from one evaluator's verdict and
/// re-labelled with another's name while staying valid.
pub fn canonical_digest(verdict: &Verdict, evaluator: &str) -> Digest {
    let body = serde_json::to_string(verdict).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(b"germline-attestation-v1");
    absorb(&mut h, evaluator.as_bytes());
    absorb(&mut h, body.as_bytes());
    Digest(format!("{:x}", h.finalize()))
}

/// Issues attestations for one evaluator identity.
pub struct Attestor {
    evaluator: String,
    key: Vec<u8>,
}

impl Attestor {
    pub fn new(evaluator: impl Into<String>, key: impl Into<Vec<u8>>) -> Self {
        Attestor { evaluator: evaluator.into(), key: key.into() }
    }

    pub fn evaluator(&self) -> &str {
        &self.evaluator
    }

    pub fn attest(&self, verdict: &Verdict) -> Attestation {
        let subject = canonical_digest(verdict, &self.evaluator);
        Attestation {
            mac: hex_encode(&hmac_sha256(&self.key, subject.0.as_bytes())),
            subject,
            evaluator: self.evaluator.clone(),
        }
    }

    pub fn verify(&self, att: &Attestation, verdict: &Verdict) -> bool {
        att.verify_with(&self.key, verdict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenerationId;

    fn verdict() -> Verdict {
        Verdict::Promote {
            generation: GenerationId(7),
            gate: Digest::of(b"gate"),
            evaluator: "independent-harness".into(),
        }
    }

    fn attestor() -> Attestor {
        Attestor::new("independent-harness", b"a shared secret".to_vec())
    }

    #[test]
    fn a_genuine_attestation_verifies() {
        let a = attestor();
        let v = verdict();
        assert!(a.verify(&a.attest(&v), &v));
    }

    #[test]
    fn a_tampered_verdict_fails_verification() {
        let a = attestor();
        let att = a.attest(&verdict());
        let altered = Verdict::Promote {
            generation: GenerationId(8), // a different generation
            gate: Digest::of(b"gate"),
            evaluator: "independent-harness".into(),
        };
        assert!(!a.verify(&att, &altered), "swapping the subject must invalidate the proof");
    }

    #[test]
    fn a_forged_mac_fails() {
        let a = attestor();
        let mut att = a.attest(&verdict());
        att.mac = "00".repeat(32);
        assert!(!a.verify(&att, &verdict()));
    }

    #[test]
    fn the_wrong_key_fails() {
        let real = attestor();
        let impostor = Attestor::new("independent-harness", b"guessed".to_vec());
        let att = impostor.attest(&verdict());
        assert!(!real.verify(&att, &verdict()), "a verdict is only as good as the key behind it");
    }

    #[test]
    fn an_attestation_cannot_be_relabelled_with_another_evaluator() {
        let a = attestor();
        let mut att = a.attest(&verdict());
        att.evaluator = "some-other-harness".into();
        assert!(
            !a.verify(&att, &verdict()),
            "the evaluator name is inside the signed material, not beside it"
        );
    }

    #[test]
    fn malformed_macs_are_rejected_rather_than_panicking() {
        let a = attestor();
        let mut att = a.attest(&verdict());
        att.mac = "not hex".into();
        assert!(!a.verify(&att, &verdict()));
        att.mac = "abc".into(); // odd length
        assert!(!a.verify(&att, &verdict()));
    }

}
