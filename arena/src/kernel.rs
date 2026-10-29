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
//! learner proposed by the loop would need that checked.
//!
//! A lineage names the champion, so it is part of the boundary too. It is not
//! `Deserialize`: a restarted run rebuilds it with
//! `germline::lineage::Lineage::replay`, which verifies the journal's hash
//! chain and re-applies each succession event under the rules the live
//! transition enforced (plan 8.3).

use crate::energy::{self, Reading};
use crate::learner::ByteLearner;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use crate::energy::Meter;
pub use crate::substrate::Substrate;
use crate::substrate::{Outcome, Prepared};
pub use germline::gate::{Approval, Episode};
pub use germline::journal::Journal;
pub use mage_prototype::eval::run_bounded;

/// Ledger accounts: what the loop spends energy on.
pub const PROGRAMS: &str = "programs";
pub const TRAINING: &str = "training";
pub const PROBES: &str = "probes";

/// Every metered joule the loop spends, against a fixed budget (plan 3.5).
///
/// The budget is decision D2's fixed unit of compute. Agents' proposing and
/// programs, learner training and probe scoring are charged to it. Held-out
/// evaluation is metered and reported but not charged: it is the kernel
/// measuring the loop, not the loop's work, and charging it would make
/// evaluating more often look like doing more.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnergyLedger {
    pub budget_joules: Option<f64>,
    /// Joules charged, by account.
    pub charged: BTreeMap<String, f64>,
    /// The agents' share of the charges, by role (plan 7.2): who in the loop
    /// spent the budget, beside what it was spent on.
    #[serde(default)]
    pub by_role: BTreeMap<String, f64>,
    /// Joules spent on held-out evaluation, not charged.
    pub evaluation_joules: f64,
    /// False when any charged or evaluation reading was an estimate.
    pub all_measured: bool,
    /// Spans no meter produced a figure for, so their energy is missing
    /// from the totals rather than counted as zero silently.
    pub unmetered_spans: usize,
    /// Why the run stopped early, if it did.
    pub halted: Option<String>,
}

impl EnergyLedger {
    pub fn new(budget_joules: Option<f64>) -> EnergyLedger {
        EnergyLedger { budget_joules, all_measured: true, ..EnergyLedger::default() }
    }

    /// Charge a span's `readings` to `account`. Returns the joules and
    /// whether they were measured, or `None` when no meter read anything.
    pub fn charge(&mut self, account: &str, readings: &[Reading]) -> Option<(f64, bool)> {
        let reading = energy::total(readings);
        match reading {
            Some((j, measured)) => {
                *self.charged.entry(account.to_string()).or_insert(0.0) += j;
                self.all_measured &= measured;
            }
            None => self.unmetered_spans += 1,
        }
        reading
    }

    /// Charge a span an agent acting in `role` spent, to `account`.
    pub fn charge_role(&mut self, account: &str, role: &str, readings: &[Reading]) -> Option<(f64, bool)> {
        let reading = self.charge(account, readings);
        if let Some((j, _)) = reading {
            *self.by_role.entry(role.to_string()).or_insert(0.0) += j;
        }
        reading
    }

    /// Record held-out evaluation's energy, which is not charged.
    pub fn evaluate(&mut self, readings: &[Reading]) {
        if let Some((j, measured)) = energy::total(readings) {
            self.evaluation_joules += j;
            self.all_measured &= measured;
        }
    }

    pub fn account(&self, account: &str) -> f64 {
        self.charged.get(account).copied().unwrap_or(0.0)
    }

    /// Joules charged against the budget.
    pub fn total(&self) -> f64 {
        self.charged.values().sum()
    }

