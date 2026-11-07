//! Loss-to-loss curves and compute multipliers (plan M1–M2).
//!
//! Cartesia's *From Tokens to Bytes II*
//! (<https://www.cartesia.ai/blog/from-tokens-to-bytes-part-2>) finds that runs
//! of one architecture family fall on one smooth loss-to-loss curve, and that
//! families differ in where their curve sits. So a family is characterised by
//! its curve, not by one run: held-out loss against the loss on the training
//! distribution, across seeds and rounds ([`fit_loss_to_loss`]). And two
//! families are compared by the compute each needs to reach the same held-out
//! loss, read off each family's frontier ([`multiplier`]). That ratio is the
//! compute multiplier.
//!
//! The arena's checkpoints supply the points. `proxy_bpb` is the loss on the
//! pool's own data, which is what the loop optimises; `mean_bpb` is held-out.
//!
//! The cost axis ([`Axis`]) is either joules, which are measured and noisy, or
//! `bytes_seen`, the bytes a learner was trained on. The default is bytes: under
//! `--cost fuel` two runs of one seed agree on held-out and proxy figures and
//! on bytes, so a multiplier reproduces exactly, while joules move with the
//! machine. Bytes is a data-efficiency axis, though, not a compute one: it does
//! not charge a bigger model for its larger per-byte cost. Use joules when that
//! is the question, and read the noise off the seed spread.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A loss-to-loss fit `heldout = a + b · proxy^c` (plan M1).
///
/// The form is monotone in the proxy for any sign of `b`, because `proxy^c` is
/// monotone for `c > 0` and proxy losses are non-negative. `n` is the number of
/// points fitted. With no points every field is NaN.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fit {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    /// Root-mean-square residual, in bits per byte.
    pub rmse: f64,
    pub n: usize,
}

/// The exponents searched: 0.25 to 3.0 in steps of 0.05.
fn exponent_grid() -> impl Iterator<Item = f64> {
    (5..=60).map(|k| k as f64 * 0.05)
}

/// Fit `heldout = a + b · proxy^c` to `(proxy, heldout)` points, by least squares
/// over a grid of `c`, with `a` and `b` solved linearly at each `c`. The `c`
/// with the smallest residual sum wins. When the proxies do not vary, `b` is 0
/// and `a` is the mean held-out loss.
pub fn fit_loss_to_loss(points: &[(f64, f64)]) -> Fit {
    let n = points.len();
    if n == 0 {
        return Fit { a: f64::NAN, b: f64::NAN, c: f64::NAN, rmse: f64::NAN, n };
    }
    let my = points.iter().map(|p| p.1).sum::<f64>() / n as f64;
    let mut best: Option<(f64, Fit)> = None;
    for c in exponent_grid() {
        let xs: Vec<f64> = points.iter().map(|p| p.0.powf(c)).collect();
        let mx = xs.iter().sum::<f64>() / n as f64;
        let sxx: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
        let sxy: f64 = xs.iter().zip(points).map(|(x, p)| (x - mx) * (p.1 - my)).sum();
        // A relative floor: equal proxies can leave `sxx` a rounding error above zero.
        let b = if sxx > 1e-18 * n as f64 * (1.0 + mx * mx) { sxy / sxx } else { 0.0 };
        let a = my - b * mx;
        let sse: f64 = xs
            .iter()
            .zip(points)
            .map(|(x, p)| {
                let r = p.1 - (a + b * x);
                r * r
            })
            .sum();
        if best.map_or(true, |(s, _)| sse < s) {
            best = Some((sse, Fit { a, b, c, rmse: (sse / n as f64).sqrt(), n }));
        }
    }
    best.map(|(_, f)| f).unwrap_or_else(|| unreachable!("the exponent grid is not empty"))
}

/// The lower envelope of `(cost, held-out bits/byte)` points: those that no
/// other point beats on both cost and loss. Sorted by cost, with the loss
/// strictly falling, so each point is the cheapest way to reach its loss.
#[derive(Debug, Clone, PartialEq)]
pub struct Frontier {
    pub points: Vec<(f64, f64)>,
}

impl Frontier {
    /// The envelope of `samples`. Points with a non-positive or non-finite cost
    /// are dropped, because the interpolation is in log cost.
    pub fn new(samples: &[(f64, f64)]) -> Frontier {
        let mut s: Vec<(f64, f64)> = samples
            .iter()
            .copied()
            .filter(|(c, b)| c.is_finite() && *c > 0.0 && b.is_finite())
            .collect();
        s.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
        let mut points: Vec<(f64, f64)> = Vec::new();
        for (c, b) in s {
            if points.last().map_or(true, |&(_, best)| b < best) {
                points.push((c, b));
            }
        }
        Frontier { points }
    }

    /// The loss range the frontier covers, as `(lowest, highest)`.
    pub fn range(&self) -> Option<(f64, f64)> {
        Some((self.points.last()?.1, self.points.first()?.1))
    }

    /// The cost to reach held-out loss `t`: linear interpolation in log cost
    /// between the envelope points either side of `t`. `None` when `t` is
    /// outside the frontier's range, because the frontier then says nothing
    /// about it.
    pub fn cost_at(&self, t: f64) -> Option<f64> {
        let (lo, hi) = self.range()?;
        if !(lo..=hi).contains(&t) {
            return None;
        }
        if t >= hi {
            return Some(self.points[0].0);
        }
        for w in self.points.windows(2) {
            let ((c0, b0), (c1, b1)) = (w[0], w[1]);
            if t <= b0 && t >= b1 {
                let f = (b0 - t) / (b0 - b1);
                return Some((c0.ln() + f * (c1.ln() - c0.ln())).exp());
            }
        }
        None
    }
}

/// The compute multiplier of family `a` against `b` at held-out loss `t`:
/// `cost_a(t) / cost_b(t)`. Above 1 means `a` needs more to reach `t`. `None`
/// when `t` is outside either frontier.
pub fn multiplier(a: &Frontier, b: &Frontier, t: f64) -> Option<f64> {
    Some(a.cost_at(t)? / b.cost_at(t)?)
}

/// The cost axis a frontier is drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    /// `bytes_seen`: deterministic, and the default. See the module docs.
    #[default]
    Bytes,
    /// The checkpoint's measured joules. Checkpoints without them are skipped.
    Joules,
    /// Wall-clock seconds of training so far (plan L3). Noisy, like joules, and
    /// only carried by the physics-baseline curves; arena checkpoints lack it.
    Seconds,
}

impl Axis {
    fn cost(self, s: &Sample) -> Option<f64> {
        match self {
            Axis::Bytes => Some(s.bytes as f64),
            Axis::Joules => s.joules,
            Axis::Seconds => s.seconds,
        }
    }
}

