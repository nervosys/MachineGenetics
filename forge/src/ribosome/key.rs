//! Deterministic action keys.
//!
//! The key is a SHA-256 over a **canonical, length-prefixed** encoding of
//! everything that can change an action's result, and nothing else.
//!
//! Two properties matter more than the choice of hash:
//!
//! **Unambiguous.** Every field is length-prefixed, so no combination of field
//! values can collide by concatenation. Without this, an action with tool `"ab"`
//! and arg `"c"` keys identically to tool `"a"` and arg `"bc"` — a real and
//! silently wrong cache hit. Length prefixing costs nothing and removes the
//! entire class.
//!
//! **Order-insensitive where order is not semantic.** Inputs are sorted by
//! `(path, digest)` and env is a `BTreeMap`, so two agents that declare the same
//! inputs in different orders produce the same key and share a cache entry.
//! Args are *not* sorted: `-o a b` and `-o b a` are different commands.
//!
//! Deliberately excluded from the key:
//!
//! - `name` — a label. Renaming a target must not rebuild it.
//! - `cost` — a scheduling hint with no effect on output.
//! - `outputs` — see [`action_key`]; the paths are promises, not inputs.
//! - wall-clock time, hostname, worker identity, absolute paths — the things
//!   that make a build non-reproducible. They are unrepresentable here rather
//!   than merely discouraged.

use super::{Action, Digest};
use sha2::{Digest as _, Sha256};

/// Domain separator. Bump when the encoding changes so old entries miss rather
/// than being misinterpreted — a cache miss costs time, a misread costs
/// correctness.
const KEY_VERSION: &str = "ribosome-actionkey-v1";

/// Feed one length-prefixed field into the hasher.
fn field(h: &mut Sha256, tag: u8, bytes: &[u8]) {
    h.update([tag]);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
}

/// Compute an action's cache key.
///
/// Note `outputs` participates only as its *count and names*, not contents:
/// the names select which produced blobs are retained, so an action asked for
/// different outputs from the same computation is a different action, but the
/// output contents are the result and cannot be an input to their own key.
pub fn action_key(a: &Action) -> Digest {
    let mut h = Sha256::new();
    field(&mut h, 0x01, KEY_VERSION.as_bytes());
    field(&mut h, 0x02, a.tool.as_bytes());

    field(&mut h, 0x03, &(a.args.len() as u64).to_le_bytes());
    for arg in &a.args {
        field(&mut h, 0x04, arg.as_bytes());
    }

    // Sorted: declaration order of inputs is not semantic.
    let mut inputs: Vec<(&str, &str)> =
        a.inputs.iter().map(|i| (i.path.as_str(), i.digest.0.as_str())).collect();
    inputs.sort_unstable();
    field(&mut h, 0x05, &(inputs.len() as u64).to_le_bytes());
    for (path, digest) in inputs {
        field(&mut h, 0x06, path.as_bytes());
        field(&mut h, 0x07, digest.as_bytes());
    }

    // BTreeMap iterates in key order, so this is already canonical.
    field(&mut h, 0x08, &(a.env.len() as u64).to_le_bytes());
    for (k, v) in &a.env {
        field(&mut h, 0x09, k.as_bytes());
        field(&mut h, 0x0a, v.as_bytes());
    }

    let mut outs: Vec<&str> = a.outputs.iter().map(|s| s.as_str()).collect();
    outs.sort_unstable();
    field(&mut h, 0x0b, &(outs.len() as u64).to_le_bytes());
    for o in outs {
        field(&mut h, 0x0c, o.as_bytes());
    }

    field(&mut h, 0x0d, a.platform.tag().as_bytes());

    Digest(format!("{:x}", h.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribosome::Platform;

    fn base() -> Action {
        Action::new("lower", "mage-parse@0.2.0")
            .arg("--build=abl")
            .input("model.mg", Digest::of(b"net M { }"))
            .output("model.abl")
    }

    #[test]
    fn key_is_stable_across_calls() {
        assert_eq!(base().key(), base().key());
    }

    #[test]
    fn name_and_cost_do_not_affect_the_key() {
        let a = base();
        let mut b = base();
        b.name = "a completely different label".into();
        b.cost = 9_999;
        assert_eq!(a.key(), b.key(), "renaming a target must not invalidate its cache entry");
    }

    #[test]
    fn tool_version_affects_the_key() {
        let a = base();
        let b = base().tool_for_test("mage-parse@0.3.0");
        assert_ne!(a.key(), b.key(), "a new compiler must produce a new key");
    }

    #[test]
    fn input_content_affects_the_key() {
        let a = base();
        let b = Action::new("lower", "mage-parse@0.2.0")
            .arg("--build=abl")
            .input("model.mg", Digest::of(b"net M { layer fc: Linear(1,1); }"))
            .output("model.abl");
        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn input_declaration_order_does_not_matter() {
        let d1 = Digest::of(b"one");
        let d2 = Digest::of(b"two");
        let a = Action::new("t", "tool@1").input("a", d1.clone()).input("b", d2.clone());
        let b = Action::new("t", "tool@1").input("b", d2).input("a", d1);
        assert_eq!(a.key(), b.key(), "two agents declaring the same inputs must share a cache entry");
    }

    #[test]
    fn argument_order_does_matter() {
        let a = Action::new("t", "tool@1").arg("-o").arg("x");
        let b = Action::new("t", "tool@1").arg("x").arg("-o");
        assert_ne!(a.key(), b.key(), "argument order is semantic");
    }

    #[test]
    fn env_participates_in_the_key() {
        let a = Action::new("t", "tool@1");
        let b = Action::new("t", "tool@1").env("OPT_LEVEL", "3");
        assert_ne!(a.key(), b.key());
    }

    #[test]
    fn accelerator_partitions_the_cache() {
        let cpu = Action::new("t", "tool@1");
        let gpu = Action::new("t", "tool@1").platform(Platform::any().with_accelerator("cuda"));
        assert_ne!(cpu.key(), gpu.key(), "a device-pinned action needs its own cache line");
    }

    #[test]
    fn device_independent_actions_share_across_hosts() {
        // The common case: a lowering keyed identically no matter which host
        // asks for it, so a heterogeneous fleet shares one entry.
        let a = Action::new("t", "tool@1").platform(Platform::any());
        let b = Action::new("t", "tool@1").platform(Platform::any());
        assert_eq!(a.key(), b.key());
    }

    #[test]
    fn concatenation_cannot_forge_a_collision() {
        // Without length prefixing these two hash identically.
        let a = Action::new("t", "ab").arg("c");
        let b = Action::new("t", "a").arg("bc");
        assert_ne!(a.key(), b.key(), "length prefixing must prevent field-boundary collisions");
    }

    #[test]
    fn outputs_requested_affect_the_key() {
        let a = Action::new("t", "tool@1").output("x");
        let b = Action::new("t", "tool@1").output("x").output("y");
        assert_ne!(a.key(), b.key());
    }

    // Small helper so the tool-version test reads clearly.
    impl Action {
        fn tool_for_test(mut self, t: &str) -> Self {
            self.tool = t.to_string();
            self
        }
    }
}
