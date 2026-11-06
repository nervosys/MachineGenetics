//! A published architecture **block** handle — the registry's index entry for a
//! `block Name(params) { … }` definition.
//!
//! The `sha256` is the content-address (integrity + dedup key); the `name` is
//! the short handle an agent references (≈1 token), and `signature` is the
//! `Name(p1, p2)` shown by `forge block` for progressive disclosure. The block's
//! body lives off-context in the store, keyed by `sha256`.

use serde::{Deserialize, Serialize};

/// One entry in the content-addressed block registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHandle {
    /// Short reference name (the agent-facing handle).
    pub name: String,
    /// SHA-256 of the canonical block source — content address + dedup key.
    pub sha256: String,
    /// `Name(p1, p2)` — the parameter signature, for progressive disclosure.
    pub signature: String,
    /// What is known about the effects this block performs.
    ///
    /// `#[serde(default)]` so an index written before this field existed reads
    /// back as [`Effects::Unchecked`], which is the true statement about those
    /// entries: nobody checked them, and the absence of a field is not a
    /// declaration of purity.
    #[serde(default)]
    pub effects: Effects,
}

/// What is known about the effects a block performs.
///
/// **Not `Vec<String>`.** An empty vector and "nobody looked" are different
/// facts, and only one of them is safe to build a capability argument on. The
/// distinction matters because of which way the error runs: a consumer that
/// reads an unchecked block as pure admits exactly the escalation the check
/// exists to catch, while one that refuses it loses a candidate.
///
/// `forge` stores and hashes blocks; it does not typecheck them. It therefore
/// cannot honestly say a block is pure, and this type is how it declines to —
/// [`Effects::Checked`] is only reachable by an [`EffectOracle`] that read the
/// exact bytes being stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Effects {
    /// Nobody has checked this block. **Not** a claim that it has no effects.
    #[default]
    Unchecked,
    /// An oracle read this exact source and reported these effects.
    ///
    /// `declared` being empty here *is* a purity claim, and a checked one.
    Checked { declared: Vec<String> },
}

impl Effects {
    /// The effects, or `None` if nobody has checked.
    ///
    /// Callers that treat `None` as "no effects" have reintroduced the bug this
    /// type exists to prevent; the shape makes that a choice rather than a
    /// default.
    pub fn declared(&self) -> Option<&[String]> {
        match self {
            Effects::Unchecked => None,
            Effects::Checked { declared } => Some(declared),
        }
    }

    pub fn is_checked(&self) -> bool {
        matches!(self, Effects::Checked { .. })
    }
}
