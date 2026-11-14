//! A command-line driver for the self-modification workload.
//!
//! An external agent drives recursive self-improvement on compiler source with
//! a loop:
//!
//! 1. write a candidate into the outbox: `<outbox>/<id>/manifest.json` and
//!    `files/<path>...` (the format [`DirProposer`] reads);
//! 2. run `selfmod --config run.json --cycles 1`;
//! 3. read the JSON report on the last line of stdout;
//! 4. repeat.
//!
//! Every invocation resumes from the journal. The journal is the only state
//! that matters: the lineage is rebuilt from it ([`Lineage::replay`]), so the
//! gate's decisions, the refusals and the proposer's trust all carry over.
//!
//! ## What a resume has to rebuild
//!
//! - **Genesis is not re-measured.** The evaluation includes wall-clock
//!   efficiency, so measuring the base tree again would give a different
//!   fitness and replay would refuse the journal. The first invocation saves
//!   the genesis measurement to `genesis.json` beside the journal, and every
//!   later one rebuilds the genesis generation from that file.
//! - **The champion is rebuilt on disk.** [`SelfModWorkload`] keeps what it
//!   built in memory only. After replay the champion's genome is materialized
//!   again and must give the digest the journal recorded. Without that, the
//!   workload would call the champion unmaterialized and the runner's
//!   supervisor (`observe_champion`) would read each probe as a failure and
//!   demote a good champion, and a demotion would have no materialized
//!   fallback to return to.
//! - **The suite is pinned.** Its id and digest come from the evaluator
//!   command: the argv, and the bytes of every argv element that is a file. A
//!   changed evaluator changes the suite, and a resume refuses it.
//!
//! One candidate is run per cycle, and no more candidates are consumed than
//! cycles are run: the rest stay in the outbox. A run that would find nothing
//! to propose is not started, because an empty proposal halts the runner as a
//! workload failure.

use super::{canon, BlobStore, Cmd, DirProposer, SelfModConfig, SelfModWorkload};
use crate::attest::Attestor;
use crate::directed::CandidateSpec;
use crate::gate::{Episode, PromotionGate};
use crate::journal::{Entry, Journal};
use crate::lineage::Lineage;
use crate::runner::{Halt, Proposer, Runner, RunnerPolicy, Workload};
use crate::supervisor::SupervisionPolicy;

use crate::{EvalSuite, FitnessVector, Generation, Measurement, SuiteKind};
use ribosome::mac::absorb;
use ribosome::Digest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The identity that issues verdicts for this driver.
const EVALUATOR: &str = "selfmod-cli";

pub const USAGE: &str = "usage: selfmod --config <run.json> [--cycles <n>] [--export-champion <dir>]\n\
Runs up to <n> (default 1) self-modification cycles, one outbox candidate per cycle, resuming from the journal.\n\
--export-champion writes the champion's tree (the base with its edits applied) to the new directory <dir>\n\
and runs no cycle, so a proposer can build its next candidate on the champion's edits.\n\
The last line of stdout is a JSON report.";

fn default_axis() -> String {
    "capability".into()
}
fn default_improvement() -> f64 {
    0.01
}
fn default_guards() -> Vec<String> {
    vec!["safety".into(), "correctness".into()]
}
fn default_one() -> u32 {
    1
}
/// The attestation key when `SELFMOD_ATTEST_KEY` is unset. HMAC is symmetric,
/// so this only says a verdict came from a process that knows the key. The key
/// is read from the environment, not the config file, so it is not in a file
/// the proposing agent is handed. See `germline::attest`.
const DEV_ATTEST_KEY: &str = "germline-selfmod-development-key";

fn attest_key() -> String {
    std::env::var("SELFMOD_ATTEST_KEY").unwrap_or_else(|_| {
        eprintln!("selfmod: SELFMOD_ATTEST_KEY is unset; attesting with the development key");
        DEV_ATTEST_KEY.to_string()
    })
}

/// A command and its timeout.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CmdConfig {
    pub argv: Vec<String>,
    pub timeout_secs: u64,
}

impl CmdConfig {
    fn cmd(&self) -> Cmd {
        Cmd { argv: self.argv.clone(), timeout: Duration::from_secs(self.timeout_secs) }
    }
}

