// cost-aware-optimizer — Cost-Model-Aware Compilation Optimizer.
//
// Evaluate multiple compilation strategies across target architectures,
// score each by latency, throughput, code size, and energy cost, then
// select the optimal strategy per target. Includes calibration from
// benchmark data and budget-aware pruning.
//
// Demonstrates:
//   - Cost modeling with multi-objective scoring
//   - Architecture-aware strategy selection
//   - Calibration from historical benchmarks
//   - Budget-constrained optimization (time, tokens, cost)
//   - Contracts for correctness invariants
//   - Pipelines for staged optimization passes

use std::col;
use std::fmt;
use std::io;

// ─────────────────────────────────────────────────────────────────────
// §1 — Target architectures and cost dimensions
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub data Architecture {
    X86_64,
    Aarch64,
    Wasm32,
    RiscV64,
}

extend Architecture {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Architecture::X86_64  => write!(f, "x86_64"),
            Architecture::Aarch64 => write!(f, "aarch64"),
            Architecture::Wasm32  => write!(f, "wasm32"),
            Architecture::RiscV64 => write!(f, "riscv64"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub data CostVector {
    latency_ms: f64,        // Execution latency.
    throughput_ops_sec: f64, // Operations per second.
    code_size_kb: f64,      // Binary size.
    energy_mj: f64,         // Energy consumption millijoules.
    compile_time_ms: f64,   // Time to compile.
}

extend CostVector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "lat={lat:.1}ms thr={thr:.0}op/s size={sz:.1}KB energy={e:.1}mJ compile={ct:.1}ms",
            lat = self.latency_ms,
            thr = self.throughput_ops_sec,
            sz = self.code_size_kb,
            e = self.energy_mj,
            ct = self.compile_time_ms)
    }
}

extend CostVector {
    /// Compute a weighted score (lower is better).
    ///
    /// @req weights.len() == 5
    /// @ens result >= 0.0
    /// @fx  pure
    pub fn score(&self, weights: &CostWeights) -> f64 {
        val lat_score = self.latency_ms * weights.latency;
        val thr_score = (1.0 / self.throughput_ops_sec.max(1.0)) * weights.throughput * 1000.0;
        val size_score = self.code_size_kb * weights.code_size;
        val energy_score = self.energy_mj * weights.energy;
        val compile_score = self.compile_time_ms * weights.compile_time;
        lat_score + thr_score + size_score + energy_score + compile_score
    }
}

#[derive(Debug, Clone, Copy)]
pub data CostWeights {
    latency: f64,
    throughput: f64,
    code_size: f64,
    energy: f64,
    compile_time: f64,
}

extend CostWeights {
    /// Cloud server profile: latency and throughput dominant.
    pub fn server() -> CostWeights {
        CostWeights {
            latency: 3.0,
            throughput: 3.0,
            code_size: 0.5,
            energy: 0.2,
            compile_time: 0.5,
        }
    }

    /// Embedded device profile: code size and energy dominant.
    pub fn embedded() -> CostWeights {
        CostWeights {
            latency: 1.0,
            throughput: 0.5,
            code_size: 4.0,
            energy: 4.0,
            compile_time: 0.3,
        }
    }

    /// Client WASM profile: code size and compile time dominant.
    pub fn wasm_client() -> CostWeights {
        CostWeights {
            latency: 2.0,
            throughput: 1.0,
            code_size: 4.0,
            energy: 0.2,
            compile_time: 3.0,
        }
    }

    /// Developer iteration profile: compile time dominant.
    pub fn dev_iteration() -> CostWeights {
        CostWeights {
            latency: 0.5,
            throughput: 0.3,
            code_size: 0.2,
            energy: 0.1,
            compile_time: 5.0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// §2 — Optimization strategies
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub data OptLevel {
    None,     // -O0
    Basic,    // -O1
    Standard, // -O2
    Full,     // -O3
    Size,     // -Os
    MinSize,  // -Oz
}

extend OptLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OptLevel::None     => write!(f, "O0"),
            OptLevel::Basic    => write!(f, "O1"),
            OptLevel::Standard => write!(f, "O2"),
            OptLevel::Full     => write!(f, "O3"),
            OptLevel::Size     => write!(f, "Os"),
            OptLevel::MinSize  => write!(f, "Oz"),
        }
    }
}

#[derive(Debug, Clone)]
pub data Strategy {
    name: String,
    opt_level: OptLevel,
    lto: bool,
    inline_threshold: u32,
    vectorize: bool,
    unroll_loops: bool,
    strip_debug: bool,
}

