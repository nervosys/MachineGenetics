//! # ACI Performance Advisor Engine
//!
//! Generates performance suggestions by combining MLIR cost model analysis
//! with runtime profiling data. Unlike generic profilers, the advisor
//! understands the MLIR optimization pipeline and can suggest specific
//! transformations (tiling, vectorization, device placement) with predicted
//! speedup estimates.
//!
//! Pipeline:
//! ```text
//! MLIR Cost Model ──┐
//!                   ├──▶ Opportunity Detector ──▶ Suggestion Ranker ──▶ PerfAdvice
//! Profiling Data ───┘                                  ↑
//!                                             Target Hardware Profile
//! ```
//!
//! Reference: REDOX_PROPOSAL.md — ACI Performance Advisor Engine
//!   "MLIR cost model + profiling data suggestions"
//!
//! (ROADMAP Step 64)

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Profiling Data
// ═══════════════════════════════════════════════════════════════════════════

/// A profiling measurement for a code region.
#[derive(Debug, Clone)]
pub struct ProfileEntry {
    pub region_name: String,
    pub file: String,
    pub line: u32,
    /// Wall-clock time in microseconds.
    pub wall_time_us: f64,
    /// Number of times this region was executed.
    pub call_count: u64,
    /// Total allocated bytes during this region.
    pub alloc_bytes: u64,
    /// Peak memory usage in bytes.
    pub peak_memory_bytes: u64,
    /// Number of cache misses (if available).
    pub cache_misses: Option<u64>,
    /// FLOP count (if measurable).
    pub flop_count: Option<u64>,
}

impl ProfileEntry {
    pub fn new(name: &str, file: &str, line: u32, wall_time_us: f64) -> Self {
        ProfileEntry {
            region_name: name.to_string(),
            file: file.to_string(),
            line,
            wall_time_us,
            call_count: 1,
            alloc_bytes: 0,
            peak_memory_bytes: 0,
            cache_misses: None,
            flop_count: None,
        }
    }

    pub fn with_calls(mut self, count: u64) -> Self {
        self.call_count = count;
        self
    }

    pub fn with_alloc(mut self, bytes: u64) -> Self {
        self.alloc_bytes = bytes;
        self
    }

    pub fn with_cache_misses(mut self, misses: u64) -> Self {
        self.cache_misses = Some(misses);
        self
    }

    pub fn with_flops(mut self, flops: u64) -> Self {
        self.flop_count = Some(flops);
        self
    }

    /// Average time per call in microseconds.
    pub fn avg_time_us(&self) -> f64 {
        if self.call_count > 0 {
            self.wall_time_us / self.call_count as f64
        } else {
            self.wall_time_us
        }
    }

    /// Arithmetic intensity (FLOP/byte) if both metrics available.
    pub fn arithmetic_intensity(&self) -> Option<f64> {
        match (self.flop_count, self.alloc_bytes) {
            (Some(flops), bytes) if bytes > 0 => Some(flops as f64 / bytes as f64),
            _ => None,
        }
    }
}

/// A profile of the full program.
#[derive(Debug, Clone)]
pub struct ProgramProfile {
    pub entries: Vec<ProfileEntry>,
}

impl ProgramProfile {
    pub fn new(entries: Vec<ProfileEntry>) -> Self {
        ProgramProfile { entries }
    }

    pub fn total_time_us(&self) -> f64 {
        self.entries.iter().map(|e| e.wall_time_us).sum()
    }