/// One checkpoint, tagged with its learner family and the seed of its run.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub family: String,
    pub seed: u64,
    pub proxy_bpb: f64,
    pub heldout_bpb: f64,
    pub joules: Option<f64>,
    pub bytes: u64,
    /// Cumulative training seconds, where the run recorded them.
    pub seconds: Option<f64>,
}

/// A learner's family name: its kind (`ngram`, `transformer`) and its describe
/// string, which carries the configuration.
pub fn family_name(config: &serde_json::Value, describe: &str) -> String {
    // `LearnerSpec` is internally tagged, so its kind is the `kind` field.
    let kind = config
        .get("kind")
        .and_then(|k| k.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".into());
    format!("{kind} {describe}")
}

#[derive(Deserialize)]
struct RunFile {
    config: RunConfig,
    #[serde(default)]
    learners: Vec<LearnerRef>,
    checkpoints: Vec<CheckpointRef>,
}

#[derive(Deserialize)]
struct RunConfig {
    seed: u64,
}

#[derive(Deserialize)]
struct LearnerRef {
    index: usize,
    describe: String,
    config: serde_json::Value,
}

#[derive(Deserialize)]
struct CheckpointRef {
    learner: usize,
    mean_bpb: f64,
    #[serde(default)]
    proxy_bpb: f64,
    #[serde(default)]
    joules: Option<f64>,
    bytes_seen: u64,
}

/// The checkpoints of one `arena --json` report, each tagged with its family.
/// Reads only the fields it needs, so older reports still parse.
pub fn parse_run(json: &str) -> Result<Vec<Sample>, String> {
    let run: RunFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let family_of: BTreeMap<usize, String> =
        run.learners.iter().map(|l| (l.index, family_name(&l.config, &l.describe))).collect();
    run.checkpoints
        .iter()
        .map(|c| {
            let family = family_of
                .get(&c.learner)
                .cloned()
                .ok_or_else(|| format!("a checkpoint names learner {}, which the run does not list", c.learner))?;
            Ok(Sample {
                family,
                seed: run.config.seed,
                proxy_bpb: c.proxy_bpb,
                heldout_bpb: c.mean_bpb,
                joules: c.joules,
                bytes: c.bytes_seen,
                seconds: None,
            })
        })
        .collect()
}

/// The loss-to-loss fit of one learner in a run (plan M1), carried by the
/// report `arena --json` writes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerFit {
    pub learner: usize,
    pub describe: String,
    pub fit: Fit,
}

/// The fit of each learner in a run, over its checkpoints.
pub fn learner_fits(
    checkpoints: &[crate::arena::Checkpoint],
    learners: &[crate::arena::LearnerResult],
) -> Vec<LearnerFit> {
    learners
        .iter()
        .map(|l| {
            let points: Vec<(f64, f64)> =
                checkpoints.iter().filter(|c| c.learner == l.index).map(|c| (c.proxy_bpb, c.mean_bpb)).collect();
            LearnerFit { learner: l.index, describe: l.describe.clone(), fit: fit_loss_to_loss(&points) }
        })
        .collect()
}

/// One family in a measurement: its loss-to-loss fit over all its checkpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FamilyReport {
    pub family: String,
    pub n: usize,
    pub seeds: usize,
    pub fit: Fit,
}

/// The multiplier of family `a` against `b`, pooled over seeds and per seed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairReport {
    pub a: String,
    pub b: String,
    pub target: f64,
    /// The pooled frontiers' multiplier, `None` when `target` is outside one.
    pub multiplier: Option<f64>,
    /// `(seed, multiplier)`, for each seed whose own frontiers reach `target`.
    pub per_seed: Vec<(u64, f64)>,
    pub mean: Option<f64>,
    /// Sample standard deviation over `per_seed`; `None` with fewer than two.
    pub sd: Option<f64>,
}

/// The whole measurement, for the text report and for `--json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub axis: Axis,
    pub target: f64,
    pub runs: usize,
    pub checkpoints: usize,
    pub families: Vec<FamilyReport>,
    pub pairs: Vec<PairReport>,
}

fn frontier_of(samples: &[&Sample], axis: Axis) -> Frontier {
    let pts: Vec<(f64, f64)> = samples.iter().filter_map(|s| axis.cost(s).map(|c| (c, s.heldout_bpb))).collect();
    Frontier::new(&pts)
}

fn of_seed<'a>(ss: &[&'a Sample], seed: u64) -> Vec<&'a Sample> {
    ss.iter().copied().filter(|s| s.seed == seed).collect()
}

fn median(mut xs: Vec<f64>) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(f64::total_cmp);
    let m = xs.len() / 2;
    Some(if xs.len() % 2 == 1 { xs[m] } else { (xs[m - 1] + xs[m]) / 2.0 })
}

/// Mean and sample standard deviation of `xs`.
fn mean_sd(xs: &[f64]) -> (Option<f64>, Option<f64>) {
    if xs.is_empty() {
        return (None, None);
    }
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    if xs.len() < 2 {
        return (Some(mean), None);
    }
    let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0);
    (Some(mean), Some(var.sqrt()))
}

/// Measure every family and every pair of families from pooled samples.
///
/// The target defaults to the median held-out loss of all checkpoints. Each
/// pair's multiplier is `cost_a / cost_b` (plan M2) on the pooled frontiers,
/// and on each seed's own frontiers, where a seed's runs of both families are
/// pooled across variants that share it.
pub fn measure(samples: &[Sample], axis: Axis, target: Option<f64>, runs: usize) -> Measurement {
    let target = target
        .or_else(|| median(samples.iter().map(|s| s.heldout_bpb).collect()))
        .unwrap_or(f64::NAN);
    let mut by_family: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
    for s in samples {
        by_family.entry(s.family.as_str()).or_default().push(s);
    }
    let seeds: BTreeSet<u64> = samples.iter().map(|s| s.seed).collect();

    let families: Vec<FamilyReport> = by_family
        .iter()
        .map(|(name, ss)| {
            let points: Vec<(f64, f64)> = ss.iter().map(|s| (s.proxy_bpb, s.heldout_bpb)).collect();
            FamilyReport {
                family: name.to_string(),
                n: ss.len(),
                seeds: ss.iter().map(|s| s.seed).collect::<BTreeSet<_>>().len(),
                fit: fit_loss_to_loss(&points),
            }
        })
        .collect();

    let names: Vec<&str> = by_family.keys().copied().collect();
    let mut pairs = Vec::new();
    for (i, &a) in names.iter().enumerate() {
        for &b in &names[i + 1..] {
            let (fa, fb) = (frontier_of(&by_family[a], axis), frontier_of(&by_family[b], axis));
            let pooled = multiplier(&fa, &fb, target);
            let mut per_seed = Vec::new();
            for &seed in &seeds {
                let (sa, sb) = (of_seed(&by_family[a], seed), of_seed(&by_family[b], seed));
                if let Some(m) = multiplier(&frontier_of(&sa, axis), &frontier_of(&sb, axis), target) {
                    per_seed.push((seed, m));
                }
            }
            let values: Vec<f64> = per_seed.iter().map(|(_, m)| *m).collect();
            let (mean, sd) = mean_sd(&values);
            pairs.push(PairReport {
                a: a.to_string(),
                b: b.to_string(),
                target,
                multiplier: pooled,
                per_seed,
                mean,
                sd,
            });
        }
    }
    Measurement { axis, target, runs, checkpoints: samples.len(), families, pairs }
}

