//! Real CUDA 13.2 backend via IronAccelerator.
//!
//! Activated only when the prototype is built with `--features cuda`.
//! Uses `ironaccelerator_cuda::cudarc_compat::CudaDevice` which:
//!
//! - Targets CUDA Toolkit 13.2 ABI explicitly (vs RecursiveMachineIntelligence's stale
//!   cudarc-0.10 path which only handles CUDA 11/12).
//! - Uses libloading so `cargo build --features cuda` succeeds on
//!   machines WITHOUT the CUDA toolkit installed (libs dlopen'd at
//!   runtime). Driver presence is checked when `CudaBackend::new()`
//!   is actually called, not at link time.
//! - Is the cudarc 0.19+ migration path that IronAccelerator was
//!   designed for ("decisively faster than cudarc 0.19 across 16-19
//!   of 20 host-side workloads" per IA's README).
//!
//! This module is the agent-visible CUDA surface. It supplies the
//! `SelectedBackend::Cuda` variant a handle, exposes device id /
//! info, and prepares the ground for the generic-over-Backend
//! `rmil_compute::run_pipeline` refactor (follow-on phase) that
//! routes actual op dispatch through the GPU.

#![cfg(feature = "cuda")]

use async_trait::async_trait;
use ironaccelerator_cuda::blas::{
    self as blas, ComputeType, DType as CuDType, MatmulDesc, MatrixLayout, Op, Preference,
};
use ironaccelerator_cuda::cudarc_compat::{compile_ptx, CudaDevice};
use ironaccelerator_cuda::drv::{DeviceBuf, Module};
use ironaccelerator_cuda::launch::launch_1d;
use ironaccelerator_cuda::rng::Rng;
use rmi::compute::cpu::CpuBackend;
use rmi::compute::{Backend, BackendType, DType, DeviceInfo, TensorHandle};
use rmi::error::{Result, RmiError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

/// CUDA backend. Owns:
///   - IA `CudaDevice` (CUDA 13.2 device handle, lazily-constructed
///     default stream)
///   - An inner `CpuBackend` for ops we haven't yet routed through
///     the GPU. Lets the `Backend` trait surface be complete TODAY so
///     `rmil_compute::run_pipeline(&cuda_backend, ...)` compiles and
///     runs (CPU semantics, with GPU acquired for downstream use).
///
/// **Why this hybrid shape**: P100 made `run_pipeline` generic over
/// `&dyn Backend`, so the type system is unblocked. The remaining
/// gap was a `Backend` impl on a CUDA-typed handle. This file ships
/// the impl; per-method `// TODO(P102+)` markers identify which ops
/// still need real IA cudarc-compat routes (matmul -> cuBLASLt,
/// elementwise -> NVRTC kernels, reductions -> CUB-style).
///
/// Operators with NVIDIA driver installed get a real device acquired
/// at construction; subsequent ops happen on CPU until each TODO is
/// replaced. **No semantic regression** vs P100 - CPU dispatch is
/// the floor; GPU routes are the ratchet.
pub struct CudaBackend {
    device: Arc<CudaDevice>,
    ordinal: usize,
    /// Fallback compute path for trait methods not yet routed
    /// through the GPU. Storage actually lives here; the IA device
    /// is held for kernel launches we wire incrementally.
    cpu: CpuBackend,
    /// Device-info snapshot computed at construction. Cheap to
    /// hand back from `device_info()`.
    info: DeviceInfo,
    /// Counter for matmul GPU dispatches (observability — proves the
    /// real cuBLASLt path is exercised, not the CPU bounce).
    matmul_gpu_count: AtomicU64,
    /// Counter for elementwise/activation GPU dispatches via NVRTC
    /// kernels (P104). Bumped once per real-GPU `add/sub/mul/div/scale/
    /// relu/gelu/sigmoid/tanh` call.
    elementwise_gpu_count: AtomicU64,
    /// Lazily-compiled NVRTC module containing the elementwise +
    /// activation kernels. Compiled on first use (a few hundred ms for
    /// `compute_80`), then reused for every subsequent op.
    kernels: OnceLock<Arc<Module>>,
    /// P118: separate NVRTC module for the half-precision (F16/BF16)
    /// conversion kernels. Kept apart from `kernels` because it
    /// `#include`s `<cuda_fp16.h>`/`<cuda_bf16.h>` — isolating it means
    /// an fp16-header issue can never break the proven F32 module.
    half_kernels: OnceLock<Arc<Module>>,
    /// P-storage: GPU-resident storage for F32 tensors created by this
    /// backend. Handle id → DeviceBuf. Non-F32 handles still live in
    /// `self.cpu` (CudaBackend just passes those through). Reshape
    /// shares storage by inserting a new id pointing at the same Arc.
    gpu_storage: RwLock<HashMap<u64, Arc<DeviceBuf<f32>>>>,
    /// P118: GPU storage for half-precision tensors (F16 and BF16),
    /// holding the raw 16-bit payload as `u16`. The handle's `dtype`
    /// (F16 vs BF16) distinguishes interpretation. Separate id space
    /// shared with `gpu_storage` via `next_id`.
    gpu_storage_u16: RwLock<HashMap<u64, Arc<DeviceBuf<u16>>>>,
    /// P121: GPU storage for INT8-quantized tensors. The per-tensor
    /// quantization scale is NOT stored here (it travels alongside the
    /// handle through the `quantize_i8`/`matmul_i8` inherent API).
    gpu_storage_i8: RwLock<HashMap<u64, Arc<DeviceBuf<i8>>>>,
    /// P124: cache of per-channel-quantized WEIGHTS for `quantized_matmul`,
    /// keyed by the source F32 weight's handle id. Weights are static
    /// across forward passes (the pipeline reuses one handle id), so they
    /// only need quantizing once. Maps id → (i8 handle, per-column scales).
    /// Assumes weights aren't mutated in place (inference, not training).
    quant_weight_cache: RwLock<HashMap<u64, (TensorHandle, Vec<f32>)>>,
    /// P139: cache of packed-INT4 per-channel weights for the W4A8
    /// calibrated path, keyed by source F32 weight handle id.
    quant_w4_cache: RwLock<HashMap<u64, (TensorHandle, Vec<f32>)>>,
    /// Count of `quantized_matmul` calls that reused a cached weight.
    quant_cache_hits: AtomicU64,
    /// P126: count of INT8 GEMMs that took the real cuBLASLt IMMA
    /// tensor-core path (vs the F32 fallback). Lets tests/bench confirm
    /// IMMA actually ran.
    quant_imma_count: AtomicU64,
    /// Monotonic id counter for handles backed by `gpu_storage`. The
    /// id space is independent from CpuBackend's; the `backend` field
    /// on `TensorHandle` is the source of truth for which to look in.
    next_id: AtomicU64,
    /// Counter for ops that took the per-op CPU bounce path (input on
    /// GPU, op runs on CPU, result copied back to GPU). High value
    /// means there's still op coverage to write. Observability only.
    bounce_count: AtomicU64,
    /// Monotonic seed source for cuRAND fills (P112). Each `rand`/`randn`
    /// call consumes one value so successive calls produce independent
    /// streams; the fixed start makes a run reproducible.
    rng_counter: AtomicU64,
}

impl CudaBackend {
    /// Acquire CUDA device `ordinal` (default 0). Returns Err if the
    /// driver isn't loaded at runtime, the ordinal is out of range,
    /// or context creation fails.
    pub fn new() -> Result<Self> {
        Self::with_device(0)
    }

    pub fn with_device(ordinal: usize) -> Result<Self> {
        let device = CudaDevice::new(ordinal).map_err(|e| {
            RmiError::compute_simple(format!("CUDA device {ordinal} init: {e:?}"))
        })?;
        let name = device.name().unwrap_or_else(|_| format!("CUDA Device {ordinal}"));
        let (free_mem, total_mem) = device
            .mem_get_info()
            .unwrap_or((0, 0));
        let info = DeviceInfo {
            name,
            backend_type: BackendType::Cuda,
            total_memory: total_mem as u64,
            available_memory: free_mem as u64,
            compute_capability: device.compute_capability().ok(),
            compute_units: 0, // IA doesn't surface SM count via cudarc_compat
        };
        Ok(Self {
            device,
            ordinal,
            cpu: CpuBackend::new(),
            info,
            matmul_gpu_count: AtomicU64::new(0),
            elementwise_gpu_count: AtomicU64::new(0),
            kernels: OnceLock::new(),
            half_kernels: OnceLock::new(),
            gpu_storage: RwLock::new(HashMap::new()),
            gpu_storage_u16: RwLock::new(HashMap::new()),
            gpu_storage_i8: RwLock::new(HashMap::new()),
            quant_weight_cache: RwLock::new(HashMap::new()),
            quant_w4_cache: RwLock::new(HashMap::new()),
            quant_cache_hits: AtomicU64::new(0),
            quant_imma_count: AtomicU64::new(0),
            next_id: AtomicU64::new(1),
            bounce_count: AtomicU64::new(0),
            rng_counter: AtomicU64::new(0x9E37_79B9_7F4A_7C15),
        })
    }

    pub fn device_id(&self) -> usize {
        self.ordinal
    }

    /// Number of matmul ops that took the real GPU (cuBLASLt) path
    /// since this backend was constructed.
    pub fn matmul_gpu_count(&self) -> u64 {
        self.matmul_gpu_count.load(Ordering::Relaxed)
    }

    /// Number of elementwise/activation ops that took the real GPU
    /// (NVRTC kernel) path since this backend was constructed.
    pub fn elementwise_gpu_count(&self) -> u64 {
        self.elementwise_gpu_count.load(Ordering::Relaxed)
    }

    /// Number of ops that took the CPU bounce path (GPU input → CPU
    /// compute → GPU result) because the op isn't yet GPU-routed.
    pub fn bounce_count(&self) -> u64 {
        self.bounce_count.load(Ordering::Relaxed)
    }

    /// Number of F32 tensors currently held in GPU storage.
    pub fn gpu_storage_len(&self) -> usize {
        self.gpu_storage.read().expect("gpu_storage poisoned").len()
    }

    /// Number of `quantized_matmul` calls that reused a cached quantized
    /// weight (P124) instead of re-quantizing it.
    pub fn quant_cache_hits(&self) -> u64 {
        self.quant_cache_hits.load(Ordering::Relaxed)
    }

    /// Number of INT8 GEMMs that took the cuBLASLt IMMA tensor-core path
    /// (P126). If this stays 0 after a quantized run, IMMA fell back to F32.
    pub fn quant_imma_count(&self) -> u64 {
        self.quant_imma_count.load(Ordering::Relaxed)
    }

    /// Borrow the IA device handle. Callers can pass this to any
    /// `ironaccelerator_cuda::*` API for kernel launches, memory
    /// allocation, cuBLASLt dispatch, etc.
    pub fn device(&self) -> &Arc<CudaDevice> {
        &self.device
    }
}

impl std::fmt::Debug for CudaBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CudaBackend")
            .field("ordinal", &self.ordinal)
            .field("provider", &"IronAccelerator CUDA 13.2")
            .finish()
    }
}

// ────────────────────────────────────────────────────────────────────
// Backend trait impl
//
// Routing strategy:
//   - device_info / is_available / backend_type: real GPU answers
//     where possible, fall back to declared metadata
//   - allocate / free / copy: route through CpuBackend storage
//     (TODO: replace with IA DeviceBuf::alloc + copy_from_host /
//     copy_to_host so handles back GPU memory)
//   - matmul / elementwise / reductions / activations: route through
//     CpuBackend (TODO: cuBLASLt for matmul, NVRTC kernels for
//     elementwise, custom reductions)
//
// Each TODO is one well-scoped follow-on phase. The shape of the
// impl is settled now so the per-method swaps land cleanly without
// touching run_pipeline / dispatch_* / TensorHandle.
// ────────────────────────────────────────────────────────────────────

#[async_trait]
impl Backend for CudaBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Cuda
    }
    fn device_info(&self) -> &DeviceInfo {
        &self.info
    }
    fn is_available(&self) -> bool {
        // We constructed; IA libloading didn't fail. Real driver
        // calls happen lazily on first op; treat ourselves as
        // available unless we know otherwise.
        true
    }

    // ─── P-storage: F32 lives in self.gpu_storage as DeviceBuf;
    //     non-F32 still lives in self.cpu (CudaBackend passes through). ───
    fn allocate(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        if dtype != DType::F32 {
            return self.cpu.allocate(shape, dtype);
        }
        let stream = self.device.default_stream();
        let n = shape.iter().product::<usize>();
        let buf = DeviceBuf::<f32>::alloc(stream, n)
            .map_err(|e| RmiError::compute_simple(format!("DeviceBuf::alloc: {e:?}")))?;
        self.gpu_store_f32(buf, shape)
    }
    fn free(&self, h: &TensorHandle) -> Result<()> {
        if matches!(h.backend, BackendType::Cuda) {
            // Half (F16/BF16) tensors live in the u16 map; F32 in the
            // f32 map. Remove from whichever holds this id.
            if matches!(h.dtype, DType::F16 | DType::BF16) {
                self.gpu_storage_u16
                    .write()
                    .expect("gpu_storage_u16 poisoned")
                    .remove(&h.id);
            } else if matches!(h.dtype, DType::I8 | DType::I4) {
                self.gpu_storage_i8
                    .write()
                    .expect("gpu_storage_i8 poisoned")
                    .remove(&h.id);
            } else {
                self.gpu_storage.write().expect("gpu_storage poisoned").remove(&h.id);
            }
            return Ok(());
        }
        self.cpu.free(h)
    }
    fn copy_to_device(&self, h: &TensorHandle, data: &[u8]) -> Result<()> {
        if matches!(h.backend, BackendType::Cuda) && h.dtype == DType::F32 {
            if data.len() != h.numel() * 4 {
                return Err(RmiError::compute_simple(format!(
                    "copy_to_device: expected {} bytes, got {}",
                    h.numel() * 4,
                    data.len()
                )));
            }
            let host: Vec<f32> = data
                .chunks_exact(4)
                .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            // Replace the DeviceBuf for this id with a fresh htod'd one.
            let stream = self.device.default_stream();
            let buf = DeviceBuf::<f32>::from_host(stream, &host)
                .map_err(|e| RmiError::compute_simple(format!("htod: {e:?}")))?;
            self.gpu_storage
                .write()
                .expect("gpu_storage poisoned")
                .insert(h.id, Arc::new(buf));
            return Ok(());
        }
        self.cpu.copy_to_device(h, data)
    }
    fn copy_to_host(&self, h: &TensorHandle) -> Result<Vec<u8>> {
        // INT8 tensors: return the raw 1-byte payload (numel bytes).
        if matches!(h.backend, BackendType::Cuda) && h.dtype == DType::I8 {
            let buf = self.i8_get(h)?;
            let n = h.numel();
            let mut out = vec![0i8; n];
            buf.copy_to_host(&mut out)
                .map_err(|e| RmiError::compute_simple(format!("dtoh i8: {e:?}")))?;
            self.device
                .default_stream()
                .synchronize()
                .map_err(|e| RmiError::compute_simple(format!("stream sync: {e:?}")))?;
            return Ok(out.iter().map(|&v| v as u8).collect());
        }
        // Half tensors: return the raw 16-bit payload (numel*2 bytes).
        if matches!(h.backend, BackendType::Cuda) && matches!(h.dtype, DType::F16 | DType::BF16) {
            let buf = self.half_get(h)?;
            let n = h.numel();
            let mut out = vec![0u16; n];
            buf.copy_to_host(&mut out)
                .map_err(|e| RmiError::compute_simple(format!("dtoh half: {e:?}")))?;
            self.device
                .default_stream()
                .synchronize()
                .map_err(|e| RmiError::compute_simple(format!("stream sync: {e:?}")))?;
            let mut bytes = Vec::with_capacity(n * 2);
            for v in &out {
                bytes.extend_from_slice(&v.to_ne_bytes());
            }
            return Ok(bytes);
        }
        if matches!(h.backend, BackendType::Cuda) && h.dtype == DType::F32 {
            let buf = self.gpu_get_f32(h)?;
            let n = h.numel();
            let mut out = vec![0f32; n];
            buf.copy_to_host(&mut out)
                .map_err(|e| RmiError::compute_simple(format!("dtoh: {e:?}")))?;
            self.device
                .default_stream()
                .synchronize()
                .map_err(|e| RmiError::compute_simple(format!("stream sync: {e:?}")))?;
            // Reinterpret to bytes in native order — matches CpuBackend.
            let mut bytes = Vec::with_capacity(n * 4);
            for v in &out {
                bytes.extend_from_slice(&v.to_ne_bytes());
            }
            return Ok(bytes);
        }
        self.cpu.copy_to_host(h)
    }
    fn copy(&self, s: &TensorHandle, d: &TensorHandle) -> Result<()> {
        // Device-to-device copy when both ends are Cuda+F32 with same numel.
        if matches!(s.backend, BackendType::Cuda)
            && matches!(d.backend, BackendType::Cuda)
            && s.dtype == DType::F32
            && d.dtype == DType::F32
            && s.numel() == d.numel()
        {
            let src = self.gpu_get_f32(s)?;
            // d2d: read src out then htod into a fresh DeviceBuf
            // bound to d's id. Less efficient than copy_from_device
            // but avoids needing &mut on Arc-wrapped buffers.
            let n = s.numel();
            let mut tmp = vec![0f32; n];
            src.copy_to_host(&mut tmp)
                .map_err(|e| RmiError::compute_simple(format!("d2d dtoh: {e:?}")))?;
            self.device
                .default_stream()
                .synchronize()
                .map_err(|e| RmiError::compute_simple(format!("stream sync: {e:?}")))?;
            let new_buf = DeviceBuf::<f32>::from_host(self.device.default_stream(), &tmp)
                .map_err(|e| RmiError::compute_simple(format!("d2d htod: {e:?}")))?;
            self.gpu_storage
                .write()
                .expect("gpu_storage poisoned")
                .insert(d.id, Arc::new(new_buf));
            return Ok(());
        }
        self.cpu.copy(s, d)
    }

    fn zeros(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        if dtype != DType::F32 {
            return self.cpu.zeros(shape, dtype);
        }
        let stream = self.device.default_stream();
        let n = shape.iter().product::<usize>();
        let buf = DeviceBuf::<f32>::alloc_zeros(stream, n)
            .map_err(|e| RmiError::compute_simple(format!("DeviceBuf::alloc_zeros: {e:?}")))?;
        self.gpu_store_f32(buf, shape)
    }
    fn ones(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        if dtype != DType::F32 {
            return self.cpu.ones(shape, dtype);
        }
        // No fill-kernel yet; compute on CPU then upload.
        let n = shape.iter().product::<usize>();
        let host = vec![1.0f32; n];
        self.host_to_gpu_f32(&host, shape)
    }
    fn rand(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        if dtype != DType::F32 {
            return self.cpu.rand(shape, dtype);
        }
        // P112: cuRAND uniform [0,1) directly into GPU storage. Falls
        // back to CPU-generate + upload if curand isn't loadable.
        match self.fill_rng_f32_gpu(shape, false) {
            Ok(h) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(h)
            }
            Err(_) => self.rng_cpu_fallback(shape, dtype, false),
        }
    }
    fn randn(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        if dtype != DType::F32 {
            return self.cpu.randn(shape, dtype);
        }
        // P112: cuRAND normal(0,1) directly into GPU storage.
        match self.fill_rng_f32_gpu(shape, true) {
            Ok(h) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(h)
            }
            Err(_) => self.rng_cpu_fallback(shape, dtype, true),
        }
    }
    fn from_slice_f32(&self, data: &[f32], shape: &[usize]) -> Result<TensorHandle> {
        self.host_to_gpu_f32(data, shape)
    }

    // ─── arithmetic: P104 NVRTC kernels for elementwise (f32, same-shape).
    // Broadcasting / mismatched shapes / non-F32 dtype bounce to CPU. ───
    fn add(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_binary(a, b, |s, x, y| s.add(x, y)) {
            return r;
        }
        self.elementwise_binary_or_bounce("k_add_f32", a, b, |a, b| {
            self.bounce_binary(a, b, |cpu, x, y| cpu.add(x, y))
        })
    }
    fn sub(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_binary(a, b, |s, x, y| s.sub(x, y)) {
            return r;
        }
        self.elementwise_binary_or_bounce("k_sub_f32", a, b, |a, b| {
            self.bounce_binary(a, b, |cpu, x, y| cpu.sub(x, y))
        })
    }
    fn mul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_binary(a, b, |s, x, y| s.mul(x, y)) {
            return r;
        }
        self.elementwise_binary_or_bounce("k_mul_f32", a, b, |a, b| {
            self.bounce_binary(a, b, |cpu, x, y| cpu.mul(x, y))
        })
    }
    fn div(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_binary(a, b, |s, x, y| s.div(x, y)) {
            return r;
        }
        self.elementwise_binary_or_bounce("k_div_f32", a, b, |a, b| {
            self.bounce_binary(a, b, |cpu, x, y| cpu.div(x, y))
        })
    }
    fn matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        // P118: half-precision (F16/BF16) 2-D matmul → tensor cores.
        // Both operands must share the half dtype; result is F32.
        if matches!(a.dtype, DType::F16 | DType::BF16)
            && a.dtype == b.dtype
            && a.shape.len() == 2
            && b.shape.len() == 2
        {
            return match self.matmul_half_gpu(a, b) {
                Ok(h) => {
                    self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
                    Ok(h)
                }
                Err(e) => Err(e), // no CPU half-matmul to fall back to
            };
        }
        if a.dtype != DType::F32 || b.dtype != DType::F32 {
            // Other non-F32 bounce via CPU.
            return self.bounce_binary(a, b, |cpu, x, y| cpu.matmul(x, y));
        }
        // 2-D: plain cuBLASLt. 3-D+ with matching leading dims: P109
        // strided-batched cuBLASLt (CpuBackend can't do ND matmul, so
        // there's no CPU fallback for the batched case — the GPU path
        // is the only implementation).
        if a.shape.len() == 2 && b.shape.len() == 2 {
            return match self.matmul_2d_f32_gpu(a, b) {
                Ok(h) => {
                    self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
                    Ok(h)
                }
                Err(_) => self.bounce_binary(a, b, |cpu, x, y| cpu.matmul(x, y)),
            };
        }
        if a.shape.len() >= 3
            && b.shape.len() == a.shape.len()
            && a.shape[..a.shape.len() - 2] == b.shape[..b.shape.len() - 2]
        {
            return match self.matmul_batched_f32_gpu(a, b) {
                Ok(h) => {
                    self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
                    Ok(h)
                }
                // No CPU batched matmul to fall back to; surface the error.
                Err(e) => Err(e),
            };
        }
        // Mismatched ranks / broadcasting we don't support: bounce
        // (CpuBackend will error, preserving prior behavior).
        self.bounce_binary(a, b, |cpu, x, y| cpu.matmul(x, y))
    }
    fn quantized_matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        // P126: INT8 dynamic quantized matmul on cuBLASLt IMMA tensor
        // cores. Activations `a` → per-tensor INT8 (dynamic per call);
        // weights `b` → per-channel INT8, transposed to [N,K] (IMMA
        // TN layout), quantized once and cached (P124). INT8 needs dims
        // multiples of 4; otherwise (or on any error) we fall back to the
        // exact F32 matmul (always numerically correct).
        if a.dtype != DType::F32 || b.dtype != DType::F32 || a.shape.len() != 2 || b.shape.len() != 2
        {
            return self.matmul(a, b);
        }
        let (m, k, n) = (a.shape[0], a.shape[1], b.shape[1]);
        if m % 4 != 0 || k % 4 != 0 || n % 4 != 0 {
            return self.matmul(a, b); // IMMA-ineligible dims → exact F32
        }
        let result = (|| -> Result<TensorHandle> {
            // Activations: dynamic per-tensor quant every call (transient).
            let (aq, sa) = self.quantize_i8(a)?;
            // Weights: quantize transposed once, cache by source id (P124).
            let cached = self
                .quant_weight_cache
                .read()
                .expect("quant_weight_cache poisoned")
                .get(&b.id)
                .cloned();
            let (bq_t, sb) = match cached {
                Some((h, s)) => {
                    self.quant_cache_hits.fetch_add(1, Ordering::Relaxed);
                    (h, s)
                }
                None => {
                    let (h, s) = self.quantize_i8_perchannel_t(b)?;
                    self.quant_weight_cache
                        .write()
                        .expect("quant_weight_cache poisoned")
                        .insert(b.id, (h.clone(), s.clone()));
                    (h, s)
                }
            };
            let out = self.matmul_i8_immma(&aq, sa, &bq_t, &sb)?;
            let _ = self.free(&aq); // activation is transient
            Ok(out)
        })();
        // On any quantization / cuBLAS error, fall back to exact F32.
        match result {
            Ok(h) => Ok(h),
            Err(_) => self.matmul(a, b),
        }
    }
    fn quantized_matmul_calibrated(
        &self,
        a: &TensorHandle,
        a_scale: f32,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        // Delegate to the inherent IMMA calibrated path (P128).
        self.quantized_matmul_calibrated_impl(a, a_scale, b)
    }
    fn quantized_matmul_w4_calibrated(
        &self,
        a: &TensorHandle,
        a_scale: f32,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        // P139: W4A8 calibrated path. INT8 activation (calibrated scale,
        // no reduction/sync) × packed-INT4 per-channel weight (quantized
        // once, cached). Naive GEMM (no IMMA for int4); exact F32 fallback.
        if a.dtype != DType::F32 || b.dtype != DType::F32 || a.shape.len() != 2 || b.shape.len() != 2
        {
            return self.matmul(a, b);
        }
        let result = (|| -> Result<TensorHandle> {
            let aq = self.quantize_i8_with_scale(a, a_scale)?;
            let cached = self
                .quant_w4_cache
                .read()
                .expect("quant_w4_cache poisoned")
                .get(&b.id)
                .cloned();
            let (wq, sb) = match cached {
                Some((h, s)) => {
                    self.quant_cache_hits.fetch_add(1, Ordering::Relaxed);
                    (h, s)
                }
                None => {
                    let (h, s) = self.quantize_i4_perchannel(b)?;
                    self.quant_w4_cache
                        .write()
                        .expect("quant_w4_cache poisoned")
                        .insert(b.id, (h.clone(), s.clone()));
                    (h, s)
                }
            };
            let out = self.matmul_i8a_i4b(&aq, a_scale, &wq, &sb)?;
            let _ = self.free(&aq);
            Ok(out)
        })();
        match result {
            Ok(h) => Ok(h),
            Err(_) => self.matmul(a, b),
        }
    }
    fn quantized_matmul_asym_calibrated(
        &self,
        a: &TensorHandle,
        a_lo: f32,
        a_hi: f32,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        // P136: asymmetric calibrated activations on the IMMA path.
        // Quantize the activation with the calibrated [lo,hi] range +
        // zero-point (no reduction/sync), reuse the cached transposed
        // per-channel weight, run the IMMA GEMM with the exact zero-point
        // correction. Falls back to exact F32 on ineligible dims/errors.
        if a.dtype != DType::F32 || b.dtype != DType::F32 || a.shape.len() != 2 || b.shape.len() != 2
        {
            return self.matmul(a, b);
        }
        let (m, k, n) = (a.shape[0], a.shape[1], b.shape[1]);
        if m % 4 != 0 || k % 4 != 0 || n % 4 != 0 {
            return self.matmul(a, b);
        }
        let result = (|| -> Result<TensorHandle> {
            let (aq, sa, za) = self.quantize_i8_asym(a, a_lo, a_hi)?;
            let cached = self
                .quant_weight_cache
                .read()
                .expect("quant_weight_cache poisoned")
                .get(&b.id)
                .cloned();
            let (bq_t, sb) = match cached {
                Some((h, s)) => {
                    self.quant_cache_hits.fetch_add(1, Ordering::Relaxed);
                    (h, s)
                }
                None => {
                    let (h, s) = self.quantize_i8_perchannel_t(b)?;
                    self.quant_weight_cache
                        .write()
                        .expect("quant_weight_cache poisoned")
                        .insert(b.id, (h.clone(), s.clone()));
                    (h, s)
                }
            };
            let out = self.matmul_i8_immma_asym(&aq, sa, za, &bq_t, &sb)?;
            let _ = self.free(&aq);
            Ok(out)
        })();
        match result {
            Ok(h) => Ok(h),
            Err(_) => self.matmul(a, b),
        }
    }
    fn scale(&self, a: &TensorHandle, s: f64) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_unary(a, move |bk, x| bk.scale(x, s)) {
            return r;
        }
        if a.dtype != DType::F32 {
            return self.cpu.scale(a, s);
        }
        match self.scale_f32_gpu(a, s as f32) {
            Ok(h) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(h)
            }
            Err(_) => self.bounce_unary(a, move |cpu, x| cpu.scale(x, s)),
        }
    }

    // ─── reductions: P105 scalar reductions via NVRTC (F32 only;
    // axis-reductions and non-F32 bounce) ───
    fn sum(&self, a: &TensorHandle) -> Result<f64> {
        if let Some(r) = self.try_half_scalar(a, |s, x| s.sum(x)) {
            return r;
        }
        // Scalar reductions return f64, not TensorHandle — bounce-via-cpu
        // for fallback must use handle_to_host_f32 then cpu math.
        if a.dtype != DType::F32 || a.numel() == 0 {
            // For non-F32 or empty, ensure cpu has the data via host pass.
            return self.scalar_reduce_bounce(a, |cpu, h| cpu.sum(h));
        }
        match self.reduce_scalar_f32_gpu("k_sum_f32", a) {
            Ok(v) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(v as f64)
            }
            Err(_) => self.scalar_reduce_bounce(a, |cpu, h| cpu.sum(h)),
        }
    }
    fn sum_axis(&self, a: &TensorHandle, ax: usize) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_unary(a, move |s, x| s.sum_axis(x, ax)) {
            return r;
        }
        if a.dtype != DType::F32 || a.shape.is_empty() || ax >= a.shape.len() {
            return self.bounce_unary(a, move |cpu, x| cpu.sum_axis(x, ax));
        }
        // Last axis → optimized block-per-row reduce (P107). Any other
        // axis → general [outer, axis, inner] kernel (P110).
        let is_last = ax == a.shape.len() - 1;
        let res = if is_last {
            self.sum_axis_lastdim_f32_gpu(a)
        } else {
            self.sum_axis_any_f32_gpu(a, ax)
        };
        match res {
            Ok(h) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(h)
            }
            Err(_) => self.bounce_unary(a, move |cpu, x| cpu.sum_axis(x, ax)),
        }
    }
    fn mean(&self, a: &TensorHandle) -> Result<f64> {
        if let Some(r) = self.try_half_scalar(a, |s, x| s.mean(x)) {
            return r;
        }
        if a.dtype != DType::F32 || a.numel() == 0 {
            return self.scalar_reduce_bounce(a, |cpu, h| cpu.mean(h));
        }
        match self.reduce_scalar_f32_gpu("k_sum_f32", a) {
            Ok(s) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok((s as f64) / a.numel() as f64)
            }
            Err(_) => self.scalar_reduce_bounce(a, |cpu, h| cpu.mean(h)),
        }
    }
    fn mean_axis(&self, a: &TensorHandle, ax: usize) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_unary(a, move |s, x| s.mean_axis(x, ax)) {
            return r;
        }
        if a.dtype != DType::F32 || a.shape.is_empty() || ax >= a.shape.len() {
            return self.bounce_unary(a, move |cpu, x| cpu.mean_axis(x, ax));
        }
        let inner = a.shape[ax] as f32;
        if inner == 0.0 {
            return self.bounce_unary(a, move |cpu, x| cpu.mean_axis(x, ax));
        }
        let is_last = ax == a.shape.len() - 1;
        let summed = if is_last {
            self.sum_axis_lastdim_f32_gpu(a)
        } else {
            self.sum_axis_any_f32_gpu(a, ax)
        };
        match summed.and_then(|h| self.scale_f32_gpu(&h, 1.0 / inner)) {
            Ok(h) => {
                self.elementwise_gpu_count.fetch_add(2, Ordering::Relaxed);
                Ok(h)
            }
            Err(_) => self.bounce_unary(a, move |cpu, x| cpu.mean_axis(x, ax)),
        }
    }
    fn max(&self, a: &TensorHandle) -> Result<f64> {
        if let Some(r) = self.try_half_scalar(a, |s, x| s.max(x)) {
            return r;
        }
        if a.dtype != DType::F32 || a.numel() == 0 {
            return self.scalar_reduce_bounce(a, |cpu, h| cpu.max(h));
        }
        match self.reduce_scalar_f32_gpu("k_max_f32", a) {
            Ok(v) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(v as f64)
            }
            Err(_) => self.scalar_reduce_bounce(a, |cpu, h| cpu.max(h)),
        }
    }
    fn min(&self, a: &TensorHandle) -> Result<f64> {
        if let Some(r) = self.try_half_scalar(a, |s, x| s.min(x)) {
            return r;
        }
        if a.dtype != DType::F32 || a.numel() == 0 {
            return self.scalar_reduce_bounce(a, |cpu, h| cpu.min(h));
        }
        match self.reduce_scalar_f32_gpu("k_min_f32", a) {
            Ok(v) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(v as f64)
            }
            Err(_) => self.scalar_reduce_bounce(a, |cpu, h| cpu.min(h)),
        }
    }

    // ─── activations: P104 NVRTC kernels (f32; half upcasts; other dtypes bounce) ───
    fn relu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_unary(a, |s, x| s.relu(x)) {
            return r;
        }
        self.elementwise_unary_or_bounce("k_relu_f32", a, |a| {
            self.bounce_unary(a, |cpu, x| cpu.relu(x))
        })
    }
    fn gelu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_unary(a, |s, x| s.gelu(x)) {
            return r;
        }
        self.elementwise_unary_or_bounce("k_gelu_f32", a, |a| {
            self.bounce_unary(a, |cpu, x| cpu.gelu(x))
        })
    }
    fn sigmoid(&self, a: &TensorHandle) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_unary(a, |s, x| s.sigmoid(x)) {
            return r;
        }
        self.elementwise_unary_or_bounce("k_sigmoid_f32", a, |a| {
            self.bounce_unary(a, |cpu, x| cpu.sigmoid(x))
        })
    }
    fn tanh(&self, a: &TensorHandle) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_unary(a, |s, x| s.tanh(x)) {
            return r;
        }
        self.elementwise_unary_or_bounce("k_tanh_f32", a, |a| {
            self.bounce_unary(a, |cpu, x| cpu.tanh(x))
        })
    }
    fn softmax(&self, a: &TensorHandle, ax: i32) -> Result<TensorHandle> {
        if let Some(r) = self.try_half_unary(a, move |s, x| s.softmax(x, ax)) {
            return r;
        }
        if a.dtype != DType::F32 || a.shape.is_empty() {
            return self.bounce_unary(a, move |cpu, x| cpu.softmax(x, ax));
        }
        let ndim = a.shape.len() as i32;
        let last = ndim - 1;
        // Normalize a possibly-negative axis index to [0, ndim).
        let norm = if ax < 0 { ax + ndim } else { ax };
        if norm < 0 || norm >= ndim {
            return self.bounce_unary(a, move |cpu, x| cpu.softmax(x, ax));
        }
        let is_last = norm == last;
        // Last axis → optimized block-per-row kernel (P105). Any other
        // axis → general [outer, axis, inner] kernel (P110).
        let res = if is_last {
            self.softmax_lastdim_f32_gpu(a)
        } else {
            self.softmax_axis_any_f32_gpu(a, norm as usize)
        };
        match res {
            Ok(h) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(h)
            }
            Err(_) => self.bounce_unary(a, move |cpu, x| cpu.softmax(x, ax)),
        }
    }

    // ─── shape ops ───
    fn reshape(&self, a: &TensorHandle, new_shape: &[usize]) -> Result<TensorHandle> {
        // Reshape is metadata-only when storage is contiguous. Share
        // the same DeviceBuf under a new id with the new shape. Numel
        // must match.
        if matches!(a.backend, BackendType::Cuda) && a.dtype == DType::F32 {
            let old_n: usize = a.shape.iter().product();
            let new_n: usize = new_shape.iter().product();
            if old_n != new_n {
                return Err(RmiError::compute_simple(format!(
                    "reshape numel mismatch: {old_n} → {new_n}"
                )));
            }
            let buf = self.gpu_get_f32(a)?;
            let id = self.fresh_id();
            self.gpu_storage
                .write()
                .expect("gpu_storage poisoned")
                .insert(id, buf);
            return Ok(TensorHandle {
                id,
                shape: new_shape.to_vec(),
                dtype: DType::F32,
                backend: BackendType::Cuda,
                size_bytes: new_n * 4,
            });
        }
        self.cpu.reshape(a, new_shape)
    }
    fn cast(&self, a: &TensorHandle, target: DType) -> Result<TensorHandle> {
        use DType::{BF16, F16, F32};
        // Same dtype → return a view sharing the underlying buffer.
        if a.dtype == target {
            return Ok(a.clone());
        }
        match (a.dtype, target) {
            (F32, F16) | (F32, BF16) => self.to_half_gpu(a, target),
            (F16, F32) | (BF16, F32) => self.half_to_f32_gpu(a),
            (F16, BF16) | (BF16, F16) => {
                let f = self.half_to_f32_gpu(a)?;
                let r = self.to_half_gpu(&f, target);
                let _ = self.free(&f);
                r
            }
            // F64/integer targets/sources have no GPU representation here.
            _ => Err(RmiError::compute_simple(format!(
                "CUDA cast {:?}→{:?} unsupported (only F32/F16/BF16)",
                a.dtype, target
            ))),
        }
    }
    fn transpose(&self, a: &TensorHandle, axes: &[usize]) -> Result<TensorHandle> {
        if matches!(a.dtype, DType::F16 | DType::BF16) {
            let axes_owned = axes.to_vec();
            return self.half_unary_via_f32(a, move |x| {
                // Re-dispatch on the F32 handle through the trait method.
                <Self as Backend>::transpose(self, x, &axes_owned)
            });
        }
        if a.dtype != DType::F32 {
            return self.cpu.transpose(a, axes);
        }
        let ndim = a.shape.len();
        // `axes` must be a permutation of 0..ndim for the GPU paths.
        let valid_perm = axes.len() == ndim && {
            let mut seen = vec![false; ndim];
            axes.iter().all(|&ax| ax < ndim && !std::mem::replace(&mut seen[ax], true))
        };
        if valid_perm {
            // 2-D [1,0] → optimized coalesced 16×16 kernel (P108).
            // Any other rank/perm → general stride-aware kernel (P111).
            let res = if ndim == 2 && axes == [1usize, 0] {
                self.transpose_2d_f32_gpu(a)
            } else {
                self.permute_f32_gpu(a, axes)
            };
            match res {
                Ok(h) => {
                    self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(h);
                }
                Err(_) => {
                    let axes_owned = axes.to_vec();
                    return self.bounce_unary(a, move |cpu, x| cpu.transpose(x, &axes_owned));
                }
            }
        }
        // Malformed axes — let CpuBackend surface the error via bounce.
        let axes_owned = axes.to_vec();
        self.bounce_unary(a, move |cpu, x| cpu.transpose(x, &axes_owned))
    }
    fn conv2d(
        &self,
        input: &TensorHandle,
        weight: &TensorHandle,
        stride: usize,
        padding: usize,
        dilation: usize,
    ) -> Result<TensorHandle> {
        // P118: half conv — upcast input+weight to F32, run the F32 conv,
        // downcast the result back to the input's half dtype (half in,
        // half out). Reuses the whole im2col+GEMM path.
        if matches!(input.dtype, DType::F16 | DType::BF16) && input.dtype == weight.dtype {
            let xi = self.half_to_f32_gpu(input)?;
            let wi = self.half_to_f32_gpu(weight)?;
            let yf = self.conv2d(&xi, &wi, stride, padding, dilation)?;
            let out = self.to_half_gpu(&yf, input.dtype);
            let _ = self.free(&xi);
            let _ = self.free(&wi);
            let _ = self.free(&yf);
            return out;
        }
        // P113: conv2d as im2col + GEMM, entirely on the GPU. Reuses the
        // proven cuBLASLt matmul + reshape + permute paths; the only new
        // kernel is im2col. Any validation failure or GPU error bounces
        // to the CpuBackend reference (which now implements conv2d).
        let cpu_fallback = |b: &CudaBackend| -> Result<TensorHandle> {
            let xs = b.handle_to_host_f32(input)?;
            let ws = b.handle_to_host_f32(weight)?;
            let cx = b.cpu.from_slice_f32(&xs, &input.shape)?;
            let cw = b.cpu.from_slice_f32(&ws, &weight.shape)?;
            let cy = b.cpu.conv2d(&cx, &cw, stride, padding, dilation)?;
            let out_shape = cy.shape.clone();
            let ys = b.cpu.copy_to_host(&cy)?;
            let yf: Vec<f32> = ys
                .chunks_exact(4)
                .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let _ = b.cpu.free(&cx);
            let _ = b.cpu.free(&cw);
            let _ = b.cpu.free(&cy);
            b.bounce_count.fetch_add(1, Ordering::Relaxed);
            b.host_to_gpu_f32(&yf, &out_shape)
        };

        if input.dtype != DType::F32
            || weight.dtype != DType::F32
            || input.shape.len() != 4
            || weight.shape.len() != 4
            || stride == 0
            || dilation == 0
        {
            return cpu_fallback(self);
        }
        match self.conv2d_im2col_gpu(input, weight, stride, padding, dilation) {
            Ok(h) => Ok(h),
            Err(_) => cpu_fallback(self),
        }
    }
    fn concat(&self, ts: &[&TensorHandle], ax: usize) -> Result<TensorHandle> {
        // P114: device-side strided copy when every input is GPU-resident
        // F32. Falls back to the CPU bounce on any error / ineligible input.
        if !ts.is_empty()
            && ts.iter().all(|t| matches!(t.backend, BackendType::Cuda) && t.dtype == DType::F32)
        {
            match self.concat_gpu(ts, ax) {
                Ok(h) => {
                    self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(h);
                }
                Err(_) => return self.concat_bounce(ts, ax),
            }
        }
        self.cpu.concat(ts, ax)
    }
    fn split(&self, a: &TensorHandle, ax: usize, n: usize) -> Result<Vec<TensorHandle>> {
        if matches!(a.backend, BackendType::Cuda) && a.dtype == DType::F32 {
            match self.split_gpu(a, ax, n) {
                Ok(parts) => {
                    self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(parts);
                }
                Err(_) => return self.split_bounce(a, ax, n),
            }
        }
        self.cpu.split(a, ax, n)
    }

    // (matmul_2d_f32_gpu implementation lives in the inherent impl below)

    fn synchronize(&self) -> Result<()> {
        // Real sync on the IA stream when we have actual GPU work
        // queued. For now (CPU-backed storage), just sync any IA
        // state that exists; ignore errors so the CPU dispatch
        // isn't blocked.
        let _ = self.device.synchronize();
        self.cpu.synchronize()
    }
}