    /// Entries sorted by wall time descending (hottest first).
    pub fn hotspots(&self) -> Vec<&ProfileEntry> {
        let mut sorted: Vec<&ProfileEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| {
            b.wall_time_us.partial_cmp(&a.wall_time_us).unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted
    }

    /// Fraction of total time spent in a region.
    pub fn time_fraction(&self, entry: &ProfileEntry) -> f64 {
        let total = self.total_time_us();
        if total > 0.0 { entry.wall_time_us / total } else { 0.0 }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// MLIR Cost Model Integration
// ═══════════════════════════════════════════════════════════════════════════

/// MLIR cost estimate for a region.
#[derive(Debug, Clone)]
pub struct MlirCostEstimate {
    pub region_name: String,
    /// Estimated cycles on target.
    pub estimated_cycles: u64,
    /// Estimated memory traffic (bytes).
    pub estimated_memory_bytes: u64,
    /// Whether vectorization is applicable.
    pub vectorizable: bool,
    /// Whether tiling is applicable.
    pub tileable: bool,
    /// Whether the region can be offloaded to GPU.
    pub gpu_offloadable: bool,
    /// Whether the region has parallelism potential.
    pub parallelizable: bool,
    /// Current optimization level applied.
    pub current_opt_level: OptLevel,
}

/// Optimization level currently applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    None,
    Basic,
    Aggressive,
    TargetSpecific,
}

impl fmt::Display for OptLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptLevel::None => write!(f, "O0"),
            OptLevel::Basic => write!(f, "O1"),
            OptLevel::Aggressive => write!(f, "O2"),
            OptLevel::TargetSpecific => write!(f, "O3"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Performance Suggestions
// ═══════════════════════════════════════════════════════════════════════════

/// Category of performance suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuggestionKind {
    Vectorize,
    Tile,
    GpuOffload,
    Parallelize,
    ReduceAllocations,
    ImproveLocality,
    InlineFunction,
    LoopUnroll,
    Fuse,
    ChangeDataLayout,
}

impl fmt::Display for SuggestionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SuggestionKind::Vectorize => write!(f, "vectorize"),
            SuggestionKind::Tile => write!(f, "tile"),
            SuggestionKind::GpuOffload => write!(f, "gpu-offload"),
            SuggestionKind::Parallelize => write!(f, "parallelize"),
            SuggestionKind::ReduceAllocations => write!(f, "reduce-alloc"),
            SuggestionKind::ImproveLocality => write!(f, "improve-locality"),
            SuggestionKind::InlineFunction => write!(f, "inline"),
            SuggestionKind::LoopUnroll => write!(f, "unroll"),
            SuggestionKind::Fuse => write!(f, "fuse"),
            SuggestionKind::ChangeDataLayout => write!(f, "layout-change"),
        }
    }
}

/// A performance suggestion from the advisor.
#[derive(Debug, Clone)]
pub struct PerfSuggestion {
    pub kind: SuggestionKind,
    pub region_name: String,
    pub file: String,
    pub line: u32,
    pub message: String,
    /// Annotation to add (e.g., `#[perf::vectorize]`).
    pub annotation: String,
    /// Estimated speedup factor (e.g., 2.5 = 2.5× faster).
    pub estimated_speedup: f64,
    /// Priority score: higher = more impactful.
    pub priority: f64,
}

impl PerfSuggestion {
    pub fn estimated_time_savings_us(&self, current_time_us: f64) -> f64 {
        if self.estimated_speedup > 1.0 {
            current_time_us * (1.0 - 1.0 / self.estimated_speedup)
        } else {
            0.0
        }
    }
}

impl fmt::Display for PerfSuggestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{:.1}×] {} at {}:{} — {} (add: {})",
            self.estimated_speedup, self.kind, self.file, self.line, self.message, self.annotation
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Opportunity Detection
// ═══════════════════════════════════════════════════════════════════════════