/// The measurement as the lines `arena measure` prints.
pub fn render(m: &Measurement) -> String {
    let mut out = String::new();
    let axis_note = match m.axis {
        Axis::Bytes => "bytes seen (deterministic)",
        Axis::Joules => "measured joules (noisy)",
        Axis::Seconds => "wall-clock seconds (noisy)",
    };
    out.push_str(&format!(
        "measure: {} runs, {} checkpoints, {} families; axis {axis_note}; target {:.4} bits/byte\n",
        m.runs,
        m.checkpoints,
        m.families.len(),
        m.target
    ));
    out.push_str("\nloss-to-loss fits, heldout = a + b · proxy^c:\n");
    for f in &m.families {
        out.push_str(&format!(
            "  {:<44} n {:>4}  seeds {:>2}  a {:>8.4}  b {:>8.4}  c {:>5.2}  rmse {:.4}\n",
            f.family, f.n, f.seeds, f.fit.a, f.fit.b, f.fit.c, f.fit.rmse
        ));
    }
    out.push_str(&format!("\ncompute multipliers at {:.4} bits/byte, cost_A / cost_B:\n", m.target));
    if m.pairs.is_empty() {
        out.push_str("  (fewer than two families: no pairs)\n");
    }
    let fmt = |x: Option<f64>| x.map(|v| format!("{v:.4}")).unwrap_or_else(|| "n/a".into());
    for p in &m.pairs {
        let seeds: Vec<String> = p.per_seed.iter().map(|(s, v)| format!("seed {s}: {v:.4}")).collect();
        out.push_str(&format!(
            "  A = {}\n  B = {}\n    pooled {}  per seed mean {} sd {} (n {})  [{}]\n",
            p.a,
            p.b,
            fmt(p.multiplier),
            fmt(p.mean),
            fmt(p.sd),
            p.per_seed.len(),
            seeds.join(", ")
        ));
    }
    out
}

const USAGE: &str = "usage: arena measure [--axis bytes|joules] [--target T] [--json OUT] <run.json | dir>...\n\
    A directory stands for the *.json files in it. Default axis: bytes (see the module docs).";

/// The files a command reads: each path as given, and each `*.json` in a
/// directory. The flag is true for files found in a directory. Those may not be
/// the kind a command reads: a `--json` written beside the runs is one of them,
/// so callers skip them with a note rather than failing. Files named on the
/// command line must be what the command reads.
fn expand_inputs(inputs: &[String]) -> Result<Vec<(std::path::PathBuf, bool)>, String> {
    let mut files: Vec<(std::path::PathBuf, bool)> = Vec::new();
    for input in inputs {
        let p = std::path::Path::new(input);
        if p.is_dir() {
            let mut found: Vec<std::path::PathBuf> = std::fs::read_dir(p)
                .map_err(|e| format!("{input}: {e}"))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|q| q.extension().is_some_and(|x| x == "json") && q.is_file())
                .collect();
            found.sort();
            files.extend(found.into_iter().map(|f| (f, true)));
        } else {
            files.push((p.to_path_buf(), false));
        }
    }
    if files.is_empty() {
        return Err(format!("no *.json files found in {}", inputs.join(", ")));
    }
    Ok(files)
}

/// `arena measure …`: read run reports, print fits and multipliers. Writes the
/// measurement as JSON too when `--json` is given.
pub fn measure_cli(args: &[String]) -> Result<(), String> {
    let mut axis = Axis::default();
    let mut target: Option<f64> = None;
    let mut json_out: Option<String> = None;
    let mut inputs: Vec<String> = Vec::new();
    let mut i = 0;
    let value = |i: usize| -> Result<&str, String> {
        args.get(i + 1).map(String::as_str).ok_or_else(|| format!("{} needs a value\n{USAGE}", args[i]))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--axis" => {
                axis = match value(i)? {
                    "bytes" => Axis::Bytes,
                    "joules" => Axis::Joules,
                    other => return Err(format!("--axis: `{other}` is neither bytes nor joules")),
                };
                i += 2;
            }
            "--target" => {
                let t: f64 = value(i)?.parse().map_err(|_| "--target: not a number".to_string())?;
                if !t.is_finite() {
                    return Err("--target: must be finite".into());
                }
                target = Some(t);
                i += 2;
            }
            "--json" => {
                json_out = Some(value(i)?.to_string());
                i += 2;
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            other if other.starts_with("--") => return Err(format!("unknown argument `{other}`\n{USAGE}")),
            path => {
                inputs.push(path.to_string());
                i += 1;
            }
        }
    }
    if inputs.is_empty() {
        return Err(format!("no run reports given\n{USAGE}"));
    }

    let files = expand_inputs(&inputs)?;
    let mut samples = Vec::new();
    let mut runs = 0;
    for (f, from_dir) in &files {
        let parsed = std::fs::read_to_string(f)
            .map_err(|e| format!("{}: {e}", f.display()))
            .and_then(|text| parse_run(&text).map_err(|e| format!("{}: {e}", f.display())));
        match (parsed, from_dir) {
            (Ok(s), _) => {
                samples.extend(s);
                runs += 1;
            }
            (Err(e), true) => eprintln!("measure: skipped {} (not a run report: {e})", f.display()),
            (Err(e), false) => return Err(e),
        }
    }
    if runs == 0 {
        return Err("no run reports among the files given".into());
    }
    let m = measure(&samples, axis, target, runs);
    print!("{}", render(&m));
    if let Some(path) = json_out {
        let json = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("{path}: {e}"))?;
        println!("measurement written to {path}");
    }
    Ok(())
}

// ---- Learner comparison (plan L3) ----
//
// The learning curves of `physics-baseline --json`, grouped by corpus. Each
// learner's multiplier is taken against the corpus's reference, the n-gram, on
// the same two frontier functions as above: its cost to reach the target over
// the reference's cost. The target is shared by every learner on a corpus, so
// no frontier is asked for a loss it cannot reach unless the target is given
// explicitly.

/// One checkpoint of a learning curve, as `physics-baseline --json` writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveCheckpoint {
    pub bytes_trained: u64,
    pub train_bpb: f64,
    pub heldout_bpb: f64,
    /// Cumulative wall-clock seconds of train steps, evaluation excluded.
    pub seconds: f64,
}

