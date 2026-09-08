//! The verdict vocabulary — what kind of evidence stands behind a claim.
//!
//! MAGE makes claims of two different epistemic kinds and, until now, had words
//! for only one of them.
//!
//! * **Deductive.** `verify.rs` checks contracts and `differentiable.rs`
//!   computes a property over the call graph. Both quantify over *every*
//!   execution: "this function has a derivative" is a statement about all
//!   inputs, not about the ones anyone tried.
//! * **Inductive.** Gradient checking compares an exact derivative against a
//!   central difference at *n* sampled points. It cannot establish a property
//!   for every input, and it is the standard practice in every AD
//!   implementation because it catches the thing deduction cannot: an
//!   implementation that computes the wrong derivative of a function that
//!   genuinely has one.
//!
//! Before this module there was no way to say "evidenced at *n* samples"
//! anywhere in this language, so the two kinds had to share one word or go
//! unsaid. Sharing a word is how an inductive result gets quoted as a proof.
//!
//! ## The vocabulary is borrowed, deliberately
//!
//! `../StatodynamicAnalysis` designed this lattice first, for the same problem
//! in a different setting — correlating static findings with runtime evidence.
//! Its README states the rule this module exists to obey:
//!
//! > | suspect | never executed | **Unreached** | the claim is untested, *not* clean |
//! >
//! > That third row is the one most tools get wrong. Treating "no runtime
//! > defect" as evidence of correctness, without checking whether the code ever
//! > ran, silently converts a coverage hole into a clean bill of health.
//!
//! and, for the deductive half:
//!
//! > **A component nobody specified verifies vacuously**, and that is reported,
//! > not counted as a pass.
//!
//! **MAGE's own verifier had that second defect.** `VerifyStatus::Trivial` meant
//! *no contracts to verify*, and the reporter skipped its rows — so a function
//! nobody had specified was invisible, while the summary line counted it. A
//! module with one contract across two functions reported `Contracts checked: 2`.
//! The vacuous case was not merely unlabelled; it was **absorbed into a number
//! that read as coverage**.
//!
//! The correspondence, stated so it can be argued with rather than inferred:
//!
//! | MAGE | StatodynamicAnalysis | why the names differ |
//! |---|---|---|
//! | `Proved` | `Proved` | same claim: no execution violates it |
//! | `Evidenced { n }` | `Refuted` (of a defect) | both are "it held on the runs we did"; MAGE's is a positive claim about a derivative, the sibling's a negative one about a finding |
//! | `Unspecified` | vacuous verification | nothing was claimed. Reported, never counted as a pass |
//! | `Unreached { why }` | `Unreached` | nothing checked it. **The claim is untested, not clean** |
//! | `Refuted { at }` | `Confirmed` (of a defect) | the claim was checked and it failed |
//!
//! The sharing is one-directional today: these words are adopted here, and
//! nothing in `StatodynamicAnalysis` has been changed to match. Saying so is
//! better than implying a coordination that has not happened.
//!
//! ## What is deliberately *not* here
//!
//! There is no ordering on `Evidence`, and no method returning a number between
//! zero and one. Both would invite exactly the collapse the module exists to
//! prevent: a lattice would let `Evidenced { n }` sit above or below `Proved`
//! as though they measured the same thing on one scale, and a score would
//! average a proof together with a sample count. The sibling refuses the score
//! for the same reason — "`VerificationLedger` has no method returning a number
//! between zero and one".

use serde::{Deserialize, Serialize};

/// What kind of evidence stands behind one claim.
///
/// Deliberately not `Ord`: see the module docs. Two verdicts of different kinds
/// are not comparable, and pretending otherwise is the error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Evidence {
    /// **Deductive.** Established for every execution, by the named pass.
    ///
    /// Carries *what established it*, because a proof is only as good as the
    /// thing that produced it, and "verified" with no attribution is a claim
    /// nobody can go and check.
    Proved { by: String },

    /// **Inductive.** It held at `n` sampled points, by the named method.
    ///
    /// This is not proof and the type says so: `n` is in the value, so a
    /// renderer cannot print this verdict without printing the sample count
    /// beside it.
    Evidenced { n: usize, by: String },

    /// Nothing was claimed here, so nothing was verified.
    ///
    /// **Reported, never counted as a pass.** A function with no contracts and
    /// a function whose contracts all hold are different facts, and the whole
    /// reason this variant exists is that they had been rendered identically —
    /// by omission.
    Unspecified,

    /// Nothing checked it. *The claim is untested, not clean.*
    ///
    /// Distinct from `Unspecified`: there **was** a claim, and no analysis
    /// reached it. Carries why, so the gap is actionable rather than silent.
    Unreached { why: String },

    /// It was checked, and it failed. Carries where.
    Refuted { at: String },
}

