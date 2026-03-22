//! # Redox Automatic Device Placement
//!
//! Implements the `@pt(auto)` annotation: MLIR cost model evaluates available
//! devices (CPU, GPU, NPU) and selects the optimal target for each kernel.
//! Decisions are agent-queryable via RAP.
//!
//! Pipeline:
//! ```text
//! @pt(auto) kernel →
//!   Cost model evaluates: CPU cost, GPU cost, NPU cost
//!   → Select lowest cost → Emit placement decision
//!   → Decision queryable: rap.query("placement_decision", func)
//! ```
//!
//! Reference: REDOX_PROPOSAL.md §5.4.2 — "Automatic device placement (@pt(auto)):
//! MLIR cost model, agent-queryable"
//!
//! (ROADMAP Step 59)

use std::collections::BTreeMap;
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Device Types
// ═══════════════════════════════════════════════════════════════════════════

/// Target device for kernel execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Device {
    Cpu,
    Gpu,
    Npu,
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Device::Cpu => write!(f, "CPU"),
            Device::Gpu => write!(f, "GPU"),
            Device::Npu => write!(f, "NPU"),
        }
    }
}

/// Placement annotation parsed from source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementTarget {
    /// `@pt(auto)` — compiler decides.
    Auto,
    /// `@pt(cpu)` — explicitly target CPU.
    Explicit(Device),
}