/// One learner trained on one corpus under one seed. `seed` and `encoding` are
/// `None` for a corpus no seed reaches (the text corpora): the same curve comes
/// back under every seed, so it is one run, not several.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveRun {
    pub corpus: String,
    pub seed: Option<u64>,
    pub encoding: Option<String>,
    /// The learner's describe string, which names its kind and configuration.
    pub describe: String,
    /// The learner's config. Its `kind` field says which family it belongs to.
    pub config: serde_json::Value,
    pub checkpoints: Vec<CurveCheckpoint>,
}

/// A curve file: the runs `physics-baseline --json` writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveFile {
    pub runs: Vec<CurveRun>,
}

/// The runs of one curve file. Unknown fields are ignored.
pub fn parse_curves(json: &str) -> Result<Vec<CurveRun>, String> {
    let f: CurveFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(f.runs)
}

/// The reference learner kind: the n-gram, which every multiplier is against.
const REFERENCE_KIND: &str = "ngram";

/// The samples of one curve, one per trained checkpoint. The untrained start is
/// not a point on a cost axis, so it is dropped. A seed-independent curve is
/// filed under seed 0.
fn curve_samples(run: &CurveRun) -> Vec<Sample> {
    run.checkpoints
        .iter()
        .filter(|c| c.bytes_trained > 0)
        .map(|c| Sample {
            family: run.describe.clone(),
            seed: run.seed.unwrap_or(0),
            proxy_bpb: c.train_bpb,
            heldout_bpb: c.heldout_bpb,
            joules: None,
            bytes: c.bytes_trained,
            seconds: Some(c.seconds),
        })
        .collect()
}

/// A learner's multiplier against the reference, on one cost axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relative {
    /// On the pooled frontiers. `None` when the target is outside one of them.
    pub pooled: Option<f64>,
    /// `(seed, multiplier)`, for each seed whose own frontiers reach the target.
    pub per_seed: Vec<(u64, f64)>,
    pub mean: Option<f64>,
    /// Sample standard deviation over `per_seed`; `None` with fewer than two.
    pub sd: Option<f64>,
}

/// One learner in a corpus: M1's fit, and its multipliers against the reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveRow {
    pub family: String,
    /// Checkpoints fitted, pooled over seeds.
    pub n: usize,
    /// The loss-to-loss fit, with train bits per byte as the proxy and held-out
    /// bits per byte as the target.
    pub fit: Fit,
    /// `None` for the reference itself.
    pub bytes: Option<Relative>,
    pub seconds: Option<Relative>,
}

/// One corpus: its learners, and the target every multiplier is read at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusReport {
    pub corpus: String,
    /// True when no seed reaches the corpus, so there is one run per learner.
    pub seed_independent: bool,
    pub seeds: Vec<u64>,
    pub reference: String,
    pub target: f64,
    /// True when `target` is the default: the worst final held-out bits per byte
    /// among this corpus's learners, so that every frontier reaches it.
    pub target_is_default: bool,
    pub rows: Vec<CurveRow>,
}

/// The whole curve measurement, for the text report and for `--json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurveMeasurement {
    pub curves: usize,
    pub corpora: Vec<CorpusReport>,
}

/// The multipliers of `fam` against `reference` on one axis, at `target`: pooled
/// over `seeds`, and for each seed on its own.
fn relative(fam: &[&Sample], reference: &[&Sample], axis: Axis, target: f64, seeds: &[u64]) -> Relative {
    let pooled = multiplier(&frontier_of(fam, axis), &frontier_of(reference, axis), target);
    let mut per_seed = Vec::new();
    for &seed in seeds {
        let (f, r) = (of_seed(fam, seed), of_seed(reference, seed));
        if let Some(m) = multiplier(&frontier_of(&f, axis), &frontier_of(&r, axis), target) {
            per_seed.push((seed, m));
        }
    }
    let values: Vec<f64> = per_seed.iter().map(|(_, m)| *m).collect();
    let (mean, sd) = mean_sd(&values);
    Relative { pooled, per_seed, mean, sd }
}

/// A seed-independent corpus is run once per learner, whatever seeds the sweep
/// used. Its repeated runs must agree on every bits-per-byte figure, or the
/// claim that no seed reaches it is wrong, and this says so. Timings are not
/// compared. The first run is kept.
fn dedupe_seed_independent<'a>(corpus: &str, runs: &[&'a CurveRun]) -> Result<Vec<&'a CurveRun>, String> {
    let mut kept: Vec<&CurveRun> = Vec::new();
    for &r in runs {
        if r.seed.is_some() {
            kept.push(r);
            continue;
        }
        match kept.iter().find(|k| k.seed.is_none() && k.describe == r.describe) {
            Some(first) => {
                let same = first.checkpoints.len() == r.checkpoints.len()
                    && first.checkpoints.iter().zip(&r.checkpoints).all(|(a, b)| {
                        a.bytes_trained == b.bytes_trained && a.train_bpb == b.train_bpb && a.heldout_bpb == b.heldout_bpb
                    });
                if !same {
                    return Err(format!(
                        "corpus `{corpus}`: two runs of `{}` with no seed disagree on bits per byte, so the corpus is not seed-independent",
                        r.describe
                    ));
                }
            }
            None => kept.push(r),
        }
    }
    Ok(kept)
}