extend Strategy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        var flags = [&str]~.new();
        if self.lto { flags.push("lto"); }
        if self.vectorize { flags.push("vec"); }
        if self.unroll_loops { flags.push("unroll"); }
        if self.strip_debug { flags.push("strip"); }
        write!(f, "{name}({level} inline={th} {flags})",
            name = self.name,
            level = self.opt_level,
            th = self.inline_threshold,
            flags = flags.join("+"))
    }
}

/// Predefined strategy catalog.
///
/// @fx pure
fn strategy_catalog() -> [Strategy]~ {
    vec![
        Strategy {
            name: "debug".to_string(),
            opt_level: OptLevel::None,
            lto: false,
            inline_threshold: 50,
            vectorize: false,
            unroll_loops: false,
            strip_debug: false,
        },
        Strategy {
            name: "balanced".to_string(),
            opt_level: OptLevel::Standard,
            lto: false,
            inline_threshold: 200,
            vectorize: true,
            unroll_loops: false,
            strip_debug: false,
        },
        Strategy {
            name: "aggressive".to_string(),
            opt_level: OptLevel::Full,
            lto: true,
            inline_threshold: 500,
            vectorize: true,
            unroll_loops: true,
            strip_debug: true,
        },
        Strategy {
            name: "compact".to_string(),
            opt_level: OptLevel::Size,
            lto: true,
            inline_threshold: 50,
            vectorize: false,
            unroll_loops: false,
            strip_debug: true,
        },
        Strategy {
            name: "minimal".to_string(),
            opt_level: OptLevel::MinSize,
            lto: true,
            inline_threshold: 25,
            vectorize: false,
            unroll_loops: false,
            strip_debug: true,
        },
        Strategy {
            name: "fast-compile".to_string(),
            opt_level: OptLevel::Basic,
            lto: false,
            inline_threshold: 100,
            vectorize: false,
            unroll_loops: false,
            strip_debug: false,
        },
    ]
}

// ─────────────────────────────────────────────────────────────────────
// §3 — Cost oracle: predict costs per strategy × architecture
// ─────────────────────────────────────────────────────────────────────

/// Simulate cost prediction for a given strategy on a target architecture.
///
/// @req strategy is a valid strategy
/// @req arch is a valid architecture
/// @ens result latency_ms > 0.0
/// @fx   pure
fn predict_cost(strategy: &Strategy, arch: &Architecture) -> CostVector {
    // Base costs per optimization level.
    val (base_lat, base_thr, base_size, base_compile) = match strategy.opt_level {
        OptLevel::None     => (50.0,  1000.0,  200.0, 100.0),
        OptLevel::Basic    => (30.0,  3000.0,  180.0, 150.0),
        OptLevel::Standard => (15.0,  8000.0,  160.0, 300.0),
        OptLevel::Full     => (8.0,   15000.0, 250.0, 800.0),
        OptLevel::Size     => (20.0,  5000.0,  80.0,  500.0),
        OptLevel::MinSize  => (25.0,  3500.0,  50.0,  400.0),
    };

    // Architecture scaling factors.
    val (arch_lat, arch_thr, arch_size, arch_energy) = match arch {
        Architecture::X86_64  => (1.0, 1.0, 1.0, 1.0),
        Architecture::Aarch64 => (0.9, 1.1, 0.95, 0.7),
        Architecture::Wasm32  => (2.5, 0.4, 0.6, 0.3),
        Architecture::RiscV64 => (1.3, 0.7, 0.85, 0.6),
    };

    // LTO adjustments.
    val lto_factor = if strategy.lto { 0.85 } else { 1.0 };
    val lto_compile = if strategy.lto { 1.5 } else { 1.0 };
    val lto_size = if strategy.lto { 0.8 } else { 1.0 };

    // Vectorization boost.
    val vec_thr = if strategy.vectorize { 1.4 } else { 1.0 };
    val vec_size = if strategy.vectorize { 1.1 } else { 1.0 };

    // Loop unrolling.
    val unroll_thr = if strategy.unroll_loops { 1.2 } else { 1.0 };
    val unroll_size = if strategy.unroll_loops { 1.3 } else { 1.0 };

    // Strip debug info.
    val strip_size = if strategy.strip_debug { 0.7 } else { 1.0 };

    val final_lat = base_lat * arch_lat * lto_factor;
    val final_thr = base_thr * arch_thr * vec_thr * unroll_thr;
    val final_size = base_size * arch_size * lto_size * vec_size * unroll_size * strip_size;
    val final_energy = final_lat * 0.8 * arch_energy;
    val final_compile = base_compile * lto_compile;

    CostVector {
        latency_ms: final_lat,
        throughput_ops_sec: final_thr,
        code_size_kb: final_size,
        energy_mj: final_energy,
        compile_time_ms: final_compile,
    }
}