// ────────────────────────────────────────────────────────────────────
// Real-GPU op implementations.
//
// Each method here is one TODO marker discharged from the trait impl
// above. They take the same arguments as the trait method and return
// the same TensorHandle shape, so the trait impl can swap to the GPU
// path with a single `self.foo_gpu(args)?` line.
// ────────────────────────────────────────────────────────────────────

impl CudaBackend {
    // ─── P-storage primitives ─────────────────────────────────────

    /// Mint a fresh handle id for a GPU-resident tensor.
    fn fresh_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Wrap a freshly-allocated `DeviceBuf<f32>` in `Arc`, insert into
    /// GPU storage, and return a `BackendType::Cuda`-tagged handle.
    fn gpu_store_f32(&self, buf: DeviceBuf<f32>, shape: &[usize]) -> Result<TensorHandle> {
        let id = self.fresh_id();
        let numel = shape.iter().product::<usize>();
        let handle = TensorHandle {
            id,
            shape: shape.to_vec(),
            dtype: DType::F32,
            backend: BackendType::Cuda,
            size_bytes: numel * std::mem::size_of::<f32>(),
        };
        self.gpu_storage
            .write()
            .expect("gpu_storage poisoned")
            .insert(id, Arc::new(buf));
        Ok(handle)
    }

    /// Look up a GPU-resident F32 buffer by handle. Errors if the
    /// handle isn't Cuda-backed F32 or its id isn't in GPU storage.
    fn gpu_get_f32(&self, h: &TensorHandle) -> Result<Arc<DeviceBuf<f32>>> {
        if h.dtype != DType::F32 {
            return Err(RmiError::compute_simple(format!(
                "gpu_get_f32: handle is {:?}, not F32",
                h.dtype
            )));
        }
        if !matches!(h.backend, BackendType::Cuda) {
            return Err(RmiError::compute_simple(format!(
                "gpu_get_f32: handle backend is {:?}, not Cuda",
                h.backend
            )));
        }
        self.gpu_storage
            .read()
            .expect("gpu_storage poisoned")
            .get(&h.id)
            .cloned()
            .ok_or_else(|| {
                RmiError::compute_simple(format!(
                    "gpu_get_f32: id {} not in GPU storage (free'd?)",
                    h.id
                ))
            })
    }

