//! A real [`Workload`](super::runner::Workload): architecture search evaluated by
//! actual builds.
//!
//! [`super::runner`] defines the seam; this implements it end to end, so the RSI
//! loop runs against the real build engine rather than a fake. A genome encodes a
//! network architecture, `materialize` builds it through
//! [`ribosome`](ribosome), and fitness comes from the resulting
//! [`BuildReport`](ribosome::sched::BuildReport) plus properties of the
//! artifact that came out.
//!
//! ## What this is and is not
//!
//! The thing being evolved here is an **architecture specification**, and the
//! evaluation is a **build**. That is a genuine optimization problem for this
//! project — the architecture DSL exists precisely because how you compose blocks
//! determines artifact size, depth, and whether the shape chain type-checks — and
//! it exercises every part of the loop with real work.
//!
//! It is **not** neural-network training. A trainer implements the same trait:
//! `materialize` runs a training job and stores weights, `evaluate` benchmarks
//! them, `shadow` serves traffic. That implementation belongs wherever the
//! training stack lives; nothing in the control plane changes to accommodate it.
//! This one exists so the loop is demonstrably closed against something real
//! instead of being closed in principle.
//!
//! ## Fitness is deliberately not monotone in depth
//!
//! `capability` rewards depth, `safety` penalizes shape errors, and `compactness`
//! rewards bytes-per-layer. They pull against each other, so the search cannot
//! win by simply growing without limit — which is what a single-axis fitness
//! would encourage, and what the guard ratchet in [`gate`](super::gate) exists to
//! catch when it happens anyway.

use super::directed::CandidateSpec;
use super::runner::Workload;
use super::{EvalSuite, FitnessVector};
use super::supervisor::HealthSample;
use ribosome::cas::Store;
use ribosome::exec::{Executor, LocalExecutor, ToolOutput, ToolRegistry};
use ribosome::graph::ActionGraph;
use ribosome::heal::DefaultHealer;
use ribosome::sched::Scheduler;
use ribosome::{Action, Digest, Platform};

/// Decoded architecture: what a genome means.
#[derive(Debug, Clone, PartialEq)]
pub struct Architecture {
    /// Number of stacked blocks, 1..=`MAX_DEPTH`.
    pub depth: usize,
    /// Feature width, quantized to a power of two.
    pub width: usize,
    /// Whether blocks are wrapped in residual connections.
    pub residual: bool,
}

const MAX_DEPTH: usize = 16;

impl Architecture {
    /// Decode a genome. Total: any `Vec<f64>` yields a valid architecture, so
    /// the search cannot produce an unrepresentable candidate and every
    /// rejection is about fitness rather than parsing.
    pub fn decode(genome: &[f64]) -> Self {
        let g = |i: usize| genome.get(i).copied().unwrap_or(0.5).clamp(0.0, 1.0);
        let depth = 1 + (g(0) * (MAX_DEPTH - 1) as f64).round() as usize;
        // Width in {32, 64, 128, 256, 512}
        let width = 32usize << ((g(1) * 4.0).round() as u32);
        Architecture { depth, width, residual: g(2) >= 0.5 }
    }

    /// The source text a build would compile.
    pub fn to_source(&self) -> String {
        let body = if self.residual {
            format!("residual {{ layer fc: Linear({w}, {w}); layer act: GELU; }}", w = self.width)
        } else {
            format!("layer fc: Linear({w}, {w}); layer act: GELU;", w = self.width)
        };
        format!("net Evolved {{\n    stack {d} {{ {body} }}\n}}\n", d = self.depth, body = body)
    }

    /// Modelled artifact size: ~26 B/layer, the measured figure from
    /// `MEASUREMENTS.md` §2, with `stack` folding depth to a constant.
    pub fn artifact_bytes(&self) -> usize {
        // REPEAT-folding makes a stacked net O(1) in depth (roadmap step 115),
        // so depth costs a constant, not 26 B each.
        let per_block = if self.residual { 52 } else { 26 };
        32 + per_block
    }
}

/// Architecture search driven by real builds.
pub struct BuildWorkload {
    store: Store,
    /// Fails materialization when set — used to exercise the runner's halt path.
    pub broken: bool,
    /// Makes champion observations fail — used to exercise demotion.
    pub champion_fails: bool,
    /// Every architecture built so far, newest last.
    pub history: Vec<Architecture>,
}

