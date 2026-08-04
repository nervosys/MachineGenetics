//! Signed build provenance.
//!
//! [`cas`](super::cas) draws a line between two kinds of stored thing: blobs are
//! immutable and self-verifying, so they can come from anywhere; **action-cache
//! entries are claims** — "running this action produces these outputs" — and are
//! only as good as whoever made them.
//!
//! That distinction is what makes a shared cache safe in principle. It becomes
//! safe in practice only once a claim carries proof of its author, which is what
//! this module adds. Until then, a shared action cache is a channel by which any
//! participant can hand every other participant an arbitrary build result, and
//! the recipient's only defence is that the digest of the output it was told to
//! expect matches the output it was given — which it always will, because the
//! liar chose both.
//!
//! ## What is signed
//!
//! `(action key, output digests, worker identity)`. Not the output *bytes*: those
//! are in the CAS and verify themselves. The claim being authenticated is the
//! association — *this* key legitimately maps to *these* digests, and *this*
//! worker says so.
//!
//! ## Trust model, stated plainly
//!
//! HMAC is symmetric, so every worker able to verify a provenance record is also
//! able to mint one. In a fleet where all workers are equally trusted that is
//! exactly the right property — it makes cache entries attributable and detects
//! corruption or a misconfigured worker, which are the realistic failures.
//!
//! It does **not** survive a compromised worker: an attacker holding the fleet
//! key can sign whatever they like. Defending against that needs per-worker
//! asymmetric keys so a single compromise is contained and revocable, and it is
//! the reason [`Provenance::worker`] is inside the signed material — the
//! attribution is already there, waiting for a signature scheme that makes it
//! mean something.

use super::cas::ActionResult;
use super::Digest;
use crate::mac::{absorb, ct_eq, hex_decode, hex_encode, hmac_sha256};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// An authenticated claim about a build result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// The action this describes.
    pub action_key: Digest,
    /// Which worker is claiming it.
    pub worker: String,
    /// Digest over the canonical `(action_key, outputs, worker)` encoding.
    pub subject: Digest,
    pub mac: String,
}

/// Canonical digest of what a provenance record asserts.
pub fn canonical_digest(action_key: &Digest, result: &ActionResult, worker: &str) -> Digest {
    let mut h = Sha256::new();
    h.update(b"ribosome-provenance-v1");
    absorb(&mut h, action_key.0.as_bytes());
    absorb(&mut h, worker.as_bytes());
    // BTreeMap iterates in key order, so this is already canonical.
    h.update((result.outputs.len() as u64).to_le_bytes());
    for (path, digest) in &result.outputs {
        absorb(&mut h, path.as_bytes());
        absorb(&mut h, digest.0.as_bytes());
    }
    h.update(result.exit_code.to_le_bytes());
    Digest(format!("{:x}", h.finalize()))
}

/// Signs and verifies provenance for one worker identity.
pub struct Signer {
    worker: String,
    key: Vec<u8>,
}

impl Signer {
    pub fn new(worker: impl Into<String>, key: impl Into<Vec<u8>>) -> Self {
        Signer { worker: worker.into(), key: key.into() }
    }

    pub fn worker(&self) -> &str {
        &self.worker
    }

    pub fn sign(&self, action_key: &Digest, result: &ActionResult) -> Provenance {
        let subject = canonical_digest(action_key, result, &self.worker);
        Provenance {
            mac: hex_encode(&hmac_sha256(&self.key, subject.0.as_bytes())),
            action_key: action_key.clone(),
            worker: self.worker.clone(),
            subject,
        }
    }

    /// Verify a record against the result it claims to describe.
    ///
    /// Recomputes the subject rather than trusting the field, so a record cannot
    /// carry a valid MAC over one subject while naming another.
    pub fn verify(&self, p: &Provenance, action_key: &Digest, result: &ActionResult) -> bool {
        let subject = canonical_digest(action_key, result, &p.worker);
        if subject != p.subject || &p.action_key != action_key {
            return false;
        }
        let Ok(got) = hex_decode(&p.mac) else { return false };
        ct_eq(&hmac_sha256(&self.key, subject.0.as_bytes()), &got)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result() -> ActionResult {
        ActionResult::ok("linux-x86_64").with_output("out.abl", Digest::of(b"artifact"))
    }

    fn signer() -> Signer {
        Signer::new("worker-7", b"fleet key".to_vec())
    }

    fn key() -> Digest {
        Digest::of(b"action key")
    }

    #[test]
    fn a_genuine_record_verifies() {
        let s = signer();
        let p = s.sign(&key(), &result());
        assert!(s.verify(&p, &key(), &result()));
    }

    #[test]
    fn substituting_the_output_digest_invalidates_the_record() {
        let s = signer();
        let p = s.sign(&key(), &result());
        // The lie a shared cache makes possible: same action, different artifact.
        let swapped = ActionResult::ok("linux-x86_64")
            .with_output("out.abl", Digest::of(b"malicious artifact"));
        assert!(!s.verify(&p, &key(), &swapped));
    }

    #[test]
    fn reusing_a_record_for_a_different_action_fails() {
        let s = signer();
        let p = s.sign(&key(), &result());
        assert!(!s.verify(&p, &Digest::of(b"some other action"), &result()));
    }

    #[test]
    fn a_record_cannot_be_relabelled_with_another_worker() {
        let s = signer();
        let mut p = s.sign(&key(), &result());
        p.worker = "worker-1".into();
        assert!(!s.verify(&p, &key(), &result()), "the worker is inside the signed material");
    }

    #[test]
    fn a_foreign_key_does_not_verify() {
        let real = signer();
        let outsider = Signer::new("worker-7", b"not the fleet key".to_vec());
        let p = outsider.sign(&key(), &result());
        assert!(!real.verify(&p, &key(), &result()));
    }

    #[test]
    fn a_tampered_subject_is_recomputed_and_caught() {
        let s = signer();
        let mut p = s.sign(&key(), &result());
        p.subject = Digest::of(b"something convenient");
        assert!(!s.verify(&p, &key(), &result()));
    }

    #[test]
    fn exit_code_participates() {
        let s = signer();
        let ok = result();
        let mut failed = result();
        failed.exit_code = 1;
        let p = s.sign(&key(), &ok);
        assert!(!s.verify(&p, &key(), &failed), "a failure must not inherit a success's provenance");
    }

    #[test]
    fn malformed_macs_are_rejected_not_panicked_on() {
        let s = signer();
        let mut p = s.sign(&key(), &result());
        p.mac = "nonsense".into();
        assert!(!s.verify(&p, &key(), &result()));
    }
}
