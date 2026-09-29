//! Content-addressed **definition** store (plan 2.4).
//!
//! [`super::BlockStore`] addresses a `block` by the SHA-256 of its source, so
//! the same block written with different names is two entries. This store
//! addresses a function definition by its **canonical** hash: its normal form
//! with bound names renamed in binding order and its own name dropped
//! (`mage_prototype::canon`). Alpha-equivalent definitions are one entry, which
//! is what the self-improvement loop needs. Its agents rewrite programs
//! constantly, and "the same idea, renamed" must be recognisable as such in
//! storage and in the journal.
//!
//! `forge` does not parse MAGE, so the hash comes from a [`Canonicalizer`]
//! that does, the same seam [`super::blocks::EffectOracle`] uses for effects.
//! The store never accepts a hash from its caller: it asks the canonicalizer,
//! for the exact bytes being stored, every time. A caller that could name the
//! hash could file one definition under another's address.

use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// What a canonicalizer reports for one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canonical {
    /// The definition's canonical hash: equal for alpha-equivalent sources.
    pub exact: String,
    /// The hash with literal values erased too: equal for sources that differ
    /// only in their constants.
    pub shape: String,
}

/// Computes canonical hashes. Implemented where MAGE can be parsed.
pub trait Canonicalizer {
    /// The canonical hashes of the one definition in `source`, or why it has
    /// none (it doesn't parse, fails a gate, or holds no single definition).
    fn canonicalize(&self, source: &str) -> Result<Canonical, String>;
}

/// One stored definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionHandle {
    /// The canonical hash: the definition's address.
    pub exact: String,
    pub shape: String,
    /// SHA-256 of the source stored for it: the first one published.
    pub sha256: String,
    /// How many times it was published, counting alpha-equivalent variants:
    /// how often the loop arrived at this definition.
    pub publishes: u64,
}

/// A content-addressed store of definitions under `<root>/definitions/`.
pub struct DefinitionStore {
    root: PathBuf,
}

impl DefinitionStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn dir(&self) -> PathBuf {
        self.root.join("definitions")
    }

    fn index_path(&self) -> PathBuf {
        self.dir().join("index.json")
    }

    fn source_path(&self, exact: &str) -> PathBuf {
        self.dir().join(format!("{exact}.mg"))
    }

    /// Every stored definition, in first-publish order.
    pub fn list(&self) -> Vec<DefinitionHandle> {
        std::fs::read_to_string(self.index_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Store `source` under the canonical hash `canon` computes for it. An
    /// alpha-equivalent definition already stored is counted, not duplicated,
    /// and keeps its first source.
    pub fn publish(&self, source: &str, canon: &dyn Canonicalizer) -> Result<DefinitionHandle, String> {
        let c = canon.canonicalize(source)?;
        if c.exact.is_empty() || !c.exact.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            // The hash becomes a file name; anything else is refused rather
            // than escaped.
            return Err(format!("the canonicalizer returned an unusable hash `{}`", c.exact));
        }
        let mut index = self.list();
        let handle = match index.iter_mut().find(|h| h.exact == c.exact) {
            Some(h) => {
                h.publishes += 1;
                h.clone()
            }
            None => {
                std::fs::create_dir_all(self.dir()).map_err(|e| format!("creating {}: {e}", self.dir().display()))?;
                std::fs::write(self.source_path(&c.exact), source).map_err(|e| format!("writing definition: {e}"))?;
                let h = DefinitionHandle {
                    exact: c.exact,
                    shape: c.shape,
                    sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
                    publishes: 1,
                };
                index.push(h.clone());
                h
            }
        };
        let json = serde_json::to_string_pretty(&index).map_err(|e| format!("encoding index: {e}"))?;
        std::fs::write(self.index_path(), json).map_err(|e| format!("writing index: {e}"))?;
        Ok(handle)
    }

    /// The stored source for `exact`, verified against the SHA-256 the index
    /// recorded for it. A file edited after publishing is refused, not served.
    pub fn get(&self, exact: &str) -> Result<String, String> {
        let h = self.list().into_iter().find(|h| h.exact == exact).ok_or_else(|| format!("no definition {exact}"))?;
        let src = std::fs::read_to_string(self.source_path(exact)).map_err(|e| format!("reading {exact}: {e}"))?;
        let actual = format!("{:x}", Sha256::digest(src.as_bytes()));
        if actual != h.sha256 {
            return Err(format!("definition {exact} fails verification: stored {actual}, indexed {}", h.sha256));
        }
        Ok(src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for MAGE's canonicalizer: names are the words after `f`,
    /// so dropping the first word of the source and hashing the rest makes
    /// `f a(x) …` and `f b(x) …` equal. Digits erased gives the shape.
    struct DropName;
    impl Canonicalizer for DropName {
        fn canonicalize(&self, source: &str) -> Result<Canonical, String> {
            let body = source.split_once('(').map(|(_, b)| b).ok_or("no definition")?;
            let hex = |s: &str| format!("{:x}", Sha256::digest(s.as_bytes()));
            let shape: String = body.chars().filter(|c| !c.is_ascii_digit()).collect();
            Ok(Canonical { exact: hex(body), shape: hex(&shape) })
        }
    }

    fn temp_store(tag: &str) -> DefinitionStore {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        DefinitionStore::new(std::env::temp_dir().join(format!("forge-defstore-{tag}-{}-{n}", std::process::id())))
    }

    #[test]
    fn equivalent_definitions_share_one_address_and_count_as_two_publishes() {
        let s = temp_store("dedup");
        let a = s.publish("f a(x: usize) -> usize { x + 1 }", &DropName).unwrap();
        let b = s.publish("f b(x: usize) -> usize { x + 1 }", &DropName).unwrap();
        assert_eq!(a.exact, b.exact);
        assert_eq!(b.publishes, 2);
        assert_eq!(s.list().len(), 1);
        assert_eq!(s.get(&a.exact).unwrap(), "f a(x: usize) -> usize { x + 1 }", "the first source is kept");

        let c = s.publish("f a(x: usize) -> usize { x + 2 }", &DropName).unwrap();
        assert_ne!(c.exact, a.exact, "a different constant is a different definition");
        assert_eq!(c.shape, a.shape, "and the same shape");
    }

    #[test]
    fn a_source_edited_after_publishing_is_refused() {
        let s = temp_store("tamper");
        let h = s.publish("f a(x: usize) -> usize { x }", &DropName).unwrap();
        std::fs::write(s.source_path(&h.exact), "f a(x: usize) -> usize { 0 }").unwrap();
        assert!(s.get(&h.exact).unwrap_err().contains("fails verification"));
    }

    #[test]
    fn the_address_comes_from_the_canonicalizer_and_must_be_a_plain_hash() {
        struct Escapes;
        impl Canonicalizer for Escapes {
            fn canonicalize(&self, _: &str) -> Result<Canonical, String> {
                Ok(Canonical { exact: "../../escape".into(), shape: "s".into() })
            }
        }
        let s = temp_store("escape");
        assert!(s.publish("f a() {}", &Escapes).unwrap_err().contains("unusable hash"));
        assert!(s.publish("no definition here", &DropName).is_err(), "a canonicalizer's refusal is the store's");
        assert!(s.list().is_empty());
    }
}
