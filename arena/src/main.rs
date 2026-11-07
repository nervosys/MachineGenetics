//! `arena` — run the self-play arena and report its front.
//!
//! ```text
//! arena measure [--axis bytes|joules] [--target T] [--json OUT] <run.json | dir>…
//! arena --heldout <file>[,<file>…] [--rounds N] [--programs N] [--seed S]
//!       [--agents grammar:2,uniform:1,mutator:1,attacker:1]
//!       [--learners ng:orders:log2buckets:lr | tf:d:layers:heads:ctx:lr[:gpu] [,…]]
//!       [--fuel F] [--cpu-watts W] [--meter-gpus <i>[,<i>…] | none] [--cost joules|fuel]
//!       [--allocate share|hypervolume] [--novelty behaviour|shape]
//!       [--length-unit tokens|bytes] [--canary <string>]
//!       [--energy-budget J] [--joules-per-program J] [--definitions <forge registry dir>]
//!       [--heldout-slices K] [--refresh-drift X]
//!       [--proposer-command "<cmd> <args…>"] [--proposer-timeout S]
//!       [--json <out.json>]
//! ```
//!
//! Held-out files are read once, hashed, and never shown to an agent. The
//! summary goes to stderr; `--json` writes the full report.

use arena::arena::{meters_for, run, Config, Corpus};
use arena::learner::{LearnerConfig, LearnerSpec, TransformerConfig};

fn fail(msg: impl std::fmt::Display) -> ! {
    eprintln!("arena: {msg}");
    std::process::exit(2);
}

