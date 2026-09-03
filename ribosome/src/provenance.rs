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
    /// Which key signed this, when the signer was given an id.
    ///
    /// Without this a verifier cannot tell *which* secret produced a MAC, so
    /// there is no way to accept two keys at once and therefore no way to
    /// rotate: every worker and every verifier would have to change in the
    /// same instant. That was open item 20. A [`Keyring`] uses this to pick
    /// the right key, which gives rotation an overlapping window.
    ///
    /// `Option`, and `#[serde(default)]`, because records written before this
    /// field existed must still deserialise and still verify. It is absorbed
    /// into the subject digest **only when present**, so an unkeyed record
    /// hashes exactly as it did before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
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

/// The smallest HMAC key this will accept, in bytes.
///
/// RFC 2104 §3: a key shorter than the hash output length is "strongly
/// discouraged"; for SHA-256 that is 32 bytes.
pub const MIN_KEY_LEN: usize = 32;

/// Why a key was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    /// Shorter than [`MIN_KEY_LEN`]. `len == 0` is the case that motivated
    /// this: an unset environment variable or a missing config field becomes
    /// an empty `Vec`, and an empty key signs perfectly well and verifies
    /// perfectly well — so an unprovisioned fleet authenticated every claim
    /// and reported success, and a second signer built the same way agreed
    /// with it. Nothing downstream could tell: a MAC over an empty key is a
    /// well-formed MAC. Open item 19.
    TooShort { len: usize, min: usize },
}

impl std::fmt::Display for KeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyError::TooShort { len: 0, min } => write!(
                f,
                "the HMAC key is empty; {min} bytes are required. An unset                  environment variable is the usual cause, and an empty key                  authenticates everything while reporting success"
            ),
            KeyError::TooShort { len, min } => write!(
                f,
                "the HMAC key is {len} bytes; {min} are required (RFC 2104 §3)"
            ),
        }
    }
}

impl std::error::Error for KeyError {}

/// Signs and verifies provenance for one worker identity.
pub struct Signer {
    worker: String,
    key: Vec<u8>,
    key_id: Option<String>,
}

/// Redacting, deliberately hand-written rather than derived.
///
/// `#[derive(Debug)]` here would print the fleet's shared secret into any log
/// line, panic message or test failure that formats a `Signer` — and one of
/// those, `unwrap_err()` on a `Result<Signer, _>`, is exactly what the tests
/// below need. The key length is safe to show and is the thing worth knowing
/// when a construction was refused.
impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("worker", &self.worker)
            .field("key_id", &self.key_id)
            .field("key", &format_args!("<{} bytes redacted>", self.key.len()))
            .finish()
    }
}

impl Signer {
    /// Build a signer for one worker identity from the fleet's shared key.
    ///
    /// **Fallible since 2026-09-02, and this is a breaking change on purpose.**
    /// It used to take any key at all and return a `Signer`. `Signer::new("w",
    /// Vec::new())` signed, the record verified, and a second empty-key signer
    /// verified it too — so a fleet whose key was never provisioned
    /// authenticated every claim and reported success. An empty `Vec` is what
    /// an unset environment variable or a missing config field naturally
    /// becomes, which made it the likely state rather than an exotic one.
    ///
    /// A warning would not have fixed it: the whole failure is that nothing
    /// downstream can tell a MAC over an empty key from any other MAC. The
    /// only place the difference is knowable is here, at construction, so this
    /// is where it is refused. See [`MIN_KEY_LEN`] and open item 19.
    pub fn new(
        worker: impl Into<String>,
        key: impl Into<Vec<u8>>,
    ) -> Result<Self, KeyError> {
        let key = key.into();
        if key.len() < MIN_KEY_LEN {
            return Err(KeyError::TooShort { len: key.len(), min: MIN_KEY_LEN });
        }
        Ok(Signer { worker: worker.into(), key, key_id: None })
    }

    /// The same, tagging every record with which key signed it.
    ///
    /// The id is not a secret and is written into the record in clear. It
    /// exists so a verifier can hold two keys at once and know which to try,
    /// which is the whole of what rotation needs — see [`Keyring`] and open
    /// item 20.
    pub fn with_key_id(
        worker: impl Into<String>,
        key_id: impl Into<String>,
        key: impl Into<Vec<u8>>,
    ) -> Result<Self, KeyError> {
        let mut s = Self::new(worker, key)?;
        s.key_id = Some(key_id.into());
        Ok(s)
    }

