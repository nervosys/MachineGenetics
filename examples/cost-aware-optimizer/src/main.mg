// cost-aware-optimizer — choosing a compilation strategy by measured cost.
//
// `forge run` evaluates `main` and prints its result. Demonstrates:
//   - a cost model as data: three axes, kept separate until something weighs them
//   - `trait` + `impl Trait for T` as the objective function, so "fastest",
//     "smallest", and "greenest" are three types rather than three branches
//   - calibration: per-architecture scale factors applied before scoring, so
//     the same strategy scores differently on different targets
//   - budget pruning with `filter`, and selection with `fold` — `reduce` would
//     return `?A`, and the empty case is answered here rather than unwrapped
//   - `/ fs` on the one function that would read a benchmark file
//
// The numbers are made up, but the arithmetic is real: change a weight and the
// selected strategy changes, which is the property a cost model has to have.
//
// Run:  forge run        (or:  mage-parse --eval src/main.mg main)

// ── Targets and strategies ───────────────────────────────────────────

enum Arch {
    X86,
    Arm,
    Riscv,
    Wasm,
}

enum Strategy {
    Size,
    Speed,
    Balanced,
}

fn arch_name(arch: Arch) -> String {
    ?= arch {
        Arch.X86 => "x86-64",
        Arch.Arm => "aarch64",
        Arch.Riscv => "riscv64",
        Arch.Wasm => "wasm32",
    }
}

fn strategy_name(strategy: Strategy) -> String {
    ?= strategy {
        Strategy.Size => "size",
        Strategy.Speed => "speed",
        Strategy.Balanced => "balanced",
    }
}

// ── The cost model ───────────────────────────────────────────────────

// Three axes, deliberately not collapsed into one number. Combining them is a
// policy decision, and policy lives in the objective below, not in the model.
struct Cost {
    latency_us: i32,
    code_bytes: i32,
    energy_uj: i32,
}

struct Candidate {
    strategy: Strategy,
    cost: Cost,
}

// ── Calibration ──────────────────────────────────────────────────────

// A strategy does not cost the same everywhere: a size-first build helps most
// where instruction fetch is expensive. These are the scale factors that make
// the model architecture-aware, expressed in percent to stay in integers.
struct Calibration {
    arch: Arch,
    latency_pct: i32,
    size_pct: i32,
    energy_pct: i32,
}

fn calibration_for(arch: Arch) -> Calibration {
    ?= arch {
        Arch.X86 => @Calibration { arch: arch, latency_pct: 100, size_pct: 100, energy_pct: 100 },
        Arch.Arm => @Calibration { arch: arch, latency_pct: 120, size_pct: 90, energy_pct: 60 },
        Arch.Riscv => @Calibration { arch: arch, latency_pct: 140, size_pct: 85, energy_pct: 55 },
        Arch.Wasm => @Calibration { arch: arch, latency_pct: 210, size_pct: 70, energy_pct: 130 },
    }
}

// Percent-scaling, integer only. Every cost the optimizer compares has been
// through this, so a comparison is always between calibrated numbers — and the
// result stays in the original units, so a budget can be written in real
// microseconds instead of in percent-scaled ones.
fn calibrate(cost: Cost, cal: Calibration) -> Cost {
    @Cost {
        latency_us: cost.latency_us * cal.latency_pct / 100,
        code_bytes: cost.code_bytes * cal.size_pct / 100,
        energy_uj: cost.energy_uj * cal.energy_pct / 100,
    }
}

// ── Objectives ───────────────────────────────────────────────────────

// The weighing policy. Three implementations of one trait: `objective.score(c)`
// dispatches on the objective's type, so a new policy is a new impl and no
// existing function changes. Lower is better throughout.
trait Objective {
    fn score(&self, cost: Cost) -> i32;
    fn label(&self) -> String;
}

struct LatencyFirst {
    size_weight: i32,
}

struct SizeFirst {
    latency_weight: i32,
}

struct EnergyFirst {
    latency_weight: i32,
}

impl Objective for LatencyFirst {
    fn score(&self, cost: Cost) -> i32 {
        cost.latency_us * 10 + cost.code_bytes * self.size_weight
    }

    fn label(&self) -> String {
        "latency-first"
    }
}

impl Objective for SizeFirst {
    fn score(&self, cost: Cost) -> i32 {
        cost.code_bytes * 10 + cost.latency_us * self.latency_weight
    }

    fn label(&self) -> String {
        "size-first"
    }
}

impl Objective for EnergyFirst {
    fn score(&self, cost: Cost) -> i32 {
        cost.energy_uj * 10 + cost.latency_us * self.latency_weight
    }

