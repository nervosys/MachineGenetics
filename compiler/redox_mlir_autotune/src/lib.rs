//! # Redox MLIR Autotuning Engine
//!
//! Implements the `@pa(N)` annotation: generates N lowering variants for
//! a kernel, benchmarks each on the target hardware, and selects the optimal
//! variant. Agent swarms can parallelize the search across machines.
//!
//! Pipeline:
//! ```text
//! @pa(variants = N) kernel →
//!   [Variant 1: tiled] [Variant 2: vectorized] [Variant 3: unrolled] [Variant 4: fused]
//!     → Benchmark each on target → Select fastest → Emit winner
//! ```
//!
//! Reference: REDOX_PROPOSAL.md §5.4.2 — "MLIR-native autotuning (@pa):
//! generate N variants, benchmark per-target"
//!
//! (ROADMAP Step 58)

use std::collections::BTreeMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Variant Strategies
// ═══════════════════════════════════════════════════════════════════════════

/// A lowering strategy that produces a distinct variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoweringStrategy {
    /// Tile loops for cache locality.
    Tiled,
    /// Vectorize inner loops for SIMD.
    Vectorized,
    /// Unroll loops by a fixed factor.
    Unrolled,
    /// Fuse adjacent loops.
    Fused,
    /// Parallelize across threads / workgroups.
    Parallelized,
    /// Scalar baseline (no optimization).
    Scalar,
    /// Memory-layout-optimized (SoA vs AoS).
    LayoutOptimized,
    /// Software pipelining.
    Pipelined,
}

impl LoweringStrategy {
    /// Default strategies for a given variant count.
    pub fn default_set(n: usize) -> Vec<LoweringStrategy> {
        let all = [
            LoweringStrategy::Tiled,
            LoweringStrategy::Vectorized,
            LoweringStrategy::Unrolled,
            LoweringStrategy::Fused,
            LoweringStrategy::Parallelized,
            LoweringStrategy::Scalar,
            LoweringStrategy::LayoutOptimized,
            LoweringStrategy::Pipelined,
        ];
        all.iter().copied().take(n).collect()
    }
}

impl fmt::Display for LoweringStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoweringStrategy::Tiled => write!(f, "tiled"),
            LoweringStrategy::Vectorized => write!(f, "vectorized"),
            LoweringStrategy::Unrolled => write!(f, "unrolled"),
            LoweringStrategy::Fused => write!(f, "fused"),
            LoweringStrategy::Parallelized => write!(f, "parallelized"),
            LoweringStrategy::Scalar => write!(f, "scalar"),
            LoweringStrategy::LayoutOptimized => write!(f, "layout-optimized"),
            LoweringStrategy::Pipelined => write!(f, "pipelined"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Target Hardware
// ═══════════════════════════════════════════════════════════════════════════

/// Target hardware for autotuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetHardware {
    Cpu,
    Gpu,
    Npu,
}

impl fmt::Display for TargetHardware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TargetHardware::Cpu => write!(f, "CPU"),
            TargetHardware::Gpu => write!(f, "GPU"),
            TargetHardware::Npu => write!(f, "NPU"),
        }
    }
}

/// Target hardware characteristics for cost modeling.
#[derive(Debug, Clone)]
pub struct TargetProfile {
    pub hardware: TargetHardware,
    pub name: String,
    /// Vector width in bits (e.g., 128 for SSE, 256 for AVX2, 512 for AVX-512).
    pub vector_width_bits: u32,
    /// Number of compute units / cores.
    pub compute_units: u32,
    /// Approximate clock frequency in MHz.
    pub clock_mhz: u32,
    /// Memory bandwidth in GB/s.
    pub memory_bandwidth_gbps: u32,
    /// L1 cache size in KB.
    pub l1_cache_kb: u32,
}

impl TargetProfile {
    pub fn generic_cpu() -> Self {
        TargetProfile {
            hardware: TargetHardware::Cpu,
            name: "generic-cpu".to_string(),
            vector_width_bits: 256,
            compute_units: 8,
            clock_mhz: 3000,
            memory_bandwidth_gbps: 50,
            l1_cache_kb: 32,
        }
    }

