//! Shared message-authentication primitives.
//!
//! Two subsystems need to prove authorship of a record — [`germline::attest`]
//! for verdicts, [`ribosome::provenance`] for build results — and both were
//! going to grow their own copy of HMAC. One implementation, checked once
//! against the standard's vectors, is the only defensible arrangement: a second
//! hand-rolled copy is a second chance to get the padding wrong.
//!
//! [`germline::attest`]: crate::germline::attest
//! [`ribosome::provenance`]: crate::ribosome::provenance
//!
//! **Symmetric, deliberately scoped.** HMAC proves a record came from someone
//! holding the key, which means any verifier can also forge. That is adequate
//! inside one trust domain and inadequate across one. Where this crate crosses a
//! trust boundary the limitation is stated at the call site rather than assumed
//! away; asymmetric signatures are a dependency away and this module's callers
//! would not change shape.

use sha2::{Digest, Sha256};

const BLOCK: usize = 64;

/// HMAC-SHA256 (RFC 2104), implemented directly rather than pulling a
/// dependency for forty lines of well-specified construction.
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    outer.finalize().into()
}

/// Constant-time equality.
///
/// A byte-by-byte early-return compare leaks, through timing, how many leading
/// bytes of a guess were right — which turns forging a MAC from a 2^256 problem
/// into roughly a 32×256 one. Folding over the whole buffer costs nothing.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() % 2 != 0 {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

/// Length-prefixed field feeding, for canonical encodings.
///
/// Without length prefixing, `("ab", "c")` and `("a", "bc")` hash identically —
/// a real source of forged-by-accident collisions in any scheme that
/// concatenates fields.
pub fn absorb(h: &mut Sha256, bytes: &[u8]) {
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_4231_case_1() {
        // key = 20 × 0x0b, data = "Hi There"
        assert_eq!(
            hex_encode(&hmac_sha256(&[0x0b; 20], b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn rfc_4231_case_2() {
        // key = "Jefe", data = "what do ya want for nothing?"
        assert_eq!(
            hex_encode(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn rfc_4231_case_3_long_key_is_hashed_first() {
        // key = 131 × 0xaa (> block size), data = "Test Using Larger Than
        // Block-Size Key - Hash Key First"
        let key = [0xaa; 131];
        assert_eq!(
            hex_encode(&hmac_sha256(
                &key,
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn ct_eq_is_total_and_correct() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
        assert!(ct_eq(b"", b""));
    }

    #[test]
    fn hex_round_trips_and_rejects_garbage() {
        assert_eq!(hex_decode(&hex_encode(&[0, 15, 255])).unwrap(), vec![0, 15, 255]);
        assert!(hex_decode("abc").is_err(), "odd length");
        assert!(hex_decode("zz").is_err(), "not hex");
    }

    #[test]
    fn absorb_prevents_field_boundary_collisions() {
        let mut a = Sha256::new();
        absorb(&mut a, b"ab");
        absorb(&mut a, b"c");
        let mut b = Sha256::new();
        absorb(&mut b, b"a");
        absorb(&mut b, b"bc");
        assert_ne!(a.finalize(), b.finalize());
    }
}
