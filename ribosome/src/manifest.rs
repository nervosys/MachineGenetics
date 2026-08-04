//! A build file, and the resolution that turns it into something buildable.
//!
//! [`lang`](super::lang) describes languages; this describes *a project*. It is
//! the smallest layer that makes the engine usable without writing Rust: a JSON
//! document naming targets and toolchains, resolved into a [`Registry`], a set
//! of [`Target`]s, source bytes read from disk, and the executable allowlist a
//! [`SubprocessExecutor`](super::subprocess::SubprocessExecutor) needs.
//!
//! ## Why JSON, and why so little of it
//!
//! Every build system eventually grows a configuration language, and most of
//! them regret it — Starlark, Groovy, and CMake's macro layer are all answers to
//! "the config needs to compute things" that ended up needing their own
//! debuggers. This one refuses the question. A manifest is *data*: no
//! conditionals, no functions, no includes, no globs.
//!
//! That is not minimalism for its own sake. The engine's whole premise is that
//! an action is data an agent submits rather than a command line it composes
//! ([`super`]), and a manifest that could compute would be a second place for
//! ambient state to enter — the thing the action key exists to exclude. If a
//! project needs generated targets, the right move is to generate the JSON with
//! whatever language you like and hand it over. Then the generation is visible,
//! and the build is still reproducible from the artifact it produced.
//!
//! No globs, specifically, because a glob makes the source list depend on the
//! filesystem at the moment of the build. Two workers with slightly different
//! checkouts would expand it differently, agree on an action key, and disagree
//! on the answer — the exact failure `lang`'s hermeticity tiers exist to prevent,
//! reintroduced one convenience feature above them.
//!
//! ## Pinning is an operation, not a literal
//!
//! A manifest cannot state a toolchain digest, because a digest is a property of
//! *this machine's* executable and a checked-in one would be a claim about
//! someone else's. Instead a toolchain says `"pin": true` and resolution hashes
//! the executable it points at. That is the honest form of the upgrade path in
//! [`lang::Hermeticity`](super::lang::Hermeticity): the operator says "verify
//! this", and the verification happens where the binary actually is.

use super::lang::{Registry, Target, Toolchain};
use super::Digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One toolchain override: which executable a language should actually invoke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainSpec {
    /// Path to the executable. Resolved relative to the manifest's directory if
    /// relative, so a vendored toolchain travels with the project.
    pub path: String,
    /// Version string. Recorded in the tool id and therefore in every action
    /// key, so changing it is a rebuild — which is correct, since a different
    /// compiler version is a different result.
    #[serde(default)]
    pub version: Option<String>,
    /// Hash the executable and pin the toolchain to its digest, making this
    /// language's results shareable across machines. Off by default: pinning
    /// silently would make a claim the operator did not ask for.
    #[serde(default)]
    pub pin: bool,
    /// Arguments prepended to every invocation — `sh -c`, `cmd /C`, a wrapper.
    #[serde(default)]
    pub prefix_args: Vec<String>,
}

/// One target: sources in a language, plus artifacts other targets produce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSpec {
    pub name: String,
    pub language: String,
    pub sources: Vec<String>,
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub args: Vec<String>,
}