    fn label(&self) -> String {
        "energy-first"
    }
}

// ── Budget pruning ───────────────────────────────────────────────────

struct Budget {
    max_latency_us: i32,
    max_code_bytes: i32,
}

// Pruning happens before scoring, not after: a candidate that violates the
// budget is not a cheaper option, it is not an option.
//
// It is also applied to the *calibrated* cost. Pruning the raw numbers made the
// budget architecture-independent, so all four targets reported the same answer
// and the calibration table might as well not have existed.
fn within_budget(cost: Cost, budget: Budget) -> bool {
    cost.latency_us <= budget.max_latency_us && cost.code_bytes <= budget.max_code_bytes
}

// ── Selection ────────────────────────────────────────────────────────

// The candidate set. In a real optimizer these come from measurement; here the
// shape is what matters — one entry per strategy, uncalibrated.
fn candidates() -> [Candidate]~ {
    [
        @Candidate {
            strategy: Strategy.Size,
            cost: @Cost { latency_us: 900, code_bytes: 4_000, energy_uj: 700 },
        },
        @Candidate {
            strategy: Strategy.Speed,
            cost: @Cost { latency_us: 300, code_bytes: 14_000, energy_uj: 1_500 },
        },
        @Candidate {
            strategy: Strategy.Balanced,
            cost: @Cost { latency_us: 500, code_bytes: 7_000, energy_uj: 950 },
        },
    ]
}

// ── Reporting ────────────────────────────────────────────────────────

// One line per architecture: prune, calibrate, score, pick.
fn optimize(arch: Arch, budget: Budget, weight: i32) -> String {
    val cal = calibration_for(arch)
    val priced = map(
        candidates(),
        fn(c) => @Candidate { strategy: c.strategy, cost: calibrate(c.cost, cal) },
    )
    val affordable = filter(priced, fn(c) => within_budget(c.cost, budget))
    val objective = @EnergyFirst { latency_weight: weight }
    val scored = map(affordable, fn(c) => (objective.score(c.cost), c.strategy))
    ?= pick_strategy(scored) {
        Some(strategy) => f"{arch_name(arch)}: {strategy_name(strategy)}",
        None => f"{arch_name(arch)}: no candidate within budget",
    }
}

// The minimum, as a fold. `reduce` would also give `?A`, but the accumulator
// here carries the score alongside the strategy, so a plain fold is clearer.
// The empty case is genuine — a tight budget can prune every candidate — and
// `?Strategy` makes the caller answer for it rather than invent a default.
fn pick_strategy(scored: [(i32, Strategy)]~) -> ?Strategy {
    // Seeded from `first` rather than from `None`: a bare `None` seed leaves the
    // accumulator's type with nothing to infer from, and it also forces every
    // step to unwrap an option it can never actually be empty in. `first`
    // answers the empty case once, up front, where it belongs.
    ?= first(scored) {
        None => None,
        Some(head) => ?= fold(
            scored,
            head,
            fn(best, entry) => ?= entry {
                (score, strategy) => ?= best {
                    (best_score, best_strategy) => ? score < best_score {
                        (score, strategy)
                    } : {
                        (best_score, best_strategy)
                    },
                },
            },
        ) {
            (_, winner) => Some(winner),
        },
    }
}

// ── Calibration input ────────────────────────────────────────────────

// The one function that would read a benchmark file, so the one that carries
// `/ fs`. Everything above is pure: the optimizer can be exercised without any
// measurement on disk.
fn load_benchmarks(path: String) -> usize / fs {
    ?= path {
        "bench.csv" => 3,
        _ => 0,
    }
}

// ── Entry point ──────────────────────────────────────────────────────

pub fn main() -> String / fs {
    val samples = load_benchmarks("bench.csv")

    val generous = @Budget { max_latency_us: 2_000, max_code_bytes: 20_000 }
    // Chosen to land *between* the targets rather than below all of them: it
    // admits the size-first build on x86 and Arm, admits only the balanced one
    // on wasm — whose 70% size factor is what brings it under the byte cap —
    // and rules out everything on RISC-V.
    val tight = @Budget { max_latency_us: 1_100, max_code_bytes: 5_000 }

    val targets = [Arch.X86, Arch.Arm, Arch.Riscv, Arch.Wasm]
    val chosen = map(targets, fn(arch) => optimize(arch, generous, 4))
    val squeezed = map(targets, fn(arch) => optimize(arch, tight, 4))

    join(
        [
            f"{samples} benchmark samples",
            join(chosen, " | "),
            join(squeezed, " | "),
        ],
        "; ",
    )
}
