//! Self-modification as a [`Workload`]: candidates are edits to a source tree.
//!
//! The tree is typically the MAGE compiler's own source. A candidate is a set of
//! files that differ from a frozen base tree (the champion's edits plus its own).
//! `materialize` copies the base tree into a scratch directory, applies the
//! change set (writes and deletions), builds it and runs its tests. `evaluate` then runs a fixed evaluator against the built
//! binary. Every change goes through the same gate as any other candidate
//! ([`crate::gate`]): this module adds no way around it. It makes the compiler
//! editable by the loop on the user's decision that RSI self-modification is
//! open-ended, and it keeps every edit measurable by something the edit cannot
//! touch.
//!
//! ## Genome encoding
//!
//! A genome is the **effective change set against the frozen base tree**: one
//! [`Locus::Gene`] per file that differs from it, with the relative path
//! (forward slashes) as `name`, `Effects::Unchecked` as `effects` (nobody has
//! read these files and declared what they do), and one of two shapes:
//!
//! - a **file gene**: the content's [`BlobStore`] hash as `sha256` and `"file"` as
//!   `signature`; the file is added or replaced;
//! - a **tombstone** ([`tombstone_for`]): `"delete"` as `signature` and
//!   [`TOMBSTONE_SHA`] as `sha256`; the file is removed, so a genome can shrink.
//!
//! An empty genome is the base tree unchanged. [`change_set`] refuses scalar
//! loci, other signatures, malformed hashes and duplicate paths ([`changed_files`]
//! additionally refuses tombstones).
//!
//! ## Champion-relative candidates
//!
//! A proposer edits the **champion's tree**, not the base. [`Workload::rebase`]
//! (called by the runner for every external candidate, before it is built or
//! journalled) overlays the candidate's change set on the champion's genome:
//!
//! 1. start from the champion's genes, keyed by path;
//! 2. a candidate file gene replaces the champion's gene for that path, or is
//!    dropped if its bytes equal the base file (a revert);
//! 3. a candidate tombstone removes the champion's gene for a file the champion
//!    added, or becomes a tombstone for a base file; it is refused for a path in
//!    neither (nothing to delete) and for a path already deleted;
//! 4. what remains, sorted by path, is the candidate's genome. It is complete
//!    (self-contained: it does not mention the champion), is what
//!    [`SelfModWorkload::materialize`] builds on a copy of the base tree, and is
//!    what the runner journals, so lineage replay and inheritance read the same
//!    full genome as before. A genome against the base is the degenerate case: the
//!    champion's genes are empty, and nothing changes.
//!
//! A candidate whose overlay equals the champion changes nothing and is refused.
//! Because the effective genome is complete, materialization still ignores the
//! champion: the artifact digest depends only on the effective change set and the
//! base identity. After a demotion the champion is the fallback, so the next
//! candidate is overlaid on the fallback's genome, not the demoted one's.
//!
//! **Stale candidates are refused, not merged.** A candidate may state the
//! identity ([`genome_id`]) of the champion genome it was written against
//! ([`CandidateSpec::based_on`]; [`DirProposer`] reads it from a `based_on` file
//! or stamps the champion it diffed against). If the champion has moved since,
//! the candidate is refused as stale: the change set might silently undo, or
//! depend on, an edit it never saw. No attempt is made to rebase onto disjoint
//! files. A proposer re-reads the new champion tree and resubmits. A candidate
//! with no stated identity is taken to be against the champion at the moment
//! the runner reads it.
//!
//! ### Deleting
//!
//! Tombstones go through exactly the checks file genes do: [`check_write_set`]
//! (writable prefix, protected prefixes, path syntax) runs over deleted paths too,
//! so a protected path is as undeletable as it is unwritable, and
//! [`SelfModConfig::protect_tests`] refuses deleting a `.rs` file that holds tests.
//!
//! ### Format and compatibility
//!
//! The artifact digest of a genome with no tombstone is byte-for-byte what it was
//! (`germline-selfmod-v1`), so existing journals and lineages replay and rebuild
//! to the digests they recorded. A genome with a tombstone digests under
//! `germline-selfmod-v2`, which an older build cannot have produced, and an older
//! build refuses such a genome (unknown signature) rather than ignoring the
//! deletion.
//!
//! Limits that remain:
//!
//! - Genomes carry no scalars, so the built-in undirected search (mutation and
//!   crossover over `Param` loci) has nothing to vary. Candidates come from a
//!   [`Proposer`].
//! - A stale candidate is refused whole. Several candidates in one invocation
//!   after a promotion are therefore refused; submit one per promotion (the CLI
//!   reports the new champion's tree).
//! - Empty directories left by a deletion are not removed from the tree.
//!
//! ## Trust boundary
//!
//! Three things are outside what a candidate can write, and the safety of the
//! scheme rests on them:
//!
//! 1. **The evaluator.** Its program and script files must lie outside the
//!    scratch root and outside every writable prefix of the base tree. This is
//!    checked at construction ([`SelfModWorkload::new`]). It runs in a fresh empty
//!    working directory, never in a candidate tree, and receives only the
//!    candidate's built binary (a private copy) as its last argument.
//! 2. **The base tree.** A read-only snapshot. Candidates are built in a copy.
//! 3. **Protected paths.** [`SelfModConfig::protected`] prefixes are refused even
//!    inside a writable prefix. Put the tests and any corpus there. Otherwise a
//!    candidate can pass its tests by editing them.
//!
//! 4. **Inline tests** (opt-in, [`SelfModConfig::protect_tests`]). Unit tests that
//!    live inside writable `.rs` files cannot be put under a protected prefix. With
//!    the flag on, a candidate is refused if any test function of the base version
//!    of a file it changes is missing or textually different in the candidate
//!    ([`check_tests_kept`]). Tests the candidate adds are fine. This is a textual
//!    guard, not a semantic one: see [`test_fns`] for what it can be fooled by.
//!
//! The write set is enforced at exactly one place, [`check_write_set`], called by
//! [`SelfModWorkload::materialize`] before anything is copied. [`DirProposer`]
//! deliberately does not repeat it.
//!
//! ### What a refusal does to a run
//!
//! A write-set violation or a malformed genome is the candidate's doing, so it
//! is refused like a failed build: `materialize` returns `Ok` without copying or
//! running anything, the report is marked [`ArtifactReport::refused`], the
//! fitness is zero and [`SelfModWorkload::materialized`] is false, so the gate
//! refuses the candidate, the refusal is journalled, the proposer's calibration
//! sees it, and the search goes on. (In [`crate::runner::Runner`] an `Err` from
//! `materialize` halts the run with [`Halt::WorkloadFailed`](crate::runner::Halt),
//! so a proposer that could cause one could stop the run at will.) A build
//! failure, a test failure and a bad evaluator output are recorded the same way.
//! `Err` is kept for failures of this side: a gene missing from the blob store,
//! an unreadable base tree, a command that cannot be started.
//!
//! ## What is NOT isolated
//!
//! There is no OS sandbox. The candidate's build scripts (`build.rs`, proc
//! macros, anything the build command runs), its tests and the binary the
//! evaluator and shadow command execute all run **as the user who runs this
//! process**, with only the hygiene below. They can read and write any file that
//! user can, open network connections, and spawn processes. The write-set check
//! constrains which *source files* a candidate may change, not what the code it
//! compiles may do when it runs. Hygiene is:
//!
//! - the environment is cut down to `PATH` (plus `SYSTEMROOT`, `TEMP`, `TMP`,
//!   `PATHEXT` on Windows) and the pairs in [`SelfModConfig::env`] (build and
//!   test only), so an API key in the parent environment is not inherited;
//! - a timeout per command, after which the child is killed (on Windows only the
//!   child, not necessarily its descendants);
//! - the evaluator and shadow command start in a fresh empty directory;
//! - symlinks in the base tree are not copied.
//!
//! Run this only in a disposable account, container or VM if the proposer is not
//! fully trusted. A candidate that tampers with files outside the tree is not
//! detected here.
//!
//! ## Other notes
//!
//! - The built binary is copied to a private per-artifact path right after the
//!   build, so a `CARGO_TARGET_DIR` shared across candidates (for incremental
//!   builds) cannot make one artifact's binary change under another.
//! - Fitness axes come from the evaluator's JSON and are clamped to `[0,1]` by
//!   [`FitnessVector`]. A failed candidate gets zero on `capability`,
//!   `correctness`, and every axis any earlier evaluation reported, so it stays
//!   comparable with the champion and the gate refuses it on the merits.
//! - Scratch trees are left on disk (for inspection). Remove `scratch_root`
//!   when done.

pub mod cli;

use super::directed::CandidateSpec;
use super::manifest::PredictionManifest;
use super::runner::{Proposer, Workload};
use super::supervisor::HealthSample;
use super::variation::{Effects, Locus};
use super::workload::GeneResolver;
use super::{EvalSuite, FitnessVector};
use ribosome::mac::absorb;
use ribosome::Digest;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The signature every file gene carries.
pub const FILE_SIGNATURE: &str = "file";

/// The signature of a tombstone: a gene that deletes the file it names.
pub const TOMBSTONE_SIGNATURE: &str = "delete";

/// The hash a tombstone carries. There is no content to hash; 64 zeros cannot be
/// the SHA-256 of anything stored, and a file gene bearing it is refused.
pub const TOMBSTONE_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// How much of a child's stdout/stderr is kept (the tail).
const OUTPUT_KEEP: usize = 64 * 1024;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique() -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    format!("{}-{n}-{nanos}", std::process::id())
}