    /// Read an F32 tensor into a host `Vec<f32>`, regardless of where
    /// its storage lives — GPU-resident via `dtoh`, CPU-resident via
    /// `cpu.copy_to_host`.
    fn handle_to_host_f32(&self, h: &TensorHandle) -> Result<Vec<f32>> {
        if matches!(h.backend, BackendType::Cuda) {
            let buf = self.gpu_get_f32(h)?;
            let n = h.numel();
            let mut out = vec![0f32; n];
            buf.copy_to_host(&mut out).map_err(|e| {
                RmiError::compute_simple(format!("dtoh in handle_to_host_f32: {e:?}"))
            })?;
            // dtoh is async on default stream; sync so the host slice
            // is observable before we return.
            self.device
                .default_stream()
                .synchronize()
                .map_err(|e| RmiError::compute_simple(format!("stream sync: {e:?}")))?;
            return Ok(out);
        }
        // Legacy fallback: handle is CPU-resident.
        let bytes = self.cpu.copy_to_host(h)?;
        let n = h.numel();
        if bytes.len() != n * 4 {
            return Err(RmiError::compute_simple(format!(
                "handle_to_host_f32: expected {} bytes, got {}",
                n * 4,
                bytes.len()
            )));
        }
        let mut out = Vec::with_capacity(n);
        for chunk in bytes.chunks_exact(4) {
            out.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(out)
    }

    /// Read host data → allocate GPU buffer → store and return a Cuda
    /// handle. Used at fresh-input boundaries (from_slice_f32) and at
    /// the end of bounces (cpu computed → upload back to GPU).
    fn host_to_gpu_f32(&self, data: &[f32], shape: &[usize]) -> Result<TensorHandle> {
        let stream = self.device.default_stream();
        let buf = DeviceBuf::<f32>::from_host(stream, data).map_err(|e| {
            RmiError::compute_simple(format!("htod in host_to_gpu_f32: {e:?}"))
        })?;
        self.gpu_store_f32(buf, shape)
    }

    /// P112: fill a fresh GPU-resident F32 tensor with cuRAND. `normal`
    /// selects N(0,1) vs uniform[0,1). One independent seed per call
    /// (Philox4_32_10). Over-allocates to an even length when needed
    /// (some pseudo generators require an even count for the normal
    /// fill) then truncates the logical length back to `n`.
    fn fill_rng_f32_gpu(&self, shape: &[usize], normal: bool) -> Result<TensorHandle> {
        let n: usize = shape.iter().product();
        if n == 0 {
            return self.host_to_gpu_f32(&[], shape);
        }
        let stream = self.device.default_stream();
        let seed = self.rng_counter.fetch_add(1, Ordering::Relaxed);
        let mut rng = Rng::new(self.device.raw(), &stream, seed)
            .map_err(|e| RmiError::compute_simple(format!("curand init: {e:?}")))?;
        // Even-length allocation guards the normal-fill count requirement.
        let alloc_n = if normal && n % 2 == 1 { n + 1 } else { n };
        let mut buf = DeviceBuf::<f32>::alloc(stream.clone(), alloc_n)
            .map_err(|e| RmiError::compute_simple(format!("alloc rng buf: {e:?}")))?;
        if normal {
            rng.fill_normal_f32(&mut buf, 0.0, 1.0)
                .map_err(|e| RmiError::compute_simple(format!("curand normal: {e:?}")))?;
        } else {
            rng.fill_uniform_f32(&mut buf)
                .map_err(|e| RmiError::compute_simple(format!("curand uniform: {e:?}")))?;
        }
        if alloc_n != n {
            buf.truncate(n);
        }
        self.gpu_store_f32(buf, shape)
    }

    /// Fallback for `rand`/`randn` when cuRAND isn't available: generate
    /// on the CPU and upload (preserves the old behavior + GPU residency).
    fn rng_cpu_fallback(
        &self,
        shape: &[usize],
        dtype: DType,
        normal: bool,
    ) -> Result<TensorHandle> {
        let cpu_h = if normal {
            self.cpu.randn(shape, dtype)?
        } else {
            self.cpu.rand(shape, dtype)?
        };
        let bytes = self.cpu.copy_to_host(&cpu_h)?;
        let host: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let _ = self.cpu.free(&cpu_h);
        self.host_to_gpu_f32(&host, shape)
    }

    /// Bounce a unary F32 op through CpuBackend (input on GPU → CPU
    /// compute → result on GPU). For ops we haven't GPU-routed yet.
    /// Used as the fallback in trait methods and as the direct path
    /// for shape ops we deliberately leave on CPU.
    fn bounce_unary<F>(&self, a: &TensorHandle, cpu_op: F) -> Result<TensorHandle>
    where
        F: FnOnce(&CpuBackend, &TensorHandle) -> Result<TensorHandle>,
    {
        let host = self.handle_to_host_f32(a)?;
        let cpu_a = self.cpu.from_slice_f32(&host, &a.shape)?;
        let cpu_result = cpu_op(&self.cpu, &cpu_a)?;
        let out_shape = cpu_result.shape.clone();
        let out_host = self.cpu.copy_to_host(&cpu_result)?;
        // Reinterpret bytes as f32. Only F32 paths use this bounce.
        let out_f32: Vec<f32> = out_host
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let _ = self.cpu.free(&cpu_a);
        let _ = self.cpu.free(&cpu_result);
        self.bounce_count.fetch_add(1, Ordering::Relaxed);
        self.host_to_gpu_f32(&out_f32, &out_shape)
    }

    /// Like `bounce_unary` but for two-input ops.
    fn bounce_binary<F>(
        &self,
        a: &TensorHandle,
        b: &TensorHandle,
        cpu_op: F,
    ) -> Result<TensorHandle>
    where
        F: FnOnce(&CpuBackend, &TensorHandle, &TensorHandle) -> Result<TensorHandle>,
    {
        let a_host = self.handle_to_host_f32(a)?;
        let b_host = self.handle_to_host_f32(b)?;
        let cpu_a = self.cpu.from_slice_f32(&a_host, &a.shape)?;
        let cpu_b = self.cpu.from_slice_f32(&b_host, &b.shape)?;
        let cpu_result = cpu_op(&self.cpu, &cpu_a, &cpu_b)?;
        let out_shape = cpu_result.shape.clone();
        let out_host = self.cpu.copy_to_host(&cpu_result)?;
        let out_f32: Vec<f32> = out_host
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let _ = self.cpu.free(&cpu_a);
        let _ = self.cpu.free(&cpu_b);
        let _ = self.cpu.free(&cpu_result);
        self.bounce_count.fetch_add(1, Ordering::Relaxed);
        self.host_to_gpu_f32(&out_f32, &out_shape)
    }

    /// 2-D F32 matmul via cuBLASLt. `a` is `[M, K]` row-major, `b` is
    /// `[K, N]` row-major; result is `[M, N]` row-major. Returns a
    /// host-resident TensorHandle (storage in CpuBackend) so subsequent
    /// trait calls Just Work — the GPU acceleration is the matmul
    /// itself, not the long-term storage.
    ///
    /// Row-major-via-col-major trick: cuBLAS computes column-major. To
    /// get row-major `D = A · B`, compute `D^T = B^T · A^T` in
    /// column-major view:
    ///   - cuBLAS A operand = our B with layout (N, K, ld=N), op N
    ///   - cuBLAS B operand = our A with layout (K, M, ld=K), op N
    ///   - cuBLAS C/D operand = our D with layout (N, M, ld=N)
    /// The resulting `D^T` in col-major IS our row-major `D`.
    fn matmul_2d_f32_gpu(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        let m = a.shape[0];
        let k = a.shape[1];
        let k2 = b.shape[0];
        let n = b.shape[1];
        if k != k2 {
            return Err(RmiError::compute_simple(format!(
                "matmul shape mismatch: [{m},{k}] x [{k2},{n}]"
            )));
        }
        let a_dev_arc = self.gpu_get_f32(a)?;
        let b_dev_arc = self.gpu_get_f32(b)?;
        let d_dev = self
            .matmul_lt_f32_gpu(&a_dev_arc, &b_dev_arc, m, k, n, 1)
            .map_err(|e| {
                RmiError::compute_simple(format!("CUDA matmul {m}x{k} @ {k}x{n}: {e:?}"))
            })?;
        self.gpu_store_f32(d_dev, &[m, n])
    }

    /// P118: 2-D half-precision matmul on tensor cores via cuBLASLt.
    /// Both operands must be the same half dtype (F16 or BF16); inputs
    /// are read at half width, accumulated in F32, and the result is
    /// returned as an **F32** tensor (the accumulation type). This is
    /// the whole point of non-F32 on GPU — tensor-core throughput at
    /// half the memory traffic. Same row-major-as-col-major trick as
    /// the F32 path.
    fn matmul_half_gpu(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        use ironaccelerator_cuda::drv::Error as DrvErr;
        let m = a.shape[0];
        let k = a.shape[1];
        let k2 = b.shape[0];
        let n = b.shape[1];
        if k != k2 {
            return Err(RmiError::compute_simple(format!(
                "half matmul shape mismatch: [{m},{k}] x [{k2},{n}]"
            )));
        }
        if a.dtype != b.dtype {
            return Err(RmiError::compute_simple(
                "half matmul operands must share dtype",
            ));
        }
        let in_dt = match a.dtype {
            DType::F16 => CuDType::R16F,
            DType::BF16 => CuDType::R16BF,
            _ => return Err(RmiError::compute_simple("matmul_half_gpu: not a half dtype")),
        };
        let a_dev = self.half_get(a)?;
        let b_dev = self.half_get(b)?;
        let stream = self.device.default_stream();

        let mk = || -> std::result::Result<DeviceBuf<f32>, DrvErr> {
            let d_dev = DeviceBuf::<f32>::alloc(stream.clone(), m * n)?;
            const WS_BYTES: usize = 4 * 1024 * 1024;
            let mut ws = DeviceBuf::<u8>::alloc(stream.clone(), WS_BYTES)?;
            let handle = blas::handle_for(&stream).map_err(|e| DrvErr::Precondition {
                op: "BlasLt::handle_for",
                msg: format!("{e:?}"),
            })?;
            // Compute + scale in F32; only the A/B operand layouts are half.
            let mut desc = MatmulDesc::new(ComputeType::F32, CuDType::R32F).map_err(|e| {
                DrvErr::Precondition { op: "MatmulDesc::new", msg: format!("{e:?}") }
            })?;
            desc.set_transpose(Op::N, Op::N).map_err(|e| DrvErr::Precondition {
                op: "MatmulDesc::set_transpose",
                msg: format!("{e:?}"),
            })?;
            // A operand = our B (half) [N,K] ld=N; B operand = our A (half)
            // [K,M] ld=K; C/D = our D (f32) [N,M] ld=N.
            let a_layout = MatrixLayout::new(in_dt, n as u64, k as u64, n as i64)
                .map_err(|e| DrvErr::Precondition { op: "MatrixLayout::new(A)", msg: format!("{e:?}") })?;
            let b_layout = MatrixLayout::new(in_dt, k as u64, m as u64, k as i64)
                .map_err(|e| DrvErr::Precondition { op: "MatrixLayout::new(B)", msg: format!("{e:?}") })?;
            let cd_layout = MatrixLayout::new(CuDType::R32F, n as u64, m as u64, n as i64)
                .map_err(|e| DrvErr::Precondition { op: "MatrixLayout::new(C/D)", msg: format!("{e:?}") })?;
            let mut pref = Preference::new().map_err(|e| DrvErr::Precondition {
                op: "Preference::new",
                msg: format!("{e:?}"),
            })?;
            pref.set_max_workspace(WS_BYTES).map_err(|e| DrvErr::Precondition {
                op: "Preference::set_max_workspace",
                msg: format!("{e:?}"),
            })?;
            let algo = blas::heuristic(
                &handle, &desc, &a_layout, &b_layout, &cd_layout, &cd_layout, &pref,
            )
            .map_err(|e| DrvErr::Precondition { op: "blas::heuristic", msg: format!("{e:?}") })?;
            let alpha: f32 = 1.0;
            let beta: f32 = 0.0;
            unsafe {
                blas::matmul(
                    &handle,
                    &desc,
                    &alpha.to_ne_bytes(),
                    &beta.to_ne_bytes(),
                    b_dev.device_ptr(),
                    &a_layout,
                    a_dev.device_ptr(),
                    &b_layout,
                    d_dev.device_ptr(),
                    &cd_layout,
                    d_dev.device_ptr(),
                    &cd_layout,
                    Some(&algo),
                    Some(&mut ws),
                    &stream,
                )
                .map_err(|e| DrvErr::Precondition { op: "blas::matmul", msg: format!("{e:?}") })?;
            }
            Ok(d_dev)
        };
        let d = mk().map_err(|e| {
            RmiError::compute_simple(format!("CUDA half matmul {m}x{k}@{k}x{n}: {e:?}"))
        })?;
        self.gpu_store_f32(d, &[m, n])
    }

    /// P109: batched F32 matmul. `a` is `[..., M, K]`, `b` is
    /// `[..., K, N]` with identical leading (batch) dims; result is
    /// `[..., M, N]`. All batches share one cuBLASLt strided-batched
    /// call. Leading dims are flattened to a single batch count `B`.
    ///
    /// **Why a new path**: `CpuBackend::matmul` only handles rank-2 (it
    /// errors on ND), so before this there was *no* batched matmul at
    /// all — the trait method bounced to CPU which then failed. This is
    /// net-new capability, GPU-only.
    fn matmul_batched_f32_gpu(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        let ar = a.shape.len();
        let br = b.shape.len();
        let m = a.shape[ar - 2];
        let k = a.shape[ar - 1];
        let k2 = b.shape[br - 2];
        let n = b.shape[br - 1];
        if k != k2 {
            return Err(RmiError::compute_simple(format!(
                "batched matmul inner mismatch: {:?} x {:?}",
                a.shape, b.shape
            )));
        }
        let a_batch: usize = a.shape[..ar - 2].iter().product();
        let b_batch: usize = b.shape[..br - 2].iter().product();
        if a_batch != b_batch || a.shape[..ar - 2] != b.shape[..br - 2] {
            return Err(RmiError::compute_simple(format!(
                "batched matmul batch mismatch: {:?} x {:?}",
                a.shape, b.shape
            )));
        }
        let batch = a_batch;
        let a_dev_arc = self.gpu_get_f32(a)?;
        let b_dev_arc = self.gpu_get_f32(b)?;
        let d_dev = self
            .matmul_lt_f32_gpu(&a_dev_arc, &b_dev_arc, m, k, n, batch)
            .map_err(|e| {
                RmiError::compute_simple(format!(
                    "CUDA batched matmul b={batch} {m}x{k}@{k}x{n}: {e:?}"
                ))
            })?;
        let mut out_shape = a.shape[..ar - 2].to_vec();
        out_shape.push(m);
        out_shape.push(n);
        self.gpu_store_f32(d_dev, &out_shape)
    }

    /// Core cuBLASLt invocation shared by the 2-D and batched paths.
    /// `a_dev` is our A `[batch, M, K]` row-major contiguous, `b_dev`
    /// our B `[batch, K, N]`; returns D `[batch, M, N]` on the GPU.
    /// `batch == 1` is the plain 2-D case (no batch attribute set).
    ///
    /// Row-major-via-col-major trick (per batch slice): cuBLAS computes
    /// column-major, so to get row-major `D = A·B` we compute
    /// `D^T = B^T·A^T`:
    ///   - cuBLAS A operand = our B, layout (N, K, ld=N), stride K·N
    ///   - cuBLAS B operand = our A, layout (K, M, ld=K), stride M·K
    ///   - cuBLAS C/D operand = our D, layout (N, M, ld=N), stride M·N
    fn matmul_lt_f32_gpu(
        &self,
        a_dev: &DeviceBuf<f32>,
        b_dev: &DeviceBuf<f32>,
        m: usize,
        k: usize,
        n: usize,
        batch: usize,
    ) -> std::result::Result<DeviceBuf<f32>, ironaccelerator_cuda::drv::Error> {
        use ironaccelerator_cuda::drv::Error as DrvErr;
        let stream = self.device.default_stream();
        let d_dev = DeviceBuf::<f32>::alloc(stream.clone(), batch * m * n)?;
        // Workspace: 4 MiB is comfortably above any reasonable
        // heuristic for these op sizes on Ampere.
        const WS_BYTES: usize = 4 * 1024 * 1024;
        let mut ws = DeviceBuf::<u8>::alloc(stream.clone(), WS_BYTES)?;

        let handle = blas::handle_for(&stream).map_err(|e| DrvErr::Precondition {
            op: "BlasLt::handle_for",
            msg: format!("{e:?}"),
        })?;

        let mut desc = MatmulDesc::new(ComputeType::F32, CuDType::R32F).map_err(|e| {
            DrvErr::Precondition { op: "MatmulDesc::new", msg: format!("{e:?}") }
        })?;
        desc.set_transpose(Op::N, Op::N).map_err(|e| DrvErr::Precondition {
            op: "MatmulDesc::set_transpose",
            msg: format!("{e:?}"),
        })?;

        // cuBLAS A operand = our B [K,N] row-major, viewed col-major
        // [N,K] ld=N. Per-batch stride in elements = K·N.
        let mut a_layout = MatrixLayout::new(CuDType::R32F, n as u64, k as u64, n as i64)
            .map_err(|e| DrvErr::Precondition { op: "MatrixLayout::new(A)", msg: format!("{e:?}") })?;
        // cuBLAS B operand = our A [M,K] row-major, viewed col-major
        // [K,M] ld=K. Per-batch stride = M·K.
        let mut b_layout = MatrixLayout::new(CuDType::R32F, k as u64, m as u64, k as i64)
            .map_err(|e| DrvErr::Precondition { op: "MatrixLayout::new(B)", msg: format!("{e:?}") })?;
        // cuBLAS C/D operand = our D [M,N] row-major, viewed col-major
        // [N,M] ld=N. Per-batch stride = M·N.
        let mut cd_layout = MatrixLayout::new(CuDType::R32F, n as u64, m as u64, n as i64)
            .map_err(|e| DrvErr::Precondition { op: "MatrixLayout::new(C/D)", msg: format!("{e:?}") })?;

        if batch > 1 {
            let bc = batch as i32;
            a_layout.set_batch(bc, (k * n) as i64).map_err(|e| DrvErr::Precondition {
                op: "MatrixLayout::set_batch(A)",
                msg: format!("{e:?}"),
            })?;
            b_layout.set_batch(bc, (m * k) as i64).map_err(|e| DrvErr::Precondition {
                op: "MatrixLayout::set_batch(B)",
                msg: format!("{e:?}"),
            })?;
            cd_layout.set_batch(bc, (m * n) as i64).map_err(|e| DrvErr::Precondition {
                op: "MatrixLayout::set_batch(C/D)",
                msg: format!("{e:?}"),
            })?;
        }

        let mut pref = Preference::new().map_err(|e| DrvErr::Precondition {
            op: "Preference::new",
            msg: format!("{e:?}"),
        })?;
        pref.set_max_workspace(WS_BYTES).map_err(|e| DrvErr::Precondition {
            op: "Preference::set_max_workspace",
            msg: format!("{e:?}"),
        })?;

        let algo = blas::heuristic(
            &handle, &desc, &a_layout, &b_layout, &cd_layout, &cd_layout, &pref,
        )
        .map_err(|e| DrvErr::Precondition { op: "blas::heuristic", msg: format!("{e:?}") })?;

        let alpha: f32 = 1.0;
        let beta: f32 = 0.0;
        unsafe {
            blas::matmul(
                &handle,
                &desc,
                &alpha.to_ne_bytes(),
                &beta.to_ne_bytes(),
                b_dev.device_ptr(),
                &a_layout,
                a_dev.device_ptr(),
                &b_layout,
                d_dev.device_ptr(),
                &cd_layout,
                d_dev.device_ptr(),
                &cd_layout,
                Some(&algo),
                Some(&mut ws),
                &stream,
            )
            .map_err(|e| DrvErr::Precondition { op: "blas::matmul", msg: format!("{e:?}") })?;
        }
        // Result stays on the GPU — no sync, no dtoh.
        Ok(d_dev)
    }

    // ─── P104: NVRTC kernels (elementwise + activations) ─────────────

    /// CUDA C++ source for all P104 f32 kernels — compiled once per
    /// `CudaBackend` instance on first use.
    ///
    /// GELU uses the tanh approximation that matches CpuBackend's
    /// `(sqrt(2/π) * (x + 0.044715 x³)).tanh()` form so GPU and CPU
    /// results agree within f32 rounding.
    const KERNEL_SRC: &'static str = r#"
extern "C" {

__global__ void k_add_f32(const float* a, const float* b, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = a[i] + b[i];
}
__global__ void k_sub_f32(const float* a, const float* b, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = a[i] - b[i];
}
__global__ void k_mul_f32(const float* a, const float* b, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = a[i] * b[i];
}
__global__ void k_div_f32(const float* a, const float* b, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = a[i] / b[i];
}
__global__ void k_scale_f32(const float* a, float s, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = a[i] * s;
}
__global__ void k_relu_f32(const float* a, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) { float x = a[i]; out[i] = x > 0.f ? x : 0.f; }
}
__global__ void k_sigmoid_f32(const float* a, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = 1.f / (1.f + expf(-a[i]));
}
__global__ void k_tanh_f32(const float* a, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = tanhf(a[i]);
}
__global__ void k_gelu_f32(const float* a, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        float x = a[i];
        // sqrtf(2.f / Pi) = 0.7978845608028654f
        float inner = 0.7978845608028654f * (x + 0.044715f * x * x * x);
        out[i] = 0.5f * x * (1.f + tanhf(inner));
    }
}

// ── P105: scalar reductions (single block, strided accumulate, classic
// shared-mem pairwise reduce). Launch with 1 block × 256 threads. Works
// for any n — slower than multi-block + secondary reduce for huge n but
// correct everywhere.

#define BS 256

__global__ void k_sum_f32(const float* x, float* out, int n) {
    __shared__ float s[BS];
    int t = threadIdx.x;
    float v = 0.f;
    for (int i = t; i < n; i += BS) v += x[i];
    s[t] = v;
    __syncthreads();
    for (int off = BS >> 1; off > 0; off >>= 1) {
        if (t < off) s[t] += s[t + off];
        __syncthreads();
    }
    if (t == 0) *out = s[0];
}
__global__ void k_max_f32(const float* x, float* out, int n) {
    __shared__ float s[BS];
    int t = threadIdx.x;
    // Guard initial value: if no element falls to this thread we mustn't
    // contaminate the reduction with -inf for the actual max. n is always
    // ≥1 at the dispatcher (empty bounces to CPU).
    float v = (t < n) ? x[t] : x[0];
    for (int i = t + BS; i < n; i += BS) v = fmaxf(v, x[i]);
    s[t] = v;
    __syncthreads();
    for (int off = BS >> 1; off > 0; off >>= 1) {
        if (t < off) s[t] = fmaxf(s[t], s[t + off]);
        __syncthreads();
    }
    if (t == 0) *out = s[0];
}
__global__ void k_min_f32(const float* x, float* out, int n) {
    __shared__ float s[BS];
    int t = threadIdx.x;
    float v = (t < n) ? x[t] : x[0];
    for (int i = t + BS; i < n; i += BS) v = fminf(v, x[i]);
    s[t] = v;
    __syncthreads();
    for (int off = BS >> 1; off > 0; off >>= 1) {
        if (t < off) s[t] = fminf(s[t], s[t + off]);
        __syncthreads();
    }
    if (t == 0) *out = s[0];
}

// ── P105: softmax along the last (contiguous) axis. Launch with
// grid=outer, block=BS. Each block owns one row of length `inner`.
// 1D softmax is handled by setting outer=1, inner=n.

// ── P108: 2-D F32 transpose (axes = [1, 0]). 2-D tile kernel would
// give better memory coalescing but element-per-thread is correct and
// easy to validate. Input shape [M, K] row-major → output shape [K, M].
//
// Each thread writes one output element at (j, i) by reading input
// (i, j). Launch grid covers KxM with 16x16 blocks.

__global__ void k_transpose_2d_f32(const float* in, float* out, int m, int k) {
    int j = blockIdx.x * blockDim.x + threadIdx.x; // output col (0..m)
    int i = blockIdx.y * blockDim.y + threadIdx.y; // output row (0..k)
    if (i < k && j < m) {
        out[i * m + j] = in[j * k + i];
    }
}

// ── P111: arbitrary-rank transpose / permute. For each output linear
// index, decompose into output coords via `out_dims`, then accumulate
// the input linear index using per-output-axis multipliers
// `mult[k] = in_stride[perm[k]]` (row-major input stride of the source
// axis that output axis k came from). One thread per output element.
__global__ void k_permute_f32(const float* in, float* out,
                              const int* out_dims, const int* mult,
                              int ndim, int total) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= total) return;
    int rem = idx;
    long in_idx = 0;
    for (int kk = ndim - 1; kk >= 0; kk--) {
        int d = out_dims[kk];
        int c = rem % d;
        rem /= d;
        in_idx += (long)c * (long)mult[kk];
    }
    out[idx] = in[in_idx];
}

// ── P107: per-row reductions along the last (contiguous) axis. Same
// launch layout as softmax: grid=outer, block=BS, each block reduces
// one row of length `inner`. Output is a single f32 per row.

__global__ void k_sum_axis_lastdim_f32(const float* x, float* out, int outer, int inner) {
    int row = blockIdx.x;
    if (row >= outer) return;
    const float* xr = x + (size_t)row * (size_t)inner;
    __shared__ float s[BS];
    int t = threadIdx.x;
    float v = 0.f;
    for (int i = t; i < inner; i += BS) v += xr[i];
    s[t] = v;
    __syncthreads();
    for (int off = BS >> 1; off > 0; off >>= 1) {
        if (t < off) s[t] += s[t + off];
        __syncthreads();
    }
    if (t == 0) out[row] = s[0];
}

__global__ void k_softmax_lastdim_f32(const float* x, float* out, int outer, int inner) {
    int row = blockIdx.x;
    if (row >= outer) return;
    const float* xr = x + (size_t)row * (size_t)inner;
    float* outr = out + (size_t)row * (size_t)inner;
    __shared__ float s[BS];
    int t = threadIdx.x;

    // 1. row max
    float m = (t < inner) ? xr[t] : xr[0];
    for (int i = t + BS; i < inner; i += BS) m = fmaxf(m, xr[i]);
    s[t] = m;
    __syncthreads();
    for (int off = BS >> 1; off > 0; off >>= 1) {
        if (t < off) s[t] = fmaxf(s[t], s[t + off]);
        __syncthreads();
    }
    float row_max = s[0];
    __syncthreads();

    // 2. exp(x - max) → outr, accumulate sum
    float sum = 0.f;
    for (int i = t; i < inner; i += BS) {
        float e = expf(xr[i] - row_max);
        outr[i] = e;
        sum += e;
    }
    s[t] = sum;
    __syncthreads();
    for (int off = BS >> 1; off > 0; off >>= 1) {
        if (t < off) s[t] += s[t + off];
        __syncthreads();
    }
    float row_sum = s[0];

    // 3. normalize
    float inv = 1.f / row_sum;
    for (int i = t; i < inner; i += BS) outr[i] *= inv;
}

// ── P110: arbitrary-axis reduction / softmax over a [outer, axis, inner]
// view. Element (o, j, i) lives at ((o*axis + j)*inner + i). One thread
// owns one (o, i) output lane and walks the `axis` dimension serially
// (strided by `inner`). Subsumes the last-axis kernels (inner == 1) but
// we keep those for the hot last-axis path. Grid over outer*inner.

__global__ void k_sum_axis_f32(const float* x, float* out, int outer, int axis, int inner) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = outer * inner;
    if (idx >= total) return;
    int o = idx / inner;
    int i = idx % inner;
    size_t base = (size_t)o * (size_t)axis * (size_t)inner + (size_t)i;
    float v = 0.f;
    for (int j = 0; j < axis; j++) v += x[base + (size_t)j * (size_t)inner];
    out[idx] = v;
}

__global__ void k_softmax_axis_f32(const float* x, float* out, int outer, int axis, int inner) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = outer * inner;
    if (idx >= total) return;
    int o = idx / inner;
    int i = idx % inner;
    size_t base = (size_t)o * (size_t)axis * (size_t)inner + (size_t)i;
    // 1. max over axis
    float m = x[base];
    for (int j = 1; j < axis; j++) m = fmaxf(m, x[base + (size_t)j * (size_t)inner]);
    // 2. exp(x - max) → out, accumulate sum
    float sum = 0.f;
    for (int j = 0; j < axis; j++) {
        size_t p = base + (size_t)j * (size_t)inner;
        float e = expf(x[p] - m);
        out[p] = e;
        sum += e;
    }
    // 3. normalize
    float inv = 1.f / sum;
    for (int j = 0; j < axis; j++) out[base + (size_t)j * (size_t)inner] *= inv;
}

// ── P113: im2col for conv2d-as-GEMM. Lays out the [M, K] column matrix
// where M = N*Hout*Wout (one row per output pixel across the batch) and
// K = Cin*KH*KW. `col[m*K + k]` is the input value feeding output pixel
// m through filter tap k, with zero-padding for out-of-bounds. One
// thread per (m, k) entry.
__global__ void k_im2col_f32(const float* x, float* col,
                            int N, int Cin, int H, int W,
                            int KH, int KW, int Hout, int Wout,
                            int stride, int pad, int dilation, int M, int K) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= M * K) return;
    int m = idx / K;
    int k = idx % K;
    // decode output pixel m → (n, ho, wo)
    int wo = m % Wout;
    int t  = m / Wout;
    int ho = t % Hout;
    int n  = t / Hout;
    // decode filter tap k → (ci, ky, kx)
    int kx = k % KW;
    int t2 = k / KW;
    int ky = t2 % KH;
    int ci = t2 / KH;
    int hi = ho * stride + ky * dilation - pad;
    int wi = wo * stride + kx * dilation - pad;
    float v = 0.f;
    if (hi >= 0 && hi < H && wi >= 0 && wi < W) {
        v = x[(((size_t)n * Cin + ci) * H + hi) * W + wi];
    }
    col[idx] = v;
}

// ── P114: concat / split as strided slab copies. Both view tensors as
// [outer, axis, inner]. `k_concat_copy` writes a small src slab into a
// big dst at axis-offset `off`; `k_split_copy` reads a slab out of a
// big src into a small dst. One thread per SMALL-tensor element.
__global__ void k_concat_copy_f32(const float* src, float* dst,
                                 int outer, int ax_src, int inner,
                                 int ax_dst, int off) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = outer * ax_src * inner;
    if (idx >= total) return;
    int ii = idx % inner;
    int t  = idx / inner;
    int a  = t % ax_src;
    int o  = t / ax_src;
    size_t dst_idx = ((size_t)o * ax_dst + (off + a)) * (size_t)inner + ii;
    dst[dst_idx] = src[idx];
}

__global__ void k_split_copy_f32(const float* src, float* dst,
                                int outer, int ax_dst, int inner,
                                int ax_src, int off) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = outer * ax_dst * inner;
    if (idx >= total) return;
    int ii = idx % inner;
    int t  = idx / inner;
    int a  = t % ax_dst;
    int o  = t / ax_dst;
    size_t src_idx = ((size_t)o * ax_src + (off + a)) * (size_t)inner + ii;
    dst[idx] = src[src_idx];
}

// ── P121: INT8 quantization. Symmetric per-tensor: q = round(x/scale)
// clamped to [-127,127]; dequant = q*scale. The quantized GEMM
// accumulates in int32 and folds the combined scale (sa*sb) into the
// f32 output. `signed char` is CUDA's int8.
__global__ void k_quantize_i8(const float* in, signed char* out, float inv_scale, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        float q = roundf(in[i] * inv_scale);
        q = fminf(fmaxf(q, -127.f), 127.f);
        out[i] = (signed char)q;
    }
}
__global__ void k_dequantize_i8(const signed char* in, float* out, float scale, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = (float)in[i] * scale;
}
// P127: parallel amax (max|x|). Stage 1: each block grid-strides over a
// slice, reduces to one partial (already non-negative). Reduce the
// partials with the existing single-block k_max_f32 (partials count is
// small). Replaces the prior two full single-block min+max reductions.
__global__ void k_amax_partial_f32(const float* x, float* partials, int n) {
    __shared__ float s[BS];
    int t = threadIdx.x;
    float v = 0.f;
    for (int i = blockIdx.x * BS + t; i < n; i += BS * gridDim.x) {
        v = fmaxf(v, fabsf(x[i]));
    }
    s[t] = v;
    __syncthreads();
    for (int off = BS >> 1; off > 0; off >>= 1) {
        if (t < off) s[t] = fmaxf(s[t], s[t + off]);
        __syncthreads();
    }
    if (t == 0) partials[blockIdx.x] = s[0];
}
// Row-major A[M,K] · B[K,N] → C[M,N], int32 accumulate, scaled to f32.
// One thread per output element.
__global__ void k_matmul_i8_deq(const signed char* A, const signed char* B,
                               float* C, int M, int K, int N, float scale) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = M * N;
    if (idx >= total) return;
    int r = idx / N;
    int c = idx % N;
    int acc = 0;
    const signed char* arow = A + (size_t)r * K;
    for (int k = 0; k < K; k++) acc += (int)arow[k] * (int)B[(size_t)k * N + c];
    C[idx] = (float)acc * scale;
}

// ── P122: per-channel (per-column) INT8 quantization. Each column n of
// a [K,N] matrix uses its own inv_scale[n]. Much lower error than
// per-tensor when columns differ in magnitude (the standard for weights).
__global__ void k_quantize_i8_pc(const float* in, signed char* out,
                                const float* inv_scales, int K, int N) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = K * N;
    if (idx >= total) return;
    int col = idx % N;
    float q = roundf(in[idx] * inv_scales[col]);
    q = fminf(fmaxf(q, -127.f), 127.f);
    out[idx] = (signed char)q;
}
// Quantized GEMM with per-column dequant: out[m,n] = acc * sa * sb[n].
__global__ void k_matmul_i8_deq_pc(const signed char* A, const signed char* B,
                                  float* C, int M, int K, int N,
                                  float sa, const float* sb) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = M * N;
    if (idx >= total) return;
    int r = idx / N;
    int c = idx % N;
    int acc = 0;
    const signed char* arow = A + (size_t)r * K;
    for (int k = 0; k < K; k++) acc += (int)arow[k] * (int)B[(size_t)k * N + c];
    C[idx] = (float)acc * sa * sb[c];
}

// ── P126: cuBLASLt IMMA (tensor-core INT8) helpers. The IMMA TN path
// needs the weight K-major, so quantize it transposed to [N,K]; and the
// INT32 result comes back col-major[M,N], so dequant transposes it back
// to row-major while applying the per-column scale.
__global__ void k_quantize_i8_pc_t(const float* in, signed char* out,
                                  const float* inv_scales, int K, int N) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = K * N;
    if (idx >= total) return;
    int k = idx / N;
    int n = idx % N;
    float q = roundf(in[idx] * inv_scales[n]);
    q = fminf(fmaxf(q, -127.f), 127.f);
    out[(size_t)n * K + k] = (signed char)q;   // [N,K] layout
}
// C is col-major[M,N] (buffer[m + n*M]); write row-major out[M,N].
__global__ void k_dequant_i32_pc_t(const int* C, float* out, int M, int N,
                                  float sa, const float* sb) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = M * N;
    if (idx >= total) return;
    int m = idx / N;
    int n = idx % N;
    out[idx] = (float)C[(size_t)m + (size_t)n * M] * sa * sb[n];
}
// P134: asymmetric dequant for the IMMA path. col-major[M,N] int32 C,
// minus the zero-point correction za·wsum[n], ×sa×sb[n] → row-major.
__global__ void k_dequant_i32_pc_t_asym(const int* C, const int* wsum,
                                       float* out, int M, int N,
                                       float sa, const float* sb, int za) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = M * N;
    if (idx >= total) return;
    int m = idx / N;
    int n = idx % N;
    int acc = C[(size_t)m + (size_t)n * M];
    out[idx] = sa * sb[n] * (float)(acc - za * wsum[n]);
}
// P134: row sums of bq_t[N,K] (int8) → int32[N]. Row n = logical weight
// column n (the IMMA weight is stored transposed), so this is the
// per-output-column weight sum the zero-point correction needs.
__global__ void k_rowsum_i8(const signed char* B, int* out, int N, int K) {
    int n = blockIdx.x * blockDim.x + threadIdx.x;
    if (n >= N) return;
    int s = 0;
    const signed char* row = B + (size_t)n * K;
    for (int k = 0; k < K; k++) s += (int)row[k];
    out[n] = s;
}

// ── P131: asymmetric (zero-point) INT8 for ACTIVATIONS. q = round(x/sa)
// + za, clamped to signed-int8 [-128,127]; dequant x ≈ sa*(q - za). With
// symmetric weights (zb=0): A·B = sa*sb·(Σqa·qb − za·Σqb). The Σqb term
// is a per-output-column weight sum, computed exactly on-device once.
__global__ void k_quantize_i8_asym(const float* in, signed char* out,
                                  float inv_scale, int za, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        int q = (int)roundf(in[i] * inv_scale) + za;
        q = max(-128, min(127, q));
        out[i] = (signed char)q;
    }
}
// Column sums of B[K,N] (int8) → int32[N]. One thread per column.
__global__ void k_colsum_i8(const signed char* B, int* out, int K, int N) {
    int n = blockIdx.x * blockDim.x + threadIdx.x;
    if (n >= N) return;
    int s = 0;
    for (int k = 0; k < K; k++) s += (int)B[(size_t)k * N + n];
    out[n] = s;
}
// Asymmetric-activation GEMM: A[M,K] qa (zero-point za), B[K,N] qb
// (symmetric, per-col scale sb), colsum_b[N] = Σ_k qb. Dequant per col.
__global__ void k_matmul_i8_asym(const signed char* A, const signed char* B,
                                const int* colsum_b, float* C,
                                int M, int K, int N, float sa,
                                const float* sb, int za) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = M * N;
    if (idx >= total) return;
    int r = idx / N;
    int c = idx % N;
    int acc = 0;
    const signed char* arow = A + (size_t)r * K;
    for (int k = 0; k < K; k++) acc += (int)arow[k] * (int)B[(size_t)k * N + c];
    C[idx] = sa * sb[c] * (float)(acc - za * colsum_b[c]);
}

