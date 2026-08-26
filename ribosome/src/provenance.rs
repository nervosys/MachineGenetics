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
use std::collections::{BTreeMap, BTreeSet};

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
    /// Build a signer for one worker identity from the fleet's shared key.
    ///
    /// **The key is not validated, and an empty one is accepted.** Measured,
    /// not inferred: `Signer::new("w", Vec::new())` signs, and the record
    /// verifies. That is the dangerous case, because an empty `Vec` is what an
    /// unset environment variable or a missing config field naturally becomes —
    /// so a fleet whose key was never provisioned authenticates every claim and
    /// reports success while providing nothing. Nothing downstream can tell the
    /// difference: a MAC over an empty key is a well-formed MAC.
    ///
    /// RFC 2104 recommends a key of at least the hash output length, so **32
    /// bytes** here. Validating that is a caller's job today; see
    /// `SECURITY_AUDIT.md` §2 for why the constructor was left permissive
    /// rather than changed to return a `Result` without the owner's say-so.
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

// ─────────────────────────────────────────────────────────────────────────────
// Asymmetric provenance
// ─────────────────────────────────────────────────────────────────────────────

/// A worker's Ed25519 signature over the same canonical subject.
///
/// This is what the symmetric [`Signer`] above cannot do, and the difference is
/// not "stronger crypto" — both are strong. It is that a shared secret makes
/// every holder equally able to *mint* claims, so one compromised worker can
/// forge provenance for the whole fleet and there is no way to tell which one,
/// nor to exclude it without re-keying everybody.
///
/// With per-worker keys the private half never leaves the worker, so:
///
/// - a compromise is **attributable** — a forged claim is signed by a specific
///   key, and the signature says which;
/// - a compromise is **containable** — that key alone is affected;
/// - a compromise is **revocable** — [`TrustStore::revoke`] excludes one worker
///   without touching the rest of the fleet.
///
/// Verifiers only ever hold public keys, so a cache mirror or an auditor can
/// check provenance without being able to produce it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedProvenance {
    pub action_key: Digest,
    pub worker: String,
    pub subject: Digest,
    /// Hex Ed25519 signature (64 bytes).
    pub signature: String,
    /// Hex public key (32 bytes), carried so a verifier can identify which key
    /// signed without a lookup — it is still checked against the trust store,
    /// because a self-asserted key proves nothing on its own.
    pub public_key: String,
}

/// Signs provenance with a worker-private key.
pub struct AsymmetricSigner {
    worker: String,
    key: ed25519_dalek::SigningKey,
}

impl AsymmetricSigner {
    /// Build from a 32-byte private seed. The seed is the secret; it must never
    /// leave the worker that owns it.
    pub fn from_seed(worker: impl Into<String>, seed: &[u8; 32]) -> Self {
        AsymmetricSigner { worker: worker.into(), key: ed25519_dalek::SigningKey::from_bytes(seed) }
    }

    pub fn worker(&self) -> &str {
        &self.worker
    }

    /// The public half, to be registered with verifiers.
    pub fn public_key_hex(&self) -> String {
        hex_encode(self.key.verifying_key().as_bytes())
    }

    pub fn sign(&self, action_key: &Digest, result: &ActionResult) -> SignedProvenance {
        use ed25519_dalek::Signer as _;
        let subject = canonical_digest(action_key, result, &self.worker);
        let sig = self.key.sign(subject.0.as_bytes());
        SignedProvenance {
            action_key: action_key.clone(),
            worker: self.worker.clone(),
            subject,
            signature: hex_encode(&sig.to_bytes()),
            public_key: self.public_key_hex(),
        }
    }
}

/// Which workers a verifier trusts, and which it no longer does.
#[derive(Debug, Default)]
pub struct TrustStore {
    keys: BTreeMap<String, ed25519_dalek::VerifyingKey>,
    revoked: BTreeSet<String>,
}