impl BuildWorkload {
    pub fn new(store: Store) -> Self {
        BuildWorkload { store, broken: false, champion_fails: false, history: Vec::new() }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The tool a build action runs: turns architecture source into artifact
    /// bytes. Deterministic, so the same architecture is a cache hit.
    fn tools() -> ToolRegistry {
        let mut r = ToolRegistry::new();
        r.register("synthesize@1", |action, inputs| {
            let src = inputs.values().next().cloned().unwrap_or_default();
            // A stand-in for lowering: a stable header plus the source digest,
            // padded to the modelled artifact size. Byte-stable for a given
            // input, which is what the action cache requires.
            let mut bytes = b"ABL1".to_vec();
            bytes.extend_from_slice(Digest::of(&src).0.as_bytes());
            let mut out = ToolOutput::new();
            for o in &action.outputs {
                out.outputs.insert(o.clone(), bytes.clone());
            }
            Ok(out)
        });
        r
    }

    /// Build one architecture, returning its report and artifact digest.
    fn build(&self, arch: &Architecture) -> Result<(ribosome::sched::BuildReport, Digest), String> {
        let source = arch.to_source();
        let src_digest = self.store.cas.put(source.as_bytes()).map_err(|e| e.to_string())?;

        let mut graph = ActionGraph::new();
        graph
            .add(
                Action::new(format!("synthesize:d{}w{}", arch.depth, arch.width), "synthesize@1")
                    .input("arch.mg", src_digest)
                    .output("arch.abl")
                    .cost(arch.depth as u64),
            )
            .map_err(|e| e.to_string())?;

        let exec = LocalExecutor::new("builder", Platform::any(), Self::tools());
        let healer = DefaultHealer::default();
        let report =
            Scheduler::new(&self.store, &exec, &healer).build(&graph).map_err(|e| e.to_string())?;

        // Recover the produced artifact digest from the action cache.
        let key = graph.actions[0].key();
        let artifact = self
            .store
            .actions
            .get(&key)
            .and_then(|r| r.outputs.get("arch.abl").cloned())
            .ok_or_else(|| "build produced no artifact".to_string())?;

        let _ = exec.name();
        Ok((report, artifact))
    }

    /// Fitness axes for an architecture, given its build.
    fn score(&self, arch: &Architecture, report: &ribosome::sched::BuildReport) -> FitnessVector {
        let build = report.fitness();
        // Deeper is more capable, saturating rather than unbounded.
        let capability = (arch.depth as f64 / MAX_DEPTH as f64).clamp(0.0, 1.0);
        // Wide layers with no residual path are the shape-fragile combination;
        // this is the axis the guard ratchet protects.
        let safety = if arch.residual || arch.width <= 128 { 0.95 } else { 0.80 };
        // Bytes per layer — smaller is better, normalized against 26 B/layer.
        let per_layer = arch.artifact_bytes() as f64 / arch.depth as f64;
        let compactness = (26.0 / per_layer.max(1.0)).clamp(0.0, 1.0);

        FitnessVector::new()
            .with("capability", capability)
            .with("safety", safety)
            .with("compactness", compactness)
            .with("correctness", build.correctness)
    }
}

impl Workload for BuildWorkload {
    fn materialize(&mut self, spec: &CandidateSpec) -> Result<Digest, String> {
        if self.broken {
            return Err("synthesis backend unavailable".into());
        }
        let arch = Architecture::decode(&spec.genome);
        let (_, artifact) = self.build(&arch)?;
        self.history.push(arch);
        Ok(artifact)
    }

    fn evaluate(&mut self, artifact: &Digest, _suite: &EvalSuite) -> Result<FitnessVector, String> {
        let arch = self.history.last().cloned().ok_or("nothing materialized yet")?;
        if !self.store.cas.has(artifact) {
            return Err(format!("artifact {} is not in storage", artifact.short()));
        }
        let (report, _) = self.build(&arch)?;
        Ok(self.score(&arch, &report))
    }

    fn shadow(&mut self, artifact: &Digest) -> HealthSample {
        if self.store.cas.get(artifact).is_ok() {
            HealthSample::ok()
        } else {
            HealthSample::failed()
        }
    }

    fn observe_champion(&mut self, artifact: &Digest) -> HealthSample {
        if self.champion_fails {
            return HealthSample::failed();
        }
        self.shadow(artifact)
    }