// ─────────────────────────────────────────────────────────────────────
// §4 — Calibration: adjust costs from historical benchmarks
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data BenchmarkResult {
    strategy_name: String,
    architecture: Architecture,
    measured_latency_ms: f64,
    measured_throughput: f64,
    measured_size_kb: f64,
}

#[derive(Debug, Clone)]
pub data CalibrationData {
    benchmarks: [BenchmarkResult]~,
    drift_factor: f64,  // How much predictions deviated from reality.
}

extend CalibrationData {
    pub fn new() -> CalibrationData {
        CalibrationData {
            benchmarks: []~.new(),
            drift_factor: 1.0,
        }
    }

    /// Add a benchmark measurement.
    pub fn record(&mut self, result: BenchmarkResult) {
        self.benchmarks.push(result);
    }

    /// Calculate the drift between predicted and measured.
    ///
    /// @ens self.drift_factor > 0.0
    pub fn calibrate(&mut self, strategies: &[Strategy]~) {
        if self.benchmarks.is_empty() {
            return;
        }

        var total_drift = 0.0;
        var count = 0u32;

        for bench in &self.benchmarks {
            for strat in strategies {
                if strat.name == bench.strategy_name {
                    val predicted = predict_cost(strat, &bench.architecture);
                    val lat_drift = bench.measured_latency_ms / predicted.latency_ms;
                    val thr_drift = bench.measured_throughput / predicted.throughput_ops_sec;
                    total_drift = total_drift + lat_drift + thr_drift;
                    count = count + 2;
                }
            }
        }

        if count > 0 {
            self.drift_factor = total_drift / (count as f64);
        }
    }

    /// Apply calibration correction to a predicted cost vector.
    ///
    /// @fx pure
    pub fn adjust(&self, cost: CostVector) -> CostVector {
        CostVector {
            latency_ms: cost.latency_ms * self.drift_factor,
            throughput_ops_sec: cost.throughput_ops_sec / self.drift_factor,
            code_size_kb: cost.code_size_kb,
            energy_mj: cost.energy_mj * self.drift_factor,
            compile_time_ms: cost.compile_time_ms,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// §5 — Optimizer: select best strategy per architecture
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub data OptimizationResult {
    architecture: Architecture,
    chosen_strategy: String,
    cost: CostVector,
    score: f64,
    alternatives: [(String, f64)]~, // (name, score) of runners-up.
}

extend OptimizationResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[{arch}] → {strat} (score={sc:.2}) | {cost}",
            arch = self.architecture,
            strat = self.chosen_strategy,
            sc = self.score,
            cost = self.cost)
    }
}

#[derive(Debug)]
pub data Optimizer {
    strategies: [Strategy]~,
    calibration: CalibrationData,
    budget_ceiling_ms: f64,
}

extend Optimizer {
    pub fn new(budget_ceiling_ms: f64) -> Optimizer {
        Optimizer {
            strategies: strategy_catalog(),
            calibration: CalibrationData.new(),
            budget_ceiling_ms: budget_ceiling_ms,
        }
    }

    /// Select the optimal strategy for a given architecture and weight profile.
    ///
    /// @req  !self.strategies.is_empty()
    /// @ens  result.score >= 0.0
    /// @fx   pure
    pub fn optimize(&self, arch: &Architecture, weights: &CostWeights) -> OptimizationResult {
        var results: [(String, CostVector, f64)]~ = []~.new();

        for strat in &self.strategies {
            val raw_cost = predict_cost(strat, arch);
            val cost = self.calibration.adjust(raw_cost);

            // Budget pruning: skip strategies that exceed the time ceiling.
            if cost.compile_time_ms > self.budget_ceiling_ms {
                continue;
            }

            val sc = cost.score(weights);
            results.push((strat.name.clone(), cost, sc));
        }

        // Sort by score ascending (lower = better).
        results.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        val (best_name, best_cost, best_score) = results[0].clone();

        val alts: [(String, f64)]~ = results.iter()
            .skip(1)
            .take(3)
            .map(|(name, _, sc)| (name.clone(), *sc))
            .collect();

        OptimizationResult {
            architecture: arch.clone(),
            chosen_strategy: best_name,
            cost: best_cost,
            score: best_score,
            alternatives: alts,
        }
    }

    /// Optimize all target architectures.
    pub fn optimize_all(&self, targets: &[Architecture]~, weights: &CostWeights) -> [OptimizationResult]~ {
        var results: [OptimizationResult]~ = []~.new();
        for arch in targets {
            results.push(self.optimize(arch, weights));
        }
        results
    }
}

