//! Hardware accelerator backends — extensible registry.
//!
//! Originally a hardcoded match (P91); now a runtime registry that
//! merges built-in entries with user-supplied JSON descriptors. An
//! agent or operator can advertise a new accelerator without
//! recompiling MechGen by either:
//!
//! 1. Setting `RDX_BACKENDS_PATH=/path/to/backends.json`, OR
//! 2. Dropping a file at `~/.mechgen/backends.json`, OR
//! 3. Passing `--backends-file <path>` to `MechGen-parse`.
//!
//! Descriptor schema (one JSON object per backend, list of objects):
//!
//! ```json
//! [
//!   {
//!     "name": "rocm",
//!     "family": "gpu",
//!     "vendor": "AMD",
//!     "requires": "feature:rocm + ROCm 6.0+",
//!     "summary": "AMD GPU via ROCm. CDNA + RDNA support.",
//!     "available_at_runtime": false
//!   }
//! ]
//! ```
//!
//! Dispatch is still limited to backends the prototype build links
//! against (today: just CPU). Registry entries with
//! `available_at_runtime: true` that AREN'T in the dispatcher's
//! match arm return a helpful error explaining the gap.

use rmi::compute::cpu::CpuBackend;
#[cfg(feature = "cuda")]
use crate::cuda_backend::CudaBackend;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

/// External reference: IronAccelerator (separate Rust workspace at
/// `utilities/IronAccelerator/`) is a production-quality hardware-
/// agnostic driver substrate covering NVIDIA / AMD / Apple / Qualcomm
/// / Intel / Google / AWS plus open APIs (Vulkan / OpenGL / WebGPU),
/// with its own `ironaccelerator-ontology` crate enumerating ~38
/// per-model HardwareNode entries, ~32 WorkloadClass entries, ~38
/// StrategyClass entries, ~24 Optimization entries.
///
/// MechGen's `hardware_accelerators` section deliberately stays at
/// the **backend-family** level (cpu / cuda / metal / ...) - that's
/// what an agent needs to know to write `+f` / `net{}` source. For
/// model-specific guidance ("does my MoE block fit on a Hopper SM90
/// FP8 path?") the IronAccelerator ontology is the right tool;
/// reach for it from `ontology.docs.IronAccelerator`.
pub const IRONACCELERATOR_REFERENCE: &str =
    "utilities/IronAccelerator/ - production HW-agnostic driver substrate \
     + agent-queryable ontology (per-model). Surfaced as a docs pointer, \
     not inlined here, so MechGen's ontology stays at the actionable level.";

/// How a registered backend executes Agentic Binary Language bytecode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DispatchKind {
    /// Built-in compiled backend. Today: only CPU.
    Builtin,
    /// Spawn an external process per dispatch. stdin = Agentic Binary Language blob,
    /// env carries metadata (`RDX_BACKEND`, `RDX_ITEM_NAME`,
    /// `RDX_INPUT_SHAPE`), stdout = JSON result `{ ok, output_shape,
    /// output_sum, dispatched }`. Mirrors the P47 refine wrapper
    /// protocol. Lets ANY accelerator with a CLI tool become a
    /// dispatch target without recompiling MechGen.
    Subprocess { command: String },
}

impl Default for DispatchKind {
    fn default() -> Self {
        Self::Builtin
    }
}

/// One backend descriptor. Field shape is the public schema for the
/// JSON registry file - changing it is a breaking change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendDescriptor {
    pub name: String,
    pub family: String,
    pub vendor: String,
    pub requires: String,
    pub summary: String,
    /// True if the prototype build can actually construct this
    /// backend. Set by registration logic, not user-overridable.
    #[serde(default)]
    pub available_at_runtime: bool,
    /// Free-form tags an agent can filter on (e.g. "edge", "low-power").
    #[serde(default)]
    pub tags: Vec<String>,
    /// Where this descriptor came from: "builtin" / "env:RDX_BACKENDS_PATH"
    /// / "home:~/.mechgen/backends.json" / "cli:--backends-file".
    #[serde(default)]
    pub source: String,
    /// How to execute on this backend. Defaults to `Builtin`; a
    /// registered descriptor can override to `Subprocess` for any
    /// accelerator with a CLI wrapper. Operator's responsibility to
    /// ensure the wrapper actually understands Agentic Binary Language.
    #[serde(default)]
    pub dispatch: DispatchKind,
}

