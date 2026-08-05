//! Arbitrary languages: toolchains, rules, and the honest limits of caching them.
//!
//! The engine below this module is already language-neutral — an [`Action`] is a
//! tool, some args, declared inputs, declared outputs. Nothing in [`super::key`],
//! [`super::cas`], [`super::graph`], or [`super::sched`] mentions MAGE. What was
//! missing is the layer that turns *"here is a target written in C"* into that
//! action graph, and — more importantly — the layer that is honest about what a
//! cache key means once the tool is no longer ours.
//!
//! ## The problem foreign languages actually create
//!
//! [`super`]'s module docs claim hermeticity is *structural* for MAGE: ABL output
//! is byte-stable and construction is tool-mediated, so a content hash is exact
//! identity rather than a bet on discipline. That claim does not survive contact
//! with `gcc`.
//!
//! A foreign toolchain breaks it in a specific way. `Action::tool` is a string,
//! and `"gcc@13"` on two workers is not evidence that the same compiler ran. One
//! machine's `gcc-13.2.0` is patched by its distribution; another's embeds a build
//! ID in `__DATE__`; a third resolves a different `libc` header. The action keys
//! agree, the outputs do not, and a shared cache serves one machine's artifact to
//! another. That is the single worst failure a build system can have, because it
//! is silent and it is *more* likely the better your cache hit rate is.
//!
//! Pretending otherwise is the standard mistake, so this module refuses to. It
//! makes the strength of the claim an explicit, per-toolchain, *keyed* value:
//!
//! | [`Hermeticity`] | Means | Remote cache |
//! |---|---|---|
//! | [`Structural`](Hermeticity::Structural) | output is byte-stable by construction (MAGE/ABL) | yes |
//! | [`Pinned`](Hermeticity::Pinned) | the toolchain binary is identified by content digest | yes |
//! | [`Declared`](Hermeticity::Declared) | the toolchain is named, not verified | **no** |
//!
//! Two consequences, both enforced here rather than documented and hoped for:
//!
//! 1. A pinned toolchain's **digest is part of the action key** (via
//!    [`Toolchain::tool_id`]), so two different `gcc-13.2.0` binaries cannot
//!    collide in the cache — they are different tools, and the key says so.
//! 2. A `Declared` toolchain yields a plan that reports
//!    [`Plan::remote_cacheable`] as `false`. Its results are still worth caching
//!    *locally*, where "same machine, same binary" is a reasonable assumption;
//!    they are not worth shipping to a fleet, where it is not.
//!
//! This is the useful version of what Bazel calls hermeticity: not a promise the
//! build makes, but a property the key can express, degrade to, and be audited
//! for.
//!
//! ## Granularity is a language property, not a convention
//!
//! C compiles per translation unit and links; Rust and Go compile a whole crate
//! or package at once. Modelling both as "one action per file" would be wrong for
//! half the languages and would produce a graph that does not match what the
//! compiler actually does — so [`Granularity`] is declared, and the planner emits
//! the shape the toolchain really has.
//!
//! ## What this is not
//!
//! It is not a package manager and does not resolve external dependencies. A
//! dependency here is a logical artifact produced by another target in the same
//! graph. Fetching third-party code is a separate concern with its own trust
//! problem, and conflating the two is how build systems become unauditable.

use super::graph::{ActionGraph, GraphError};
use super::{Action, Digest, Input, Platform};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The strength of the identity claim a cache key makes for a given toolchain.
///
/// Ordered strongest-first, so derived `Ord` puts `Structural` lowest and
/// `Declared` highest and `max()` over a set of toolchains yields the *weakest*.
/// That is the only correct way to combine them: a mixed-language plan is exactly
/// as trustworthy as its least trustworthy step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hermeticity {
    /// Output is byte-stable by construction and the tool is ours. A content
    /// hash *is* identity. Only MAGE's own pipeline qualifies, and it qualifies
    /// because it was measured (`MEASUREMENTS.md` §2), not because it is ours.
    Structural,
    /// The toolchain binary is identified by content digest. Two workers running
    /// this tool are demonstrably running the same bytes.
    Pinned,
    /// The toolchain is named but unverified — `gcc@13`, `python@3.12`. Honest
    /// about being a convention rather than a fact.
    Declared,
}

impl Hermeticity {
    /// May results from this toolchain be shared across machines?
    ///
    /// `Declared` is deliberately excluded. The failure it admits is a silent
    /// wrong answer, and a cache miss is cheaper than that by an unbounded
    /// margin.
    pub fn remote_cacheable(self) -> bool {
        matches!(self, Hermeticity::Structural | Hermeticity::Pinned)
    }

    /// Why, in one line, for agent-facing output.
    pub fn reason(self) -> &'static str {
        match self {
            Hermeticity::Structural => "output is byte-stable by construction",
            Hermeticity::Pinned => "toolchain identified by content digest",
            Hermeticity::Declared => "toolchain named but unverified; local cache only",
        }
    }
}

/// A compiler, interpreter, or other program a language's rules invoke.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toolchain {
    /// Program identity, matching a registered [`Program`](super::subprocess::Program).
    pub tool: String,
    pub version: String,
    /// Content digest of the executable, when it has been measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<Digest>,
    pub hermeticity: Hermeticity,
}

impl Toolchain {
    /// A named, unverified toolchain — the weakest and most common case.
    pub fn declared(tool: impl Into<String>, version: impl Into<String>) -> Self {
        Toolchain {
            tool: tool.into(),
            version: version.into(),
            digest: None,
            hermeticity: Hermeticity::Declared,
        }
    }

    /// A toolchain pinned to the digest of its executable.
    pub fn pinned(tool: impl Into<String>, version: impl Into<String>, digest: Digest) -> Self {
        Toolchain {
            tool: tool.into(),
            version: version.into(),
            digest: Some(digest),
            hermeticity: Hermeticity::Pinned,
        }
    }