// ── P135: INT4 (4-bit) symmetric quantization, packed 2 values/byte.
// q = clamp(round(x/scale), -7, 7) stored as a signed nibble; two
// consecutive logical elements share a byte (low nibble = even index,
// high nibble = odd). Halves INT8 memory (8× vs F32). `n` is the logical
// element count; the packed buffer has ceil(n/2) bytes.
__device__ __forceinline__ int unpack_nib(unsigned char byte, int hi) {
    int v = hi ? (byte >> 4) : (byte & 0xF);
    if (v & 0x8) v -= 16;  // sign-extend 4-bit → int
    return v;
}
__global__ void k_quantize_i4(const float* in, unsigned char* out,
                             float inv_scale, int n) {
    int p = blockIdx.x * blockDim.x + threadIdx.x; // byte index
    int npacked = (n + 1) / 2;
    if (p >= npacked) return;
    int i0 = 2 * p, i1 = 2 * p + 1;
    int q0 = (int)roundf(in[i0] * inv_scale);
    q0 = max(-7, min(7, q0));
    int q1 = 0;
    if (i1 < n) { q1 = (int)roundf(in[i1] * inv_scale); q1 = max(-7, min(7, q1)); }
    out[p] = (unsigned char)((q0 & 0xF) | ((q1 & 0xF) << 4));
}
__global__ void k_dequantize_i4(const unsigned char* in, float* out,
                               float scale, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= n) return;
    unsigned char byte = in[i >> 1];
    out[i] = (float)unpack_nib(byte, i & 1) * scale;
}
// INT4 × INT4 matmul, both packed [.,K] along K (K even). Unpacks two
// nibbles per byte inline. A is [M,K] packed (K/2 bytes/row), B is [K,N]
// packed column-major-ish: to keep it simple B is [K,N] with each (k,n)
// nibble; we store B packed along K so B has (K/2) byte-rows of N? — to
// avoid ambiguity P135 packs ONLY along the contiguous last dim. Here we
// pass B already unpacked-per-element is wasteful; instead pack A along K
// and keep B packed along K too with stride N in NIBBLES. We use a
// byte-row layout: B_packed[(k/2)*N + n] holds nibble (k even/odd) — NO,
// that mixes. For correctness+simplicity, P135 GEMM unpacks A only and
// takes B as int8. (Weights in int8, activations in int4 is a common
// W8A4-style mix; here we do A:int4 × B:int8.)
__global__ void k_matmul_i4a_i8b(const unsigned char* Apk, const signed char* B,
                                float* C, int M, int K, int N,
                                float sa, const float* sb) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = M * N;
    if (idx >= total) return;
    int r = idx / N;
    int c = idx % N;
    int acc = 0;
    const unsigned char* arow = Apk + (size_t)r * ((K + 1) / 2);
    for (int k = 0; k < K; k++) {
        int qa = unpack_nib(arow[k >> 1], k & 1);
        acc += qa * (int)B[(size_t)k * N + c];
    }
    C[idx] = sa * sb[c] * (float)acc;
}

// ── P137: INT4 WEIGHTS. Per-channel quantize of W[K,N] to packed
// nibbles at flat row-major index (element (k,n) → nibble k*N+n).
// Scale per column n. Range ±7. K*N may be odd → tail nibble zero.
__global__ void k_quantize_i4_pc(const float* in, unsigned char* out,
                                const float* inv_scales, int K, int N) {
    int p = blockIdx.x * blockDim.x + threadIdx.x; // byte index
    int total = K * N;
    int npacked = (total + 1) / 2;
    if (p >= npacked) return;
    int i0 = 2 * p, i1 = 2 * p + 1;
    int q0 = (int)roundf(in[i0] * inv_scales[i0 % N]);
    q0 = max(-7, min(7, q0));
    int q1 = 0;
    if (i1 < total) {
        q1 = (int)roundf(in[i1] * inv_scales[i1 % N]);
        q1 = max(-7, min(7, q1));
    }
    out[p] = (unsigned char)((q0 & 0xF) | ((q1 & 0xF) << 4));
}
// W4A8: int8 activation A[M,K] × packed-int4 weight W[K,N] (per-col
// scales sb). Unpacks W's nibble at flat (k*N+c) inline.
__global__ void k_matmul_i8a_i4b(const signed char* A, const unsigned char* Wpk,
                                float* C, int M, int K, int N,
                                float sa, const float* sb) {
    int idx = blockIdx.x * blockDim.x + threadIdx.x;
    int total = M * N;
    if (idx >= total) return;
    int r = idx / N;
    int c = idx % N;
    int acc = 0;
    const signed char* arow = A + (size_t)r * K;
    for (int k = 0; k < K; k++) {
        size_t flat = (size_t)k * N + c;
        int qw = unpack_nib(Wpk[flat >> 1], (int)(flat & 1));
        acc += (int)arow[k] * qw;
    }
    C[idx] = sa * sb[c] * (float)acc;
}

}
"#;

    /// Compile (once) and return the NVRTC module holding every P104 kernel.
    fn kernel_module(&self) -> Result<&Arc<Module>> {
        if let Some(m) = self.kernels.get() {
            return Ok(m);
        }
        let ptx = compile_ptx(Self::KERNEL_SRC).map_err(|e| {
            RmiError::compute_simple(format!("NVRTC compile failed: {e:?}"))
        })?;
        let module = Module::load(self.device.raw().clone(), &ptx).map_err(|e| {
            RmiError::compute_simple(format!("Module::load failed: {e:?}"))
        })?;
        // Race-tolerant set: if another thread won, drop ours and use theirs.
        let _ = self.kernels.set(module);
        Ok(self.kernels.get().expect("just initialized"))
    }

    // ─── P118: half-precision (F16/BF16) support ─────────────────────

    /// CUDA C++ source for the half-precision conversion kernels.
    /// `#include`s the NVRTC built-in fp16/bf16 headers. Half tensors
    /// are stored as raw `unsigned short` bits and reinterpreted as
    /// `__half`/`__nv_bfloat16` inside the kernels.
    const HALF_KERNEL_SRC: &'static str = r#"