impl BackendDescriptor {
    fn builtin(
        name: &str, family: &str, vendor: &str, requires: &str, summary: &str,
        available: bool,
    ) -> Self {
        Self {
            name: name.into(),
            family: family.into(),
            vendor: vendor.into(),
            requires: requires.into(),
            summary: summary.into(),
            available_at_runtime: available,
            tags: Vec::new(),
            source: "builtin".into(),
            dispatch: DispatchKind::Builtin,
        }
    }
}

/// The seven RecursiveMachineIntelligence-defined backends + BLAS. Loaded at registry
/// init and merged with anything in the JSON registry file.
pub fn builtin_backends() -> Vec<BackendDescriptor> {
    vec![
        BackendDescriptor::builtin(
            "cpu", "cpu", "any", "always",
            "Pure-Rust CPU dispatch. Always available. Default backend.",
            true,
        ),
        BackendDescriptor::builtin(
            "cuda", "gpu", "NVIDIA",
            "build --features cuda. Uses IronAccelerator's CUDA 13.2 \
             libloading path - the build does NOT require CUDA_PATH \
             set, so cargo build --features cuda succeeds on dev boxes \
             without the SDK. At dispatch time `--backend=cuda` calls \
             into the NVIDIA driver via dlopen; install the driver \
             before runtime use.",
            "NVIDIA GPU via IronAccelerator's cudarc-compat layer \
             (CUDA 13.2 ABI). Device acquisition + cuBLASLt + NVRTC. \
             Production-quality wrapper; faster than cudarc 0.19 on \
             host-side hot paths per IA bench.",
            cfg!(feature = "cuda"),
        ),
        BackendDescriptor::builtin(
            "metal", "gpu", "Apple", "feature:metal + macOS",
            "Apple Metal GPU (M-series and Intel Macs with discrete GPU).",
            false,
        ),
        BackendDescriptor::builtin(
            "apple_ane", "npu", "Apple", "feature:apple_ane + macOS 13+",
            "Apple Neural Engine. Low-power inference on M1/M2/M3 SoC.",
            false,
        ),
        BackendDescriptor::builtin(
            "vulkan", "gpu", "any (AMD/Intel/NVIDIA)",
            "feature:vulkan + Vulkan loader",
            "Cross-vendor GPU via Vulkan. Linux + Windows support.",
            false,
        ),
        BackendDescriptor::builtin(
            "webgpu", "gpu", "any", "feature:webgpu",
            "wgpu-based cross-platform GPU. Same backend works in browser.",
            false,
        ),
        BackendDescriptor::builtin(
            "qualcomm", "npu", "Qualcomm", "feature:qualcomm + Hexagon SDK",
            "Qualcomm Hexagon DSP/NPU. Mobile / edge inference.",
            false,
        ),
        BackendDescriptor::builtin(
            "blas", "cpu", "any",
            "feature:blas + BLAS library (OpenBLAS/MKL/Accelerate)",
            "CPU accelerated via vendor BLAS. Faster matmul than pure Rust.",
            false,
        ),
    ]
}

/// Process-wide backend registry. Lazy-initialised from builtins +
/// JSON sources on first access. Mutex so `register_descriptor` can
/// mutate from CLI flag handling.
static REGISTRY: OnceLock<Mutex<Vec<BackendDescriptor>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<BackendDescriptor>> {
    REGISTRY.get_or_init(|| {
        let mut all = builtin_backends();
        // Env var takes precedence over home dir.
        if let Ok(path) = std::env::var("RDX_BACKENDS_PATH") {
            load_into(&mut all, &path, "env:RDX_BACKENDS_PATH");
        }
        if let Some(home_path) = home_backends_path() {
            load_into(&mut all, &home_path.to_string_lossy(), "home:~/.mechgen/backends.json");
        }
        Mutex::new(all)
    })
}

fn home_backends_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    let p = std::path::PathBuf::from(home).join(".mechgen").join("backends.json");
    if p.exists() { Some(p) } else { None }
}