    /// A toolchain whose output is byte-stable by construction. Reserved for
    /// pipelines where that has been *measured*, not assumed.
    pub fn structural(tool: impl Into<String>, version: impl Into<String>) -> Self {
        Toolchain {
            tool: tool.into(),
            version: version.into(),
            digest: None,
            hermeticity: Hermeticity::Structural,
        }
    }

    /// Pin an already-declared toolchain by measuring its executable.
    ///
    /// The upgrade path: a fleet starts `Declared`, an operator or agent hashes
    /// the binaries, and the same build becomes remote-cacheable. Every key
    /// changes, which is correct — the previous keys were claims about a tool
    /// nobody had checked.
    pub fn pin(mut self, digest: Digest) -> Self {
        self.digest = Some(digest);
        if self.hermeticity == Hermeticity::Declared {
            self.hermeticity = Hermeticity::Pinned;
        }
        self
    }

    /// The string that goes into [`Action::tool`], and therefore into the key.
    ///
    /// Three distinguishable forms, so a key can never silently mean something
    /// weaker than it appears to:
    ///
    /// ```text
    /// mage-parse@0.2.0                     structural
    /// clang@18.1.0+sha256-1f3a9c2b7d40      pinned
    /// gcc@13.2.0+unpinned                   declared
    /// ```
    ///
    /// The `+unpinned` marker is not decoration. It means an operator reading a
    /// cache entry, or an agent auditing provenance, can tell that the entry
    /// rests on an unverified assumption without consulting anything else.
    pub fn tool_id(&self) -> String {
        match (&self.digest, self.hermeticity) {
            (Some(d), _) => format!("{}@{}+sha256-{}", self.tool, self.version, d.short()),
            (None, Hermeticity::Structural) => format!("{}@{}", self.tool, self.version),
            (None, _) => format!("{}@{}+unpinned", self.tool, self.version),
        }
    }
}

/// How a language's compiler consumes sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    /// One action per source file, then a link step. C, C++, assembly.
    ///
    /// The parallel case: N sources compile concurrently, and editing one
    /// rebuilds one.
    PerSource,
    /// One action for the whole target. Rust crates, Go packages, TypeScript
    /// projects — the compiler wants every source at once and reasons across
    /// them, so pretending otherwise would produce a graph that lies.
    WholeTarget,
}

/// A command template.
///
/// Substitution happens *within* each argument token, so `-o{out}` and
/// `--emit=obj={out}` both work. The token `{objs}` is special: it expands in
/// place into one argument per intermediate object, which is the shape every
/// linker wants and the one thing plain string substitution cannot express.
///
/// Placeholders:
///
/// | | |
/// |---|---|
/// | `{src}` | for [`Granularity::PerSource`], the file being compiled; for [`Granularity::WholeTarget`], the **first** source — the crate/package root |
/// | `{stem}` | the source path with its extension removed |
/// | `{name}` | the target name |
/// | `{out}` | this rule's output path |
/// | `{objs}` | expands to every intermediate object |
/// | `{srcs}` | expands to every source path |
///
/// The distinction between `{src}` and `{srcs}` for a whole-target language is
/// not cosmetic, and getting it wrong is how the `rust` builtin shipped broken:
/// `rustc` takes exactly one input filename and **errors** on more
/// (`multiple input filenames provided`), so a rule that expands `{srcs}` cannot
/// build a crate with two files. Every source is still a declared input and
/// still keyed — the compiler reads the siblings through the module system, and
/// editing one must still rebuild — but only the root is an *argument*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    pub args: Vec<String>,
    /// Output path template, e.g. `{stem}.o` or `{name}`.
    pub output: String,
}

impl Rule {
    pub fn new(output: impl Into<String>) -> Self {
        Rule { args: Vec::new(), output: output.into() }
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.args.push(a.into());
        self
    }

    pub fn args<I, S>(mut self, it: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(it.into_iter().map(Into::into));
        self
    }
}

/// The substitution environment for one rule expansion.
struct Subst<'a> {
    src: &'a str,
    stem: &'a str,
    name: &'a str,
    out: &'a str,
    objs: &'a [String],
    srcs: &'a [String],
}

impl Subst<'_> {
    fn scalar(&self, tok: &str) -> String {
        tok.replace("{src}", self.src)
            .replace("{stem}", self.stem)
            .replace("{name}", self.name)
            .replace("{out}", self.out)
    }

    /// Expand one template token into zero or more real arguments.
    fn expand(&self, tok: &str) -> Vec<String> {
        match tok {
            "{objs}" => self.objs.to_vec(),
            "{srcs}" => self.srcs.to_vec(),
            _ => vec![self.scalar(tok)],
        }
    }
}

/// Strip the last extension from a path.
fn stem_of(path: &str) -> &str {
    match path.rfind('.') {
        // A leading dot is a hidden file, not an extension.
        Some(i) if i > 0 && !path[i..].contains('/') => &path[..i],
        _ => path,
    }
}

/// Extension of a path, lowercased, without the dot.
fn ext_of(path: &str) -> Option<String> {
    let base = path.rsplit(['/', '\\']).next()?;
    let i = base.rfind('.')?;
    if i == 0 {
        return None;
    }
    Some(base[i + 1..].to_ascii_lowercase())
}

/// Everything the planner needs to know about one language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Language {
    pub name: String,
    /// Source extensions, without dots, lowercase.
    pub extensions: Vec<String>,
    pub toolchain: Toolchain,
    pub granularity: Granularity,
    pub compile: Rule,
    /// Absent for languages with no separate link step — interpreted languages,
    /// and compilers that emit a final artifact directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<Rule>,
    /// Environment every action of this language declares. Declared means keyed:
    /// changing it is a rebuild, which is the correct behaviour for something
    /// like `SOURCE_DATE_EPOCH` that genuinely changes output.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