/// Detect optimization opportunities from cost model and profiling data.
pub fn detect_opportunities(
    profile: &ProgramProfile,
    costs: &[MlirCostEstimate],
) -> Vec<PerfSuggestion> {
    let mut suggestions = Vec::new();

    for entry in &profile.entries {
        let time_frac = profile.time_fraction(entry);
        // Only suggest for hot regions (>5% of total time)
        if time_frac < 0.05 {
            continue;
        }

        let cost = costs.iter().find(|c| c.region_name == entry.region_name);

        // Vectorization opportunity
        if let Some(c) = cost {
            if c.vectorizable && c.current_opt_level == OptLevel::None {
                suggestions.push(PerfSuggestion {
                    kind: SuggestionKind::Vectorize,
                    region_name: entry.region_name.clone(),
                    file: entry.file.clone(),
                    line: entry.line,
                    message: "Loop is vectorizable but not vectorized".to_string(),
                    annotation: "#[perf::vectorize]".to_string(),
                    estimated_speedup: 4.0,
                    priority: time_frac * 4.0,
                });
            }
        }

        // Tiling opportunity
        if let Some(c) = cost {
            if c.tileable && entry.cache_misses.unwrap_or(0) > 1000 {
                suggestions.push(PerfSuggestion {
                    kind: SuggestionKind::Tile,
                    region_name: entry.region_name.clone(),
                    file: entry.file.clone(),
                    line: entry.line,
                    message: format!(
                        "High cache misses ({}); tiling would improve locality",
                        entry.cache_misses.unwrap_or(0)
                    ),
                    annotation: "#[perf::tile(32)]".to_string(),
                    estimated_speedup: 2.0,
                    priority: time_frac * 2.0,
                });
            }
        }

        // GPU offload opportunity
        if let Some(c) = cost {
            if c.gpu_offloadable && entry.wall_time_us > 1000.0 {
                let ai = entry.arithmetic_intensity().unwrap_or(0.0);
                let speedup = if ai > 10.0 { 20.0 } else { 5.0 };
                suggestions.push(PerfSuggestion {
                    kind: SuggestionKind::GpuOffload,
                    region_name: entry.region_name.clone(),
                    file: entry.file.clone(),
                    line: entry.line,
                    message: "Compute-heavy region suitable for GPU offload".to_string(),
                    annotation: "#[perf::target(gpu)]".to_string(),
                    estimated_speedup: speedup,
                    priority: time_frac * speedup,
                });
            }
        }

        // Parallelization opportunity
        if let Some(c) = cost {
            if c.parallelizable && entry.wall_time_us > 500.0 {
                suggestions.push(PerfSuggestion {
                    kind: SuggestionKind::Parallelize,
                    region_name: entry.region_name.clone(),
                    file: entry.file.clone(),
                    line: entry.line,
                    message: "Region has independent iterations; can parallelize".to_string(),
                    annotation: "#[perf::parallel]".to_string(),
                    estimated_speedup: 4.0, // assume 4-core
                    priority: time_frac * 4.0,
                });
            }
        }

        // Allocation reduction opportunity
        if entry.alloc_bytes > 1_000_000 && time_frac > 0.1 {
            suggestions.push(PerfSuggestion {
                kind: SuggestionKind::ReduceAllocations,
                region_name: entry.region_name.clone(),
                file: entry.file.clone(),
                line: entry.line,
                message: format!(
                    "{}MB allocated in hot region; consider pre-allocation or arena",
                    entry.alloc_bytes / 1_000_000
                ),
                annotation: "#[perf::arena]".to_string(),
                estimated_speedup: 1.5,
                priority: time_frac * 1.5,
            });
        }

        // Inlining opportunity (many small calls)
        if entry.call_count > 10_000 && entry.avg_time_us() < 1.0 {
            suggestions.push(PerfSuggestion {
                kind: SuggestionKind::InlineFunction,
                region_name: entry.region_name.clone(),
                file: entry.file.clone(),
                line: entry.line,
                message: format!(
                    "Called {}× with {:.2}µs/call — inline candidate",
                    entry.call_count,
                    entry.avg_time_us()
                ),
                annotation: "#[inline(always)]".to_string(),
                estimated_speedup: 1.3,
                priority: time_frac * 1.3,
            });
        }
    }

    // Sort by priority (highest first)
    suggestions
        .sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap_or(std::cmp::Ordering::Equal));
    suggestions
}

