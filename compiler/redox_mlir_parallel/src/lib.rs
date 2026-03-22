//! # Redox MLIR Parallel Dialects
//!
//! Implements hardware-agnostic parallelism through MLIR's OpenMP, GPU, and
//! async dialects. High-level parallel intent is preserved from source through
//! the lowering pipeline, then dispatched to the appropriate hardware backend.
//!
//! Pipeline:
//! ```text
//! Parallel intent (parallel_for, gpu_launch, async) →
//!   MLIR OpenMP dialect (CPU threading)
//!   MLIR GPU dialect (GPU compute)
//!   MLIR async dialect (asynchronous execution)
//! → Target-specific lowering (pthreads, CUDA, Vulkan, etc.)
//! ```
//!
//! Reference: REDOX_PROPOSAL.md §5.4 — "Parallelism preservation: explicit in
//! MLIR OpenMP/GPU/async dialects"
//!
//! (ROADMAP Step 60)

use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Parallelism Model
// ═══════════════════════════════════════════════════════════════════════════

/// The parallelism dialect to target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParallelDialect {
    /// OpenMP dialect — CPU multi-threading.
    OpenMP,
    /// GPU dialect — GPU compute kernels.
    Gpu,
    /// Async dialect — asynchronous / concurrent execution.
    Async,
}

impl fmt::Display for ParallelDialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParallelDialect::OpenMP => write!(f, "omp"),
            ParallelDialect::Gpu => write!(f, "gpu"),
            ParallelDialect::Async => write!(f, "async"),
        }
    }
}

/// Hardware-agnostic parallel region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParallelRegionKind {
    /// Parallel for-loop (data parallelism).
    ParallelFor {
        /// Iteration count.
        iterations: u64,
        /// Whether iterations are independent (no cross-iteration deps).
        independent: bool,
    },
    /// Parallel sections (task parallelism).
    ParallelSections {
        section_count: usize,
    },
    /// GPU kernel launch.
    GpuLaunch {
        grid: GridDim,
        block: BlockDim,
    },
    /// Async task spawn.
    AsyncTask {
        /// Task group name.
        group: String,
    },
    /// Barrier / synchronization point.
    Barrier,
    /// Reduction operation.
    Reduction {
        op: ReductionOp,
    },
}

/// Grid dimensions for GPU launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridDim {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl GridDim {
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        GridDim { x, y, z }
    }

    pub fn linear(n: u32) -> Self {
        GridDim { x: n, y: 1, z: 1 }
    }

    pub fn total_blocks(&self) -> u32 {
        self.x * self.y * self.z
    }
}

impl fmt::Display for GridDim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "grid({}, {}, {})", self.x, self.y, self.z)
    }
}

/// Block dimensions for GPU launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockDim {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl BlockDim {
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        BlockDim { x, y, z }
    }

    pub fn linear(n: u32) -> Self {
        BlockDim { x: n, y: 1, z: 1 }
    }

    pub fn total_threads(&self) -> u32 {
        self.x * self.y * self.z
    }
}

impl fmt::Display for BlockDim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "block({}, {}, {})", self.x, self.y, self.z)
    }
}

/// Supported reduction operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReductionOp {
    Add,
    Mul,
    Min,
    Max,
    And,
    Or,
    Xor,
}

impl fmt::Display for ReductionOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReductionOp::Add => write!(f, "add"),
            ReductionOp::Mul => write!(f, "mul"),
            ReductionOp::Min => write!(f, "min"),
            ReductionOp::Max => write!(f, "max"),
            ReductionOp::And => write!(f, "and"),
            ReductionOp::Or => write!(f, "or"),
            ReductionOp::Xor => write!(f, "xor"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Lowered Parallel Operations
// ═══════════════════════════════════════════════════════════════════════════

/// A lowered parallel operation in a specific dialect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParallelOp {
    pub dialect: ParallelDialect,
    pub opcode: String,
    pub operands: Vec<String>,
    pub comment: String,
}

