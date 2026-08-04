//! Content-addressed storage and the action cache.
//!
//! Two separate maps, deliberately:
//!
//! - **CAS**: `digest -> bytes`. Immutable, self-verifying, shareable by anyone
//!   who trusts SHA-256. Never needs invalidation, because a digest cannot come
//!   to mean different bytes.
//! - **Action cache**: `action key -> ActionResult`. A *claim* that running this
//!   action produces these output digests. Invalidatable, and — unlike the CAS —
//!   something you might reasonably distrust.
//!
//! Keeping them apart is what makes the fleet safe to share. Workers can pull
//! blobs from an untrusted mirror and verify them on arrival (the digest is the
//! check). Action-cache entries carry a provenance claim and are only as good as
//! the worker that made them, so they are the thing to sign, audit, or refuse.
//!
//! ## Self-verification is not optional
//!
//! [`Cas::get`] rehashes on read and refuses a blob whose contents no longer
//! match its digest. Disks rot, half-written files survive a crash, and a
//! corrupted artifact that flows into a build is a debugging catastrophe: the
//! symptom appears arbitrarily far from the cause. Rehashing costs microseconds
//! and converts that into an immediate, local, obviously-correct error — which
//! [`super::heal`] then repairs by evicting and rebuilding.

use super::{Action, Digest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What running an action produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionResult {
    /// Logical output path -> digest of its contents.
    pub outputs: BTreeMap<String, Digest>,
    pub exit_code: i32,
    /// Captured diagnostics, stored inline: they are small and always wanted
    /// when a build is being explained to an agent.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stderr: String,
    /// Which platform actually produced this, for provenance. Not part of the
    /// key — a device-independent result is valid wherever it was made — but
    /// recorded so an auditor can ask.
    pub produced_on: String,
}

impl ActionResult {
    pub fn ok(produced_on: &str) -> Self {
        ActionResult {
            outputs: BTreeMap::new(),
            exit_code: 0,
            stderr: String::new(),
            produced_on: produced_on.to_string(),
        }
    }

    pub fn with_output(mut self, path: impl Into<String>, d: Digest) -> Self {
        self.outputs.insert(path.into(), d);
        self
    }

    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// Errors from the storage layer. Each is separately actionable by the healer,
/// which is why they are distinct variants rather than one `IoError(String)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CasError {
    /// The blob is absent.
    Missing(Digest),
    /// The blob is present but its contents hash to something else: corruption.
    Corrupt { want: Digest, got: Digest },
    Io(String),
}

impl std::fmt::Display for CasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CasError::Missing(d) => write!(f, "blob {} is missing", d.short()),
            CasError::Corrupt { want, got } => write!(
                f,
                "blob {} is corrupt (contents hash to {})",
                want.short(),
                got.short()
            ),
            CasError::Io(e) => write!(f, "cas io: {e}"),
        }
    }
}

impl std::error::Error for CasError {}

/// A filesystem content-addressed store.
///
/// Blobs are sharded one level by digest prefix. A flat directory is fine until
/// it holds a few hundred thousand entries, at which point directory operations
/// on most filesystems degrade badly — and a build cache reaches that number
/// quickly.
pub struct Cas {
    root: PathBuf,
}

impl Cas {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Cas { root: root.into() }
    }

    fn blob_path(&self, d: &Digest) -> PathBuf {
        let (shard, rest) = d.0.split_at(2.min(d.0.len()));
        self.root.join("cas").join(shard).join(rest)
    }

    /// Store bytes, returning their digest. Idempotent: storing the same bytes
    /// twice is one blob and the second call is nearly free.
    pub fn put(&self, bytes: &[u8]) -> Result<Digest, CasError> {
        let d = Digest::of(bytes);
        let p = self.blob_path(&d);
        if p.exists() {
            return Ok(d);
        }
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CasError::Io(e.to_string()))?;
        }
        // Write to a temp name and rename, so a crash mid-write cannot leave a
        // truncated blob sitting at a digest that promises full contents.
        let tmp = p.with_extension("partial");
        std::fs::write(&tmp, bytes).map_err(|e| CasError::Io(e.to_string()))?;
        std::fs::rename(&tmp, &p).map_err(|e| CasError::Io(e.to_string()))?;
        Ok(d)
    }

    /// Fetch and verify. See the module note on why verification is mandatory.
    pub fn get(&self, d: &Digest) -> Result<Vec<u8>, CasError> {
        let p = self.blob_path(d);
        let bytes = std::fs::read(&p).map_err(|_| CasError::Missing(d.clone()))?;
        let actual = Digest::of(&bytes);
        if &actual != d {
            return Err(CasError::Corrupt { want: d.clone(), got: actual });
        }
        Ok(bytes)
    }

    pub fn has(&self, d: &Digest) -> bool {
        self.blob_path(d).exists()
    }

    /// Drop a blob. Used by the healer on corruption.
    pub fn evict(&self, d: &Digest) -> Result<(), CasError> {
        let p = self.blob_path(d);
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| CasError::Io(e.to_string()))?;
        }
        Ok(())
    }
}

/// `action key -> ActionResult`, persisted as JSON so an agent (or a human, or
/// a different implementation entirely) can read the cache without this code.
pub struct ActionCache {
    root: PathBuf,
}