/// Measure every corpus in the runs. The target is `target` when given, else the
/// worst final held-out bits per byte among the corpus's learners and seeds.
/// Each corpus needs exactly one n-gram learner, which is its reference.
pub fn measure_curves(runs: &[CurveRun], target: Option<f64>) -> Result<CurveMeasurement, String> {
    let mut corpora: Vec<&str> = Vec::new();
    for r in runs {
        if !corpora.contains(&r.corpus.as_str()) {
            corpora.push(r.corpus.as_str());
        }
    }
    let mut reports = Vec::new();
    for corpus in corpora {
        let mine: Vec<&CurveRun> = runs.iter().filter(|r| r.corpus == corpus).collect();
        let kept = dedupe_seed_independent(corpus, &mine)?;
        let seed_independent = kept.iter().all(|r| r.seed.is_none());
        if !seed_independent && kept.iter().any(|r| r.seed.is_none()) {
            return Err(format!("corpus `{corpus}`: some runs have a seed and some do not"));
        }
        let mut seen: BTreeSet<(&str, u64)> = BTreeSet::new();
        for r in &kept {
            if let Some(s) = r.seed {
                if !seen.insert((r.describe.as_str(), s)) {
                    return Err(format!("corpus `{corpus}`: `{}` appears twice under seed {s}", r.describe));
                }
            }
        }

        let (target_value, target_is_default) = match target {
            Some(t) => (t, false),
            None => {
                let finals: Vec<f64> = kept
                    .iter()
                    .map(|r| {
                        r.checkpoints
                            .last()
                            .map(|c| c.heldout_bpb)
                            .ok_or_else(|| format!("corpus `{corpus}`: `{}` has no checkpoints", r.describe))
                    })
                    .collect::<Result<_, _>>()?;
                (finals.into_iter().fold(f64::NEG_INFINITY, f64::max), true)
            }
        };

        let samples: Vec<Sample> = kept.iter().flat_map(|r| curve_samples(r)).collect();
        let seeds: Vec<u64> = samples.iter().map(|s| s.seed).collect::<BTreeSet<_>>().into_iter().collect();
        let mut by_family: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
        for s in &samples {
            by_family.entry(s.family.as_str()).or_default().push(s);
        }

        let references: BTreeSet<&str> = kept
            .iter()
            .filter(|r| r.config.get("kind").and_then(|k| k.as_str()) == Some(REFERENCE_KIND))
            .map(|r| r.describe.as_str())
            .collect();
        let reference = match references.len() {
            1 => *references.iter().next().expect("one reference"),
            0 => return Err(format!("corpus `{corpus}`: no {REFERENCE_KIND} learner, so there is nothing to measure against")),
            n => return Err(format!("corpus `{corpus}`: {n} {REFERENCE_KIND} learners, and the reference must be one")),
        };
        let ref_samples = by_family.get(reference).cloned().unwrap_or_default();

        let rows = by_family
            .iter()
            .map(|(&fam, ss)| {
                let points: Vec<(f64, f64)> = ss.iter().map(|s| (s.proxy_bpb, s.heldout_bpb)).collect();
                let is_ref = fam == reference;
                CurveRow {
                    family: fam.to_string(),
                    n: ss.len(),
                    fit: fit_loss_to_loss(&points),
                    bytes: (!is_ref).then(|| relative(ss, &ref_samples, Axis::Bytes, target_value, &seeds)),
                    seconds: (!is_ref).then(|| relative(ss, &ref_samples, Axis::Seconds, target_value, &seeds)),
                }
            })
            .collect();
        reports.push(CorpusReport {
            corpus: corpus.to_string(),
            seed_independent,
            seeds,
            reference: reference.to_string(),
            target: target_value,
            target_is_default,
            rows,
        });
    }
    Ok(CurveMeasurement { curves: runs.len(), corpora: reports })
}

/// `x` to three places, or `n/a`.
fn opt3(x: Option<f64>) -> String {
    x.map(|v| format!("{v:.3}")).unwrap_or_else(|| "n/a".into())
}

/// One multiplier's line: the pooled figure, then the per-seed mean ± sd.
fn relative_line(r: &Relative, seed_independent: bool) -> String {
    let pooled = format!("pooled {}", opt3(r.pooled));
    if seed_independent {
        return format!("{pooled}  (one run: no seed reaches this corpus)");
    }
    let spread = match (r.mean, r.sd) {
        (None, _) => "per seed n/a".to_string(),
        (Some(m), Some(sd)) => format!("per seed {m:.3} ± {sd:.3} (n {})", r.per_seed.len()),
        (Some(m), None) => format!("per seed {m:.3} (n {}, no sd)", r.per_seed.len()),
    };
    let each: Vec<String> = r.per_seed.iter().map(|(s, v)| format!("seed {s} {v:.3}")).collect();
    format!("{pooled}  |  {spread}  [{}]", each.join(", "))
}

/// The curve measurement as the lines `arena measure-curves` prints.
pub fn render_curves(m: &CurveMeasurement) -> String {
    let mut out = format!(
        "measure-curves: {} learning curves, {} corpora\n\
         multiplier = cost of the learner / cost of the reference (its n-gram) to reach the target held-out\n\
         bits per byte; above 1 means the learner needs more. Bytes trained is deterministic; seconds are\n\
         wall-clock and noisy. A multiplier of n/a means a frontier does not reach the target.\n",
        m.curves,
        m.corpora.len()
    );
    for c in &m.corpora {
        let seeds = if c.seed_independent {
            "one run per learner (seed-independent)".to_string()
        } else {
            format!("seeds {}", c.seeds.iter().map(u64::to_string).collect::<Vec<_>>().join(", "))
        };
        let target_note = if c.target_is_default { "default: worst final held-out of its learners" } else { "--target" };
        out.push_str(&format!(
            "\ncorpus: {}  [{seeds}]\n  reference {}; target {:.4} bits/byte ({target_note})\n",
            c.corpus, c.reference, c.target
        ));
        for r in &c.rows {
            out.push_str(&format!(
                "  {}\n    M1 fit (train bpb → held-out bpb): a {:.4}  b {:.4}  c {:.2}  rmse {:.4}  (n {})\n",
                r.family, r.fit.a, r.fit.b, r.fit.c, r.fit.rmse, r.n
            ));
            match (&r.bytes, &r.seconds) {
                (Some(b), Some(s)) => out.push_str(&format!(
                    "    bytes   {}\n    seconds {}\n",
                    relative_line(b, c.seed_independent),
                    relative_line(s, c.seed_independent)
                )),
                _ => out.push_str("    (reference)\n"),
            }
        }
    }
    out
}

const CURVE_USAGE: &str = "usage: arena measure-curves [--target T] [--json OUT] <curves.json | dir>...\n\
    Reads the curve files `arena physics-baseline --json` writes. A directory stands for the *.json files in it,\n\
    and files there that are not curve files are skipped with a note. The reference is each corpus's n-gram.";

