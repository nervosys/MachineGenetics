//! Self-modification as a [`Workload`]: candidates are edits to a source tree.
//!
//! The tree is typically the MAGE compiler's own source. A candidate is a set of
//! files that differ from a frozen base tree. `materialize` copies the base tree
//! into a scratch directory, writes the candidate's files over it, builds it and
//! runs its tests. `evaluate` then runs a fixed evaluator against the built
//! binary. Every change goes through the same gate as any other candidate
//! ([`crate::gate`]): this module adds no way around it. It makes the compiler
//! editable by the loop on the user's decision that RSI self-modification is
//! open-ended, and it keeps every edit measurable by something the edit cannot
//! touch.
//!
//! ## Genome encoding
//!
//! A genome is the set of files that differ from the base tree: one
//! [`Locus::Gene`] per file, with the relative path (forward slashes) as `name`,
//! the content's [`BlobStore`] hash as `sha256`, `"file"` as `signature`, and
//! [`Effects::Unchecked`] as `effects` (nobody has read these files and
//! declared what they do). An empty genome is the base tree unchanged.
//! [`changed_files`] refuses scalar loci, other signatures, malformed hashes and
//! duplicate paths.
//!
//! Limits of the encoding, stated plainly:
//!
//! - A candidate **cannot delete** a file. It can only add or replace.
//! - A candidate is a **complete change set against the base tree**, not against
//!   the current champion. A candidate that builds on a promoted edit must carry
//!   that edit's files again. [`DirProposer`] does not look at the champion's
//!   genome, and this module does not merge. So a long run does not stack
//!   improvements by itself: each candidate competes as a whole.
//! - Genomes carry no scalars, so the built-in undirected search (mutation and
//!   crossover over `Param` loci) has nothing to vary. Candidates come from a
//!   [`Proposer`].
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

/// The `(path, sha256)` pairs a genome names, in genome order.
///
/// Refused: any locus that is not a file gene, a malformed hash, and two genes
/// for the same path (compared without regard to case on Windows).
pub fn changed_files(genome: &[Locus]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::with_capacity(genome.len());
    let mut seen = BTreeSet::new();
    for locus in genome {
        match locus {
            Locus::Gene { name, sha256, signature, .. } => {
                if signature != FILE_SIGNATURE {
                    return Err(format!("gene `{name}` has signature `{signature}`, not `{FILE_SIGNATURE}`"));
                }
                if !is_sha_hex(sha256) {
                    return Err(format!("gene `{name}` has a malformed hash `{sha256}`"));
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

fn artifact_digest(base_identity: &str, files: &[(String, String)]) -> Digest {
    let mut sorted: Vec<&(String, String)> = files.iter().collect();
    sorted.sort();
    let mut h = Sha256::new();
    h.update(b"germline-selfmod-v1");
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

/// Run `argv` in `cwd` with the hygienic environment plus `extra`. `Err` only
/// when the process cannot be started; a non-zero exit or a timeout is `Ok`.
fn run_cmd(argv: &[String], cwd: &Path, extra: &[(String, String)], timeout: Duration) -> Result<Ran, String> {
    let (program, args) = argv.split_first().ok_or("the command is empty")?;
    let mut child = Command::new(child_program(program)?)
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
                let _ = child.kill();
                let _ = child.wait();
                timed_out = true;
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => {
                let _ = child.kill();
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

impl Workload for SelfModWorkload {
    /// Build the candidate. A write-set violation or malformed genome is `Ok`
    /// and recorded as refused, before anything is copied; so is a failed build
    /// or test. `Err` (which in the runner halts the run) only for a missing blob,
    /// an unreadable base tree, or a command that cannot be started.
    fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
        let files = match changed_files(&spec.genome).and_then(|f| check_write_set(&self.cfg, &f).map(|()| f)) {
            Ok(files) => files,
            Err(why) => {
                // Named by the genome as given, so two different refused
                // candidates do not share a report.
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
                return Ok(digest);
            }
        };
        let digest = artifact_digest(&self.cfg.base_identity, &files);
        if self.artifacts.contains_key(&digest.0) {
            return Ok(digest);
        }
        // Fail closed on a gene the store lacks, as `express` does: building
        // without it would measure a different candidate under this one's name.
        let mut blobs = Vec::with_capacity(files.len());
        for (path, sha) in &files {
            let bytes = self.store.get(sha).ok_or_else(|| {
                format!(
                    "gene `{path}` {} is not in the blob store (or fails its hash) — a candidate cannot be built without the files it names",
                    &sha[..12]
                )
            })?;
            blobs.push(bytes);
        }

        let dir = self.scratch.join(&digest.0[..16]);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("cannot clear {}: {e}", dir.display()))?;
        }
        let tree = dir.join("tree");
        copy_tree(&self.base, &tree)?;
        let tree = canon(&tree).map_err(|e| format!("cannot resolve {}: {e}", tree.display()))?;

        let mut sorted: Vec<String> = files.iter().map(|(p, s)| format!("{p}:{s}")).collect();
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
        for ((path, _), bytes) in files.iter().zip(&blobs) {
            if let Err(e) = write_gene(&tree, path, bytes) {
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
/// tree, put into the [`BlobStore`]. **A candidate is a complete change set
/// against the base tree, not against the champion**: see the module docs.
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
}

impl DirProposer {
    pub fn new(outbox: impl Into<PathBuf>, name: impl Into<String>, store: BlobStore, base_tree: impl Into<PathBuf>) -> Self {
        DirProposer { outbox: outbox.into(), name: name.into(), store, base_tree: base_tree.into(), errors: Vec::new(), limit: None }
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
        if !files_dir.is_dir() {
            return Err("no files/ directory".into());
        }
        let mut files = Vec::new();
        Self::collect(&files_dir, "", &mut files)?;
        files.sort();
        let mut genome = Vec::new();
        for (rel, path) in files {
            let bytes = std::fs::read(&path).map_err(|e| format!("cannot read {rel}: {e}"))?;
            // Unchanged files are not part of the change set.
            let base_path = rel.split('/').fold(self.base_tree.clone(), |acc, c| acc.join(c));
            let unchanged = std::fs::read(base_path).is_ok_and(|base| base == bytes);
            if unchanged {
                continue;
            }
            let sha = self.store.put(&bytes)?;
            genome.push(gene_for(&rel, &sha));
        }
        if genome.is_empty() {
            return Err("the candidate changes nothing relative to the base tree".into());
        }
        Ok(CandidateSpec::new(id, genome).with_manifest(manifest))
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
        let genome = vec![gene_for("A/x", &"0".repeat(64)), gene_for("a/X", &"0".repeat(64))];
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
}
