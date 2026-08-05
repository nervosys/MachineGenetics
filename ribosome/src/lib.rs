//! # Ribosome — the distributed, agent-operated build engine
//!
//! A ribosome reads a genetic sequence and synthesizes the protein it encodes.
//! This one reads source and synthesizes artifacts, and like its namesake it is
//! *many, identical, and concurrent*: any number of them work the same tape and
//! produce the same product.
//!
//! ## A standalone crate, on purpose
//!
//! This began inside `forge`, the MAGE package registry, which was a convenient
//! place to put it and the wrong place to leave it. A build system that ships
//! inside a package registry can only ever be that registry's build system, and
//! the central claim of [`lang`] — that no language is privileged below the
//! planner — is not credible from a crate that depends on one language's
//! compiler.
//!
//! So the dependency list is the specification: `serde`, `serde_json`, `sha2`,
//! `ed25519-dalek`. Nothing MAGE, nothing registry, no compiler. The engine can
//! be vendored into an unrelated project and will build C, Rust, or a language
//! nobody here has heard of without acquiring MAGE along the way. `forge`
//! depends on *this*, not the reverse, and `germline` drives it through the same
//! public API any other caller would use — which means that API is exercised by
//! a real consumer rather than only by its own tests.
//!
//! ## Why this is not another Bazel
//!
//! Every correct build system answers one question: **when may a previous result
//! be reused?** Bazel answers it with *hermeticity by convention* — declare your
//! inputs, promise not to read anything else, and it hashes what you declared.
//! The promise is unenforced, so the failure mode is silent: an action reads the
//! clock or an ambient file, the key does not change, and a stale artifact is
//! served. Every large Bazel deployment grows a folklore of `--nocache_test_results`
//! and repro-only CI lanes to cope with it.
//!
//! MAGE can answer it *structurally* instead, because two properties already hold
//! and are measured (`MEASUREMENTS.md` §2 "Determinism"):
//!
//! 1. **ABL artifacts are byte-stable.** The same spec builds to byte-identical
//!    bytes across runs and machines. Content hashes are exact identity, not a
//!    proxy for it.
//! 2. **Construction is tool-mediated.** An agent submits a *spec* and the
//!    compiler constructs the artifact ([`ARCHITECTURE.md`]). There is no
//!    ambient-state escape hatch to forget to declare, because the agent never
//!    hands over a command line — it hands over data.
//!
//! So a cache key here is a statement about identity rather than a bet on
//! discipline. That is the whole design: everything else in this module follows
//! from taking it seriously.
//!
//! ## Arbitrary languages, without pretending
//!
//! Both properties above are about *MAGE's* pipeline, and neither survives
//! contact with `gcc`. A build engine that only builds its own language is a toy,
//! so [`lang`] admits foreign toolchains — and admits, in the key itself, that
//! they are weaker.
//!
//! The mechanism is [`lang::Hermeticity`]: `Structural` (byte-stable by
//! construction — measured, not assumed), `Pinned` (the compiler binary is
//! identified by content digest), `Declared` (named but unverified). A pinned
//! toolchain's digest goes into `Action::tool` and therefore into the key, so two
//! machines' differently-patched `gcc-13.2.0` cannot collide. A declared one is
//! marked `+unpinned`, and [`cas::Store::open_shared`] refuses to publish its
//! claims — the result is still built and still cached locally, but never offered
//! to another machine.
//!
//! This is the honest version of what Bazel calls hermeticity. Not a promise the
//! build makes and cannot keep, but a property the key can express, degrade to,
//! and be audited for. Nothing below [`lang`] mentions any language, MAGE
//! included.
//!
//! ## Shape
//!
//! | Module | Role |
//! |---|---|
//! | [`graph`] | the action DAG — dependencies, waves, critical path |
//! | [`key`] | deterministic action keys (SHA-256 over a canonical encoding) |
//! | [`cas`] | content-addressed store + action cache |
//! | [`lang`] | languages, toolchains, and hermeticity tiers |
//! | [`exec`] | the [`exec::Executor`] seam: local, pooled, remote |
//! | [`subprocess`] | sandboxed process execution for foreign tools |
//! | [`heal`] | failure classification and repair — the self-healing layer |
//! | [`sched`] | the scheduler that ties them together |
//!
//! ## Hardware-agnostic by construction
//!
//! Actions declare a [`Platform`] *requirement*; executors advertise what they
//! *satisfy*. The scheduler matches the two and never assumes a CPU. An
//! accelerator is an open string — the same vocabulary MAGE's own backend
//! registry uses (`prototype/src/backends.rs`), by convention rather than by
//! dependency — so adding a new device class to the fleet is a registration, not
//! a code change.
//!
//! Crucially the *accelerator is part of the action key* only when the action
//! declares it. A pure source-to-ABL lowering is device-independent and its cache
//! entry is shared by every worker in the fleet; a kernel autotune is not, and
//! its entry is per-device. That distinction is what makes a heterogeneous fleet
//! share a cache safely.
//!
//! ## Agent-operated, one to many
//!
//! Every surface here is data in and data out: [`graph::ActionGraph::to_json`],
//! [`sched::BuildReport::json`], and a [`sched::Scheduler::plan`] that answers
//! "what would you do?" without doing it — the same no-exec introspection
//! discipline as `--describe=abl`. An agent can therefore inspect, predict, and
//! audit a build without running one.
//!
//! Many agents drive one build safely because the unit of coordination is the
//! *action key*, not a lock: two agents that submit the same action submit the
//! same key and the second is a cache hit. Genuinely conflicting work — two
//! agents rewriting the same target — is the case the existing lease manager and
//! consensus engine already handle (`prototype/src/lease.rs`, `consensus.rs`),
//! and the scheduler is written to be driven by them rather than to replace them.