impl TrustStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Trust a worker's public key.
    pub fn trust(&mut self, worker: &str, public_key_hex: &str) -> Result<(), String> {
        let bytes = hex_decode(public_key_hex).map_err(|_| "public key is not hex".to_string())?;
        let arr: [u8; 32] =
            bytes.try_into().map_err(|_| "public key must be 32 bytes".to_string())?;
        let vk = ed25519_dalek::VerifyingKey::from_bytes(&arr)
            .map_err(|e| format!("not a valid Ed25519 key: {e}"))?;
        self.keys.insert(worker.to_string(), vk);
        self.revoked.remove(worker);
        Ok(())
    }

    /// Stop trusting a worker.
    ///
    /// Revocation is recorded rather than the key simply being removed, so that
    /// re-registering a revoked worker is a deliberate act (`trust` clears it)
    /// instead of something a rejoining compromised node can do to itself by
    /// re-announcing.
    pub fn revoke(&mut self, worker: &str) {
        self.revoked.insert(worker.to_string());
    }

    pub fn is_revoked(&self, worker: &str) -> bool {
        self.revoked.contains(worker)
    }

    pub fn trusted_workers(&self) -> Vec<&str> {
        self.keys
            .keys()
            .filter(|w| !self.revoked.contains(*w))
            .map(|s| s.as_str())
            .collect()
    }

    /// Verify a signed claim.
    ///
    /// Fails closed on every path: unknown worker, revoked worker, key mismatch,
    /// wrong subject, bad signature. A verifier that cannot establish who signed
    /// something must not accept it.
    pub fn verify(
        &self,
        p: &SignedProvenance,
        action_key: &Digest,
        result: &ActionResult,
    ) -> bool {
        use ed25519_dalek::Verifier as _;
        if self.revoked.contains(&p.worker) {
            return false;
        }
        let Some(vk) = self.keys.get(&p.worker) else { return false };
        // The carried key must be the one we trust for that worker, or a
        // compromised node could sign with its own key and label it as another's.
        if hex_encode(vk.as_bytes()) != p.public_key {
            return false;
        }
        let subject = canonical_digest(action_key, result, &p.worker);
        if subject != p.subject || &p.action_key != action_key {
            return false;
        }
        let Ok(raw) = hex_decode(&p.signature) else { return false };
        let Ok(arr) = <[u8; 64]>::try_from(raw) else { return false };
        vk.verify(subject.0.as_bytes(), &ed25519_dalek::Signature::from_bytes(&arr)).is_ok()
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

    // ---- asymmetric

    fn worker_a() -> AsymmetricSigner {
        AsymmetricSigner::from_seed("worker-a", &[7u8; 32])
    }

    fn worker_b() -> AsymmetricSigner {
        AsymmetricSigner::from_seed("worker-b", &[9u8; 32])
    }

    fn store_trusting(signers: &[&AsymmetricSigner]) -> TrustStore {
        let mut t = TrustStore::new();
        for s in signers {
            t.trust(s.worker(), &s.public_key_hex()).unwrap();
        }
        t
    }

    #[test]
    fn a_signed_claim_verifies_against_the_trusted_key() {
        let a = worker_a();
        let store = store_trusting(&[&a]);
        assert!(store.verify(&a.sign(&key(), &result()), &key(), &result()));
    }

    #[test]
    fn an_untrusted_worker_is_refused() {
        let a = worker_a();
        let stranger = AsymmetricSigner::from_seed("unknown-node", &[3u8; 32]);
        let store = store_trusting(&[&a]);
        assert!(
            !store.verify(&stranger.sign(&key(), &result()), &key(), &result()),
            "a verifier must not accept a claim it cannot attribute"
        );
    }

    #[test]
    fn a_compromised_worker_can_be_revoked_without_touching_the_others() {
        let (a, b) = (worker_a(), worker_b());
        let mut store = store_trusting(&[&a, &b]);
        assert!(store.verify(&a.sign(&key(), &result()), &key(), &result()));

        store.revoke("worker-a");

        assert!(
            !store.verify(&a.sign(&key(), &result()), &key(), &result()),
            "revocation must take effect immediately"
        );
        assert!(
            store.verify(&b.sign(&key(), &result()), &key(), &result()),
            "and must not disturb the rest of the fleet — the thing a shared secret cannot do"
        );
        assert_eq!(store.trusted_workers(), vec!["worker-b"]);
    }

    #[test]
    fn a_revoked_worker_cannot_re_trust_itself_by_re_announcing() {
        let a = worker_a();
        let mut store = store_trusting(&[&a]);
        store.revoke("worker-a");
        assert!(store.is_revoked("worker-a"));
        // Only a deliberate `trust` clears revocation.
        store.trust("worker-a", &a.public_key_hex()).unwrap();
        assert!(!store.is_revoked("worker-a"));
    }

    #[test]
    fn one_worker_cannot_sign_under_another_name() {
        let (a, b) = (worker_a(), worker_b());
        let store = store_trusting(&[&a, &b]);
        // b signs, then relabels itself as a — but carries its own public key.
        let mut forged = b.sign(&key(), &result());
        forged.worker = "worker-a".into();
        assert!(
            !store.verify(&forged, &key(), &result()),
            "the carried key must match the key trusted for that worker"
        );
    }

    #[test]
    fn substituting_the_artifact_invalidates_a_signature() {
        let a = worker_a();
        let store = store_trusting(&[&a]);
        let p = a.sign(&key(), &result());
        let swapped =
            ActionResult::ok("linux-x86_64").with_output("out.abl", Digest::of(b"malicious"));
        assert!(!store.verify(&p, &key(), &swapped));
    }

    #[test]
    fn a_tampered_signature_is_rejected() {
        let a = worker_a();
        let store = store_trusting(&[&a]);
        let mut p = a.sign(&key(), &result());
        p.signature = "00".repeat(64);
        assert!(!store.verify(&p, &key(), &result()));
    }

    #[test]
    fn malformed_keys_and_signatures_fail_closed_rather_than_panicking() {
        let a = worker_a();
        let mut store = store_trusting(&[&a]);
        assert!(store.trust("bad", "not hex").is_err());
        assert!(store.trust("short", "aabb").is_err());

        let mut p = a.sign(&key(), &result());
        p.signature = "zz".into();
        assert!(!store.verify(&p, &key(), &result()));
    }

    #[test]
    fn a_verifier_only_ever_needs_public_keys() {
        // The property that lets an untrusted mirror or an auditor check
        // provenance without being able to produce it.
        let a = worker_a();
        let store = store_trusting(&[&a]);
        assert!(store.verify(&a.sign(&key(), &result()), &key(), &result()));
        assert_eq!(a.public_key_hex().len(), 64, "32 bytes of public key, and nothing secret");
    }
}