impl Language {
    pub fn new(
        name: impl Into<String>,
        extensions: &[&str],
        toolchain: Toolchain,
        granularity: Granularity,
        compile: Rule,
    ) -> Self {
        Language {
            name: name.into(),
            extensions: extensions.iter().map(|s| s.to_ascii_lowercase()).collect(),
            toolchain,
            granularity,
            compile,
            link: None,
            env: BTreeMap::new(),
        }
    }

    pub fn link(mut self, r: Rule) -> Self {
        self.link = Some(r);
        self
    }

    pub fn env(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.env.insert(k.into(), v.into());
        self
    }

    /// Replace the toolchain — how an operator pins a builtin, or points a
    /// language at a specific installed compiler.
    pub fn with_toolchain(mut self, t: Toolchain) -> Self {
        self.toolchain = t;
        self
    }

    pub fn handles(&self, path: &str) -> bool {
        ext_of(path).map(|e| self.extensions.contains(&e)).unwrap_or(false)
    }

    pub fn hermeticity(&self) -> Hermeticity {
        self.toolchain.hermeticity
    }
}

/// A thing to build: sources in one language, plus artifacts from other targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub name: String,
    /// Language name, matching a [`Language::name`] in the registry.
    pub language: String,
    /// Source paths with the digest of their current contents.
    pub sources: Vec<Input>,
    /// Logical artifact paths produced by other targets in the same graph.
    /// These become inputs of the final action, so [`ActionGraph`] derives the
    /// cross-target edge without anyone declaring it.
    #[serde(default)]
    pub deps: Vec<String>,
    #[serde(default)]
    pub platform: Platform,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Per-target extra arguments, appended after the rule's own.
    #[serde(default)]
    pub extra_args: Vec<String>,
}

impl Target {
    pub fn new(name: impl Into<String>, language: impl Into<String>) -> Self {
        Target {
            name: name.into(),
            language: language.into(),
            sources: Vec::new(),
            deps: Vec::new(),
            platform: Platform::any(),
            env: BTreeMap::new(),
            extra_args: Vec::new(),
        }
    }

    pub fn source(mut self, path: impl Into<String>, digest: Digest) -> Self {
        self.sources.push(Input::new(path, digest));
        self
    }

    pub fn dep(mut self, artifact: impl Into<String>) -> Self {
        self.deps.push(artifact.into());
        self
    }

    pub fn platform(mut self, p: Platform) -> Self {
        self.platform = p;
        self
    }

    pub fn env(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.env.insert(k.into(), v.into());
        self
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.extra_args.push(a.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    UnknownLanguage { target: String, language: String },
    NoSources { target: String },
    /// A per-source language with no link rule cannot produce one artifact from
    /// many objects. Caught at plan time rather than producing a graph whose
    /// final artifact nobody writes.
    MissingLinkRule { target: String, language: String },
    /// A source whose extension the declared language does not handle. Usually a
    /// misfiled source, and always a sign the plan is not what was meant.
    ForeignSource { target: String, language: String, path: String },
    /// Two targets promise the same artifact path.
    Graph(GraphError),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::UnknownLanguage { target, language } => {
                write!(f, "target `{target}` declares unregistered language `{language}`")
            }
            PlanError::NoSources { target } => write!(f, "target `{target}` has no sources"),
            PlanError::MissingLinkRule { target, language } => write!(
                f,
                "target `{target}`: language `{language}` compiles per source but declares no link rule"
            ),
            PlanError::ForeignSource { target, language, path } => write!(
                f,
                "target `{target}`: `{path}` is not a `{language}` source"
            ),
            PlanError::Graph(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PlanError {}

impl From<GraphError> for PlanError {
    fn from(e: GraphError) -> Self {
        PlanError::Graph(e)
    }
}

/// Placeholder digest for an input no action has produced yet.
///
/// [`super::sched::Scheduler::run_one`] rebinds these to the digests actually
/// produced upstream before keying, so the placeholder never reaches a cache
/// entry. It exists because an [`Input`] is a path *and* a digest, and at plan
/// time only the path is knowable.
fn pending() -> Digest {
    Digest::of(b"ribosome:pending-artifact")
}

/// A planned build: actions, the artifacts they produce, and how far the
/// resulting cache entries can be trusted.
#[derive(Debug, Clone)]
pub struct Plan {
    pub actions: Vec<Action>,
    /// Final artifact path per target, in target order.
    pub artifacts: Vec<String>,
    /// The *weakest* hermeticity of any toolchain involved.
    pub hermeticity: Hermeticity,
    /// Languages that appear in this plan, sorted.
    pub languages: Vec<String>,
}

impl Plan {
    /// May this plan's results be shared across machines?
    ///
    /// One `Declared` toolchain anywhere in a mixed-language plan makes the whole
    /// plan local-only. That is not conservatism for its own sake: the artifacts
    /// are linked together, so an unverified C object contaminates the Rust
    /// binary that consumes it.
    pub fn remote_cacheable(&self) -> bool {
        self.hermeticity.remote_cacheable()
    }

    /// Toolchains in this plan that hold it back from being remote-cacheable.
    ///
    /// The actionable form of the answer: an agent that wants a shareable cache
    /// needs to know *which* tools to pin, not merely that something is unpinned.
    pub fn unpinned_tools(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .actions
            .iter()
            .filter(|a| a.tool.ends_with("+unpinned"))
            .map(|a| a.tool.as_str())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Build the action graph, deriving every edge from declared inputs.
    pub fn graph(&self) -> Result<ActionGraph, PlanError> {
        let mut g = ActionGraph::new();
        for a in &self.actions {
            g.add(a.clone())?;
        }
        Ok(g)
    }

    pub fn json(&self) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "actions": self.actions.len(),
            "artifacts": self.artifacts,
            "languages": self.languages,
            "hermeticity": self.hermeticity,
            "hermeticity_reason": self.hermeticity.reason(),
            "remote_cacheable": self.remote_cacheable(),
            "unpinned_tools": self.unpinned_tools(),
        }))
        .unwrap_or_default()
    }
}

/// The set of languages a fleet knows how to build.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    langs: Vec<Language>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every builtin. See [`builtin`].
    pub fn with_builtins() -> Self {
        let mut r = Registry::new();
        for l in builtin::all() {
            r.register(l);
        }
        r
    }