fn prior_lz(n: usize, sub: arena::substrate::Substrate) {
    use arena::grammar::{Policy, Rng, LIST_RULES};
    use arena::substrate::Outcome;
    let base = Policy::default();
    let mut old = Policy::default();
    // The nine productions before `copy`, `cycle` and `nest`.
    for l in old.list_logits.iter_mut().skip(9) {
        *l = -1e9;
    }
    for (name, policy) in [("original 9 productions", &old), (&*format!("all {}", LIST_RULES.len()), &base)] {
        let mut rng = Rng(2026);
        let (mut lz, mut len, mut ok) = (0.0, 0usize, 0usize);
        for _ in 0..n {
            let (p, _) = policy.sample(&mut rng);
            // Like with like: LZ76 is biased on short inputs, and the two
            // vocabularies produce different lengths, so score only outputs of
            // at least 32 bytes, truncated to exactly 32.
            if let Outcome::Bytes { bytes, .. } = sub.run(&p.source(), rng.next_u64()) {
                if bytes.len() >= 32 {
                    lz += arena::measure::lz_bits_per_byte(&bytes[..32]);
                    len += bytes.len();
                    ok += 1;
                }
            }
        }
        eprintln!(
            "{name:<24} {ok}/{n} produced >= 32 bytes; mean length {:.1}; mean LZ76 {:.3} bits/byte",
            len as f64 / ok.max(1) as f64,
            lz / ok.max(1) as f64
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // `arena measure …` reads run reports and prints fits and multipliers (plan M1–M2).
    if args.first().map(String::as_str) == Some("measure") {
        if let Err(e) = arena::transfer::measure_cli(&args[1..]) {
            fail(e);
        }
        return;
    }
    let mut cfg = Config::default();
    let mut heldout_paths: Vec<String> = Vec::new();
    let mut json_out: Option<String> = None;
    let mut meter_gpus: Option<Vec<u32>> = None;
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
            "--length-charge" => {
                cfg.length_charge = value(i).parse().unwrap_or_else(|_| fail("--length-charge: not a number"))
            }
            "--entropy-floor" => {
                cfg.entropy_floor = value(i).parse().unwrap_or_else(|_| fail("--entropy-floor: not a number"))
            }
            "--reward" => {
                cfg.reward = match value(i) {
                    "alignment" => arena::arena::Reward::Alignment,
                    "compression" => arena::arena::Reward::Compression,
                    other => fail(format!("--reward: `{other}` is neither alignment nor compression")),
                }
            }
            "--meter-gpus" => {
                meter_gpus = Some(match value(i) {
                    "none" => Vec::new(),
                    v => v.split(',').map(|g| num(g, "--meter-gpus") as u32).collect(),
                })
            }
            "--reward-program" => cfg.reward_program = Some(value(i).to_string()),
            "--allocation-program" => cfg.allocation_program = Some(value(i).to_string()),
            "--retention-program" => cfg.retention_program = Some(value(i).to_string()),
            "--energy-budget" => {
                cfg.energy_budget = Some(value(i).parse().unwrap_or_else(|_| fail("--energy-budget: not a number of joules")))
            }
            "--joules-per-program" => {
                cfg.joules_per_program =
                    Some(value(i).parse().unwrap_or_else(|_| fail("--joules-per-program: not a number of joules")))
            }
            "--definitions" => cfg.definitions_dir = Some(value(i).to_string()),
            "--cost" => {
                cfg.cost_unit = match value(i) {
                    "joules" => arena::arena::CostUnit::Joules,
                    "fuel" => arena::arena::CostUnit::Fuel,
                    other => fail(format!("--cost: `{other}` is neither joules nor fuel")),
                }
            }
            "--canary" => cfg.canary = Some(value(i).to_string()),
            "--heldout-slices" => {
                let k = num(value(i), "--heldout-slices");
                if k == 0 {
                    fail("--heldout-slices: must be at least 1 (1 means no slicing)");
                }
                cfg.heldout_slices = k as usize;
            }
            "--refresh-drift" => {
                let x: f64 = value(i).parse().unwrap_or_else(|_| fail("--refresh-drift: not a number"));
                if !x.is_finite() || x < 0.0 {
                    fail("--refresh-drift: must be finite and at least 0");
                }
                cfg.refresh_drift = x;
            }
            "--length-unit" => {
                cfg.length_unit = match value(i) {
                    "tokens" => arena::arena::LengthUnit::Tokens,
                    "bytes" => arena::arena::LengthUnit::Bytes,
                    other => fail(format!("--length-unit: `{other}` is neither tokens nor bytes")),
                }
            }
            "--novelty" => {
                cfg.novelty = match value(i) {
                    "shape" => arena::arena::NoveltyKey::Shape,
                    "behaviour" => arena::arena::NoveltyKey::Behaviour,
                    other => fail(format!("--novelty: `{other}` is neither shape nor behaviour")),
                }
            }
            "--allocate" => {
                cfg.allocation = match value(i) {
                    "share" => arena::arena::Allocation::Share,
                    "hypervolume" => arena::arena::Allocation::Hypervolume,
                    other => fail(format!("--allocate: `{other}` is neither share nor hypervolume")),
                }
            }
            "--learner-steps" => cfg.learner_steps = num(value(i), "--learner-steps") as usize,
            "--eval-every" => cfg.eval_every = num(value(i), "--eval-every").max(1) as usize,
            "--cpu-watts" => cfg.cpu_watts = value(i).parse().unwrap_or_else(|_| fail("--cpu-watts: not a number")),
            "--json" => json_out = Some(value(i).to_string()),
            // The command's argv is the value split on whitespace, so it
            // cannot carry an argument with a space in it.
            "--proposer-command" => {
                cfg.proposer_command = Some(value(i).split_whitespace().map(str::to_string).collect())
            }
            "--proposer-timeout" => {
                let s = num(value(i), "--proposer-timeout");
                if s == 0 {
                    fail("--proposer-timeout: must be at least 1 second");
                }
                cfg.proposer_timeout_s = s;
            }
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
                let float = |s: &str| -> f64 { s.parse().unwrap_or_else(|_| fail(format!("`{s}` is not a number"))) };
                cfg.learners = value(i)
                    .split(',')
                    .map(|p| {
                        let f: Vec<&str> = p.split(':').collect();
                        match f.as_slice() {
                            ["tf", d, l, h, ctx, lr, rest @ ..] => LearnerSpec::Transformer(TransformerConfig {
                                d: num(d, "d") as usize,
                                layers: num(l, "layers") as usize,
                                heads: num(h, "heads") as usize,
                                ctx: num(ctx, "ctx") as usize,
                                lr: float(lr),
                                gpu: rest.first() == Some(&"gpu"),
                            }),
                            ["ng", o, b, lr] | [o, b, lr] => LearnerSpec::Ngram(LearnerConfig {
                                orders: num(o, "orders") as usize,
                                log2_buckets: num(b, "log2buckets") as u32,
                                lr: float(lr),
                            }),
                            _ => fail(format!(
                                "--learners: `{p}` is neither ng:orders:log2buckets:lr nor tf:d:layers:heads:ctx:lr[:gpu]"
                            )),
                        }
                    })
                    .collect();
            }
            // Vocabulary diagnostic: the structure of what the uniform prior
            // writes, with and without the structural productions. No run.
            "--prior-lz" => {
                prior_lz(num(value(i), "--prior-lz") as usize, cfg.substrate);
                std::process::exit(0);
            }
            "-h" | "--help" => {
                eprintln!("{}", include_str!("main.rs").lines().skip(2).take(10).map(|l| l.trim_start_matches("//! ")).collect::<Vec<_>>().join("\n"));
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

    let (meters, notes) = meters_for(cfg.cpu_watts, meter_gpus.as_deref());
    for n in &notes {
        eprintln!("meter: {n}");
    }
    let report = run(cfg, heldout, meters, notes).unwrap_or_else(|e| fail(e));

    eprintln!("\n=== agents (credit per unit cost drives the budget) ===");
    for a in &report.agents {
        eprintln!(
            "  {:<12} proposed {:>5}  produced {:>5}  credit {:>10.4}  charged {:>9.1} J  score {:>10.3e}  last share {:>3}",
            a.id, a.proposed, a.produced, a.credit, a.joules, a.score, a.last_share
        );
    }
    eprintln!(
        "\n  distinct outputs {} of {} ({:.0}%), distinct shapes {}, cache hits {}, mean output LZ {:.3} bits/byte",
        report.distinct_outputs,
        report.total_outputs,
        100.0 * report.distinct_outputs as f64 / report.total_outputs.max(1) as f64,
        report.distinct_shapes,
        report.cache_hits,
        report.mean_output_lz_bits
    );
    match &report.fuel_calibration {
        Some(c) => eprintln!(
            "fuel: {:.3e} J/fuel + {:.3} J/program overhead, r²={:.2} over {} programs ({})",
            c.joules_per_fuel,
            c.overhead_joules,
            c.r_squared,
            c.samples,
            if c.all_measured { "measured" } else { "includes estimates" }
        ),
        None => eprintln!("fuel: not calibrated (too few rounds, or fuel never varied)"),
    }
    let e = &report.energy;
    eprintln!(
        "energy: {:.1} J charged ({}) against {}, evaluation {:.1} J uncharged, {} rounds run{}",
        e.total(),
        e.charged.iter().map(|(k, v)| format!("{k} {v:.1}")).collect::<Vec<_>>().join(", "),
        e.budget_joules.map(|b| format!("a budget of {b:.1} J")).unwrap_or_else(|| "no budget".into()),
        e.evaluation_joules,
        report.rounds_completed,
        e.halted.as_ref().map(|h| format!(" (halted: {h})")).unwrap_or_default()
    );
    if report.attack_claims > 0 {
        eprintln!(
            "\nattacks: {} of {} output-preserving claims were credited as novel",
            report.attacks.len(),
            report.attack_claims
        );
    }
    eprintln!("\n=== refusals ===");
    for (k, v) in &report.refusals {
        eprintln!("  {k:<10} {v}");
    }
    eprintln!("\n=== learners (front: bits/byte, joules, latency — all minimised) ===");
    for l in &report.learners {
        eprintln!(
            "  #{} {:<40} params {:>9}  bpb {:.3} (pool {:.3})  joules {}  s/KB {:.2e}  bits/byte/J {}  {}",
            l.index,
            l.describe,
            l.parameters,
            l.mean_bpb,
            l.pool_bpb,
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
    for (agent, reward, src, _) in &report.best_programs {
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