impl fmt::Display for PlacementTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacementTarget::Auto => write!(f, "@pt(auto)"),
            PlacementTarget::Explicit(d) => write!(f, "@pt({d})"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Device Capabilities
// ═══════════════════════════════════════════════════════════════════════════

/// Hardware capabilities of a device.
#[derive(Debug, Clone)]
pub struct DeviceCapabilities {
    pub device: Device,
    pub name: String,
    /// Peak GFLOPS (single precision).
    pub peak_gflops_f32: f64,
    /// Memory bandwidth in GB/s.
    pub memory_bandwidth_gbps: f64,
    /// Data transfer overhead (device→host or host→device) in microseconds.
    pub transfer_overhead_us: f64,
    /// Whether the device is available.
    pub available: bool,
    /// Compute units / cores.
    pub compute_units: u32,
    /// Supports vector operations.
    pub supports_vector: bool,
    /// Supports matrix operations (tensor cores, AMX, etc.).
    pub supports_matrix: bool,
}

impl DeviceCapabilities {
    pub fn generic_cpu() -> Self {
        DeviceCapabilities {
            device: Device::Cpu,
            name: "generic-cpu".to_string(),
            peak_gflops_f32: 200.0,
            memory_bandwidth_gbps: 50.0,
            transfer_overhead_us: 0.0, // no transfer cost
            available: true,
            compute_units: 8,
            supports_vector: true,
            supports_matrix: false,
        }
    }

    pub fn generic_gpu() -> Self {
        DeviceCapabilities {
            device: Device::Gpu,
            name: "generic-gpu".to_string(),
            peak_gflops_f32: 10000.0,
            memory_bandwidth_gbps: 900.0,
            transfer_overhead_us: 100.0,
            available: true,
            compute_units: 2048,
            supports_vector: true,
            supports_matrix: true,
        }
    }

    pub fn generic_npu() -> Self {
        DeviceCapabilities {
            device: Device::Npu,
            name: "generic-npu".to_string(),
            peak_gflops_f32: 50000.0,
            memory_bandwidth_gbps: 400.0,
            transfer_overhead_us: 200.0,
            available: true,
            compute_units: 256,
            supports_vector: false,
            supports_matrix: true,
        }
    }

    pub fn unavailable(device: Device) -> Self {
        DeviceCapabilities {
            device,
            name: format!("unavailable-{device}"),
            peak_gflops_f32: 0.0,
            memory_bandwidth_gbps: 0.0,
            transfer_overhead_us: f64::MAX,
            available: false,
            compute_units: 0,
            supports_vector: false,
            supports_matrix: false,
        }
    }
}

/// Set of available devices in the system.
#[derive(Debug, Clone)]
pub struct DeviceRegistry {
    devices: Vec<DeviceCapabilities>,
}

impl DeviceRegistry {
    pub fn new() -> Self {
        DeviceRegistry { devices: Vec::new() }
    }

    pub fn add(&mut self, cap: DeviceCapabilities) {
        self.devices.push(cap);
    }

    pub fn get(&self, device: Device) -> Option<&DeviceCapabilities> {
        self.devices.iter().find(|d| d.device == device && d.available)
    }

    pub fn available_devices(&self) -> Vec<Device> {
        self.devices.iter().filter(|d| d.available).map(|d| d.device).collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &DeviceCapabilities> {
        self.devices.iter().filter(|d| d.available)
    }

    /// Default registry with CPU + GPU + NPU.
    pub fn default_system() -> Self {
        let mut reg = DeviceRegistry::new();
        reg.add(DeviceCapabilities::generic_cpu());
        reg.add(DeviceCapabilities::generic_gpu());
        reg.add(DeviceCapabilities::generic_npu());
        reg
    }

    /// CPU-only system.
    pub fn cpu_only() -> Self {
        let mut reg = DeviceRegistry::new();
        reg.add(DeviceCapabilities::generic_cpu());
        reg
    }
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        DeviceRegistry::default_system()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Kernel Workload Characterization
// ═══════════════════════════════════════════════════════════════════════════

/// Characterization of a kernel's workload for cost modeling.
#[derive(Debug, Clone)]
pub struct WorkloadProfile {
    pub kernel_name: String,
    /// Total floating-point operations.
    pub flop_count: u64,
    /// Total memory bytes read.
    pub memory_read_bytes: u64,
    /// Total memory bytes written.
    pub memory_write_bytes: u64,
    /// Arithmetic intensity (FLOP / byte).
    pub arithmetic_intensity: f64,
    /// Whether the workload is embarrassingly parallel.
    pub is_parallel: bool,
    /// Whether the workload uses matrix operations.
    pub uses_matrix_ops: bool,
    /// Estimated parallelism (independent operations that can execute concurrently).
    pub parallelism: u64,
    /// Loop nest depth (deeper = more parallelizable on GPU).
    pub loop_depth: u32,
}

impl WorkloadProfile {
    pub fn new(kernel_name: &str) -> Self {
        WorkloadProfile {
            kernel_name: kernel_name.to_string(),
            flop_count: 0,
            memory_read_bytes: 0,
            memory_write_bytes: 0,
            arithmetic_intensity: 0.0,
            is_parallel: false,
            uses_matrix_ops: false,
            parallelism: 1,
            loop_depth: 0,
        }
    }

    /// Total memory traffic (read + write).
    pub fn total_memory_bytes(&self) -> u64 {
        self.memory_read_bytes + self.memory_write_bytes
    }

    /// Recompute arithmetic intensity from FLOP and memory values.
    pub fn compute_intensity(&mut self) {
        let total_bytes = self.total_memory_bytes();
        self.arithmetic_intensity = if total_bytes > 0 {
            self.flop_count as f64 / total_bytes as f64
        } else {
            f64::MAX
        };
    }

    /// Is this a compute-bound workload?
    pub fn is_compute_bound(&self) -> bool {
        self.arithmetic_intensity > 10.0
    }

    /// Is this a memory-bound workload?
    pub fn is_memory_bound(&self) -> bool {
        self.arithmetic_intensity < 1.0
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Cost Model
// ═══════════════════════════════════════════════════════════════════════════

/// Cost estimate for running a kernel on a specific device.
#[derive(Debug, Clone)]
pub struct DeviceCost {
    pub device: Device,
    pub device_name: String,
    /// Estimated execution time in microseconds.
    pub execution_time_us: f64,
    /// Data transfer time in microseconds.
    pub transfer_time_us: f64,
    /// Total cost (execution + transfer).
    pub total_time_us: f64,
    /// Reasoning for this cost estimate.
    pub reasoning: Vec<String>,
}

impl DeviceCost {
    pub fn new(device: Device, device_name: &str) -> Self {
        DeviceCost {
            device,
            device_name: device_name.to_string(),
            execution_time_us: 0.0,
            transfer_time_us: 0.0,
            total_time_us: 0.0,
            reasoning: Vec::new(),
        }
    }

    fn add_reason(&mut self, reason: &str) {
        self.reasoning.push(reason.to_string());
    }
}

impl fmt::Display for DeviceCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: exec={:.1}us, xfer={:.1}us, total={:.1}us",
            self.device, self.execution_time_us, self.transfer_time_us, self.total_time_us
        )
    }
}

/// Estimate cost of running a workload on a device.
pub fn estimate_cost(
    workload: &WorkloadProfile,
    device: &DeviceCapabilities,
) -> DeviceCost {
    let mut cost = DeviceCost::new(device.device, &device.name);

    // Compute time: FLOPs / peak_GFLOPS → seconds → microseconds
    let compute_us = if device.peak_gflops_f32 > 0.0 {
        let seconds = workload.flop_count as f64 / (device.peak_gflops_f32 * 1e9);
        seconds * 1_000_000.0
    } else {
        f64::MAX
    };

    // Memory time: bytes / bandwidth → seconds → microseconds
    let total_bytes = workload.total_memory_bytes() as f64;
    let memory_us = if device.memory_bandwidth_gbps > 0.0 {
        let seconds = total_bytes / (device.memory_bandwidth_gbps * 1e9);
        seconds * 1_000_000.0
    } else {
        0.0
    };

    // Execution time is max of compute and memory time (roofline model)
    cost.execution_time_us = compute_us.max(memory_us);

    // Transfer overhead
    cost.transfer_time_us = device.transfer_overhead_us;

    // Parallelism benefit: GPU/NPU benefit from high parallelism
    let parallelism_factor = match device.device {
        Device::Cpu => {
            // CPU benefits up to core count
            let useful = (workload.parallelism as f64).min(device.compute_units as f64);
            if useful > 1.0 { device.compute_units as f64 / useful } else { 1.0 }
        }
        Device::Gpu => {
            // GPU massively parallel — big benefit if workload is parallel
            if workload.is_parallel && workload.parallelism > 1000 {
                cost.add_reason("high parallelism suits GPU well");
                0.5 // additional speedup
            } else if !workload.is_parallel {
                cost.add_reason("sequential workload penalizes GPU");
                5.0 // penalty for non-parallel work
            } else {
                1.0
            }
        }
        Device::Npu => {
            if workload.uses_matrix_ops {
                cost.add_reason("matrix ops benefit from NPU tensor cores");
                0.3 // big benefit for matrix workloads
            } else {
                cost.add_reason("non-matrix workload suboptimal for NPU");
                10.0 // penalty for non-matrix work
            }
        }
    };

    cost.execution_time_us *= parallelism_factor;
    cost.total_time_us = cost.execution_time_us + cost.transfer_time_us;

    // Add roofline reasoning
    if workload.is_compute_bound() {
        cost.add_reason("compute-bound workload (high arithmetic intensity)");
    } else if workload.is_memory_bound() {
        cost.add_reason("memory-bound workload (low arithmetic intensity)");
    } else {
        cost.add_reason("balanced workload");
    }

    cost
}

// ═══════════════════════════════════════════════════════════════════════════
// Placement Decision
// ═══════════════════════════════════════════════════════════════════════════

/// A placement decision with reasoning.
#[derive(Debug, Clone)]
pub struct PlacementDecision {
    pub kernel_name: String,
    pub selected_device: Device,
    pub costs: Vec<DeviceCost>,
    pub speedup_vs_cpu: f64,
    pub reasoning: Vec<String>,
}

impl PlacementDecision {
    /// Get cost for the selected device.
    pub fn selected_cost(&self) -> Option<&DeviceCost> {
        self.costs.iter().find(|c| c.device == self.selected_device)
    }

    /// Get cost for a specific device.
    pub fn cost_for(&self, device: Device) -> Option<&DeviceCost> {
        self.costs.iter().find(|c| c.device == device)
    }

    /// Summary as human-readable string.
    pub fn summary(&self) -> String {
        let total = self.selected_cost().map_or(0.0, |c| c.total_time_us);
        format!(
            "Placed '{}' on {}: {:.1}us (speedup vs CPU: {:.2}x)",
            self.kernel_name, self.selected_device, total, self.speedup_vs_cpu
        )
    }

    /// Detailed reasoning as a structured report (agent-queryable via RAP).
    pub fn detailed_report(&self) -> BTreeMap<String, String> {
        let mut report = BTreeMap::new();
        report.insert("kernel".to_string(), self.kernel_name.clone());
        report.insert("selected_device".to_string(), self.selected_device.to_string());
        report.insert("speedup_vs_cpu".to_string(), format!("{:.2}x", self.speedup_vs_cpu));

        for cost in &self.costs {
            let key = format!("cost_{}", cost.device);
            report.insert(key, format!("{:.1}us", cost.total_time_us));
        }

        for (i, reason) in self.reasoning.iter().enumerate() {
            report.insert(format!("reason_{i}"), reason.clone());
        }

        report
    }
}

impl fmt::Display for PlacementDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Placement Engine
// ═══════════════════════════════════════════════════════════════════════════

/// Run automatic device placement for a kernel.
pub fn place_kernel(
    workload: &WorkloadProfile,
    registry: &DeviceRegistry,
) -> PlacementDecision {
    let mut costs: Vec<DeviceCost> = registry
        .iter()
        .map(|dev| estimate_cost(workload, dev))
        .collect();

    // Sort by total time
    costs.sort_by(|a, b| a.total_time_us.partial_cmp(&b.total_time_us).unwrap_or(std::cmp::Ordering::Equal));

    let selected = costs.first().map(|c| c.device).unwrap_or(Device::Cpu);

    // Compute speedup vs CPU
    let cpu_cost = costs.iter().find(|c| c.device == Device::Cpu).map(|c| c.total_time_us).unwrap_or(1.0);
    let selected_cost = costs.iter().find(|c| c.device == selected).map(|c| c.total_time_us).unwrap_or(1.0);
    let speedup = if selected_cost > 0.0 { cpu_cost / selected_cost } else { 1.0 };

    // Build reasoning
    let mut reasoning = Vec::new();
    reasoning.push(format!("evaluated {} devices", costs.len()));
    reasoning.push(format!("selected {} with estimated {:.1}us", selected, selected_cost));
    if speedup > 1.0 && selected != Device::Cpu {
        reasoning.push(format!("{:.2}x faster than CPU", speedup));
    }

    // Collect device-specific reasoning
    for cost in &costs {
        for reason in &cost.reasoning {
            reasoning.push(format!("{}: {reason}", cost.device));
        }
    }

    PlacementDecision {
        kernel_name: workload.kernel_name.clone(),
        selected_device: selected,
        costs,
        speedup_vs_cpu: speedup,
        reasoning,
    }
}

/// Place a kernel with an explicit annotation.
pub fn place_with_annotation(
    workload: &WorkloadProfile,
    annotation: &PlacementTarget,
    registry: &DeviceRegistry,
) -> PlacementDecision {
    match annotation {
        PlacementTarget::Auto => place_kernel(workload, registry),
        PlacementTarget::Explicit(device) => {
            // Explicit placement — still evaluate costs for reporting
            let costs: Vec<DeviceCost> = registry
                .iter()
                .map(|dev| estimate_cost(workload, dev))
                .collect();

            let cpu_cost = costs.iter().find(|c| c.device == Device::Cpu).map(|c| c.total_time_us).unwrap_or(1.0);
            let selected_cost = costs.iter().find(|c| c.device == *device).map(|c| c.total_time_us).unwrap_or(1.0);
            let speedup = if selected_cost > 0.0 { cpu_cost / selected_cost } else { 1.0 };

            let mut reasoning = vec![
                format!("explicit placement on {device}"),
            ];

            // Warn if not optimal
            let optimal = costs.iter().min_by(|a, b| a.total_time_us.partial_cmp(&b.total_time_us).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(opt) = optimal {
                if opt.device != *device {
                    reasoning.push(format!(
                        "note: {} would be faster ({:.1}us vs {:.1}us)",
                        opt.device, opt.total_time_us, selected_cost
                    ));
                }
            }

            PlacementDecision {
                kernel_name: workload.kernel_name.clone(),
                selected_device: *device,
                costs,
                speedup_vs_cpu: speedup,
                reasoning,
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Annotation Parsing
// ═══════════════════════════════════════════════════════════════════════════

/// Parse `@pt(...)` annotation.
pub fn parse_pt_annotation(input: &str) -> Option<PlacementTarget> {
    let trimmed = input.trim();
    if !trimmed.starts_with("@pt(") || !trimmed.ends_with(')') {
        return None;
    }
    let inner = trimmed[4..trimmed.len() - 1].trim();
    match inner {
        "auto" => Some(PlacementTarget::Auto),
        "cpu" => Some(PlacementTarget::Explicit(Device::Cpu)),
        "gpu" => Some(PlacementTarget::Explicit(Device::Gpu)),
        "npu" => Some(PlacementTarget::Explicit(Device::Npu)),
        _ => None,
    }
}

/// Parse `#[perf::target(...)]` attribute.
pub fn parse_target_attr(input: &str) -> Option<PlacementTarget> {
    let trimmed = input.trim();
    let prefix = "#[perf::target(";
    if !trimmed.starts_with(prefix) || !trimmed.ends_with(")]") {
        return None;
    }
    let inner = trimmed[prefix.len()..trimmed.len() - 2].trim();
    match inner {
        "auto" => Some(PlacementTarget::Auto),
        "cpu" => Some(PlacementTarget::Explicit(Device::Cpu)),
        "gpu" => Some(PlacementTarget::Explicit(Device::Gpu)),
        "npu" => Some(PlacementTarget::Explicit(Device::Npu)),
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// RAP Query Interface
// ═══════════════════════════════════════════════════════════════════════════

/// A RAP query for placement decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementQuery {
    pub kernel_name: String,
    pub query_type: PlacementQueryType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementQueryType {
    /// Why was this device selected?
    WhySelected,
    /// What are all device costs?
    AllCosts,
    /// Would another device be better?
    AlternativeRecommendation,
    /// Full detailed report.
    DetailedReport,
}

/// Process a RAP query against a placement decision.
pub fn process_rap_query(
    decision: &PlacementDecision,
    query: &PlacementQuery,
) -> BTreeMap<String, String> {
    let mut response = BTreeMap::new();
    response.insert("kernel".to_string(), decision.kernel_name.clone());

    match &query.query_type {
        PlacementQueryType::WhySelected => {
            response.insert("selected_device".to_string(), decision.selected_device.to_string());
            for (i, reason) in decision.reasoning.iter().enumerate() {
                response.insert(format!("reason_{i}"), reason.clone());
            }
        }
        PlacementQueryType::AllCosts => {
            for cost in &decision.costs {
                response.insert(
                    format!("{}_cost_us", cost.device),
                    format!("{:.1}", cost.total_time_us),
                );
            }
        }
        PlacementQueryType::AlternativeRecommendation => {
            let best = decision.costs.iter()
                .min_by(|a, b| a.total_time_us.partial_cmp(&b.total_time_us).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(best) = best {
                response.insert("recommended_device".to_string(), best.device.to_string());
                response.insert("recommended_cost_us".to_string(), format!("{:.1}", best.total_time_us));
                if best.device != decision.selected_device {
                    response.insert("suggestion".to_string(), format!(
                        "Change @pt({}) to @pt(auto) — {} is {:.2}x faster",
                        decision.selected_device,
                        best.device,
                        decision.selected_cost().map_or(1.0, |c| c.total_time_us) / best.total_time_us.max(0.001),
                    ));
                } else {
                    response.insert("suggestion".to_string(), "current placement is optimal".to_string());
                }
            }
        }
        PlacementQueryType::DetailedReport => {
            return decision.detailed_report();
        }
    }

    response
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn compute_heavy_workload() -> WorkloadProfile {
        let mut w = WorkloadProfile::new("matmul");
        w.flop_count = 1_000_000_000; // 1 GFLOP
        w.memory_read_bytes = 10_000_000; // 10 MB
        w.memory_write_bytes = 5_000_000; // 5 MB
        w.is_parallel = true;
        w.uses_matrix_ops = true;
        w.parallelism = 100_000;
        w.loop_depth = 3;
        w.compute_intensity();
        w
    }

    fn memory_heavy_workload() -> WorkloadProfile {
        let mut w = WorkloadProfile::new("memcopy");
        w.flop_count = 1_000; // tiny compute
        w.memory_read_bytes = 1_000_000_000; // 1 GB
        w.memory_write_bytes = 1_000_000_000; // 1 GB
        w.is_parallel = true;
        w.parallelism = 10_000;
        w.loop_depth = 1;
        w.compute_intensity();
        w
    }

    fn tiny_sequential_workload() -> WorkloadProfile {
        let mut w = WorkloadProfile::new("scalar_add");
        w.flop_count = 100;
        w.memory_read_bytes = 800;
        w.memory_write_bytes = 400;
        w.is_parallel = false;
        w.parallelism = 1;
        w.loop_depth = 0;
        w.compute_intensity();
        w
    }

    fn matrix_workload() -> WorkloadProfile {
        let mut w = WorkloadProfile::new("transformer_attention");
        w.flop_count = 10_000_000_000; // 10 GFLOP
        w.memory_read_bytes = 100_000_000; // 100 MB
        w.memory_write_bytes = 50_000_000; // 50 MB
        w.is_parallel = true;
        w.uses_matrix_ops = true;
        w.parallelism = 1_000_000;
        w.loop_depth = 4;
        w.compute_intensity();
        w
    }

    // ── Device ───────────────────────────────────────────────────────────

    #[test]
    fn device_display() {
        assert_eq!(Device::Cpu.to_string(), "CPU");
        assert_eq!(Device::Gpu.to_string(), "GPU");
        assert_eq!(Device::Npu.to_string(), "NPU");
    }

    #[test]
    fn placement_target_display() {
        assert_eq!(PlacementTarget::Auto.to_string(), "@pt(auto)");
        assert_eq!(PlacementTarget::Explicit(Device::Gpu).to_string(), "@pt(GPU)");
    }

    // ── Device Registry ──────────────────────────────────────────────────

    #[test]
    fn registry_default() {
        let reg = DeviceRegistry::default_system();
        assert_eq!(reg.available_devices().len(), 3);
        assert!(reg.get(Device::Cpu).is_some());
        assert!(reg.get(Device::Gpu).is_some());
        assert!(reg.get(Device::Npu).is_some());
    }

    #[test]
    fn registry_cpu_only() {
        let reg = DeviceRegistry::cpu_only();
        assert_eq!(reg.available_devices().len(), 1);
        assert!(reg.get(Device::Cpu).is_some());
        assert!(reg.get(Device::Gpu).is_none());
    }

    #[test]
    fn registry_unavailable_device() {
        let mut reg = DeviceRegistry::new();
        reg.add(DeviceCapabilities::generic_cpu());
        reg.add(DeviceCapabilities::unavailable(Device::Gpu));
        assert!(reg.get(Device::Gpu).is_none());
        assert_eq!(reg.available_devices().len(), 1);
    }

    // ── Workload Profile ─────────────────────────────────────────────────

    #[test]
    fn workload_intensity() {
        let w = compute_heavy_workload();
        assert!(w.is_compute_bound());
        assert!(!w.is_memory_bound());
    }

    #[test]
    fn workload_memory_bound() {
        let w = memory_heavy_workload();
        assert!(w.is_memory_bound());
        assert!(!w.is_compute_bound());
    }

    #[test]
    fn workload_total_bytes() {
        let w = compute_heavy_workload();
        assert_eq!(w.total_memory_bytes(), 15_000_000);
    }

    // ── Cost Estimation ──────────────────────────────────────────────────

    #[test]
    fn cpu_cost_estimate() {
        let w = compute_heavy_workload();
        let dev = DeviceCapabilities::generic_cpu();
        let cost = estimate_cost(&w, &dev);
        assert!(cost.execution_time_us > 0.0);
        assert_eq!(cost.transfer_time_us, 0.0); // CPU has no transfer
        assert!(cost.total_time_us > 0.0);
    }

    #[test]
    fn gpu_has_transfer_overhead() {
        let w = compute_heavy_workload();
        let dev = DeviceCapabilities::generic_gpu();
        let cost = estimate_cost(&w, &dev);
        assert!(cost.transfer_time_us > 0.0);
    }

    #[test]
    fn gpu_faster_for_parallel_compute() {
        let w = compute_heavy_workload();
        let cpu_cost = estimate_cost(&w, &DeviceCapabilities::generic_cpu());
        let gpu_cost = estimate_cost(&w, &DeviceCapabilities::generic_gpu());
        assert!(gpu_cost.total_time_us < cpu_cost.total_time_us);
    }

    #[test]
    fn cpu_faster_for_sequential() {
        let w = tiny_sequential_workload();
        let cpu_cost = estimate_cost(&w, &DeviceCapabilities::generic_cpu());
        let gpu_cost = estimate_cost(&w, &DeviceCapabilities::generic_gpu());
        // CPU wins for tiny sequential work (no transfer overhead)
        assert!(cpu_cost.total_time_us < gpu_cost.total_time_us);
    }

    // ── Placement Decisions ──────────────────────────────────────────────

    #[test]
    fn place_compute_heavy_selects_gpu_or_npu() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        // GPU or NPU should be selected for compute-heavy parallel workloads
        assert!(decision.selected_device == Device::Gpu || decision.selected_device == Device::Npu);
    }

    #[test]
    fn place_sequential_selects_cpu() {
        let w = tiny_sequential_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        assert_eq!(decision.selected_device, Device::Cpu);
    }

    #[test]
    fn place_matrix_prefers_npu() {
        let w = matrix_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        // NPU should be preferred for matrix-heavy workloads
        assert_eq!(decision.selected_device, Device::Npu);
    }

    #[test]
    fn place_cpu_only_falls_back() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::cpu_only();
        let decision = place_kernel(&w, &reg);
        assert_eq!(decision.selected_device, Device::Cpu);
    }

    #[test]
    fn placement_speedup() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        if decision.selected_device != Device::Cpu {
            assert!(decision.speedup_vs_cpu > 1.0);
        }
    }

    #[test]
    fn placement_summary() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        let summary = decision.summary();
        assert!(summary.contains("matmul"));
        assert!(summary.contains("speedup"));
    }

    #[test]
    fn placement_detailed_report() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        let report = decision.detailed_report();
        assert!(report.contains_key("kernel"));
        assert!(report.contains_key("selected_device"));
        assert!(report.contains_key("speedup_vs_cpu"));
    }

    // ── Explicit Placement ───────────────────────────────────────────────

    #[test]
    fn explicit_placement_honors_target() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_with_annotation(&w, &PlacementTarget::Explicit(Device::Cpu), &reg);
        assert_eq!(decision.selected_device, Device::Cpu);
    }

    #[test]
    fn explicit_placement_warns_suboptimal() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_with_annotation(&w, &PlacementTarget::Explicit(Device::Cpu), &reg);
        // Should warn that another device is faster
        let has_note = decision.reasoning.iter().any(|r| r.contains("would be faster"));
        assert!(has_note);
    }

    #[test]
    fn auto_placement_same_as_place_kernel() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let auto = place_with_annotation(&w, &PlacementTarget::Auto, &reg);
        let direct = place_kernel(&w, &reg);
        assert_eq!(auto.selected_device, direct.selected_device);
    }

    // ── Annotation Parsing ───────────────────────────────────────────────

    #[test]
    fn parse_pt_auto() {
        assert_eq!(parse_pt_annotation("@pt(auto)"), Some(PlacementTarget::Auto));
    }

    #[test]
    fn parse_pt_cpu() {
        assert_eq!(parse_pt_annotation("@pt(cpu)"), Some(PlacementTarget::Explicit(Device::Cpu)));
    }

    #[test]
    fn parse_pt_gpu() {
        assert_eq!(parse_pt_annotation("@pt(gpu)"), Some(PlacementTarget::Explicit(Device::Gpu)));
    }

    #[test]
    fn parse_pt_npu() {
        assert_eq!(parse_pt_annotation("@pt(npu)"), Some(PlacementTarget::Explicit(Device::Npu)));
    }

    #[test]
    fn parse_pt_invalid() {
        assert!(parse_pt_annotation("@pt(fpga)").is_none());
        assert!(parse_pt_annotation("@pa(4)").is_none());
        assert!(parse_pt_annotation("hello").is_none());
    }

    #[test]
    fn parse_attr_auto() {
        assert_eq!(
            parse_target_attr("#[perf::target(auto)]"),
            Some(PlacementTarget::Auto)
        );
    }

    #[test]
    fn parse_attr_gpu() {
        assert_eq!(
            parse_target_attr("#[perf::target(gpu)]"),
            Some(PlacementTarget::Explicit(Device::Gpu))
        );
    }

    #[test]
    fn parse_attr_invalid() {
        assert!(parse_target_attr("#[perf::target(fpga)]").is_none());
        assert!(parse_target_attr("not an attr").is_none());
    }

    // ── RAP Queries ──────────────────────────────────────────────────────

    #[test]
    fn rap_query_why_selected() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        let query = PlacementQuery {
            kernel_name: "matmul".to_string(),
            query_type: PlacementQueryType::WhySelected,
        };
        let response = process_rap_query(&decision, &query);
        assert!(response.contains_key("selected_device"));
        assert!(response.contains_key("reason_0"));
    }

    #[test]
    fn rap_query_all_costs() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        let query = PlacementQuery {
            kernel_name: "matmul".to_string(),
            query_type: PlacementQueryType::AllCosts,
        };
        let response = process_rap_query(&decision, &query);
        assert!(response.contains_key("CPU_cost_us"));
        assert!(response.contains_key("GPU_cost_us"));
    }

    #[test]
    fn rap_query_alternative() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_with_annotation(&w, &PlacementTarget::Explicit(Device::Cpu), &reg);
        let query = PlacementQuery {
            kernel_name: "matmul".to_string(),
            query_type: PlacementQueryType::AlternativeRecommendation,
        };
        let response = process_rap_query(&decision, &query);
        assert!(response.contains_key("recommended_device"));
        assert!(response.contains_key("suggestion"));
    }

    #[test]
    fn rap_query_detailed_report() {
        let w = compute_heavy_workload();
        let reg = DeviceRegistry::default_system();
        let decision = place_kernel(&w, &reg);
        let query = PlacementQuery {
            kernel_name: "matmul".to_string(),
            query_type: PlacementQueryType::DetailedReport,
        };
        let response = process_rap_query(&decision, &query);
        assert!(response.contains_key("kernel"));
    }

    // ── Cost Display ─────────────────────────────────────────────────────

    #[test]
    fn cost_display() {
        let w = compute_heavy_workload();
        let dev = DeviceCapabilities::generic_cpu();
        let cost = estimate_cost(&w, &dev);
        let s = cost.to_string();
        assert!(s.contains("CPU"));
        assert!(s.contains("us"));
    }
}