    /// Which key this signer stamps its records with, if any.
    pub fn key_id(&self) -> Option<&str> {
        self.key_id.as_deref()
    }

    pub fn worker(&self) -> &str {
        &self.worker
    }

    pub fn sign(&self, action_key: &Digest, result: &ActionResult) -> Provenance {
        let subject = canonical_digest(action_key, result, &self.worker);
        Provenance {
            mac: hex_encode(&hmac_sha256(&self.key, &Self::mac_input(&subject, self.key_id.as_deref()))),
            action_key: action_key.clone(),
            worker: self.worker.clone(),
            subject,
            key_id: self.key_id.clone(),
        }
    }

    /// What the MAC is computed over: the subject, and the key id when there
    /// is one.
    ///
    /// Binding the id in means a record cannot have its `key_id` rewritten to
    /// point a verifier at a different key — the MAC would no longer match.
    /// It is length-prefixed rather than concatenated so that a subject ending
    /// in one thing and an id beginning with another cannot be confused for a
    /// different pair.
    ///
    /// **A record with no id hashes exactly as it did before this field
    /// existed**, so every provenance record written by an earlier build still
    /// verifies. That is the only reason the id is `Option` rather than always
    /// present.
    fn mac_input(subject: &Digest, key_id: Option<&str>) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(subject.0.as_bytes());
        if let Some(id) = key_id {
            v.extend_from_slice(b"|kid:");
            v.extend_from_slice(&(id.len() as u64).to_le_bytes());
            v.extend_from_slice(id.as_bytes());
        }
        v
    }

    /// Verify a record against the result it claims to describe.
    ///
    /// Recomputes the subject rather than trusting the field, so a record cannot
    /// carry a valid MAC over one subject while naming another.
    ///
    /// A record's `key_id` must match this signer's: a signer holding one key
    /// has no business accepting a record minted under another, and saying so
    /// here is what makes [`Keyring`] the only place that decides *which* key
    /// applies.
    pub fn verify(&self, p: &Provenance, action_key: &Digest, result: &ActionResult) -> bool {
        let subject = canonical_digest(action_key, result, &p.worker);
        if subject != p.subject || &p.action_key != action_key {
            return false;
        }
        if p.key_id.as_deref() != self.key_id.as_deref() {
            return false;
        }
        let Ok(got) = hex_decode(&p.mac) else { return false };
        ct_eq(
            &hmac_sha256(&self.key, &Self::mac_input(&subject, self.key_id.as_deref())),
            &got,
        )
    }
}

/// Several keys, so a fleet can change one.
///
/// Open item 20 was that the HMAC path could not be rotated: one key per
/// `Signer`, one `auth_key` per server, no key id, and therefore no way to
/// accept the old and the new secret at once. Changing the fleet secret meant
/// changing every worker and every verifier in the same instant — which is not
/// a rotation, it is an outage with a key change in it. The Ed25519 path beside
/// it rotates properly, and that contrast is what made the gap visible.
///
/// A verifier holds every key it will accept. Workers move to the new one in
/// any order; when none is left signing with the old id, it is dropped from the
/// ring. That is the overlapping window.
pub struct Keyring {
    keys: Vec<Signer>,
}

impl Keyring {
    /// An empty ring accepts nothing, which is the correct starting point.
    pub fn new() -> Self {
        Keyring { keys: Vec::new() }
    }

    /// Add a key this verifier will accept.
    pub fn with(mut self, signer: Signer) -> Self {
        self.keys.push(signer);
        self
    }

    /// How many keys are accepted. During a rotation this is 2.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Verify against whichever held key the record names.
    ///
    /// **The `filter` is an optimisation, not the security boundary**, and it
    /// is worth being exact about which is which. Correctness comes from
    /// [`Signer::verify`], which refuses any record whose `key_id` is not its
    /// own; the filter only avoids computing MACs that are already known to
    /// fail, so adding keys to the ring does not multiply per-record work.
    /// Deleting the filter changes no outcome — checked by deleting it and
    /// watching every test still pass, which is why this comment no longer
    /// claims the filter is what keeps an old record from verifying under a
    /// new key.
    pub fn verify(&self, p: &Provenance, action_key: &Digest, result: &ActionResult) -> bool {
        self.keys
            .iter()
            .filter(|s| s.key_id.as_deref() == p.key_id.as_deref())
            .any(|s| s.verify(p, action_key, result))
    }
}

impl Default for Keyring {
    fn default() -> Self {
        Self::new()
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
        Signer::new("worker-7", b"fleet key that is at least 32 bytes long!".to_vec()).expect("32-byte test key")
    }