/// A whole build file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub toolchains: BTreeMap<String, ToolchainSpec>,
    #[serde(default)]
    pub targets: Vec<TargetSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Parse(String),
    Io { path: String, reason: String },
    /// A toolchain override names a language the registry does not have.
    UnknownLanguage(String),
    /// A target names no sources, or the manifest names no targets. Caught here
    /// so an empty build is an error rather than a silent success.
    Empty(String),
    /// Two targets share a name. Their artifacts would collide, and the graph
    /// would reject it later — better to say so while the manifest is still the
    /// thing being talked about.
    DuplicateTarget(String),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Parse(m) => write!(f, "malformed manifest: {m}"),
            ManifestError::Io { path, reason } => write!(f, "cannot read `{path}`: {reason}"),
            ManifestError::UnknownLanguage(l) => {
                write!(f, "toolchain override for unregistered language `{l}`")
            }
            ManifestError::Empty(what) => write!(f, "{what} is empty"),
            ManifestError::DuplicateTarget(n) => write!(f, "duplicate target `{n}`"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// A manifest, resolved against a real filesystem.
#[derive(Debug)]
pub struct Resolved {
    pub registry: Registry,
    pub targets: Vec<Target>,
    /// Source path -> contents. The caller stores these in the CAS; digests in
    /// `targets` already match.
    pub sources: BTreeMap<String, Vec<u8>>,
    /// Tool id -> (executable, prefix args), for the subprocess allowlist. Only
    /// languages the manifest overrode appear: a builtin nobody pointed at a
    /// real binary is unrunnable, and being unable to *run* it is better than
    /// guessing at `cc` on the `PATH`.
    pub programs: BTreeMap<String, (PathBuf, Vec<String>)>,
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Self, ManifestError> {
        // Strip a UTF-8 byte-order mark. Windows PowerShell's `Set-Content`
        // writes one by default and `ConvertTo-Json | Set-Content` is the most
        // obvious way to generate a manifest on that platform — so without this
        // the common case fails with "expected value at line 1 column 1", an
        // error that names an invisible byte and helps nobody. JSON has no BOM
        // in its grammar; ignoring one costs nothing and is not ambiguous.
        let json = json.strip_prefix('\u{feff}').unwrap_or(json);
        serde_json::from_str(json).map_err(|e| ManifestError::Parse(e.to_string()))
    }

    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let raw = std::fs::read_to_string(path).map_err(|e| ManifestError::Io {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        Manifest::parse(&raw)
    }

    /// Resolve against `root`: read sources, hash toolchains, build the registry.
    pub fn resolve(&self, root: &Path) -> Result<Resolved, ManifestError> {
        if self.targets.is_empty() {
            return Err(ManifestError::Empty("manifest `targets`".into()));
        }

        let mut registry = Registry::with_builtins();
        let mut programs = BTreeMap::new();

        for (language, spec) in &self.toolchains {
            let lang = registry
                .get(language)
                .ok_or_else(|| ManifestError::UnknownLanguage(language.clone()))?
                .clone();

            let exe = {
                let p = PathBuf::from(&spec.path);
                if p.is_absolute() {
                    p
                } else {
                    root.join(p)
                }
            };

            // Name the tool after the executable, so a report saying `clang@18`
            // means clang actually ran — not that a language called "c" did.
            let tool = exe
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| language.clone());
            let version = spec.version.clone().unwrap_or_else(|| "unknown".into());

            let toolchain = if spec.pin {
                let bytes = std::fs::read(&exe).map_err(|e| ManifestError::Io {
                    path: exe.display().to_string(),
                    reason: format!("cannot hash toolchain: {e}"),
                })?;
                Toolchain::pinned(tool, version, Digest::of(&bytes))
            } else {
                Toolchain::declared(tool, version)
            };

            let id = toolchain.tool_id();
            registry.register(lang.with_toolchain(toolchain));
            programs.insert(id, (exe, spec.prefix_args.clone()));
        }

        let mut targets = Vec::new();
        let mut sources = BTreeMap::new();
        let mut seen = BTreeMap::new();

        for spec in &self.targets {
            if let Some(_prev) = seen.insert(spec.name.clone(), ()) {
                return Err(ManifestError::DuplicateTarget(spec.name.clone()));
            }
            if spec.sources.is_empty() {
                return Err(ManifestError::Empty(format!("target `{}` sources", spec.name)));
            }

            let mut t = Target::new(&spec.name, &spec.language);
            for rel in &spec.sources {
                let full = root.join(rel);
                let bytes = std::fs::read(&full).map_err(|e| ManifestError::Io {
                    path: full.display().to_string(),
                    reason: e.to_string(),
                })?;
                // The logical path is what the manifest said, not where it
                // happens to live on this machine. An absolute path in a key
                // would make every checkout a cache miss.
                t = t.source(rel.clone(), Digest::of(&bytes));
                sources.insert(rel.clone(), bytes);
            }
            for d in &spec.deps {
                t = t.dep(d.clone());
            }
            for (k, v) in &spec.env {
                t = t.env(k.clone(), v.clone());
            }
            for a in &spec.args {
                t = t.arg(a.clone());
            }
            targets.push(t);
        }

        Ok(Resolved { registry, targets, sources, programs })
    }
}

impl Resolved {
    /// Tool ids the plan needs but the manifest never pointed at an executable.
    ///
    /// The actionable form of "this will not run": a builtin language definition
    /// knows the *flags* `cc` takes, not where `cc` is, and guessing at the
    /// `PATH` would reintroduce exactly the ambient-state dependency the key is
    /// designed to exclude.
    pub fn missing_programs(&self, plan: &super::lang::Plan) -> Vec<String> {
        let mut v: Vec<String> = plan
            .actions
            .iter()
            .map(|a| a.tool.clone())
            .filter(|t| !self.programs.contains_key(t))
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ribosome-manifest-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, bytes).unwrap();
    }

    #[test]
    fn a_minimal_manifest_resolves_to_targets_and_source_bytes() {
        let root = tmp("minimal");
        write(&root, "src/a.c", b"int a;");
        write(&root, "src/b.c", b"int b;");

        let m = Manifest::parse(
            r#"{"targets":[{"name":"app","language":"c","sources":["src/a.c","src/b.c"]}]}"#,
        )
        .unwrap();
        let r = m.resolve(&root).unwrap();

        assert_eq!(r.targets.len(), 1);
        assert_eq!(r.targets[0].sources[0].digest, Digest::of(b"int a;"));
        assert_eq!(r.sources["src/b.c"], b"int b;");
        // The logical path is what the manifest said — an absolute path here
        // would make every checkout a cache miss.
        assert_eq!(r.targets[0].sources[0].path, "src/a.c");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn pinning_hashes_the_actual_executable_on_this_machine() {
        let root = tmp("pin");
        write(&root, "a.c", b"int a;");
        write(&root, "tools/cc", b"#!/bin/sh\nexec true\n");

        let m = Manifest::parse(
            r#"{
                 "toolchains": {"c": {"path": "tools/cc", "version": "1.0", "pin": true}},
                 "targets": [{"name":"app","language":"c","sources":["a.c"]}]
               }"#,
        )
        .unwrap();
        let r = m.resolve(&root).unwrap();

        let plan = r.registry.plan(&r.targets[0]).unwrap();
        assert!(plan.remote_cacheable(), "a hashed toolchain earns the shared cache");
        // Named after the executable, not the language: a report saying `cc@1.0`
        // must mean cc ran.
        assert!(plan.actions[0].tool.starts_with("cc@1.0+sha256-"), "{}", plan.actions[0].tool);
        assert_eq!(
            plan.actions[0].tool,
            format!("cc@1.0+sha256-{}", Digest::of(b"#!/bin/sh\nexec true\n").short())
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn without_pin_the_same_toolchain_is_declared_and_local_only() {
        let root = tmp("nopin");
        write(&root, "a.c", b"int a;");
        write(&root, "tools/cc", b"binary");

        let m = Manifest::parse(
            r#"{
                 "toolchains": {"c": {"path": "tools/cc", "version": "1.0"}},
                 "targets": [{"name":"app","language":"c","sources":["a.c"]}]
               }"#,
        )
        .unwrap();
        let r = m.resolve(&root).unwrap();
        let plan = r.registry.plan(&r.targets[0]).unwrap();

        assert!(!plan.remote_cacheable());
        assert_eq!(plan.actions[0].tool, "cc@1.0+unpinned");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn changing_the_toolchain_binary_changes_every_key() {
        let root = tmp("rehash");
        write(&root, "a.c", b"int a;");
        write(&root, "tools/cc", b"version one");

        let json = r#"{
            "toolchains": {"c": {"path": "tools/cc", "pin": true}},
            "targets": [{"name":"app","language":"c","sources":["a.c"]}]
        }"#;
        let m = Manifest::parse(json).unwrap();
        let before = {
            let r = m.resolve(&root).unwrap();
            r.registry.plan(&r.targets[0]).unwrap().actions[0].key()
        };

        // Someone upgrades the compiler in place. The manifest is byte-identical.
        write(&root, "tools/cc", b"version two");
        let after = {
            let r = m.resolve(&root).unwrap();
            r.registry.plan(&r.targets[0]).unwrap().actions[0].key()
        };

        assert_ne!(before, after, "an in-place compiler upgrade must invalidate the cache");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_language_with_no_executable_is_reported_rather_than_guessed() {
        let root = tmp("missing");
        write(&root, "a.c", b"int a;");

        let m = Manifest::parse(r#"{"targets":[{"name":"app","language":"c","sources":["a.c"]}]}"#)
            .unwrap();
        let r = m.resolve(&root).unwrap();
        let plan = r.registry.plan(&r.targets[0]).unwrap();

        // Resolution succeeds — planning is still useful without a compiler.
        // Running is not, and this says so instead of reaching for `PATH`.
        assert_eq!(r.missing_programs(&plan), vec!["cc@unknown+unpinned"]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cross_target_dependencies_survive_resolution() {
        let root = tmp("deps");
        write(&root, "util.c", b"void u(){}");
        write(&root, "main.rs", b"fn main(){}");

        let m = Manifest::parse(
            r#"{"targets":[
                 {"name":"libutil.a","language":"c","sources":["util.c"]},
                 {"name":"app","language":"rust","sources":["main.rs"],"deps":["libutil.a"]}
               ]}"#,
        )
        .unwrap();
        let r = m.resolve(&root).unwrap();
        let plan = r.registry.plan_all(&r.targets).unwrap();
        let g = plan.graph().unwrap();

        let order = g.topological_order().unwrap();
        let names: Vec<&str> = order.iter().map(|&i| g.actions[i].name.as_str()).collect();
        assert!(
            names.iter().position(|n| *n == "link:libutil.a").unwrap()
                < names.iter().position(|n| *n == "build:app").unwrap()
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn env_and_extra_args_reach_the_plan_and_the_key() {
        let root = tmp("env");
        write(&root, "a.c", b"int a;");

        let plain = Manifest::parse(
            r#"{"targets":[{"name":"app","language":"c","sources":["a.c"]}]}"#,
        )
        .unwrap();
        let tuned = Manifest::parse(
            r#"{"targets":[{"name":"app","language":"c","sources":["a.c"],
                 "env":{"SOURCE_DATE_EPOCH":"0"},"args":["-lm"]}]}"#,
        )
        .unwrap();

        let a = plain.resolve(&root).unwrap();
        let b = tuned.resolve(&root).unwrap();
        let pa = a.registry.plan(&a.targets[0]).unwrap();
        let pb = b.registry.plan(&b.targets[0]).unwrap();

        assert_ne!(pa.actions[0].key(), pb.actions[0].key());
        assert!(pb.actions.last().unwrap().args.contains(&"-lm".to_string()));

        let _ = std::fs::remove_dir_all(&root);
    }

    // ── refusals ─────────────────────────────────────────────────────────────

    #[test]
    fn a_manifest_with_no_targets_is_an_error_not_a_silent_success() {
        let root = tmp("empty");
        let m = Manifest::parse(r#"{"targets":[]}"#).unwrap();
        assert!(matches!(m.resolve(&root), Err(ManifestError::Empty(_))));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn duplicate_target_names_are_caught_while_the_manifest_is_the_subject() {
        let root = tmp("dup");
        write(&root, "a.c", b"x");
        let m = Manifest::parse(
            r#"{"targets":[
                 {"name":"app","language":"c","sources":["a.c"]},
                 {"name":"app","language":"c","sources":["a.c"]}]}"#,
        )
        .unwrap();
        assert!(matches!(m.resolve(&root), Err(ManifestError::DuplicateTarget(_))));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_source_names_the_file_it_could_not_read() {
        let root = tmp("nofile");
        let m = Manifest::parse(
            r#"{"targets":[{"name":"app","language":"c","sources":["nope.c"]}]}"#,
        )
        .unwrap();
        match m.resolve(&root) {
            Err(ManifestError::Io { path, .. }) => assert!(path.ends_with("nope.c"), "{path}"),
            other => panic!("expected an Io error naming the file: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_toolchain_override_for_an_unknown_language_is_refused() {
        let root = tmp("badlang");
        let m = Manifest::parse(
            r#"{"toolchains":{"cobol":{"path":"cobc"}},
                "targets":[{"name":"x","language":"c","sources":["a.c"]}]}"#,
        )
        .unwrap();
        assert!(matches!(m.resolve(&root), Err(ManifestError::UnknownLanguage(_))));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn malformed_json_fails_at_parse_with_a_reason() {
        assert!(matches!(Manifest::parse("{ not json"), Err(ManifestError::Parse(_))));
    }

    #[test]
    fn a_utf8_bom_does_not_break_a_manifest() {
        // `ConvertTo-Json | Set-Content` on Windows writes one. Rejecting it
        // produced "expected value at line 1 column 1" for a byte you cannot
        // see, which is how a first-run experience becomes a bug report.
        let body = r#"{"targets":[{"name":"a","language":"c","sources":["a.c"]}]}"#;
        let with_bom = format!("\u{feff}{body}");
        assert_eq!(Manifest::parse(&with_bom).unwrap(), Manifest::parse(body).unwrap());
    }

    #[test]
    fn a_manifest_round_trips_through_serde() {
        // Agents generate these; the type must be writable as well as readable.
        let m = Manifest::parse(
            r#"{"toolchains":{"c":{"path":"/usr/bin/cc","version":"13","pin":true,
                 "prefix_args":["-fno-common"]}},
                "targets":[{"name":"app","language":"c","sources":["a.c"],"deps":["libx.a"]}]}"#,
        )
        .unwrap();
        let round = Manifest::parse(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(m, round);
    }
}