/// The run configuration file. Relative paths resolve against the directory the
/// file is in.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliConfig {
    pub base_tree: PathBuf,
    pub base_identity: String,
    pub writable: Vec<String>,
    #[serde(default)]
    pub protected: Vec<String>,
    pub scratch_root: PathBuf,
    pub build: CmdConfig,
    pub test: CmdConfig,
    pub evaluator: CmdConfig,
    #[serde(default)]
    pub shadow: Option<CmdConfig>,
    pub binary: String,
    #[serde(default)]
    pub target_dir: Option<PathBuf>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub blob_store: PathBuf,
    pub outbox: PathBuf,
    pub journal: PathBuf,
    pub proposer_name: String,
    #[serde(default = "default_axis")]
    pub primary_axis: String,
    #[serde(default = "default_improvement")]
    pub min_improvement: f64,
    #[serde(default = "default_guards")]
    pub guard_axes: Vec<String>,
    #[serde(default)]
    pub guard_tolerance: f64,
    #[serde(default = "default_one")]
    pub min_shadow_successes: u32,
    #[serde(default = "default_one")]
    pub shadow_runs: u32,
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// The directory the config file is in. Not read from the file.
    #[serde(skip)]
    pub dir: PathBuf,
}

fn default_seed() -> u64 {
    1
}

impl CliConfig {
    /// Read and resolve a config file.
    pub fn load(path: &Path) -> Result<CliConfig, String> {
        let real = canon(path).map_err(|e| format!("cannot read the config {}: {e}", path.display()))?;
        let text = std::fs::read_to_string(&real).map_err(|e| format!("cannot read the config {}: {e}", real.display()))?;
        let mut cfg: CliConfig =
            serde_json::from_str(&text).map_err(|e| format!("the config {} is not valid: {e}", real.display()))?;
        cfg.dir = real.parent().map(Path::to_path_buf).unwrap_or_default();
        let at = |p: &PathBuf| if p.is_absolute() { p.clone() } else { cfg.dir.join(p) };
        cfg.base_tree = at(&cfg.base_tree);
        cfg.scratch_root = at(&cfg.scratch_root);
        cfg.target_dir = cfg.target_dir.as_ref().map(at);
        cfg.blob_store = at(&cfg.blob_store);
        cfg.outbox = at(&cfg.outbox);
        cfg.journal = at(&cfg.journal);
        Ok(cfg)
    }

    fn selfmod_config(&self, suite: &EvalSuite) -> SelfModConfig {
        let mut c = SelfModConfig::new(
            &self.base_tree,
            &self.base_identity,
            &self.scratch_root,
            self.build.cmd(),
            self.test.cmd(),
            &self.binary,
            self.evaluator.cmd(),
        );
        c.writable = self.writable.clone();
        c.protected = self.protected.clone();
        c.target_dir = self.target_dir.clone();
        c.env = self.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        c.shadow = self.shadow.as_ref().map(CmdConfig::cmd);
        c.expected_suite = Some(suite.digest.clone());
        c
    }

    fn gate(&self) -> PromotionGate {
        PromotionGate {
            primary_axis: self.primary_axis.clone(),
            min_improvement: self.min_improvement,
            guard_axes: self.guard_axes.clone(),
            guard_tolerance: self.guard_tolerance,
            min_shadow_successes: self.min_shadow_successes,
            dominance: None,
        }
    }

    fn genesis_path(&self) -> PathBuf {
        match self.journal.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.join("genesis.json"),
            _ => PathBuf::from("genesis.json"),
        }
    }
}

/// The suite the evaluator command defines: a digest over its argv and the bytes
/// of every argv element that names a file (resolved against the config
/// directory first, then as given). Changing the evaluator or a manifest it
/// reads changes the suite, and the gate refuses to mix suites.
pub fn evaluator_suite(cfg: &CliConfig) -> EvalSuite {
    let mut h = Sha256::new();
    h.update(b"germline-selfmod-cli-suite-v1");
    for el in &cfg.evaluator.argv {
        absorb(&mut h, el.as_bytes());
        let joined = cfg.dir.join(el);
        let file = if joined.is_file() {
            Some(joined)
        } else if Path::new(el).is_file() {
            Some(PathBuf::from(el))
        } else {
            None
        };
        match file.and_then(|f| std::fs::read(f).ok()) {
            Some(bytes) => {
                absorb(&mut h, b"file");
                absorb(&mut h, &bytes);
            }
            None => absorb(&mut h, b"-"),
        }
    }
    EvalSuite::new("selfmod-evaluator", SuiteKind::HeldOut, Digest(format!("{:x}", h.finalize())))
}