    fn key() -> Digest {
        Digest::of(b"action key")
    }

    // ── Item 19: the key that authenticates everything ───────────────

    /// The measured failure, now impossible to construct.
    ///
    /// Before this, `Signer::new("w", Vec::new())` signed, the record verified,
    /// and — the part that makes it a fleet-wide failure rather than a local
    /// one — a *second, independently built* empty-key signer verified it too.
    /// Every unprovisioned worker therefore accepted every other's claims while
    /// reporting success.
    #[test]
    fn an_empty_key_is_refused_at_construction() {
        assert_eq!(
            Signer::new("w", Vec::new()).unwrap_err(),
            KeyError::TooShort { len: 0, min: MIN_KEY_LEN }
        );
        // And the near-miss that is just as dangerous: a short key that looks
        // deliberate. The old tests used one of these.
        assert!(Signer::new("w", b"fleet key".to_vec()).is_err());
        // The boundary itself, from both sides.
        assert!(Signer::new("w", vec![7u8; MIN_KEY_LEN - 1]).is_err());
        assert!(Signer::new("w", vec![7u8; MIN_KEY_LEN]).is_ok());
    }

    // ── Item 20: two keys at once, which is what rotation is ─────────

    /// A ring holding the old and the new key verifies records from both, which
    /// is the overlapping window that was impossible without a key id.
    #[test]
    fn a_keyring_accepts_both_keys_during_a_rotation() {
        let old = Signer::with_key_id("w", "2026-08", vec![1u8; 32]).unwrap();
        let new = Signer::with_key_id("w", "2026-09", vec![2u8; 32]).unwrap();
        let ring = Keyring::new()
            .with(Signer::with_key_id("w", "2026-08", vec![1u8; 32]).unwrap())
            .with(Signer::with_key_id("w", "2026-09", vec![2u8; 32]).unwrap());
        assert_eq!(ring.len(), 2);

        for s in [&old, &new] {
            let p = s.sign(&key(), &result());
            assert!(ring.verify(&p, &key(), &result()), "ring must accept key id {:?}", s.key_id());
        }

        // Retiring the old key ends the window: its records stop verifying.
        let only_new = Keyring::new()
            .with(Signer::with_key_id("w", "2026-09", vec![2u8; 32]).unwrap());
        assert!(!only_new.verify(&old.sign(&key(), &result()), &key(), &result()));
    }

    /// A rewritten key id is rejected — and this test proves it is the **MAC
    /// binding** that rejects it, not the cheaper equality check beside it.
    ///
    /// The obvious version of this test does not. Signing under id `A`,
    /// rewriting the field to `B` and verifying with the `A` signer fails at
    /// `p.key_id != self.key_id` before any MAC is computed, so it passes
    /// whether or not the id is in the MAC at all — verified by removing the
    /// binding and watching that version still pass.
    ///
    /// So: two signers sharing one key and differing only in id. The record is
    /// minted as `A`, relabelled `B`, and offered to the `B` signer. The
    /// equality check now agrees, both hold the same secret, and the only thing
    /// left that can refuse it is the id being inside the MAC.
    #[test]
    fn a_relabelled_record_fails_on_the_mac_not_the_label() {
        let shared = vec![9u8; 32];
        let a = Signer::with_key_id("w", "2026-08", shared.clone()).unwrap();
        let b = Signer::with_key_id("w", "2026-09", shared).unwrap();

        let mut p = a.sign(&key(), &result());
        assert!(a.verify(&p, &key(), &result()));

        p.key_id = Some("2026-09".to_string());
        assert!(
            !b.verify(&p, &key(), &result()),
            "a record relabelled onto another id must fail even when that id's              holder has the same key — the id is covered by the MAC"
        );
    }

    /// Records written before `key_id` existed carry none, and must still
    /// verify — the reason the field is `Option` and is absorbed into the MAC
    /// only when present.
    #[test]
    fn an_unkeyed_record_still_verifies() {
        let s = Signer::new("w", vec![3u8; 32]).unwrap();
        let p = s.sign(&key(), &result());
        assert!(p.key_id.is_none());
        assert!(s.verify(&p, &key(), &result()));
        // And a keyed signer must not accept it, nor the reverse.
        let keyed = Signer::with_key_id("w", "2026-09", vec![3u8; 32]).unwrap();
        assert!(!keyed.verify(&p, &key(), &result()));
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
        let outsider = Signer::new("worker-7", b"a different 32+ byte fleet key here.....".to_vec()).expect("32-byte test key");
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
