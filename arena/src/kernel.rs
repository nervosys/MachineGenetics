//! # The trusted kernel
//!
//! Plan task 3.1. The redesign keeps a small trusted kernel and puts
//! everything else in MAGE, where the loop can vary it. The kernel is what the
//! loop must *not* be able to vary, because it decides whether the loop
//! improved: the typechecker, the metered evaluator, the meter, the held-out
//! store, the gate and the journal. This module is where the arena gets each
//! of them, and it owns the one that was not owned anywhere: held-out data.
//!
//! Two capabilities define the boundary, and each is refused outside it at
//! two levels.
//!
//! | capability | in MAGE programs | in Rust |
//! |---|---|---|
//! | read held-out data | the `heldout` effect, allowed only to `@role(evaluator)` and `@role(gate)` (`E0551`) | [`Corpus`]'s bytes are private; the only reader is [`Corpus::score`] |
//! | move authority | the `promote` effect, allowed only to `@role(gate)` | `Lineage::promote` takes a [`germline::gate::Approval`], which only [`germline::gate::Episode::approve`] mints |
//!
//! Both Rust halves are enforced by the compiler, and tested as such: the
//! `compile_fail` examples below and on `Approval` fail to build if the
//! privacy is removed, which was checked by removing it.
//!
//! **What the boundary does not cover.** A learner sees held-out bytes when it
//! is scored, as any model must. It sees them through `&self`, so it cannot
//! train on them without interior mutability, which no learner here has; a
//! learner proposed by the loop would need that checked. And
//! `germline::lineage::Lineage` is `Deserialize`, so a forged lineage file
//! could name a champion; that is closed by verifying the journal's hash chain
//! on resume, which is plan 8.3.

use crate::learner::ByteLearner;
use serde::{Deserialize, Serialize};

pub use crate::energy::Meter;
pub use crate::substrate::Substrate;
pub use germline::gate::{Approval, Episode};
pub use germline::journal::Journal;
pub use mage_prototype::eval::run_bounded;

/// A held-out corpus: data no agent can reach, used only for scoring.
///
/// Anyone can build one from bytes they already have. No one outside the
/// kernel can read the bytes back:
///
/// ```compile_fail
/// let c = arena::kernel::Corpus::new("text", b"held out".to_vec());
/// let leaked: &Vec<u8> = &c.bytes;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corpus {
    name: String,
    #[serde(skip)]
    bytes: Vec<u8>,
    /// SHA-256 of the bytes, so a report states exactly what it was scored on.
    sha256: String,
    len: usize,
}

impl Corpus {
    pub fn new(name: impl Into<String>, bytes: Vec<u8>) -> Corpus {
        use sha2::{Digest, Sha256};
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        Corpus { name: name.into(), len: bytes.len(), bytes, sha256 }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `learner`'s bits per byte on this corpus: the one way held-out bytes
    /// leave the kernel, and only as far as a `&self` method.
    pub fn score(&self, learner: &dyn ByteLearner) -> f64 {
        learner.bits_per_byte(&self.bytes)
    }

    /// Seconds per kilobyte to score up to `cap` bytes of this corpus.
    pub fn latency_s_per_kb(&self, learner: &dyn ByteLearner, cap: usize) -> f64 {
        let sample = &self.bytes[..self.bytes.len().min(cap)];
        let t = std::time::Instant::now();
        let _ = learner.bits_per_byte(sample);
        t.elapsed().as_secs_f64() / (sample.len().max(1) as f64 / 1024.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mage_prototype::hir::{DiagnosticCategory, Severity, ROLES};
    use mage_prototype::{effects, lexer, parser};

    /// Whether the language refuses `body` in a function of role `role` for
    /// a role violation (`E0551`).
    fn role_refuses(role: &str, body: &str) -> bool {
        let src = format!("@role({role})\nf k(s: usize) -> usize {{\n    {body}\n    s\n}}\n");
        let module = parser::parse(&lexer::lex(&src)).expect("parses");
        effects::infer_effects(&module)
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error && d.category == Some(DiagnosticCategory::RoleViolation))
    }

    #[test]
    fn only_the_kernels_roles_hold_its_capabilities() {
        // Every role the language defines, against both capabilities. The
        // expected table is written out, not derived from `ROLES`, so a change
        // to the language's grant is a failing test and not a silent widening.
        let expected: &[(&str, bool, bool)] = &[
            // role,       may read held-out, may promote
            ("candidate", false, false),
            ("learner", false, false),
            ("evaluator", true, false),
            ("gate", true, true),
        ];
        assert_eq!(
            ROLES.iter().map(|(r, _)| *r).collect::<Vec<_>>(),
            expected.iter().map(|(r, ..)| *r).collect::<Vec<_>>(),
            "a role was added or removed; decide its place on the boundary here"
        );
        for &(role, heldout, promote) in expected {
            assert_eq!(!role_refuses(role, "heldout.read(\"test\")"), heldout, "{role} reading held-out data");
            assert_eq!(!role_refuses(role, "gate.promote(1)"), promote, "{role} promoting");
        }
    }

    #[test]
    fn a_corpus_is_scored_without_being_handed_out() {
        let c = Corpus::new("text", b"abcabcabc".to_vec());
        let l = crate::learner::Learner::new(crate::learner::LearnerConfig::default());
        assert_eq!(c.score(&l), l.bits_per_byte(b"abcabcabc"));
        assert_eq!((c.name(), c.len()), ("text", 9));
    }
}