impl Evidence {
    /// A one-word label, for a table cell.
    pub fn label(&self) -> &'static str {
        match self {
            Evidence::Proved { .. } => "proved",
            Evidence::Evidenced { .. } => "evidenced",
            Evidence::Unspecified => "unspecified",
            Evidence::Unreached { .. } => "unreached",
            Evidence::Refuted { .. } => "refuted",
        }
    }

    /// Is this the kind of verdict that quantifies over every execution?
    pub fn is_deductive(&self) -> bool {
        matches!(self, Evidence::Proved { .. })
    }

    /// Does this verdict say the claim *holds*, by evidence of any kind?
    ///
    /// `Unspecified` and `Unreached` are **not** holds, and that is the whole
    /// point: an absence of evidence answers `false` here, so a caller counting
    /// "how many hold" cannot accidentally count the ones nobody looked at.
    pub fn holds(&self) -> bool {
        matches!(
            self,
            Evidence::Proved { .. } | Evidence::Evidenced { .. }
        )
    }

    /// The verdict as a sentence, with its qualification attached.
    ///
    /// `Evidenced` never renders without its sample count. A caller who wants
    /// "evidenced" alone has to reach past this method to get it, which is the
    /// intended amount of friction.
    pub fn describe(&self) -> String {
        match self {
            Evidence::Proved { by } => format!("proved, by {by}"),
            Evidence::Evidenced { n, by } => {
                format!("evidenced at {n} sample{}, by {by} — not a proof", plural(*n))
            }
            Evidence::Unspecified => "unspecified — nothing was claimed here".into(),
            Evidence::Unreached { why } => {
                format!("unreached — untested, not clean: {why}")
            }
            Evidence::Refuted { at } => format!("refuted at {at}"),
        }
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// A tally that keeps the kinds apart.
///
/// One counter per verdict, and **no total that mixes them**: "8 of 10 verified"
/// over a population where two were never specified is the sentence this whole
/// module exists to make unwritable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    pub proved: usize,
    pub evidenced: usize,
    pub unspecified: usize,
    pub unreached: usize,
    pub refuted: usize,
}

impl Tally {
    pub fn add(&mut self, e: &Evidence) {
        match e {
            Evidence::Proved { .. } => self.proved += 1,
            Evidence::Evidenced { .. } => self.evidenced += 1,
            Evidence::Unspecified => self.unspecified += 1,
            Evidence::Unreached { .. } => self.unreached += 1,
            Evidence::Refuted { .. } => self.refuted += 1,
        }
    }

    /// Every subject considered, whatever the verdict.
    pub fn subjects(&self) -> usize {
        self.proved + self.evidenced + self.unspecified + self.unreached + self.refuted
    }

    /// Subjects with no verdict either way — nothing claimed, or nothing
    /// checked. The number a summary must not hide.
    pub fn unadjudicated(&self) -> usize {
        self.unspecified + self.unreached
    }

    /// A summary line that cannot omit the coverage hole.
    ///
    /// It names `unspecified` and `unreached` explicitly and always, including
    /// when they are zero, because a field that disappears when empty teaches a
    /// reader to stop looking for it.
    pub fn summary(&self) -> String {
        format!(
            "{} subject(s): {} proved, {} evidenced, {} refuted, \
             {} unspecified, {} unreached",
            self.subjects(),
            self.proved,
            self.evidenced,
            self.refuted,
            self.unspecified,
            self.unreached
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The property this module exists for.** Neither absence counts as a
    /// hold, so a caller tallying "how many are fine" cannot sweep up the ones
    /// nobody looked at.
    #[test]
    fn an_absence_of_evidence_is_not_a_hold() {
        assert!(!Evidence::Unspecified.holds());
        assert!(!Evidence::Unreached { why: "no pass".into() }.holds());
        assert!(Evidence::Proved { by: "x".into() }.holds());
        assert!(Evidence::Evidenced { n: 5, by: "y".into() }.holds());
        assert!(!Evidence::Refuted { at: "z".into() }.holds());
    }

    /// An inductive verdict is not a deductive one, and cannot be mistaken for
    /// one by a caller asking the direct question.
    #[test]
    fn evidenced_is_not_proved() {
        let e = Evidence::Evidenced { n: 1000, by: "central differences".into() };
        assert!(!e.is_deductive());
        assert!(e.holds(), "it does hold — it is just not a proof");
    }

    /// `Evidenced` cannot be described without its sample count. A rendering
    /// that says "evidenced" alone is a rendering that reads as a proof.
    #[test]
    fn evidenced_always_renders_its_sample_count() {
        let d = Evidence::Evidenced { n: 32, by: "central differences".into() }.describe();
        assert!(d.contains("32"), "{d}");
        assert!(d.contains("not a proof"), "{d}");
    }

    /// The two absences are different facts and must not be merged.
    /// `Unspecified` is "nobody claimed anything"; `Unreached` is "something
    /// was claimed and nothing checked it".
    #[test]
    fn the_two_absences_stay_distinct() {
        let a = Evidence::Unspecified;
        let b = Evidence::Unreached { why: "no analysis models this".into() };
        assert_ne!(a, b);
        assert_ne!(a.label(), b.label());
        assert!(b.describe().contains("untested, not clean"), "{}", b.describe());
    }

    /// The summary names the coverage hole even when it is empty, so a reader
    /// never learns that the field only appears when there is bad news.
    #[test]
    fn the_summary_always_names_the_coverage_hole() {
        let mut t = Tally::default();
        t.add(&Evidence::Proved { by: "p".into() });
        let s = t.summary();
        assert!(s.contains("0 unspecified"), "{s}");
        assert!(s.contains("0 unreached"), "{s}");
        assert_eq!(t.unadjudicated(), 0);

        t.add(&Evidence::Unspecified);
        assert_eq!(t.unadjudicated(), 1);
        assert_eq!(t.subjects(), 2);
        assert!(t.summary().contains("1 unspecified"));
    }
}