    pub fn generic_gpu() -> Self {
        TargetProfile {
            hardware: TargetHardware::Gpu,
            name: "generic-gpu".to_string(),
            vector_width_bits: 512,
            compute_units: 2048,
            clock_mhz: 1500,
            memory_bandwidth_gbps: 900,
            l1_cache_kb: 128,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Kernel IR (simplified)
// ═══════════════════════════════════════════════════════════════════════════

/// A simplified kernel operation for autotuning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelOp {
    pub name: String,
    pub kind: KernelOpKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelOpKind {
    Load,
    Store,
    Compute,
    Branch,
    Call,
    Loop,
}

impl KernelOp {
    pub fn new(name: &str, kind: KernelOpKind) -> Self {
        KernelOp { name: name.to_string(), kind }
    }
}

/// A kernel to be autotuned.
#[derive(Debug, Clone)]
pub struct Kernel {
    pub name: String,
    pub ops: Vec<KernelOp>,
    pub metadata: BTreeMap<String, String>,
}

impl Kernel {
    pub fn new(name: &str) -> Self {
        Kernel {
            name: name.to_string(),
            ops: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn add_op(&mut self, op: KernelOp) {
        self.ops.push(op);
    }

    pub fn set_metadata(&mut self, key: &str, value: &str) {
        self.metadata.insert(key.to_string(), value.to_string());
    }

    pub fn op_count(&self) -> usize {
        self.ops.len()
    }

    /// Count ops of a given kind.
    pub fn count_kind(&self, kind: KernelOpKind) -> usize {
        self.ops.iter().filter(|o| o.kind == kind).count()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Lowered Variant
// ═══════════════════════════════════════════════════════════════════════════

/// A lowered variant produced by applying a strategy to a kernel.
#[derive(Debug, Clone)]
pub struct LoweredVariant {
    pub id: usize,
    pub strategy: LoweringStrategy,
    pub ops: Vec<LoweredOp>,
    pub estimated_cost: CostEstimate,
}

/// A lowered operation in a variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredOp {
    pub instruction: String,
    pub comment: String,
}

impl LoweredOp {
    pub fn new(instruction: &str, comment: &str) -> Self {
        LoweredOp {
            instruction: instruction.to_string(),
            comment: comment.to_string(),
        }
    }
}

/// Cost estimate for a variant on a specific target.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct CostEstimate {
    /// Estimated execution cycles.
    pub cycles: f64,
    /// Estimated memory traffic in bytes.
    pub memory_bytes: f64,
    /// Overall score (lower is better).
    pub score: f64,
}

impl CostEstimate {
    pub fn new(cycles: f64, memory_bytes: f64) -> Self {
        // Score: weighted combination (cycles dominate)
        let score = cycles + memory_bytes * 0.01;
        CostEstimate { cycles, memory_bytes, score }
    }
}

impl fmt::Display for CostEstimate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cycles={:.0}, mem={:.0}B, score={:.2}", self.cycles, self.memory_bytes, self.score)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Variant Generation
// ═══════════════════════════════════════════════════════════════════════════

/// Generate a lowered variant by applying a strategy to a kernel on a target.
pub fn generate_variant(
    kernel: &Kernel,
    strategy: LoweringStrategy,
    target: &TargetProfile,
    id: usize,
) -> LoweredVariant {
    let mut ops = Vec::new();
    let mut total_cycles: f64 = 0.0;
    let mut total_mem: f64 = 0.0;

    for kop in &kernel.ops {
        match (&strategy, &kop.kind) {
            (LoweringStrategy::Vectorized, KernelOpKind::Compute) => {
                let lanes = target.vector_width_bits / 32;
                ops.push(LoweredOp::new(
                    &format!("v.compute.{lanes}wide"),
                    &format!("vectorized {lanes}-lane compute"),
                ));
                total_cycles += 1.0 / lanes as f64;
            }
            (LoweringStrategy::Vectorized, KernelOpKind::Load) => {
                let lanes = target.vector_width_bits / 32;
                ops.push(LoweredOp::new(
                    &format!("v.load.{lanes}wide"),
                    &format!("vectorized {lanes}-lane load"),
                ));
                total_cycles += 2.0 / lanes as f64;
                total_mem += 4.0 * lanes as f64;
            }
            (LoweringStrategy::Tiled, KernelOpKind::Loop) => {
                let tile_size = target.l1_cache_kb * 256; // elements that fit in L1
                ops.push(LoweredOp::new(
                    &format!("tiled.loop tile_sz={tile_size}"),
                    "loop tiled for cache locality",
                ));
                total_cycles += 0.8; // tiling reduces cache misses
                total_mem += tile_size as f64 * 4.0;
            }
            (LoweringStrategy::Tiled, KernelOpKind::Compute) => {
                ops.push(LoweredOp::new("tiled.compute", "compute within tile"));
                total_cycles += 1.0;
            }
            (LoweringStrategy::Unrolled, KernelOpKind::Loop) => {
                ops.push(LoweredOp::new("unrolled.loop x4", "loop unrolled 4x"));
                total_cycles += 0.9; // unrolling amortizes branch overhead
            }
            (LoweringStrategy::Unrolled, KernelOpKind::Compute) => {
                ops.push(LoweredOp::new("unrolled.compute x4", "4x compute per iteration"));
                total_cycles += 0.85;
            }
            (LoweringStrategy::Fused, KernelOpKind::Store) => {
                ops.push(LoweredOp::new("fused.store_compute", "fused store with next compute"));
                total_cycles += 0.7;
                total_mem += 4.0; // reduced memory round-trip
            }
            (LoweringStrategy::Fused, KernelOpKind::Load) => {
                ops.push(LoweredOp::new("fused.load_compute", "fused load with compute"));
                total_cycles += 0.7;
                total_mem += 4.0;
            }
            (LoweringStrategy::Parallelized, KernelOpKind::Loop) => {
                let units = target.compute_units;
                ops.push(LoweredOp::new(
                    &format!("parallel.loop units={units}"),
                    &format!("parallelized across {units} units"),
                ));
                total_cycles += 1.0 / units as f64;
            }
            (LoweringStrategy::LayoutOptimized, KernelOpKind::Load) => {
                let layout = if matches!(target.hardware, TargetHardware::Gpu) {
                    "SoA"
                } else {
                    "AoS"
                };
                ops.push(LoweredOp::new(
                    &format!("layout.load.{layout}"),
                    &format!("{layout} load for {}", target.hardware),
                ));
                total_cycles += 1.5;
                total_mem += 4.0;
            }
            (LoweringStrategy::Pipelined, KernelOpKind::Loop) => {
                ops.push(LoweredOp::new("sw_pipeline.loop", "software pipelined loop"));
                total_cycles += 0.6;
            }
            // Scalar / default
            (_, KernelOpKind::Load) => {
                ops.push(LoweredOp::new("s.load", "scalar load"));
                total_cycles += 4.0;
                total_mem += 4.0;
            }
            (_, KernelOpKind::Store) => {
                ops.push(LoweredOp::new("s.store", "scalar store"));
                total_cycles += 4.0;
                total_mem += 4.0;
            }
            (_, KernelOpKind::Compute) => {
                ops.push(LoweredOp::new("s.compute", "scalar compute"));
                total_cycles += 1.0;
            }
            (_, KernelOpKind::Branch) => {
                ops.push(LoweredOp::new("s.branch", "branch"));
                total_cycles += 0.5;
            }
            (_, KernelOpKind::Call) => {
                ops.push(LoweredOp::new("s.call", "function call"));
                total_cycles += 5.0;
            }
            (_, KernelOpKind::Loop) => {
                ops.push(LoweredOp::new("s.loop", "scalar loop"));
                total_cycles += 1.0;
            }
        }
    }

    let cost = CostEstimate::new(total_cycles, total_mem);
    LoweredVariant { id, strategy, ops, estimated_cost: cost }
}

// ═══════════════════════════════════════════════════════════════════════════
// Benchmark Result
// ═══════════════════════════════════════════════════════════════════════════

/// Simulated benchmark result for a variant.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub variant_id: usize,
    pub strategy: LoweringStrategy,
    pub estimated_cost: CostEstimate,
    pub target_name: String,
}

impl BenchmarkResult {
    pub fn score(&self) -> f64 {
        self.estimated_cost.score
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Autotuning Configuration
// ═══════════════════════════════════════════════════════════════════════════

/// Configuration for the `@pa(N)` annotation.
#[derive(Debug, Clone)]
pub struct AutotuneConfig {
    /// Number of variants to generate.
    pub variant_count: usize,
    /// Target hardware to benchmark on.
    pub target: TargetProfile,
    /// Specific strategies to use (if empty, use defaults).
    pub strategies: Vec<LoweringStrategy>,
    /// Maximum variants to keep (top-k selection).
    pub top_k: usize,
}

impl AutotuneConfig {
    pub fn new(variant_count: usize, target: TargetProfile) -> Self {
        AutotuneConfig {
            variant_count,
            target,
            strategies: Vec::new(),
            top_k: 1,
        }
    }

    /// Get effective strategies (user-provided or defaults).
    pub fn effective_strategies(&self) -> Vec<LoweringStrategy> {
        if self.strategies.is_empty() {
            LoweringStrategy::default_set(self.variant_count)
        } else {
            self.strategies.iter().copied().take(self.variant_count).collect()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Autotuning Engine
// ═══════════════════════════════════════════════════════════════════════════

/// Result of autotuning a kernel.
#[derive(Debug, Clone)]
pub struct AutotuneResult {
    pub kernel_name: String,
    pub variants: Vec<LoweredVariant>,
    pub benchmarks: Vec<BenchmarkResult>,
    pub winner_id: usize,
    pub winner_strategy: LoweringStrategy,
    pub target_name: String,
}

impl AutotuneResult {
    /// Get the winning variant.
    pub fn winner(&self) -> Option<&LoweredVariant> {
        self.variants.iter().find(|v| v.id == self.winner_id)
    }

    /// Get the winning benchmark result.
    pub fn winner_benchmark(&self) -> Option<&BenchmarkResult> {
        self.benchmarks.iter().find(|b| b.variant_id == self.winner_id)
    }

    /// Speedup of winner vs worst variant.
    pub fn speedup_ratio(&self) -> f64 {
        let winner_score = self.benchmarks.iter()
            .find(|b| b.variant_id == self.winner_id)
            .map(|b| b.score())
            .unwrap_or(1.0);
        let worst_score = self.benchmarks.iter()
            .map(|b| b.score())
            .fold(f64::MIN, f64::max);
        if winner_score > 0.0 {
            worst_score / winner_score
        } else {
            1.0
        }
    }

    /// Summary as a human-readable string.
    pub fn summary(&self) -> String {
        format!(
            "Autotuned '{}': {} variants on {}, winner={} ({}), speedup={:.2}x",
            self.kernel_name,
            self.variants.len(),
            self.target_name,
            self.winner_strategy,
            self.winner().map_or("N/A".to_string(), |v| v.estimated_cost.to_string()),
            self.speedup_ratio(),
        )
    }
}

/// Run the autotuning engine on a kernel.
pub fn autotune(kernel: &Kernel, config: &AutotuneConfig) -> AutotuneResult {
    let strategies = config.effective_strategies();

    // Generate variants
    let variants: Vec<LoweredVariant> = strategies
        .iter()
        .enumerate()
        .map(|(i, s)| generate_variant(kernel, *s, &config.target, i))
        .collect();

    // Benchmark (cost-model-based simulation)
    let benchmarks: Vec<BenchmarkResult> = variants
        .iter()
        .map(|v| BenchmarkResult {
            variant_id: v.id,
            strategy: v.strategy,
            estimated_cost: v.estimated_cost.clone(),
            target_name: config.target.name.clone(),
        })
        .collect();

    // Select winner (lowest score)
    let winner_id = benchmarks
        .iter()
        .min_by(|a, b| a.score().partial_cmp(&b.score()).unwrap_or(std::cmp::Ordering::Equal))
        .map(|b| b.variant_id)
        .unwrap_or(0);

    let winner_strategy = variants
        .iter()
        .find(|v| v.id == winner_id)
        .map(|v| v.strategy)
        .unwrap_or(LoweringStrategy::Scalar);

    AutotuneResult {
        kernel_name: kernel.name.clone(),
        variants,
        benchmarks,
        winner_id,
        winner_strategy,
        target_name: config.target.name.clone(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Multi-Target Autotuning
// ═══════════════════════════════════════════════════════════════════════════

/// Result of autotuning across multiple targets.
#[derive(Debug, Clone)]
pub struct MultiTargetResult {
    pub kernel_name: String,
    pub results: Vec<AutotuneResult>,
}

impl MultiTargetResult {
    /// Get the best result across all targets (lowest absolute score).
    pub fn global_best(&self) -> Option<&AutotuneResult> {
        self.results.iter().min_by(|a, b| {
            let sa = a.winner_benchmark().map(|b| b.score()).unwrap_or(f64::MAX);
            let sb = b.winner_benchmark().map(|b| b.score()).unwrap_or(f64::MAX);
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Get the result for a specific target.
    pub fn for_target(&self, name: &str) -> Option<&AutotuneResult> {
        self.results.iter().find(|r| r.target_name == name)
    }
}

/// Run autotuning on a kernel across multiple targets.
pub fn autotune_multi_target(
    kernel: &Kernel,
    variant_count: usize,
    targets: &[TargetProfile],
) -> MultiTargetResult {
    let results: Vec<AutotuneResult> = targets
        .iter()
        .map(|t| {
            let config = AutotuneConfig::new(variant_count, t.clone());
            autotune(kernel, &config)
        })
        .collect();

    MultiTargetResult {
        kernel_name: kernel.name.clone(),
        results,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Swarm-Parallel Task Distribution
// ═══════════════════════════════════════════════════════════════════════════

/// An autotuning task that can be dispatched to a swarm agent.
#[derive(Debug, Clone)]
pub struct AutotuneTask {
    pub task_id: usize,
    pub kernel_name: String,
    pub strategy: LoweringStrategy,
    pub target_name: String,
}

/// Decompose an autotune request into parallelizable tasks for swarm agents.
pub fn decompose_for_swarm(
    kernel: &Kernel,
    config: &AutotuneConfig,
) -> Vec<AutotuneTask> {
    let strategies = config.effective_strategies();
    strategies
        .iter()
        .enumerate()
        .map(|(i, s)| AutotuneTask {
            task_id: i,
            kernel_name: kernel.name.clone(),
            strategy: *s,
            target_name: config.target.name.clone(),
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
// Annotation Parsing
// ═══════════════════════════════════════════════════════════════════════════

/// Parsed `@pa(N)` or `#[perf::autotune(variants = N)]` annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutotuneAnnotation {
    pub variant_count: usize,
    pub strategies: Vec<LoweringStrategy>,
}

impl AutotuneAnnotation {
    pub fn new(variant_count: usize) -> Self {
        AutotuneAnnotation { variant_count, strategies: Vec::new() }
    }

    pub fn with_strategies(mut self, strategies: Vec<LoweringStrategy>) -> Self {
        self.strategies = strategies;
        self
    }
}

/// Parse the compact `@pa(N)` annotation.
pub fn parse_pa_annotation(input: &str) -> Option<AutotuneAnnotation> {
    let trimmed = input.trim();
    // @pa(N) or @pa(variants = N)
    if !trimmed.starts_with("@pa(") || !trimmed.ends_with(')') {
        return None;
    }
    let inner = &trimmed[4..trimmed.len() - 1];
    if let Some(rest) = inner.strip_prefix("variants") {
        // @pa(variants = N)
        let rest = rest.trim();
        let rest = rest.strip_prefix('=')?;
        let n: usize = rest.trim().parse().ok()?;
        Some(AutotuneAnnotation::new(n))
    } else {
        // @pa(N)
        let n: usize = inner.trim().parse().ok()?;
        Some(AutotuneAnnotation::new(n))
    }
}

/// Parse the Rust attribute form `#[perf::autotune(variants = N)]`.
pub fn parse_autotune_attr(input: &str) -> Option<AutotuneAnnotation> {
    let trimmed = input.trim();
    let prefix = "#[perf::autotune(";
    if !trimmed.starts_with(prefix) || !trimmed.ends_with(")]") {
        return None;
    }
    let inner = &trimmed[prefix.len()..trimmed.len() - 2];
    // variants = N
    if let Some(rest) = inner.strip_prefix("variants") {
        let rest = rest.trim();
        let rest = rest.strip_prefix('=')?;
        let n: usize = rest.trim().parse().ok()?;
        Some(AutotuneAnnotation::new(n))
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_kernel() -> Kernel {
        let mut k = Kernel::new("convolution");
        k.add_op(KernelOp::new("load_input", KernelOpKind::Load));
        k.add_op(KernelOp::new("load_kernel", KernelOpKind::Load));
        k.add_op(KernelOp::new("outer_loop", KernelOpKind::Loop));
        k.add_op(KernelOp::new("inner_compute", KernelOpKind::Compute));
        k.add_op(KernelOp::new("store_output", KernelOpKind::Store));
        k
    }

    // ── Strategy Defaults ────────────────────────────────────────────────

    #[test]
    fn default_strategy_set() {
        let set = LoweringStrategy::default_set(4);
        assert_eq!(set.len(), 4);
        assert_eq!(set[0], LoweringStrategy::Tiled);
        assert_eq!(set[1], LoweringStrategy::Vectorized);
        assert_eq!(set[2], LoweringStrategy::Unrolled);
        assert_eq!(set[3], LoweringStrategy::Fused);
    }

    #[test]
    fn default_set_clamped() {
        let set = LoweringStrategy::default_set(100);
        assert_eq!(set.len(), 8); // max 8 strategies
    }

    #[test]
    fn strategy_display() {
        assert_eq!(LoweringStrategy::Tiled.to_string(), "tiled");
        assert_eq!(LoweringStrategy::Vectorized.to_string(), "vectorized");
        assert_eq!(LoweringStrategy::Scalar.to_string(), "scalar");
    }

    // ── Target Profile ───────────────────────────────────────────────────

    #[test]
    fn generic_cpu_profile() {
        let p = TargetProfile::generic_cpu();
        assert_eq!(p.hardware, TargetHardware::Cpu);
        assert_eq!(p.vector_width_bits, 256);
    }

    #[test]
    fn generic_gpu_profile() {
        let p = TargetProfile::generic_gpu();
        assert_eq!(p.hardware, TargetHardware::Gpu);
        assert!(p.compute_units > 1000);
    }

    // ── Kernel ───────────────────────────────────────────────────────────

    #[test]
    fn kernel_ops() {
        let k = sample_kernel();
        assert_eq!(k.op_count(), 5);
        assert_eq!(k.count_kind(KernelOpKind::Load), 2);
        assert_eq!(k.count_kind(KernelOpKind::Compute), 1);
        assert_eq!(k.count_kind(KernelOpKind::Store), 1);
        assert_eq!(k.count_kind(KernelOpKind::Loop), 1);
    }

    #[test]
    fn kernel_metadata() {
        let mut k = Kernel::new("test");
        k.set_metadata("origin", "swarm-agent-1");
        assert_eq!(k.metadata.get("origin").map(String::as_str), Some("swarm-agent-1"));
    }

    // ── Variant Generation ───────────────────────────────────────────────

    #[test]
    fn generate_vectorized_variant() {
        let k = sample_kernel();
        let t = TargetProfile::generic_cpu();
        let v = generate_variant(&k, LoweringStrategy::Vectorized, &t, 0);
        assert_eq!(v.strategy, LoweringStrategy::Vectorized);
        assert!(!v.ops.is_empty());
        // Vectorized loads should mention wide
        let has_wide = v.ops.iter().any(|o| o.instruction.contains("wide"));
        assert!(has_wide);
    }

    #[test]
    fn generate_tiled_variant() {
        let k = sample_kernel();
        let t = TargetProfile::generic_cpu();
        let v = generate_variant(&k, LoweringStrategy::Tiled, &t, 1);
        assert_eq!(v.strategy, LoweringStrategy::Tiled);
        let has_tiled = v.ops.iter().any(|o| o.instruction.contains("tiled"));
        assert!(has_tiled);
    }

    #[test]
    fn generate_scalar_baseline() {
        let k = sample_kernel();
        let t = TargetProfile::generic_cpu();
        let v = generate_variant(&k, LoweringStrategy::Scalar, &t, 2);
        assert_eq!(v.strategy, LoweringStrategy::Scalar);
        // All ops should be scalar
        let all_scalar = v.ops.iter().all(|o| o.instruction.starts_with("s."));
        assert!(all_scalar);
    }

    #[test]
    fn generate_fused_variant() {
        let k = sample_kernel();
        let t = TargetProfile::generic_cpu();
        let v = generate_variant(&k, LoweringStrategy::Fused, &t, 3);
        let has_fused = v.ops.iter().any(|o| o.instruction.contains("fused"));
        assert!(has_fused);
    }

    #[test]
    fn generate_parallelized_variant() {
        let k = sample_kernel();
        let t = TargetProfile::generic_gpu();
        let v = generate_variant(&k, LoweringStrategy::Parallelized, &t, 4);
        let has_parallel = v.ops.iter().any(|o| o.instruction.contains("parallel"));
        assert!(has_parallel);
    }

    #[test]
    fn layout_optimized_gpu_soa() {
        let k = sample_kernel();
        let t = TargetProfile::generic_gpu();
        let v = generate_variant(&k, LoweringStrategy::LayoutOptimized, &t, 0);
        let has_soa = v.ops.iter().any(|o| o.instruction.contains("SoA"));
        assert!(has_soa);
    }

    #[test]
    fn layout_optimized_cpu_aos() {
        let k = sample_kernel();
        let t = TargetProfile::generic_cpu();
        let v = generate_variant(&k, LoweringStrategy::LayoutOptimized, &t, 0);
        let has_aos = v.ops.iter().any(|o| o.instruction.contains("AoS"));
        assert!(has_aos);
    }

    // ── Cost Estimate ────────────────────────────────────────────────────

    #[test]
    fn cost_estimate_scoring() {
        let c = CostEstimate::new(100.0, 1000.0);
        assert_eq!(c.score, 100.0 + 1000.0 * 0.01);
        assert!(c.to_string().contains("cycles=100"));
    }

    #[test]
    fn vectorized_cheaper_than_scalar() {
        let k = sample_kernel();
        let t = TargetProfile::generic_cpu();
        let scalar = generate_variant(&k, LoweringStrategy::Scalar, &t, 0);
        let vector = generate_variant(&k, LoweringStrategy::Vectorized, &t, 1);
        assert!(vector.estimated_cost.score < scalar.estimated_cost.score);
    }

    // ── Autotuning Engine ────────────────────────────────────────────────

    #[test]
    fn autotune_basic() {
        let k = sample_kernel();
        let config = AutotuneConfig::new(4, TargetProfile::generic_cpu());
        let result = autotune(&k, &config);
        assert_eq!(result.kernel_name, "convolution");
        assert_eq!(result.variants.len(), 4);
        assert_eq!(result.benchmarks.len(), 4);
        // Winner should exist
        assert!(result.winner().is_some());
    }

    #[test]
    fn autotune_winner_is_best() {
        let k = sample_kernel();
        let config = AutotuneConfig::new(4, TargetProfile::generic_cpu());
        let result = autotune(&k, &config);
        let winner_score = result.winner_benchmark().unwrap().score();
        for b in &result.benchmarks {
            assert!(winner_score <= b.score() + f64::EPSILON);
        }
    }

    #[test]
    fn autotune_speedup_ratio() {
        let k = sample_kernel();
        let config = AutotuneConfig::new(4, TargetProfile::generic_cpu());
        let result = autotune(&k, &config);
        assert!(result.speedup_ratio() >= 1.0);
    }

    #[test]
    fn autotune_summary() {
        let k = sample_kernel();
        let config = AutotuneConfig::new(4, TargetProfile::generic_cpu());
        let result = autotune(&k, &config);
        let summary = result.summary();
        assert!(summary.contains("convolution"));
        assert!(summary.contains("4 variants"));
        assert!(summary.contains("generic-cpu"));
    }

    #[test]
    fn autotune_custom_strategies() {
        let k = sample_kernel();
        let mut config = AutotuneConfig::new(2, TargetProfile::generic_cpu());
        config.strategies = vec![LoweringStrategy::Scalar, LoweringStrategy::Vectorized];
        let result = autotune(&k, &config);
        assert_eq!(result.variants.len(), 2);
        assert_eq!(result.variants[0].strategy, LoweringStrategy::Scalar);
        assert_eq!(result.variants[1].strategy, LoweringStrategy::Vectorized);
    }

    #[test]
    fn autotune_gpu_target() {
        let k = sample_kernel();
        let config = AutotuneConfig::new(4, TargetProfile::generic_gpu());
        let result = autotune(&k, &config);
        assert_eq!(result.target_name, "generic-gpu");
        assert!(result.winner().is_some());
    }

    // ── Multi-Target ─────────────────────────────────────────────────────

    #[test]
    fn multi_target_autotune() {
        let k = sample_kernel();
        let targets = vec![
            TargetProfile::generic_cpu(),
            TargetProfile::generic_gpu(),
        ];
        let result = autotune_multi_target(&k, 4, &targets);
        assert_eq!(result.results.len(), 2);
        assert!(result.for_target("generic-cpu").is_some());
        assert!(result.for_target("generic-gpu").is_some());
    }

    #[test]
    fn multi_target_global_best() {
        let k = sample_kernel();
        let targets = vec![
            TargetProfile::generic_cpu(),
            TargetProfile::generic_gpu(),
        ];
        let result = autotune_multi_target(&k, 4, &targets);
        assert!(result.global_best().is_some());
    }

    // ── Swarm Decomposition ──────────────────────────────────────────────

    #[test]
    fn decompose_tasks() {
        let k = sample_kernel();
        let config = AutotuneConfig::new(4, TargetProfile::generic_cpu());
        let tasks = decompose_for_swarm(&k, &config);
        assert_eq!(tasks.len(), 4);
        for (i, task) in tasks.iter().enumerate() {
            assert_eq!(task.task_id, i);
            assert_eq!(task.kernel_name, "convolution");
        }
    }

    // ── Annotation Parsing ───────────────────────────────────────────────

    #[test]
    fn parse_pa_simple() {
        let ann = parse_pa_annotation("@pa(4)").unwrap();
        assert_eq!(ann.variant_count, 4);
    }

    #[test]
    fn parse_pa_named() {
        let ann = parse_pa_annotation("@pa(variants = 8)").unwrap();
        assert_eq!(ann.variant_count, 8);
    }

    #[test]
    fn parse_pa_whitespace() {
        let ann = parse_pa_annotation("  @pa( 16 )  ").unwrap();
        assert_eq!(ann.variant_count, 16);
    }

    #[test]
    fn parse_pa_invalid() {
        assert!(parse_pa_annotation("@pa()").is_none());
        assert!(parse_pa_annotation("@pb(4)").is_none());
        assert!(parse_pa_annotation("hello").is_none());
    }

    #[test]
    fn parse_attr_form() {
        let ann = parse_autotune_attr("#[perf::autotune(variants = 4)]").unwrap();
        assert_eq!(ann.variant_count, 4);
    }

    #[test]
    fn parse_attr_invalid() {
        assert!(parse_autotune_attr("#[perf::other(variants = 4)]").is_none());
        assert!(parse_autotune_attr("not an attr").is_none());
    }

    #[test]
    fn annotation_with_strategies() {
        let ann = AutotuneAnnotation::new(2)
            .with_strategies(vec![LoweringStrategy::Tiled, LoweringStrategy::Vectorized]);
        assert_eq!(ann.variant_count, 2);
        assert_eq!(ann.strategies.len(), 2);
    }

    // ── Target Hardware Display ──────────────────────────────────────────

    #[test]
    fn target_hardware_display() {
        assert_eq!(TargetHardware::Cpu.to_string(), "CPU");
        assert_eq!(TargetHardware::Gpu.to_string(), "GPU");
        assert_eq!(TargetHardware::Npu.to_string(), "NPU");
    }

    // ── Config ───────────────────────────────────────────────────────────

    #[test]
    fn config_effective_strategies_default() {
        let config = AutotuneConfig::new(3, TargetProfile::generic_cpu());
        let strats = config.effective_strategies();
        assert_eq!(strats.len(), 3);
    }

    #[test]
    fn config_effective_strategies_custom() {
        let mut config = AutotuneConfig::new(5, TargetProfile::generic_cpu());
        config.strategies = vec![LoweringStrategy::Scalar, LoweringStrategy::Vectorized];
        let strats = config.effective_strategies();
        assert_eq!(strats.len(), 2); // capped by available strategies
    }
}