pub mod cas;
pub mod exec;
pub mod graph;
pub mod heal;
pub mod key;
pub mod lang;
pub mod mac;
pub mod manifest;
pub mod provenance;
pub mod remote;
pub mod sched;
pub mod subprocess;
#[cfg(feature = "tls")]
pub mod tls;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A SHA-256 content digest, lowercase hex.
///
/// Newtyped rather than a bare `String` because digests and logical paths are
/// both strings and confusing them is the classic build-system bug.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Digest(pub String);

impl Digest {
    /// Digest of a byte slice.
    pub fn of(bytes: &[u8]) -> Self {
        use sha2::{Digest as _, Sha256};
        let mut h = Sha256::new();
        h.update(bytes);
        Digest(format!("{:x}", h.finalize()))
    }

    /// The short form used in logs and agent-facing summaries.
    pub fn short(&self) -> &str {
        &self.0[..self.0.len().min(12)]
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where an action may run.
///
/// `accelerator` is `None` for device-independent work — the common case, and the
/// one that lets a heterogeneous fleet share cache entries. `Some(dev)` pins the
/// action to a device class and, via [`key::action_key`], to its own cache line.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Platform {
    pub os: String,
    pub arch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accelerator: Option<String>,
}

impl Platform {
    /// A device-independent platform — the default for compilation actions.
    pub fn any() -> Self {
        Platform { os: "any".into(), arch: "any".into(), accelerator: None }
    }

    /// A platform pinned to a host triple, still device-independent.
    pub fn host(os: &str, arch: &str) -> Self {
        Platform { os: os.into(), arch: arch.into(), accelerator: None }
    }

    /// Pin this platform to an accelerator class (`cuda`, `rocm`, `metal`, …).
    pub fn with_accelerator(mut self, dev: &str) -> Self {
        self.accelerator = Some(dev.to_string());
        self
    }

    /// Does an executor advertising `self` satisfy a `required` platform?
    ///
    /// `"any"` on the requirement side is a wildcard, so a device-independent
    /// action runs anywhere. An accelerator requirement must match exactly: we
    /// would rather leave work unscheduled than silently run a CUDA action on a
    /// CPU and cache the result under a key that claims otherwise.
    pub fn satisfies(&self, required: &Platform) -> bool {
        let os_ok = required.os == "any" || required.os == self.os;
        let arch_ok = required.arch == "any" || required.arch == self.arch;
        let dev_ok = match (&required.accelerator, &self.accelerator) {
            (None, _) => true,
            (Some(r), Some(h)) => r == h,
            (Some(_), None) => false,
        };
        os_ok && arch_ok && dev_ok
    }

    /// Stable string form, used in action keys and agent output.
    pub fn tag(&self) -> String {
        match &self.accelerator {
            Some(d) => format!("{}-{}-{}", self.os, self.arch, d),
            None => format!("{}-{}", self.os, self.arch),
        }
    }
}

impl Default for Platform {
    fn default() -> Self {
        Platform::any()
    }
}

/// One declared input: a logical path plus the digest of its contents.
///
/// The digest, not the path, participates in the action key. Moving a file
/// without changing it must not invalidate a cache entry, and changing it
/// without moving it must.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub path: String,
    pub digest: Digest,
}

impl Input {
    pub fn new(path: impl Into<String>, digest: Digest) -> Self {
        Input { path: path.into(), digest }
    }
}

/// A unit of work: a tool applied to declared inputs, producing declared outputs.
///
/// An action is *data*. It is submitted, hashed, cached, shipped to a remote
/// worker, and replayed — none of which is possible if it is a shell string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    /// Human/agent-facing label, e.g. `lower:model.mg`. Not part of the key —
    /// renaming a target must not invalidate its cache entry.
    pub name: String,
    /// Tool identity *including version* (`mage-parse@0.2.0`). Part of the key:
    /// a new compiler is a new result, and that is the whole point.
    pub tool: String,
    pub args: Vec<String>,
    pub inputs: Vec<Input>,
    /// Logical output paths this action promises to produce.
    pub outputs: Vec<String>,
    /// Allowlisted environment. Anything not listed here is not visible to the
    /// action, so it cannot influence the result without appearing in the key.
    pub env: BTreeMap<String, String>,
    pub platform: Platform,
    /// Scheduling hint only — never part of the key.
    pub cost: u64,
}

impl Action {
    /// A device-independent action with no env and unit cost.
    pub fn new(name: impl Into<String>, tool: impl Into<String>) -> Self {
        Action {
            name: name.into(),
            tool: tool.into(),
            args: Vec::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            env: BTreeMap::new(),
            platform: Platform::any(),
            cost: 1,
        }
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.args.push(a.into());
        self
    }

    pub fn input(mut self, path: impl Into<String>, digest: Digest) -> Self {
        self.inputs.push(Input::new(path, digest));
        self
    }

    pub fn output(mut self, path: impl Into<String>) -> Self {
        self.outputs.push(path.into());
        self
    }

    pub fn env(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.env.insert(k.into(), v.into());
        self
    }

    pub fn platform(mut self, p: Platform) -> Self {
        self.platform = p;
        self
    }

    pub fn cost(mut self, c: u64) -> Self {
        self.cost = c;
        self
    }

    /// This action's cache key. See [`key::action_key`].
    pub fn key(&self) -> Digest {
        key::action_key(self)
    }
}