/// The genesis measurement, saved by the first invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GenesisFile {
    digest: String,
    base_identity: String,
    fitness: BTreeMap<String, f64>,
    suite_id: String,
    suite_digest: String,
    shadow_successes: u32,
}

/// The candidates the proposer was asked for, handed to the runner one per cycle.
struct Queue {
    name: String,
    specs: Vec<CandidateSpec>,
}

impl Proposer for Queue {
    fn name(&self) -> &str {
        &self.name
    }
    fn propose(&mut self, _champion: Option<&FitnessVector>, _seed: u64) -> Vec<CandidateSpec> {
        if self.specs.is_empty() {
            Vec::new()
        } else {
            vec![self.specs.remove(0)]
        }
    }
}

struct Options {
    config: PathBuf,
    cycles: u32,
    export: Option<PathBuf>,
}

fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut config = None;
    let mut cycles = 1u32;
    let mut export = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => return Ok(None),
            "--config" => config = Some(PathBuf::from(it.next().ok_or("--config needs a path")?)),
            "--cycles" => {
                let v = it.next().ok_or("--cycles needs a number")?;
                cycles = v.parse().map_err(|_| format!("--cycles `{v}` is not a number"))?;
                if cycles == 0 {
                    return Err("--cycles must be at least 1".into());
                }
            }
            "--export-champion" => export = Some(PathBuf::from(it.next().ok_or("--export-champion needs a directory")?)),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    let config = config.ok_or_else(|| format!("--config is required\n{USAGE}"))?;
    Ok(Some(Options { config, cycles, export }))
}

/// Run the driver: `args` are the command-line arguments after the program
/// name. `Ok` is the report (one line of JSON), or the usage text for `--help`.
/// `Err` is a message for a failure before or around the run. A run that
/// started and halted for a workload failure is an `Ok` report with `"ok":
/// false`.
pub fn run(args: &[String]) -> Result<String, String> {
    let Some(opts) = parse_args(args)? else {
        return Ok(USAGE.to_string());
    };
    let cfg = CliConfig::load(&opts.config)?;
    execute(&cfg, opts.cycles, opts.export.as_deref())
}

fn fitness_json(f: &FitnessVector) -> Value {
    json!(f.0)
}

fn count_candidates(outbox: &Path) -> usize {
    std::fs::read_dir(outbox)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .count()
        })
        .unwrap_or(0)
}

fn champion_json(lineage: &Lineage) -> Value {
    match lineage.champion() {
        Some(g) => json!({
            "generation": g.id.0,
            "artifact": g.artifact.0,
            "files": g.definitions,
            "fitness": g.heldout_fitness().map(fitness_json),
        }),
        None => Value::Null,
    }
}