fn load_into(dst: &mut Vec<BackendDescriptor>, path: &str, source_tag: &str) {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let extras: Vec<BackendDescriptor> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("backends: failed to parse {path}: {e}");
            return;
        }
    };
    for mut d in extras {
        if d.source.is_empty() {
            d.source = source_tag.to_string();
        }
        merge(dst, d);
    }
}

/// Merge a descriptor into the registry. If a descriptor with the
/// same name already exists, the new one overrides it (this lets
/// users override built-in entries' metadata / availability via the
/// JSON file).
fn merge(dst: &mut Vec<BackendDescriptor>, d: BackendDescriptor) {
    if let Some(idx) = dst.iter().position(|x| x.name == d.name) {
        dst[idx] = d;
    } else {
        dst.push(d);
    }
}

/// Register a descriptor at runtime. Called from `--backends-file
/// <path>` handling and useable by tests.
pub fn register_descriptors_from_file(path: &str) -> Result<usize, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let extras: Vec<BackendDescriptor> =
        serde_json::from_str(&raw).map_err(|e| format!("parse {path}: {e}"))?;
    let mut guard = registry().lock().unwrap();
    let n = extras.len();
    for mut d in extras {
        if d.source.is_empty() {
            d.source = format!("cli:{path}");
        }
        merge(&mut guard, d);
    }
    Ok(n)
}

/// Snapshot the registry. Used by the ontology section + tests.
pub fn all_descriptors() -> Vec<BackendDescriptor> {
    registry().lock().unwrap().clone()
}

/// Names of backends marked `available_at_runtime: true`.
pub fn available_backends() -> Vec<String> {
    all_descriptors()
        .into_iter()
        .filter(|d| d.available_at_runtime)
        .map(|d| d.name)
        .collect()
}

/// Dispatched-against-backend shim. `Cpu` is the only compiled-in
/// variant; `Subprocess` is the extensible escape hatch (P94) - a
/// registered descriptor with `dispatch.kind=subprocess` selects it.
pub enum SelectedBackend {
    Cpu(CpuBackend),
    /// Real NVIDIA GPU dispatch via RecursiveMachineIntelligence's cudarc/cuBLAS/NVRTC
    /// backend (`cuda_full.rs`). Compiled in only when the prototype
    /// is built with `--features cuda` and the CUDA Toolkit + NVIDIA
    /// driver are present at link time.
    #[cfg(feature = "cuda")]
    Cuda(CudaBackend),
    Subprocess {
        name: String,
        command: String,
    },
}

impl std::fmt::Debug for SelectedBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelectedBackend").field("name", &self.name()).finish()
    }
}

impl SelectedBackend {
    pub fn name(&self) -> &str {
        match self {
            Self::Cpu(_) => "cpu",
            #[cfg(feature = "cuda")]
            Self::Cuda(_) => "cuda",
            Self::Subprocess { name, .. } => name,
        }
    }
    pub fn as_cpu(&self) -> Option<&CpuBackend> {
        match self {
            Self::Cpu(b) => Some(b),
            #[cfg(feature = "cuda")]
            Self::Cuda(_) => None,
            Self::Subprocess { .. } => None,
        }
    }
    #[cfg(feature = "cuda")]
    pub fn as_cuda(&self) -> Option<&CudaBackend> {
        match self {
            Self::Cuda(b) => Some(b),
            _ => None,
        }
    }
    /// Returns the subprocess command if this backend dispatches via
    /// an external process, else None.
    pub fn subprocess_command(&self) -> Option<&str> {
        match self {
            Self::Subprocess { command, .. } => Some(command),
            _ => None,
        }
    }
}

/// JSON result the subprocess wrapper is expected to print on stdout.
/// Fields mirror what `abl/run` returns over RAP so an agent gets
/// the same shape regardless of which backend ran the work.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubprocessResult {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub dispatched: usize,
    #[serde(default)]
    pub output_shape: Vec<usize>,
    #[serde(default)]
    pub output_sum: f64,
    #[serde(default)]
    pub error: Option<String>,
}