impl ParallelOp {
    pub fn new(dialect: ParallelDialect, opcode: &str, comment: &str) -> Self {
        ParallelOp {
            dialect,
            opcode: opcode.to_string(),
            operands: Vec::new(),
            comment: comment.to_string(),
        }
    }

    pub fn with_operands(mut self, operands: &[&str]) -> Self {
        self.operands = operands.iter().map(|o| o.to_string()).collect();
        self
    }
}

impl fmt::Display for ParallelOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.operands.is_empty() {
            write!(f, "{}.{}", self.dialect, self.opcode)
        } else {
            write!(f, "{}.{} {}", self.dialect, self.opcode, self.operands.join(", "))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OpenMP Dialect Lowering
// ═══════════════════════════════════════════════════════════════════════════

/// Lower a parallel region to OpenMP dialect ops.
pub fn lower_to_openmp(region: &ParallelRegionKind) -> Vec<ParallelOp> {
    match region {
        ParallelRegionKind::ParallelFor { iterations, independent } => {
            let mut ops = vec![
                ParallelOp::new(ParallelDialect::OpenMP, "parallel", "begin parallel region"),
            ];
            if *independent {
                ops.push(
                    ParallelOp::new(ParallelDialect::OpenMP, "wsloop", "worksharing loop")
                        .with_operands(&[&format!("trips={iterations}")]),
                );
            } else {
                ops.push(
                    ParallelOp::new(ParallelDialect::OpenMP, "ordered_wsloop", "ordered worksharing loop")
                        .with_operands(&[&format!("trips={iterations}")]),
                );
            }
            ops.push(ParallelOp::new(ParallelDialect::OpenMP, "terminator", "end parallel"));
            ops
        }
        ParallelRegionKind::ParallelSections { section_count } => {
            let mut ops = vec![
                ParallelOp::new(ParallelDialect::OpenMP, "parallel", "begin parallel sections"),
                ParallelOp::new(ParallelDialect::OpenMP, "sections", &format!("{section_count} sections")),
            ];
            for i in 0..*section_count {
                ops.push(ParallelOp::new(
                    ParallelDialect::OpenMP,
                    "section",
                    &format!("section {i}"),
                ));
            }
            ops.push(ParallelOp::new(ParallelDialect::OpenMP, "terminator", "end sections"));
            ops
        }
        ParallelRegionKind::Barrier => {
            vec![ParallelOp::new(ParallelDialect::OpenMP, "barrier", "thread barrier")]
        }
        ParallelRegionKind::Reduction { op } => {
            vec![
                ParallelOp::new(ParallelDialect::OpenMP, "parallel", "begin reduction region"),
                ParallelOp::new(
                    ParallelDialect::OpenMP,
                    "reduction",
                    &format!("reduce with {op}"),
                ),
                ParallelOp::new(ParallelDialect::OpenMP, "terminator", "end reduction"),
            ]
        }
        ParallelRegionKind::GpuLaunch { .. } => {
            // OpenMP can offload to GPU via target
            vec![
                ParallelOp::new(ParallelDialect::OpenMP, "target", "offload to device"),
                ParallelOp::new(ParallelDialect::OpenMP, "teams", "create teams"),
                ParallelOp::new(ParallelDialect::OpenMP, "distribute", "distribute work"),
                ParallelOp::new(ParallelDialect::OpenMP, "terminator", "end offload"),
            ]
        }
        ParallelRegionKind::AsyncTask { group } => {
            vec![
                ParallelOp::new(ParallelDialect::OpenMP, "task", &format!("spawn task: {group}")),
                ParallelOp::new(ParallelDialect::OpenMP, "terminator", "end task"),
            ]
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GPU Dialect Lowering
// ═══════════════════════════════════════════════════════════════════════════

/// Lower a parallel region to GPU dialect ops.
pub fn lower_to_gpu(region: &ParallelRegionKind) -> Vec<ParallelOp> {
    match region {
        ParallelRegionKind::GpuLaunch { grid, block } => {
            vec![
                ParallelOp::new(ParallelDialect::Gpu, "launch", "launch GPU kernel")
                    .with_operands(&[
                        &format!("gridX={}", grid.x),
                        &format!("gridY={}", grid.y),
                        &format!("gridZ={}", grid.z),
                        &format!("blockX={}", block.x),
                        &format!("blockY={}", block.y),
                        &format!("blockZ={}", block.z),
                    ]),
                ParallelOp::new(ParallelDialect::Gpu, "terminator", "end kernel"),
            ]
        }
        ParallelRegionKind::ParallelFor { iterations, .. } => {
            // Map parallel for to GPU: compute grid dimensions
            let block_size = 256u32;
            let grid_size = ((*iterations as u32).max(1) + block_size - 1) / block_size;
            vec![
                ParallelOp::new(ParallelDialect::Gpu, "alloc", "allocate device memory"),
                ParallelOp::new(ParallelDialect::Gpu, "memcpy", "host→device transfer"),
                ParallelOp::new(ParallelDialect::Gpu, "launch", "launch kernel")
                    .with_operands(&[
                        &format!("gridX={grid_size}"),
                        &format!("blockX={block_size}"),
                    ]),
                ParallelOp::new(ParallelDialect::Gpu, "memcpy", "device→host transfer"),
                ParallelOp::new(ParallelDialect::Gpu, "dealloc", "free device memory"),
            ]
        }
        ParallelRegionKind::Barrier => {
            vec![ParallelOp::new(ParallelDialect::Gpu, "barrier", "__syncthreads()")]
        }
        ParallelRegionKind::Reduction { op } => {
            vec![
                ParallelOp::new(ParallelDialect::Gpu, "shuffle_down", &format!("warp reduce {op}")),
                ParallelOp::new(ParallelDialect::Gpu, "shared_store", "store to shared mem"),
                ParallelOp::new(ParallelDialect::Gpu, "barrier", "sync after store"),
                ParallelOp::new(ParallelDialect::Gpu, "shared_load", "load from shared mem"),
            ]
        }
        ParallelRegionKind::ParallelSections { section_count } => {
            // Map sections to GPU thread blocks
            vec![
                ParallelOp::new(ParallelDialect::Gpu, "launch", &format!("{section_count} blocks"))
                    .with_operands(&[&format!("gridX={section_count}"), "blockX=1"]),
                ParallelOp::new(ParallelDialect::Gpu, "terminator", "end kernel"),
            ]
        }
        ParallelRegionKind::AsyncTask { group } => {
            // Map async to GPU stream
            vec![
                ParallelOp::new(ParallelDialect::Gpu, "stream_create", &format!("stream for {group}")),
                ParallelOp::new(ParallelDialect::Gpu, "launch_on_stream", "async kernel launch"),
                ParallelOp::new(ParallelDialect::Gpu, "stream_sync", "sync stream"),
            ]
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Async Dialect Lowering
// ═══════════════════════════════════════════════════════════════════════════

/// Lower a parallel region to async dialect ops.
pub fn lower_to_async(region: &ParallelRegionKind) -> Vec<ParallelOp> {
    match region {
        ParallelRegionKind::AsyncTask { group } => {
            vec![
                ParallelOp::new(ParallelDialect::Async, "execute", &format!("spawn async: {group}")),
                ParallelOp::new(ParallelDialect::Async, "yield", "yield token"),
            ]
        }
        ParallelRegionKind::ParallelFor { iterations, .. } => {
            vec![
                ParallelOp::new(ParallelDialect::Async, "create_group", &format!("{iterations} tasks")),
                ParallelOp::new(ParallelDialect::Async, "execute", "launch task"),
                ParallelOp::new(ParallelDialect::Async, "yield", "yield token"),
                ParallelOp::new(ParallelDialect::Async, "await_all", "join all tasks"),
            ]
        }
        ParallelRegionKind::ParallelSections { section_count } => {
            let mut ops = vec![
                ParallelOp::new(ParallelDialect::Async, "create_group", &format!("{section_count} sections")),
            ];
            for i in 0..*section_count {
                ops.push(ParallelOp::new(
                    ParallelDialect::Async,
                    "execute",
                    &format!("section {i}"),
                ));
            }
            ops.push(ParallelOp::new(ParallelDialect::Async, "await_all", "join sections"));
            ops
        }
        ParallelRegionKind::Barrier => {
            vec![ParallelOp::new(ParallelDialect::Async, "await_all", "barrier via await")]
        }
        ParallelRegionKind::Reduction { op } => {
            vec![
                ParallelOp::new(ParallelDialect::Async, "create_group", "reduction tasks"),
                ParallelOp::new(ParallelDialect::Async, "execute", &format!("partial reduce {op}")),
                ParallelOp::new(ParallelDialect::Async, "await_all", "gather partials"),
                ParallelOp::new(ParallelDialect::Async, "execute", &format!("final reduce {op}")),
            ]
        }
        ParallelRegionKind::GpuLaunch { .. } => {
            // Async wraps GPU launch for non-blocking dispatch
            vec![
                ParallelOp::new(ParallelDialect::Async, "execute", "async GPU dispatch"),
                ParallelOp::new(ParallelDialect::Async, "yield", "return token for GPU completion"),
            ]
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Unified Lowering
// ═══════════════════════════════════════════════════════════════════════════

/// Lower a parallel region to the specified dialect.
pub fn lower_parallel(region: &ParallelRegionKind, dialect: ParallelDialect) -> Vec<ParallelOp> {
    match dialect {
        ParallelDialect::OpenMP => lower_to_openmp(region),
        ParallelDialect::Gpu => lower_to_gpu(region),
        ParallelDialect::Async => lower_to_async(region),
    }
}

/// Automatically select the best dialect for a parallel region.
pub fn auto_select_dialect(region: &ParallelRegionKind) -> ParallelDialect {
    match region {
        ParallelRegionKind::GpuLaunch { .. } => ParallelDialect::Gpu,
        ParallelRegionKind::AsyncTask { .. } => ParallelDialect::Async,
        ParallelRegionKind::ParallelFor { iterations, .. } => {
            // Large iteration counts → GPU; small → OpenMP
            if *iterations > 10_000 {
                ParallelDialect::Gpu
            } else {
                ParallelDialect::OpenMP
            }
        }
        ParallelRegionKind::ParallelSections { section_count } => {
            if *section_count > 32 {
                ParallelDialect::Gpu
            } else {
                ParallelDialect::OpenMP
            }
        }
        ParallelRegionKind::Barrier => ParallelDialect::OpenMP,
        ParallelRegionKind::Reduction { .. } => ParallelDialect::OpenMP,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Parallel Pipeline
// ═══════════════════════════════════════════════════════════════════════════

/// A parallel region in the pipeline.
#[derive(Debug, Clone)]
pub struct ParallelRegion {
    pub name: String,
    pub kind: ParallelRegionKind,
}

impl ParallelRegion {
    pub fn new(name: &str, kind: ParallelRegionKind) -> Self {
        ParallelRegion { name: name.to_string(), kind }
    }
}

/// Result of lowering a parallel program.
#[derive(Debug, Clone)]
pub struct ParallelPipelineResult {
    pub regions: Vec<(String, ParallelDialect, Vec<ParallelOp>)>,
}

impl ParallelPipelineResult {
    pub fn total_ops(&self) -> usize {
        self.regions.iter().map(|(_, _, ops)| ops.len()).sum()
    }

    pub fn dialects_used(&self) -> Vec<ParallelDialect> {
        let mut dialects: Vec<ParallelDialect> = self.regions.iter().map(|(_, d, _)| *d).collect();
        dialects.sort_by_key(|d| format!("{d}"));
        dialects.dedup();
        dialects
    }
}

/// Lower multiple parallel regions, auto-selecting dialects.
pub fn lower_parallel_program(regions: &[ParallelRegion]) -> ParallelPipelineResult {
    let lowered: Vec<(String, ParallelDialect, Vec<ParallelOp>)> = regions
        .iter()
        .map(|r| {
            let dialect = auto_select_dialect(&r.kind);
            let ops = lower_parallel(&r.kind, dialect);
            (r.name.clone(), dialect, ops)
        })
        .collect();

    ParallelPipelineResult { regions: lowered }
}

/// Lower multiple parallel regions with explicit dialect choices.
pub fn lower_parallel_program_with_dialect(
    regions: &[(ParallelRegion, ParallelDialect)],
) -> ParallelPipelineResult {
    let lowered: Vec<(String, ParallelDialect, Vec<ParallelOp>)> = regions
        .iter()
        .map(|(r, d)| {
            let ops = lower_parallel(&r.kind, *d);
            (r.name.clone(), *d, ops)
        })
        .collect();

    ParallelPipelineResult { regions: lowered }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── Dialect Display ──────────────────────────────────────────────────

    #[test]
    fn dialect_display() {
        assert_eq!(ParallelDialect::OpenMP.to_string(), "omp");
        assert_eq!(ParallelDialect::Gpu.to_string(), "gpu");
        assert_eq!(ParallelDialect::Async.to_string(), "async");
    }

    // ── GridDim / BlockDim ───────────────────────────────────────────────

    #[test]
    fn grid_dim_linear() {
        let g = GridDim::linear(128);
        assert_eq!(g.total_blocks(), 128);
        assert_eq!(g.to_string(), "grid(128, 1, 1)");
    }

    #[test]
    fn block_dim_3d() {
        let b = BlockDim::new(16, 16, 4);
        assert_eq!(b.total_threads(), 1024);
    }

    // ── Reduction Op ─────────────────────────────────────────────────────

    #[test]
    fn reduction_op_display() {
        assert_eq!(ReductionOp::Add.to_string(), "add");
        assert_eq!(ReductionOp::Max.to_string(), "max");
        assert_eq!(ReductionOp::Xor.to_string(), "xor");
    }

    // ── ParallelOp Display ───────────────────────────────────────────────

    #[test]
    fn parallel_op_display_no_operands() {
        let op = ParallelOp::new(ParallelDialect::OpenMP, "barrier", "test");
        assert_eq!(format!("{op}"), "omp.barrier");
    }

    #[test]
    fn parallel_op_display_with_operands() {
        let op = ParallelOp::new(ParallelDialect::Gpu, "launch", "test")
            .with_operands(&["gridX=4", "blockX=256"]);
        assert_eq!(format!("{op}"), "gpu.launch gridX=4, blockX=256");
    }

    // ── OpenMP Lowering ──────────────────────────────────────────────────

    #[test]
    fn omp_parallel_for() {
        let region = ParallelRegionKind::ParallelFor { iterations: 1000, independent: true };
        let ops = lower_to_openmp(&region);
        assert!(ops.iter().any(|o| o.opcode == "parallel"));
        assert!(ops.iter().any(|o| o.opcode == "wsloop"));
        assert!(ops.iter().any(|o| o.opcode == "terminator"));
    }

    #[test]
    fn omp_parallel_for_ordered() {
        let region = ParallelRegionKind::ParallelFor { iterations: 100, independent: false };
        let ops = lower_to_openmp(&region);
        assert!(ops.iter().any(|o| o.opcode == "ordered_wsloop"));
    }

    #[test]
    fn omp_sections() {
        let region = ParallelRegionKind::ParallelSections { section_count: 4 };
        let ops = lower_to_openmp(&region);
        let section_count = ops.iter().filter(|o| o.opcode == "section").count();
        assert_eq!(section_count, 4);
    }

    #[test]
    fn omp_barrier() {
        let region = ParallelRegionKind::Barrier;
        let ops = lower_to_openmp(&region);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].opcode, "barrier");
    }

    #[test]
    fn omp_reduction() {
        let region = ParallelRegionKind::Reduction { op: ReductionOp::Add };
        let ops = lower_to_openmp(&region);
        assert!(ops.iter().any(|o| o.opcode == "reduction"));
    }

    #[test]
    fn omp_gpu_offload() {
        let region = ParallelRegionKind::GpuLaunch {
            grid: GridDim::linear(64),
            block: BlockDim::linear(256),
        };
        let ops = lower_to_openmp(&region);
        assert!(ops.iter().any(|o| o.opcode == "target"));
        assert!(ops.iter().any(|o| o.opcode == "teams"));
    }

    // ── GPU Dialect Lowering ─────────────────────────────────────────────

    #[test]
    fn gpu_launch() {
        let region = ParallelRegionKind::GpuLaunch {
            grid: GridDim::new(16, 16, 1),
            block: BlockDim::new(32, 8, 1),
        };
        let ops = lower_to_gpu(&region);
        assert!(ops.iter().any(|o| o.opcode == "launch"));
        // Should have grid and block operands
        let launch = ops.iter().find(|o| o.opcode == "launch").unwrap();
        assert!(launch.operands.iter().any(|o| o.contains("gridX=16")));
        assert!(launch.operands.iter().any(|o| o.contains("blockX=32")));
    }

    #[test]
    fn gpu_parallel_for() {
        let region = ParallelRegionKind::ParallelFor { iterations: 100_000, independent: true };
        let ops = lower_to_gpu(&region);
        // Should include alloc, memcpy, launch, memcpy, dealloc
        assert!(ops.iter().any(|o| o.opcode == "alloc"));
        assert!(ops.iter().any(|o| o.opcode == "launch"));
        assert!(ops.iter().any(|o| o.opcode == "dealloc"));
    }

    #[test]
    fn gpu_barrier() {
        let region = ParallelRegionKind::Barrier;
        let ops = lower_to_gpu(&region);
        assert_eq!(ops[0].opcode, "barrier");
    }

    #[test]
    fn gpu_reduction() {
        let region = ParallelRegionKind::Reduction { op: ReductionOp::Max };
        let ops = lower_to_gpu(&region);
        assert!(ops.iter().any(|o| o.opcode == "shuffle_down"));
    }

    #[test]
    fn gpu_async_stream() {
        let region = ParallelRegionKind::AsyncTask { group: "compute".to_string() };
        let ops = lower_to_gpu(&region);
        assert!(ops.iter().any(|o| o.opcode == "stream_create"));
        assert!(ops.iter().any(|o| o.opcode == "stream_sync"));
    }

    // ── Async Dialect Lowering ───────────────────────────────────────────

    #[test]
    fn async_task() {
        let region = ParallelRegionKind::AsyncTask { group: "io".to_string() };
        let ops = lower_to_async(&region);
        assert!(ops.iter().any(|o| o.opcode == "execute"));
        assert!(ops.iter().any(|o| o.opcode == "yield"));
    }

    #[test]
    fn async_parallel_for() {
        let region = ParallelRegionKind::ParallelFor { iterations: 500, independent: true };
        let ops = lower_to_async(&region);
        assert!(ops.iter().any(|o| o.opcode == "create_group"));
        assert!(ops.iter().any(|o| o.opcode == "await_all"));
    }

    #[test]
    fn async_sections() {
        let region = ParallelRegionKind::ParallelSections { section_count: 3 };
        let ops = lower_to_async(&region);
        let exec_count = ops.iter().filter(|o| o.opcode == "execute").count();
        assert_eq!(exec_count, 3);
    }

    #[test]
    fn async_barrier() {
        let region = ParallelRegionKind::Barrier;
        let ops = lower_to_async(&region);
        assert!(ops.iter().any(|o| o.opcode == "await_all"));
    }

    #[test]
    fn async_reduction() {
        let region = ParallelRegionKind::Reduction { op: ReductionOp::Mul };
        let ops = lower_to_async(&region);
        assert!(ops.iter().any(|o| o.opcode == "create_group"));
    }

    // ── Unified Lowering ─────────────────────────────────────────────────

    #[test]
    fn unified_dispatch() {
        let region = ParallelRegionKind::Barrier;
        let omp = lower_parallel(&region, ParallelDialect::OpenMP);
        let gpu = lower_parallel(&region, ParallelDialect::Gpu);
        let async_ops = lower_parallel(&region, ParallelDialect::Async);
        assert_eq!(omp[0].dialect, ParallelDialect::OpenMP);
        assert_eq!(gpu[0].dialect, ParallelDialect::Gpu);
        assert_eq!(async_ops[0].dialect, ParallelDialect::Async);
    }

    // ── Auto Selection ───────────────────────────────────────────────────

    #[test]
    fn auto_select_gpu_launch() {
        let region = ParallelRegionKind::GpuLaunch {
            grid: GridDim::linear(64),
            block: BlockDim::linear(256),
        };
        assert_eq!(auto_select_dialect(&region), ParallelDialect::Gpu);
    }

    #[test]
    fn auto_select_async_task() {
        let region = ParallelRegionKind::AsyncTask { group: "io".to_string() };
        assert_eq!(auto_select_dialect(&region), ParallelDialect::Async);
    }

    #[test]
    fn auto_select_large_for_goes_gpu() {
        let region = ParallelRegionKind::ParallelFor { iterations: 1_000_000, independent: true };
        assert_eq!(auto_select_dialect(&region), ParallelDialect::Gpu);
    }

    #[test]
    fn auto_select_small_for_goes_omp() {
        let region = ParallelRegionKind::ParallelFor { iterations: 100, independent: true };
        assert_eq!(auto_select_dialect(&region), ParallelDialect::OpenMP);
    }

    // ── Pipeline ─────────────────────────────────────────────────────────

    #[test]
    fn pipeline_multi_region() {
        let regions = vec![
            ParallelRegion::new("loop", ParallelRegionKind::ParallelFor {
                iterations: 100_000,
                independent: true,
            }),
            ParallelRegion::new("io", ParallelRegionKind::AsyncTask {
                group: "io".to_string(),
            }),
            ParallelRegion::new("sync", ParallelRegionKind::Barrier),
        ];
        let result = lower_parallel_program(&regions);
        assert_eq!(result.regions.len(), 3);
        assert!(result.total_ops() > 0);
    }

    #[test]
    fn pipeline_dialects_used() {
        let regions = vec![
            ParallelRegion::new("a", ParallelRegionKind::ParallelFor {
                iterations: 100_000,
                independent: true,
            }),
            ParallelRegion::new("b", ParallelRegionKind::AsyncTask {
                group: "io".to_string(),
            }),
        ];
        let result = lower_parallel_program(&regions);
        let dialects = result.dialects_used();
        assert!(dialects.len() >= 2);
    }

    #[test]
    fn pipeline_explicit_dialect() {
        let regions = vec![
            (
                ParallelRegion::new("loop", ParallelRegionKind::ParallelFor {
                    iterations: 100,
                    independent: true,
                }),
                ParallelDialect::Async,
            ),
        ];
        let result = lower_parallel_program_with_dialect(&regions);
        assert_eq!(result.regions[0].1, ParallelDialect::Async);
    }
}