// ─────────────────────────────────────────────────────────────────────
// §6 — Entry point: run multi-target cost-aware optimization
// ─────────────────────────────────────────────────────────────────────

pub fn main() / io {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║  MAGE Cost-Aware Compilation Optimizer                   ║");
    println!("╚═══════════════════════════════════════════════════════════╝");
    println!("");

    // Create optimizer with a 1-second compile budget.
    var optimizer = Optimizer.new(1000.0);

    // Simulate historical benchmarks for calibration.
    println!("─── Calibration ──────────────────────────────────────────");
    optimizer.calibration.record(BenchmarkResult {
        strategy_name: "balanced".to_string(),
        architecture: Architecture::X86_64,
        measured_latency_ms: 16.2,
        measured_throughput: 7800.0,
        measured_size_kb: 155.0,
    });
    optimizer.calibration.record(BenchmarkResult {
        strategy_name: "aggressive".to_string(),
        architecture: Architecture::Aarch64,
        measured_latency_ms: 7.5,
        measured_throughput: 16200.0,
        measured_size_kb: 230.0,
    });
    optimizer.calibration.record(BenchmarkResult {
        strategy_name: "compact".to_string(),
        architecture: Architecture::Wasm32,
        measured_latency_ms: 45.0,
        measured_throughput: 2100.0,
        measured_size_kb: 40.0,
    });
    optimizer.calibration.calibrate(&optimizer.strategies);
    println!("  Drift factor: {optimizer.calibration.drift_factor:.3}");
    println!("  Benchmarks ingested: {optimizer.calibration.benchmarks.len()}");
    println!("");

    // Define targets.
    val targets = vec![
        Architecture::X86_64,
        Architecture::Aarch64,
        Architecture::Wasm32,
        Architecture::RiscV64,
    ];

    // Run optimization with different weight profiles.
    val profiles: [(&str, CostWeights)]~ = vec![
        ("Server",        CostWeights::server()),
        ("Embedded",      CostWeights::embedded()),
        ("WASM Client",   CostWeights::wasm_client()),
        ("Dev Iteration", CostWeights::dev_iteration()),
    ];

    for (profile_name, weights) in &profiles {
        println!("═══ Profile: {profile_name} ══════════════════════════════");
        println!("  Weights: lat={weights.latency} thr={weights.throughput} size={weights.code_size} energy={weights.energy} compile={weights.compile_time}");
        println!("");

        val results = optimizer.optimize_all(&targets, weights);

        for result in &results {
            println!("  {result}");

            if !result.alternatives.is_empty() {
                var alt_strs: [String]~ = []~.new();
                for (name, sc) in &result.alternatives {
                    alt_strs.push(format!("{name}={sc:.2}"));
                }
                println!("    runners-up: {alt_strs.join(\", \")}");
            }
        }
        println!("");
    }

    // Strategy coverage matrix.
    println!("─── Strategy × Architecture Cost Matrix ──────────────────");
    val strategies = strategy_catalog();
    val server_weights = CostWeights::server();

    println!("  ┌──────────────┬──────────┬──────────┬──────────┬──────────┐");
    println!("  │ Strategy     │ x86_64   │ aarch64  │ wasm32   │ riscv64  │");
    println!("  ├──────────────┼──────────┼──────────┼──────────┼──────────┤");

    for strat in &strategies {
        var scores: [String]~ = []~.new();
        for arch in &targets {
            val cost = predict_cost(strat, arch);
            val cost = optimizer.calibration.adjust(cost);
            val sc = cost.score(&server_weights);
            scores.push(format!("{sc:>7.1}"));
        }
        println!("  │ {name:<12} │ {s0} │ {s1} │ {s2} │ {s3} │",
            name = strat.name,
            s0 = scores[0], s1 = scores[1], s2 = scores[2], s3 = scores[3]);
    }

    println!("  └──────────────┴──────────┴──────────┴──────────┴──────────┘");
    println!("");

    // Budget analysis.
    println!("─── Budget Analysis ──────────────────────────────────────");
    println!("  Compile-time ceiling: {optimizer.budget_ceiling_ms}ms");
    println!("  Strategies within budget:");
    for strat in &strategies {
        val cost = predict_cost(strat, &Architecture::X86_64);
        val within = if cost.compile_time_ms <= optimizer.budget_ceiling_ms { "✓" } else { "✗" };
        println!("    {within} {strat.name}: {cost.compile_time_ms:.0}ms");
    }
    println!("");

    println!("═══════════════════════════════════════════════════════════");
    println!("  Optimization complete for {targets.len()} architectures × {profiles.len()} profiles.");
    println!("═══════════════════════════════════════════════════════════");
}