    /// Whether the budget is spent. Never, without one.
    pub fn exhausted(&self) -> bool {
        self.budget_joules.is_some_and(|b| self.total() >= b)
    }
}

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

    /// This corpus with `canary` planted in it (plan 7.5): a string no agent
    /// should be able to write unless held-out bytes reached it. The hash and
    /// length are of the planted bytes, so a report states what was scored.
    pub fn with_canary(self, canary: &[u8]) -> Corpus {
        let mut bytes = self.bytes;
        bytes.push(b'\n');
        bytes.extend_from_slice(canary);
        bytes.push(b'\n');
        Corpus::new(self.name, bytes)
    }

    /// This corpus cut into `k` contiguous, disjoint slices that together cover
    /// every byte of it (plan 7.5). Slice `i` (numbered from 1) is named
    /// `<name>#i/k`. Each slice but the last is `len / k` bytes, and the last
    /// takes the remainder. `k = 1` returns the corpus unchanged, under its
    /// plain name, whatever its size.
    ///
    /// Refused when any slice would be under [`MIN_SLICE_BYTES`], so a slice
    /// too small to score reliably is an error at start-up rather than noise
    /// in a report. The slices are the kernel's to make, because only the
    /// kernel reads held-out bytes.
    pub fn slices(&self, k: usize) -> Result<Vec<Corpus>, String> {
        if k <= 1 {
            return Ok(vec![self.clone()]);
        }
        let width = self.bytes.len() / k;
        if width < MIN_SLICE_BYTES {
            return Err(format!(
                "held-out corpus `{}` is {} bytes, which cut into {k} slices gives slices of {width} bytes; \
                 the minimum slice is {MIN_SLICE_BYTES} bytes (use fewer slices or larger held-out data)",
                self.name,
                self.bytes.len()
            ));
        }
        Ok((0..k)
            .map(|i| {
                let start = i * width;
                let end = if i + 1 == k { self.bytes.len() } else { start + width };
                Corpus::new(format!("{}#{}/{k}", self.name, i + 1), self.bytes[start..end].to_vec())
            })
            .collect())
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

/// The smallest held-out slice the arena will score on (plan 7.5).
pub const MIN_SLICE_BYTES: usize = 1024;

/// Whether `output` carries `canary`: held-out bytes reached an agent.
pub fn leaks(output: &[u8], canary: &[u8]) -> bool {
    !canary.is_empty() && output.windows(canary.len()).any(|w| w == canary)
}

/// Check an attacker's claim that `variant` behaves exactly as `parent`
/// (plan 7.4): both produce bytes, the same bytes, on every seed in `seeds`,
/// and the variant produces the same bytes twice on the first. Both run
/// directly, never from a cache, since a cache would answer the second run
/// from the first. Every run is under the substrate's fuel and output bounds.
pub fn verify_preserving(sub: &Substrate, parent: &Prepared, variant: &Prepared, seeds: &[u64]) -> Result<(), String> {
    let bytes = |p: &Prepared, s: u64| match sub.execute(p, s) {
        Outcome::Bytes { bytes, .. } => Some(bytes),
        Outcome::Refused(..) => None,
    };
    for &s in seeds {
        match (bytes(parent, s), bytes(variant, s)) {
            (Some(a), Some(b)) if a == b => {}
            (Some(_), Some(_)) => return Err(format!("seed {s}: the variant's output differs from its parent's")),
            _ => return Err(format!("seed {s}: a refusal, so nothing to compare")),
        }
    }
    if let Some(&s) = seeds.first() {
        if bytes(variant, s) != bytes(variant, s) {
            return Err(format!("seed {s}: the variant is not deterministic"));
        }
    }
    Ok(())
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
            ("proposer", false, false),
            ("mutator", false, false),
            ("attacker", false, false),
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
    fn every_agent_role_is_a_language_role() {
        // An agent's role is the effect ceiling its MAGE runs under, so each
        // must name one the typechecker enforces.
        for role in crate::agents::Role::ALL {
            assert!(ROLES.iter().any(|(r, _)| *r == role.name()), "{} is not a language role", role.name());
        }
    }

    #[test]
    fn a_canary_is_planted_in_the_corpus_and_found_in_any_output_that_carries_it() {
        let plain = Corpus::new("text", b"held out".to_vec());
        let planted = Corpus::new("text", b"held out".to_vec()).with_canary(b"CANARY-1");
        assert_eq!(planted.len(), plain.len() + 10);
        assert_ne!(planted.sha256(), plain.sha256(), "the report names the bytes actually scored");
        assert!(leaks(b"xxCANARY-1yy", b"CANARY-1"));
        assert!(!leaks(b"CANARY-", b"CANARY-1"));
        assert!(!leaks(b"anything", b""), "an empty canary detects nothing");
    }

    #[test]
    fn a_corpus_is_scored_without_being_handed_out() {
        let c = Corpus::new("text", b"abcabcabc".to_vec());
        let l = crate::learner::Learner::new(crate::learner::LearnerConfig::default());
        assert_eq!(c.score(&l), l.bits_per_byte(b"abcabcabc"));
        assert_eq!((c.name(), c.len()), ("text", 9));
    }

    #[test]
    fn slices_cover_the_corpus_exactly_disjoint_and_named_by_index() {
        // 3 KiB + 7 bytes: thirds of 1026 bytes, so the remainder lands in the
        // last slice and every slice clears the minimum.
        let bytes: Vec<u8> = (0..3 * 1024 + 7).map(|i| (i % 251) as u8).collect();
        let c = Corpus::new("text", bytes.clone());
        for k in [2usize, 3] {
            let slices = c.slices(k).expect("slices are at least 1 KiB");
            assert_eq!(slices.len(), k);
            let joined: Vec<u8> = slices.iter().flat_map(|s| s.bytes.iter().copied()).collect();
            assert_eq!(joined, bytes, "k = {k}: the slices' concatenation is the corpus");
            for (i, s) in slices.iter().enumerate() {
                assert_eq!(s.name(), format!("text#{}/{k}", i + 1));
            }
            let widths: Vec<usize> = slices.iter().map(Corpus::len).collect();
            assert_eq!(widths[..k - 1].iter().min(), widths[..k - 1].iter().max(), "all but the last are equal");
            assert!(widths[k - 1] >= widths[0], "the last takes the remainder");
        }
        let one = c.slices(1).expect("one slice is the corpus");
        assert_eq!((one.len(), one[0].name(), one[0].len()), (1, "text", bytes.len()));
    }

    #[test]
    fn a_slice_under_the_minimum_is_refused_and_one_slice_is_never_refused() {
        let small = Corpus::new("text", vec![b'a'; 1500]);
        assert!(small.slices(2).is_err(), "750-byte slices are under the 1 KiB minimum");
        assert_eq!(small.slices(1).expect("no slicing, no minimum").len(), 1);
        assert!(Corpus::new("text", vec![b'a'; 2048]).slices(2).is_ok(), "exactly 1 KiB each is allowed");
    }
}