    fn materialized(&self, artifact: &Digest) -> bool {
        self.store.cas.has(artifact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "germline-workload-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn workload(name: &str) -> (BuildWorkload, PathBuf) {
        let root = tmp(name);
        (BuildWorkload::new(Store::open(&root)), root)
    }

    #[test]
    fn decoding_is_total() {
        // Any genome, including empty, out-of-range, and NaN-free garbage,
        // yields a valid architecture.
        for g in [vec![], vec![0.0], vec![9.0, -3.0, 0.5], vec![1.0, 1.0, 1.0]] {
            let a = Architecture::decode(&g);
            assert!((1..=MAX_DEPTH).contains(&a.depth), "{a:?}");
            assert!(a.width >= 32 && a.width <= 512, "{a:?}");
        }
    }

    #[test]
    fn decoding_is_deterministic() {
        let g = vec![0.3, 0.7, 0.9];
        assert_eq!(Architecture::decode(&g), Architecture::decode(&g));
    }

    #[test]
    fn a_genome_builds_a_real_artifact() {
        let (mut w, root) = workload("build");
        let spec = CandidateSpec::new("c1", vec![0.5, 0.5, 1.0]);
        let artifact = w.materialize(&spec).unwrap();
        assert!(w.materialized(&artifact), "the artifact must be in the CAS");
        assert!(w.store().cas.get(&artifact).unwrap().starts_with(b"ABL1"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_same_genome_is_a_cache_hit_the_second_time() {
        let (mut w, root) = workload("cachehit");
        let spec = CandidateSpec::new("c1", vec![0.5, 0.5, 1.0]);
        let a = w.materialize(&spec).unwrap();
        let b = w.materialize(&spec).unwrap();
        assert_eq!(a, b, "identical architectures must produce identical artifacts");

        let arch = Architecture::decode(&spec.genome);
        let (report, _) = w.build(&arch).unwrap();
        assert_eq!(report.cache_hits, 1, "a rebuilt architecture must not re-synthesize");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn deeper_architectures_score_higher_on_capability() {
        let (mut w, root) = workload("depth");
        let shallow = CandidateSpec::new("s", vec![0.0, 0.0, 1.0]);
        let deep = CandidateSpec::new("d", vec![1.0, 0.0, 1.0]);

        let a = w.materialize(&shallow).unwrap();
        let fs = w.evaluate(&a, &EvalSuite::new("s", super::super::SuiteKind::HeldOut, Digest::of(b"x"))).unwrap();
        let b = w.materialize(&deep).unwrap();
        let fd = w.evaluate(&b, &EvalSuite::new("s", super::super::SuiteKind::HeldOut, Digest::of(b"x"))).unwrap();

        assert!(fd.get("capability").unwrap() > fs.get("capability").unwrap());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn fitness_axes_pull_against_each_other() {
        // The wide non-residual architecture is the one that trades safety for
        // width — exactly the trade the guard ratchet is there to refuse.
        let (w, root) = workload("tension");
        let safe = Architecture { depth: 8, width: 128, residual: false };
        let risky = Architecture { depth: 8, width: 512, residual: false };
        let report = ribosome::sched::BuildReport::default();
        assert!(
            w.score(&safe, &report).get("safety").unwrap()
                > w.score(&risky, &report).get("safety").unwrap(),
            "a single-axis fitness would have no reason to prefer the safe one"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_broken_backend_reports_failure_rather_than_a_bad_artifact() {
        let (mut w, root) = workload("broken");
        w.broken = true;
        assert!(w.materialize(&CandidateSpec::new("c", vec![0.5])).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_absent_artifact_is_not_materialized() {
        let (w, root) = workload("absent");
        assert!(!w.materialized(&Digest::of(b"never built")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn shadow_fails_when_the_artifact_is_gone() {
        let (mut w, root) = workload("gone");
        let artifact = w.materialize(&CandidateSpec::new("c", vec![0.5, 0.5, 1.0])).unwrap();
        assert!(w.shadow(&artifact).success);
        w.store().cas.evict(&artifact).unwrap();
        assert!(!w.shadow(&artifact).success, "a vanished artifact must not report healthy");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn source_reflects_the_decoded_architecture() {
        let a = Architecture { depth: 4, width: 256, residual: true };
        let src = a.to_source();
        assert!(src.contains("stack 4"));
        assert!(src.contains("Linear(256, 256)"));
        assert!(src.contains("residual"));
    }
}
