//! Content-addressed **block** store — the registry's storage layer for
//! architecture building blocks (`block Name(params) { … }`).
//!
//! This is the shared, cross-project backing for the registry-handle workflow:
//! a project's local `blocks/*.mg` library resolves handles from one directory;
//! this store resolves them from a **shared** registry root (`$FORGE_REGISTRY`,
//! or `~/.forge/registry`) so a block published once is referenceable by name
//! from any project, with the definition off-context.
//!
//! Each block is stored under the SHA-256 of its canonical source
//! (`<root>/blocks/<sha256>.mg`), so an identical definition published twice is
//! **deduplicated** to one artifact — the content-addressed "precompute the
//! lowering, store it once" idea (ARCHITECTURE_DSL §3) at the block grain. The
//! SHA-256 is the integrity/dedup key; the agent still references the block by
//! its short **name** (≈1 token), which the index maps to the content hash.
//!
//! The networked `forge-server` exposes this same store over HTTP; the store
//! itself is pure filesystem + hashing, so it is deterministic and unit-testable
//! without a running server.

use crate::models::BlockHandle;
use std::path::PathBuf;

/// A content-addressed store of published blocks, rooted at a shared directory.
pub struct BlockStore {
    root: PathBuf,
}

impl BlockStore {
    /// Open the default shared registry: `$FORGE_REGISTRY`, else
    /// `$HOME`/`$USERPROFILE` + `.forge/registry`, else `./.forge/registry`.
    pub fn open_default() -> Self {
        let root = std::env::var_os("FORGE_REGISTRY")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".forge").join("registry"));
        Self::new(root)
    }

    /// Open a store at an explicit root (used by tests and the server).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn blocks_dir(&self) -> PathBuf {
        self.root.join("blocks")
    }

    fn index_path(&self) -> PathBuf {
        self.blocks_dir().join("index.json")
    }

    fn block_path(&self, sha: &str) -> PathBuf {
        self.blocks_dir().join(format!("{sha}.mg"))
    }

    /// The published-block index (in publish order). Empty if the store is new
    /// or unreadable — resolution then simply finds nothing (no hard error).
    pub fn list(&self) -> Vec<BlockHandle> {
        std::fs::read_to_string(self.index_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write_index(&self, idx: &[BlockHandle]) -> Result<(), String> {
        std::fs::create_dir_all(self.blocks_dir())
            .map_err(|e| format!("creating registry dir: {e}"))?;
        let json = serde_json::to_string_pretty(idx).map_err(|e| format!("encoding index: {e}"))?;
        std::fs::write(self.index_path(), json).map_err(|e| format!("writing index: {e}"))
    }

    /// Publish every `block` definition found in `src`. Each is stored under its
    /// content hash (deduplicated) and indexed by name. Returns the handles in
    /// source order. Errors only on I/O or if `src` has no block definitions.
    pub fn publish_source(&self, src: &str) -> Result<Vec<BlockHandle>, String> {
        let parsed = split_blocks(src);
        if parsed.is_empty() {
            return Err("no `block Name(...) { ... }` definitions found".into());
        }
        std::fs::create_dir_all(self.blocks_dir())
            .map_err(|e| format!("creating registry dir: {e}"))?;
        let mut idx = self.list();
        let mut published = Vec::new();
        for b in parsed {
            let sha = sha256_hex(&b.source);
            let path = self.block_path(&sha);
            if !path.exists() {
                std::fs::write(&path, &b.source).map_err(|e| format!("storing block: {e}"))?;
            }
            let handle = BlockHandle {
                name: b.name,
                sha256: sha,
                signature: b.signature,
            };
            // Dedup the index by content hash (re-publishing the same bytes is a
            // no-op for the index, not a duplicate entry).
            if !idx.iter().any(|h| h.sha256 == handle.sha256) {
                idx.push(handle.clone());
            }
            published.push(handle);
        }
        self.write_index(&idx)?;
        Ok(published)
    }

    /// Fetch a block's source by its exact content hash.
    pub fn get_by_sha(&self, sha: &str) -> Option<String> {
        std::fs::read_to_string(self.block_path(sha)).ok()
    }

    /// Fetch the most-recently-published block with this name.
    pub fn get_by_name(&self, name: &str) -> Option<String> {
        self.list()
            .into_iter()
            .rev()
            .find(|h| h.name == name)
            .and_then(|h| self.get_by_sha(&h.sha256))
    }
}

/// `$HOME` (Unix) / `$USERPROFILE` (Windows), falling back to `.`.
fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// SHA-256 of a string, lowercase hex.
fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

/// One block definition extracted from a source file.
struct ParsedBlock {
    name: String,
    /// `Name(p1, p2)` — the signature up to the opening brace.
    signature: String,
    /// The full, trimmed `block … { … }` text (what gets content-hashed).
    source: String,
}

/// The names of every top-level `block` defined in `src`.
pub fn block_names(src: &str) -> Vec<String> {
    split_blocks(src).into_iter().map(|b| b.name).collect()
}

/// Whether `name` appears as a whole word in `src` (identifier boundaries).
pub fn mentions_word(src: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(rel) = src[from..].find(name) {
        let start = from + rel;
        let end = start + name.len();
        let before_ok = start == 0 || !src[..start].chars().next_back().map(is_word).unwrap_or(false);
        let after_ok = src[end..].chars().next().map(|c| !is_word(c)).unwrap_or(true);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Split a source file into its top-level `block` definitions by brace-balancing.
/// Tolerant: ignores anything that isn't a `block Name(...) { ... }` form.
fn split_blocks(src: &str) -> Vec<ParsedBlock> {
    let chars: Vec<char> = src.chars().collect();
    let n = chars.len();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        // A `block` keyword at an identifier boundary.
        let is_kw = chars[i..].starts_with(&['b', 'l', 'o', 'c', 'k'])
            && (i == 0 || !is_word(chars[i - 1]))
            && chars.get(i + 5).map(|c| c.is_whitespace()).unwrap_or(false);
        if !is_kw {
            i += 1;
            continue;
        }
        let start = i;
        i += 5; // past "block"
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        let name_start = i;
        while i < n && is_word(chars[i]) {
            i += 1;
        }
        let name: String = chars[name_start..i].iter().collect();
        // Signature: from the name up to the first `{`.
        let mut j = i;
        while j < n && chars[j] != '{' {
            j += 1;
        }
        let signature: String = chars[name_start..j].iter().collect::<String>().trim().to_string();
        if name.is_empty() || j >= n {
            i += 1;
            continue;
        }
        // Balance braces from the first `{`.
        let mut depth = 0usize;
        i = j;
        while i < n {
            match chars[i] {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        let source: String = chars[start..i].iter().collect::<String>().trim().to_string();
        out.push(ParsedBlock { name, signature, source });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "block TransformerBlock(d, h, ff) {\n    layer attn: MultiHeadAttention(d, h);\n    layer norm1: LayerNorm;\n}\n";

    fn temp_store(tag: &str) -> BlockStore {
        let dir = std::env::temp_dir().join(format!("forge-blockstore-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        BlockStore::new(dir)
    }

    #[test]
    fn publish_then_resolve_by_name_and_sha() {
        let store = temp_store("resolve");
        let handles = store.publish_source(SRC).expect("publishes");
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].name, "TransformerBlock");
        assert_eq!(handles[0].signature, "TransformerBlock(d, h, ff)");
        assert_eq!(handles[0].sha256.len(), 64, "sha-256 hex");
        // Resolvable by name and by content hash.
        let by_name = store.get_by_name("TransformerBlock").expect("by name");
        assert!(by_name.contains("MultiHeadAttention"));
        let by_sha = store.get_by_sha(&handles[0].sha256).expect("by sha");
        assert_eq!(by_name, by_sha);
    }

    #[test]
    fn identical_block_is_deduplicated() {
        let store = temp_store("dedup");
        let h1 = store.publish_source(SRC).unwrap();
        let h2 = store.publish_source(SRC).unwrap(); // same bytes again
        assert_eq!(h1[0].sha256, h2[0].sha256, "same content → same hash");
        // The index holds one entry, not two.
        assert_eq!(store.list().len(), 1, "re-publish must not duplicate the index");
    }

    #[test]
    fn splits_multiple_blocks() {
        let two = format!("{SRC}\nblock Tiny(d) {{ layer fc: Linear(d, d); }}\n");
        let store = temp_store("multi");
        let handles = store.publish_source(&two).unwrap();
        assert_eq!(handles.len(), 2);
        assert_eq!(handles[1].name, "Tiny");
        assert_ne!(handles[0].sha256, handles[1].sha256);
    }

    #[test]
    fn mentions_word_respects_boundaries() {
        assert!(mentions_word("stack 12 { TransformerBlock(256, 8, 1024) }", "TransformerBlock"));
        assert!(!mentions_word("MyTransformerBlockX(1)", "TransformerBlock"));
        assert!(!mentions_word("nothing here", "Tiny"));
    }

    #[test]
    fn empty_source_is_an_error() {
        let store = temp_store("empty");
        assert!(store.publish_source("net N { layer a: Linear(2, 2); }").is_err());
    }
}