/// Measure the base tree as genesis, or load the saved measurement.
fn genesis(cfg: &CliConfig, suite: &EvalSuite, w: &mut SelfModWorkload) -> Result<(GenesisFile, bool), String> {
    let path = cfg.genesis_path();
    if path.is_file() {
        let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let g: GenesisFile = serde_json::from_str(&text).map_err(|e| format!("{} is not valid: {e}", path.display()))?;
        if g.suite_digest != suite.digest.0 {
            return Err(format!(
                "the evaluation suite changed since genesis: {} was measured under suite `{}` digest {}, the evaluator now gives {}. \
                 The evaluator argv or a file it names (the script, a manifest) differs from the one the journal was built under; \
                 restore it, or start a new journal and genesis.json",
                path.display(),
                g.suite_id,
                g.suite_digest,
                suite.digest.0
            ));
        }
        if g.base_identity != cfg.base_identity {
            return Err(format!(
                "base_identity changed since genesis: {} says `{}`, the config says `{}`",
                path.display(),
                g.base_identity,
                cfg.base_identity
            ));
        }
        return Ok((g, false));
    }
    let journal = Journal::open(&cfg.journal).map_err(|e| format!("{}: {e}", cfg.journal.display()))?;
    if !journal.is_empty() {
        return Err(format!(
            "the journal {} has records but {} is missing; genesis cannot be rebuilt without re-measuring, which would break replay",
            cfg.journal.display(),
            path.display()
        ));
    }
    let d = w.materialize(&CandidateSpec::new("genesis", Vec::new()))?;
    if !w.materialized(&d) {
        let why = w.report(&d).and_then(|r| r.failure.clone()).unwrap_or_else(|| "no failure was recorded".into());
        return Err(format!("the base tree cannot be the champion, it did not build and pass its tests: {why}"));
    }
    let fitness = w.evaluate(&d, suite)?;
    if let Some(r) = w.report(&d).filter(|r| r.eval_failed) {
        return Err(format!(
            "the base tree cannot be the champion, the evaluator failed on it: {}",
            r.failure.clone().unwrap_or_default()
        ));
    }
    let shadow_successes = (0..cfg.shadow_runs).filter(|_| w.shadow(&d).success).count() as u32;
    let g = GenesisFile {
        digest: d.0,
        base_identity: cfg.base_identity.clone(),
        fitness: fitness.0,
        suite_id: suite.id.clone(),
        suite_digest: suite.digest.0.clone(),
        shadow_successes,
    };
    let text = serde_json::to_string_pretty(&g).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok((g, true))
}

/// Write the champion's tree to `dir`, which must not exist: the base tree with
/// the champion's genes applied. A candidate is a complete change set against the
/// base, so a proposer that starts from this tree and writes every file it differs
/// from the base in keeps the champion's edits.
fn export_champion(cfg: &CliConfig, store: &BlobStore, lineage: &Lineage, dir: &Path) -> Result<Vec<String>, String> {
    if dir.exists() {
        return Err(format!("--export-champion: {} already exists", dir.display()));
    }
    let champ = lineage.champion().ok_or("the journal has no champion")?;
    let genome = champ.genome.clone().ok_or("the champion has no genome in the journal")?;
    let files = super::changed_files(&genome)?;
    super::copy_tree(&cfg.base_tree, dir)?;
    for (path, sha) in &files {
        let bytes = store.get(sha).ok_or_else(|| format!("gene `{path}` is not in the blob store"))?;
        super::write_gene(dir, path, &bytes)?;
    }
    Ok(files.into_iter().map(|(p, s)| format!("{p}:{s}")).collect())
}