    /// Register a language, replacing any existing one of the same name.
    ///
    /// Replace rather than reject so an operator can override a builtin's
    /// toolchain — pinning `clang` to a measured binary is the expected first
    /// thing a serious deployment does.
    pub fn register(&mut self, l: Language) {
        match self.langs.iter_mut().find(|e| e.name == l.name) {
            Some(slot) => *slot = l,
            None => self.langs.push(l),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Language> {
        self.langs.iter().find(|l| l.name == name)
    }

    /// Which language claims this path, if any.
    pub fn for_path(&self, path: &str) -> Option<&Language> {
        self.langs.iter().find(|l| l.handles(path))
    }

    pub fn names(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self.langs.iter().map(|l| l.name.as_str()).collect();
        v.sort_unstable();
        v
    }

    pub fn len(&self) -> usize {
        self.langs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.langs.is_empty()
    }

    /// Pin a registered language's toolchain to a measured executable digest.
    pub fn pin(&mut self, language: &str, digest: Digest) -> bool {
        match self.langs.iter_mut().find(|l| l.name == language) {
            Some(l) => {
                l.toolchain = l.toolchain.clone().pin(digest);
                true
            }
            None => false,
        }
    }

    /// Plan one target.
    pub fn plan(&self, t: &Target) -> Result<Plan, PlanError> {
        self.plan_all(std::slice::from_ref(t))
    }

    /// Plan several targets into one graph.
    ///
    /// Cross-target edges are *not* declared here either: a target's `deps` name
    /// artifacts, those artifacts are other targets' outputs, and
    /// [`ActionGraph::add`] derives the edge. So a Rust binary depending on a C
    /// library is an ordinary data dependency, and a cycle between two targets in
    /// different languages is caught by the same check as any other cycle.
    pub fn plan_all(&self, targets: &[Target]) -> Result<Plan, PlanError> {
        let mut actions = Vec::new();
        let mut artifacts = Vec::new();
        let mut languages: Vec<String> = Vec::new();
        // Vacuously the strongest: an empty plan has nothing to distrust.
        let mut herm = Hermeticity::Structural;

        for t in targets {
            let lang = self.get(&t.language).ok_or_else(|| PlanError::UnknownLanguage {
                target: t.name.clone(),
                language: t.language.clone(),
            })?;
            if t.sources.is_empty() {
                return Err(PlanError::NoSources { target: t.name.clone() });
            }
            for s in &t.sources {
                if !lang.handles(&s.path) {
                    return Err(PlanError::ForeignSource {
                        target: t.name.clone(),
                        language: lang.name.clone(),
                        path: s.path.clone(),
                    });
                }
            }

            herm = herm.max(lang.hermeticity());
            if !languages.contains(&lang.name) {
                languages.push(lang.name.clone());
            }

            let (mut acts, artifact) = self.plan_target(t, lang)?;
            actions.append(&mut acts);
            artifacts.push(artifact);
        }

        languages.sort();
        Ok(Plan { actions, artifacts, hermeticity: herm, languages })
    }

    fn plan_target(&self, t: &Target, lang: &Language) -> Result<(Vec<Action>, String), PlanError> {
        let srcs: Vec<String> = t.sources.iter().map(|i| i.path.clone()).collect();
        let tool = lang.toolchain.tool_id();
        let mut actions = Vec::new();

        // Shared decoration: declared env is keyed env, so language-level and
        // target-level settings both reach the key.
        let decorate = |mut a: Action| -> Action {
            for (k, v) in lang.env.iter().chain(t.env.iter()) {
                a = a.env(k.clone(), v.clone());
            }
            a.platform(t.platform.clone())
        };

        match lang.granularity {
            Granularity::WholeTarget => {
                // `{src}` is the crate/package root: the first declared source.
                // Sources are ordered as the manifest wrote them, so "first" is
                // the caller's choice rather than a filesystem accident.
                let root = srcs.first().map(String::as_str).unwrap_or("");
                let root_stem = stem_of(root).to_string();

                let out = Subst {
                    src: root,
                    stem: &root_stem,
                    name: &t.name,
                    out: "",
                    objs: &[],
                    srcs: &srcs,
                }
                .scalar(&lang.compile.output);

                let s = Subst {
                    src: root,
                    stem: &root_stem,
                    name: &t.name,
                    out: &out,
                    objs: &[],
                    srcs: &srcs,
                };
                let mut a = Action::new(format!("build:{}", t.name), tool.clone());
                for tok in &lang.compile.args {
                    for arg in s.expand(tok) {
                        a = a.arg(arg);
                    }
                }
                for extra in &t.extra_args {
                    a = a.arg(s.scalar(extra));
                }
                for src in &t.sources {
                    a = a.input(src.path.clone(), src.digest.clone());
                }
                for d in &t.deps {
                    a = a.input(d.clone(), pending());
                }
                // Cost is a scheduling hint only, never keyed. Sources are a
                // crude but monotone proxy, and a wrong hint costs ordering, not
                // correctness.
                actions.push(decorate(a.output(out.clone()).cost(t.sources.len().max(1) as u64)));
                Ok((actions, out))
            }

            Granularity::PerSource => {
                let link = lang.link.as_ref();
                let mut objs = Vec::new();

                for src in &t.sources {
                    let stem = stem_of(&src.path).to_string();
                    let out = Subst {
                        src: &src.path,
                        stem: &stem,
                        name: &t.name,
                        out: "",
                        objs: &[],
                        srcs: &srcs,
                    }
                    .scalar(&lang.compile.output);

                    let s = Subst {
                        src: &src.path,
                        stem: &stem,
                        name: &t.name,
                        out: &out,
                        objs: &[],
                        srcs: &srcs,
                    };
                    let mut a = Action::new(format!("compile:{}", src.path), tool.clone());
                    for tok in &lang.compile.args {
                        for arg in s.expand(tok) {
                            a = a.arg(arg);
                        }
                    }
                    // Extra args go to the compile step for interpreted
                    // languages (there is no link step to carry them) and to the
                    // link step otherwise, where flags like `-L` belong.
                    if link.is_none() {
                        for extra in &t.extra_args {
                            a = a.arg(s.scalar(extra));
                        }
                    }
                    a = a.input(src.path.clone(), src.digest.clone());
                    // With no link step, each source's dependency is its own.
                    if link.is_none() {
                        for d in &t.deps {
                            a = a.input(d.clone(), pending());
                        }
                    }
                    actions.push(decorate(a.output(out.clone())));
                    objs.push(out);
                }

                let Some(link) = link else {
                    // No link step: the per-source outputs *are* the artifacts.
                    // Report the last one so `artifacts` stays one-per-target;
                    // the full set is visible in the graph.
                    let last = objs.last().cloned().unwrap_or_else(|| t.name.clone());
                    return Ok((actions, last));
                };

                let out = Subst {
                    src: "",
                    stem: "",
                    name: &t.name,
                    out: "",
                    objs: &objs,
                    srcs: &srcs,
                }
                .scalar(&link.output);

                let s = Subst {
                    src: "",
                    stem: "",
                    name: &t.name,
                    out: &out,
                    objs: &objs,
                    srcs: &srcs,
                };
                let mut a = Action::new(format!("link:{}", t.name), tool.clone());
                for tok in &link.args {
                    for arg in s.expand(tok) {
                        a = a.arg(arg);
                    }
                }
                for extra in &t.extra_args {
                    a = a.arg(s.scalar(extra));
                }
                for o in &objs {
                    a = a.input(o.clone(), pending());
                }
                for d in &t.deps {
                    a = a.input(d.clone(), pending());
                }
                actions.push(decorate(a.output(out.clone()).cost(objs.len().max(1) as u64)));
                Ok((actions, out))
            }
        }
    }
}

/// Builtin language definitions.
///
/// These are starting points, not policy. Every one ships `Declared` — naming a
/// compiler is not evidence of which compiler — so a fresh registry builds
/// correctly and caches *locally*, and a deployment earns remote caching by
/// pinning ([`Registry::pin`]). Shipping them pre-pinned would be a lie, since
/// the digest depends on the machine.
///
/// The exception is MAGE, whose byte-stability is measured rather than asserted.
///
/// Arguments are the real flags each toolchain takes, so a `Program` registered
/// in [`super::subprocess`] pointing at the actual binary runs these unmodified.
///
/// ## How far these have actually been verified
///
/// The unit tests here check what the *planner emits*. That is not the same as
/// checking that a real compiler accepts it, and the difference was not
/// theoretical: `rust` shipped passing every source to `rustc`, which rejects
/// more than one input filename outright, and `python` shipped declaring an
/// output that `-m py_compile` never writes. Both were found by running the CLI
/// against a real toolchain, and both are fixed above.
///
/// Every builtin has now been built end to end against a real toolchain, and
/// **three of the seven were broken** until that happened:
///
/// | Language | Verified against | Was it right? |
/// |---|---|---|
/// | `mage` | the test suite throughout | yes |
/// | `rust` | `rustc` 1.97.1 | **no** — passed every source to a compiler that accepts exactly one |
/// | `python` | CPython 3.12.11 | **no** — declared an output `-m py_compile` never writes (PEP 3147) |
/// | `typescript` | `tsc` 5.9.3 | **no** — `--outFile` is rejected for any source with an import (TS6131) |
/// | `c` | `clang` 22.1.8 | yes — compile-then-link, artifact executed |
/// | `cpp` | `clang++` 22.1.8 | yes |
/// | `go` | `go` 1.26.5 | yes — multi-file, artifact executed |
///
/// Three out of seven is the whole argument for this table existing. Every one
/// of those templates looked right; each was assembled from the tool's
/// documented interface by someone being careful. A plausible command line is
/// not evidence, and nothing short of running it is.
///
/// A builtin is still only a *starting point* to override with
/// [`Registry::register`] — these were verified on one machine, with one version
/// of each tool. The engine is what is tested; a builtin is configuration.
pub mod builtin {
    use super::*;

    /// MAGE — the only `Structural` toolchain, and only because `MEASUREMENTS.md`
    /// §2 measures byte-identical output across runs and machines.
    pub fn mage() -> Language {
        Language::new(
            "mage",
            &["mg", "mage"],
            Toolchain::structural("mage-parse", "0.2.0"),
            Granularity::WholeTarget,
            Rule::new("{name}.abl").args(["--build=abl", "{srcs}", "-o", "{out}"]),
        )
    }

    pub fn c() -> Language {
        Language::new(
            "c",
            &["c", "h"],
            Toolchain::declared("cc", "unknown"),
            Granularity::PerSource,
            Rule::new("{stem}.o").args(["-c", "{src}", "-o", "{out}"]),
        )
        .link(Rule::new("{name}").args(["{objs}", "-o", "{out}"]))
    }

    pub fn cpp() -> Language {
        Language::new(
            "cpp",
            &["cc", "cpp", "cxx", "hpp", "hh"],
            Toolchain::declared("c++", "unknown"),
            Granularity::PerSource,
            Rule::new("{stem}.o").args(["-c", "{src}", "-o", "{out}"]),
        )
        .link(Rule::new("{name}").args(["{objs}", "-o", "{out}"]))
    }

    /// Rust — whole-crate, because `rustc` reasons across the whole crate and a
    /// per-file graph would misrepresent both its parallelism and its rebuilds.
    ///
    /// Passes `{src}` — the **first** source, the crate root — not `{srcs}`.
    /// `rustc` accepts exactly one input filename and errors with
    /// `multiple input filenames provided` on more; this shipped with `{srcs}`
    /// and could not build a two-file crate. The siblings are still declared
    /// inputs and still in the key, which is what makes editing one rebuild the
    /// crate; they are simply not arguments.
    pub fn rust() -> Language {
        Language::new(
            "rust",
            &["rs"],
            Toolchain::declared("rustc", "unknown"),
            Granularity::WholeTarget,
            Rule::new("{name}").args(["--crate-name", "{name}", "{src}", "-o", "{out}"]),
        )
    }

    pub fn go() -> Language {
        Language::new(
            "go",
            &["go"],
            Toolchain::declared("go", "unknown"),
            Granularity::WholeTarget,
            Rule::new("{name}").args(["build", "-o", "{out}", "{srcs}"]),
        )
    }

    /// Python — byte-compilation only. There is no artifact to link, so the
    /// `.pyc` per source *is* the output. Honest about what a build system can
    /// do for an interpreted language: catch syntax errors and cache the result.
    ///
    /// Calls `py_compile.compile` with an explicit destination rather than
    /// `-m py_compile`, which follows PEP 3147 and writes
    /// `__pycache__/<name>.cpython-3XX.pyc` — a path this rule cannot name,
    /// since it embeds the interpreter version. The declared output would never
    /// appear and every action would fail `MissingOutput`. `doraise` makes a
    /// syntax error a non-zero exit instead of a silent skip.
    pub fn python() -> Language {
        Language::new(
            "python",
            &["py"],
            Toolchain::declared("python", "unknown"),
            Granularity::PerSource,
            Rule::new("{stem}.pyc").args([
                "-c",
                "import py_compile,sys; py_compile.compile(sys.argv[1], sys.argv[2], doraise=True)",
                "{src}",
                "{out}",
            ]),
        )
    }

    /// TypeScript — transpile per source, no bundling.
    ///
    /// Uses `--outDir . --rootDir .` rather than `--outFile {out}`. `--outFile`
    /// fails outright on any source with an import or export
    /// (`TS6131: Cannot compile modules using option 'outFile' unless the
    /// '--module' flag is 'amd' or 'system'`), which is nearly every real
    /// TypeScript file — this shipped with `--outFile` and could not compile a
    /// module. `--rootDir .` is what keeps `src/a.ts` emitting `src/a.js`
    /// instead of `a.js`: without it `tsc` infers the root from the single input
    /// and flattens the path, so the declared output would never appear.
    pub fn typescript() -> Language {
        Language::new(
            "typescript",
            &["ts", "tsx"],
            Toolchain::declared("tsc", "unknown"),
            Granularity::PerSource,
            Rule::new("{stem}.js").args(["--outDir", ".", "--rootDir", ".", "{src}"]),
        )
    }

    pub fn all() -> Vec<Language> {
        vec![mage(), c(), cpp(), rust(), go(), python(), typescript()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Digest {
        Digest::of(s.as_bytes())
    }

    // ── hermeticity ──────────────────────────────────────────────────────────

    #[test]
    fn an_unpinned_toolchain_is_not_remote_cacheable() {
        assert!(!Hermeticity::Declared.remote_cacheable());
        assert!(Hermeticity::Pinned.remote_cacheable());
        assert!(Hermeticity::Structural.remote_cacheable());
    }

    #[test]
    fn a_toolchain_digest_participates_in_the_action_key() {
        // The whole point: two machines' `gcc-13.2.0` are different tools, and
        // the key must say so rather than letting one serve the other's cache.
        let reg = Registry::with_builtins();
        let t = Target::new("app", "c").source("a.c", d("int main(){}"));

        let mut pinned_a = reg.clone();
        pinned_a.pin("c", d("gcc-binary-from-machine-A"));
        let mut pinned_b = reg.clone();
        pinned_b.pin("c", d("gcc-binary-from-machine-B"));

        let ka = pinned_a.plan(&t).unwrap().actions[0].key();
        let kb = pinned_b.plan(&t).unwrap().actions[0].key();
        assert_ne!(ka, kb, "two different compiler binaries must not share a cache entry");
    }

    #[test]
    fn pinning_changes_every_key_and_that_is_correct() {
        let reg = Registry::with_builtins();
        let t = Target::new("app", "c").source("a.c", d("x"));
        let unpinned = reg.plan(&t).unwrap();

        let mut pinned = reg.clone();
        pinned.pin("c", d("measured"));
        let after = pinned.plan(&t).unwrap();

        assert!(!unpinned.remote_cacheable());
        assert!(after.remote_cacheable());
        assert_ne!(
            unpinned.actions[0].key(),
            after.actions[0].key(),
            "the old keys were claims about a tool nobody had checked"
        );
    }

    #[test]
    fn an_unpinned_tool_id_says_so_in_the_key_material() {
        let reg = Registry::with_builtins();
        let plan = reg.plan(&Target::new("app", "c").source("a.c", d("x"))).unwrap();
        assert!(plan.actions[0].tool.ends_with("+unpinned"), "{}", plan.actions[0].tool);
        assert_eq!(plan.unpinned_tools(), vec!["cc@unknown+unpinned"]);
    }

    #[test]
    fn mage_is_structural_and_carries_no_marker() {
        let reg = Registry::with_builtins();
        let plan = reg.plan(&Target::new("m", "mage").source("m.mg", d("net M {}"))).unwrap();
        assert_eq!(plan.hermeticity, Hermeticity::Structural);
        assert!(plan.remote_cacheable());
        assert_eq!(plan.actions[0].tool, "mage-parse@0.2.0");
        assert!(plan.unpinned_tools().is_empty());
    }

    #[test]
    fn one_weak_toolchain_makes_a_mixed_plan_local_only() {
        // The contaminating case: a MAGE artifact linked against an unverified C
        // object is exactly as trustworthy as the C object.
        let reg = Registry::with_builtins();
        let plan = reg
            .plan_all(&[
                Target::new("m", "mage").source("m.mg", d("net M {}")),
                Target::new("helper", "c").source("h.c", d("void h(){}")),
            ])
            .unwrap();
        assert_eq!(plan.hermeticity, Hermeticity::Declared);
        assert!(!plan.remote_cacheable());
        assert_eq!(plan.languages, vec!["c", "mage"]);
    }

    // ── planning shape ───────────────────────────────────────────────────────

    #[test]
    fn a_per_source_language_compiles_each_file_then_links() {
        let reg = Registry::with_builtins();
        let t = Target::new("app", "c")
            .source("src/a.c", d("a"))
            .source("src/b.c", d("b"));
        let plan = reg.plan(&t).unwrap();

        assert_eq!(plan.actions.len(), 3, "two compiles and a link");
        assert_eq!(plan.actions[0].outputs, vec!["src/a.o"]);
        assert_eq!(plan.actions[1].outputs, vec!["src/b.o"]);

        let link = &plan.actions[2];
        assert_eq!(link.name, "link:app");
        assert_eq!(link.args, vec!["src/a.o", "src/b.o", "-o", "app"]);
        assert_eq!(plan.artifacts, vec!["app"]);
    }

    #[test]
    fn a_whole_target_language_emits_exactly_one_action() {
        let reg = Registry::with_builtins();
        let t = Target::new("tool", "rust")
            .source("src/main.rs", d("fn main(){}"))
            .source("src/lib.rs", d("pub fn f(){}"));
        let plan = reg.plan(&t).unwrap();

        assert_eq!(plan.actions.len(), 1, "rustc reasons across the whole crate");
        let a = &plan.actions[0];
        // Only the crate root is an argument: `rustc` errors on a second input
        // filename. This asserted the opposite until a real rustc said so.
        assert_eq!(a.args, vec!["--crate-name", "tool", "src/main.rs", "-o", "tool"]);
        assert_eq!(a.inputs.len(), 2, "but every source is keyed, so editing lib.rs rebuilds");
    }

    #[test]
    fn editing_a_non_root_source_still_rebuilds_a_whole_target_crate() {
        // The property that makes it safe for siblings to be inputs but not
        // arguments. If this failed, whole-target languages would silently
        // serve stale artifacts after an edit to any file but the root.
        let reg = Registry::with_builtins();
        let before = reg
            .plan(
                &Target::new("t", "rust")
                    .source("main.rs", d("fn main(){}"))
                    .source("helper.rs", d("pub fn h(){}")),
            )
            .unwrap();
        let after = reg
            .plan(
                &Target::new("t", "rust")
                    .source("main.rs", d("fn main(){}"))
                    .source("helper.rs", d("pub fn h(){ println!(); }")),
            )
            .unwrap();

        assert_eq!(before.actions[0].args, after.actions[0].args, "the command is unchanged");
        assert_ne!(
            before.actions[0].key(),
            after.actions[0].key(),
            "yet the key must change, or an edit to a non-root source is invisible"
        );
    }

    #[test]
    fn an_interpreted_language_produces_one_artifact_per_source_and_no_link() {
        let reg = Registry::with_builtins();
        let plan = reg
            .plan(&Target::new("svc", "python").source("app.py", d("x")).source("util.py", d("y")))
            .unwrap();
        assert_eq!(plan.actions.len(), 2);
        assert!(plan.actions.iter().all(|a| a.name.starts_with("compile:")));
        assert_eq!(plan.actions[0].outputs, vec!["app.pyc"]);
    }

    #[test]
    fn the_objs_token_expands_in_place_rather_than_concatenating() {
        // A single joined string would be one argument, which no linker accepts.
        let reg = Registry::with_builtins();
        let plan = reg
            .plan(
                &Target::new("app", "c")
                    .source("a.c", d("1"))
                    .source("b.c", d("2"))
                    .source("c.c", d("3")),
            )
            .unwrap();
        let link = plan.actions.last().unwrap();
        assert_eq!(link.args[..3], ["a.o", "b.o", "c.o"]);
    }

    #[test]
    fn substitution_works_inside_a_token_not_only_as_a_whole_token() {
        let mut reg = Registry::new();
        reg.register(Language::new(
            "odd",
            &["q"],
            Toolchain::declared("q", "1"),
            Granularity::WholeTarget,
            Rule::new("{name}.out").args(["--emit=obj={out}", "-Wl,-soname,{name}"]),
        ));
        let plan = reg.plan(&Target::new("thing", "odd").source("a.q", d("x"))).unwrap();
        assert_eq!(plan.actions[0].args, vec!["--emit=obj=thing.out", "-Wl,-soname,thing"]);
    }

    // ── graph integration ────────────────────────────────────────────────────

    #[test]
    fn cross_language_dependencies_become_derived_graph_edges() {
        // A Rust binary consuming a C library: nobody declares an edge, and the
        // graph derives one because the artifact paths line up.
        let reg = Registry::with_builtins();
        let plan = reg
            .plan_all(&[
                Target::new("libutil.a", "c").source("util.c", d("void u(){}")),
                Target::new("app", "rust").source("main.rs", d("fn main(){}")).dep("libutil.a"),
            ])
            .unwrap();

        let g = plan.graph().unwrap();
        let order = g.topological_order().unwrap();
        let names: Vec<&str> = order.iter().map(|&i| g.actions[i].name.as_str()).collect();

        let c_link = names.iter().position(|n| *n == "link:libutil.a").unwrap();
        let rust_build = names.iter().position(|n| *n == "build:app").unwrap();
        assert!(c_link < rust_build, "the C library must be built before the Rust binary");
    }

    #[test]
    fn per_source_compiles_are_independent_and_can_run_in_one_wave() {
        let reg = Registry::with_builtins();
        let plan = reg
            .plan(&Target::new("app", "c").source("a.c", d("1")).source("b.c", d("2")))
            .unwrap();
        let g = plan.graph().unwrap();
        let waves = g.parallel_waves().unwrap();
        assert_eq!(waves[0].len(), 2, "two translation units are independent");
        assert_eq!(waves[1].len(), 1, "the link waits for both");
    }

    #[test]
    fn two_targets_promising_the_same_artifact_are_rejected() {
        let reg = Registry::with_builtins();
        let plan = reg
            .plan_all(&[
                Target::new("app", "c").source("a.c", d("1")),
                Target::new("app", "c").source("b.c", d("2")),
            ])
            .unwrap();
        assert!(matches!(plan.graph(), Err(PlanError::Graph(GraphError::DuplicateOutput { .. }))));
    }

    // ── errors ───────────────────────────────────────────────────────────────

    #[test]
    fn an_unregistered_language_is_refused_at_plan_time() {
        let reg = Registry::with_builtins();
        let err = reg.plan(&Target::new("x", "cobol").source("x.cob", d("x"))).unwrap_err();
        assert!(matches!(err, PlanError::UnknownLanguage { .. }));
    }

    #[test]
    fn a_source_the_language_does_not_handle_is_caught() {
        let reg = Registry::with_builtins();
        let err = reg.plan(&Target::new("x", "c").source("x.rs", d("x"))).unwrap_err();
        match err {
            PlanError::ForeignSource { path, .. } => assert_eq!(path, "x.rs"),
            other => panic!("a misfiled source must not plan: {other:?}"),
        }
    }

    #[test]
    fn a_target_with_no_sources_is_refused() {
        let reg = Registry::with_builtins();
        assert!(matches!(
            reg.plan(&Target::new("empty", "c")),
            Err(PlanError::NoSources { .. })
        ));
    }

    // ── registry ─────────────────────────────────────────────────────────────

    #[test]
    fn a_language_claims_paths_by_extension_case_insensitively() {
        let reg = Registry::with_builtins();
        assert_eq!(reg.for_path("src/main.RS").map(|l| l.name.as_str()), Some("rust"));
        assert_eq!(reg.for_path("a/b/c.cpp").map(|l| l.name.as_str()), Some("cpp"));
        assert_eq!(reg.for_path("Makefile"), None);
        assert_eq!(reg.for_path(".gitignore"), None, "a dotfile has no extension");
    }

    #[test]
    fn registering_an_existing_name_replaces_rather_than_duplicates() {
        let mut reg = Registry::with_builtins();
        let n = reg.len();
        reg.register(builtin::c().with_toolchain(Toolchain::pinned("clang", "18.1.0", d("bin"))));
        assert_eq!(reg.len(), n, "an override must not shadow a duplicate");
        assert_eq!(reg.get("c").unwrap().toolchain.tool, "clang");
    }

    #[test]
    fn adding_a_language_needs_no_change_to_the_engine() {
        // The load-bearing claim of this module: a new language is data.
        let mut reg = Registry::new();
        reg.register(
            Language::new(
                "zig",
                &["zig"],
                Toolchain::pinned("zig", "0.13.0", d("zig-binary")),
                Granularity::WholeTarget,
                Rule::new("{name}").args(["build-exe", "{srcs}", "-femit-bin={out}"]),
            )
            .env("ZIG_GLOBAL_CACHE_DIR", "/nonexistent"),
        );
        let plan = reg.plan(&Target::new("hello", "zig").source("main.zig", d("pub fn main(){}"))).unwrap();
        assert!(plan.remote_cacheable(), "a pinned toolchain earns a shared cache");
        assert_eq!(plan.actions[0].args, vec!["build-exe", "main.zig", "-femit-bin=hello"]);
        assert_eq!(plan.actions[0].env["ZIG_GLOBAL_CACHE_DIR"], "/nonexistent");
    }

    #[test]
    fn declared_env_reaches_the_key_so_changing_it_rebuilds() {
        let reg = Registry::with_builtins();
        let base = Target::new("app", "c").source("a.c", d("x"));
        let opt = Target::new("app", "c").source("a.c", d("x")).env("SOURCE_DATE_EPOCH", "0");
        assert_ne!(
            reg.plan(&base).unwrap().actions[0].key(),
            reg.plan(&opt).unwrap().actions[0].key()
        );
    }

    #[test]
    fn target_platform_reaches_every_action() {
        let reg = Registry::with_builtins();
        let t = Target::new("k", "c")
            .source("k.c", d("x"))
            .platform(Platform::host("linux", "x86_64").with_accelerator("cuda"));
        let plan = reg.plan(&t).unwrap();
        assert!(plan.actions.iter().all(|a| a.platform.accelerator.as_deref() == Some("cuda")));
    }

    #[test]
    fn plan_json_is_agent_readable_and_names_what_to_pin() {
        let reg = Registry::with_builtins();
        let plan = reg.plan(&Target::new("app", "c").source("a.c", d("x"))).unwrap();
        let j = plan.json();
        assert!(j.contains("\"remote_cacheable\": false"));
        assert!(j.contains("cc@unknown+unpinned"));
        assert!(j.contains("toolchain named but unverified"));
    }

    #[test]
    fn stem_and_extension_handle_directories_and_dotfiles() {
        assert_eq!(stem_of("src/a.c"), "src/a");
        assert_eq!(stem_of("noext"), "noext");
        assert_eq!(ext_of("a/b.c.d"), Some("d".into()));
        assert_eq!(ext_of("dir.d/file"), None, "an extension on a directory is not the file's");
        assert_eq!(ext_of(".hidden"), None);
    }
}
