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
}