fn execute(cfg: &CliConfig, requested: u32, export: Option<&Path>) -> Result<String, String> {
    let suite = evaluator_suite(cfg);
    let store = BlobStore::open(&cfg.blob_store)?;
    let mut w = SelfModWorkload::new(cfg.selfmod_config(&suite), store.clone())?;
    let ep = Episode::open(cfg.gate(), suite.digest.clone(), EVALUATOR);
    let at = Attestor::new(EVALUATOR, attest_key().into_bytes());

    // Genesis, measured once and then only read back.
    let (g, created) = genesis(cfg, &suite, &mut w)?;
    let mut journal = Journal::open(&cfg.journal).map_err(|e| format!("{}: {e}", cfg.journal.display()))?;
    if journal.is_empty() {
        journal
            .append(Entry::Note { text: format!("selfmod genesis {} under suite {}", &g.digest[..12.min(g.digest.len())], suite.digest.short()) })
            .map_err(|e| format!("cannot write the journal: {e}"))?;
    }
    let genesis_digest = Digest(g.digest.clone());
    let mut base = Lineage::new();
    let id = base.next_id();
    let mut gen = Generation::new(id, genesis_digest.clone()).measured(Measurement {
        suite: suite.clone(),
        fitness: FitnessVector(g.fitness.clone()),
        evaluator: EVALUATOR.into(),
    });
    gen.genome = Some(Vec::new());
    base.add(gen);
    // Genesis proved itself when it was measured (or the first invocation would
    // have refused it); this process has not built it, so say so to the gate.
    let approval = ep
        .approve(base.get(id).expect("genesis was just added"), &base, g.shadow_successes, &|d| d == &genesis_digest)
        .map_err(|v| format!("genesis is refused by the gate: {:?}", v.reasons()))?;
    base.promote(approval).map_err(|e| e.to_string())?;

    let resumed = Lineage::replay(base, &journal, None)
        .map_err(|e| format!("cannot resume from {}: {e}", cfg.journal.display()))?;

    let mut report = json!({
        "ok": true,
        "genesis_created": created,
        "suite_digest": suite.digest.0,
        "journal": cfg.journal.display().to_string(),
        "cycles_requested": requested,
        "cycles_run": 0,
        "promotions": 0,
        "refusals": 0,
        "halt": null,
        "cycles": [],
        "proposer_errors": [],
    });
    let finish = |mut report: Value, lineage: &Lineage, head: Option<&Digest>| {
        report["champion"] = champion_json(lineage);
        report["journal_head"] = json!(head.map(|d| d.0.clone()));
        serde_json::to_string(&report).map_err(|e| e.to_string())
    };

    if let Some(dir) = export {
        let files = export_champion(cfg, &store, &resumed.lineage, dir)?;
        report["exported"] = json!({"dir": dir.display().to_string(), "files": files});
        return finish(report, &resumed.lineage, journal.head());
    }

    // Nothing to propose: do not start a cycle. An empty proposal halts the
    // runner as a workload failure, which would put a failure in the journal
    // that nothing failed to cause.
    if count_candidates(&cfg.outbox) == 0 {
        report["no_candidates"] = json!(true);
        report["message"] = json!(format!("no candidates in the outbox {}; no cycle was run", cfg.outbox.display()));
        return finish(report, &resumed.lineage, journal.head());
    }

    // Bring the champion back on disk. The workload's artifact cache is memory
    // only, so after a replay it knows no artifact until it is built again.
    let champ = resumed.lineage.champion().ok_or("the journal has no champion")?;
    let champ_id = champ.id;
    let genome = champ.genome.clone().ok_or_else(|| format!("champion {champ_id} has no genome in the journal"))?;
    let rebuilt = w.materialize(&CandidateSpec::new(format!("champion-{champ_id}"), genome))?;
    if rebuilt != champ.artifact {
        let why = w.report(&rebuilt).and_then(|r| r.failure.clone()).map(|f| format!(" ({f})")).unwrap_or_default();
        return Err(format!(
            "champion {champ_id} was journalled as artifact {} but rebuilding its genome gives {}{why}; \
             the base_identity, the blob store or the write set differs from the one the journal was built under",
            champ.artifact.short(),
            rebuilt.short()
        ));
    }
    if !w.materialized(&rebuilt) {
        let why = w.report(&rebuilt).and_then(|r| r.failure.clone()).unwrap_or_else(|| "no failure was recorded".into());
        return Err(format!("champion {champ_id} no longer builds and passes its tests, so it cannot be run or fallen back to: {why}"));
    }
    let champ_fitness = champ.heldout_fitness().cloned();
    if let Some(f) = &champ_fitness {
        w.note_axes(f.axes().map(str::to_string));
    }

    // One valid candidate per cycle, and no more consumed than will be run.
    let mut dir = DirProposer::new(&cfg.outbox, &cfg.proposer_name, store, &cfg.base_tree).with_limit(requested as usize);
    let specs = dir.propose(champ_fitness.as_ref(), cfg.seed);
    report["proposer_errors"] = json!(dir.last_errors());
    if specs.is_empty() {
        report["no_candidates"] = json!(true);
        report["message"] = json!("the outbox has candidate directories but none is usable (see proposer_errors); no cycle was run");
        return finish(report, &resumed.lineage, journal.head());
    }
    let cycles = specs.len() as u32;

    let policy = RunnerPolicy {
        name: "selfmod-cli".into(),
        max_cycles: cycles,
        // The agent decides when to stop, not a refusal count carried across invocations.
        max_consecutive_refusals: u32::MAX,
        candidates_per_cycle: 1,
        shadow_runs: cfg.shadow_runs,
        supervision: SupervisionPolicy { window: cfg.shadow_runs.max(1), ..SupervisionPolicy::default() },
        ..RunnerPolicy::default()
    };
    let queue = Queue { name: cfg.proposer_name.clone(), specs };
    let runner = Runner::new(policy, &ep, &at, suite.clone(), cfg.seed).resume(&resumed).with_proposer(Box::new(queue));
    let mut runner = runner;
    let mut lineage = resumed.lineage;
    let rr = runner.run(&mut lineage, &mut journal, &mut w);

    // A refused candidate is not kept in the lineage, so what each candidate
    // was and measured is read from the journal this run wrote.
    let mut seen: BTreeMap<u64, (Digest, Vec<String>, Option<Value>)> = BTreeMap::new();
    if let Ok(records) = journal.verified() {
        for r in records {
            match r.entry {
                Entry::Proposed { generation, artifact, definitions, .. } => {
                    seen.insert(generation.0, (artifact, definitions, None));
                }
                Entry::Evaluated { generation, fitness, .. } => {
                    if let Some(e) = seen.get_mut(&generation.0) {
                        e.2 = Some(serde_json::from_str(&fitness).unwrap_or(Value::String(fitness)));
                    }
                }
                _ => {}
            }
        }
    }
    let cycle_reports: Vec<Value> = rr
        .cycles
        .iter()
        .map(|c| {
            let known = c.candidate.and_then(|id| seen.get(&id.0));
            let rec = known.and_then(|(a, _, _)| w.report(a));
            json!({
                "cycle": c.cycle,
                "generation": c.candidate.map(|id| id.0),
                "promoted": c.promoted,
                "fitness": known.and_then(|k| k.2.clone()),
                "reasons": c.reasons,
                "failure": rec.and_then(|r| r.failure.clone()),
                "refused_before_build": rec.is_some_and(|r| r.refused),
                "files": known.map(|k| k.1.clone()).unwrap_or_default(),
            })
        })
        .collect();
    let halted_badly = matches!(rr.halt, Halt::WorkloadFailed { .. } | Halt::NoFallbackAvailable { .. });
    report["ok"] = json!(!halted_badly);
    report["cycles_run"] = json!(rr.cycles.len());
    report["promotions"] = json!(rr.promotions);
    report["refusals"] = json!(rr.refusals);
    report["halt"] = serde_json::to_value(&rr.halt).map_err(|e| e.to_string())?;
    report["cycles"] = json!(cycle_reports);
    report["proposer_errors"] = json!(dir.last_errors());
    if halted_badly {
        report["error"] = report["halt"].clone();
    }
    finish(report, &lineage, rr.journal_head.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVAL_OK: &str = "cap=$(awk 'END{print NR/10}' \"$1\")\necho noise\necho \"{\\\"capability\\\": $cap, \\\"safety\\\": 1}\"\n";

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
        fn new(name: &str) -> Fx {
            Fx::with(name, "mkdir -p out && cat src/*.txt > out/bin.txt", "grep -q ok src/lib.txt")
        }

        fn with(name: &str, build: &str, test: &str) -> Fx {
            let root = std::env::temp_dir().join(format!("germline-selfmod-cli-{name}-{}", super::super::unique()));
            let _ = std::fs::remove_dir_all(&root);
            put(&root.join("base/src/lib.txt"), "ok\nb\n");
            put(&root.join("base/tests/expected.txt"), "x\n");
            put(&root.join("eval/eval.sh"), EVAL_OK);
            let config = json!({
                "base_tree": "base",
                "base_identity": "base-v1",
                "writable": ["src/"],
                "protected": ["src/frozen/"],
                "scratch_root": "scratch",
                "build": {"argv": ["sh", "-c", build], "timeout_secs": 60},
                "test": {"argv": ["sh", "-c", test], "timeout_secs": 60},
                "evaluator": {"argv": ["sh", fwd(&root.join("eval/eval.sh"))], "timeout_secs": 60},
                "binary": "out/bin.txt",
                "blob_store": "state/blobs",
                "outbox": "outbox",
                "journal": "state/journal.jsonl",
                "proposer_name": "agent",
            });
            put(&root.join("run.json"), &serde_json::to_string_pretty(&config).unwrap());
            Fx { root }
        }

        fn run(&self) -> Result<Value, String> {
            let args = vec!["--config".to_string(), fwd(&self.root.join("run.json")), "--cycles".into(), "1".into()];
            let out = run(&args)?;
            let last = out.lines().last().unwrap().to_string();
            Ok(serde_json::from_str(&last).unwrap_or_else(|e| panic!("last line is not JSON: {e}: {out}")))
        }

        fn candidate(&self, id: &str, files: &[(&str, &str)]) {
            let dir = self.root.join("outbox").join(id);
            for (p, c) in files {
                put(&dir.join("files").join(p), c);
            }
            put(&dir.join("manifest.json"), "{\"metric\":\"capability\",\"direction\":\"increase\",\"magnitude\":0.2}");
        }

        fn genesis_json(&self) -> String {
            std::fs::read_to_string(self.root.join("state/genesis.json")).unwrap()
        }
    }

    impl Drop for Fx {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn capability(v: &Value) -> f64 {
        v["champion"]["fitness"]["capability"].as_f64().unwrap()
    }

    #[test]
    fn the_first_run_makes_genesis_and_a_journal_and_runs_no_cycle_on_an_empty_outbox() {
        let fx = Fx::new("empty");
        let r = fx.run().unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["no_candidates"], true, "{r}");
        assert_eq!(r["cycles_run"], 0);
        assert_eq!(r["genesis_created"], true);
        assert!(r["message"].as_str().unwrap().contains("no candidates"));
        assert!(fx.root.join("state/journal.jsonl").is_file(), "the journal exists");
        let g: Value = serde_json::from_str(&fx.genesis_json()).unwrap();
        assert!(g["digest"].is_string() && g["suite_digest"].is_string() && g["suite_id"].is_string());
        assert!((g["fitness"]["capability"].as_f64().unwrap() - 0.2).abs() < 1e-9, "{g}");
        assert!((capability(&r) - 0.2).abs() < 1e-9);
        // Again: resumes, still nothing to do, genesis untouched.
        let before = fx.genesis_json();
        let r = fx.run().unwrap();
        assert_eq!(r["genesis_created"], false);
        assert_eq!(r["no_candidates"], true);
        assert_eq!(before, fx.genesis_json());
    }

    #[test]
    fn candidates_are_promoted_across_invocations_and_the_champion_persists() {
        let fx = Fx::new("promote");
        fx.run().unwrap();
        fx.candidate("c1", &[("src/lib.txt", "ok\n1\n2\n3\n")]);
        let r = fx.run().unwrap();
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["cycles_run"], 1, "{r}");
        assert_eq!(r["promotions"], 1, "{r}");
        assert_eq!(r["cycles"][0]["promoted"], true);
        assert_eq!(r["cycles"][0]["generation"], 1);
        assert!((capability(&r) - 0.4).abs() < 1e-9, "{r}");
        assert!(r["champion"]["files"][0].as_str().unwrap().starts_with("src/lib.txt:"));
        assert_eq!(r["champion"]["generation"], 1);

        let genesis = fx.genesis_json();
        fx.candidate("c2", &[("src/lib.txt", "ok\n1\n2\n3\n4\n5\n")]);
        let r = fx.run().unwrap();
        assert_eq!(r["promotions"], 1, "{r}");
        assert_eq!(r["cycles"][0]["generation"], 2, "ids continue from the journal: {r}");
        assert!((capability(&r) - 0.6).abs() < 1e-9, "{r}");
        assert_eq!(r["champion"]["generation"], 2);
        assert_eq!(genesis, fx.genesis_json(), "genesis is never re-measured");
        // The outbox is drained and an invocation with nothing in it keeps the champion.
        let r = fx.run().unwrap();
        assert_eq!(r["no_candidates"], true);
        assert_eq!(r["champion"]["generation"], 2);
    }

    #[test]
    fn the_champion_exports_as_base_plus_its_edits_and_runs_no_cycle() {
        let fx = Fx::new("export");
        fx.run().unwrap();
        fx.candidate("c1", &[("src/lib.txt", "ok\n1\n2\n3\n")]);
        fx.run().unwrap();
        let dir = fx.root.join("exported");
        let args = vec![
            "--config".to_string(),
            fwd(&fx.root.join("run.json")),
            "--export-champion".into(),
            fwd(&dir),
        ];
        let out = run(&args).unwrap();
        let r: Value = serde_json::from_str(out.lines().last().unwrap()).unwrap();
        assert_eq!(r["cycles_run"], 0, "{r}");
        assert!(r["exported"]["files"][0].as_str().unwrap().starts_with("src/lib.txt:"), "{r}");
        let lib = std::fs::read_to_string(dir.join("src/lib.txt")).unwrap();
        assert_eq!(lib.replace('\r', ""), "ok\n1\n2\n3\n", "the champion's edit");
        assert!(dir.join("tests/expected.txt").is_file(), "the rest of the base");
        assert!(run(&args).unwrap_err().contains("already exists"), "never overwrites");
    }

    #[test]
    fn a_write_set_violation_is_refused_and_the_champion_is_unchanged() {
        let fx = Fx::new("violation");
        fx.run().unwrap();
        fx.candidate("c1", &[("src/lib.txt", "ok\n1\n2\n3\n")]);
        fx.run().unwrap();
        fx.candidate("c2-evil", &[("src/lib.txt", "ok\n1\n2\n3\n4\n5\n6\n7\n8\n"), ("tests/expected.txt", "rigged\n")]);
        let r = fx.run().unwrap();
        assert_eq!(r["ok"], true, "a refusal is not a failure: {r}");
        assert_eq!(r["cycles_run"], 1);
        assert_eq!(r["promotions"], 0);
        assert_eq!(r["refusals"], 1);
        assert_eq!(r["cycles"][0]["promoted"], false);
        assert_eq!(r["cycles"][0]["refused_before_build"], true, "{r}");
        assert!(r["cycles"][0]["failure"].as_str().unwrap().contains("tests/expected.txt"), "{r}");
        assert_eq!(r["champion"]["generation"], 1);
        assert!((capability(&r) - 0.4).abs() < 1e-9);
    }

    #[test]
    fn a_candidate_whose_tests_fail_is_refused_with_its_failure_text() {
        let fx = Fx::new("badtests");
        fx.run().unwrap();
        fx.candidate("c1", &[("src/lib.txt", "bad\n1\n2\n3\n4\n5\n6\n7\n8\n")]);
        let r = fx.run().unwrap();
        assert_eq!(r["promotions"], 0, "{r}");
        assert!(r["cycles"][0]["failure"].as_str().unwrap().contains("tests"), "{r}");
        assert_eq!(r["champion"]["generation"], 0);
    }

    #[test]
    fn a_changed_evaluator_between_invocations_is_refused() {
        let fx = Fx::new("evalchange");
        fx.run().unwrap();
        put(&fx.root.join("eval/eval.sh"), &format!("{EVAL_OK}# tuned\n"));
        fx.candidate("c1", &[("src/lib.txt", "ok\n1\n2\n3\n")]);
        let e = fx.run().unwrap_err();
        assert!(e.contains("suite changed"), "{e}");
        assert!(e.contains("evaluator"), "{e}");
        assert!(fx.root.join("outbox/c1").is_dir(), "a refused resume consumes nothing");
    }

    #[test]
    fn a_broken_base_cannot_be_the_champion() {
        let fx = Fx::with("badbase", "mkdir -p out && cat src/*.txt > out/bin.txt", "false");
        let e = fx.run().unwrap_err();
        assert!(e.contains("base tree cannot be the champion"), "{e}");
        assert!(e.contains("tests"), "the recorded failure is named: {e}");
        assert!(!fx.root.join("state/genesis.json").exists());
    }

    #[test]
    fn arguments_are_checked() {
        assert!(run(&[]).unwrap_err().contains("--config"));
        assert!(run(&["--config".into(), "x".into(), "--cycles".into(), "0".into()]).is_err());
        assert!(run(&["--wat".into()]).unwrap_err().contains("unknown"));
        assert!(run(&["--help".into()]).unwrap().contains("usage"));
        assert!(run(&["--config".into(), "does-not-exist.json".into()]).is_err());
    }
}