/// `arena measure-curves …` (plan L3): read learning curves, print each learner's
/// multiplier against the n-gram on each corpus, on bytes and on seconds.
pub fn measure_curves_cli(args: &[String]) -> Result<(), String> {
    let mut target: Option<f64> = None;
    let mut json_out: Option<String> = None;
    let mut inputs: Vec<String> = Vec::new();
    let mut i = 0;
    let value = |i: usize| -> Result<&str, String> {
        args.get(i + 1).map(String::as_str).ok_or_else(|| format!("{} needs a value\n{CURVE_USAGE}", args[i]))
    };
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                let t: f64 = value(i)?.parse().map_err(|_| "--target: not a number".to_string())?;
                if !t.is_finite() {
                    return Err("--target: must be finite".into());
                }
                target = Some(t);
                i += 2;
            }
            "--json" => {
                json_out = Some(value(i)?.to_string());
                i += 2;
            }
            "-h" | "--help" => {
                println!("{CURVE_USAGE}");
                return Ok(());
            }
            other if other.starts_with("--") => return Err(format!("unknown argument `{other}`\n{CURVE_USAGE}")),
            path => {
                inputs.push(path.to_string());
                i += 1;
            }
        }
    }
    if inputs.is_empty() {
        return Err(format!("no curve files given\n{CURVE_USAGE}"));
    }
    let mut runs: Vec<CurveRun> = Vec::new();
    let mut files_read = 0;
    for (f, from_dir) in expand_inputs(&inputs)? {
        let parsed = std::fs::read_to_string(&f)
            .map_err(|e| format!("{}: {e}", f.display()))
            .and_then(|text| parse_curves(&text).map_err(|e| format!("{}: {e}", f.display())));
        match (parsed, from_dir) {
            (Ok(r), _) => {
                runs.extend(r);
                files_read += 1;
            }
            (Err(e), true) => eprintln!("measure-curves: skipped {} (not a curve file: {e})", f.display()),
            (Err(e), false) => return Err(e),
        }
    }
    if files_read == 0 {
        return Err("no curve files among the files given".into());
    }
    let m = measure_curves(&runs, target)?;
    print!("{}", render_curves(&m));
    if let Some(path) = json_out {
        let json = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("{path}: {e}"))?;
        println!("curve measurement written to {path}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noiseless(a: f64, b: f64, c: f64) -> Vec<(f64, f64)> {
        (1..=40).map(|k| {
            let p = k as f64 * 0.2;
            (p, a + b * p.powf(c))
        }).collect()
    }

    #[test]
    fn fit_recovers_a_known_curve_from_noiseless_points() {
        let f = fit_loss_to_loss(&noiseless(1.0, 2.0, 1.5));
        assert!((f.a - 1.0).abs() < 1e-3, "{f:?}");
        assert!((f.b - 2.0).abs() < 1e-3, "{f:?}");
        assert!((f.c - 1.5).abs() < 1e-3, "{f:?}");
        assert!(f.rmse < 1e-6, "{f:?}");
        assert_eq!(f.n, 40);
    }

    #[test]
    fn fit_on_noisy_points_has_rmse_about_the_noise() {
        // Uniform noise on ±h has standard deviation h / √3.
        let h = 0.02;
        let mut rng = crate::grammar::Rng(7);
        let pts: Vec<(f64, f64)> = (0..2000)
            .map(|k| {
                let p = 1.0 + (k as f64) * 0.003;
                let u = rng.next_u64() as f64 / u64::MAX as f64;
                (p, 1.0 + 2.0 * p.powf(1.5) + (2.0 * u - 1.0) * h)
            })
            .collect();
        let sigma = h / 3f64.sqrt();
        let f = fit_loss_to_loss(&pts);
        assert!(f.rmse > 0.7 * sigma && f.rmse < 1.3 * sigma, "rmse {} vs sigma {sigma}", f.rmse);
    }

    #[test]
    fn fit_with_no_points_is_nan_and_with_equal_proxies_is_flat() {
        assert_eq!(fit_loss_to_loss(&[]).n, 0);
        assert!(fit_loss_to_loss(&[]).a.is_nan());
        let f = fit_loss_to_loss(&[(2.0, 3.0), (2.0, 3.5)]);
        assert_eq!(f.b, 0.0);
        assert!((f.a - 3.25).abs() < 1e-12);
    }

    #[test]
    fn frontier_interpolates_exactly_in_log_cost_and_ignores_dominated_points() {
        // Log-linear between the envelope points: 100 → 1000 → 10000 as the loss falls 4 → 3 → 2.
        let f = Frontier::new(&[(100.0, 4.0), (1000.0, 3.0), (10000.0, 2.0), (5000.0, 3.5), (50.0, 4.5)]);
        assert_eq!(f.points, vec![(50.0, 4.5), (100.0, 4.0), (1000.0, 3.0), (10000.0, 2.0)]);
        let at = |t: f64| f.cost_at(t).unwrap();
        assert!((at(3.5) / 10f64.powf(2.5) - 1.0).abs() < 1e-12, "{}", at(3.5));
        assert!((at(2.5) / 10f64.powf(3.5) - 1.0).abs() < 1e-12, "{}", at(2.5));
        assert!((at(3.0) - 1000.0).abs() < 1e-9);
        assert!((at(4.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn a_frontier_returns_none_outside_its_range() {
        let f = Frontier::new(&[(100.0, 4.0), (1000.0, 3.0)]);
        assert_eq!(f.range(), Some((3.0, 4.0)));
        assert!(f.cost_at(4.5).is_none());
        assert!(f.cost_at(2.9).is_none());
        assert!(f.cost_at(f64::NAN).is_none());
        assert!(Frontier::new(&[]).cost_at(3.0).is_none());
    }

    #[test]
    fn a_family_against_itself_has_multiplier_one() {
        let f = Frontier::new(&[(100.0, 4.0), (1000.0, 3.0), (10000.0, 2.0)]);
        for t in [2.2, 2.7, 3.0, 3.6, 4.0] {
            assert!((multiplier(&f, &f, t).unwrap() - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn a_family_made_twice_as_costly_has_multiplier_two_in_range() {
        let base = [(100.0, 4.0), (1000.0, 3.0), (10000.0, 2.0)];
        let costly: Vec<(f64, f64)> = base.iter().map(|&(c, b)| (2.0 * c, b)).collect();
        let (a, b) = (Frontier::new(&costly), Frontier::new(&base));
        for t in [2.1, 2.5, 3.0, 3.9, 4.0] {
            let m = multiplier(&a, &b, t).unwrap();
            assert!((m - 2.0).abs() < 1e-12, "t {t}: {m}");
        }
        assert!(multiplier(&a, &b, 4.5).is_none());
    }

    #[test]
    fn the_target_outside_either_frontier_gives_no_multiplier() {
        let a = Frontier::new(&[(100.0, 4.0), (1000.0, 3.0)]);
        let b = Frontier::new(&[(100.0, 3.5), (1000.0, 2.5)]);
        // a covers 3.0..4.0 and b covers 2.5..3.5, so 3.7 is inside a and outside b.
        assert!(multiplier(&a, &b, 3.7).is_none());
        assert!(multiplier(&a, &b, 3.2).is_some());
    }

    const RUN_SEED_1: &str = r#"{
        "config": {"seed": 1},
        "learners": [
            {"index": 0, "describe": "ngram 3×2^16 lr0.01", "config": {"kind": "ngram", "orders": 3, "log2_buckets": 16, "lr": 0.01}},
            {"index": 1, "describe": "transformer d64 L2 h4 ctx128 lr0.001 on cpu", "config": {"kind": "transformer", "d": 64}}
        ],
        "checkpoints": [
            {"round": 1, "learner": 0, "mean_bpb": 7.5, "proxy_bpb": 7.0, "joules": 1.0, "bytes_seen": 100},
            {"round": 1, "learner": 1, "mean_bpb": 7.2, "proxy_bpb": 6.9, "joules": null, "bytes_seen": 100},
            {"round": 2, "learner": 0, "mean_bpb": 7.4, "proxy_bpb": 6.8, "joules": 2.0, "bytes_seen": 200}
        ]
    }"#;

    // Learner indices are per run: index 0 here is a different family from index 0 in RUN_SEED_1.
    const RUN_SEED_2: &str = r#"{
        "config": {"seed": 2, "rounds": 2},
        "learners": [
            {"index": 0, "describe": "transformer d64 L2 h4 ctx128 lr0.001 on cpu", "config": {"kind": "transformer", "d": 64}}
        ],
        "checkpoints": [
            {"round": 1, "learner": 0, "mean_bpb": 7.1, "proxy_bpb": 6.7, "joules": 3.0, "bytes_seen": 100}
        ],
        "checkpoints_note": "an unknown field is ignored"
    }"#;

    #[test]
    fn checkpoints_group_by_family_across_several_reports() {
        let mut all = parse_run(RUN_SEED_1).unwrap();
        all.extend(parse_run(RUN_SEED_2).unwrap());
        let mut by: BTreeMap<&str, Vec<&Sample>> = BTreeMap::new();
        for s in &all {
            by.entry(s.family.as_str()).or_default().push(s);
        }
        assert_eq!(by.keys().copied().collect::<Vec<_>>(), vec![
            "ngram ngram 3×2^16 lr0.01",
            "transformer transformer d64 L2 h4 ctx128 lr0.001 on cpu",
        ]);
        assert_eq!(by["ngram ngram 3×2^16 lr0.01"].len(), 2);
        let tf = &by["transformer transformer d64 L2 h4 ctx128 lr0.001 on cpu"];
        assert_eq!(tf.len(), 2);
        assert_eq!(tf.iter().map(|s| s.seed).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(tf[0].joules, None, "a null joules field reads as not measured");
    }

    #[test]
    fn a_family_is_named_by_the_kind_the_arena_really_writes() {
        // Guards the field name: the arena's report serialises the learner with its kind as a `kind` field.
        let config = serde_json::to_value(crate::learner::LearnerSpec::default()).unwrap();
        assert_eq!(family_name(&config, "ngram 3×2^16 lr0.01"), "ngram ngram 3×2^16 lr0.01");
    }

    #[test]
    fn a_checkpoint_naming_an_unlisted_learner_is_an_error() {
        let bad = RUN_SEED_1.replace("\"learner\": 1", "\"learner\": 9");
        assert!(parse_run(&bad).unwrap_err().contains("learner 9"));
    }

    #[test]
    fn measure_prints_pairs_with_pooled_and_per_seed_multipliers() {
        // Family A costs twice B's bytes at every loss, on both seeds.
        let mut samples = Vec::new();
        for seed in [1u64, 2] {
            for (k, (proxy, bpb)) in [(7.0, 4.0), (6.0, 3.0), (5.0, 2.0)].into_iter().enumerate() {
                let bytes = 100 * 10u64.pow(k as u32);
                samples.push(Sample { family: "a".into(), seed, proxy_bpb: proxy, heldout_bpb: bpb, joules: None, bytes: 2 * bytes, seconds: None });
                samples.push(Sample { family: "b".into(), seed, proxy_bpb: proxy, heldout_bpb: bpb, joules: Some(1.0), bytes, seconds: None });
            }
        }
        let m = measure(&samples, Axis::Bytes, Some(3.0), 2);
        assert_eq!(m.families.len(), 2);
        let p = &m.pairs[0];
        assert!((p.multiplier.unwrap() - 2.0).abs() < 1e-12);
        assert_eq!(p.per_seed.len(), 2);
        assert!((p.mean.unwrap() - 2.0).abs() < 1e-12);
        assert!((p.sd.unwrap()).abs() < 1e-12, "identical seeds have no spread");
        let text = render(&m);
        assert!(text.contains("pooled 2.0000"), "{text}");
        // Joules: B has them, A does not, so A has no frontier and no multiplier.
        let j = measure(&samples, Axis::Joules, Some(3.0), 2);
        assert!(j.pairs[0].multiplier.is_none());
    }

    #[test]
    fn the_default_target_is_the_median_of_the_pooled_held_out_loss() {
        let mk = |b: f64| Sample { family: "f".into(), seed: 1, proxy_bpb: 1.0, heldout_bpb: b, joules: None, bytes: 1, seconds: None };
        let m = measure(&[mk(1.0), mk(4.0), mk(2.0), mk(9.0)], Axis::Bytes, None, 1);
        assert!((m.target - 3.0).abs() < 1e-12);
    }
}

#[cfg(test)]
mod curve_tests {
    use super::*;

    /// A curve: the untrained start (8 bits, no bytes) then `pts` of
    /// `(bytes, train bpb, held-out bpb, seconds)`.
    fn curve(corpus: &str, seed: Option<u64>, describe: &str, kind: &str, pts: &[(u64, f64, f64, f64)]) -> CurveRun {
        let start = CurveCheckpoint { bytes_trained: 0, train_bpb: 8.0, heldout_bpb: 8.0, seconds: 0.0 };
        CurveRun {
            corpus: corpus.into(),
            seed,
            encoding: seed.map(|_| "planes".to_string()),
            describe: describe.into(),
            config: serde_json::json!({ "kind": kind }),
            checkpoints: std::iter::once(start)
                .chain(pts.iter().map(|&(bytes_trained, train_bpb, heldout_bpb, seconds)| CurveCheckpoint {
                    bytes_trained,
                    train_bpb,
                    heldout_bpb,
                    seconds,
                }))
                .collect(),
        }
    }

    // The reference reaches 4.0, 3.5, 3.0, 2.5 at 1,000, 2,000, 3,000, 4,000 bytes
    // and 10, 20, 30, 40 seconds. The learner reaches the same losses at twice the
    // bytes and three times the seconds, so its multipliers are exactly 2 and 3.
    const REF: [(u64, f64, f64, f64); 4] = [(1000, 4.0, 4.0, 10.0), (2000, 3.5, 3.5, 20.0), (3000, 3.0, 3.0, 30.0), (4000, 2.5, 2.5, 40.0)];
    const LEARNER: [(u64, f64, f64, f64); 4] = [(2000, 4.0, 4.0, 30.0), (4000, 3.5, 3.5, 60.0), (6000, 3.0, 3.0, 90.0), (8000, 2.5, 2.5, 120.0)];

    fn synthetic(seeds: &[u64]) -> Vec<CurveRun> {
        let mut runs = Vec::new();
        for &s in seeds {
            runs.push(curve("synthetic", Some(s), "ngram 4×2^14 lr0.02", "ngram", &REF));
            runs.push(curve("synthetic", Some(s), "ssm d16 bptt64 lr0.01", "ssm", &LEARNER));
        }
        runs
    }

    fn row<'a>(c: &'a CorpusReport, family: &str) -> &'a CurveRow {
        c.rows.iter().find(|r| r.family == family).unwrap_or_else(|| panic!("no row for {family}"))
    }

    #[test]
    fn a_known_multiplier_is_recovered_on_the_bytes_and_seconds_axes() {
        let m = measure_curves(&synthetic(&[1, 2]), Some(3.2)).unwrap();
        let c = &m.corpora[0];
        assert_eq!(c.reference, "ngram 4×2^14 lr0.02");
        assert!(!c.target_is_default);
        assert_eq!(c.seeds, vec![1, 2]);
        assert!(row(c, "ngram 4×2^14 lr0.02").bytes.is_none(), "the reference has no multiplier against itself");
        let r = row(c, "ssm d16 bptt64 lr0.01");
        let b = r.bytes.as_ref().unwrap();
        assert!((b.pooled.unwrap() - 2.0).abs() < 1e-9, "{b:?}");
        assert_eq!(b.per_seed.len(), 2);
        assert!(b.per_seed.iter().all(|(_, m)| (m - 2.0).abs() < 1e-9), "{b:?}");
        assert!((b.mean.unwrap() - 2.0).abs() < 1e-9);
        assert!(b.sd.unwrap().abs() < 1e-9, "identical seeds have no spread");
        let s = r.seconds.as_ref().unwrap();
        assert!((s.pooled.unwrap() - 3.0).abs() < 1e-9, "{s:?}");
        let text = render_curves(&m);
        assert!(text.contains("pooled 2.000"), "{text}");
        assert!(text.contains("pooled 3.000"), "{text}");
    }

    #[test]
    fn the_default_target_is_the_worst_final_held_out_loss() {
        // Both curves end at 2.5, so the default is 2.5, and the reference's last
        // point is then exactly on its frontier.
        let m = measure_curves(&synthetic(&[1]), None).unwrap();
        let c = &m.corpora[0];
        assert!(c.target_is_default);
        assert!((c.target - 2.5).abs() < 1e-12);
        let b = row(c, "ssm d16 bptt64 lr0.01").bytes.as_ref().unwrap();
        assert!((b.pooled.unwrap() - 2.0).abs() < 1e-9, "{b:?}");
    }

    #[test]
    fn a_target_outside_a_frontier_gives_no_multiplier() {
        // 1.0 is below both frontiers' lowest loss, so neither reaches it.
        let m = measure_curves(&synthetic(&[1, 2]), Some(1.0)).unwrap();
        let b = row(&m.corpora[0], "ssm d16 bptt64 lr0.01").bytes.as_ref().unwrap().clone();
        assert!(b.pooled.is_none() && b.mean.is_none() && b.per_seed.is_empty(), "{b:?}");
        let text = render_curves(&m);
        assert!(text.contains("pooled n/a"), "{text}");
        assert!(text.contains("per seed n/a"), "{text}");
    }

    #[test]
    fn the_untrained_start_is_not_a_point_on_the_curve() {
        let m = measure_curves(&synthetic(&[1, 2]), Some(3.2)).unwrap();
        // Four trained checkpoints per curve, two curves per seed, two seeds.
        assert_eq!(row(&m.corpora[0], "ngram 4×2^14 lr0.02").n, 8);
    }

    #[test]
    fn the_m1_fit_of_a_curve_recovers_a_known_loss_to_loss_curve() {
        // Held-out = 1 + 2 · train^1.5 at each checkpoint, so the fit must return it.
        let pts: Vec<(u64, f64, f64, f64)> = (1..=8u64)
            .map(|k| {
                let x = 1.0 + 0.25 * k as f64;
                (1000 * k, x, 1.0 + 2.0 * x.powf(1.5), k as f64)
            })
            .collect();
        let runs = vec![
            curve("known", Some(1), "ngram 4×2^14 lr0.02", "ngram", &REF),
            curve("known", Some(1), "ssm d16 bptt64 lr0.01", "ssm", &pts),
        ];
        let m = measure_curves(&runs, Some(3.2)).unwrap();
        let f = row(&m.corpora[0], "ssm d16 bptt64 lr0.01").fit;
        assert!((f.a - 1.0).abs() < 1e-3 && (f.b - 2.0).abs() < 1e-3 && (f.c - 1.5).abs() < 1e-6, "{f:?}");
        assert!(f.rmse < 1e-6, "{f:?}");
    }

    #[test]
    fn a_seed_independent_corpus_is_one_run_and_its_repeats_must_agree() {
        let text = |heldout_last: f64| {
            curve("text", None, "ngram 4×2^14 lr0.02", "ngram", &[(1000, 4.0, 4.0, 1.0), (2000, 3.5, heldout_last, 2.0)])
        };
        // Six seeds' sweeps each carry the same text curve.
        let mut runs: Vec<CurveRun> = (1..=6).map(|_| text(3.0)).collect();
        runs.push(curve("text", None, "ssm d16 bptt64 lr0.01", "ssm", &[(1000, 4.0, 4.0, 1.5), (2000, 3.0, 3.0, 2.5)]));
        let m = measure_curves(&runs, Some(3.2)).unwrap();
        let c = &m.corpora[0];
        assert!(c.seed_independent);
        assert_eq!(c.seeds, vec![0]);
        let r = row(c, "ssm d16 bptt64 lr0.01").bytes.as_ref().unwrap();
        assert_eq!(r.per_seed.len(), 1);
        assert!(r.sd.is_none(), "one run has no spread");
        assert!(render_curves(&m).contains("seed-independent"));
        // A repeat that disagrees on bits per byte is an error, not a silent pick.
        let mut bad = runs.clone();
        bad.push(text(3.4));
        assert!(measure_curves(&bad, Some(3.2)).unwrap_err().contains("not seed-independent"));
    }

    #[test]
    fn a_reference_must_be_exactly_one_ngram_learner() {
        let none = vec![curve("c", Some(1), "ssm d16 bptt64 lr0.01", "ssm", &LEARNER)];
        assert!(measure_curves(&none, None).unwrap_err().contains("no ngram learner"));
        let two = vec![
            curve("c", Some(1), "ngram 4×2^14 lr0.02", "ngram", &REF),
            curve("c", Some(1), "ngram 3×2^16 lr0.01", "ngram", &REF),
        ];
        assert!(measure_curves(&two, None).unwrap_err().contains("2 ngram learners"));
    }

    #[test]
    fn a_seed_appearing_twice_for_one_learner_is_an_error() {
        let mut runs = synthetic(&[1]);
        runs.extend(synthetic(&[1]));
        assert!(measure_curves(&runs, Some(3.2)).unwrap_err().contains("appears twice under seed 1"));
    }

    #[test]
    fn a_curve_file_round_trips_and_unknown_fields_are_ignored() {
        let runs = synthetic(&[1]);
        let json = serde_json::to_string(&CurveFile { runs: runs.clone() }).unwrap();
        assert_eq!(parse_curves(&json).unwrap(), runs);
        let extra = r#"{"budget": 1, "runs": [{"corpus": "c", "seed": null, "encoding": null, "describe": "d",
            "config": {"kind": "ngram"}, "extra": true,
            "checkpoints": [{"bytes_trained": 10, "train_bpb": 1.0, "heldout_bpb": 1.5, "seconds": 0.1}]}]}"#;
        let parsed = parse_curves(extra).unwrap();
        assert_eq!(parsed[0].checkpoints[0].heldout_bpb, 1.5);
        assert!(parse_curves("{\"checkpoints\": []}").is_err(), "a run report is not a curve file");
    }
}