// ═══════════════════════════════════════════════════════════════════════════
// Advisor Report
// ═══════════════════════════════════════════════════════════════════════════

/// Full performance advisor report.
#[derive(Debug)]
pub struct PerfAdvisorReport {
    pub suggestions: Vec<PerfSuggestion>,
    pub total_time_us: f64,
    pub hotspot_count: usize,
    pub estimated_total_speedup: f64,
}

impl PerfAdvisorReport {
    pub fn top_suggestion(&self) -> Option<&PerfSuggestion> {
        self.suggestions.first()
    }

    pub fn suggestions_by_kind(&self, kind: SuggestionKind) -> Vec<&PerfSuggestion> {
        self.suggestions.iter().filter(|s| s.kind == kind).collect()
    }

    pub fn summary(&self) -> String {
        let top =
            self.top_suggestion().map(|s| format!("{s}")).unwrap_or_else(|| "none".to_string());
        format!(
            "Performance Report: {} suggestions, {:.1}µs total, est. {:.1}× overall speedup\n  Top: {}",
            self.suggestions.len(),
            self.total_time_us,
            self.estimated_total_speedup,
            top,
        )
    }
}

/// Run the full advisor pipeline.
pub fn advise(profile: &ProgramProfile, costs: &[MlirCostEstimate]) -> PerfAdvisorReport {
    let suggestions = detect_opportunities(profile, costs);

    // Estimate overall speedup from applying all suggestions
    // Model: each suggestion reduces the hot region's contribution
    let total_time = profile.total_time_us();
    // Track max speedup per region to avoid double-counting
    let mut region_speedups: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    for suggestion in &suggestions {
        let entry = region_speedups.entry(suggestion.region_name.clone()).or_insert(1.0_f64);
        if suggestion.estimated_speedup > *entry {
            *entry = suggestion.estimated_speedup;
        }
    }

    let mut remaining_time = total_time;
    for (region_name, speedup) in &region_speedups {
        if let Some(entry) = profile.entries.iter().find(|e| e.region_name == *region_name) {
            let savings = entry.wall_time_us * (1.0 - 1.0 / speedup);
            remaining_time -= savings.min(remaining_time);
        }
    }
    let estimated_total_speedup =
        if remaining_time > 0.0 { total_time / remaining_time } else { 1.0 };

    let hotspot_count = profile.entries.iter().filter(|e| profile.time_fraction(e) >= 0.05).count();

    PerfAdvisorReport {
        suggestions,
        total_time_us: total_time,
        hotspot_count,
        estimated_total_speedup,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> ProgramProfile {
        ProgramProfile::new(vec![
            ProfileEntry::new("matmul", "compute.mg", 10, 50_000.0)
                .with_calls(1)
                .with_alloc(2_000_000)
                .with_cache_misses(50_000)
                .with_flops(1_000_000_000),
            ProfileEntry::new("sort", "util.mg", 50, 10_000.0).with_calls(100).with_alloc(500_000),
            ProfileEntry::new("parse", "io.mg", 5, 5_000.0).with_calls(50_000),
            ProfileEntry::new("tiny", "misc.mg", 1, 10.0).with_calls(1),
        ])
    }

    fn sample_costs() -> Vec<MlirCostEstimate> {
        vec![
            MlirCostEstimate {
                region_name: "matmul".to_string(),
                estimated_cycles: 500_000_000,
                estimated_memory_bytes: 8_000_000,
                vectorizable: true,
                tileable: true,
                gpu_offloadable: true,
                parallelizable: true,
                current_opt_level: OptLevel::None,
            },
            MlirCostEstimate {
                region_name: "sort".to_string(),
                estimated_cycles: 10_000_000,
                estimated_memory_bytes: 500_000,
                vectorizable: false,
                tileable: false,
                gpu_offloadable: false,
                parallelizable: true,
                current_opt_level: OptLevel::Basic,
            },
            MlirCostEstimate {
                region_name: "parse".to_string(),
                estimated_cycles: 5_000_000,
                estimated_memory_bytes: 100_000,
                vectorizable: false,
                tileable: false,
                gpu_offloadable: false,
                parallelizable: false,
                current_opt_level: OptLevel::Aggressive,
            },
        ]
    }

    // ── Profile Entry ────────────────────────────────────────────────────

    #[test]
    fn avg_time_per_call() {
        let entry = ProfileEntry::new("f", "a.mg", 1, 1000.0).with_calls(10);
        assert!((entry.avg_time_us() - 100.0).abs() < 0.01);
    }

    #[test]
    fn arithmetic_intensity_some() {
        let entry = ProfileEntry::new("f", "a.mg", 1, 100.0).with_alloc(1000).with_flops(10_000);
        assert!((entry.arithmetic_intensity().unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn arithmetic_intensity_none() {
        let entry = ProfileEntry::new("f", "a.mg", 1, 100.0);
        assert!(entry.arithmetic_intensity().is_none());
    }

    // ── Program Profile ──────────────────────────────────────────────────

    #[test]
    fn total_time() {
        let profile = sample_profile();
        assert!((profile.total_time_us() - 65010.0).abs() < 1.0);
    }

    #[test]
    fn hotspots_sorted() {
        let profile = sample_profile();
        let hs = profile.hotspots();
        assert_eq!(hs[0].region_name, "matmul");
        assert_eq!(hs[1].region_name, "sort");
    }

    #[test]
    fn time_fraction() {
        let profile = sample_profile();
        let frac = profile.time_fraction(&profile.entries[0]);
        assert!(frac > 0.7); // matmul dominates
    }

    // ── Opt Level Display ────────────────────────────────────────────────

    #[test]
    fn opt_level_display() {
        assert_eq!(OptLevel::None.to_string(), "O0");
        assert_eq!(OptLevel::TargetSpecific.to_string(), "O3");
    }

    // ── Suggestion Kind Display ──────────────────────────────────────────

    #[test]
    fn suggestion_kind_display() {
        assert_eq!(SuggestionKind::Vectorize.to_string(), "vectorize");
        assert_eq!(SuggestionKind::GpuOffload.to_string(), "gpu-offload");
    }

    // ── Opportunity Detection ────────────────────────────────────────────

    #[test]
    fn detect_vectorize() {
        let profile = sample_profile();
        let costs = sample_costs();
        let suggestions = detect_opportunities(&profile, &costs);
        let vectorize = suggestions.iter().any(|s| s.kind == SuggestionKind::Vectorize);
        assert!(vectorize, "should suggest vectorization for matmul");
    }

    #[test]
    fn detect_tile() {
        let profile = sample_profile();
        let costs = sample_costs();
        let suggestions = detect_opportunities(&profile, &costs);
        let tile = suggestions.iter().any(|s| s.kind == SuggestionKind::Tile);
        assert!(tile, "should suggest tiling for high-cache-miss matmul");
    }

    #[test]
    fn detect_gpu_offload() {
        let profile = sample_profile();
        let costs = sample_costs();
        let suggestions = detect_opportunities(&profile, &costs);
        let gpu = suggestions.iter().any(|s| s.kind == SuggestionKind::GpuOffload);
        assert!(gpu, "should suggest GPU offload for matmul");
    }

    #[test]
    fn detect_parallelize() {
        let profile = sample_profile();
        let costs = sample_costs();
        let suggestions = detect_opportunities(&profile, &costs);
        let par = suggestions.iter().any(|s| s.kind == SuggestionKind::Parallelize);
        assert!(par, "should suggest parallelization");
    }

    #[test]
    fn detect_reduce_alloc() {
        let profile = sample_profile();
        let costs = sample_costs();
        let suggestions = detect_opportunities(&profile, &costs);
        let alloc = suggestions.iter().any(|s| s.kind == SuggestionKind::ReduceAllocations);
        assert!(alloc, "should suggest allocation reduction for matmul");
    }

    #[test]
    fn detect_inline_many_calls() {
        let profile = sample_profile();
        let costs = sample_costs();
        let suggestions = detect_opportunities(&profile, &costs);
        let inline = suggestions.iter().any(|s| s.kind == SuggestionKind::InlineFunction);
        assert!(inline, "should suggest inlining for parse (50k calls, low avg time)");
    }

    #[test]
    fn no_suggestions_for_cold() {
        let profile = sample_profile();
        let costs = sample_costs();
        let suggestions = detect_opportunities(&profile, &costs);
        let tiny = suggestions.iter().any(|s| s.region_name == "tiny");
        assert!(!tiny, "should not suggest for cold region");
    }

    #[test]
    fn suggestions_sorted_by_priority() {
        let profile = sample_profile();
        let costs = sample_costs();
        let suggestions = detect_opportunities(&profile, &costs);
        for w in suggestions.windows(2) {
            assert!(w[0].priority >= w[1].priority);
        }
    }

    // ── PerfSuggestion ───────────────────────────────────────────────────

    #[test]
    fn time_savings() {
        let s = PerfSuggestion {
            kind: SuggestionKind::Vectorize,
            region_name: "x".to_string(),
            file: "x.mg".to_string(),
            line: 1,
            message: String::new(),
            annotation: String::new(),
            estimated_speedup: 4.0,
            priority: 1.0,
        };
        let savings = s.estimated_time_savings_us(1000.0);
        assert!((savings - 750.0).abs() < 1.0); // 1 - 1/4 = 0.75
    }

    #[test]
    fn suggestion_display() {
        let s = PerfSuggestion {
            kind: SuggestionKind::Tile,
            region_name: "matmul".to_string(),
            file: "compute.mg".to_string(),
            line: 10,
            message: "high cache misses".to_string(),
            annotation: "#[perf::tile(32)]".to_string(),
            estimated_speedup: 2.0,
            priority: 1.0,
        };
        let display = format!("{s}");
        assert!(display.contains("2.0×"));
        assert!(display.contains("tile"));
        assert!(display.contains("#[perf::tile(32)]"));
    }

    // ── Advisor Report ───────────────────────────────────────────────────

    #[test]
    fn advise_full_pipeline() {
        let profile = sample_profile();
        let costs = sample_costs();
        let report = advise(&profile, &costs);
        assert!(!report.suggestions.is_empty());
        assert!(report.estimated_total_speedup > 1.0);
        assert!(report.hotspot_count > 0);
    }

    #[test]
    fn report_summary() {
        let profile = sample_profile();
        let costs = sample_costs();
        let report = advise(&profile, &costs);
        let summary = report.summary();
        assert!(summary.contains("suggestions"));
        assert!(summary.contains("speedup"));
    }

    #[test]
    fn report_by_kind() {
        let profile = sample_profile();
        let costs = sample_costs();
        let report = advise(&profile, &costs);
        let gpu = report.suggestions_by_kind(SuggestionKind::GpuOffload);
        assert!(!gpu.is_empty());
    }

    #[test]
    fn report_top_suggestion() {
        let profile = sample_profile();
        let costs = sample_costs();
        let report = advise(&profile, &costs);
        assert!(report.top_suggestion().is_some());
    }

    // ── Edge Cases ───────────────────────────────────────────────────────

    #[test]
    fn empty_profile() {
        let profile = ProgramProfile::new(vec![]);
        let report = advise(&profile, &[]);
        assert!(report.suggestions.is_empty());
    }

    #[test]
    fn no_cost_data() {
        let profile = sample_profile();
        let suggestions = detect_opportunities(&profile, &[]);
        // Should still detect allocation-based and call-count-based suggestions
        let has_alloc_or_inline = suggestions.iter().any(|s| {
            s.kind == SuggestionKind::ReduceAllocations || s.kind == SuggestionKind::InlineFunction
        });
        assert!(has_alloc_or_inline);
    }
}