impl ActionCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        ActionCache { root: root.into() }
    }

    fn entry_path(&self, key: &Digest) -> PathBuf {
        let (shard, rest) = key.0.split_at(2.min(key.0.len()));
        self.root.join("actions").join(shard).join(format!("{rest}.json"))
    }

    /// Look up a previous result. A malformed entry reads as a miss rather than
    /// an error: the worst case is redoing work, and refusing to build because
    /// the cache is unreadable is the wrong trade.
    pub fn get(&self, key: &Digest) -> Option<ActionResult> {
        let raw = std::fs::read_to_string(self.entry_path(key)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn put(&self, key: &Digest, result: &ActionResult) -> Result<(), CasError> {
        let p = self.entry_path(key);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CasError::Io(e.to_string()))?;
        }
        let raw = serde_json::to_string_pretty(result).map_err(|e| CasError::Io(e.to_string()))?;
        std::fs::write(&p, raw).map_err(|e| CasError::Io(e.to_string()))?;
        Ok(())
    }

    pub fn invalidate(&self, key: &Digest) -> Result<(), CasError> {
        let p = self.entry_path(key);
        if p.exists() {
            std::fs::remove_file(&p).map_err(|e| CasError::Io(e.to_string()))?;
        }
        Ok(())
    }
}

/// Both stores rooted at one directory — what a worker or an agent is handed.
pub struct Store {
    pub cas: Cas,
    pub actions: ActionCache,
    root: PathBuf,
    shared: bool,
}

impl Store {
    /// A store private to one machine.
    ///
    /// The default, and the permissive one: "same host, same binaries" is a
    /// reasonable assumption to make about yourself, so results from unverified
    /// toolchains may be cached here.
    pub fn open(root: impl Into<PathBuf>) -> Self {
        let root: PathBuf = root.into();
        Store { cas: Cas::new(&root), actions: ActionCache::new(&root), root, shared: false }
    }

    /// A store other machines read from.
    ///
    /// This turns [`lang::Hermeticity`](super::lang::Hermeticity) from a label
    /// into an enforced rule: an action whose tool is unpinned is *executed*
    /// normally but its claim is never published here, because publishing it
    /// would offer another machine an artifact built by a compiler nobody
    /// verified was the same compiler. The cost is a repeated build; the
    /// alternative cost is a silently wrong binary.
    pub fn open_shared(root: impl Into<PathBuf>) -> Self {
        let mut s = Store::open(root);
        s.shared = true;
        s
    }

    pub fn is_shared(&self) -> bool {
        self.shared
    }

    /// May this action's result be written to the action cache?
    ///
    /// Keyed off the `+unpinned` marker that [`Toolchain::tool_id`] puts in the
    /// tool string — so the decision is made from the same bytes that went into
    /// the key, not from metadata that could disagree with it.
    ///
    /// [`Toolchain::tool_id`]: super::lang::Toolchain::tool_id
    pub fn may_publish(&self, action: &Action) -> bool {
        !self.shared || !action.tool.ends_with("+unpinned")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("ribosome-cas-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn put_then_get_round_trips() {
        let root = tmp("roundtrip");
        let cas = Cas::new(&root);
        let d = cas.put(b"hello ribosome").unwrap();
        assert_eq!(cas.get(&d).unwrap(), b"hello ribosome");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn put_is_idempotent() {
        let root = tmp("idem");
        let cas = Cas::new(&root);
        let a = cas.put(b"same").unwrap();
        let b = cas.put(b"same").unwrap();
        assert_eq!(a, b, "identical bytes must dedup to one blob");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_blob_is_distinguishable_from_corrupt() {
        let root = tmp("missing");
        let cas = Cas::new(&root);
        let d = Digest::of(b"never stored");
        assert_eq!(cas.get(&d), Err(CasError::Missing(d)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn corruption_is_detected_on_read() {
        let root = tmp("corrupt");
        let cas = Cas::new(&root);
        let d = cas.put(b"trustworthy").unwrap();

        // Simulate rot: overwrite the blob in place, keeping its filename.
        let p = cas.blob_path(&d);
        std::fs::write(&p, b"tampered!!!").unwrap();

        match cas.get(&d) {
            Err(CasError::Corrupt { want, .. }) => assert_eq!(want, d),
            other => panic!("expected corruption to be caught, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn evict_removes_a_blob() {
        let root = tmp("evict");
        let cas = Cas::new(&root);
        let d = cas.put(b"temporary").unwrap();
        assert!(cas.has(&d));
        cas.evict(&d).unwrap();
        assert!(!cas.has(&d));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn action_cache_round_trips() {
        let root = tmp("actions");
        let ac = ActionCache::new(&root);
        let key = Digest::of(b"some action key");
        let result = ActionResult::ok("linux-x86_64").with_output("out.abl", Digest::of(b"abl"));
        ac.put(&key, &result).unwrap();
        assert_eq!(ac.get(&key), Some(result));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_key_is_a_miss_not_an_error() {
        let root = tmp("miss");
        let ac = ActionCache::new(&root);
        assert_eq!(ac.get(&Digest::of(b"nope")), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn malformed_entry_reads_as_a_miss() {
        let root = tmp("malformed");
        let ac = ActionCache::new(&root);
        let key = Digest::of(b"k");
        let p = ac.entry_path(&key);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"{ this is not valid json").unwrap();
        assert_eq!(ac.get(&key), None, "an unreadable cache must not fail the build");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn invalidate_removes_an_entry() {
        let root = tmp("inval");
        let ac = ActionCache::new(&root);
        let key = Digest::of(b"k2");
        ac.put(&key, &ActionResult::ok("any")).unwrap();
        ac.invalidate(&key).unwrap();
        assert_eq!(ac.get(&key), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