fn sha_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Text as compared for path prefixes: lowercased on Windows, where the file
/// system does not tell `Src/` from `src/`.
fn fold(s: &str) -> String {
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// BlobStore
// ---------------------------------------------------------------------------

/// A content-addressed directory: `root/<sha256 hex>` holds the bytes whose
/// SHA-256 is that name.
#[derive(Debug, Clone)]
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    /// Open (creating if needed) a store at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root.into();
        std::fs::create_dir_all(&root)
            .map_err(|e| format!("cannot create blob store {}: {e}", root.display()))?;
        Ok(BlobStore { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Store `bytes`; returns their SHA-256 (lowercase hex). Written to a
    /// temporary file and renamed into place, so a reader never sees a partial
    /// blob. Idempotent: storing the same bytes again changes nothing.
    pub fn put(&self, bytes: &[u8]) -> Result<String, String> {
        let sha = sha_hex(bytes);
        let dest = self.root.join(&sha);
        if dest.is_file() {
            return Ok(sha);
        }
        let tmp = self.root.join(format!(".tmp-{}", unique()));
        std::fs::write(&tmp, bytes).map_err(|e| format!("cannot write blob: {e}"))?;
        if let Err(e) = std::fs::rename(&tmp, &dest) {
            let _ = std::fs::remove_file(&tmp);
            // Another writer may have won the race, with the same bytes.
            if !dest.is_file() {
                return Err(format!("cannot place blob {sha}: {e}"));
            }
        }
        Ok(sha)
    }

    /// The bytes for `sha`, or `None` if absent, if `sha` is not a 64-digit
    /// lowercase hex name, or if the content does not hash to `sha`.
    pub fn get(&self, sha: &str) -> Option<Vec<u8>> {
        if !is_sha_hex(sha) {
            return None;
        }
        let bytes = std::fs::read(self.root.join(sha)).ok()?;
        (sha_hex(&bytes) == sha).then_some(bytes)
    }
}

impl GeneResolver for BlobStore {
    fn resolve(&self, sha256: &str) -> Option<String> {
        String::from_utf8(self.get(sha256)?).ok()
    }
}

// ---------------------------------------------------------------------------
// Genome encoding
// ---------------------------------------------------------------------------

/// The gene for one changed file.
pub fn gene_for(path: &str, sha256: &str) -> Locus {
    Locus::gene(path, sha256, FILE_SIGNATURE, Effects::default())
}

/// The gene that deletes the file at `path`.
pub fn tombstone_for(path: &str) -> Locus {
    Locus::gene(path, TOMBSTONE_SHA, TOMBSTONE_SIGNATURE, Effects::default())
}

/// Whether a `sha256` as returned by [`change_set`] marks a deletion.
pub fn is_tombstone(sha256: &str) -> bool {
    sha256 == TOMBSTONE_SHA
}

/// The `(path, sha256)` pairs a genome names, in genome order; a deletion has
/// [`TOMBSTONE_SHA`] as its hash (see [`is_tombstone`]).
///
/// Refused: any locus that is not a file gene or a tombstone, a malformed hash,
/// a file gene bearing the tombstone hash, and two genes for the same path
/// (compared without regard to case on Windows).
pub fn change_set(genome: &[Locus]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::with_capacity(genome.len());
    let mut seen = BTreeSet::new();
    for locus in genome {
        match locus {
            Locus::Gene { name, sha256, signature, .. } => {
                if signature == TOMBSTONE_SIGNATURE {
                    if !is_tombstone(sha256) {
                        return Err(format!("tombstone `{name}` carries a hash `{sha256}`, not the tombstone hash"));
                    }
                } else if signature != FILE_SIGNATURE {
                    return Err(format!("gene `{name}` has signature `{signature}`, not `{FILE_SIGNATURE}` or `{TOMBSTONE_SIGNATURE}`"));
                } else if !is_sha_hex(sha256) {
                    return Err(format!("gene `{name}` has a malformed hash `{sha256}`"));
                } else if is_tombstone(sha256) {
                    return Err(format!("file gene `{name}` carries the tombstone hash"));
                }
                if !seen.insert(fold(name)) {
                    return Err(format!("the genome changes `{name}` twice"));
                }
                out.push((name.clone(), sha256.clone()));
            }
            Locus::Param { .. } => {
                return Err("a self-modification genome carries file genes only, found a scalar locus".into())
            }
        }
    }
    Ok(out)
}

/// The `(path, sha256)` pairs of a genome that only adds or replaces files:
/// [`change_set`], but a tombstone is refused.
pub fn changed_files(genome: &[Locus]) -> Result<Vec<(String, String)>, String> {
    let set = change_set(genome)?;
    if let Some((p, _)) = set.iter().find(|(_, s)| is_tombstone(s)) {
        return Err(format!("the genome deletes `{p}`; use `change_set` for a genome that can delete"));
    }
    Ok(set)
}

/// A stable identity for a genome: independent of locus order, covering every
/// field of every locus. Two champions with the same effective change set have
/// the same id; this is what a candidate's `based_on` names.
pub fn genome_id(genome: &[Locus]) -> String {
    let mut parts: Vec<String> = genome.iter().map(|l| serde_json::to_string(l).unwrap_or_default()).collect();
    parts.sort();
    let mut h = Sha256::new();
    h.update(b"germline-genome-v1");
    for p in &parts {
        absorb(&mut h, p.as_bytes());
    }
    format!("{:x}", h.finalize())
}

fn artifact_digest(base_identity: &str, files: &[(String, String)]) -> Digest {
    let mut sorted: Vec<&(String, String)> = files.iter().collect();
    sorted.sort();
    let mut h = Sha256::new();
    // v1 for a genome with no deletion, exactly as before tombstones existed, so
    // every digest in an existing journal still reproduces.
    h.update(if sorted.iter().any(|(_, s)| is_tombstone(s)) { &b"germline-selfmod-v2"[..] } else { &b"germline-selfmod-v1"[..] });
    absorb(&mut h, base_identity.as_bytes());
    for (p, sha) in sorted {
        absorb(&mut h, p.as_bytes());
        absorb(&mut h, sha.as_bytes());
    }
    Digest(format!("{:x}", h.finalize()))
}

// ---------------------------------------------------------------------------
// Write set
// ---------------------------------------------------------------------------

/// Characters no path component may contain, on any platform: they are either
/// separators or reserved by Windows, and a genome written on one platform must
/// mean the same on another.
const BAD_CHARS: &[char] = &['\\', ':', '<', '>', '"', '|', '?', '*'];

const WINDOWS_DEVICES: &[&str] =
    &["con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9"];

/// Whether `path` is a canonical relative path with forward slashes.
///
/// **`.` components are refused, not normalised.** A genome should have one
/// spelling per file: if `prototype/src/./x` were quietly read as
/// `prototype/src/x`, the duplicate check and the prefix comparison would both
/// have to agree on that reading, and any disagreement is a bypass. Refusing
/// costs a proposer nothing. For the same reason `..`, empty components (`a//b`),
/// and components ending in `.` or a space (which Windows silently strips, so
/// `tests./x` would reach `tests/x`) are refused. On Windows also `~` in a
/// component (8.3 short names such as `PROTEC~1`) and device names (`nul`,
/// `con`, ...).
fn check_path_syntax(path: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("path `{path}` refused: {why}"));
    if path.is_empty() {
        return bad("empty");
    }
    if path.starts_with('/') {
        return bad("absolute");
    }
    if path.ends_with('/') {
        return bad("names a directory");
    }
    if let Some(c) = path.chars().find(|c| c.is_control() || BAD_CHARS.contains(c)) {
        return bad(&format!("contains `{}` (backslash, drive letter or reserved character)", c.escape_default()));
    }
    for comp in path.split('/') {
        match comp {
            "" => return bad("empty component"),
            "." => return bad("`.` component"),
            ".." => return bad("`..` component"),
            _ => {}
        }
        if comp.ends_with('.') || comp.ends_with(' ') {
            return bad("a component ends in `.` or a space");
        }
        if cfg!(windows) {
            if comp.contains('~') {
                return bad("`~` in a component (short-name alias)");
            }
            let stem = comp.split('.').next().unwrap_or("").to_lowercase();
            if WINDOWS_DEVICES.contains(&stem.as_str()) {
                return bad("a reserved device name");
            }
        }
    }
    Ok(())
}

/// The prefix as compared: folded, no trailing slash. `None` for a prefix that
/// would match nothing sensibly (empty).
fn norm_prefix(prefix: &str) -> Option<String> {
    let p = fold(prefix.trim_end_matches('/'));
    (!p.is_empty()).then_some(p)
}

/// Whether the (folded) `path` is the prefix itself or lies under it, on a
/// component boundary.
fn covers(path: &str, prefix: &str) -> bool {
    match norm_prefix(prefix) {
        None => false,
        Some(p) => path == p || path.strip_prefix(p.as_str()).is_some_and(|rest| rest.starts_with('/')),
    }
}

/// The safety core: may these files be changed?
///
/// Every path must be syntactically safe (see [`check_path_syntax`]: relative,
/// no `.`/`..`, no backslash or drive letter, non-empty), lie under at least one
/// [`SelfModConfig::writable`] prefix, and lie under no
/// [`SelfModConfig::protected`] prefix. Prefixes match on whole components
/// (`prototype/src` does not cover `prototype/src2/x`), and without regard to
/// case on Windows. The first offending path is named in the error.
pub fn check_write_set(cfg: &SelfModConfig, files: &[(String, String)]) -> Result<(), String> {
    for (path, _) in files {
        check_path_syntax(path)?;
        let f = fold(path);
        if !cfg.writable.iter().any(|w| covers(&f, w)) {
            return Err(format!("path `{path}` is outside the write set (not under any writable prefix)"));
        }
        if let Some(p) = cfg.protected.iter().find(|p| covers(&f, p)) {
            return Err(format!("path `{path}` is under the protected prefix `{p}`"));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Test integrity
// ---------------------------------------------------------------------------

/// Copy of `src` with comments and the insides of string and char literals
/// replaced by spaces (newlines kept), byte for byte, so offsets in the result
/// are offsets in `src`. Lifetimes (`'a`) are not char literals and are kept.
fn mask_rust(src: &str) -> Vec<u8> {
    let b = src.as_bytes();
    let mut m = b.to_vec();
    let blank = |m: &mut Vec<u8>, from: usize, to: usize| {
        for c in &mut m[from..to.min(b.len())] {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    let is_ident = |c: u8| c == b'_' || c.is_ascii_alphanumeric() || c >= 0x80;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                let end = b[i..].iter().position(|&c| c == b'\n').map_or(b.len(), |p| i + p);
                blank(&mut m, i, end);
                i = end;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                let (mut depth, mut j) = (1, i + 2);
                while j < b.len() && depth > 0 {
                    if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                        depth += 1;
                        j += 2;
                    } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                        depth -= 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                blank(&mut m, i, j);
                i = j;
            }
            // `r"..."`, `r#"..."#`, `br#"..."#`, `cr"..."`: no escapes, ends at a
            // quote followed by as many `#` as opened it.
            b'r' if (i == 0 || !is_ident(b[i - 1]) || matches!(b[i - 1], b'b' | b'c') && (i < 2 || !is_ident(b[i - 2]))) => {
                let mut j = i + 1;
                while b.get(j) == Some(&b'#') {
                    j += 1;
                }
                if b.get(j) == Some(&b'"') {
                    let hashes = j - i - 1;
                    let mut k = j + 1;
                    let end = loop {
                        match b[k..].iter().position(|&c| c == b'"') {
                            None => break b.len(),
                            Some(p) => {
                                k += p + 1;
                                if b.len() >= k + hashes && b[k..k + hashes].iter().all(|&c| c == b'#') {
                                    break k + hashes;
                                }
                            }
                        }
                    };
                    blank(&mut m, j, end);
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'"' => {
                let mut j = i + 1;
                while j < b.len() && b[j] != b'"' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let end = (j + 1).min(b.len());
                blank(&mut m, i + 1, end);
                i = end;
            }
            b'\'' => {
                // A char literal is `'\...'` or one char then `'`; anything else
                // (`'a`, `'static`, `'_`) is a lifetime or label.
                let end = if b.get(i + 1) == Some(&b'\\') {
                    let mut j = i + 2;
                    while j < b.len() && b[j] != b'\'' && b[j] != b'\n' {
                        j += if b[j] == b'\\' { 2 } else { 1 };
                    }
                    Some((j + 1).min(b.len()))
                } else {
                    src[i + 1..].chars().next().and_then(|c| {
                        let after = i + 1 + c.len_utf8();
                        (c != '\'' && b.get(after) == Some(&b'\'')).then_some(after + 1)
                    })
                };
                match end {
                    Some(end) => {
                        blank(&mut m, i + 1, end);
                        i = end;
                    }
                    None => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    m
}

/// Index just past the `close` that matches the `open` at `at`, in masked text.
fn matching(m: &[u8], at: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    for (j, &c) in m.iter().enumerate().skip(at) {
        if c == open {
            depth += 1;
        } else if c == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(j + 1);
            }
        }
    }
    None
}

fn skip_ws(m: &[u8], mut i: usize) -> usize {
    while i < m.len() && m[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// The identifier starting at `i` in masked text, and the index after it.
fn ident_at(m: &[u8], i: usize) -> (&str, usize) {
    let mut j = i;
    while j < m.len() && (m[j] == b'_' || m[j].is_ascii_alphanumeric()) {
        j += 1;
    }
    (std::str::from_utf8(&m[i..j]).unwrap_or(""), j)
}

/// Whether the attribute body (text between `#[` and `]`, masked) names a test
/// attribute: `test`, `tokio::test`, `tokio::test(flavor = ...)`, any path whose
/// last segment is `test`.
fn is_test_attr(body: &str) -> bool {
    let path: String = body.chars().take_while(|&c| c != '(' && c != '=').filter(|c| !c.is_whitespace()).collect();
    path.rsplit("::").next() == Some("test")
}

/// Every test function in a Rust source file as `(path::name, normalised text)`.
///
/// A test function is an `fn` item (after optional `pub`, `async`, `const`,
/// `unsafe`, `extern`) directly preceded by an attribute group that holds a
/// `#[test]` or `#[...::test]`. Its identity is its name under the enclosing
/// inline `mod` names (`tests::foo`). Its text runs from the first attribute of
/// the group to the closing brace of the body, with leading and trailing
/// whitespace of each line trimmed and blank lines dropped, so re-indenting is
/// not a change. Comments and strings are masked while finding structure, so a
/// `#[test]` or a brace inside either is neither a test nor a nesting level;
/// raw strings, char literals and lifetimes are handled.
///
/// What it can be fooled by (this is a textual guard, not a semantic one):
/// - Any edit to the text counts, including a comment inside a test. That is a
///   false refusal, the safe direction.
/// - Whitespace inside a multi-line string literal is also normalised away, so
///   changing only that is not seen.
/// - Tests defined by a macro, or marked by an attribute that is not named
///   `test` (`#[rstest]`, `#[proptest]`), are not extracted, so are not guarded.
/// - Only the test's own text is compared. A candidate can still weaken a test
///   by changing what it calls (a helper outside the test, the code under test),
///   or by `#[cfg(...)]` on the enclosing module, or by moving the test out of
///   its module, which changes its identity and is refused.
/// - A test inside a test function is not extracted separately.
pub fn test_fns(src: &str) -> Vec<(String, String)> {
    let m = mask_rust(src);
    let mut out = Vec::new();
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut pending_mod: Option<String> = None;
    let mut i = 0;
    while i < m.len() {
        let c = m[i];
        if c == b'{' {
            stack.push(pending_mod.take());
        } else if c == b'}' {
            stack.pop();
        } else if c == b'#' {
            // An attribute group: `#[..]` and `#![..]`, possibly several.
            let start = i;
            let mut j = i;
            let mut test = false;
            loop {
                let mut k = skip_ws(&m, j);
                if m.get(k) != Some(&b'#') {
                    break;
                }
                k = skip_ws(&m, k + 1);
                if m.get(k) == Some(&b'!') {
                    k = skip_ws(&m, k + 1);
                }
                if m.get(k) != Some(&b'[') {
                    break;
                }
                let Some(end) = matching(&m, k, b'[', b']') else { break };
                test |= is_test_attr(std::str::from_utf8(&m[k + 1..end - 1]).unwrap_or(""));
                j = end;
            }
            if j == i {
                i += 1;
                continue;
            }
            i = j;
            if !test {
                continue;
            }
            // Qualifiers, then `fn name`.
            let mut k = skip_ws(&m, i);
            loop {
                let (word, after) = ident_at(&m, k);
                match word {
                    "pub" => {
                        k = skip_ws(&m, after);
                        if m.get(k) == Some(&b'(') {
                            k = skip_ws(&m, matching(&m, k, b'(', b')').unwrap_or(k + 1));
                        }
                    }
                    "async" | "const" | "unsafe" | "extern" => {
                        k = skip_ws(&m, after);
                        // `extern "C"`: the ABI string is masked to a quote pair.
                        if m.get(k) == Some(&b'"') {
                            k = skip_ws(&m, k + 1);
                            while m.get(k) == Some(&b' ') || m.get(k) == Some(&b'"') {
                                k += 1;
                            }
                        }
                    }
                    _ => break,
                }
            }
            let (word, after) = ident_at(&m, k);
            if word != "fn" {
                continue;
            }
            let (name, after) = ident_at(&m, skip_ws(&m, after));
            if name.is_empty() {
                continue;
            }
            // The body: the first `{` outside parentheses and brackets of the
            // signature; a `;` first means no body.
            let (mut paren, mut open) = (0i32, None);
            for (q, &ch) in m.iter().enumerate().skip(after) {
                match ch {
                    b'(' | b'[' => paren += 1,
                    b')' | b']' => paren -= 1,
                    b'{' if paren <= 0 => {
                        open = Some(q);
                        break;
                    }
                    b';' if paren <= 0 => break,
                    _ => {}
                }
            }
            let Some(open) = open else { continue };
            let Some(end) = matching(&m, open, b'{', b'}') else { continue };
            let mut path: Vec<&str> = stack.iter().flatten().map(String::as_str).collect();
            path.push(name);
            out.push((path.join("::"), normalise_lines(&src[start..end])));
            i = end;
            continue;
        } else if c.is_ascii_alphabetic() || c == b'_' {
            // Whole identifiers only, so `submod` is not `mod`.
            let (word, after) = ident_at(&m, i);
            if word == "mod" {
                let (name, after_name) = ident_at(&m, skip_ws(&m, after));
                if !name.is_empty() && m.get(skip_ws(&m, after_name)) == Some(&b'{') {
                    pending_mod = Some(name.to_string());
                }
            }
            i = after.max(i + 1);
            continue;
        }
        i += 1;
    }
    out
}

/// Trim each line, drop blank ones, join with `\n`.
fn normalise_lines(text: &str) -> String {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Compare the test functions of the `base` and `candidate` versions of `path`.
/// Every base test must be in the candidate with the same normalised text;
/// tests the candidate adds are allowed. The error names the first test that
/// was removed or changed. A candidate that is not valid UTF-8 has no tests.
pub fn check_tests_kept(path: &str, base: &str, candidate: &[u8]) -> Result<(), String> {
    let base_tests = test_fns(base);
    if base_tests.is_empty() {
        return Ok(());
    }
    let mut cand = std::str::from_utf8(candidate).map(test_fns).unwrap_or_default();
    for (name, text) in base_tests {
        match cand.iter().position(|(n, t)| *n == name && *t == text) {
            // Used up, so two base tests of one name need two in the candidate.
            Some(at) => {
                cand.swap_remove(at);
            }
            None => return Err(format!("test `{name}` in {path} was removed or changed (protect_tests)")),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// A command to run: argv and a timeout.
#[derive(Debug, Clone)]
pub struct Cmd {
    pub argv: Vec<String>,
    pub timeout: Duration,
}

impl Cmd {
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(argv: I, timeout: Duration) -> Self {
        Cmd { argv: argv.into_iter().map(Into::into).collect(), timeout }
    }
}

#[derive(Debug, Clone)]
pub struct SelfModConfig {
    /// Read-only snapshot of the source tree.
    pub base_tree: PathBuf,
    /// What the snapshot is, folded into every artifact digest: the frozen
    /// snapshot's hash, say. Two different bases must not share digests.
    pub base_identity: String,
    /// Path prefixes (relative to the tree, forward slashes) candidates may change.
    pub writable: Vec<String>,
    /// Prefixes never writable, even inside a writable one.
    pub protected: Vec<String>,
    /// Where candidate trees are built. Must be outside `base_tree`.
    pub scratch_root: PathBuf,
    /// Run with the candidate tree as cwd.
    pub build: Cmd,
    /// Run after a successful build, same cwd.
    pub test: Cmd,
    /// The built binary, relative to `target_dir` if set, else to the tree.
    pub binary: String,
    pub target_dir: Option<PathBuf>,
    /// Extra environment for build and test (for example `CARGO_TARGET_DIR`).
    pub env: Vec<(String, String)>,
    /// The evaluator. Receives the built binary's absolute path as its last
    /// argument and prints a JSON object of axis to number on its last stdout line.
    pub evaluator: Cmd,
    /// Optional shadow/champion probe; receives the binary path as last argument.
    pub shadow: Option<Cmd>,
    /// If set, `evaluate` refuses a suite with a different digest.
    pub expected_suite: Option<Digest>,
    /// Refuse a candidate that removes or changes a test function that the base
    /// version of a changed `.rs` file has (see [`check_tests_kept`]). Rust unit
    /// tests live inline in the files a candidate may rewrite, so without this a
    /// candidate can pass its tests by deleting, `#[ignore]`-ing or weakening them.
    /// Off by default.
    pub protect_tests: bool,
}

impl SelfModConfig {
    /// A config with the given essentials. `writable` and `protected` start
    /// empty (nothing may change) and are set by the caller.
    pub fn new(
        base_tree: impl Into<PathBuf>,
        base_identity: impl Into<String>,
        scratch_root: impl Into<PathBuf>,
        build: Cmd,
        test: Cmd,
        binary: impl Into<String>,
        evaluator: Cmd,
    ) -> Self {
        SelfModConfig {
            base_tree: base_tree.into(),
            base_identity: base_identity.into(),
            writable: Vec::new(),
            protected: Vec::new(),
            scratch_root: scratch_root.into(),
            build,
            test,
            binary: binary.into(),
            target_dir: None,
            env: Vec::new(),
            evaluator,
            shadow: None,
            expected_suite: None,
            protect_tests: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Child processes
// ---------------------------------------------------------------------------

#[cfg(windows)]
const KEPT_ENV: &[&str] = &["PATH", "SYSTEMROOT", "TEMP", "TMP", "PATHEXT"];
#[cfg(not(windows))]
const KEPT_ENV: &[&str] = &["PATH"];

fn inherited_env() -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
    std::env::vars_os()
        .filter(|(k, _)| {
            let k = k.to_string_lossy();
            KEPT_ENV.iter().any(|kept| k.eq_ignore_ascii_case(kept))
        })
        .collect()
}

/// A relative program with a directory part would be resolved against the
/// child's new cwd on some platforms and ours on others; make it absolute.
fn child_program(program: &str) -> Result<PathBuf, String> {
    let p = Path::new(program);
    if p.components().count() > 1 && p.is_relative() {
        let cwd = std::env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
        Ok(cwd.join(p))
    } else {
        Ok(p.to_path_buf())
    }
}

struct Ran {
    success: bool,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

impl Ran {
    fn describe(&self, what: &str) -> String {
        if self.timed_out {
            return format!("{what} timed out");
        }
        let tail: String = {
            let s = self.stderr.trim();
            let n = s.chars().count();
            s.chars().skip(n.saturating_sub(300)).collect()
        };
        format!("{what} failed: {tail}")
    }
}

/// Keeps the tail of a pipe, discarding the rest, and reports it at EOF.
fn drain<R: Read + Send + 'static>(mut pipe: R) -> std::sync::mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut kept: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    kept.extend_from_slice(&chunk[..n]);
                    if kept.len() > OUTPUT_KEEP * 2 {
                        kept.drain(..kept.len() - OUTPUT_KEEP);
                    }
                }
            }
        }
        if kept.len() > OUTPUT_KEEP {
            kept.drain(..kept.len() - OUTPUT_KEEP);
        }
        let _ = tx.send(kept);
    });
    rx
}

/// Kill `child` and everything it started. A build is a tree (cargo, rustc,
/// build scripts, test binaries), and killing only the top of it on a timeout
/// leaves the rest running. On Windows `taskkill /T` walks the process tree; on
/// Unix the child leads its own process group (see [`run_cmd`]) and the group is
/// killed. Best effort: the direct kill always follows. **Gap:** on Windows a
/// descendant whose parent link is broken (its parent exited, or MSYS `sh`
/// emulated a fork or exec) is not found by `taskkill /T`; a Job Object would
/// catch it but needs `unsafe` FFI, which this crate has none of. A process
/// being created at the moment of the kill can also be missed, since
/// `taskkill` works from a snapshot of the tree, and under load it was seen to
/// miss a live descendant (one test run in three). So on Windows this is best
/// effort. On Unix a descendant that starts
/// its own process group or session escapes likewise.
#[cfg(windows)]
const KILL_TREE_BOUND: Duration = Duration::from_secs(5);

fn kill_tree(child: &mut std::process::Child) {
    let pid = child.id().to_string();
    #[cfg(windows)]
    {
        let taskkill = std::env::var_os("SYSTEMROOT")
            .map(|r| PathBuf::from(r).join("System32").join("taskkill.exe"))
            .unwrap_or_else(|| PathBuf::from("taskkill"));
        // Bounded: `taskkill` was seen to take over ten seconds under load, and a
        // hung one must not hold the run past its timeout. Past the bound the
        // direct child is still killed below.
        if let Ok(mut tk) = Command::new(taskkill)
            .args(["/T", "/F", "/PID", &pid])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            let began = Instant::now();
            while matches!(tk.try_wait(), Ok(None)) {
                if began.elapsed() >= KILL_TREE_BOUND {
                    let _ = tk.kill();
                    let _ = tk.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

/// Run `argv` in `cwd` with the hygienic environment plus `extra`. `Err` only
/// when the process cannot be started; a non-zero exit or a timeout is `Ok`.
/// On a timeout the whole process tree is killed ([`kill_tree`]).
fn run_cmd(argv: &[String], cwd: &Path, extra: &[(String, String)], timeout: Duration) -> Result<Ran, String> {
    let (program, args) = argv.split_first().ok_or("the command is empty")?;
    let mut cmd = Command::new(child_program(program)?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(inherited_env())
        .envs(extra.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start `{program}`: {e}"))?;
    let out = child.stdout.take().map(drain);
    let err = child.stderr.take().map(drain);
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if started.elapsed() >= timeout => {
                kill_tree(&mut child);
                let _ = child.wait();
                timed_out = true;
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                kill_tree(&mut child);
                let _ = child.wait();
                return Err(format!("waiting for `{program}`: {e}"));
            }
        }
    };
    // A descendant that kept a pipe open must not hold us: wait briefly only.
    let grace = Duration::from_secs(if timed_out { 0 } else { 2 });
    let grab = |rx: Option<std::sync::mpsc::Receiver<Vec<u8>>>| {
        rx.and_then(|r| r.recv_timeout(grace).ok()).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
    };
    Ok(Ran {
        success: status.is_some_and(|s| s.success()),
        timed_out,
        stdout: grab(out),
        stderr: grab(err),
    })
}

// ---------------------------------------------------------------------------
// Workload
// ---------------------------------------------------------------------------

/// What `materialize` and `evaluate` recorded about one artifact.
#[derive(Debug, Clone)]
pub struct ArtifactReport {
    /// The candidate's tree.
    pub tree: PathBuf,
    /// The `path:sha` entries of its genome, sorted.
    pub files: Vec<String>,
    pub build_ok: bool,
    pub tests_ok: bool,
    /// Wall-clock seconds of build plus tests.
    pub seconds: f64,
    /// The private copy of the built binary, if the build produced one.
    pub binary: Option<PathBuf>,
    /// Why it failed, when it did (build, tests, missing binary, or the evaluator).
    pub failure: Option<String>,
    /// Set when the evaluator's output was refused (and capability recorded as 0).
    pub eval_failed: bool,
    /// Set when the candidate was refused before anything was copied: a
    /// write-set violation or a malformed genome. `failure` says why.
    pub refused: bool,
}

/// A [`Workload`] whose candidates are edits to a source tree. See the module docs.
pub struct SelfModWorkload {
    cfg: SelfModConfig,
    store: BlobStore,
    /// Canonical base tree and scratch root.
    base: PathBuf,
    scratch: PathBuf,
    artifacts: HashMap<String, ArtifactReport>,
    /// Axes any evaluation has reported, for zero-filling a failure.
    known_axes: BTreeSet<String>,
}

/// `fs::canonicalize`, without the `\\?\` verbatim prefix Windows adds: a
/// child such as an MSYS `sh` cannot open a path in that form, and the binary
/// path is handed to one.
fn canon(p: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    let real = std::fs::canonicalize(p)?;
    let text = real.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let b = rest.as_bytes();
        if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
            return Ok(PathBuf::from(rest));
        }
    }
    Ok(real)
}

fn is_inside(p: &Path, root: &Path) -> bool {
    let (a, b) = (fold(&p.to_string_lossy()), fold(&root.to_string_lossy()));
    Path::new(&a).starts_with(Path::new(&b))
}

impl SelfModWorkload {
    /// Validate the config and open the workload.
    ///
    /// Refused: an unreadable base tree; empty, absolute, `..`-bearing or
    /// otherwise unsafe writable/protected prefixes, or no writable prefix at
    /// all; empty commands; an unsafe `binary`; a scratch root inside the base
    /// tree; and an evaluator (its program, or any argument that names an
    /// existing path) that lies inside the scratch root or inside a writable
    /// prefix of the base tree, which would let a candidate rewrite its own judge.
    pub fn new(cfg: SelfModConfig, store: BlobStore) -> Result<Self, String> {
        let base = canon(&cfg.base_tree)
            .map_err(|e| format!("unreadable base tree {}: {e}", cfg.base_tree.display()))?;
        if !base.is_dir() {
            return Err(format!("base tree {} is not a directory", base.display()));
        }
        if cfg.writable.is_empty() {
            return Err("no writable prefix: a candidate could change nothing".into());
        }
        for (kind, prefixes) in [("writable", &cfg.writable), ("protected", &cfg.protected)] {
            for p in prefixes {
                let trimmed = p.trim_end_matches('/');
                check_path_syntax(trimmed).map_err(|e| format!("{kind} prefix `{p}`: {e}"))?;
            }
        }
        for (what, cmd) in [("build", &cfg.build), ("test", &cfg.test), ("evaluator", &cfg.evaluator)] {
            if cmd.argv.is_empty() {
                return Err(format!("the {what} command is empty"));
            }
        }
        check_path_syntax(&cfg.binary).map_err(|e| format!("binary: {e}"))?;

        std::fs::create_dir_all(&cfg.scratch_root)
            .map_err(|e| format!("cannot create scratch root {}: {e}", cfg.scratch_root.display()))?;
        let scratch = canon(&cfg.scratch_root)
            .map_err(|e| format!("cannot resolve scratch root: {e}"))?;
        if is_inside(&scratch, &base) {
            return Err("the scratch root is inside the base tree".into());
        }

        let writable_dirs: Vec<PathBuf> = cfg
            .writable
            .iter()
            .map(|w| {
                let joined = w.trim_end_matches('/').split('/').fold(base.clone(), |acc, c| acc.join(c));
                canon(&joined).unwrap_or(joined)
            })
            .collect();
        for arg in &cfg.evaluator.argv {
            let Ok(real) = canon(arg) else { continue };
            if is_inside(&real, &scratch) {
                return Err(format!("the evaluator `{arg}` lies inside the scratch root, where candidates write"));
            }
            if let Some(w) = writable_dirs.iter().find(|w| is_inside(&real, w)) {
                return Err(format!(
                    "the evaluator `{arg}` lies inside the writable prefix {}, so a candidate could rewrite it",
                    w.display()
                ));
            }
        }

        Ok(SelfModWorkload { cfg, store, base, scratch, artifacts: HashMap::new(), known_axes: BTreeSet::new() })
    }

    /// What was recorded about an artifact, if it was materialized.
    pub fn report(&self, artifact: &Digest) -> Option<&ArtifactReport> {
        self.artifacts.get(&artifact.0)
    }

    pub fn config(&self) -> &SelfModConfig {
        &self.cfg
    }

    /// Record axes a measurement made elsewhere reported, so that a failed
    /// candidate is zero-filled on them like on any axis this process has
    /// evaluated. A workload rebuilt in a new process (a resumed run) has
    /// evaluated nothing yet; without this a failure would omit the champion's
    /// guard axes instead of scoring zero on them.
    pub fn note_axes<I: IntoIterator<Item = String>>(&mut self, axes: I) {
        self.known_axes.extend(axes);
    }

    /// The SHA-256 of the base tree's file at `path` (already syntax-checked), or
    /// `None` if the base has no such file.
    fn base_file_hash(&self, path: &str) -> Result<Option<String>, String> {
        let p = path.split('/').fold(self.base.clone(), |acc, c| acc.join(c));
        match std::fs::symlink_metadata(&p) {
            Ok(md) if md.is_file() => {
                let bytes = std::fs::read(&p).map_err(|e| format!("cannot read base file {path}: {e}"))?;
                Ok(Some(sha_hex(&bytes)))
            }
            Ok(_) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("cannot read base file {path}: {e}")),
        }
    }

    fn fresh_dir(&self) -> Result<PathBuf, String> {
        let d = self.scratch.join(".run").join(unique());
        std::fs::create_dir_all(&d).map_err(|e| format!("cannot create a scratch directory: {e}"))?;
        Ok(d)
    }

    fn failed_vector(&self) -> FitnessVector {
        let mut v = FitnessVector::new();
        for a in &self.known_axes {
            v = v.with(a.clone(), 0.0);
        }
        v.with("capability", 0.0).with("correctness", 0.0)
    }

    /// Run a probe command with the binary appended, in a fresh empty directory.
    fn run_with_binary(&self, cmd: &Cmd, binary: &Path) -> Result<Ran, String> {
        let dir = self.fresh_dir()?;
        let mut argv = cmd.argv.clone();
        argv.push(binary.to_string_lossy().into_owned());
        let r = run_cmd(&argv, &dir, &[], cmd.timeout);
        let _ = std::fs::remove_dir_all(&dir);
        r
    }

    fn probe(&self, artifact: &Digest) -> HealthSample {
        let Some(binary) = self.report(artifact).filter(|_| self.materialized(artifact)).and_then(|r| r.binary.clone())
        else {
            return HealthSample::failed();
        };
        match &self.cfg.shadow {
            None => HealthSample::ok(),
            Some(cmd) => match self.run_with_binary(cmd, &binary) {
                Ok(r) if r.success => HealthSample::ok(),
                _ => HealthSample::failed(),
            },
        }
    }
}

/// Copy `from` into `to`, skipping `target/` and `.git/` at any depth and
/// every symlink.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("cannot create {}: {e}", to.display()))?;
    let rd = std::fs::read_dir(from).map_err(|e| format!("unreadable base tree at {}: {e}", from.display()))?;
    for entry in rd {
        let entry = entry.map_err(|e| format!("unreadable base tree at {}: {e}", from.display()))?;
        let ft = entry.file_type().map_err(|e| format!("unreadable base tree entry {}: {e}", entry.path().display()))?;
        let name = entry.file_name();
        let dest = to.join(&name);
        if ft.is_symlink() {
            continue;
        } else if ft.is_dir() {
            if name == "target" || name == ".git" {
                continue;
            }
            copy_tree(&entry.path(), &dest)?;
        } else if ft.is_file() {
            std::fs::copy(entry.path(), &dest)
                .map_err(|e| format!("unreadable base tree file {}: {e}", entry.path().display()))?;
        }
    }
    Ok(())
}

/// Write one gene's bytes over `rel` in `tree`. Parent directories are created
/// only if they stay inside `tree`. `Err` here is the candidate's doing (a path
/// that collides with a file, say), not an infrastructure failure.
fn write_gene(tree: &Path, rel: &str, bytes: &[u8]) -> Result<(), String> {
    let mut dest = tree.to_path_buf();
    for c in rel.split('/') {
        dest.push(c);
    }
    if !dest.components().skip(tree.components().count()).all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("`{rel}` escapes the tree"));
    }
    let parent = dest.parent().ok_or_else(|| format!("`{rel}` has no parent"))?;
    std::fs::create_dir_all(parent).map_err(|e| format!("cannot create the directory for `{rel}`: {e}"))?;
    let real = canon(parent).map_err(|e| format!("cannot resolve the directory for `{rel}`: {e}"))?;
    if !is_inside(&real, tree) {
        return Err(format!("`{rel}` resolves outside the tree"));
    }
    if let Ok(md) = std::fs::symlink_metadata(&dest) {
        if md.file_type().is_symlink() || md.is_dir() {
            return Err(format!("`{rel}` collides with an existing link or directory"));
        }
        // Remove first so a read-only copy of the base file does not block the write.
        std::fs::remove_file(&dest).map_err(|e| format!("cannot replace `{rel}`: {e}"))?;
    }
    std::fs::write(&dest, bytes).map_err(|e| format!("cannot write `{rel}`: {e}"))
}

/// Delete the file at `rel` in `tree`. `Err` is the candidate's doing: the file
/// is absent (so the deletion has nothing to act on), or is a directory or link.
fn delete_path(tree: &Path, rel: &str) -> Result<(), String> {
    let mut dest = tree.to_path_buf();
    for c in rel.split('/') {
        dest.push(c);
    }
    if !dest.components().skip(tree.components().count()).all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("`{rel}` escapes the tree"));
    }
    let parent = dest.parent().ok_or_else(|| format!("`{rel}` has no parent"))?;
    let real = canon(parent).map_err(|_| format!("cannot delete `{rel}`: it does not exist"))?;
    if !is_inside(&real, tree) {
        return Err(format!("`{rel}` resolves outside the tree"));
    }
    match std::fs::symlink_metadata(&dest) {
        Ok(md) if md.file_type().is_symlink() || md.is_dir() => {
            Err(format!("`{rel}` is a link or directory, not a file"))
        }
        Ok(_) => remove_file_force(&dest).map_err(|e| format!("cannot delete `{rel}`: {e}")),
        Err(_) => Err(format!("cannot delete `{rel}`: it does not exist")),
    }
}

/// Clear the read-only flag of `p`, if set (Windows refuses to remove such a file).
fn clear_read_only(p: &Path) {
    if let Ok(md) = std::fs::metadata(p) {
        let mut perm = md.permissions();
        if perm.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            perm.set_readonly(false);
            let _ = std::fs::set_permissions(p, perm);
        }
    }
}

fn remove_file_force(p: &Path) -> std::io::Result<()> {
    clear_read_only(p);
    std::fs::remove_file(p)
}

/// Remove a directory tree that may hold read-only files.
pub fn remove_tree(dir: &Path) -> std::io::Result<()> {
    fn clear(d: &Path) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                match e.file_type() {
                    Ok(t) if t.is_dir() => clear(&p),
                    Ok(t) if t.is_file() => clear_read_only(&p),
                    _ => {}
                }
            }
        }
    }
    clear(dir);
    std::fs::remove_dir_all(dir)
}

/// Mark every file under `dir` read-only (a snapshot a proposer reads, not edits).
pub fn make_read_only(dir: &Path) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => make_read_only(&p),
                Ok(t) if t.is_file() => {
                    if let Ok(md) = std::fs::metadata(&p) {
                        let mut perm = md.permissions();
                        perm.set_readonly(true);
                        let _ = std::fs::set_permissions(&p, perm);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Write `base` plus the genome's change set to `dir`, which is created and must
/// not exist: the tree a genome denotes, without building it. Returns the
/// `path:sha` (or `path:deleted`) entries applied.
pub fn export_genome_tree(base: &Path, store: &BlobStore, genome: &[Locus], dir: &Path) -> Result<Vec<String>, String> {
    if dir.exists() {
        return Err(format!("{} already exists", dir.display()));
    }
    let files = change_set(genome)?;
    copy_tree(base, dir)?;
    let tree = canon(dir).map_err(|e| format!("cannot resolve {}: {e}", dir.display()))?;
    for (path, sha) in &files {
        if is_tombstone(sha) {
            delete_path(&tree, path)?;
        } else {
            let bytes = store.get(sha).ok_or_else(|| format!("gene `{path}` is not in the blob store"))?;
            write_gene(&tree, path, &bytes)?;
        }
    }
    Ok(files.iter().map(|(p, s)| if is_tombstone(s) { format!("{p}:deleted") } else { format!("{p}:{s}") }).collect())
}

fn parse_axes(stdout: &str) -> Result<BTreeMap<String, f64>, String> {
    let line = stdout
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or("the evaluator printed nothing")?;
    let v: serde_json::Value =
        serde_json::from_str(line.trim()).map_err(|e| format!("the evaluator's last line is not JSON: {e}"))?;
    let obj = v.as_object().ok_or("the evaluator's last line is not a JSON object")?;
    let mut out = BTreeMap::new();
    for (k, v) in obj {
        let n = v.as_f64().ok_or_else(|| format!("axis `{k}` is not a number"))?;
        if !n.is_finite() {
            return Err(format!("axis `{k}` is not finite"));
        }
        out.insert(k.clone(), n);
    }
    if out.is_empty() {
        return Err("the evaluator reported no axes".into());
    }
    Ok(out)
}

impl SelfModWorkload {
    /// Record a candidate as refused before anything is built and return the
    /// digest it is recorded under.
    fn refuse(&mut self, spec: &CandidateSpec, why: &str) -> Digest {
        // Named by the genome as given, so two different refused candidates do
        // not share a report.
        let genome = serde_json::to_string(&spec.genome).unwrap_or_default();
        let digest = Digest::of(format!("refused\n{}\n{genome}", self.cfg.base_identity).as_bytes());
        self.artifacts.insert(
            digest.0.clone(),
            ArtifactReport {
                tree: PathBuf::new(),
                files: Vec::new(),
                build_ok: false,
                tests_ok: false,
                seconds: 0.0,
                binary: None,
                failure: Some(format!("refused before building: {why}")),
                eval_failed: false,
                refused: true,
            },
        );
        digest
    }
}

impl Workload for SelfModWorkload {
    /// Build the candidate. A write-set violation or malformed genome is `Ok`
    /// and recorded as refused, before anything is copied; so is a failed build
    /// or test. `Err` (which in the runner halts the run) only for a missing blob,
    /// an unreadable base tree, or a command that cannot be started.
    fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
        let files = match change_set(&spec.genome).and_then(|f| check_write_set(&self.cfg, &f).map(|()| f)) {
            Ok(files) => files,
            Err(why) => return Ok(self.refuse(spec, &why)),
        };
        let digest = artifact_digest(&self.cfg.base_identity, &files);
        if self.artifacts.contains_key(&digest.0) {
            return Ok(digest);
        }
        // Fail closed on a gene the store lacks, as `express` does: building
        // without it would measure a different candidate under this one's name.
        let mut blobs = Vec::with_capacity(files.len());
        for (path, sha) in &files {
            if is_tombstone(sha) {
                // A deletion has no content; protect_tests sees it as an empty file.
                blobs.push(Vec::new());
                continue;
            }
            let bytes = self.store.get(sha).ok_or_else(|| {
                format!(
                    "gene `{path}` {} is not in the blob store (or fails its hash) — a candidate cannot be built without the files it names",
                    &sha[..12]
                )
            })?;
            blobs.push(bytes);
        }
        if self.cfg.protect_tests {
            for ((path, _), bytes) in files.iter().zip(&blobs) {
                if !path.to_ascii_lowercase().ends_with(".rs") {
                    continue;
                }
                let base = match std::fs::read(self.base.join(path)) {
                    Ok(b) => b,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(format!("cannot read base file {path}: {e}")),
                };
                // A base file that is not UTF-8 is not Rust source we can read.
                let Ok(base) = String::from_utf8(base) else { continue };
                if let Err(why) = check_tests_kept(path, &base, bytes) {
                    return Ok(self.refuse(spec, &why));
                }
            }
        }

        let dir = self.scratch.join(&digest.0[..16]);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("cannot clear {}: {e}", dir.display()))?;
        }
        let tree = dir.join("tree");
        copy_tree(&self.base, &tree)?;
        let tree = canon(&tree).map_err(|e| format!("cannot resolve {}: {e}", tree.display()))?;

        let mut sorted: Vec<String> =
            files.iter().map(|(p, s)| if is_tombstone(s) { format!("{p}:deleted") } else { format!("{p}:{s}") }).collect();
        sorted.sort();
        let mut rec = ArtifactReport {
            tree: tree.clone(),
            files: sorted,
            build_ok: false,
            tests_ok: false,
            seconds: 0.0,
            binary: None,
            failure: None,
            eval_failed: false,
            refused: false,
        };

        let mut applied = Ok(());
        for ((path, sha), bytes) in files.iter().zip(&blobs) {
            let done = if is_tombstone(sha) { delete_path(&tree, path) } else { write_gene(&tree, path, bytes) };
            if let Err(e) = done {
                applied = Err(e);
                break;
            }
        }
        if let Err(e) = applied {
            rec.failure = Some(format!("applying the candidate failed: {e}"));
            self.artifacts.insert(digest.0.clone(), rec);
            return Ok(digest);
        }

        let started = Instant::now();
        let built = run_cmd(&self.cfg.build.argv, &tree, &self.cfg.env, self.cfg.build.timeout)?;
        rec.build_ok = built.success;
        if !built.success {
            rec.failure = Some(built.describe("build"));
        } else {
            let tested = run_cmd(&self.cfg.test.argv, &tree, &self.cfg.env, self.cfg.test.timeout)?;
            rec.tests_ok = tested.success;
            if !tested.success {
                rec.failure = Some(tested.describe("tests"));
            }
        }
        rec.seconds = started.elapsed().as_secs_f64();

        if rec.build_ok && rec.tests_ok {
            let root = self.cfg.target_dir.clone().unwrap_or_else(|| tree.clone());
            let src = self.cfg.binary.split('/').fold(root, |acc, c| acc.join(c));
            if src.is_file() {
                let name = src.file_name().map(|n| n.to_os_string()).unwrap_or_else(|| "binary".into());
                let private = dir.join("bin");
                std::fs::create_dir_all(&private).map_err(|e| format!("cannot create {}: {e}", private.display()))?;
                let dest = private.join(name);
                std::fs::copy(&src, &dest).map_err(|e| format!("cannot copy the built binary: {e}"))?;
                rec.binary = Some(dest);
            } else {
                rec.failure = Some(format!("the build succeeded but produced no binary at {}", src.display()));
            }
        }
        self.artifacts.insert(digest.0.clone(), rec);
        Ok(digest)
    }

    fn evaluate(&mut self, artifact: &Digest, suite: &EvalSuite) -> Result<FitnessVector, String> {
        if let Some(expected) = &self.cfg.expected_suite {
            if &suite.digest != expected {
                return Err(format!(
                    "suite `{}` has digest {}, but this workload is configured for {}",
                    suite.id, suite.digest.0, expected.0
                ));
            }
        }
        let rec = self.artifacts.get(&artifact.0).ok_or("artifact was not materialized by this workload")?;
        if !(rec.build_ok && rec.tests_ok) {
            return Ok(self.failed_vector());
        }
        let Some(binary) = rec.binary.clone() else {
            return Ok(self.failed_vector());
        };

        let ran = self.run_with_binary(&self.cfg.evaluator, &binary)?;
        let parsed = if ran.success { parse_axes(&ran.stdout) } else { Err(ran.describe("the evaluator")) };
        match parsed {
            Ok(axes) => {
                let mut v = FitnessVector::new();
                for (k, n) in &axes {
                    v = v.with(k.clone(), *n);
                    self.known_axes.insert(k.clone());
                }
                if !axes.contains_key("correctness") {
                    // The tests passed, or we would not be here.
                    v = v.with("correctness", 1.0);
                }
                self.known_axes.insert("correctness".into());
                Ok(v)
            }
            Err(why) => {
                if let Some(rec) = self.artifacts.get_mut(&artifact.0) {
                    rec.eval_failed = true;
                    rec.failure = Some(why);
                }
                Ok(self.failed_vector())
            }
        }
    }

    fn rebase(&mut self, champion: &[Locus], mut spec: CandidateSpec) -> Result<CandidateSpec, String> {
        let champion_id = genome_id(champion);
        if let Some(stated) = &spec.based_on {
            if *stated != champion_id {
                return Err(format!(
                    "candidate {} is stale: it was written against champion genome {} but the champion is now {}; re-read the champion's tree and resubmit",
                    spec.id,
                    &stated[..12.min(stated.len())],
                    &champion_id[..12]
                ));
            }
        }
        // A malformed candidate genome is the write-set check's to refuse, with
        // its usual record; leave it untouched.
        let Ok(cand) = change_set(&spec.genome) else { return Ok(spec) };
        if cand.iter().any(|(p, _)| check_path_syntax(p).is_err()) {
            return Ok(spec);
        }
        let champ = change_set(champion).map_err(|e| format!("the champion's genome is not a self-modification genome: {e}"))?;
        let mut eff: BTreeMap<String, (String, String)> =
            champ.into_iter().map(|(p, s)| (fold(&p), (p, s))).collect();
        for (path, sha) in cand {
            let key = fold(&path);
            let in_base = self.base_file_hash(&path)?;
            if is_tombstone(&sha) {
                let held = eff.get(&key).map(|(_, s)| is_tombstone(s));
                match (in_base.is_some(), held) {
                    (true, Some(true)) => return Err(format!("`{path}` is already deleted in the champion")),
                    (true, _) => {
                        eff.insert(key, (path, sha));
                    }
                    (false, Some(false)) => {
                        eff.remove(&key);
                    }
                    (false, _) => {
                        return Err(format!("`{path}` cannot be deleted: it is in neither the base tree nor the champion's edits"))
                    }
                }
            } else if in_base.as_deref() == Some(sha.as_str()) {
                eff.remove(&key);
            } else {
                eff.insert(key, (path, sha));
            }
        }
        let genome: Vec<Locus> = eff
            .into_values()
            .map(|(p, s)| if is_tombstone(&s) { tombstone_for(&p) } else { gene_for(&p, &s) })
            .collect();
        if genome_id(&genome) == champion_id {
            return Err(format!("candidate {} changes nothing relative to the champion", spec.id));
        }
        spec.genome = genome;
        spec.based_on = Some(champion_id);
        Ok(spec)
    }

    fn shadow(&mut self, artifact: &Digest) -> HealthSample {
        self.probe(artifact)
    }

    fn observe_champion(&mut self, artifact: &Digest) -> HealthSample {
        self.probe(artifact)
    }

    fn materialized(&self, artifact: &Digest) -> bool {
        self.report(artifact)
            .is_some_and(|r| r.build_ok && r.tests_ok && r.binary.as_deref().is_some_and(Path::is_file))
    }

    fn attribution(&self, artifact: &Digest) -> Vec<String> {
        self.report(artifact).map(|r| r.files.clone()).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// DirProposer
// ---------------------------------------------------------------------------

/// A [`Proposer`] for agents that write candidates as files.
///
/// Each subdirectory `<outbox>/<id>/` holds `manifest.json` (a
/// [`PredictionManifest`]) and `files/<relative path>...`. The candidate's genome
/// is every file under `files/` whose bytes differ from the same path in the base
/// tree (or, with [`DirProposer::with_champion`], from the champion's tree), put
/// into the [`BlobStore`]. A `delete.txt` (one relative path per line) deletes
/// files; an optional `based_on` file names the champion genome the candidate
/// was written against. Candidates are relative to the champion: see the module
/// docs.
///
/// A consumed directory is moved to `<outbox>/.consumed/` so it is proposed once;
/// a malformed one is moved to `<outbox>/.malformed/` (so it is reported once, not
/// every cycle) and listed by [`DirProposer::last_errors`]. Directories whose
/// name starts with `.` are ignored. This proposer does **not** check the write
/// set: [`SelfModWorkload::materialize`] is the one enforcement point.
pub struct DirProposer {
    outbox: PathBuf,
    name: String,
    store: BlobStore,
    base_tree: PathBuf,
    errors: Vec<String>,
    limit: Option<usize>,
    /// The champion's materialized tree and genome id, if candidates are written
    /// against it rather than the base.
    champion: Option<(PathBuf, String)>,
}

impl DirProposer {
    pub fn new(outbox: impl Into<PathBuf>, name: impl Into<String>, store: BlobStore, base_tree: impl Into<PathBuf>) -> Self {
        DirProposer {
            outbox: outbox.into(),
            name: name.into(),
            store,
            base_tree: base_tree.into(),
            errors: Vec::new(),
            limit: None,
            champion: None,
        }
    }

    /// Diff candidates against the champion's tree (a read-only snapshot, see
    /// [`export_genome_tree`]) instead of the base, and stamp each with the
    /// champion's [`genome_id`] unless its directory has a `based_on` file.
    /// A file the candidate carries that equals the champion's is no change; one
    /// that equals the base but not the champion's is a revert.
    pub fn with_champion(mut self, tree: impl Into<PathBuf>, genome_id: impl Into<String>) -> Self {
        self.champion = Some((tree.into(), genome_id.into()));
        self
    }

    /// Consume at most `n` valid candidates per `propose` (in id order); the
    /// rest stay in the outbox for a later call. Malformed ones are still set
    /// aside and reported, and do not count toward the limit.
    pub fn with_limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// What was skipped by the last `propose`, and why.
    pub fn last_errors(&self) -> &[String] {
        &self.errors
    }

    fn collect(dir: &Path, rel: &str, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
        let rd = std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for entry in rd {
            let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
            let name = entry.file_name().into_string().map_err(|_| "a file name is not UTF-8".to_string())?;
            let path = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            let ft = entry.file_type().map_err(|e| format!("cannot stat {path}: {e}"))?;
            if ft.is_symlink() {
                return Err(format!("`{path}` is a symlink"));
            } else if ft.is_dir() {
                Self::collect(&entry.path(), &path, out)?;
            } else {
                out.push((path, entry.path()));
            }
        }
        Ok(())
    }

    fn read_candidate(&self, dir: &Path, id: &str) -> Result<CandidateSpec, String> {
        let manifest_text = std::fs::read_to_string(dir.join("manifest.json"))
            .map_err(|e| format!("manifest.json: {e}"))?;
        let manifest: PredictionManifest =
            serde_json::from_str(&manifest_text).map_err(|e| format!("manifest.json: {e}"))?;
        let files_dir = dir.join("files");
        let delete_file = dir.join("delete.txt");
        if !files_dir.is_dir() && !delete_file.is_file() {
            return Err("no files/ directory".into());
        }
        let mut files = Vec::new();
        if files_dir.is_dir() {
            Self::collect(&files_dir, "", &mut files)?;
        }
        let compare_root = self.champion.as_ref().map_or(&self.base_tree, |(t, _)| t);
        files.sort();
        let mut genome = Vec::new();
        for (rel, path) in files {
            let bytes = std::fs::read(&path).map_err(|e| format!("cannot read {rel}: {e}"))?;
            // Unchanged files are not part of the change set.
            let base_path = rel.split('/').fold(compare_root.clone(), |acc, c| acc.join(c));
            let unchanged = std::fs::read(base_path).is_ok_and(|base| base == bytes);
            if unchanged {
                continue;
            }
            let sha = self.store.put(&bytes)?;
            genome.push(gene_for(&rel, &sha));
        }
        // Deletions: one relative path per line of `delete.txt`.
        if delete_file.is_file() {
            let text = std::fs::read_to_string(&delete_file).map_err(|e| format!("delete.txt: {e}"))?;
            for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
                if genome.iter().any(|g| matches!(g, Locus::Gene { name, .. } if fold(name) == fold(line))) {
                    return Err(format!("`{line}` is both written and deleted"));
                }
                genome.push(tombstone_for(line));
            }
        }
        if genome.is_empty() {
            let what = if self.champion.is_some() { "the champion" } else { "the base tree" };
            return Err(format!("the candidate changes nothing relative to {what}"));
        }
        let mut spec = CandidateSpec::new(id, genome).with_manifest(manifest);
        match std::fs::read_to_string(dir.join("based_on")) {
            Ok(text) if !text.trim().is_empty() => spec.based_on = Some(text.trim().to_string()),
            _ => spec.based_on = self.champion.as_ref().map(|(_, id)| id.clone()),
        }
        Ok(spec)
    }

    fn move_to(&self, from: &Path, bucket: &str, id: &str) -> Result<(), String> {
        let bucket_dir = self.outbox.join(bucket);
        std::fs::create_dir_all(&bucket_dir).map_err(|e| format!("cannot create {}: {e}", bucket_dir.display()))?;
        let mut dest = bucket_dir.join(id);
        if dest.exists() {
            dest = bucket_dir.join(format!("{id}-{}", unique()));
        }
        std::fs::rename(from, &dest).map_err(|e| format!("cannot move {} to {}: {e}", from.display(), dest.display()))
    }
}

impl Proposer for DirProposer {
    fn name(&self) -> &str {
        &self.name
    }

    fn propose(&mut self, _champion: Option<&FitnessVector>, _seed: u64) -> Vec<CandidateSpec> {
        self.errors.clear();
        let Ok(rd) = std::fs::read_dir(&self.outbox) else {
            self.errors.push(format!("cannot read the outbox {}", self.outbox.display()));
            return Vec::new();
        };
        let mut dirs: Vec<(String, PathBuf)> = rd
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().into_string().ok().map(|n| (n, e.path())))
            .filter(|(n, _)| !n.starts_with('.'))
            .collect();
        dirs.sort();

        let mut out = Vec::new();
        for (id, path) in dirs {
            if self.limit.is_some_and(|n| out.len() >= n) {
                break;
            }
            match self.read_candidate(&path, &id) {
                Ok(spec) => match self.move_to(&path, ".consumed", &id) {
                    Ok(()) => out.push(spec),
                    Err(e) => self.errors.push(format!("candidate `{id}` skipped: {e}")),
                },
                Err(e) => {
                    self.errors.push(format!("candidate `{id}` is malformed: {e}"));
                    if let Err(e) = self.move_to(&path, ".malformed", &id) {
                        self.errors.push(format!("candidate `{id}` could not be set aside: {e}"));
                    }
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attest::Attestor;
    use crate::gate::{Episode, PromotionGate};
    use crate::journal::Journal;
    use crate::lineage::Lineage;
    use crate::manifest::Direction;
    use crate::runner::{Halt, Runner, RunnerPolicy};
    use crate::supervisor::SupervisionPolicy;
    use crate::{Generation, Measurement, SuiteKind};

    const EVAL_OK: &str = r#"cap=$(awk 'END{print NR/10}' "$1")
echo "noise"
echo "{\"capability\": $cap, \"safety\": 1}"
"#;

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn sh(script: &str) -> Vec<String> {
        vec!["sh".into(), "-c".into(), script.into()]
    }

    fn fwd(p: &Path) -> String {
        p.to_string_lossy().replace('\\', "/")
    }

    fn put(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content.replace('\r', "")).unwrap();
    }

    struct Fx {
        root: PathBuf,
    }

    impl Fx {
        fn new(name: &str, eval_script: &str) -> Fx {
            let root = std::env::temp_dir().join(format!("germline-selfmod-{name}-{}", unique()));
            let _ = std::fs::remove_dir_all(&root);
            put(&root.join("base/src/lib.txt"), "ok\nb\n");
            put(&root.join("base/tests/expected.txt"), "x\n");
            put(&root.join("eval/eval.sh"), eval_script);
            std::fs::create_dir_all(root.join("scratch")).unwrap();
            Fx { root }
        }

        fn config(&self) -> SelfModConfig {
            let mut cfg = SelfModConfig::new(
                self.root.join("base"),
                "base-v1",
                self.root.join("scratch"),
                Cmd { argv: sh("mkdir -p out && cat src/*.txt > out/bin.txt"), timeout: secs(60) },
                Cmd { argv: sh("grep -q ok src/lib.txt"), timeout: secs(60) },
                "out/bin.txt",
                Cmd { argv: vec!["sh".into(), fwd(&self.root.join("eval/eval.sh"))], timeout: secs(60) },
            );
            cfg.writable = vec!["src/".into()];
            cfg.protected = vec!["src/frozen/".into()];
            cfg
        }

        fn store(&self) -> BlobStore {
            BlobStore::open(self.root.join("blobs")).unwrap()
        }

        fn workload(&self) -> SelfModWorkload {
            SelfModWorkload::new(self.config(), self.store()).unwrap()
        }

        fn spec(&self, id: &str, files: &[(&str, &str)]) -> CandidateSpec {
            let store = self.store();
            let genome = files.iter().map(|(p, c)| gene_for(p, &store.put(c.as_bytes()).unwrap())).collect();
            CandidateSpec::new(id, genome)
        }
    }

    impl Drop for Fx {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn suite() -> EvalSuite {
        EvalSuite::new("heldout", SuiteKind::HeldOut, Digest::of(b"selfmod-suite"))
    }

    fn cfg_for_prefixes(writable: &[&str], protected: &[&str]) -> SelfModConfig {
        let mut c = SelfModConfig::new(
            ".",
            "id",
            "scratch",
            Cmd { argv: vec!["x".into()], timeout: secs(1) },
            Cmd { argv: vec!["x".into()], timeout: secs(1) },
            "bin",
            Cmd { argv: vec!["x".into()], timeout: secs(1) },
        );
        c.writable = writable.iter().map(|s| s.to_string()).collect();
        c.protected = protected.iter().map(|s| s.to_string()).collect();
        c
    }

    fn ws(cfg: &SelfModConfig, paths: &[&str]) -> Result<(), String> {
        let files: Vec<(String, String)> = paths.iter().map(|p| (p.to_string(), "0".repeat(64))).collect();
        check_write_set(cfg, &files)
    }

    // ---- write set ---------------------------------------------------------

    #[test]
    fn write_set_accepts_paths_under_writable_and_not_protected() {
        let c = cfg_for_prefixes(&["prototype/src/", "docs"], &["prototype/src/tests/"]);
        assert!(ws(&c, &["prototype/src/main.rs", "prototype/src/a/b/c.rs", "docs/x.md"]).is_ok());
        assert!(ws(&c, &[]).is_ok());
    }

    #[test]
    fn write_set_refuses_traversal_and_dot_components() {
        let c = cfg_for_prefixes(&["prototype/src/"], &[]);
        for p in [
            "prototype/src/../../evil",
            "prototype/src/..",
            "../prototype/src/x",
            "prototype/src/./x",
            "./prototype/src/x",
            "prototype/src/x/.",
            "prototype//src/x",
            "prototype/src/x/",
        ] {
            assert!(ws(&c, &[p]).is_err(), "`{p}` must be refused");
        }
    }

    #[test]
    fn write_set_refuses_absolute_drive_backslash_and_empty() {
        let c = cfg_for_prefixes(&["prototype/src/"], &[]);
        for p in [
            "/prototype/src/x",
            "/etc/passwd",
            "C:/prototype/src/x",
            "C:prototype/src/x",
            "prototype\\src\\x",
            "prototype/src/x:stream",
            "",
            "prototype/src/nul\0x",
        ] {
            assert!(ws(&c, &[p]).is_err(), "`{p}` must be refused");
        }
    }

    #[test]
    fn write_set_refuses_outside_writable_and_sibling_prefixes() {
        let c = cfg_for_prefixes(&["prototype/src/"], &[]);
        assert!(ws(&c, &["prototype/Cargo.toml"]).is_err());
        assert!(ws(&c, &["other/x"]).is_err());
        assert!(ws(&c, &["prototype/src2/x"]).is_err(), "prefixes match whole components");
        assert!(ws(&c, &["prototype/src"]).is_ok(), "the prefix itself is covered");
        assert!(ws(&c, &["prototype/srcx"]).is_err());
    }

    #[test]
    fn write_set_refuses_protected_inside_writable() {
        let c = cfg_for_prefixes(&["prototype/"], &["prototype/tests/", "prototype/src/lock.rs"]);
        assert!(ws(&c, &["prototype/src/x.rs"]).is_ok());
        let e = ws(&c, &["prototype/tests/t.rs"]).unwrap_err();
        assert!(e.contains("protected"), "{e}");
        assert!(ws(&c, &["prototype/tests/deep/er/t.rs"]).is_err());
        assert!(ws(&c, &["prototype/src/lock.rs"]).is_err(), "a protected exact file");
        assert!(ws(&c, &["prototype/src/ok.rs", "prototype/tests/x"]).is_err(), "one bad path refuses the set");
    }

    #[test]
    fn write_set_refuses_names_windows_would_alias() {
        let c = cfg_for_prefixes(&["prototype/"], &["prototype/tests/"]);
        // Windows strips trailing dots and spaces, so these would reach tests/.
        assert!(ws(&c, &["prototype/tests./evil"]).is_err());
        assert!(ws(&c, &["prototype/tests /evil"]).is_err());
    }

    /// Case folding and Windows aliases on Windows; exact, case-sensitive
    /// matching elsewhere. One test on every platform, so the test count does
    /// not depend on where it runs.
    #[test]
    fn write_set_case_rules_follow_the_platform() {
        let c = cfg_for_prefixes(&["prototype/src/"], &["prototype/src/Tests/"]);
        // Not all zeros: that hash is the tombstone marker, which `changed_files` refuses.
        let genome = vec![gene_for("A/x", &"1".repeat(64)), gene_for("a/X", &"1".repeat(64))];
        if cfg!(windows) {
            assert!(ws(&c, &["PROTOTYPE/SRC/x.rs"]).is_ok(), "a case variant of a writable prefix is under it");
            assert!(ws(&c, &["Prototype/Src/tests/t.rs"]).is_err(), "a case variant of a protected prefix is protected");
            assert!(ws(&c, &["prototype/src/TESTS/t.rs"]).is_err());
            assert!(ws(&c, &["prototype/src/PROTEC~1/t.rs"]).is_err(), "short names are refused");
            assert!(ws(&c, &["prototype/src/nul.txt"]).is_err(), "device names are refused");
            assert!(changed_files(&genome).is_err(), "case variants are one file on Windows");
        } else {
            assert!(ws(&c, &["PROTOTYPE/SRC/x.rs"]).is_err(), "another directory on a case-sensitive file system");
            assert!(ws(&c, &["prototype/src/Tests/t.rs"]).is_err(), "the protected prefix itself");
            assert!(ws(&c, &["prototype/src/TESTS/t.rs"]).is_ok(), "a different directory from `Tests`");
            assert!(changed_files(&genome).is_ok(), "two files on a case-sensitive file system");
        }
    }

    #[test]
    fn a_timeout_kills_the_whole_process_tree() {
        let dir = std::env::temp_dir().join(format!("germline-selfmod-killtree-{}", unique()));
        std::fs::create_dir_all(&dir).unwrap();
        // A grandchild that would leave a mark after the timeout has passed. On
        // Windows the tree is native processes (as cargo, rustc and the linker
        // are), since MSYS `sh` re-parents what it forks and execs.
        let argv: Vec<String> = if cfg!(windows) {
            // Full paths throughout: cmd may be told not to search the current
            // directory (NoDefaultCurrentDirectoryInExePath), and then a bare
            // `grand.bat` never runs and the test would pass vacuously.
            std::fs::write(dir.join("grand.bat"), "@ping -n 7 127.0.0.1 >nul\r\n@echo late> \"%~dp0mark.txt\"\r\n").unwrap();
            std::fs::write(dir.join("run.bat"), "@start /b cmd /c \"%~dp0grand.bat\"\r\n@ping -n 31 127.0.0.1 >nul\r\n").unwrap();
            vec!["cmd".into(), "/c".into(), dir.join("run.bat").to_string_lossy().into_owned()]
        } else {
            sh("sh -c 'sleep 6; echo late > mark.txt' & sleep 30")
        };
        let started = Instant::now();
        // The timeout falls well after the grandchild has started, so this tests
        // the tree kill, not the spawn race described at `kill_tree`.
        let ran = run_cmd(&argv, &dir, &[], Duration::from_secs(3)).unwrap();
        assert!(ran.timed_out && !ran.success);
        // Timeout 3 s plus at most 5 s of `taskkill`; the parent alone would run
        // 30 s, so this bound separates "returned at the timeout" from "waited".
        assert!(started.elapsed() < Duration::from_secs(20), "the run returned at the timeout");
        std::thread::sleep(Duration::from_secs(6));
        // On Unix the process group is killed as a unit, so this is exact. On
        // Windows `taskkill /T` was seen to miss a live descendant under load (one
        // full-suite run in three), so there the tree kill is best effort and only
        // the timeout itself is asserted; see `kill_tree`.
        if !cfg!(windows) {
            assert!(!dir.join("mark.txt").exists(), "the grandchild was killed with its parent");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- blob store and genome ---------------------------------------------

    #[test]
    fn blob_store_is_content_addressed_idempotent_and_verifying() {
        let fx = Fx::new("blob", EVAL_OK);
        let s = fx.store();
        let sha = s.put(b"hello").unwrap();
        assert_eq!(sha, sha_hex(b"hello"));
        assert_eq!(s.put(b"hello").unwrap(), sha, "idempotent");
        assert_eq!(s.get(&sha).unwrap(), b"hello");
        assert_eq!(s.resolve(&sha).as_deref(), Some("hello"));
        assert!(s.get(&"0".repeat(64)).is_none());
        assert!(s.get("../x").is_none(), "a non-hash name never touches the file system");
        // Tamper: content no longer matches its name.
        std::fs::write(s.root().join(&sha), b"HELLO").unwrap();
        assert!(s.get(&sha).is_none());
        assert!(s.resolve(&sha).is_none());
        // Non-UTF-8 content resolves as None, but is still retrievable as bytes.
        let bin = s.put(&[0xff, 0xfe]).unwrap();
        assert!(s.resolve(&bin).is_none());
        assert!(s.get(&bin).is_some());
    }

    #[test]
    fn changed_files_refuses_scalars_duplicates_and_bad_genes() {
        let sha = "a".repeat(64);
        assert_eq!(changed_files(&[gene_for("a/b", &sha)]).unwrap(), vec![("a/b".to_string(), sha.clone())]);
        assert!(changed_files(&[Locus::param(0.5)]).is_err());
        assert!(changed_files(&[gene_for("a", &sha), gene_for("a", &sha)]).is_err());
        assert!(changed_files(&[gene_for("a", "short")]).is_err());
        assert!(changed_files(&[Locus::gene("a", &sha, "Other(x)", Effects::default())]).is_err());
        assert!(changed_files(&[]).unwrap().is_empty());
    }

    // ---- construction ------------------------------------------------------

    #[test]
    fn an_evaluator_inside_the_scratch_root_is_refused() {
        let fx = Fx::new("evscratch", EVAL_OK);
        put(&fx.root.join("scratch/judge.sh"), EVAL_OK);
        let mut cfg = fx.config();
        cfg.evaluator.argv = vec!["sh".into(), fwd(&fx.root.join("scratch/judge.sh"))];
        let e = SelfModWorkload::new(cfg, fx.store()).err().expect("must be refused");
        assert!(e.contains("scratch root"), "{e}");
    }

    #[test]
    fn an_evaluator_inside_a_writable_prefix_is_refused() {
        let fx = Fx::new("evwritable", EVAL_OK);
        put(&fx.root.join("base/src/judge.sh"), EVAL_OK);
        let mut cfg = fx.config();
        cfg.evaluator.argv = vec!["sh".into(), fwd(&fx.root.join("base/src/judge.sh"))];
        let e = SelfModWorkload::new(cfg, fx.store()).err().expect("must be refused");
        assert!(e.contains("writable prefix"), "{e}");
    }

    #[test]
    fn unsafe_configs_are_refused() {
        let fx = Fx::new("badcfg", EVAL_OK);
        let mut c = fx.config();
        c.writable = vec![];
        assert!(SelfModWorkload::new(c, fx.store()).is_err(), "no writable prefix");
        let mut c = fx.config();
        c.writable = vec!["".into()];
        assert!(SelfModWorkload::new(c, fx.store()).is_err(), "an empty prefix would cover everything");
        let mut c = fx.config();
        c.protected = vec!["../x".into()];
        assert!(SelfModWorkload::new(c, fx.store()).is_err());
        let mut c = fx.config();
        c.scratch_root = fx.root.join("base/scratch");
        assert!(SelfModWorkload::new(c, fx.store()).is_err(), "scratch inside the base tree");
        let mut c = fx.config();
        c.base_tree = fx.root.join("missing");
        assert!(SelfModWorkload::new(c, fx.store()).is_err());
    }

    // ---- materialize and evaluate ------------------------------------------

    #[test]
    fn a_good_candidate_is_built_tested_and_measured() {
        let fx = Fx::new("good", EVAL_OK);
        let mut w = fx.workload();
        let spec = fx.spec("good", &[("src/lib.txt", "ok\n1\n2\n3\n4\n")]);
        let d = w.materialize(&spec).unwrap();
        assert!(w.materialized(&d));
        let r = w.report(&d).unwrap();
        assert!(r.build_ok && r.tests_ok && r.binary.is_some(), "{r:?}");
        assert!(r.seconds >= 0.0);
        assert_eq!(w.attribution(&d).len(), 1);
        assert!(w.attribution(&d)[0].starts_with("src/lib.txt:"));
        let f = w.evaluate(&d, &suite()).unwrap();
        assert!((f.get("capability").unwrap() - 0.5).abs() < 1e-9, "{f:?} {:?}", w.report(&d));
        assert_eq!(f.get("correctness"), Some(1.0));
        assert_eq!(f.get("safety"), Some(1.0));
        // Same genome, same artifact, no rebuild.
        assert_eq!(w.materialize(&spec).unwrap(), d);
        // The base tree is untouched.
        assert_eq!(std::fs::read_to_string(fx.root.join("base/src/lib.txt")).unwrap(), "ok\nb\n");
    }

    #[test]
    fn an_empty_genome_is_the_base_tree_and_the_base_identity_is_in_the_digest() {
        let fx = Fx::new("empty", EVAL_OK);
        let mut w = fx.workload();
        let d = w.materialize(&CandidateSpec::new("genesis", vec![])).unwrap();
        assert!(w.materialized(&d));
        let f = w.evaluate(&d, &suite()).unwrap();
        assert!((f.get("capability").unwrap() - 0.2).abs() < 1e-9, "{f:?}");
        assert_ne!(artifact_digest("base-v1", &[]), artifact_digest("base-v2", &[]));
    }

    #[test]
    fn a_candidate_that_breaks_the_tests_scores_zero_and_is_not_an_error() {
        let fx = Fx::new("badtests", EVAL_OK);
        let mut w = fx.workload();
        let d = w.materialize(&CandidateSpec::new("genesis", vec![])).unwrap();
        w.evaluate(&d, &suite()).unwrap();
        let spec = fx.spec("bad", &[("src/lib.txt", "bad\n1\n2\n3\n4\n5\n6\n7\n8\n9\n")]);
        let bad = w.materialize(&spec).expect("a failing test is not an Err: it would halt the run");
        assert!(!w.materialized(&bad), "a failed candidate can never be promoted");
        let r = w.report(&bad).unwrap();
        assert!(r.build_ok && !r.tests_ok);
        assert!(r.failure.as_deref().unwrap().contains("tests"));
        let f = w.evaluate(&bad, &suite()).unwrap();
        assert_eq!(f.get("capability"), Some(0.0));
        assert_eq!(f.get("correctness"), Some(0.0));
        assert_eq!(f.get("safety"), Some(0.0), "axes seen before are zero-filled so the vector stays comparable");
    }

    #[test]
    fn a_candidate_that_breaks_the_build_scores_zero_and_is_not_an_error() {
        let fx = Fx::new("badbuild", EVAL_OK);
        let mut cfg = fx.config();
        cfg.build.argv = sh("mkdir -p out && cat src/*.txt > out/bin.txt && ! grep -q BREAK src/lib.txt");
        let mut w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        let bad = w.materialize(&fx.spec("b", &[("src/lib.txt", "ok\nBREAK\n")])).unwrap();
        assert!(!w.materialized(&bad));
        assert!(!w.report(&bad).unwrap().build_ok);
        assert_eq!(w.evaluate(&bad, &suite()).unwrap().get("capability"), Some(0.0));
    }

    #[test]
    fn a_build_that_hangs_is_killed_and_recorded() {
        let fx = Fx::new("hang", EVAL_OK);
        let mut cfg = fx.config();
        cfg.build = Cmd { argv: sh("sleep 30"), timeout: Duration::from_millis(300) };
        let mut w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        let t = Instant::now();
        let d = w.materialize(&fx.spec("h", &[("src/lib.txt", "ok\n")])).unwrap();
        assert!(t.elapsed() < secs(20));
        assert!(!w.materialized(&d));
        assert!(w.report(&d).unwrap().failure.as_deref().unwrap().contains("timed out"));
    }

    #[test]
    fn a_missing_blob_is_an_error_not_a_silent_omission() {
        let fx = Fx::new("noblob", EVAL_OK);
        let mut w = fx.workload();
        let spec = CandidateSpec::new("ghost", vec![gene_for("src/lib.txt", &"b".repeat(64))]);
        let e = w.materialize(&spec).unwrap_err();
        assert!(e.contains("not in the blob store"), "{e}");
    }

    #[test]
    fn write_set_violations_are_refused_before_anything_is_copied() {
        let fx = Fx::new("violation", EVAL_OK);
        let mut w = fx.workload();
        for (p, why) in [
            ("tests/expected.txt", "outside"),
            ("src/../tests/expected.txt", ".."),
            ("src/./lib.txt", "`.`"),
            ("/abs/x", "absolute"),
            ("src/frozen/x", "protected"),
        ] {
            let d = w.materialize(&fx.spec("v", &[("src/ok.txt", "ok\n"), (p, "x")])).expect("a refusal, not an Err");
            let r = w.report(&d).unwrap();
            let e = r.failure.clone().unwrap();
            assert!(r.refused, "{p}");
            assert!(e.contains(why) || e.contains("write set"), "{p}: {e}");
            assert!(!w.materialized(&d), "{p}");
            let f = w.evaluate(&d, &suite()).unwrap();
            assert_eq!(f.get("capability"), Some(0.0), "{p}");
        }
        let entries: Vec<_> = std::fs::read_dir(fx.root.join("scratch")).unwrap().collect();
        assert!(entries.is_empty(), "nothing was copied: {entries:?}");
    }

    // ---- protect_tests -----------------------------------------------------

    const RS_BASE: &str = "pub fn f() -> u32 { 1 }\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn keeps() {\n        assert_eq!(f(), 1);\n    }\n}\n";

    fn tests_fx(name: &str) -> (Fx, SelfModWorkload) {
        let fx = Fx::new(name, EVAL_OK);
        put(&fx.root.join("base/src/m.rs"), RS_BASE);
        let mut cfg = fx.config();
        cfg.protect_tests = true;
        let w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        (fx, w)
    }

    fn refused_with(w: &mut SelfModWorkload, fx: &Fx, rs: &str) -> Option<String> {
        let d = w.materialize(&fx.spec("p", &[("src/m.rs", rs)])).expect("a refusal, not an Err");
        let r = w.report(&d).unwrap();
        r.refused.then(|| r.failure.clone().unwrap())
    }

    #[test]
    fn protect_tests_accepts_an_added_test_and_reindentation() {
        let (fx, mut w) = tests_fx("pt-add");
        let added = format!("{RS_BASE}#[test]\nfn extra() {{ assert!(true); }}\n");
        assert_eq!(refused_with(&mut w, &fx, &added), None);
        let reindented = RS_BASE.replace("    ", "\t\t").replace("\n\n", "\n\n\n");
        assert_eq!(refused_with(&mut w, &fx, &reindented), None);
    }

    #[test]
    fn protect_tests_refuses_removed_ignored_and_weakened_tests() {
        let (fx, mut w) = tests_fx("pt-bad");
        let removed = "pub fn f() -> u32 { 1 }\n";
        let ignored = RS_BASE.replace("    #[test]", "    #[test]\n    #[ignore]");
        let weakened = RS_BASE.replace("assert_eq!(f(), 1)", "assert_eq!(f(), f())");
        let emptied = RS_BASE.replace("assert_eq!(f(), 1);", "");
        for (what, rs) in [("removed", removed.to_string()), ("ignored", ignored), ("weakened", weakened), ("emptied", emptied)] {
            let e = refused_with(&mut w, &fx, &rs).unwrap_or_else(|| panic!("{what} must be refused"));
            assert!(e.contains("test `tests::keeps` in src/m.rs was removed or changed (protect_tests)"), "{what}: {e}");
            assert!(e.starts_with("refused before building:"), "{e}");
        }
        let entries: Vec<_> = std::fs::read_dir(fx.root.join("scratch")).unwrap().collect();
        assert!(entries.is_empty(), "nothing was copied: {entries:?}");
        // An emptied file (the closest a genome gets to deleting one) is refused too.
        assert!(refused_with(&mut w, &fx, "").is_some());
    }

    #[test]
    fn protect_tests_off_does_not_refuse_a_removal() {
        let fx = Fx::new("pt-off", EVAL_OK);
        put(&fx.root.join("base/src/m.rs"), RS_BASE);
        let mut w = fx.workload();
        assert_eq!(refused_with(&mut w, &fx, "pub fn f() -> u32 { 1 }\n"), None);
    }

    #[test]
    fn test_extraction_ignores_attributes_in_strings_and_comments() {
        let src = "// #[test]\n// fn a() {}\n/* #[test] fn b() {} */\nconst S: &str = \"#[test] fn c() {}\";\nconst R: &str = r#\"#[test]\nfn d() {}\"#;\n#[test]\nfn real() {}\n";
        let names: Vec<String> = test_fns(src).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["real"]);
        let tokio = "#[tokio::test(flavor = \"multi_thread\")]\nasync fn t() {}\n#[test_case(1)]\nfn not() {}\n";
        let names: Vec<String> = test_fns(tokio).into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, ["t"]);
    }

    #[test]
    fn test_extraction_matches_braces_past_strings_chars_lifetimes_and_comments() {
        let src = concat!(
            "mod m {\n",
            "    #[test]\n",
            "    fn tricky<'a>(x: &'a str) {\n",
            "        let _ = \"}}}{\\\"}\";\n",
            "        let _ = '}'; let _ = '{'; let _ = '\\''; let _ = '\\\\';\n",
            "        let _ = r##\"} \"# {\"##;\n",
            "        // }\n",
            "        /* { /* } */ } */\n",
            "        let _: &'static str = x;\n",
            "    }\n",
            "    #[test]\n",
            "    fn after() {}\n",
            "}\n",
        );
        let t = test_fns(src);
        let names: Vec<&str> = t.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["m::tricky", "m::after"]);
        assert!(t[0].1.ends_with("let _: &'static str = x;\n}"), "{}", t[0].1);
        assert_eq!(t[1].1, "#[test]\nfn after() {}");
    }

    #[test]
    fn an_evaluator_printing_nan_is_recorded_as_failed_with_zero_capability() {
        let fx = Fx::new("nan", "echo '{\"capability\": NaN, \"safety\": 1}'\n");
        let mut w = fx.workload();
        let d = w.materialize(&fx.spec("n", &[("src/lib.txt", "ok\n")])).unwrap();
        let f = w.evaluate(&d, &suite()).unwrap();
        assert_eq!(f.get("capability"), Some(0.0));
        assert_eq!(f.get("correctness"), Some(0.0));
        let r = w.report(&d).unwrap();
        assert!(r.eval_failed, "{r:?}");
        assert!(w.materialized(&d), "materialized is about the build; the zero fitness is what refuses it");
    }

    #[test]
    fn evaluator_output_is_parsed_strictly() {
        assert!(parse_axes("x\n{\"a\": 0.5}\n\n").is_ok());
        assert!(parse_axes("{\"a\": \"x\"}").is_err());
        assert!(parse_axes("[1]").is_err());
        assert!(parse_axes("{}").is_err());
        assert!(parse_axes("").is_err());
        assert!(parse_axes("{\"a\": 1e999}").is_err());
    }

    #[test]
    fn an_evaluator_that_reports_correctness_keeps_its_value_and_a_failing_one_is_zero() {
        let fx = Fx::new("corr", "echo '{\"capability\": 0.4, \"correctness\": 0.5}'\n");
        let mut w = fx.workload();
        let d = w.materialize(&fx.spec("c", &[("src/lib.txt", "ok\n")])).unwrap();
        assert_eq!(w.evaluate(&d, &suite()).unwrap().get("correctness"), Some(0.5));
        let fx = Fx::new("evfail", "exit 3\n");
        let mut w = fx.workload();
        let d = w.materialize(&fx.spec("c", &[("src/lib.txt", "ok\n")])).unwrap();
        let f = w.evaluate(&d, &suite()).unwrap();
        assert_eq!(f.get("capability"), Some(0.0));
        assert!(w.report(&d).unwrap().eval_failed);
    }

    #[test]
    fn a_suite_with_the_wrong_digest_is_refused() {
        let fx = Fx::new("suite", EVAL_OK);
        let mut cfg = fx.config();
        cfg.expected_suite = Some(Digest::of(b"another-suite"));
        let mut w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        let d = w.materialize(&fx.spec("s", &[("src/lib.txt", "ok\n")])).unwrap();
        assert!(w.evaluate(&d, &suite()).unwrap_err().contains("configured for"));
        let mut cfg = fx.config();
        cfg.expected_suite = Some(suite().digest);
        let mut w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        let d = w.materialize(&fx.spec("s", &[("src/lib.txt", "ok\n")])).unwrap();
        assert!(w.evaluate(&d, &suite()).is_ok());
    }

    #[test]
    fn shadow_and_champion_probes_follow_the_command_and_materialization() {
        let fx = Fx::new("shadow", EVAL_OK);
        let mut w = fx.workload();
        let good = w.materialize(&fx.spec("g", &[("src/lib.txt", "ok\n")])).unwrap();
        let bad = w.materialize(&fx.spec("b", &[("src/lib.txt", "bad\n")])).unwrap();
        assert!(w.shadow(&good).success && w.observe_champion(&good).success);
        assert!(!w.shadow(&bad).success, "no shadow command: ok only if materialized");
        assert!(!w.shadow(&Digest::of(b"unknown")).success);

        let mut cfg = fx.config();
        cfg.shadow = Some(Cmd { argv: sh("test -f \"$0\""), timeout: secs(30) });
        let mut w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        let good = w.materialize(&fx.spec("g", &[("src/lib.txt", "ok\n")])).unwrap();
        assert!(w.shadow(&good).success, "exit 0");
        let mut cfg = fx.config();
        cfg.shadow = Some(Cmd { argv: sh("exit 1"), timeout: secs(30) });
        let mut w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        let good = w.materialize(&fx.spec("g", &[("src/lib.txt", "ok\n")])).unwrap();
        assert!(!w.shadow(&good).success && !w.observe_champion(&good).success, "non-zero exit");
    }

    #[test]
    fn a_candidate_colliding_with_a_base_directory_is_recorded_not_an_error() {
        let fx = Fx::new("collide", EVAL_OK);
        put(&fx.root.join("base/src/dir/a.txt"), "ok\n");
        let mut w = fx.workload();
        let d = w.materialize(&fx.spec("c", &[("src/dir", "ok\n")])).unwrap();
        assert!(!w.materialized(&d));
        assert!(w.report(&d).unwrap().failure.as_deref().unwrap().contains("applying"));
    }

    // ---- DirProposer -------------------------------------------------------

    fn write_candidate(outbox: &Path, id: &str, files: &[(&str, &str)], metric: &str, magnitude: f64) {
        let dir = outbox.join(id);
        for (p, c) in files {
            put(&dir.join("files").join(p), c);
        }
        put(
            &dir.join("manifest.json"),
            &format!("{{\"metric\":\"{metric}\",\"direction\":\"increase\",\"magnitude\":{magnitude}}}"),
        );
    }

    #[test]
    fn a_dir_proposer_reads_candidates_once_and_reports_malformed_ones() {
        let fx = Fx::new("dirprop", EVAL_OK);
        let outbox = fx.root.join("outbox");
        write_candidate(&outbox, "b-two", &[("src/lib.txt", "ok\n1\n2\n"), ("src/new.txt", "n\n")], "capability", 0.1);
        write_candidate(&outbox, "a-one", &[("src/lib.txt", "ok\n1\n")], "capability", 0.1);
        // Unchanged relative to the base: not part of the change set.
        write_candidate(&outbox, "c-same", &[("src/lib.txt", "ok\nb\n")], "capability", 0.1);
        // No manifest.
        put(&outbox.join("d-nomanifest/files/src/x.txt"), "x");
        // Bad manifest.
        put(&outbox.join("e-badmanifest/manifest.json"), "{");
        put(&outbox.join("e-badmanifest/files/src/x.txt"), "x");
        // No files dir.
        put(&outbox.join("f-nofiles/manifest.json"), "{\"metric\":\"m\",\"direction\":\"increase\",\"magnitude\":1}");
        // Ignored.
        std::fs::create_dir_all(outbox.join(".hidden")).unwrap();

        let mut p = DirProposer::new(&outbox, "agent", fx.store(), fx.root.join("base"));
        assert_eq!(p.name(), "agent");
        let specs = p.propose(None, 1);
        let ids: Vec<&str> = specs.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a-one", "b-two"], "sorted, unchanged and malformed excluded");
        assert_eq!(specs[1].genome.len(), 2);
        assert!(specs.iter().all(|s| s.manifest.as_ref().is_some_and(|m| m.direction == Direction::Increase)));
        let errs = p.last_errors().join("\n");
        for id in ["c-same", "d-nomanifest", "e-badmanifest", "f-nofiles"] {
            assert!(errs.contains(id), "{id} is reported: {errs}");
        }
        assert!(outbox.join(".consumed/a-one").is_dir() && !outbox.join("a-one").exists());
        assert!(outbox.join(".malformed/e-badmanifest").is_dir());
        // Proposed once.
        assert!(p.propose(None, 2).is_empty());
        assert!(p.last_errors().is_empty());
        // The genes resolve to the files' contents.
        let Locus::Gene { sha256, .. } = &specs[0].genome[0] else { panic!() };
        assert_eq!(fx.store().resolve(sha256).as_deref(), Some("ok\n1\n"));
    }

    #[test]
    fn a_dir_proposer_candidate_outside_the_write_set_is_refused_by_the_workload() {
        let fx = Fx::new("dirviolation", EVAL_OK);
        let outbox = fx.root.join("outbox");
        write_candidate(
            &outbox,
            "evil",
            &[("src/lib.txt", "ok\n1\n2\n3\n4\n5\n6\n7\n8\n9\n"), ("tests/expected.txt", "rigged\n")],
            "capability",
            0.5,
        );
        let mut p = DirProposer::new(&outbox, "agent", fx.store(), fx.root.join("base"));
        let specs = p.propose(None, 1);
        assert_eq!(specs.len(), 1, "the proposer does not enforce the write set");
        let mut w = fx.workload();
        let d = w.materialize(&specs[0]).expect("a refusal, not an Err");
        let r = w.report(&d).unwrap();
        assert!(r.refused && r.failure.as_deref().unwrap().contains("tests/expected.txt"), "{r:?}");
        assert!(!w.materialized(&d));
        assert_eq!(std::fs::read_dir(fx.root.join("scratch")).unwrap().count(), 0);
    }

    // ---- the real runner ---------------------------------------------------

    /// Writes the next scripted candidate into the outbox on each call, then
    /// reads it back through a [`DirProposer`].
    struct Scripted {
        outbox: PathBuf,
        queue: Vec<(&'static str, Vec<(&'static str, &'static str)>)>,
        inner: DirProposer,
    }

    impl Proposer for Scripted {
        fn name(&self) -> &str {
            self.inner.name()
        }
        fn propose(&mut self, champion: Option<&FitnessVector>, seed: u64) -> Vec<CandidateSpec> {
            if !self.queue.is_empty() {
                let (id, files) = self.queue.remove(0);
                write_candidate(&self.outbox, id, &files, "capability", 0.3);
            }
            self.inner.propose(champion, seed)
        }
    }

    #[test]
    fn the_runner_promotes_an_improving_edit_and_refuses_a_write_set_violation() {
        const EVALUATOR: &str = "selfmod-harness";
        let fx = Fx::new("runner", EVAL_OK);
        let mut w = fx.workload();
        let outbox = fx.root.join("outbox");

        let ep = Episode::open(
            PromotionGate {
                primary_axis: "capability".into(),
                min_improvement: 0.01,
                guard_axes: vec!["safety".into()],
                guard_tolerance: 0.01,
                min_shadow_successes: 2,
                dominance: None,
            },
            suite().digest,
            EVALUATOR,
        );
        let at = Attestor::new(EVALUATOR, b"key".to_vec());

        // Genesis: the base tree, empty genome, measured by the same evaluator.
        let genesis = CandidateSpec::new("genesis", vec![]);
        let gd = w.materialize(&genesis).unwrap();
        let gf = w.evaluate(&gd, &suite()).unwrap();
        let mut l = Lineage::new();
        let id = l.next_id();
        let mut g = Generation::new(id, gd.clone()).measured(Measurement {
            suite: suite(),
            fitness: gf,
            evaluator: EVALUATOR.into(),
        });
        g.genome = Some(vec![]);
        l.add(g);
        l.grant(id, ep.gate_digest.clone(), EVALUATOR).unwrap();

        let proposer = Scripted {
            outbox: outbox.clone(),
            queue: vec![
                ("c1-improves", vec![("src/lib.txt", "ok\n1\n2\n3\n4\n")]),
                ("c2-violates", vec![("src/lib.txt", "ok\n1\n2\n3\n4\n5\n6\n7\n8\n9\n"), ("tests/expected.txt", "rigged\n")]),
            ],
            inner: DirProposer::new(&outbox, "agent", fx.store(), fx.root.join("base")),
        };
        let policy = RunnerPolicy {
            max_cycles: 2,
            max_consecutive_refusals: 2,
            shadow_runs: 2,
            supervision: SupervisionPolicy { window: 3, ..SupervisionPolicy::default() },
            ..RunnerPolicy::default()
        };
        let jpath = fx.root.join("journal.jsonl");
        let mut j = Journal::open(&jpath).unwrap();
        let report = Runner::new(policy, &ep, &at, suite(), 1)
            .with_proposer(Box::new(proposer))
            .run(&mut l, &mut j, &mut w);

        assert_eq!(report.promotions, 1, "{:?}", report.halt);
        assert!(report.cycles[0].promoted);
        assert!(!matches!(report.halt, Halt::WorkloadFailed { .. }), "a violation must not halt: {:?}", report.halt);
        assert_eq!(report.cycles.len(), 2, "{:?}", report.halt);
        assert!(!report.cycles[1].promoted, "the violating candidate is refused");
        assert_eq!(report.refusals, 1);
        let champion = l.champion().unwrap();
        assert_ne!(champion.artifact, gd, "the improving edit holds authority");
        assert!(champion.definitions.iter().any(|d| d.starts_with("src/lib.txt:")), "attribution is journalled");
        let f = champion.heldout_fitness().unwrap();
        assert!((f.get("capability").unwrap() - 0.5).abs() < 1e-9);
        assert!(w.materialized(&champion.artifact));
        // Only the genesis and the good candidate were ever built; the violator never was.
        let built = std::fs::read_dir(fx.root.join("scratch"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .count();
        assert_eq!(built, 2);
        assert!(j.verify().is_ok());
    }

    #[test]
    fn a_failing_candidate_never_reaches_authority_and_the_run_continues() {
        const EVALUATOR: &str = "selfmod-harness";
        let fx = Fx::new("runner-fail", EVAL_OK);
        let mut w = fx.workload();
        let outbox = fx.root.join("outbox");
        let ep = Episode::open(
            PromotionGate {
                primary_axis: "capability".into(),
                min_improvement: 0.01,
                guard_axes: vec!["safety".into()],
                guard_tolerance: 0.01,
                min_shadow_successes: 2,
                dominance: None,
            },
            suite().digest,
            EVALUATOR,
        );
        let at = Attestor::new(EVALUATOR, b"key".to_vec());
        let gd = w.materialize(&CandidateSpec::new("genesis", vec![])).unwrap();
        let gf = w.evaluate(&gd, &suite()).unwrap();
        let mut l = Lineage::new();
        let id = l.next_id();
        let mut g = Generation::new(id, gd.clone()).measured(Measurement { suite: suite(), fitness: gf, evaluator: EVALUATOR.into() });
        g.genome = Some(vec![]);
        l.add(g);
        l.grant(id, ep.gate_digest.clone(), EVALUATOR).unwrap();

        let proposer = Scripted {
            outbox: outbox.clone(),
            // Many lines, so it would win on capability if its tests had not failed.
            queue: vec![("c1-breaks-tests", vec![("src/lib.txt", "bad\n1\n2\n3\n4\n5\n6\n7\n8\n9\n")])],
            inner: DirProposer::new(&outbox, "agent", fx.store(), fx.root.join("base")),
        };
        let policy = RunnerPolicy { max_cycles: 1, shadow_runs: 2, ..RunnerPolicy::default() };
        let mut j = Journal::open(fx.root.join("journal.jsonl")).unwrap();
        let report = Runner::new(policy, &ep, &at, suite(), 1)
            .with_proposer(Box::new(proposer))
            .run(&mut l, &mut j, &mut w);
        assert_eq!(report.promotions, 0);
        assert_eq!(report.refusals, 1);
        assert!(matches!(report.halt, Halt::BudgetExhausted { .. }), "{:?}", report.halt);
        assert_eq!(l.champion().unwrap().artifact, gd);
    }

    // ---- champion-relative candidates and deletion -------------------------

    /// Capability is 1 / (lines in all of src/*.txt): shrinking the tree is
    /// what improves it.
    const EVAL_SHRINK: &str = r#"cap=$(awk 'END{print 1/NR}' "$1")
echo "{\"capability\": $cap, \"safety\": 1}"
"#;

    fn sha(fx: &Fx, text: &str) -> String {
        fx.store().put(text.as_bytes()).unwrap()
    }

    fn names(genome: &[Locus]) -> Vec<(String, bool)> {
        change_set(genome).unwrap().into_iter().map(|(p, s)| (p, is_tombstone(&s))).collect()
    }

    fn ep(min_shadow: u32) -> Episode {
        Episode::open(
            PromotionGate {
                primary_axis: "capability".into(),
                min_improvement: 0.01,
                guard_axes: vec!["safety".into()],
                guard_tolerance: 0.01,
                min_shadow_successes: min_shadow,
                dominance: None,
            },
            suite().digest,
            "selfmod-harness",
        )
    }

    fn genesis_lineage(w: &mut SelfModWorkload, ep: &Episode) -> Lineage {
        let gd = w.materialize(&CandidateSpec::new("genesis", vec![])).unwrap();
        let gf = w.evaluate(&gd, &suite()).unwrap();
        let mut l = Lineage::new();
        let id = l.next_id();
        let mut g = Generation::new(id, gd).measured(Measurement { suite: suite(), fitness: gf, evaluator: "selfmod-harness".into() });
        g.genome = Some(vec![]);
        l.add(g);
        l.grant(id, ep.gate_digest.clone(), "selfmod-harness").unwrap();
        l
    }

    type Queue = Vec<(&'static str, Vec<(&'static str, &'static str)>, Vec<&'static str>)>;

    /// Writes the next scripted candidate (files and deletions) into the outbox
    /// on each call and reads it back through a [`DirProposer`].
    struct Outbox {
        outbox: PathBuf,
        queue: Queue,
        inner: DirProposer,
    }

    impl Proposer for Outbox {
        fn name(&self) -> &str {
            self.inner.name()
        }
        fn propose(&mut self, champion: Option<&FitnessVector>, seed: u64) -> Vec<CandidateSpec> {
            if !self.queue.is_empty() {
                let (id, files, deletes) = self.queue.remove(0);
                write_candidate(&self.outbox, id, &files, "capability", 0.3);
                if !deletes.is_empty() {
                    put(&self.outbox.join(id).join("delete.txt"), &deletes.join("\n"));
                }
            }
            self.inner.propose(champion, seed)
        }
    }

    fn run_queue(
        fx: &Fx,
        w: &mut SelfModWorkload,
        l: &mut Lineage,
        j: &mut Journal,
        ep: &Episode,
        queue: Queue,
        shadow_runs: u32,
    ) -> crate::runner::RunReport {
        let outbox = fx.root.join("outbox");
        let n = queue.len() as u32;
        let proposer = Outbox { outbox: outbox.clone(), queue, inner: DirProposer::new(&outbox, "agent", fx.store(), fx.root.join("base")) };
        let at = Attestor::new("selfmod-harness", b"key".to_vec());
        let policy = RunnerPolicy {
            max_cycles: n,
            max_consecutive_refusals: n + 1,
            shadow_runs,
            supervision: SupervisionPolicy { window: 3, ..SupervisionPolicy::default() },
            ..RunnerPolicy::default()
        };
        Runner::new(policy, ep, &at, suite(), 1).with_proposer(Box::new(proposer)).run(l, j, w)
    }

    #[test]
    fn rebase_overlays_the_candidate_on_the_champions_genes() {
        let fx = Fx::new("rebase", EVAL_OK);
        put(&fx.root.join("base/src/old.txt"), "o\n");
        let mut w = fx.workload();
        let champion = vec![gene_for("src/lib.txt", &sha(&fx, "ok\n1\n")), gene_for("src/added.txt", &sha(&fx, "a\n"))];
        let rb = |w: &mut SelfModWorkload, genome: Vec<Locus>| w.rebase(&champion, CandidateSpec::new("c", genome));

        // A different file: both edits are in the effective genome, sorted.
        let out = rb(&mut w, vec![gene_for("src/more.txt", &sha(&fx, "m\n"))]).unwrap();
        let got: Vec<String> = names(&out.genome).into_iter().map(|(p, _)| p).collect();
        assert_eq!(got, ["src/added.txt", "src/lib.txt", "src/more.txt"]);

        // The same file: the candidate overrides the champion's gene.
        let out = rb(&mut w, vec![gene_for("src/lib.txt", &sha(&fx, "ok\n1\n2\n"))]).unwrap();
        assert_eq!(names(&out.genome).len(), 2);
        let want = sha(&fx, "ok\n1\n2\n");
        assert!(out.genome.iter().any(|g| matches!(g, Locus::Gene { name, sha256, .. } if name == "src/lib.txt" && *sha256 == want)));

        // Back to the base bytes: the champion's gene is dropped, not duplicated.
        let out = rb(&mut w, vec![gene_for("src/lib.txt", &sha(&fx, "ok\nb\n"))]).unwrap();
        assert_eq!(names(&out.genome), [("src/added.txt".to_string(), false)]);

        // Delete a gene the champion added: it vanishes, leaving no tombstone.
        let out = rb(&mut w, vec![tombstone_for("src/added.txt")]).unwrap();
        assert_eq!(names(&out.genome), [("src/lib.txt".to_string(), false)]);

        // Delete a base file: a tombstone.
        let out = rb(&mut w, vec![tombstone_for("src/old.txt")]).unwrap();
        assert_eq!(names(&out.genome).iter().filter(|(_, t)| *t).count(), 1);
        assert_eq!(names(&out.genome).len(), 3);

        // Delete a base file the champion rewrote: the rewrite is replaced by a tombstone.
        let out = rb(&mut w, vec![tombstone_for("src/lib.txt")]).unwrap();
        assert!(names(&out.genome).contains(&("src/lib.txt".to_string(), true)));

        // Nothing to delete, deleted twice, and no change at all.
        assert!(rb(&mut w, vec![tombstone_for("src/nope.txt")]).unwrap_err().contains("cannot be deleted"));
        let gone = vec![tombstone_for("src/old.txt")];
        assert!(w.rebase(&gone, CandidateSpec::new("c", vec![tombstone_for("src/old.txt")])).unwrap_err().contains("already deleted"));
        assert!(rb(&mut w, vec![gene_for("src/lib.txt", &sha(&fx, "ok\n1\n"))]).unwrap_err().contains("changes nothing"));
        assert!(rb(&mut w, vec![]).unwrap_err().contains("changes nothing"));

        // A base-relative genome is the degenerate case: an empty champion changes nothing.
        let spec = CandidateSpec::new("c", vec![gene_for("src/lib.txt", &sha(&fx, "ok\n1\n"))]);
        let out = w.rebase(&[], spec.clone()).unwrap();
        assert_eq!(out.genome, spec.genome);
    }

    #[test]
    fn a_candidate_against_a_stale_champion_is_refused() {
        let fx = Fx::new("stale", EVAL_OK);
        let mut w = fx.workload();
        let old = vec![gene_for("src/lib.txt", &sha(&fx, "ok\n1\n"))];
        let now = vec![gene_for("src/lib.txt", &sha(&fx, "ok\n1\n2\n"))];
        let spec = CandidateSpec::new("c", vec![gene_for("src/other.txt", &sha(&fx, "x\n"))]).based_on(genome_id(&old));
        // Against the champion it was written for: fine.
        assert!(w.rebase(&old, spec.clone()).is_ok());
        // The champion advanced, though the files are disjoint: refused, not merged.
        let e = w.rebase(&now, spec).unwrap_err();
        assert!(e.contains("stale"), "{e}");
        // A candidate that states nothing is taken against the current champion.
        assert!(w.rebase(&now, CandidateSpec::new("c", vec![gene_for("src/other.txt", &sha(&fx, "x\n"))])).is_ok());
        // The id does not depend on locus order.
        let two = vec![gene_for("src/a.txt", &sha(&fx, "a\n")), gene_for("src/b.txt", &sha(&fx, "b\n"))];
        let rev: Vec<Locus> = two.iter().rev().cloned().collect();
        assert_eq!(genome_id(&two), genome_id(&rev));
        assert_ne!(genome_id(&two), genome_id(&[]));
    }

    #[test]
    fn a_tombstone_deletes_the_file_in_the_built_tree() {
        let fx = Fx::new("tomb", EVAL_SHRINK);
        put(&fx.root.join("base/src/junk.txt"), "j1\nj2\nj3\nj4\n");
        let mut w = fx.workload();
        let base = w.materialize(&CandidateSpec::new("base", vec![])).unwrap();
        let del = w.materialize(&CandidateSpec::new("del", vec![tombstone_for("src/junk.txt")])).unwrap();
        assert_ne!(base, del, "a deletion is a different artifact");
        assert!(w.materialized(&del));
        let r = w.report(&del).unwrap();
        assert!(!r.tree.join("src/junk.txt").exists() && r.tree.join("src/lib.txt").is_file());
        assert_eq!(r.files, ["src/junk.txt:deleted"]);
        assert!(w.report(&base).unwrap().tree.join("src/junk.txt").is_file(), "the base tree is untouched");
        let (fb, fd) = (w.evaluate(&base, &suite()).unwrap(), w.evaluate(&del, &suite()).unwrap());
        assert!(fd.get("capability").unwrap() > fb.get("capability").unwrap(), "a smaller tree scores higher here");
        // Deleting what is not there is a recorded failure, not an error.
        let none = w.materialize(&CandidateSpec::new("none", vec![tombstone_for("src/nope.txt")])).unwrap();
        assert!(!w.materialized(&none));
        assert!(w.report(&none).unwrap().failure.as_deref().unwrap().contains("does not exist"));
        // The genome refuses a tombstone with a content hash and a file gene with the tombstone hash.
        let bad = Locus::gene("src/junk.txt", "a".repeat(64), TOMBSTONE_SIGNATURE, Effects::default());
        assert!(change_set(&[bad]).is_err());
        let bad = Locus::gene("src/junk.txt", TOMBSTONE_SHA, FILE_SIGNATURE, Effects::default());
        assert!(change_set(&[bad]).is_err());
        assert!(changed_files(&[tombstone_for("src/junk.txt")]).is_err(), "the old entry point refuses deletions");
    }

    #[test]
    fn deleting_a_protected_or_unwritable_path_is_refused_like_writing_it() {
        let fx = Fx::new("tombprot", EVAL_OK);
        put(&fx.root.join("base/src/frozen/keep.txt"), "k\n");
        let mut w = fx.workload();
        for (p, why) in [
            ("src/frozen/keep.txt", "protected prefix"),
            ("tests/expected.txt", "outside the write set"),
            ("src/../tests/expected.txt", "refused"),
        ] {
            let spec = CandidateSpec::new("d", vec![tombstone_for(p)]);
            // The overlay lets it through (or leaves it for the check); the write set refuses it.
            let spec = w.rebase(&[], spec).unwrap_or_else(|e| panic!("`{p}`: {e}"));
            let d = w.materialize(&spec).unwrap();
            let r = w.report(&d).unwrap();
            assert!(r.refused && r.failure.as_deref().unwrap().contains(why), "`{p}`: {r:?}");
            assert!(!w.materialized(&d));
        }
    }

    #[test]
    fn protect_tests_refuses_deleting_a_file_that_holds_tests() {
        let fx = Fx::new("tombtests", EVAL_OK);
        put(&fx.root.join("base/src/m.rs"), "fn f() {}\n#[cfg(test)]\nmod t {\n    #[test]\n    fn it() {}\n}\n");
        put(&fx.root.join("base/src/plain.rs"), "fn g() {}\n");
        let mut cfg = fx.config();
        cfg.protect_tests = true;
        let mut w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        let d = w.materialize(&CandidateSpec::new("d", vec![tombstone_for("src/m.rs")])).unwrap();
        let r = w.report(&d).unwrap();
        assert!(r.refused && r.failure.as_deref().unwrap().contains("it"), "{r:?}");
        let d = w.materialize(&CandidateSpec::new("d2", vec![tombstone_for("src/plain.rs")])).unwrap();
        assert!(w.materialized(&d), "a file with no tests may go");
    }

    #[test]
    fn artifact_digests_without_a_deletion_are_the_old_ones() {
        // The formula as it was before tombstones existed, written out.
        let old = |base: &str, files: &[(&str, &str)]| {
            let mut h = Sha256::new();
            h.update(b"germline-selfmod-v1");
            absorb(&mut h, base.as_bytes());
            for (p, s) in files {
                absorb(&mut h, p.as_bytes());
                absorb(&mut h, s.as_bytes());
            }
            Digest(format!("{:x}", h.finalize()))
        };
        let (a, b) = ("a".repeat(64), "b".repeat(64));
        let files = vec![("src/z.txt".to_string(), b.clone()), ("src/a.txt".to_string(), a.clone())];
        assert_eq!(artifact_digest("base-v1", &files), old("base-v1", &[("src/a.txt", &a), ("src/z.txt", &b)]));
        let with_del = vec![("src/a.txt".to_string(), a), ("src/gone.txt".to_string(), TOMBSTONE_SHA.to_string())];
        assert_ne!(artifact_digest("base-v1", &with_del), old("base-v1", &[("src/a.txt", &"a".repeat(64)), ("src/gone.txt", TOMBSTONE_SHA)]));
    }

    #[test]
    fn promotions_stack_and_the_champion_genome_holds_both_edits() {
        let fx = Fx::new("stack", EVAL_OK);
        let mut w = fx.workload();
        let ep = ep(2);
        let mut l = genesis_lineage(&mut w, &ep);
        let mut j = Journal::open(fx.root.join("journal.jsonl")).unwrap();
        let queue: Queue = vec![
            ("c1", vec![("src/lib.txt", "ok\n1\n2\n3\n")], vec![]),
            // Changes only a different file; does not repeat c1's.
            ("c2", vec![("src/more.txt", "m1\nm2\nm3\n")], vec![]),
        ];
        let report = run_queue(&fx, &mut w, &mut l, &mut j, &ep, queue, 2);
        assert_eq!(report.promotions, 2, "{:?}", report.halt);
        let champ = l.champion().unwrap();
        let genome = champ.genome.clone().unwrap();
        let got: Vec<String> = names(&genome).into_iter().map(|(p, _)| p).collect();
        assert_eq!(got, ["src/lib.txt", "src/more.txt"], "the recorded genome is the full effective one");
        let tree = &w.report(&champ.artifact).unwrap().tree;
        assert_eq!(std::fs::read_to_string(tree.join("src/lib.txt")).unwrap().replace('\r', ""), "ok\n1\n2\n3\n");
        assert!(tree.join("src/more.txt").is_file());
        // 4 + 3 lines on top of nothing else: stacking is what scored 0.7.
        assert!((champ.heldout_fitness().unwrap().get("capability").unwrap() - 0.7).abs() < 1e-9);
        assert!(j.verify().is_ok());
        // The genome is self-contained: rebuilding it from nothing gives the champion's digest.
        let again = w.materialize(&CandidateSpec::new("again", genome)).unwrap();
        assert_eq!(again, champ.artifact);
    }

    #[test]
    fn a_genome_can_shrink_by_deleting_and_a_later_edit_keeps_the_deletion() {
        let fx = Fx::new("shrink", EVAL_SHRINK);
        put(&fx.root.join("base/src/junk.txt"), "j1\nj2\nj3\nj4\n");
        let mut w = fx.workload();
        let ep = ep(2);
        let mut l = genesis_lineage(&mut w, &ep);
        let mut j = Journal::open(fx.root.join("journal.jsonl")).unwrap();
        let queue: Queue = vec![
            ("c1-delete", vec![], vec!["src/junk.txt"]),
            ("c2-shorter", vec![("src/lib.txt", "ok\n")], vec![]),
        ];
        let report = run_queue(&fx, &mut w, &mut l, &mut j, &ep, queue, 2);
        assert_eq!(report.promotions, 2, "{:?} {:?}", report.halt, report.cycles);
        let champ = l.champion().unwrap();
        let got = names(champ.genome.as_ref().unwrap());
        assert_eq!(got, [("src/junk.txt".to_string(), true), ("src/lib.txt".to_string(), false)]);
        let tree = &w.report(&champ.artifact).unwrap().tree;
        assert!(!tree.join("src/junk.txt").exists());
        assert!((champ.heldout_fitness().unwrap().get("capability").unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn after_a_demotion_new_candidates_descend_from_the_fallback() {
        let fx = Fx::new("demote", EVAL_OK);
        let counts = fx.root.join("counts");
        std::fs::create_dir_all(&counts).unwrap();
        // A binary passes its first probe (the shadow run) and fails every later
        // one, so a promoted champion is demoted when it is supervised.
        let script = format!(
            "k=$(echo \"$1\" | cksum | cut -d' ' -f1); echo x >> {d}/$k; n=$(wc -l < {d}/$k); [ $n -le 1 ]",
            d = fwd(&counts)
        );
        let mut cfg = fx.config();
        cfg.shadow = Some(Cmd { argv: vec!["sh".into(), "-c".into(), script, "probe".into()], timeout: secs(60) });
        let mut w = SelfModWorkload::new(cfg, fx.store()).unwrap();
        let ep = ep(1);
        let mut l = genesis_lineage(&mut w, &ep);
        let genesis = l.champion().unwrap().id;
        let mut j = Journal::open(fx.root.join("journal.jsonl")).unwrap();

        let r1 = run_queue(&fx, &mut w, &mut l, &mut j, &ep, vec![("c1", vec![("src/lib.txt", "ok\n1\n2\n3\n")], vec![])], 1);
        assert!(matches!(r1.halt, Halt::Demoted { .. }), "{:?}", r1.halt);
        assert_eq!(l.champion().unwrap().id, genesis, "fell back to the genesis");

        let r2 = run_queue(&fx, &mut w, &mut l, &mut j, &ep, vec![("c2", vec![("src/more.txt", "m1\nm2\n")], vec![])], 1);
        assert!(matches!(r2.halt, Halt::Demoted { .. }), "{:?}", r2.halt);
        // c2 is the generation after c1, and it carries only its own file: it was
        // built on the fallback, not on the demoted c1.
        let c2: Vec<&Generation> = l
            .generations()
            .iter()
            .filter(|g| g.genome.as_ref().is_some_and(|gn| names(gn).iter().any(|(p, _)| p == "src/more.txt")))
            .collect();
        assert_eq!(c2.len(), 1);
        let got = names(c2[0].genome.as_ref().unwrap());
        assert_eq!(got, [("src/more.txt".to_string(), false)], "no trace of the demoted edit");
    }
}