#include <cuda_fp16.h>
#include <cuda_bf16.h>
extern "C" {

__global__ void k_f32_to_f16(const float* in, unsigned short* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) ((__half*)out)[i] = __float2half(in[i]);
}
__global__ void k_f16_to_f32(const unsigned short* in, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = __half2float(((const __half*)in)[i]);
}
__global__ void k_f32_to_bf16(const float* in, unsigned short* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) ((__nv_bfloat16*)out)[i] = __float2bfloat16(in[i]);
}
__global__ void k_bf16_to_f32(const unsigned short* in, float* out, int n) {
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = __bfloat162float(((const __nv_bfloat16*)in)[i]);
}

}
"#;

    /// Compile (once) and return the NVRTC module holding the half
    /// conversion kernels. Separate from `kernel_module` so an fp16
    /// header issue can't break the F32 path.
    fn half_kernel_module(&self) -> Result<&Arc<Module>> {
        if let Some(m) = self.half_kernels.get() {
            return Ok(m);
        }
        let ptx = compile_ptx(Self::HALF_KERNEL_SRC).map_err(|e| {
            RmiError::compute_simple(format!("NVRTC half compile failed: {e:?}"))
        })?;
        let module = Module::load(self.device.raw().clone(), &ptx).map_err(|e| {
            RmiError::compute_simple(format!("half Module::load failed: {e:?}"))
        })?;
        let _ = self.half_kernels.set(module);
        Ok(self.half_kernels.get().expect("just initialized"))
    }

    /// Store a half-precision buffer under a fresh id, returning a Cuda
    /// handle tagged with `dtype` (must be F16 or BF16).
    fn half_store(
        &self,
        buf: DeviceBuf<u16>,
        shape: &[usize],
        dtype: DType,
    ) -> Result<TensorHandle> {
        let id = self.fresh_id();
        let numel: usize = shape.iter().product();
        let handle = TensorHandle {
            id,
            shape: shape.to_vec(),
            dtype,
            backend: BackendType::Cuda,
            size_bytes: numel * 2,
        };
        self.gpu_storage_u16
            .write()
            .expect("gpu_storage_u16 poisoned")
            .insert(id, Arc::new(buf));
        Ok(handle)
    }

    /// Look up a half-precision buffer by handle (must be Cuda + F16/BF16).
    fn half_get(&self, h: &TensorHandle) -> Result<Arc<DeviceBuf<u16>>> {
        if !matches!(h.dtype, DType::F16 | DType::BF16) {
            return Err(RmiError::compute_simple(format!(
                "half_get: handle is {:?}, not F16/BF16",
                h.dtype
            )));
        }
        if !matches!(h.backend, BackendType::Cuda) {
            return Err(RmiError::compute_simple("half_get: handle not Cuda-backed"));
        }
        self.gpu_storage_u16
            .read()
            .expect("gpu_storage_u16 poisoned")
            .get(&h.id)
            .cloned()
            .ok_or_else(|| {
                RmiError::compute_simple(format!("half_get: id {} not in half storage", h.id))
            })
    }

    /// Convert an F32 GPU tensor to F16 or BF16 (GPU-resident result).
    fn to_half_gpu(&self, a: &TensorHandle, dtype: DType) -> Result<TensorHandle> {
        let kernel = match dtype {
            DType::F16 => "k_f32_to_f16",
            DType::BF16 => "k_f32_to_bf16",
            _ => return Err(RmiError::compute_simple("to_half_gpu: dtype not F16/BF16")),
        };
        let n = a.numel();
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.half_kernel_module()?;
        let func = module
            .function(kernel)
            .map_err(|e| RmiError::compute_simple(format!("function `{kernel}`: {e:?}")))?;
        let mut out_dev = DeviceBuf::<u16>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc half out: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &func, n as u32, 256, (&*in_dev, &mut out_dev, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch {kernel}: {e:?}")))?;
        }
        self.half_store(out_dev, &a.shape, dtype)
    }

    /// Convert an F16/BF16 GPU tensor back to F32 (GPU-resident result).
    fn half_to_f32_gpu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let kernel = match a.dtype {
            DType::F16 => "k_f16_to_f32",
            DType::BF16 => "k_bf16_to_f32",
            _ => return Err(RmiError::compute_simple("half_to_f32_gpu: dtype not F16/BF16")),
        };
        let n = a.numel();
        let in_dev = self.half_get(a)?;
        let stream = self.device.default_stream();
        let module = self.half_kernel_module()?;
        let func = module
            .function(kernel)
            .map_err(|e| RmiError::compute_simple(format!("function `{kernel}`: {e:?}")))?;
        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc f32 out: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &func, n as u32, 256, (&*in_dev, &mut out_dev, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch {kernel}: {e:?}")))?;
        }
        self.gpu_store_f32(out_dev, &a.shape)
    }

    // ─── P121: INT8 quantization ─────────────────────────────────────

    /// Store an INT8 buffer under a fresh id (dtype I8).
    fn i8_store(&self, buf: DeviceBuf<i8>, shape: &[usize]) -> Result<TensorHandle> {
        let id = self.fresh_id();
        let numel: usize = shape.iter().product();
        let handle = TensorHandle {
            id,
            shape: shape.to_vec(),
            dtype: DType::I8,
            backend: BackendType::Cuda,
            size_bytes: numel,
        };
        self.gpu_storage_i8
            .write()
            .expect("gpu_storage_i8 poisoned")
            .insert(id, Arc::new(buf));
        Ok(handle)
    }

    /// Look up an INT8 buffer by handle.
    fn i8_get(&self, h: &TensorHandle) -> Result<Arc<DeviceBuf<i8>>> {
        if h.dtype != DType::I8 {
            return Err(RmiError::compute_simple(format!(
                "i8_get: handle is {:?}, not I8",
                h.dtype
            )));
        }
        self.gpu_storage_i8
            .read()
            .expect("gpu_storage_i8 poisoned")
            .get(&h.id)
            .cloned()
            .ok_or_else(|| RmiError::compute_simple(format!("i8_get: id {} not in i8 storage", h.id)))
    }

    /// Quantize an F32 tensor to symmetric per-tensor INT8. Returns the
    /// I8 handle and the scale `s` such that `x ≈ q * s`. The scale is
    /// `amax/127` (amax = max|x|), computed on-GPU via max/min reductions.
    pub fn quantize_i8(&self, a: &TensorHandle) -> Result<(TensorHandle, f32)> {
        let n = a.numel();
        if a.dtype != DType::F32 {
            return Err(RmiError::compute_simple("quantize_i8: input must be F32"));
        }
        let scale = if n == 0 {
            1.0
        } else {
            // P127: single parallel amax pass (was two full single-block
            // min+max scans — the activation-quant bottleneck the IMMA
            // benchmark exposed).
            let amax = self.amax_f32_gpu(a)?;
            if amax > 0.0 { amax / 127.0 } else { 1.0 }
        };
        let inv_scale = 1.0f32 / scale;
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_quantize_i8")
            .map_err(|e| RmiError::compute_simple(format!("function `k_quantize_i8`: {e:?}")))?;
        let mut out = DeviceBuf::<i8>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc i8: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &func, n as u32, 256, (&*in_dev, &mut out, inv_scale, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch k_quantize_i8: {e:?}")))?;
        }
        Ok((self.i8_store(out, &a.shape)?, scale))
    }

    /// P128: quantize to INT8 using a GIVEN (calibrated) per-tensor
    /// scale — no amax reduction, no host sync. The whole quantize is a
    /// single elementwise kernel, so a calibrated activation quant adds
    /// no synchronization to the IMMA path.
    pub fn quantize_i8_with_scale(&self, a: &TensorHandle, scale: f32) -> Result<TensorHandle> {
        if a.dtype != DType::F32 {
            return Err(RmiError::compute_simple("quantize_i8_with_scale: input must be F32"));
        }
        let n = a.numel();
        let inv_scale = if scale > 0.0 { 1.0 / scale } else { 1.0 };
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_quantize_i8")
            .map_err(|e| RmiError::compute_simple(format!("function `k_quantize_i8`: {e:?}")))?;
        let mut out = DeviceBuf::<i8>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc i8: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &func, n as u32, 256, (&*in_dev, &mut out, inv_scale, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch k_quantize_i8: {e:?}")))?;
        }
        self.i8_store(out, &a.shape)
    }

    /// P128: calibrated INT8 matmul — `a [M,K]` quantized with a known
    /// activation scale `sa` (no reduction/sync), `b [K,N]` weight
    /// per-channel quantized + cached, IMMA tensor-core GEMM. This is the
    /// fully-on-device INT8 path: with a calibrated `sa`, nothing syncs
    /// to host between quantize and result, so the IMMA speedup carries
    /// end-to-end. Dims must be mult of 4; else exact F32 fallback.
    pub fn quantized_matmul_calibrated_impl(
        &self,
        a: &TensorHandle,
        sa: f32,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        if a.dtype != DType::F32 || b.dtype != DType::F32 || a.shape.len() != 2 || b.shape.len() != 2
        {
            return self.matmul(a, b);
        }
        let (m, k, n) = (a.shape[0], a.shape[1], b.shape[1]);
        if m % 4 != 0 || k % 4 != 0 || n % 4 != 0 {
            return self.matmul(a, b);
        }
        let result = (|| -> Result<TensorHandle> {
            let aq = self.quantize_i8_with_scale(a, sa)?;
            let cached = self
                .quant_weight_cache
                .read()
                .expect("quant_weight_cache poisoned")
                .get(&b.id)
                .cloned();
            let (bq_t, sb) = match cached {
                Some((h, s)) => {
                    self.quant_cache_hits.fetch_add(1, Ordering::Relaxed);
                    (h, s)
                }
                None => {
                    let (h, s) = self.quantize_i8_perchannel_t(b)?;
                    self.quant_weight_cache
                        .write()
                        .expect("quant_weight_cache poisoned")
                        .insert(b.id, (h.clone(), s.clone()));
                    (h, s)
                }
            };
            let out = self.matmul_i8_immma(&aq, sa, &bq_t, &sb)?;
            let _ = self.free(&aq);
            Ok(out)
        })();
        match result {
            Ok(h) => Ok(h),
            Err(_) => self.matmul(a, b),
        }
    }

    /// P131: asymmetric (zero-point) quantize an F32 activation tensor.
    /// Given a calibrated min/max range `[lo, hi]`, picks scale =
    /// (hi-lo)/255 and zero-point so the range maps onto int8 [-128,127].
    /// Returns (i8 handle, scale `sa`, zero-point `za`). Best for
    /// non-negative (post-ReLU) activations, which symmetric quant halves.
    pub fn quantize_i8_asym(
        &self,
        a: &TensorHandle,
        lo: f32,
        hi: f32,
    ) -> Result<(TensorHandle, f32, i32)> {
        if a.dtype != DType::F32 {
            return Err(RmiError::compute_simple("quantize_i8_asym: input must be F32"));
        }
        let n = a.numel();
        let range = (hi - lo).max(1e-12);
        let sa = range / 255.0;
        // q = round(x/sa) + za maps lo→-128: za = -128 - round(lo/sa).
        let za = (-128.0 - (lo / sa).round()) as i32;
        let inv_scale = 1.0 / sa;
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_quantize_i8_asym")
            .map_err(|e| RmiError::compute_simple(format!("function `k_quantize_i8_asym`: {e:?}")))?;
        let mut out = DeviceBuf::<i8>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc i8: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &func, n as u32, 256, (&*in_dev, &mut out, inv_scale, za, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch k_quantize_i8_asym: {e:?}")))?;
        }
        Ok((self.i8_store(out, &a.shape)?, sa, za))
    }

    /// P131: matmul of an asymmetric-quantized activation `aq [M,K]`
    /// (scale `sa`, zero-point `za`) against a symmetric per-channel
    /// weight `bq [K,N]` (scales `sb`). Applies the zero-point correction
    /// `−za·Σ_k qb[:,n]` exactly (column sums computed on-device), so the
    /// result equals the dequantized A·B up to rounding. Result is F32.
    pub fn matmul_i8_asym(
        &self,
        aq: &TensorHandle,
        sa: f32,
        za: i32,
        bq: &TensorHandle,
        sb: &[f32],
    ) -> Result<TensorHandle> {
        if aq.shape.len() != 2 || bq.shape.len() != 2 {
            return Err(RmiError::compute_simple("matmul_i8_asym: operands must be 2-D"));
        }
        let m = aq.shape[0];
        let k = aq.shape[1];
        let k2 = bq.shape[0];
        let n = bq.shape[1];
        if k != k2 {
            return Err(RmiError::compute_simple("matmul_i8_asym: K mismatch"));
        }
        if sb.len() != n {
            return Err(RmiError::compute_simple("matmul_i8_asym: scale count != N"));
        }
        let a_dev = self.i8_get(aq)?;
        let b_dev = self.i8_get(bq)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        // Column sums of B (int32[N]).
        let cs_fn = module
            .function("k_colsum_i8")
            .map_err(|e| RmiError::compute_simple(format!("function `k_colsum_i8`: {e:?}")))?;
        let mut colsum = DeviceBuf::<i32>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc colsum: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &cs_fn, n as u32, 256, (&*b_dev, &mut colsum, k as i32, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch k_colsum_i8: {e:?}")))?;
        }
        let sb_dev = DeviceBuf::<f32>::from_host(stream.clone(), sb)
            .map_err(|e| RmiError::compute_simple(format!("htod sb: {e:?}")))?;
        let gemm_fn = module
            .function("k_matmul_i8_asym")
            .map_err(|e| RmiError::compute_simple(format!("function `k_matmul_i8_asym`: {e:?}")))?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), (m * n).max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc out: {e:?}")))?;
        if m * n > 0 {
            launch_1d(
                &stream,
                &gemm_fn,
                (m * n) as u32,
                256,
                (&*a_dev, &*b_dev, &colsum, &mut out, m as i32, k as i32, n as i32, sa, &sb_dev, za),
            )
            .map_err(|e| RmiError::compute_simple(format!("launch k_matmul_i8_asym: {e:?}")))?;
        }
        self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
        self.gpu_store_f32(out, &[m, n])
    }

    /// Dequantize an INT8 tensor back to F32 using its `scale`.
    pub fn dequantize_i8(&self, a: &TensorHandle, scale: f32) -> Result<TensorHandle> {
        let n = a.numel();
        let in_dev = self.i8_get(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_dequantize_i8")
            .map_err(|e| RmiError::compute_simple(format!("function `k_dequantize_i8`: {e:?}")))?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc f32: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &func, n as u32, 256, (&*in_dev, &mut out, scale, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch k_dequantize_i8: {e:?}")))?;
        }
        self.gpu_store_f32(out, &a.shape)
    }

    // ─── P135: INT4 (4-bit, packed 2/byte) symmetric quantization ────

    /// Quantize an F32 tensor to symmetric per-tensor INT4, packed two
    /// nibbles per byte. Returns the I4 handle (packed bytes stored in
    /// the i8 map; `dtype = I4`, `shape` = logical) and scale = amax/7.
    /// 8× smaller than F32, 2× smaller than INT8.
    pub fn quantize_i4(&self, a: &TensorHandle) -> Result<(TensorHandle, f32)> {
        if a.dtype != DType::F32 {
            return Err(RmiError::compute_simple("quantize_i4: input must be F32"));
        }
        let n = a.numel();
        let scale = if n == 0 {
            1.0
        } else {
            let amax = self.amax_f32_gpu(a)?;
            if amax > 0.0 { amax / 7.0 } else { 1.0 }
        };
        let inv_scale = 1.0f32 / scale;
        let npacked = n.div_ceil(2);
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_quantize_i4")
            .map_err(|e| RmiError::compute_simple(format!("function `k_quantize_i4`: {e:?}")))?;
        let mut out = DeviceBuf::<i8>::alloc(stream.clone(), npacked.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc i4: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &func, npacked as u32, 256, (&*in_dev, &mut out, inv_scale, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch k_quantize_i4: {e:?}")))?;
        }
        // Store packed bytes; tag dtype I4, shape = logical.
        let id = self.fresh_id();
        let handle = TensorHandle {
            id,
            shape: a.shape.clone(),
            dtype: DType::I4,
            backend: BackendType::Cuda,
            size_bytes: npacked,
        };
        self.gpu_storage_i8
            .write()
            .expect("gpu_storage_i8 poisoned")
            .insert(id, Arc::new(out));
        Ok((handle, scale))
    }

    /// Dequantize an INT4 (packed) tensor back to F32 using its `scale`.
    pub fn dequantize_i4(&self, a: &TensorHandle, scale: f32) -> Result<TensorHandle> {
        if a.dtype != DType::I4 {
            return Err(RmiError::compute_simple("dequantize_i4: handle not I4"));
        }
        let n = a.numel();
        let in_dev = self
            .gpu_storage_i8
            .read()
            .expect("gpu_storage_i8 poisoned")
            .get(&a.id)
            .cloned()
            .ok_or_else(|| RmiError::compute_simple(format!("dequantize_i4: id {} missing", a.id)))?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_dequantize_i4")
            .map_err(|e| RmiError::compute_simple(format!("function `k_dequantize_i4`: {e:?}")))?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc f32: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &func, n as u32, 256, (&*in_dev, &mut out, scale, n as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch k_dequantize_i4: {e:?}")))?;
        }
        self.gpu_store_f32(out, &a.shape)
    }

    /// W8A4-style matmul: INT4 activation `aq [M,K]` (packed, scale `sa`)
    /// × INT8 per-channel weight `bq [K,N]` (scales `sb`). Unpacks A's
    /// nibbles inline, INT32 accumulate, dequant per column → F32. INT4
    /// activations cut activation memory in half vs INT8.
    pub fn matmul_i4a_i8b(
        &self,
        aq: &TensorHandle,
        sa: f32,
        bq: &TensorHandle,
        sb: &[f32],
    ) -> Result<TensorHandle> {
        if aq.dtype != DType::I4 {
            return Err(RmiError::compute_simple("matmul_i4a_i8b: A must be I4"));
        }
        if aq.shape.len() != 2 || bq.shape.len() != 2 {
            return Err(RmiError::compute_simple("matmul_i4a_i8b: operands must be 2-D"));
        }
        let m = aq.shape[0];
        let k = aq.shape[1];
        let (k2, n) = (bq.shape[0], bq.shape[1]);
        if k != k2 {
            return Err(RmiError::compute_simple("matmul_i4a_i8b: K mismatch"));
        }
        if sb.len() != n {
            return Err(RmiError::compute_simple("matmul_i4a_i8b: scale count != N"));
        }
        let a_dev = self
            .gpu_storage_i8
            .read()
            .expect("gpu_storage_i8 poisoned")
            .get(&aq.id)
            .cloned()
            .ok_or_else(|| RmiError::compute_simple(format!("matmul_i4a_i8b: A id {} missing", aq.id)))?;
        let b_dev = self.i8_get(bq)?;
        let stream = self.device.default_stream();
        let sb_dev = DeviceBuf::<f32>::from_host(stream.clone(), sb)
            .map_err(|e| RmiError::compute_simple(format!("htod sb: {e:?}")))?;
        let module = self.kernel_module()?;
        let func = module
            .function("k_matmul_i4a_i8b")
            .map_err(|e| RmiError::compute_simple(format!("function `k_matmul_i4a_i8b`: {e:?}")))?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), (m * n).max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc out: {e:?}")))?;
        if m * n > 0 {
            launch_1d(
                &stream,
                &func,
                (m * n) as u32,
                256,
                (&*a_dev, &*b_dev, &mut out, m as i32, k as i32, n as i32, sa, &sb_dev),
            )
            .map_err(|e| RmiError::compute_simple(format!("launch k_matmul_i4a_i8b: {e:?}")))?;
        }
        self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
        self.gpu_store_f32(out, &[m, n])
    }

    /// P137: per-channel INT4 quantize of a weight `[K,N]` (packed two
    /// nibbles/byte at flat row-major index). Per-column scale = amax/7.
    /// 8× smaller than F32, 2× smaller than INT8 weights.
    pub fn quantize_i4_perchannel(&self, a: &TensorHandle) -> Result<(TensorHandle, Vec<f32>)> {
        if a.dtype != DType::F32 || a.shape.len() != 2 {
            return Err(RmiError::compute_simple(
                "quantize_i4_perchannel: input must be 2-D F32",
            ));
        }
        let k = a.shape[0];
        let n = a.shape[1];
        let host = self.handle_to_host_f32(a)?;
        let mut scales = vec![1.0f32; n];
        let mut inv_scales = vec![1.0f32; n];
        for c in 0..n {
            let mut amax = 0.0f32;
            for r in 0..k {
                amax = amax.max(host[r * n + c].abs());
            }
            let s = if amax > 0.0 { amax / 7.0 } else { 1.0 };
            scales[c] = s;
            inv_scales[c] = 1.0 / s;
        }
        let total = k * n;
        let npacked = total.div_ceil(2);
        let stream = self.device.default_stream();
        let inv_dev = DeviceBuf::<f32>::from_host(stream.clone(), &inv_scales)
            .map_err(|e| RmiError::compute_simple(format!("htod inv_scales: {e:?}")))?;
        let in_dev = self.gpu_get_f32(a)?;
        let module = self.kernel_module()?;
        let func = module
            .function("k_quantize_i4_pc")
            .map_err(|e| RmiError::compute_simple(format!("function `k_quantize_i4_pc`: {e:?}")))?;
        let mut out = DeviceBuf::<i8>::alloc(stream.clone(), npacked.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc i4: {e:?}")))?;
        if total > 0 {
            launch_1d(
                &stream,
                &func,
                npacked as u32,
                256,
                (&*in_dev, &mut out, &inv_dev, k as i32, n as i32),
            )
            .map_err(|e| RmiError::compute_simple(format!("launch k_quantize_i4_pc: {e:?}")))?;
        }
        let id = self.fresh_id();
        let handle = TensorHandle {
            id,
            shape: a.shape.clone(),
            dtype: DType::I4,
            backend: BackendType::Cuda,
            size_bytes: npacked,
        };
        self.gpu_storage_i8
            .write()
            .expect("gpu_storage_i8 poisoned")
            .insert(id, Arc::new(out));
        Ok((handle, scales))
    }

    /// P137: W4A8 matmul — INT8 activation `aq [M,K]` (scale `sa`) ×
    /// packed-INT4 per-channel weight `wq [K,N]` (scales `sb`). INT32
    /// accumulate, dequant per column → F32.
    pub fn matmul_i8a_i4b(
        &self,
        aq: &TensorHandle,
        sa: f32,
        wq: &TensorHandle,
        sb: &[f32],
    ) -> Result<TensorHandle> {
        if aq.dtype != DType::I8 || wq.dtype != DType::I4 {
            return Err(RmiError::compute_simple("matmul_i8a_i4b: need I8 act + I4 weight"));
        }
        if aq.shape.len() != 2 || wq.shape.len() != 2 {
            return Err(RmiError::compute_simple("matmul_i8a_i4b: operands must be 2-D"));
        }
        let m = aq.shape[0];
        let k = aq.shape[1];
        let (k2, n) = (wq.shape[0], wq.shape[1]);
        if k != k2 {
            return Err(RmiError::compute_simple("matmul_i8a_i4b: K mismatch"));
        }
        if sb.len() != n {
            return Err(RmiError::compute_simple("matmul_i8a_i4b: scale count != N"));
        }
        let a_dev = self.i8_get(aq)?;
        let w_dev = self
            .gpu_storage_i8
            .read()
            .expect("gpu_storage_i8 poisoned")
            .get(&wq.id)
            .cloned()
            .ok_or_else(|| RmiError::compute_simple(format!("matmul_i8a_i4b: W id {} missing", wq.id)))?;
        let stream = self.device.default_stream();
        let sb_dev = DeviceBuf::<f32>::from_host(stream.clone(), sb)
            .map_err(|e| RmiError::compute_simple(format!("htod sb: {e:?}")))?;
        let module = self.kernel_module()?;
        let func = module
            .function("k_matmul_i8a_i4b")
            .map_err(|e| RmiError::compute_simple(format!("function `k_matmul_i8a_i4b`: {e:?}")))?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), (m * n).max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc out: {e:?}")))?;
        if m * n > 0 {
            launch_1d(
                &stream,
                &func,
                (m * n) as u32,
                256,
                (&*a_dev, &*w_dev, &mut out, m as i32, k as i32, n as i32, sa, &sb_dev),
            )
            .map_err(|e| RmiError::compute_simple(format!("launch k_matmul_i8a_i4b: {e:?}")))?;
        }
        self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
        self.gpu_store_f32(out, &[m, n])
    }

    /// INT8 quantized 2-D matmul: `aq [M,K]` · `bq [K,N]` with scales
    /// `sa`/`sb`. Accumulates in INT32 and dequantizes the result to F32
    /// by `sa*sb`. Net-new capability; result is F32.
    pub fn matmul_i8(
        &self,
        aq: &TensorHandle,
        sa: f32,
        bq: &TensorHandle,
        sb: f32,
    ) -> Result<TensorHandle> {
        if aq.shape.len() != 2 || bq.shape.len() != 2 {
            return Err(RmiError::compute_simple("matmul_i8: operands must be 2-D"));
        }
        let m = aq.shape[0];
        let k = aq.shape[1];
        let k2 = bq.shape[0];
        let n = bq.shape[1];
        if k != k2 {
            return Err(RmiError::compute_simple(format!(
                "matmul_i8 shape mismatch: [{m},{k}] x [{k2},{n}]"
            )));
        }
        let a_dev = self.i8_get(aq)?;
        let b_dev = self.i8_get(bq)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_matmul_i8_deq")
            .map_err(|e| RmiError::compute_simple(format!("function `k_matmul_i8_deq`: {e:?}")))?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), (m * n).max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc i8 matmul out: {e:?}")))?;
        let combined = sa * sb;
        if m * n > 0 {
            launch_1d(
                &stream,
                &func,
                (m * n) as u32,
                256,
                (&*a_dev, &*b_dev, &mut out, m as i32, k as i32, n as i32, combined),
            )
            .map_err(|e| RmiError::compute_simple(format!("launch k_matmul_i8_deq: {e:?}")))?;
        }
        self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
        self.gpu_store_f32(out, &[m, n])
    }

    /// P122: per-column (per-channel) symmetric INT8 quantization of a
    /// `[K, N]` matrix. Each column n gets its own scale `amax_n/127`.
    /// Returns the I8 handle and a host `Vec<f32>` of N scales. Column
    /// amax is computed on the host (one-time, offline for weights).
    pub fn quantize_i8_perchannel(&self, a: &TensorHandle) -> Result<(TensorHandle, Vec<f32>)> {
        if a.dtype != DType::F32 || a.shape.len() != 2 {
            return Err(RmiError::compute_simple(
                "quantize_i8_perchannel: input must be 2-D F32",
            ));
        }
        let k = a.shape[0];
        let n = a.shape[1];
        // Host amax per column.
        let host = self.handle_to_host_f32(a)?;
        let mut scales = vec![1.0f32; n];
        let mut inv_scales = vec![1.0f32; n];
        for c in 0..n {
            let mut amax = 0.0f32;
            for r in 0..k {
                amax = amax.max(host[r * n + c].abs());
            }
            let s = if amax > 0.0 { amax / 127.0 } else { 1.0 };
            scales[c] = s;
            inv_scales[c] = 1.0 / s;
        }
        let stream = self.device.default_stream();
        let inv_dev = DeviceBuf::<f32>::from_host(stream.clone(), &inv_scales)
            .map_err(|e| RmiError::compute_simple(format!("htod inv_scales: {e:?}")))?;
        let in_dev = self.gpu_get_f32(a)?;
        let module = self.kernel_module()?;
        let func = module
            .function("k_quantize_i8_pc")
            .map_err(|e| RmiError::compute_simple(format!("function `k_quantize_i8_pc`: {e:?}")))?;
        let mut out = DeviceBuf::<i8>::alloc(stream.clone(), (k * n).max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc i8: {e:?}")))?;
        if k * n > 0 {
            launch_1d(
                &stream,
                &func,
                (k * n) as u32,
                256,
                (&*in_dev, &mut out, &inv_dev, k as i32, n as i32),
            )
            .map_err(|e| RmiError::compute_simple(format!("launch k_quantize_i8_pc: {e:?}")))?;
        }
        Ok((self.i8_store(out, &a.shape)?, scales))
    }

    /// P122: INT8 matmul `aq [M,K]` (per-tensor scale `sa`) · `bq [K,N]`
    /// (per-column scales `sb`, length N) → F32. Dequant per output
    /// column: `out[m,n] = acc * sa * sb[n]`.
    pub fn matmul_i8_perchannel(
        &self,
        aq: &TensorHandle,
        sa: f32,
        bq: &TensorHandle,
        sb: &[f32],
    ) -> Result<TensorHandle> {
        if aq.shape.len() != 2 || bq.shape.len() != 2 {
            return Err(RmiError::compute_simple("matmul_i8_perchannel: operands must be 2-D"));
        }
        let m = aq.shape[0];
        let k = aq.shape[1];
        let k2 = bq.shape[0];
        let n = bq.shape[1];
        if k != k2 {
            return Err(RmiError::compute_simple(format!(
                "matmul_i8_perchannel shape mismatch: [{m},{k}] x [{k2},{n}]"
            )));
        }
        if sb.len() != n {
            return Err(RmiError::compute_simple(format!(
                "matmul_i8_perchannel: {} scales for {n} columns",
                sb.len()
            )));
        }
        let a_dev = self.i8_get(aq)?;
        let b_dev = self.i8_get(bq)?;
        let stream = self.device.default_stream();
        let sb_dev = DeviceBuf::<f32>::from_host(stream.clone(), sb)
            .map_err(|e| RmiError::compute_simple(format!("htod sb scales: {e:?}")))?;
        let module = self.kernel_module()?;
        let func = module.function("k_matmul_i8_deq_pc").map_err(|e| {
            RmiError::compute_simple(format!("function `k_matmul_i8_deq_pc`: {e:?}"))
        })?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), (m * n).max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc out: {e:?}")))?;
        if m * n > 0 {
            launch_1d(
                &stream,
                &func,
                (m * n) as u32,
                256,
                (&*a_dev, &*b_dev, &mut out, m as i32, k as i32, n as i32, sa, &sb_dev),
            )
            .map_err(|e| RmiError::compute_simple(format!("launch k_matmul_i8_deq_pc: {e:?}")))?;
        }
        self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
        self.gpu_store_f32(out, &[m, n])
    }

    /// P126: per-column quantize a `[K,N]` weight into the **transposed**
    /// `[N,K]` INT8 layout the IMMA TN path needs (K-major). Returns the
    /// I8 handle (shape `[N,K]`) and the N per-column scales.
    pub fn quantize_i8_perchannel_t(&self, a: &TensorHandle) -> Result<(TensorHandle, Vec<f32>)> {
        if a.dtype != DType::F32 || a.shape.len() != 2 {
            return Err(RmiError::compute_simple(
                "quantize_i8_perchannel_t: input must be 2-D F32",
            ));
        }
        let k = a.shape[0];
        let n = a.shape[1];
        let host = self.handle_to_host_f32(a)?;
        let mut scales = vec![1.0f32; n];
        let mut inv_scales = vec![1.0f32; n];
        for c in 0..n {
            let mut amax = 0.0f32;
            for r in 0..k {
                amax = amax.max(host[r * n + c].abs());
            }
            let s = if amax > 0.0 { amax / 127.0 } else { 1.0 };
            scales[c] = s;
            inv_scales[c] = 1.0 / s;
        }
        let stream = self.device.default_stream();
        let inv_dev = DeviceBuf::<f32>::from_host(stream.clone(), &inv_scales)
            .map_err(|e| RmiError::compute_simple(format!("htod inv_scales: {e:?}")))?;
        let in_dev = self.gpu_get_f32(a)?;
        let module = self.kernel_module()?;
        let func = module.function("k_quantize_i8_pc_t").map_err(|e| {
            RmiError::compute_simple(format!("function `k_quantize_i8_pc_t`: {e:?}"))
        })?;
        let mut out = DeviceBuf::<i8>::alloc(stream.clone(), (k * n).max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc i8: {e:?}")))?;
        if k * n > 0 {
            launch_1d(
                &stream,
                &func,
                (k * n) as u32,
                256,
                (&*in_dev, &mut out, &inv_dev, k as i32, n as i32),
            )
            .map_err(|e| RmiError::compute_simple(format!("launch k_quantize_i8_pc_t: {e:?}")))?;
        }
        // Stored shape is the transposed [N, K].
        Ok((self.i8_store(out, &[n, k])?, scales))
    }

    /// P126: INT8 matmul via cuBLASLt **IMMA tensor cores**.
    /// `aq [M,K]` (per-tensor scale `sa`), `bq_t [N,K]` (= weight `[K,N]`
    /// quantized transposed, per-column scales `sb`). Uses the TN INT8
    /// path (`op(A)=T, op(B)=N`, both K-major, COL order — no COL32
    /// reorder), accumulating INT32, then dequant-transposes to F32
    /// `[M,N]`. Requires M,K,N multiples of 4; errors otherwise (caller
    /// falls back). This is the real tensor-core INT8 throughput path.
    pub fn matmul_i8_immma(
        &self,
        aq: &TensorHandle,
        sa: f32,
        bq_t: &TensorHandle,
        sb: &[f32],
    ) -> Result<TensorHandle> {
        let m = aq.shape[0];
        let n = bq_t.shape[0]; // bq_t is [N, K]
        if sb.len() != n {
            return Err(RmiError::compute_simple("matmul_i8_immma: scale count != N"));
        }
        let stream = self.device.default_stream();
        let sb_dev = DeviceBuf::<f32>::from_host(stream.clone(), sb)
            .map_err(|e| RmiError::compute_simple(format!("htod sb: {e:?}")))?;
        let c_i32 = self.immma_gemm_i32(aq, bq_t)?;

        // Dequant + transpose col-major[M,N] → row-major[M,N] F32.
        let module = self.kernel_module()?;
        let func = module.function("k_dequant_i32_pc_t").map_err(|e| {
            RmiError::compute_simple(format!("function `k_dequant_i32_pc_t`: {e:?}"))
        })?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), m * n)
            .map_err(|e| RmiError::compute_simple(format!("alloc deq out: {e:?}")))?;
        launch_1d(
            &stream,
            &func,
            (m * n) as u32,
            256,
            (&c_i32, &mut out, m as i32, n as i32, sa, &sb_dev),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch k_dequant_i32_pc_t: {e:?}")))?;
        self.quant_imma_count.fetch_add(1, Ordering::Relaxed);
        self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
        self.gpu_store_f32(out, &[m, n])
    }

    /// P126/P134: shared IMMA INT8 GEMM core. `aq [M,K]` row-major, `bq_t
    /// [N,K]` row-major (weight transposed); TN path (op(A)=T, op(B)=N)
    /// → col-major[M,N] INT32 (buffer[m + n*M]). Validates the mult-of-4
    /// IMMA constraint. Shared by symmetric + asymmetric dequant paths.
    fn immma_gemm_i32(&self, aq: &TensorHandle, bq_t: &TensorHandle) -> Result<DeviceBuf<i32>> {
        use ironaccelerator_cuda::drv::Error as DrvErr;
        if aq.shape.len() != 2 || bq_t.shape.len() != 2 {
            return Err(RmiError::compute_simple("immma_gemm_i32: operands must be 2-D"));
        }
        let m = aq.shape[0];
        let k = aq.shape[1];
        let n = bq_t.shape[0];
        let k2 = bq_t.shape[1];
        if k != k2 {
            return Err(RmiError::compute_simple("immma_gemm_i32: K mismatch"));
        }
        if m % 4 != 0 || k % 4 != 0 || n % 4 != 0 || m == 0 || k == 0 || n == 0 {
            return Err(RmiError::compute_simple("immma_gemm_i32: dims must be nonzero mult of 4"));
        }
        let a_dev = self.i8_get(aq)?;
        let b_dev = self.i8_get(bq_t)?;
        let stream = self.device.default_stream();
        (|| -> std::result::Result<DeviceBuf<i32>, DrvErr> {
            let c = DeviceBuf::<i32>::alloc(stream.clone(), m * n)?;
            const WS_BYTES: usize = 4 * 1024 * 1024;
            let mut ws = DeviceBuf::<u8>::alloc(stream.clone(), WS_BYTES)?;
            let handle = blas::handle_for(&stream).map_err(|e| DrvErr::Precondition {
                op: "BlasLt::handle_for",
                msg: format!("{e:?}"),
            })?;
            let mut desc = MatmulDesc::new(ComputeType::I32, CuDType::R32I).map_err(|e| {
                DrvErr::Precondition { op: "MatmulDesc::new(I32)", msg: format!("{e:?}") }
            })?;
            desc.set_transpose(Op::T, Op::N).map_err(|e| DrvErr::Precondition {
                op: "set_transpose(T,N)",
                msg: format!("{e:?}"),
            })?;
            let a_layout = MatrixLayout::new(CuDType::R8I, k as u64, m as u64, k as i64)
                .map_err(|e| DrvErr::Precondition { op: "layout A", msg: format!("{e:?}") })?;
            let b_layout = MatrixLayout::new(CuDType::R8I, k as u64, n as u64, k as i64)
                .map_err(|e| DrvErr::Precondition { op: "layout B", msg: format!("{e:?}") })?;
            let c_layout = MatrixLayout::new(CuDType::R32I, m as u64, n as u64, m as i64)
                .map_err(|e| DrvErr::Precondition { op: "layout C", msg: format!("{e:?}") })?;
            let mut pref = Preference::new().map_err(|e| DrvErr::Precondition {
                op: "Preference::new",
                msg: format!("{e:?}"),
            })?;
            pref.set_max_workspace(WS_BYTES).map_err(|e| DrvErr::Precondition {
                op: "set_max_workspace",
                msg: format!("{e:?}"),
            })?;
            let algo = blas::heuristic(&handle, &desc, &a_layout, &b_layout, &c_layout, &c_layout, &pref)
                .map_err(|e| DrvErr::Precondition { op: "heuristic(int8)", msg: format!("{e:?}") })?;
            let alpha: i32 = 1;
            let beta: i32 = 0;
            unsafe {
                blas::matmul(
                    &handle, &desc, &alpha.to_ne_bytes(), &beta.to_ne_bytes(),
                    a_dev.device_ptr(), &a_layout, b_dev.device_ptr(), &b_layout,
                    c.device_ptr(), &c_layout, c.device_ptr(), &c_layout,
                    Some(&algo), Some(&mut ws), &stream,
                )
                .map_err(|e| DrvErr::Precondition { op: "blas::matmul(int8)", msg: format!("{e:?}") })?;
            }
            Ok(c)
        })()
        .map_err(|e| RmiError::compute_simple(format!("CUDA i8 IMMA {m}x{k}@{k}x{n}: {e:?}")))
    }

    /// P134: asymmetric-activation INT8 matmul on IMMA tensor cores.
    /// `aq [M,K]` asymmetric (scale `sa`, zero-point `za`), `bq_t [N,K]`
    /// symmetric per-column weight (scales `sb`). IMMA GEMM, then exact
    /// zero-point correction `−za·rowsum(bq_t)[n]` in the dequant.
    pub fn matmul_i8_immma_asym(
        &self,
        aq: &TensorHandle,
        sa: f32,
        za: i32,
        bq_t: &TensorHandle,
        sb: &[f32],
    ) -> Result<TensorHandle> {
        let m = aq.shape[0];
        let n = bq_t.shape[0];
        let k = bq_t.shape[1];
        if sb.len() != n {
            return Err(RmiError::compute_simple("matmul_i8_immma_asym: scale count != N"));
        }
        let stream = self.device.default_stream();
        let sb_dev = DeviceBuf::<f32>::from_host(stream.clone(), sb)
            .map_err(|e| RmiError::compute_simple(format!("htod sb: {e:?}")))?;
        let c_i32 = self.immma_gemm_i32(aq, bq_t)?;
        // Per-output-column weight sum = row sum of bq_t[N,K].
        let b_dev = self.i8_get(bq_t)?;
        let module = self.kernel_module()?;
        let cs_fn = module
            .function("k_rowsum_i8")
            .map_err(|e| RmiError::compute_simple(format!("function `k_rowsum_i8`: {e:?}")))?;
        let mut wsum = DeviceBuf::<i32>::alloc(stream.clone(), n.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc wsum: {e:?}")))?;
        if n > 0 {
            launch_1d(&stream, &cs_fn, n as u32, 256, (&*b_dev, &mut wsum, n as i32, k as i32))
                .map_err(|e| RmiError::compute_simple(format!("launch k_rowsum_i8: {e:?}")))?;
        }
        let func = module.function("k_dequant_i32_pc_t_asym").map_err(|e| {
            RmiError::compute_simple(format!("function `k_dequant_i32_pc_t_asym`: {e:?}"))
        })?;
        let mut out = DeviceBuf::<f32>::alloc(stream.clone(), m * n)
            .map_err(|e| RmiError::compute_simple(format!("alloc deq out: {e:?}")))?;
        launch_1d(
            &stream,
            &func,
            (m * n) as u32,
            256,
            (&c_i32, &wsum, &mut out, m as i32, n as i32, sa, &sb_dev, za),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch k_dequant asym: {e:?}")))?;
        self.quant_imma_count.fetch_add(1, Ordering::Relaxed);
        self.matmul_gpu_count.fetch_add(1, Ordering::Relaxed);
        self.gpu_store_f32(out, &[m, n])
    }

    /// P118: run a binary elementwise op on two half tensors by
    /// upcasting both to F32, running the F32 GPU op, and downcasting
    /// the result back to the operands' half dtype. The intermediates
    /// are freed; the result stays GPU-resident and half-typed. This is
    /// the generic pattern for any op without a native half kernel.
    fn half_binary_via_f32<F>(
        &self,
        a: &TensorHandle,
        b: &TensorHandle,
        f32_op: F,
    ) -> Result<TensorHandle>
    where
        F: FnOnce(&TensorHandle, &TensorHandle) -> Result<TensorHandle>,
    {
        let af = self.half_to_f32_gpu(a)?;
        let bf = self.half_to_f32_gpu(b)?;
        let rf = f32_op(&af, &bf)?;
        let out = self.to_half_gpu(&rf, a.dtype);
        let _ = self.free(&af);
        let _ = self.free(&bf);
        let _ = self.free(&rf);
        out
    }

    /// Run a unary op on a half tensor by upcasting to F32, running the
    /// F32 GPU op, and downcasting the result back to `a`'s half dtype.
    fn half_unary_via_f32<F>(&self, a: &TensorHandle, f32_op: F) -> Result<TensorHandle>
    where
        F: FnOnce(&TensorHandle) -> Result<TensorHandle>,
    {
        let af = self.half_to_f32_gpu(a)?;
        let rf = f32_op(&af)?;
        let out = self.to_half_gpu(&rf, a.dtype);
        let _ = self.free(&af);
        let _ = self.free(&rf);
        out
    }

    /// If `a` is a half tensor, run the (shape-result) unary op via the
    /// upcast→F32→downcast path and return `Some(result)`; else `None`.
    fn try_half_unary<F>(&self, a: &TensorHandle, op: F) -> Option<Result<TensorHandle>>
    where
        F: FnOnce(&CudaBackend, &TensorHandle) -> Result<TensorHandle>,
    {
        if matches!(a.dtype, DType::F16 | DType::BF16) {
            Some(self.half_unary_via_f32(a, |x| op(self, x)))
        } else {
            None
        }
    }

    /// Scalar-reduce (sum/mean/max/min → f64) a half tensor by upcasting
    /// to F32 first. Returns `Some(result)` for half inputs, else `None`.
    fn try_half_scalar<F>(&self, a: &TensorHandle, op: F) -> Option<Result<f64>>
    where
        F: FnOnce(&CudaBackend, &TensorHandle) -> Result<f64>,
    {
        if matches!(a.dtype, DType::F16 | DType::BF16) {
            let r = self.half_to_f32_gpu(a).and_then(|af| {
                let v = op(self, &af);
                let _ = self.free(&af);
                v
            });
            Some(r)
        } else {
            None
        }
    }

    /// If both operands are the same half dtype and shape, run the
    /// binary op via the upcast→F32→downcast path and return
    /// `Some(result)`; otherwise `None` (caller takes its normal path).
    fn try_half_binary<F>(
        &self,
        a: &TensorHandle,
        b: &TensorHandle,
        op: F,
    ) -> Option<Result<TensorHandle>>
    where
        F: FnOnce(&CudaBackend, &TensorHandle, &TensorHandle) -> Result<TensorHandle>,
    {
        if matches!(a.dtype, DType::F16 | DType::BF16)
            && a.dtype == b.dtype
            && a.shape == b.shape
        {
            Some(self.half_binary_via_f32(a, b, |x, y| op(self, x, y)))
        } else {
            None
        }
    }

    /// Unary elementwise launcher (one input, one output, length).
    /// Falls back to `cpu_fn` if input isn't F32 or any GPU step errors.
    fn elementwise_unary_or_bounce<F>(
        &self,
        kernel_name: &'static str,
        a: &TensorHandle,
        cpu_fn: F,
    ) -> Result<TensorHandle>
    where
        F: FnOnce(&TensorHandle) -> Result<TensorHandle>,
    {
        if a.dtype != DType::F32 {
            return cpu_fn(a);
        }
        match self.unary_f32_gpu(kernel_name, a) {
            Ok(h) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(h)
            }
            Err(_) => cpu_fn(a),
        }
    }

    /// Binary elementwise launcher (two inputs, one output, length).
    fn elementwise_binary_or_bounce<F>(
        &self,
        kernel_name: &'static str,
        a: &TensorHandle,
        b: &TensorHandle,
        cpu_fn: F,
    ) -> Result<TensorHandle>
    where
        F: FnOnce(&TensorHandle, &TensorHandle) -> Result<TensorHandle>,
    {
        // Require same-shape, both F32. Broadcasting is left to CpuBackend.
        if a.dtype != DType::F32 || b.dtype != DType::F32 || a.shape != b.shape {
            return cpu_fn(a, b);
        }
        match self.binary_f32_gpu(kernel_name, a, b) {
            Ok(h) => {
                self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
                Ok(h)
            }
            Err(_) => cpu_fn(a, b),
        }
    }

    fn unary_f32_gpu(&self, kernel_name: &str, a: &TensorHandle) -> Result<TensorHandle> {
        let n = a.numel();
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function(kernel_name)
            .map_err(|e| RmiError::compute_simple(format!("function `{kernel_name}`: {e:?}")))?;

        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), n)
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;

        launch_1d(
            &stream,
            &func,
            n as u32,
            256,
            (&*in_dev, &mut out_dev, n as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch `{kernel_name}`: {e:?}")))?;
        // Kernel is enqueued on the default stream; subsequent stream
        // ops see its effects without an explicit sync. We sync only
        // at host-visible boundaries (copy_to_host).
        self.gpu_store_f32(out_dev, &a.shape)
    }

    fn binary_f32_gpu(
        &self,
        kernel_name: &str,
        a: &TensorHandle,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        let n = a.numel();
        let a_dev = self.gpu_get_f32(a)?;
        let b_dev = self.gpu_get_f32(b)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function(kernel_name)
            .map_err(|e| RmiError::compute_simple(format!("function `{kernel_name}`: {e:?}")))?;

        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), n)
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;

        launch_1d(
            &stream,
            &func,
            n as u32,
            256,
            (&*a_dev, &*b_dev, &mut out_dev, n as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch `{kernel_name}`: {e:?}")))?;
        self.gpu_store_f32(out_dev, &a.shape)
    }

    /// Scalar-reduction CPU bounce: dtoh, create cpu handle, run op,
    /// clean up. Used when GPU path can't take the input.
    fn scalar_reduce_bounce<F>(&self, a: &TensorHandle, cpu_op: F) -> Result<f64>
    where
        F: FnOnce(&CpuBackend, &TensorHandle) -> Result<f64>,
    {
        // For F32 we can pull from GPU; for non-F32 the handle is
        // already in cpu.
        if a.dtype == DType::F32 && matches!(a.backend, BackendType::Cuda) {
            let host = self.handle_to_host_f32(a)?;
            let cpu_a = self.cpu.from_slice_f32(&host, &a.shape)?;
            let v = cpu_op(&self.cpu, &cpu_a);
            let _ = self.cpu.free(&cpu_a);
            self.bounce_count.fetch_add(1, Ordering::Relaxed);
            return v;
        }
        cpu_op(&self.cpu, a)
    }

    fn reduce_scalar_f32_gpu(&self, kernel_name: &str, a: &TensorHandle) -> Result<f32> {
        let n = a.numel();
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function(kernel_name)
            .map_err(|e| RmiError::compute_simple(format!("function `{kernel_name}`: {e:?}")))?;

        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), 1)
            .map_err(|e| RmiError::compute_simple(format!("alloc scalar: {e:?}")))?;

        let cfg = ironaccelerator_cuda::drv::LaunchCfg {
            grid: (1, 1, 1),
            block: (256, 1, 1),
            shared_bytes: 0,
        };
        ironaccelerator_cuda::launch::raw_launch(
            &stream,
            &func,
            cfg,
            (&*in_dev, &mut out_dev, n as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch `{kernel_name}`: {e:?}")))?;
        // Scalar result is host-bound; sync to make it observable.
        stream
            .synchronize()
            .map_err(|e| RmiError::compute_simple(format!("stream sync: {e:?}")))?;

        let mut out = [0f32; 1];
        out_dev
            .copy_to_host(&mut out)
            .map_err(|e| RmiError::compute_simple(format!("dtoh scalar: {e:?}")))?;
        Ok(out[0])
    }

    /// P127: parallel max|x| over the whole tensor. Stage 1 launches up
    /// to 256 blocks producing one non-negative partial each; stage 2
    /// reduces the (≤256) partials with the single-block `k_max_f32`.
    /// One host sync total (vs the prior two full single-block scans).
    fn amax_f32_gpu(&self, a: &TensorHandle) -> Result<f32> {
        let n = a.numel();
        if n == 0 {
            return Ok(0.0);
        }
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        // Stage 1: partials. Cap blocks at 256 so stage 2's single block
        // (256 threads) reduces them in one strided pass.
        let blocks = ((n as u32).div_ceil(256)).min(256).max(1);
        let part_fn = module
            .function("k_amax_partial_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_amax_partial_f32`: {e:?}")))?;
        let mut partials = DeviceBuf::<f32>::alloc(stream.clone(), blocks as usize)
            .map_err(|e| RmiError::compute_simple(format!("alloc partials: {e:?}")))?;
        let cfg = ironaccelerator_cuda::drv::LaunchCfg {
            grid: (blocks, 1, 1),
            block: (256, 1, 1),
            shared_bytes: 0,
        };
        ironaccelerator_cuda::launch::raw_launch(
            &stream,
            &part_fn,
            cfg,
            (&*in_dev, &mut partials, n as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch amax partial: {e:?}")))?;
        // Stage 2: max of the (already non-negative) partials.
        let max_fn = module
            .function("k_max_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_max_f32`: {e:?}")))?;
        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), 1)
            .map_err(|e| RmiError::compute_simple(format!("alloc scalar: {e:?}")))?;
        let cfg2 = ironaccelerator_cuda::drv::LaunchCfg {
            grid: (1, 1, 1),
            block: (256, 1, 1),
            shared_bytes: 0,
        };
        ironaccelerator_cuda::launch::raw_launch(
            &stream,
            &max_fn,
            cfg2,
            (&partials, &mut out_dev, blocks as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch amax final: {e:?}")))?;
        stream
            .synchronize()
            .map_err(|e| RmiError::compute_simple(format!("stream sync: {e:?}")))?;
        let mut out = [0f32; 1];
        out_dev
            .copy_to_host(&mut out)
            .map_err(|e| RmiError::compute_simple(format!("dtoh amax: {e:?}")))?;
        Ok(out[0])
    }

    /// 2-D transpose (axes = [1, 0]). Input `[M, K]` row-major →
    /// output `[K, M]` row-major. Element-per-thread kernel; 16×16
    /// block geometry.
    fn transpose_2d_f32_gpu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let m = a.shape[0];
        let k = a.shape[1];
        let n = m * k;
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_transpose_2d_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_transpose_2d_f32`: {e:?}")))?;

        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), n)
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;

        let bx: u32 = 16;
        let by: u32 = 16;
        let cfg = ironaccelerator_cuda::drv::LaunchCfg {
            grid: ((m as u32).div_ceil(bx), (k as u32).div_ceil(by), 1),
            block: (bx, by, 1),
            shared_bytes: 0,
        };
        ironaccelerator_cuda::launch::raw_launch(
            &stream,
            &func,
            cfg,
            (&*in_dev, &mut out_dev, m as i32, k as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch transpose_2d: {e:?}")))?;
        self.gpu_store_f32(out_dev, &[k, m])
    }

    /// P111: arbitrary-rank transpose/permute. `axes` is a permutation
    /// of `0..ndim`; output axis `k` comes from input axis `axes[k]`
    /// (same convention as ndarray `permuted_axes` / CpuBackend). Builds
    /// `out_dims[k] = in_shape[axes[k]]` and the per-axis multiplier
    /// `mult[k] = in_row_major_stride[axes[k]]`, uploads both as i32
    /// device arrays, and launches one thread per output element.
    fn permute_f32_gpu(&self, a: &TensorHandle, axes: &[usize]) -> Result<TensorHandle> {
        let ndim = a.shape.len();
        let total = a.numel();
        // Row-major strides of the INPUT tensor.
        let mut in_stride = vec![1i64; ndim];
        for d in (0..ndim.saturating_sub(1)).rev() {
            in_stride[d] = in_stride[d + 1] * a.shape[d + 1] as i64;
        }
        let out_dims: Vec<i32> = axes.iter().map(|&ax| a.shape[ax] as i32).collect();
        let mult: Vec<i32> = axes.iter().map(|&ax| in_stride[ax] as i32).collect();
        let out_shape: Vec<usize> = axes.iter().map(|&ax| a.shape[ax]).collect();

        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_permute_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_permute_f32`: {e:?}")))?;

        // Upload the small index arrays to the device.
        let dims_dev = DeviceBuf::<i32>::from_host(stream.clone(), &out_dims)
            .map_err(|e| RmiError::compute_simple(format!("htod out_dims: {e:?}")))?;
        let mult_dev = DeviceBuf::<i32>::from_host(stream.clone(), &mult)
            .map_err(|e| RmiError::compute_simple(format!("htod mult: {e:?}")))?;
        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), total)
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;

        launch_1d(
            &stream,
            &func,
            total as u32,
            256,
            (
                &*in_dev,
                &mut out_dev,
                &dims_dev,
                &mult_dev,
                ndim as i32,
                total as i32,
            ),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch k_permute_f32: {e:?}")))?;
        self.gpu_store_f32(out_dev, &out_shape)
    }

    /// P113: conv2d implemented as im2col + GEMM, fully GPU-resident.
    /// `input` NCHW `[N,Cin,H,W]`, `weight` `[Cout,Cin,KH,KW]`. Steps:
    ///   1. im2col → `col [M, K]`  (M = N·Hout·Wout, K = Cin·KH·KW)
    ///   2. weight reshape `[Cout, K]` → transpose `[K, Cout]`
    ///   3. cuBLASLt GEMM `Y2 [M, Cout] = col · Wᵀ`
    ///   4. reshape `Y2 → [N, Hout, Wout, Cout]` (NHWC)
    ///   5. permute `[0,3,1,2] → [N, Cout, Hout, Wout]` (NCHW)
    /// Every step reuses an already-validated GPU primitive; only the
    /// im2col kernel is new.
    fn conv2d_im2col_gpu(
        &self,
        input: &TensorHandle,
        weight: &TensorHandle,
        stride: usize,
        padding: usize,
        dilation: usize,
    ) -> Result<TensorHandle> {
        let (n, cin, h, w) = (
            input.shape[0],
            input.shape[1],
            input.shape[2],
            input.shape[3],
        );
        let (cout, cin_w, kh, kw) = (
            weight.shape[0],
            weight.shape[1],
            weight.shape[2],
            weight.shape[3],
        );
        if cin != cin_w {
            return Err(RmiError::compute_simple(format!(
                "conv2d channel mismatch: input C={cin}, weight C={cin_w}"
            )));
        }
        let eff_h = dilation * (kh - 1) + 1;
        let eff_w = dilation * (kw - 1) + 1;
        if h + 2 * padding < eff_h || w + 2 * padding < eff_w {
            return Err(RmiError::compute_simple(
                "conv2d dilated kernel larger than padded input",
            ));
        }
        let hout = (h + 2 * padding - eff_h) / stride + 1;
        let wout = (w + 2 * padding - eff_w) / stride + 1;
        let m = n * hout * wout;
        let k = cin * kh * kw;
        if m == 0 || k == 0 || cout == 0 {
            return Err(RmiError::compute_simple("conv2d degenerate dims"));
        }

        // 1. im2col → col [M, K].
        let x_dev = self.gpu_get_f32(input)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_im2col_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_im2col_f32`: {e:?}")))?;
        let mut col_dev = DeviceBuf::<f32>::alloc(stream.clone(), m * k)
            .map_err(|e| RmiError::compute_simple(format!("alloc col: {e:?}")))?;
        launch_1d(
            &stream,
            &func,
            (m * k) as u32,
            256,
            (
                &*x_dev,
                &mut col_dev,
                n as i32,
                cin as i32,
                h as i32,
                w as i32,
                kh as i32,
                kw as i32,
                hout as i32,
                wout as i32,
                stride as i32,
                padding as i32,
                dilation as i32,
                m as i32,
                k as i32,
            ),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch k_im2col_f32: {e:?}")))?;
        self.elementwise_gpu_count.fetch_add(1, Ordering::Relaxed);
        let col_h = self.gpu_store_f32(col_dev, &[m, k])?;

        // 2. weight [Cout,Cin,KH,KW] → [Cout, K] → [K, Cout].
        let wm = self.reshape(weight, &[cout, k])?;
        let wmt = self.transpose(&wm, &[1, 0])?;

        // 3. GEMM Y2 [M, Cout] = col [M, K] · Wᵀ [K, Cout].
        let y2 = self.matmul(&col_h, &wmt)?;

        // 4 + 5. reshape to NHWC then permute to NCHW.
        let y_nhwc = self.reshape(&y2, &[n, hout, wout, cout])?;
        self.transpose(&y_nhwc, &[0, 3, 1, 2])
    }

    /// P114: concat GPU-resident F32 tensors along `ax` via per-input
    /// strided slab copies into one fresh output buffer. No host
    /// roundtrip. Validates rank/shape compatibility; errors (→ bounce)
    /// on mismatch.
    fn concat_gpu(&self, ts: &[&TensorHandle], ax: usize) -> Result<TensorHandle> {
        let first = ts[0];
        let ndim = first.shape.len();
        if ndim == 0 || ax >= ndim {
            return Err(RmiError::compute_simple("concat: bad axis"));
        }
        // All shapes equal except along `ax`; accumulate the axis total.
        let mut ax_total = 0usize;
        for t in ts {
            if t.shape.len() != ndim {
                return Err(RmiError::compute_simple("concat: rank mismatch"));
            }
            for d in 0..ndim {
                if d != ax && t.shape[d] != first.shape[d] {
                    return Err(RmiError::compute_simple("concat: shape mismatch off-axis"));
                }
            }
            ax_total += t.shape[ax];
        }
        let mut out_shape = first.shape.clone();
        out_shape[ax] = ax_total;
        let outer: usize = out_shape[..ax].iter().product();
        let inner: usize = out_shape[ax + 1..].iter().product();
        let total: usize = out_shape.iter().product();

        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_concat_copy_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_concat_copy_f32`: {e:?}")))?;
        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), total.max(1))
            .map_err(|e| RmiError::compute_simple(format!("alloc concat out: {e:?}")))?;

        let mut off = 0i32;
        for t in ts {
            let ax_src = t.shape[ax];
            let n_elems = outer * ax_src * inner;
            if n_elems > 0 {
                let src = self.gpu_get_f32(t)?;
                launch_1d(
                    &stream,
                    &func,
                    n_elems as u32,
                    256,
                    (
                        &*src,
                        &mut out_dev,
                        outer as i32,
                        ax_src as i32,
                        inner as i32,
                        ax_total as i32,
                        off,
                    ),
                )
                .map_err(|e| RmiError::compute_simple(format!("launch concat copy: {e:?}")))?;
            }
            off += ax_src as i32;
        }
        self.gpu_store_f32(out_dev, &out_shape)
    }

    /// P114: split a GPU-resident F32 tensor along `ax` into `n` equal
    /// parts via strided slab copies out of the source. No host roundtrip.
    fn split_gpu(&self, a: &TensorHandle, ax: usize, n: usize) -> Result<Vec<TensorHandle>> {
        let ndim = a.shape.len();
        if ndim == 0 || ax >= ndim || n == 0 || a.shape[ax] % n != 0 {
            return Err(RmiError::compute_simple("split: bad axis/sections"));
        }
        let ax_total = a.shape[ax];
        let part_ax = ax_total / n;
        let outer: usize = a.shape[..ax].iter().product();
        let inner: usize = a.shape[ax + 1..].iter().product();
        let mut part_shape = a.shape.clone();
        part_shape[ax] = part_ax;
        let part_elems = outer * part_ax * inner;

        let src = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_split_copy_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_split_copy_f32`: {e:?}")))?;

        let mut out = Vec::with_capacity(n);
        for s in 0..n {
            let mut part_dev = DeviceBuf::<f32>::alloc(stream.clone(), part_elems.max(1))
                .map_err(|e| RmiError::compute_simple(format!("alloc split part: {e:?}")))?;
            if part_elems > 0 {
                launch_1d(
                    &stream,
                    &func,
                    part_elems as u32,
                    256,
                    (
                        &*src,
                        &mut part_dev,
                        outer as i32,
                        part_ax as i32,
                        inner as i32,
                        ax_total as i32,
                        (s * part_ax) as i32,
                    ),
                )
                .map_err(|e| RmiError::compute_simple(format!("launch split copy: {e:?}")))?;
            }
            out.push(self.gpu_store_f32(part_dev, &part_shape)?);
        }
        Ok(out)
    }

    /// CPU-bounce fallback for concat (input on GPU → CPU concat → GPU).
    fn concat_bounce(&self, ts: &[&TensorHandle], ax: usize) -> Result<TensorHandle> {
        let mut cpu_inputs = Vec::with_capacity(ts.len());
        for t in ts {
            let host = self.handle_to_host_f32(t)?;
            cpu_inputs.push(self.cpu.from_slice_f32(&host, &t.shape)?);
        }
        let cpu_refs: Vec<&TensorHandle> = cpu_inputs.iter().collect();
        let cpu_result = self.cpu.concat(&cpu_refs, ax)?;
        let out_shape = cpu_result.shape.clone();
        let out_bytes = self.cpu.copy_to_host(&cpu_result)?;
        let out_f32: Vec<f32> = out_bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        for h in &cpu_inputs {
            let _ = self.cpu.free(h);
        }
        let _ = self.cpu.free(&cpu_result);
        self.bounce_count.fetch_add(1, Ordering::Relaxed);
        self.host_to_gpu_f32(&out_f32, &out_shape)
    }

    /// CPU-bounce fallback for split.
    fn split_bounce(&self, a: &TensorHandle, ax: usize, n: usize) -> Result<Vec<TensorHandle>> {
        let host = self.handle_to_host_f32(a)?;
        let cpu_a = self.cpu.from_slice_f32(&host, &a.shape)?;
        let cpu_parts = self.cpu.split(&cpu_a, ax, n)?;
        let mut out = Vec::with_capacity(cpu_parts.len());
        for part in &cpu_parts {
            let bytes = self.cpu.copy_to_host(part)?;
            let part_f32: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            out.push(self.host_to_gpu_f32(&part_f32, &part.shape)?);
        }
        for p in &cpu_parts {
            let _ = self.cpu.free(p);
        }
        let _ = self.cpu.free(&cpu_a);
        self.bounce_count.fetch_add(1, Ordering::Relaxed);
        Ok(out)
    }

    /// Sum along the last (contiguous) axis. Input shape `[..., inner]`
    /// → output shape `[...]` (last dim dropped). 1D → scalar tensor `[]`.
    fn sum_axis_lastdim_f32_gpu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let n = a.numel();
        let inner = *a.shape.last().unwrap_or(&n);
        if inner == 0 {
            return self.bounce_unary(a, |cpu, x| {
                cpu.sum_axis(x, x.shape.len().saturating_sub(1))
            });
        }
        let outer = n / inner;
        let out_shape: Vec<usize> = a.shape[..a.shape.len() - 1].to_vec();
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_sum_axis_lastdim_f32")
            .map_err(|e| {
                RmiError::compute_simple(format!("function `k_sum_axis_lastdim_f32`: {e:?}"))
            })?;

        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), outer)
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;

        let cfg = ironaccelerator_cuda::drv::LaunchCfg {
            grid: (outer as u32, 1, 1),
            block: (256, 1, 1),
            shared_bytes: 0,
        };
        ironaccelerator_cuda::launch::raw_launch(
            &stream,
            &func,
            cfg,
            (&*in_dev, &mut out_dev, outer as i32, inner as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch sum_axis_lastdim: {e:?}")))?;
        self.gpu_store_f32(out_dev, &out_shape)
    }

    /// P110: sum along an arbitrary axis `ax`. Views the tensor as
    /// `[outer, axis, inner]` where outer = ∏shape[..ax],
    /// axis = shape[ax], inner = ∏shape[ax+1..]. Output drops `ax`.
    /// One thread per `outer*inner` output lane.
    fn sum_axis_any_f32_gpu(&self, a: &TensorHandle, ax: usize) -> Result<TensorHandle> {
        let ndim = a.shape.len();
        let axis_len = a.shape[ax];
        let outer: usize = a.shape[..ax].iter().product();
        let inner: usize = a.shape[ax + 1..].iter().product();
        let out_total = outer * inner;
        if axis_len == 0 || out_total == 0 {
            return self.bounce_unary(a, move |cpu, x| cpu.sum_axis(x, ax));
        }
        let mut out_shape: Vec<usize> = a.shape.clone();
        out_shape.remove(ax);
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_sum_axis_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_sum_axis_f32`: {e:?}")))?;
        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), out_total)
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;
        launch_1d(
            &stream,
            &func,
            out_total as u32,
            256,
            (
                &*in_dev,
                &mut out_dev,
                outer as i32,
                axis_len as i32,
                inner as i32,
            ),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch k_sum_axis_f32: {e:?}")))?;
        self.gpu_store_f32(out_dev, &out_shape)
    }

    /// P110: softmax along an arbitrary axis `ax`. Same `[outer, axis,
    /// inner]` view as `sum_axis_any_f32_gpu`; output keeps the input
    /// shape. One thread per lane does max → exp/sum → normalize.
    fn softmax_axis_any_f32_gpu(&self, a: &TensorHandle, ax: usize) -> Result<TensorHandle> {
        let axis_len = a.shape[ax];
        let outer: usize = a.shape[..ax].iter().product();
        let inner: usize = a.shape[ax + 1..].iter().product();
        let lanes = outer * inner;
        if axis_len == 0 || lanes == 0 {
            return self.bounce_unary(a, move |cpu, x| cpu.softmax(x, ax as i32));
        }
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_softmax_axis_f32")
            .map_err(|e| {
                RmiError::compute_simple(format!("function `k_softmax_axis_f32`: {e:?}"))
            })?;
        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), a.numel())
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;
        launch_1d(
            &stream,
            &func,
            lanes as u32,
            256,
            (
                &*in_dev,
                &mut out_dev,
                outer as i32,
                axis_len as i32,
                inner as i32,
            ),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch k_softmax_axis_f32: {e:?}")))?;
        self.gpu_store_f32(out_dev, &a.shape)
    }

    /// Softmax along the last (contiguous) axis. 1D collapses to
    /// outer=1, inner=n. ND uses outer = ∏(shape[..-1]), inner = last.
    fn softmax_lastdim_f32_gpu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let n = a.numel();
        let inner = *a.shape.last().unwrap_or(&n);
        if inner == 0 {
            return self.bounce_unary(a, |cpu, x| cpu.softmax(x, -1));
        }
        let outer = n / inner;
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_softmax_lastdim_f32")
            .map_err(|e| {
                RmiError::compute_simple(format!("function `k_softmax_lastdim_f32`: {e:?}"))
            })?;

        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), n)
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;

        let cfg = ironaccelerator_cuda::drv::LaunchCfg {
            grid: (outer as u32, 1, 1),
            block: (256, 1, 1),
            shared_bytes: 0,
        };
        ironaccelerator_cuda::launch::raw_launch(
            &stream,
            &func,
            cfg,
            (&*in_dev, &mut out_dev, outer as i32, inner as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch softmax: {e:?}")))?;
        self.gpu_store_f32(out_dev, &a.shape)
    }

    fn scale_f32_gpu(&self, a: &TensorHandle, s: f32) -> Result<TensorHandle> {
        let n = a.numel();
        let in_dev = self.gpu_get_f32(a)?;
        let stream = self.device.default_stream();
        let module = self.kernel_module()?;
        let func = module
            .function("k_scale_f32")
            .map_err(|e| RmiError::compute_simple(format!("function `k_scale_f32`: {e:?}")))?;

        let mut out_dev = DeviceBuf::<f32>::alloc(stream.clone(), n)
            .map_err(|e| RmiError::compute_simple(format!("alloc output: {e:?}")))?;

        launch_1d(
            &stream,
            &func,
            n as u32,
            256,
            (&*in_dev, s, &mut out_dev, n as i32),
        )
        .map_err(|e| RmiError::compute_simple(format!("launch k_scale_f32: {e:?}")))?;
        self.gpu_store_f32(out_dev, &a.shape)
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::*;
    use rmi::compute::cpu::CpuBackend;

    /// Verify the cuBLASLt path matches CpuBackend numerically. This
    /// test is run only when the CUDA driver is actually present at
    /// runtime — otherwise `CudaBackend::new()` Errs and we skip.
    #[test]
    fn matmul_gpu_matches_cpu_2d_f32() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();

        // Deliberately non-square so we'd catch a transposed bug.
        let m = 5usize;
        let k = 7usize;
        let n = 3usize;

        let a: Vec<f32> = (0..m * k).map(|i| (i as f32) * 0.1 - 1.0).collect();
        let b: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.07).sin()).collect();

        let a_g = cuda.from_slice_f32(&a, &[m, k]).expect("alloc A on cuda");
        let b_g = cuda.from_slice_f32(&b, &[k, n]).expect("alloc B on cuda");
        let d_g = cuda.matmul(&a_g, &b_g).expect("cuda matmul");

        let a_c = cpu.from_slice_f32(&a, &[m, k]).expect("alloc A on cpu");
        let b_c = cpu.from_slice_f32(&b, &[k, n]).expect("alloc B on cpu");
        let d_c = cpu.matmul(&a_c, &b_c).expect("cpu matmul");

        let g_bytes = cuda.copy_to_host(&d_g).expect("copy_to_host gpu result");
        let c_bytes = cpu.copy_to_host(&d_c).expect("copy_to_host cpu result");

        assert_eq!(d_g.shape, vec![m, n]);
        assert_eq!(d_c.shape, vec![m, n]);
        assert_eq!(g_bytes.len(), c_bytes.len());

        let g_f32: Vec<f32> = g_bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let c_f32: Vec<f32> = c_bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        let mut max_abs = 0f32;
        for (g, c) in g_f32.iter().zip(c_f32.iter()) {
            let d = (g - c).abs();
            if d > max_abs {
                max_abs = d;
            }
        }
        // cuBLASLt is bit-exact in many configurations but allow tiny
        // slack for fused/algo differences. 1e-4 is generous for f32.
        assert!(
            max_abs < 1e-4,
            "GPU matmul diverged from CPU: max_abs_diff={max_abs}\nGPU: {g_f32:?}\nCPU: {c_f32:?}"
        );
        assert!(cuda.matmul_gpu_count() >= 1, "GPU matmul path was not taken");
    }

    /// P109: strided-batched cuBLASLt matmul. `CpuBackend` can't do ND
    /// matmul, so the reference is built by slicing each batch and
    /// running a CPU 2-D matmul. Covers a 3-D `[B,M,K]@[B,K,N]` and a
    /// 4-D `[B1,B2,M,K]@...` to exercise leading-dim flattening.
    #[test]
    fn batched_matmul_gpu_matches_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();

        // Reference: per-batch CPU 2-D matmul, concatenated.
        // `a` is [batch, m, k] flat, `b` is [batch, k, n] flat.
        fn cpu_batched_ref(
            cpu: &CpuBackend,
            a: &[f32],
            b: &[f32],
            batch: usize,
            m: usize,
            k: usize,
            n: usize,
        ) -> Vec<f32> {
            let mut out = Vec::with_capacity(batch * m * n);
            for bi in 0..batch {
                let a_slice = &a[bi * m * k..(bi + 1) * m * k];
                let b_slice = &b[bi * k * n..(bi + 1) * k * n];
                let ah = cpu.from_slice_f32(a_slice, &[m, k]).unwrap();
                let bh = cpu.from_slice_f32(b_slice, &[k, n]).unwrap();
                let dh = cpu.matmul(&ah, &bh).unwrap();
                out.extend_from_slice(&host_f32(cpu, &dh));
            }
            out
        }

        // ── 3-D case: [4, 5, 7] @ [4, 7, 3] → [4, 5, 3] ──
        {
            let (batch, m, k, n) = (4usize, 5usize, 7usize, 3usize);
            let a: Vec<f32> = (0..batch * m * k)
                .map(|i| (i as f32 * 0.013 - 1.1).sin())
                .collect();
            let b: Vec<f32> = (0..batch * k * n)
                .map(|i| (i as f32 * 0.021 + 0.3).cos())
                .collect();
            let ag = cuda.from_slice_f32(&a, &[batch, m, k]).unwrap();
            let bg = cuda.from_slice_f32(&b, &[batch, k, n]).unwrap();
            let dg = cuda.matmul(&ag, &bg).expect("cuda batched matmul 3d");
            assert_eq!(dg.shape, vec![batch, m, n]);
            let want = cpu_batched_ref(&cpu, &a, &b, batch, m, k, n);
            assert_close("batched_matmul_3d", &host_f32(&cuda, &dg), &want, 1e-4);
        }

        // ── 4-D case: [2, 3, 6, 4] @ [2, 3, 4, 5] → [2, 3, 6, 5] ──
        // batch = 2*3 = 6, flattened by the leading-dim product.
        {
            let (b1, b2, m, k, n) = (2usize, 3usize, 6usize, 4usize, 5usize);
            let batch = b1 * b2;
            let a: Vec<f32> = (0..batch * m * k)
                .map(|i| (i as f32 * 0.017 - 0.5))
                .collect();
            let b: Vec<f32> = (0..batch * k * n)
                .map(|i| (i as f32 * 0.009 - 0.2))
                .collect();
            let ag = cuda.from_slice_f32(&a, &[b1, b2, m, k]).unwrap();
            let bg = cuda.from_slice_f32(&b, &[b1, b2, k, n]).unwrap();
            let dg = cuda.matmul(&ag, &bg).expect("cuda batched matmul 4d");
            assert_eq!(dg.shape, vec![b1, b2, m, n]);
            let want = cpu_batched_ref(&cpu, &a, &b, batch, m, k, n);
            assert_close("batched_matmul_4d", &host_f32(&cuda, &dg), &want, 1e-4);
        }
    }

    fn host_f32(b: &impl Backend, h: &TensorHandle) -> Vec<f32> {
        let bytes = b.copy_to_host(h).expect("copy_to_host");
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    fn assert_close(label: &str, g: &[f32], c: &[f32], tol: f32) {
        assert_eq!(g.len(), c.len(), "{label}: length mismatch");
        let mut max_abs = 0f32;
        for (gi, ci) in g.iter().zip(c.iter()) {
            let d = (gi - ci).abs();
            if d > max_abs {
                max_abs = d;
            }
        }
        // `<=` so callers can pass tol=0.0 for exact-equality ops
        // (e.g. transpose, which is pure indexing).
        assert!(
            max_abs <= tol,
            "{label}: max_abs_diff={max_abs} (tol={tol})\nGPU={g:?}\nCPU={c:?}"
        );
    }

    /// Covers all P104 kernels: add/sub/mul/div, scale, relu/gelu/sigmoid/tanh.
    /// Each compared against CpuBackend on the same input.
    #[test]
    fn elementwise_and_activations_gpu_match_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();

        let shape = vec![17usize]; // odd length forces tail thread to be no-op
        let a: Vec<f32> = (0..17).map(|i| (i as f32) * 0.3 - 2.5).collect();
        let b: Vec<f32> = (0..17).map(|i| ((i as f32) * 0.13).cos() + 0.5).collect();

        let g_a = cuda.from_slice_f32(&a, &shape).unwrap();
        let g_b = cuda.from_slice_f32(&b, &shape).unwrap();
        let c_a = cpu.from_slice_f32(&a, &shape).unwrap();
        let c_b = cpu.from_slice_f32(&b, &shape).unwrap();

        // Binary
        for (name, gf, cf) in [
            (
                "add",
                cuda.add(&g_a, &g_b).unwrap(),
                cpu.add(&c_a, &c_b).unwrap(),
            ),
            (
                "sub",
                cuda.sub(&g_a, &g_b).unwrap(),
                cpu.sub(&c_a, &c_b).unwrap(),
            ),
            (
                "mul",
                cuda.mul(&g_a, &g_b).unwrap(),
                cpu.mul(&c_a, &c_b).unwrap(),
            ),
            (
                "div",
                cuda.div(&g_a, &g_b).unwrap(),
                cpu.div(&c_a, &c_b).unwrap(),
            ),
        ] {
            assert_close(name, &host_f32(&cuda, &gf), &host_f32(&cpu, &cf), 1e-5);
        }

        // Unary
        for (name, gf, cf) in [
            ("relu", cuda.relu(&g_a).unwrap(), cpu.relu(&c_a).unwrap()),
            (
                "sigmoid",
                cuda.sigmoid(&g_a).unwrap(),
                cpu.sigmoid(&c_a).unwrap(),
            ),
            ("tanh", cuda.tanh(&g_a).unwrap(), cpu.tanh(&c_a).unwrap()),
            // GELU uses tanh-approx on both sides; small extra slack
            // for tanhf vs ndarray's mapv(.tanh()) library divergence.
            ("gelu", cuda.gelu(&g_a).unwrap(), cpu.gelu(&c_a).unwrap()),
        ] {
            let tol = if name == "gelu" { 1e-4 } else { 1e-5 };
            assert_close(name, &host_f32(&cuda, &gf), &host_f32(&cpu, &cf), tol);
        }

        // Scale
        let gs = cuda.scale(&g_a, 2.5).unwrap();
        let cs = cpu.scale(&c_a, 2.5).unwrap();
        assert_close("scale", &host_f32(&cuda, &gs), &host_f32(&cpu, &cs), 1e-5);

        // We launched 9 GPU ops; counter must be at least that.
        assert!(
            cuda.elementwise_gpu_count() >= 9,
            "expected ≥9 GPU elementwise dispatches, got {}",
            cuda.elementwise_gpu_count()
        );
    }

    /// P105: scalar reductions (sum/mean/max/min) + softmax (1D and 2D
    /// last-axis) — verify GPU matches CPU.
    #[test]
    fn reductions_and_softmax_gpu_match_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();

        // Scalar reductions: odd-length 1D vector with both signs.
        let shape1 = vec![21usize];
        let v: Vec<f32> = (0..21).map(|i| ((i as f32) - 10.0) * 0.37).collect();
        let g = cuda.from_slice_f32(&v, &shape1).unwrap();
        let c = cpu.from_slice_f32(&v, &shape1).unwrap();

        let baseline = cuda.elementwise_gpu_count();

        let gpu_sum = cuda.sum(&g).unwrap();
        let cpu_sum = cpu.sum(&c).unwrap();
        assert!((gpu_sum - cpu_sum).abs() < 1e-4, "sum gpu={gpu_sum} cpu={cpu_sum}");

        let gpu_mean = cuda.mean(&g).unwrap();
        let cpu_mean = cpu.mean(&c).unwrap();
        assert!(
            (gpu_mean - cpu_mean).abs() < 1e-5,
            "mean gpu={gpu_mean} cpu={cpu_mean}"
        );

        let gpu_max = cuda.max(&g).unwrap();
        let cpu_max = cpu.max(&c).unwrap();
        assert!((gpu_max - cpu_max).abs() < 1e-6, "max gpu={gpu_max} cpu={cpu_max}");

        let gpu_min = cuda.min(&g).unwrap();
        let cpu_min = cpu.min(&c).unwrap();
        assert!((gpu_min - cpu_min).abs() < 1e-6, "min gpu={gpu_min} cpu={cpu_min}");

        // Softmax 1D
        let g_sm1 = cuda.softmax(&g, -1).unwrap();
        let c_sm1 = cpu.softmax(&c, -1).unwrap();
        assert_close("softmax_1d", &host_f32(&cuda, &g_sm1), &host_f32(&cpu, &c_sm1), 1e-5);

        // Softmax 2D last-axis: [3, 5]. CpuBackend was fixed alongside
        // this phase (lanes_mut instead of axis_iter_mut), so we can
        // now compare directly. Also check rows sum to 1.
        let shape2 = vec![3usize, 5];
        let v2: Vec<f32> = (0..15)
            .map(|i| ((i as f32) * 0.21 - 1.5).cos() * 2.0)
            .collect();
        let g2 = cuda.from_slice_f32(&v2, &shape2).unwrap();
        let c2 = cpu.from_slice_f32(&v2, &shape2).unwrap();
        let g_sm2 = cuda.softmax(&g2, -1).unwrap();
        let c_sm2 = cpu.softmax(&c2, -1).unwrap();
        let gpu_out = host_f32(&cuda, &g_sm2);
        assert_close("softmax_2d_last", &gpu_out, &host_f32(&cpu, &c_sm2), 1e-5);
        for row in 0..3 {
            let s: f32 = gpu_out[row * 5..(row + 1) * 5].iter().sum();
            assert!((s - 1.0).abs() < 1e-5, "GPU row {row} sum={s}");
        }

        // sum, mean(=sum), max, min, softmax_1d, softmax_2d = 6 launches.
        let after = cuda.elementwise_gpu_count();
        assert!(
            after - baseline >= 6,
            "expected ≥6 new GPU dispatches, got {} (baseline={baseline})",
            after - baseline
        );
    }

    /// P107: axis reductions (sum_axis / mean_axis) along the last axis.
    /// Non-last axes still bounce; covered here for both 1D (axis 0)
    /// and 2D last-axis (axis 1).
    #[test]
    fn axis_reductions_lastdim_gpu_match_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();
        let baseline = cuda.elementwise_gpu_count();

        // 1D: sum_axis(0) collapses to a scalar tensor (shape []).
        {
            let v: Vec<f32> = (0..16).map(|i| ((i as f32) * 0.3 - 2.0).sin()).collect();
            let g = cuda.from_slice_f32(&v, &[16]).unwrap();
            let c = cpu.from_slice_f32(&v, &[16]).unwrap();
            let gs = cuda.sum_axis(&g, 0).unwrap();
            let cs = cpu.sum_axis(&c, 0).unwrap();
            assert_eq!(gs.shape, cs.shape);
            assert_close(
                "sum_axis_1d",
                &host_f32(&cuda, &gs),
                &host_f32(&cpu, &cs),
                1e-4,
            );
        }

        // 2D last-axis: [4, 7] reduced on axis=1 → shape [4].
        {
            let shape = vec![4usize, 7];
            let v: Vec<f32> = (0..28).map(|i| ((i as f32) * 0.11 - 1.0)).collect();
            let g = cuda.from_slice_f32(&v, &shape).unwrap();
            let c = cpu.from_slice_f32(&v, &shape).unwrap();

            let gs = cuda.sum_axis(&g, 1).unwrap();
            let cs = cpu.sum_axis(&c, 1).unwrap();
            assert_eq!(gs.shape, cs.shape);
            assert_close(
                "sum_axis_2d_last",
                &host_f32(&cuda, &gs),
                &host_f32(&cpu, &cs),
                1e-4,
            );

            let gm = cuda.mean_axis(&g, 1).unwrap();
            let cm = cpu.mean_axis(&c, 1).unwrap();
            assert_eq!(gm.shape, cm.shape);
            assert_close(
                "mean_axis_2d_last",
                &host_f32(&cuda, &gm),
                &host_f32(&cpu, &cm),
                1e-4,
            );
        }

        // Non-last axis: since P110 this is GPU-routed (general
        // [outer,axis,inner] kernel), NOT a bounce. Verify correctness
        // AND that the GPU counter bumped.
        let pre = cuda.elementwise_gpu_count();
        {
            let shape = vec![3usize, 4];
            let v: Vec<f32> = (0..12).map(|i| i as f32).collect();
            let g = cuda.from_slice_f32(&v, &shape).unwrap();
            let c = cpu.from_slice_f32(&v, &shape).unwrap();
            let gs = cuda.sum_axis(&g, 0).unwrap(); // non-last
            let cs = cpu.sum_axis(&c, 0).unwrap();
            assert_close(
                "sum_axis_2d_non_last",
                &host_f32(&cuda, &gs),
                &host_f32(&cpu, &cs),
                1e-4,
            );
        }
        assert!(
            cuda.elementwise_gpu_count() > pre,
            "non-last-axis sum_axis must now take the GPU path (P110)"
        );

        // 1D sum_axis + 2D sum_axis + 2D mean_axis (=2 launches) = 4
        let after = cuda.elementwise_gpu_count();
        assert!(
            after - baseline >= 4,
            "expected ≥4 new GPU dispatches, got {}",
            after - baseline
        );
    }

    /// P110: non-last-axis reductions (sum_axis/mean_axis) and softmax
    /// routed through the general `[outer, axis, inner]` kernels.
    /// Reference is CpuBackend, which handles arbitrary axes (softmax
    /// was fixed to use `lanes_mut`).
    #[test]
    fn nonlast_axis_reductions_and_softmax_gpu_match_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();

        // 2-D [4, 5], reduce/softmax along axis 0 (non-last, stride=5).
        {
            let shape = vec![4usize, 5];
            let v: Vec<f32> = (0..20).map(|i| ((i as f32) * 0.17 - 1.3).sin() * 1.5).collect();
            let g = cuda.from_slice_f32(&v, &shape).unwrap();
            let c = cpu.from_slice_f32(&v, &shape).unwrap();

            let gs = cuda.sum_axis(&g, 0).unwrap();
            let cs = cpu.sum_axis(&c, 0).unwrap();
            assert_eq!(gs.shape, cs.shape, "sum_axis0 shape");
            assert_close("sum_axis0", &host_f32(&cuda, &gs), &host_f32(&cpu, &cs), 1e-4);

            let gm = cuda.mean_axis(&g, 0).unwrap();
            let cm = cpu.mean_axis(&c, 0).unwrap();
            assert_eq!(gm.shape, cm.shape, "mean_axis0 shape");
            assert_close("mean_axis0", &host_f32(&cuda, &gm), &host_f32(&cpu, &cm), 1e-4);

            // softmax along axis 0: each of the 5 columns sums to 1.
            let gsm = cuda.softmax(&g, 0).unwrap();
            let csm = cpu.softmax(&c, 0).unwrap();
            assert_eq!(gsm.shape, vec![4, 5]);
            let go = host_f32(&cuda, &gsm);
            assert_close("softmax_axis0", &go, &host_f32(&cpu, &csm), 1e-5);
            for col in 0..5 {
                let s: f32 = (0..4).map(|r| go[r * 5 + col]).sum();
                assert!((s - 1.0).abs() < 1e-5, "GPU col {col} softmax sum={s}");
            }
        }

        // 3-D [2, 3, 4], reduce along the MIDDLE axis 1
        // (outer=2, axis=3, inner=4).
        {
            let shape = vec![2usize, 3, 4];
            let v: Vec<f32> = (0..24).map(|i| ((i as f32) * 0.09 + 0.5).cos()).collect();
            let g = cuda.from_slice_f32(&v, &shape).unwrap();
            let c = cpu.from_slice_f32(&v, &shape).unwrap();

            let gs = cuda.sum_axis(&g, 1).unwrap();
            let cs = cpu.sum_axis(&c, 1).unwrap();
            assert_eq!(gs.shape, cs.shape, "sum_axis1(3d) shape");
            assert_close("sum_axis1_3d", &host_f32(&cuda, &gs), &host_f32(&cpu, &cs), 1e-4);

            // softmax along middle axis: output keeps [2,3,4].
            let gsm = cuda.softmax(&g, 1).unwrap();
            let csm = cpu.softmax(&c, 1).unwrap();
            assert_eq!(gsm.shape, vec![2, 3, 4]);
            assert_close("softmax_axis1_3d", &host_f32(&cuda, &gsm), &host_f32(&cpu, &csm), 1e-5);
        }
    }

    /// P108: 2-D transpose. Non-square + non-trivial perm cases verify
    /// the index math; non-eligible shapes verify the bounce.
    #[test]
    fn transpose_2d_gpu_matches_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();
        let baseline = cuda.elementwise_gpu_count();

        // Non-square so an i↔j swap would land in different memory.
        let shape = vec![5usize, 9];
        let v: Vec<f32> = (0..45).map(|i| (i as f32) * 0.5 - 7.0).collect();
        let g = cuda.from_slice_f32(&v, &shape).unwrap();
        let c = cpu.from_slice_f32(&v, &shape).unwrap();

        let gt = cuda.transpose(&g, &[1, 0]).unwrap();
        let ct = cpu.transpose(&c, &[1, 0]).unwrap();
        assert_eq!(gt.shape, vec![9, 5]);
        assert_eq!(gt.shape, ct.shape);
        assert_close(
            "transpose_2d",
            &host_f32(&cuda, &gt),
            &host_f32(&cpu, &ct),
            0.0, // exact equality — transpose is pure indexing
        );

        // 3-D reverse perm — since P111 this is GPU-routed, not a
        // bounce. Verify correctness AND that the counter bumped.
        let shape3 = vec![2usize, 3, 4];
        let v3: Vec<f32> = (0..24).map(|i| i as f32).collect();
        let g3 = cuda.from_slice_f32(&v3, &shape3).unwrap();
        let c3 = cpu.from_slice_f32(&v3, &shape3).unwrap();
        let pre = cuda.elementwise_gpu_count();
        let gt3 = cuda.transpose(&g3, &[2, 1, 0]).unwrap();
        let ct3 = cpu.transpose(&c3, &[2, 1, 0]).unwrap();
        assert_eq!(gt3.shape, vec![4, 3, 2]);
        assert_close(
            "transpose_3d_perm",
            &host_f32(&cuda, &gt3),
            &host_f32(&cpu, &ct3),
            0.0,
        );
        assert!(
            cuda.elementwise_gpu_count() > pre,
            "3-D transpose must now take the GPU path (P111)"
        );

        // Verify the 2-D run did fire on GPU.
        assert!(
            cuda.elementwise_gpu_count() - baseline >= 1,
            "expected ≥1 new GPU dispatch from 2-D transpose"
        );
    }

    /// P111: arbitrary-rank permute via the general stride-aware kernel.
    /// Covers a non-trivial 3-D perm (axis cycle) and a 4-D perm, both
    /// against CpuBackend (exact equality — pure indexing).
    #[test]
    fn permute_nd_gpu_matches_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();

        // 3-D [2,3,4], cyclic perm [1,2,0] → output [3,4,2].
        {
            let shape = vec![2usize, 3, 4];
            let v: Vec<f32> = (0..24).map(|i| i as f32 * 0.5 - 3.0).collect();
            let g = cuda.from_slice_f32(&v, &shape).unwrap();
            let c = cpu.from_slice_f32(&v, &shape).unwrap();
            let gt = cuda.transpose(&g, &[1, 2, 0]).unwrap();
            let ct = cpu.transpose(&c, &[1, 2, 0]).unwrap();
            assert_eq!(gt.shape, vec![3, 4, 2]);
            assert_eq!(gt.shape, ct.shape);
            assert_close("permute_3d_120", &host_f32(&cuda, &gt), &host_f32(&cpu, &ct), 0.0);
        }

        // 4-D [2,3,4,5], perm [0,2,3,1] (e.g. NCHW→NHWC-style) → [2,4,5,3].
        {
            let shape = vec![2usize, 3, 4, 5];
            let v: Vec<f32> = (0..120).map(|i| (i as f32 * 0.13).sin()).collect();
            let g = cuda.from_slice_f32(&v, &shape).unwrap();
            let c = cpu.from_slice_f32(&v, &shape).unwrap();
            let gt = cuda.transpose(&g, &[0, 2, 3, 1]).unwrap();
            let ct = cpu.transpose(&c, &[0, 2, 3, 1]).unwrap();
            assert_eq!(gt.shape, vec![2, 4, 5, 3]);
            assert_eq!(gt.shape, ct.shape);
            assert_close("permute_4d_0231", &host_f32(&cuda, &gt), &host_f32(&cpu, &ct), 0.0);
        }
    }

    /// P112: cuRAND `rand` (uniform) / `randn` (normal). The GPU RNG
    /// won't match CpuBackend value-for-value, so we check distribution
    /// properties, GPU residency, shape, and the odd-length truncate
    /// path. Holds whether the real cuRAND path or the CPU fallback ran.
    #[test]
    fn rng_gpu_uniform_and_normal() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };

        let n = 100_000usize;
        // ── uniform [0, 1) ──
        let u = cuda.rand(&[n], DType::F32).expect("rand");
        assert_eq!(u.shape, vec![n]);
        assert!(matches!(u.backend, BackendType::Cuda), "rand must be GPU-resident");
        let uh = host_f32(&cuda, &u);
        assert_eq!(uh.len(), n);
        let (mut umin, mut umax, mut usum) = (f32::INFINITY, f32::NEG_INFINITY, 0.0f64);
        for &x in &uh {
            assert!(x.is_finite(), "uniform produced non-finite {x}");
            assert!((0.0..1.0).contains(&x), "uniform out of [0,1): {x}");
            umin = umin.min(x);
            umax = umax.max(x);
            usum += x as f64;
        }
        let umean = usum / n as f64;
        // 100k samples: mean of U(0,1) ≈ 0.5; spread should cover the range.
        assert!((umean - 0.5).abs() < 0.02, "uniform mean={umean} (want ≈0.5)");
        assert!(umin < 0.05 && umax > 0.95, "uniform range [{umin},{umax}] too narrow");

        // Two successive draws must differ (independent seeds per call).
        let u2 = cuda.rand(&[n], DType::F32).expect("rand2");
        let u2h = host_f32(&cuda, &u2);
        let identical = uh.iter().zip(&u2h).all(|(a, b)| a == b);
        assert!(!identical, "two rand() calls returned identical data");

        // ── normal(0, 1) ──
        let g = cuda.randn(&[n], DType::F32).expect("randn");
        assert_eq!(g.shape, vec![n]);
        assert!(matches!(g.backend, BackendType::Cuda), "randn must be GPU-resident");
        let gh = host_f32(&cuda, &g);
        let mut gsum = 0.0f64;
        for &x in &gh {
            assert!(x.is_finite(), "normal produced non-finite {x}");
            gsum += x as f64;
        }
        let gmean = gsum / n as f64;
        let gvar = gh.iter().map(|&x| {
            let d = x as f64 - gmean;
            d * d
        }).sum::<f64>() / n as f64;
        let gstd = gvar.sqrt();
        assert!(gmean.abs() < 0.03, "normal mean={gmean} (want ≈0)");
        assert!((gstd - 1.0).abs() < 0.05, "normal std={gstd} (want ≈1)");

        // ── odd length exercises the even-alloc + truncate path ──
        let odd = cuda.randn(&[15], DType::F32).expect("randn odd");
        assert_eq!(odd.shape, vec![15]);
        let oh = host_f32(&cuda, &odd);
        assert_eq!(oh.len(), 15);
        assert!(oh.iter().all(|x| x.is_finite()), "odd-length normal has non-finite");
    }

    /// P113: conv2d via GPU im2col + cuBLASLt GEMM, checked against the
    /// CpuBackend reference for several stride/padding configs.
    #[test]
    fn conv2d_gpu_matches_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();

        // (N, Cin, H, W, Cout, KH, KW, stride, pad, dilation)
        let cases = [
            (2usize, 3, 5, 5, 4, 3, 3, 1, 1, 1), // same-size (pad=1, stride=1)
            (1, 2, 6, 6, 3, 3, 3, 2, 0, 1),      // strided, no pad
            (2, 4, 7, 5, 5, 3, 3, 1, 0, 1),      // valid conv, non-square HxW
            (1, 1, 4, 4, 2, 2, 2, 2, 1, 1),      // 2x2 kernel, stride 2, pad 1
            (1, 3, 9, 9, 4, 3, 3, 1, 2, 2),      // dilation=2, pad=2 (atrous)
            (2, 2, 11, 11, 3, 3, 3, 2, 2, 3),    // dilation=3, stride=2, pad=2
        ];

        for (ci, &(n, cin, h, w, cout, kh, kw, stride, pad, dil)) in cases.iter().enumerate() {
            let xn = n * cin * h * w;
            let wn = cout * cin * kh * kw;
            let x: Vec<f32> = (0..xn).map(|i| ((i as f32) * 0.037 - 1.3).sin()).collect();
            let wt: Vec<f32> = (0..wn).map(|i| ((i as f32) * 0.021 + 0.2).cos() * 0.5).collect();

            let gx = cuda.from_slice_f32(&x, &[n, cin, h, w]).unwrap();
            let gw = cuda.from_slice_f32(&wt, &[cout, cin, kh, kw]).unwrap();
            let cx = cpu.from_slice_f32(&x, &[n, cin, h, w]).unwrap();
            let cw = cpu.from_slice_f32(&wt, &[cout, cin, kh, kw]).unwrap();

            let mm_before = cuda.matmul_gpu_count();
            let gy = cuda.conv2d(&gx, &gw, stride, pad, dil).expect("cuda conv2d");
            let cy = cpu.conv2d(&cx, &cw, stride, pad, dil).expect("cpu conv2d");

            let eff = dil * (kh - 1) + 1;
            let eff_w = dil * (kw - 1) + 1;
            let hout = (h + 2 * pad - eff) / stride + 1;
            let wout = (w + 2 * pad - eff_w) / stride + 1;
            assert_eq!(gy.shape, vec![n, cout, hout, wout], "case {ci} shape");
            assert_eq!(gy.shape, cy.shape, "case {ci} gpu/cpu shape");
            assert_close(
                &format!("conv2d_case{ci}"),
                &host_f32(&cuda, &gy),
                &host_f32(&cpu, &cy),
                2e-3,
            );
            // The GEMM step must have fired on the GPU (proves the
            // im2col+GEMM path ran, not the CPU bounce).
            assert!(
                cuda.matmul_gpu_count() > mm_before,
                "case {ci}: conv2d GEMM did not take the GPU path"
            );
        }
    }

    /// P118: F16/BF16 round-trip. f32 → half → f32 must recover the
    /// original within each format's precision. Exercises the separate
    /// fp16 NVRTC module + half GPU storage.
    #[test]
    fn half_roundtrip_f16_bf16() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let v: Vec<f32> = (0..64).map(|i| (i as f32 * 0.05 - 1.6)).collect();
        let src = cuda.from_slice_f32(&v, &[8, 8]).unwrap();

        // F16: ~10-bit mantissa → rel err ≲ 1e-3 for O(1) values.
        let h16 = cuda.to_half_gpu(&src, DType::F16).expect("to f16");
        assert_eq!(h16.dtype, DType::F16);
        assert_eq!(h16.shape, vec![8, 8]);
        assert_eq!(h16.size_bytes, 64 * 2);
        let back16 = cuda.half_to_f32_gpu(&h16).expect("f16 back");
        assert_eq!(back16.dtype, DType::F32);
        let r16 = host_f32(&cuda, &back16);
        for (a, b) in v.iter().zip(&r16) {
            assert!((a - b).abs() <= 2e-3, "f16 roundtrip {a} vs {b}");
        }

        // BF16: 8-bit mantissa → coarser; rel err ≲ 1e-2.
        let hbf = cuda.to_half_gpu(&src, DType::BF16).expect("to bf16");
        assert_eq!(hbf.dtype, DType::BF16);
        let backbf = cuda.half_to_f32_gpu(&hbf).expect("bf16 back");
        let rbf = host_f32(&cuda, &backbf);
        for (a, b) in v.iter().zip(&rbf) {
            assert!((a - b).abs() <= 2e-2, "bf16 roundtrip {a} vs {b}");
        }

        // copy_to_host on a half handle returns the raw 2-byte payload.
        let raw = cuda.copy_to_host(&h16).unwrap();
        assert_eq!(raw.len(), 64 * 2);
    }

    /// P118: half-precision tensor-core matmul. F16/BF16 inputs,
    /// F32-accumulated result, checked against the F32 cuBLASLt matmul
    /// within each format's precision. The `matmul` trait method routes
    /// same-dtype 2-D half operands to the tensor-core path.
    #[test]
    fn half_matmul_matches_f32() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let (m, k, n) = (6usize, 8usize, 5usize);
        // Small magnitudes so half rounding stays well-controlled.
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.05).sin() * 0.5).collect();
        let b: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.07).cos() * 0.5).collect();

        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[k, n]).unwrap();
        let ref_f32 = cuda.matmul(&af, &bf).unwrap(); // F32 cuBLASLt reference
        let want = host_f32(&cuda, &ref_f32);

        // ── F16 ──
        let mm_before = cuda.matmul_gpu_count();
        let a16 = cuda.to_half_gpu(&af, DType::F16).unwrap();
        let b16 = cuda.to_half_gpu(&bf, DType::F16).unwrap();
        let y16 = cuda.matmul(&a16, &b16).unwrap();
        assert_eq!(y16.dtype, DType::F32, "half matmul accumulates to F32");
        assert_eq!(y16.shape, vec![m, n]);
        assert!(cuda.matmul_gpu_count() > mm_before, "half matmul took GPU path");
        assert_close("f16_matmul", &host_f32(&cuda, &y16), &want, 1e-2);

        // ── BF16 (coarser mantissa → looser tol) ──
        let abf = cuda.to_half_gpu(&af, DType::BF16).unwrap();
        let bbf = cuda.to_half_gpu(&bf, DType::BF16).unwrap();
        let ybf = cuda.matmul(&abf, &bbf).unwrap();
        assert_eq!(ybf.shape, vec![m, n]);
        assert_close("bf16_matmul", &host_f32(&cuda, &ybf), &want, 5e-2);
    }

    /// P118: half elementwise arithmetic via upcast→F32→downcast. The
    /// result stays half-typed; checked against an F32 reference.
    #[test]
    fn half_elementwise_add_mul() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let a: Vec<f32> = (0..32).map(|i| (i as f32) * 0.05 - 0.8).collect();
        let b: Vec<f32> = (0..32).map(|i| ((i as f32) * 0.09).sin() * 0.5).collect();
        let af = cuda.from_slice_f32(&a, &[4, 8]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[4, 8]).unwrap();
        let want_add = host_f32(&cuda, &cuda.add(&af, &bf).unwrap());
        let want_mul = host_f32(&cuda, &cuda.mul(&af, &bf).unwrap());

        for &dt in &[DType::F16, DType::BF16] {
            let tol = if dt == DType::F16 { 1e-2 } else { 4e-2 };
            let ah = cuda.to_half_gpu(&af, dt).unwrap();
            let bh = cuda.to_half_gpu(&bf, dt).unwrap();

            let s = cuda.add(&ah, &bh).unwrap();
            assert_eq!(s.dtype, dt, "half add keeps dtype");
            assert_eq!(s.shape, vec![4, 8]);
            let s_f32 = cuda.half_to_f32_gpu(&s).unwrap();
            assert_close(&format!("{dt:?}_add"), &host_f32(&cuda, &s_f32), &want_add, tol);

            let p = cuda.mul(&ah, &bh).unwrap();
            assert_eq!(p.dtype, dt, "half mul keeps dtype");
            let p_f32 = cuda.half_to_f32_gpu(&p).unwrap();
            assert_close(&format!("{dt:?}_mul"), &host_f32(&cuda, &p_f32), &want_mul, tol);
        }
    }

    /// P118: broad half-precision op coverage — activations, softmax,
    /// scale, reductions, transpose, conv2d — all via the upcast→F32→
    /// downcast pattern. Each checked against its F32 reference. This is
    /// enough for a half forward pass.
    #[test]
    fn half_broad_ops() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let dt = DType::F16;
        let tol = 1e-2;
        let v: Vec<f32> = (0..24).map(|i| (i as f32) * 0.13 - 1.5).collect();
        let f = cuda.from_slice_f32(&v, &[4, 6]).unwrap();
        let h = cuda.to_half_gpu(&f, dt).unwrap();

        // Helper: run a unary op on both f32 and half, compare.
        let chk = |name: &str, gf: &TensorHandle, gh: &TensorHandle, tol: f32| {
            assert_eq!(gh.dtype, dt, "{name}: result keeps half dtype");
            let back = cuda.half_to_f32_gpu(gh).unwrap();
            assert_close(name, &host_f32(&cuda, &back), &host_f32(&cuda, gf), tol);
        };

        chk("relu", &cuda.relu(&f).unwrap(), &cuda.relu(&h).unwrap(), tol);
        chk("gelu", &cuda.gelu(&f).unwrap(), &cuda.gelu(&h).unwrap(), tol);
        chk("sigmoid", &cuda.sigmoid(&f).unwrap(), &cuda.sigmoid(&h).unwrap(), tol);
        chk("scale", &cuda.scale(&f, 2.5).unwrap(), &cuda.scale(&h, 2.5).unwrap(), tol);
        chk("softmax", &cuda.softmax(&f, -1).unwrap(), &cuda.softmax(&h, -1).unwrap(), tol);
        chk("sum_axis", &cuda.sum_axis(&f, 1).unwrap(), &cuda.sum_axis(&h, 1).unwrap(), tol);
        chk("transpose", &cuda.transpose(&f, &[1, 0]).unwrap(), &cuda.transpose(&h, &[1, 0]).unwrap(), tol);

        // Scalar reductions return f64 directly.
        assert!((cuda.sum(&h).unwrap() - cuda.sum(&f).unwrap()).abs() < 5e-2, "half sum");
        assert!((cuda.max(&h).unwrap() - cuda.max(&f).unwrap()).abs() < 1e-2, "half max");

        // conv2d: half in → half out. [1,2,5,5] * [3,2,3,3], pad 1.
        let xn = 1 * 2 * 5 * 5;
        let wn = 3 * 2 * 3 * 3;
        let xv: Vec<f32> = (0..xn).map(|i| (i as f32 * 0.03).sin()).collect();
        let wv: Vec<f32> = (0..wn).map(|i| (i as f32 * 0.05).cos() * 0.3).collect();
        let xf = cuda.from_slice_f32(&xv, &[1, 2, 5, 5]).unwrap();
        let wf = cuda.from_slice_f32(&wv, &[3, 2, 3, 3]).unwrap();
        let yf = cuda.conv2d(&xf, &wf, 1, 1, 1).unwrap();
        let xh = cuda.to_half_gpu(&xf, dt).unwrap();
        let wh = cuda.to_half_gpu(&wf, dt).unwrap();
        let yh = cuda.conv2d(&xh, &wh, 1, 1, 1).unwrap();
        assert_eq!(yh.dtype, dt, "half conv keeps dtype");
        assert_eq!(yh.shape, vec![1, 3, 5, 5]);
        chk("conv2d", &yf, &yh, 3e-2);
    }

    /// P119: `cast` trait method. CUDA and CPU casts must produce the
    /// SAME half-precision bytes (validates the CUDA `__float2half` path
    /// agrees with the CPU `half` crate), and round-trip must recover.
    #[test]
    fn cast_cuda_matches_cpu_bytes() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();
        let v: Vec<f32> = (0..32).map(|i| (i as f32) * 0.137 - 2.1).collect();

        for dt in [DType::F16, DType::BF16] {
            let gf = cuda.from_slice_f32(&v, &[4, 8]).unwrap();
            let cf = cpu.from_slice_f32(&v, &[4, 8]).unwrap();
            let gh = cuda.cast(&gf, dt).unwrap();
            let ch = cpu.cast(&cf, dt).unwrap();
            assert_eq!(gh.dtype, dt);
            // Raw 16-bit payloads must match bit-for-bit between backends.
            assert_eq!(
                cuda.copy_to_host(&gh).unwrap(),
                cpu.copy_to_host(&ch).unwrap(),
                "{dt:?}: CUDA and CPU cast bytes differ"
            );
            // Round-trip back to F32 on CUDA recovers the value.
            let back = cuda.cast(&gh, DType::F32).unwrap();
            assert_eq!(back.dtype, DType::F32);
            let rv = host_f32(&cuda, &back);
            let tol = if dt == DType::F16 { 2e-3 } else { 1e-2 };
            for (a, r) in v.iter().zip(&rv) {
                assert!((a - r).abs() <= tol, "{dt:?} roundtrip {a} vs {r}");
            }
        }

        // F16 → BF16 direct cast goes through F32 internally.
        let gf = cuda.from_slice_f32(&v, &[4, 8]).unwrap();
        let g16 = cuda.cast(&gf, DType::F16).unwrap();
        let gbf = cuda.cast(&g16, DType::BF16).unwrap();
        assert_eq!(gbf.dtype, DType::BF16);
    }

    /// P121: INT8 quantization — quantize/dequantize round-trip and a
    /// quantized matmul checked against the F32 reference within
    /// quantization error.
    #[test]
    fn int8_quantize_and_matmul() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };

        // ── quantize/dequantize round-trip ──
        let v: Vec<f32> = (0..64).map(|i| (i as f32) * 0.05 - 1.6).collect();
        let f = cuda.from_slice_f32(&v, &[8, 8]).unwrap();
        let (q, scale) = cuda.quantize_i8(&f).unwrap();
        assert_eq!(q.dtype, DType::I8);
        assert_eq!(q.shape, vec![8, 8]);
        assert_eq!(q.size_bytes, 64, "int8 = 1 byte/elem (4× smaller than f32)");
        // amax of v is ~1.6 (at i=0: -1.6) → scale ≈ 1.6/127.
        assert!((scale - 1.6 / 127.0).abs() < 1e-3, "scale={scale}");
        let deq = cuda.dequantize_i8(&q, scale).unwrap();
        let r = host_f32(&cuda, &deq);
        // Per-element error bounded by half a quantization step.
        for (a, b) in v.iter().zip(&r) {
            assert!((a - b).abs() <= scale, "dequant {a} vs {b} (step {scale})");
        }
        // Raw i8 payload is numel bytes.
        assert_eq!(cuda.copy_to_host(&q).unwrap().len(), 64);

        // ── quantized matmul vs F32 reference ──
        let (m, k, n) = (5usize, 9usize, 4usize);
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.07).sin() * 0.8).collect();
        let b: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.05).cos() * 0.8).collect();
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[k, n]).unwrap();
        let want = host_f32(&cuda, &cuda.matmul(&af, &bf).unwrap());

        let (aq, sa) = cuda.quantize_i8(&af).unwrap();
        let (bq, sb) = cuda.quantize_i8(&bf).unwrap();
        let yq = cuda.matmul_i8(&aq, sa, &bq, sb).unwrap();
        assert_eq!(yq.dtype, DType::F32, "i8 matmul dequantizes to F32");
        assert_eq!(yq.shape, vec![m, n]);
        let got = host_f32(&cuda, &yq);
        // INT8 GEMM error grows with K; tolerance scales accordingly.
        let tol = (sa.max(sb)) * (k as f32) * 0.5 + 5e-2;
        for (w, g) in want.iter().zip(&got) {
            assert!((w - g).abs() <= tol, "i8 matmul {w} vs {g} (tol {tol})");
        }
    }

    /// P122: per-channel INT8 quantization. When B's columns differ
    /// wildly in magnitude, per-channel scales are far more accurate than
    /// a single per-tensor scale — verify per-channel matmul error is
    /// much smaller than per-tensor's against the F32 reference.
    #[test]
    fn int8_perchannel_beats_pertensor() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let (m, k, n) = (4usize, 6usize, 5usize);
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.11).sin() * 0.5).collect();
        // B columns span 3 orders of magnitude: column c scaled by 10^(c-2).
        let mut b = vec![0.0f32; k * n];
        for r in 0..k {
            for c in 0..n {
                let mag = 10f32.powi(c as i32 - 2); // 0.01 .. 100
                b[r * n + c] = ((r * n + c) as f32 * 0.3).cos() * mag;
            }
        }
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[k, n]).unwrap();
        let want = host_f32(&cuda, &cuda.matmul(&af, &bf).unwrap());

        let (aq, sa) = cuda.quantize_i8(&af).unwrap();

        // Per-tensor B quantization (one scale dominated by the big column).
        let (bq_pt, sb_pt) = cuda.quantize_i8(&bf).unwrap();
        let got_pt = host_f32(&cuda, &cuda.matmul_i8(&aq, sa, &bq_pt, sb_pt).unwrap());

        // Per-channel B quantization.
        let (bq_pc, sb_pc) = cuda.quantize_i8_perchannel(&bf).unwrap();
        assert_eq!(sb_pc.len(), n);
        let got_pc = host_f32(&cuda, &cuda.matmul_i8_perchannel(&aq, sa, &bq_pc, &sb_pc).unwrap());

        // Relative error over all outputs.
        let rel = |got: &[f32]| -> f32 {
            let mut num = 0.0f32;
            let mut den = 0.0f32;
            for (w, g) in want.iter().zip(got) {
                num += (w - g).abs();
                den += w.abs();
            }
            num / den.max(1e-9)
        };
        let err_pt = rel(&got_pt);
        let err_pc = rel(&got_pc);
        // Per-channel should be dramatically better (small columns survive).
        assert!(
            err_pc < err_pt * 0.5,
            "per-channel ({err_pc}) not better than per-tensor ({err_pt})"
        );
        assert!(err_pc < 0.05, "per-channel rel err {err_pc} too high");
    }

    /// P124: `quantized_matmul` caches the per-channel-quantized weight
    /// by source handle id, so repeated forward passes (same weight)
    /// quantize it only once. Result must stay identical across calls.
    #[test]
    fn quantized_matmul_caches_weight() {
        use rmi::compute::Backend as _;
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        // IMMA-eligible dims (multiples of 4).
        let (m, k, n) = (4usize, 8usize, 8usize);
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.1).sin() * 0.6).collect();
        let w: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.07).cos() * 0.6).collect();
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        // ONE weight handle reused across passes (as the pipeline does).
        let wf = cuda.from_slice_f32(&w, &[k, n]).unwrap();

        let hits0 = cuda.quant_cache_hits();
        let imma0 = cuda.quant_imma_count();
        let y1 = cuda.quantized_matmul(&af, &wf).unwrap();
        // First call: cache miss (weight quantized), no hit yet.
        assert_eq!(cuda.quant_cache_hits(), hits0, "first call should miss");
        let r1 = host_f32(&cuda, &y1);

        // Subsequent calls reuse the cached quantized weight.
        for _ in 0..3 {
            let y = cuda.quantized_matmul(&af, &wf).unwrap();
            let r = host_f32(&cuda, &y);
            assert_eq!(r1, r, "cached-weight result must be identical");
        }
        assert_eq!(cuda.quant_cache_hits(), hits0 + 3, "3 cache hits expected");
        // All 4 calls must have hit the real IMMA tensor-core path.
        assert_eq!(cuda.quant_imma_count(), imma0 + 4, "IMMA path must have run");
    }

    /// P126: cuBLASLt IMMA INT8 matmul matches the F32 reference within
    /// quantization error, and is exercised through the explicit API.
    #[test]
    fn int8_immma_matches_f32() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        // Multiples of 4.
        let (m, k, n) = (8usize, 16usize, 12usize);
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.05).sin() * 0.7).collect();
        let b: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.04).cos() * 0.7).collect();
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[k, n]).unwrap();
        let want = host_f32(&cuda, &cuda.matmul(&af, &bf).unwrap());

        let (aq, sa) = cuda.quantize_i8(&af).unwrap();
        let (bq_t, sb) = cuda.quantize_i8_perchannel_t(&bf).unwrap();
        assert_eq!(bq_t.shape, vec![n, k], "transposed weight is [N,K]");
        assert_eq!(sb.len(), n);

        let imma0 = cuda.quant_imma_count();
        let yq = cuda.matmul_i8_immma(&aq, sa, &bq_t, &sb).unwrap();
        assert_eq!(cuda.quant_imma_count(), imma0 + 1, "IMMA path ran");
        assert_eq!(yq.dtype, DType::F32);
        assert_eq!(yq.shape, vec![m, n]);
        let got = host_f32(&cuda, &yq);
        let tol = (sa.max(sb.iter().cloned().fold(0.0, f32::max))) * (k as f32) * 0.5 + 5e-2;
        for (w, g) in want.iter().zip(&got) {
            assert!((w - g).abs() <= tol, "IMMA i8 matmul {w} vs {g} (tol {tol})");
        }
    }

    /// P137: per-channel INT4 weights × INT8 activations (W4A8) — the
    /// production LLM-style mix (weights dominate memory).
    #[test]
    fn int4_weights_w4a8_matmul() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let (m, k, n) = (4usize, 8usize, 5usize); // k*n odd-ish: 40 → even; use n=5 for non-square
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.13).sin() * 0.9).collect();
        let w: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.07).cos() * 0.9).collect();
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let wf = cuda.from_slice_f32(&w, &[k, n]).unwrap();
        let want = host_f32(&cuda, &cuda.matmul(&af, &wf).unwrap());

        let (wq, sb) = cuda.quantize_i4_perchannel(&wf).unwrap();
        assert_eq!(wq.dtype, DType::I4);
        assert_eq!(wq.size_bytes, (k * n).div_ceil(2), "packed 2/byte");
        assert_eq!(sb.len(), n);
        let (aq, sa) = cuda.quantize_i8(&af).unwrap();

        let y = cuda.matmul_i8a_i4b(&aq, sa, &wq, &sb).unwrap();
        assert_eq!(y.shape, vec![m, n]);
        let got = host_f32(&cuda, &y);
        // INT4 weights: error dominated by the 4-bit weight step.
        let max_sb = sb.iter().cloned().fold(0.0f32, f32::max);
        let tol = max_sb * (k as f32) * 0.6 + 0.05;
        for (w0, g) in want.iter().zip(&got) {
            assert!((w0 - g).abs() <= tol, "w4a8 {w0} vs {g} (tol {tol})");
        }
    }

    #[test]
    fn int4_quantize_roundtrip_and_matmul() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        // ── round-trip (odd length exercises the tail nibble) ──
        let v: Vec<f32> = (0..33).map(|i| (i as f32) * 0.1 - 1.6).collect();
        let f = cuda.from_slice_f32(&v, &[33]).unwrap();
        let (q, scale) = cuda.quantize_i4(&f).unwrap();
        assert_eq!(q.dtype, DType::I4);
        assert_eq!(q.shape, vec![33]);
        assert_eq!(q.size_bytes, 17, "33 nibbles → 17 packed bytes (8× vs f32)");
        let back = cuda.dequantize_i4(&q, scale).unwrap();
        let r = host_f32(&cuda, &back);
        for (a, b) in v.iter().zip(&r) {
            assert!((a - b).abs() <= scale * 0.5 + 1e-6, "i4 roundtrip {a} vs {b} (step {scale})");
        }

        // ── W8A4 matmul vs F32 reference ──
        let (m, k, n) = (4usize, 8usize, 6usize);
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.17).sin() * 0.9).collect();
        let b: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.11).cos() * 0.9).collect();
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[k, n]).unwrap();
        let want = host_f32(&cuda, &cuda.matmul(&af, &bf).unwrap());

        let (aq, sa) = cuda.quantize_i4(&af).unwrap();
        let (bq, sb) = cuda.quantize_i8_perchannel(&bf).unwrap();
        let y = cuda.matmul_i4a_i8b(&aq, sa, &bq, &sb).unwrap();
        assert_eq!(y.shape, vec![m, n]);
        let got = host_f32(&cuda, &y);
        // 4-bit activations: ~16 levels → coarse; tolerance scales with
        // the i4 step and K.
        let tol = sa * (k as f32) * 0.6 + 0.05;
        for (w, g) in want.iter().zip(&got) {
            assert!((w - g).abs() <= tol, "i4 matmul {w} vs {g} (tol {tol})");
        }
    }

    /// P134: asymmetric-activation INT8 on the IMMA tensor-core path.
    /// On post-ReLU (all-positive) activations it matches F32 closely and
    /// beats symmetric IMMA (which wastes the negative int8 range).
    #[test]
    fn int8_immma_asym_matches_f32() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        // Mult-of-4 dims for IMMA.
        let (m, k, n) = (8usize, 16usize, 12usize);
        // Post-ReLU activations in [0, 4].
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32 * 0.31).sin() * 0.5 + 0.5) * 4.0).collect();
        let b: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.05).cos() * 0.7).collect();
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[k, n]).unwrap();
        let want = host_f32(&cuda, &cuda.matmul(&af, &bf).unwrap());

        let (bq_t, sb) = cuda.quantize_i8_perchannel_t(&bf).unwrap();
        let (lo, hi) = a.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| {
            (l.min(x), h.max(x))
        });
        let (aq, sa, za) = cuda.quantize_i8_asym(&af, lo, hi).unwrap();

        let imma0 = cuda.quant_imma_count();
        let y = cuda.matmul_i8_immma_asym(&aq, sa, za, &bq_t, &sb).unwrap();
        assert_eq!(cuda.quant_imma_count(), imma0 + 1, "asym IMMA path ran");
        assert_eq!(y.dtype, DType::F32);
        assert_eq!(y.shape, vec![m, n]);
        let got = host_f32(&cuda, &y);
        let (mut num, mut den) = (0.0f32, 0.0f32);
        for (w, g) in want.iter().zip(&got) {
            num += (w - g).abs();
            den += w.abs();
        }
        assert!(num / den.max(1e-9) < 0.03, "asym IMMA rel err too high: {}", num / den.max(1e-9));
    }

    /// P128: calibrated INT8 matmul (given activation scale, no amax)
    /// matches the F32 reference within quant error and runs IMMA.
    #[test]
    fn int8_calibrated_matmul_matches_f32() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let (m, k, n) = (8usize, 16usize, 12usize);
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.05).sin() * 0.7).collect();
        let b: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.04).cos() * 0.7).collect();
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[k, n]).unwrap();
        let want = host_f32(&cuda, &cuda.matmul(&af, &bf).unwrap());

        // Calibrated activation scale = amax/127 (here computed once;
        // in practice it would come from offline calibration).
        let amax = a.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        let sa = amax / 127.0;

        let imma0 = cuda.quant_imma_count();
        let y = cuda.quantized_matmul_calibrated(&af, sa, &bf).unwrap();
        assert_eq!(cuda.quant_imma_count(), imma0 + 1, "calibrated path uses IMMA");
        assert_eq!(y.dtype, DType::F32);
        assert_eq!(y.shape, vec![m, n]);
        let got = host_f32(&cuda, &y);
        // Should match the dynamic-quant result closely (same scales).
        let tol = sa * (k as f32) * 0.5 + 5e-2;
        for (w, g) in want.iter().zip(&got) {
            assert!((w - g).abs() <= tol, "calibrated i8 {w} vs {g} (tol {tol})");
        }
    }

    /// P131: asymmetric (zero-point) INT8 on non-negative (post-ReLU)
    /// activations is MORE accurate than symmetric, because symmetric
    /// wastes the negative half of the int8 range. Both vs F32 reference.
    #[test]
    fn int8_asymmetric_beats_symmetric_on_relu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let (m, k, n) = (4usize, 8usize, 6usize);
        // Activations all in [0, 4] (post-ReLU): symmetric maps to only
        // ~128 levels, asymmetric to all 256.
        let a: Vec<f32> = (0..m * k).map(|i| ((i as f32 * 0.37).sin() * 0.5 + 0.5) * 4.0).collect();
        let b: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.07).cos() * 0.6).collect();
        let af = cuda.from_slice_f32(&a, &[m, k]).unwrap();
        let bf = cuda.from_slice_f32(&b, &[k, n]).unwrap();
        let want = host_f32(&cuda, &cuda.matmul(&af, &bf).unwrap());

        // Weight: symmetric per-channel (shared by both).
        let (bq, sb) = cuda.quantize_i8_perchannel(&bf).unwrap();

        // Symmetric activation quant.
        let (aq_s, sa_s) = cuda.quantize_i8(&af).unwrap();
        let got_s = host_f32(&cuda, &cuda.matmul_i8_perchannel(&aq_s, sa_s, &bq, &sb).unwrap());

        // Asymmetric activation quant over the true [0,4] range.
        let (lo, hi) = a.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| {
            (l.min(x), h.max(x))
        });
        let (aq_a, sa_a, za) = cuda.quantize_i8_asym(&af, lo, hi).unwrap();
        let got_a = host_f32(&cuda, &cuda.matmul_i8_asym(&aq_a, sa_a, za, &bq, &sb).unwrap());

        let rel = |got: &[f32]| -> f32 {
            let (mut num, mut den) = (0.0f32, 0.0f32);
            for (w, g) in want.iter().zip(got) {
                num += (w - g).abs();
                den += w.abs();
            }
            num / den.max(1e-9)
        };
        let err_s = rel(&got_s);
        let err_a = rel(&got_a);
        assert!(
            err_a < err_s,
            "asymmetric ({err_a}) should beat symmetric ({err_s}) on post-ReLU acts"
        );
        assert!(err_a < 0.02, "asymmetric rel err {err_a} too high");
    }

    /// P-storage: a multi-op chain must stay on the GPU end-to-end.
    /// `bounce_count` stays at 0 (no per-op roundtrip), GPU storage
    /// grows by N for N intermediates, final result matches CPU chain.
    #[test]
    fn multi_op_chain_stays_on_gpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();
        let bounces_before = cuda.bounce_count();
        let storage_before = cuda.gpu_storage_len();

        let a_v: Vec<f32> = (0..12).map(|i| (i as f32) * 0.1 - 0.5).collect();
        let b_v: Vec<f32> = (0..12).map(|i| ((i as f32) * 0.07).sin()).collect();
        let c_v: Vec<f32> = (0..9).map(|i| (i as f32) * 0.2).collect();

        let do_chain = |b: &dyn Backend| {
            let a = b.from_slice_f32(&a_v, &[3, 4]).unwrap();
            let bm = b.from_slice_f32(&b_v, &[4, 3]).unwrap();
            let bias = b.from_slice_f32(&c_v, &[3, 3]).unwrap();
            let mm = b.matmul(&a, &bm).unwrap();
            let added = b.add(&mm, &bias).unwrap();
            let activated = b.relu(&added).unwrap();
            let summed = b.sum_axis(&activated, 1).unwrap();
            let scaled = b.scale(&summed, 0.5).unwrap();
            b.copy_to_host(&scaled).unwrap()
        };

        let gpu_bytes = do_chain(&cuda);
        let cpu_bytes = do_chain(&cpu);

        let g: Vec<f32> = gpu_bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let c: Vec<f32> = cpu_bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_close("chain", &g, &c, 1e-5);

        assert_eq!(
            cuda.bounce_count(),
            bounces_before,
            "chain bounced an op to CPU — expected pure-GPU dispatch"
        );
        // 3 inputs + 5 op outputs = 8 fresh GPU tensors.
        assert!(
            cuda.gpu_storage_len() >= storage_before + 8,
            "expected ≥8 new gpu_storage entries; got {} → {}",
            storage_before,
            cuda.gpu_storage_len()
        );
    }

    /// P114: concat is now GPU-routed (strided copy), so it must NOT
    /// bounce, must stay GPU-resident, and must place the slabs correctly.
    #[test]
    fn concat_axis0_gpu_no_bounce() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let bounces_before = cuda.bounce_count();

        let a = cuda.zeros(&[2, 3], DType::F32).unwrap();
        let b = cuda.ones(&[2, 3], DType::F32).unwrap();
        assert!(matches!(a.backend, BackendType::Cuda));
        assert!(matches!(b.backend, BackendType::Cuda));

        let concatenated = cuda.concat(&[&a, &b], 0).unwrap();
        assert!(matches!(concatenated.backend, BackendType::Cuda));
        assert_eq!(concatenated.shape, vec![4, 3]);
        assert_eq!(
            cuda.bounce_count(),
            bounces_before,
            "P114: concat must take the GPU path, not bounce"
        );

        let bytes = cuda.copy_to_host(&concatenated).unwrap();
        let f: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(&f[..6], &[0.0; 6]);
        assert_eq!(&f[6..], &[1.0; 6]);
    }

    /// P114: concat (non-zero axis) and split via strided GPU copies,
    /// checked against CpuBackend, with bounce_count asserted unchanged.
    #[test]
    fn concat_split_gpu_match_cpu() {
        let cuda = match CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let cpu = CpuBackend::new();
        let bounces_before = cuda.bounce_count();

        // ── concat along axis 1 (interleaved, exercises inner stride) ──
        // [2,3,4] ++ [2,2,4] ++ [2,1,4] along axis 1 → [2,6,4].
        let mk = |seed: f32, shape: &[usize], be: &dyn Backend| {
            let n: usize = shape.iter().product();
            let v: Vec<f32> = (0..n).map(|i| seed + i as f32 * 0.25).collect();
            be.from_slice_f32(&v, shape).unwrap()
        };
        let ga = mk(0.0, &[2, 3, 4], &cuda);
        let gb = mk(100.0, &[2, 2, 4], &cuda);
        let gc = mk(200.0, &[2, 1, 4], &cuda);
        let ca = mk(0.0, &[2, 3, 4], &cpu);
        let cb = mk(100.0, &[2, 2, 4], &cpu);
        let cc = mk(200.0, &[2, 1, 4], &cpu);

        let gcat = cuda.concat(&[&ga, &gb, &gc], 1).unwrap();
        let ccat = cpu.concat(&[&ca, &cb, &cc], 1).unwrap();
        assert_eq!(gcat.shape, vec![2, 6, 4]);
        assert_eq!(gcat.shape, ccat.shape);
        assert!(matches!(gcat.backend, BackendType::Cuda));
        assert_close("concat_axis1", &host_f32(&cuda, &gcat), &host_f32(&cpu, &ccat), 0.0);

        // ── split the result back along axis 1 into 3 equal parts ──
        // [2,6,4] / 3 → three [2,2,4]. Compare each to CPU split.
        let gparts = cuda.split(&gcat, 1, 3).unwrap();
        let cparts = cpu.split(&ccat, 1, 3).unwrap();
        assert_eq!(gparts.len(), 3);
        for (i, (gp, cp)) in gparts.iter().zip(&cparts).enumerate() {
            assert_eq!(gp.shape, vec![2, 2, 4], "split part {i} shape");
            assert!(matches!(gp.backend, BackendType::Cuda));
            assert_close(
                &format!("split_part{i}"),
                &host_f32(&cuda, gp),
                &host_f32(&cpu, cp),
                0.0,
            );
        }

        assert_eq!(
            cuda.bounce_count(),
            bounces_before,
            "P114: concat+split must stay on GPU (no bounce)"
        );
    }
}
