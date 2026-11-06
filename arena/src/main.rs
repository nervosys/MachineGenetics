//! `arena` — run the self-play arena and report its front.
//!
//! ```text
//! arena --heldout <file>[,<file>…] [--rounds N] [--programs N] [--seed S]
//!       [--agents grammar:2,uniform:1,mutator:1]
//!       [--learners orders:log2buckets:lr[,…]] [--fuel F] [--cpu-watts W]
//!       [--json <out.json>]
//! ```
//!
//! Held-out files are read once, hashed, and never shown to an agent. The
//! summary goes to stderr; `--json` writes the full report.

use arena::arena::{default_meters, run, Config, Corpus};
use arena::learner::LearnerConfig;

fn fail(msg: impl std::fmt::Display) -> ! {
    eprintln!("arena: {msg}");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut cfg = Config::default();
    let mut heldout_paths: Vec<String> = Vec::new();
    let mut json_out: Option<String> = None;
    let mut i = 0;
    let value = |i: usize| -> &str { args.get(i + 1).map(String::as_str).unwrap_or_else(|| fail(format!("{} needs a value", args[i]))) };
    let num = |s: &str, what: &str| -> u64 { s.parse().unwrap_or_else(|_| fail(format!("{what}: `{s}` is not a number"))) };
    while i < args.len() {
        match args[i].as_str() {
            "--heldout" => heldout_paths.extend(value(i).split(',').map(str::to_string)),
            "--rounds" => cfg.rounds = num(value(i), "--rounds") as usize,
            "--programs" => cfg.programs_per_round = num(value(i), "--programs") as usize,
            "--seed" => cfg.seed = num(value(i), "--seed"),
            "--fuel" => cfg.substrate.fuel = num(value(i), "--fuel"),
            "--cpu-watts" => cfg.cpu_watts = value(i).parse().unwrap_or_else(|_| fail("--cpu-watts: not a number")),
            "--json" => json_out = Some(value(i).to_string()),
            "--agents" => {
                cfg.agents = value(i)
                    .split(',')
                    .map(|p| {
                        let (k, n) = p.split_once(':').unwrap_or((p, "1"));
                        (k.to_string(), num(n, "--agents") as usize)
                    })
                    .collect();
            }
            "--learners" => {
                cfg.learners = value(i)
                    .split(',')
                    .map(|p| {
                        let f: Vec<&str> = p.split(':').collect();
                        if f.len() != 3 {
                            fail(format!("--learners: `{p}` is not orders:log2buckets:lr"));
                        }
                        LearnerConfig {
                            orders: num(f[0], "orders") as usize,
                            log2_buckets: num(f[1], "log2buckets") as u32,
                            lr: f[2].parse().unwrap_or_else(|_| fail(format!("lr `{}`", f[2]))),
                            ..LearnerConfig::default()
                        }
                    })
                    .collect();
            }
            "-h" | "--help" => {
                eprintln!("{}", include_str!("main.rs").lines().skip(2).take(7).map(|l| l.trim_start_matches("//! ")).collect::<Vec<_>>().join("\n"));
                std::process::exit(0);
            }
            other => fail(format!("unknown argument `{other}`")),
        }
        i += if matches!(args[i].as_str(), "-h" | "--help") { 1 } else { 2 };
    }
    if heldout_paths.is_empty() {
        fail("--heldout is required: without held-out data there is nothing to measure intelligence against");
    }
    let heldout: Vec<Corpus> = heldout_paths
        .iter()
        .map(|p| {
            let bytes = std::fs::read(p).unwrap_or_else(|e| fail(format!("{p}: {e}")));
            Corpus::new(p.clone(), bytes)
        })
        .collect();

    let (meters, notes) = default_meters(cfg.cpu_watts);
    for n in &notes {
        eprintln!("meter: {n}");
    }
    let report = run(cfg, heldout, meters, notes).unwrap_or_else(|e| fail(e));

    eprintln!("\n=== agents (credit per unit cost drives the budget) ===");
    for a in &report.agents {
        eprintln!(
            "  {:<12} proposed {:>5}  produced {:>5}  credit {:>10.4}  score {:>10.3e}  last share {:>3}",
            a.id, a.proposed, a.produced, a.credit, a.score, a.last_share
        );
    }
    eprintln!(
        "\n  distinct outputs {} of {} ({:.0}%), distinct shapes {}, cache hits {}",
        report.distinct_outputs,
        report.total_outputs,
        100.0 * report.distinct_outputs as f64 / report.total_outputs.max(1) as f64,
        report.distinct_shapes,
        report.cache_hits
    );
    eprintln!("\n=== refusals ===");
    for (k, v) in &report.refusals {
        eprintln!("  {k:<10} {v}");
    }
    eprintln!("\n=== learners (front: bits/byte, joules, latency — all minimised) ===");
    for l in &report.learners {
        eprintln!(
            "  #{} {:>2}×2^{:<2} lr {:<5} bpb {:.3}  joules {}  s/KB {:.2e}  bits/byte/J {}  {}",
            l.index,
            l.config.orders,
            l.config.log2_buckets,
            l.config.lr,
            l.mean_bpb,
            l.train_joules.map(|j| format!("{j:.2}")).unwrap_or_else(|| "n/a".into()),
            l.latency_s_per_kb,
            l.intelligence_per_joule.map(|x| format!("{x:.4}")).unwrap_or_else(|| "n/a".into()),
            if l.on_front { "front" } else { "" }
        );
    }
    eprintln!("\n  energy basis (last span):");
    for r in &report.energy_bases {
        eprintln!("    {} → {:?}", r.meter, r.basis);
    }
    eprintln!("\n=== best programs ===");
    for (agent, reward, src) in &report.best_programs {
        // The body is the line after the signature; `@role(…)` and the
        // signature come first, so do not count lines — find the signature.
        let body = src.lines().skip_while(|l| !l.starts_with("f gen")).nth(1).unwrap_or("").trim();
        eprintln!("  [{agent}, progress {reward:.4}] {body}");
    }
    if let Some(path) = json_out {
        let json = serde_json::to_string_pretty(&report).unwrap_or_else(|e| fail(e));
        std::fs::write(&path, json).unwrap_or_else(|e| fail(format!("{path}: {e}")));
        eprintln!("\nreport written to {path}");
    }
}