/// Run an Agentic Binary Language blob through a subprocess backend. The wrapper gets
/// the blob on stdin, metadata in env vars, and is expected to print
/// a `SubprocessResult` JSON on stdout. Returns the parsed result
/// or an error string. Mirrors the agent wrapper protocol semantics
/// from P47 (refine).
pub fn dispatch_via_subprocess(
    backend_name: &str,
    command: &str,
    item_name: &str,
    input_shape: &[usize],
    abl_blob: &[u8],
) -> Result<SubprocessResult, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut parts = command.split_whitespace();
    let prog = parts.next().ok_or_else(|| "empty backend command".to_string())?;
    let args: Vec<&str> = parts.collect();

    let shape_str = input_shape
        .iter()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut child = Command::new(prog)
        .args(&args)
        .env("RDX_BACKEND", backend_name)
        .env("RDX_ITEM_NAME", item_name)
        .env("RDX_INPUT_SHAPE", &shape_str)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {prog}: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(abl_blob)
            .map_err(|e| format!("write Agentic Binary Language to backend stdin: {e}"))?;
    }
    let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "backend exit {}: {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .map_err(|e| format!("parse backend stdout as SubprocessResult: {e}; raw='{stdout}'"))
}

/// Pick a backend by name. Checks the registry to give a precise
/// error: unknown name vs. registered-but-not-buildable. Returns
/// `SelectedBackend::Subprocess` for descriptors that ship a
/// `dispatch.kind=subprocess` field - the dispatcher then routes
/// through the external wrapper.
pub fn select_backend(name: &str) -> Result<SelectedBackend, String> {
    if name == "cpu" {
        return Ok(SelectedBackend::Cpu(CpuBackend::new()));
    }
    // Real CUDA dispatch. Compiled in only with `--features cuda`.
    // CudaBackend::new() queries device 0; surfaces driver / SDK
    // errors directly so the agent sees why GPU init failed.
    // Real CUDA 13.2 dispatch via IronAccelerator (P99). Compiled in
    // only with `--features cuda`. CudaBackend::new() uses IA's
    // CudaDevice (libloading) so build never needs CUDA_PATH; driver
    // presence is checked here. Surfaces driver errors directly.
    #[cfg(feature = "cuda")]
    if name == "cuda" {
        return CudaBackend::new()
            .map(SelectedBackend::Cuda)
            .map_err(|e| format!("CUDA init failed: {e}"));
    }
    let registry = all_descriptors();
    let found = registry.iter().find(|d| d.name == name);
    match found {
        // Subprocess-dispatchable: build a SelectedBackend even if
        // `available_at_runtime` is false - the wrapper IS the
        // runtime, and presence in the registry is the operator's
        // promise that it works.
        Some(d) => {
            if let DispatchKind::Subprocess { command } = &d.dispatch {
                return Ok(SelectedBackend::Subprocess {
                    name: d.name.clone(),
                    command: command.clone(),
                });
            }
            if d.available_at_runtime {
                // Registry says built-in available, but dispatcher
                // doesn't know it. Build is ahead of registry.
                Err(format!(
                    "backend {name} is marked available in the registry but \
                     the prototype build has no Builtin constructor for it. \
                     Either add a match arm to backends::select_backend or \
                     set dispatch.kind=subprocess in the descriptor."
                ))
            } else {
                Err(format!(
                    "backend {name} is registered (source={}, requires={}) but \
                     the prototype build doesn't enable it. Either compile with \
                     the right feature flag OR set dispatch.kind=subprocess in \
                     the descriptor to route via an external wrapper.",
                    d.source, d.requires
                ))
            }
        }
        None => Err(format!(
            "unknown backend {name:?}. Call ontology/section \
             {{\"section\":\"hardware_accelerators\"}} for the registered \
             catalog ({} entries).",
            registry.len()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_cpu_works() {
        let b = select_backend("cpu").expect("cpu always available");
        assert_eq!(b.name(), "cpu");
    }

    #[test]
    fn select_unknown_errors_with_registry_count() {
        let e = select_backend("nonexistent_backend_xyz").unwrap_err();
        assert!(e.contains("unknown backend"));
        assert!(e.contains("registered catalog"));
    }

    /// Default-build (no --features cuda): cuda is registered but
    /// not buildable, so select_backend returns an explanatory error.
    /// With --features cuda: IA's CudaDevice::new succeeds at link
    /// time (libloading defers the actual driver call), so cuda IS
    /// selectable - that case is covered by the build itself; this
    /// test only runs on the default path.
    #[cfg(not(feature = "cuda"))]
    #[test]
    fn select_registered_but_unavailable_explains_source() {
        let e = select_backend("cuda").unwrap_err();
        assert!(e.contains("cuda"));
        assert!(e.contains("registered"));
        assert!(e.contains("source=builtin"));
    }

    /// With --features cuda: cuda selection must succeed (IA's
    /// CudaDevice::new uses libloading, returns Ok even when the
    /// driver isn't actually present at runtime; real calls fail
    /// later if no GPU exists).
    #[cfg(feature = "cuda")]
    #[test]
    fn select_cuda_under_feature_returns_backend() {
        let b = select_backend("cuda").expect("cuda selectable under --features cuda");
        assert_eq!(b.name(), "cuda");
    }

    #[test]
    fn registry_contains_eight_builtins_at_minimum() {
        let names: Vec<String> = all_descriptors().into_iter().map(|d| d.name).collect();
        for required in ["cpu", "cuda", "metal", "apple_ane", "vulkan",
                         "webgpu", "qualcomm", "blas"] {
            assert!(names.contains(&required.to_string()),
                "missing builtin backend: {required}");
        }
    }

    #[test]
    fn cpu_is_available_at_runtime() {
        assert!(available_backends().contains(&"cpu".to_string()));
    }

    #[test]
    fn descriptor_round_trips_through_json() {
        let d = BackendDescriptor::builtin(
            "test_backend", "gpu", "TestCorp", "feature:test",
            "Test backend", false,
        );
        let json = serde_json::to_string(&d).unwrap();
        let parsed: BackendDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test_backend");
        assert_eq!(parsed.family, "gpu");
        assert_eq!(parsed.vendor, "TestCorp");
    }

    #[test]
    fn subprocess_descriptor_selects_to_subprocess_variant() {
        let tmp = std::env::temp_dir().join(format!(
            "mechgen_subproc_test_{}.json",
            std::process::id()
        ));
        // Use a uniquely-named backend so we don't collide with builtins.
        let json = format!(r#"[
            {{
                "name": "p94_subproc_demo",
                "family": "asic",
                "vendor": "TestVendor",
                "requires": "wrapper script",
                "summary": "Subprocess backend demo",
                "available_at_runtime": false,
                "dispatch": {{ "kind": "subprocess", "command": "echo demo" }}
            }}
        ]"#);
        std::fs::write(&tmp, json).unwrap();
        register_descriptors_from_file(tmp.to_str().unwrap()).expect("register");
        let selected = select_backend("p94_subproc_demo").expect("subprocess select");
        assert_eq!(selected.name(), "p94_subproc_demo");
        assert_eq!(selected.subprocess_command(), Some("echo demo"));
        assert!(selected.as_cpu().is_none());
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn subprocess_result_round_trips_through_json() {
        let r = SubprocessResult {
            ok: true,
            dispatched: 5,
            output_shape: vec![1, 1000],
            output_sum: 0.42,
            error: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: SubprocessResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.ok);
        assert_eq!(parsed.dispatched, 5);
        assert_eq!(parsed.output_shape, vec![1, 1000]);
    }

    #[test]
    fn register_from_file_adds_new_entry() {
        let tmp = std::env::temp_dir().join(format!("mechgen_backends_test_{}.json",
            std::process::id()));
        let json = r#"[
            {
                "name": "p93_test_arbitrary",
                "family": "exotic",
                "vendor": "TestVendor",
                "requires": "nothing - test only",
                "summary": "Verifies arbitrary backends can register",
                "available_at_runtime": false
            }
        ]"#;
        std::fs::write(&tmp, json).unwrap();
        let n = register_descriptors_from_file(tmp.to_str().unwrap())
            .expect("register should succeed");
        assert_eq!(n, 1);
        let names: Vec<String> = all_descriptors().into_iter().map(|d| d.name).collect();
        assert!(names.contains(&"p93_test_arbitrary".to_string()));
        std::fs::remove_file(&tmp).ok();
    }
}
