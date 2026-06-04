//! Compute Backend Module
//!
//! Provides abstraction over different compute backends (CPU, CUDA, WebGPU, Metal, Vulkan,
//! Apple ANE, Qualcomm Hexagon). Agents can seamlessly switch between backends based on
//! workload and available resources.

pub mod apple_ane;
pub mod blas;
pub mod cpu;
pub mod fusion;
pub mod metal;
pub mod qualcomm;
pub mod vulkan;
pub mod webgpu;

#[cfg(feature = "gpu")]
pub mod wgpu_backend;

// CUDA backend (legacy): historic stub at `cuda.rs` + the unused
// cudarc-0.10 port at `cuda_full.rs`. The new CUDA path lives in
// `prototype/src/cuda_backend.rs` and uses IronAccelerator directly
// for CUDA 13.2 support via libloading (no build-time CUDA_PATH
// required). RecursiveMachineIntelligence's feature still pulls cudarc 0.10 in for
// back-compat with any consumers depending on cuda_full.rs.
#[cfg(feature = "cuda")]
pub mod cuda;

use std::sync::Arc;

use async_trait::async_trait;

use crate::error::{Result, RmiError};

/// Backend type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendType {
    /// CPU backend
    Cpu,
    /// CUDA GPU backend
    Cuda,
    /// WebGPU backend (cross-platform GPU via wgpu)
    WebGpu,
    /// Metal backend (Apple GPU)
    Metal,
    /// Vulkan backend (cross-vendor GPU)
    Vulkan,
    /// Apple Neural Engine (ANE) backend
    AppleAne,
    /// Qualcomm Hexagon DSP/NPU backend
    Qualcomm,
}

/// Device information.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Device name
    pub name: String,
    /// Device type
    pub backend_type: BackendType,
    /// Total memory in bytes
    pub total_memory: u64,
    /// Available memory in bytes
    pub available_memory: u64,
    /// Compute capability (for CUDA)
    pub compute_capability: Option<(u32, u32)>,
    /// Number of compute units (cores/SMs)
    pub compute_units: u32,
}

/// Tensor data type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DType {
    /// 32-bit float
    F32,
    /// 64-bit float
    F64,
    /// 16-bit float (half precision)
    F16,
    /// 16-bit bfloat
    BF16,
    /// 32-bit integer
    I32,
    /// 64-bit integer
    I64,
    /// 8-bit signed integer (quantization)
    I8,
    /// 4-bit signed integer, packed 2/byte (quantization)
    I4,
    /// 8-bit unsigned integer
    U8,
    /// Boolean
    Bool,
}

impl DType {
    /// Get the size in bytes.
    #[inline]
    pub fn size_bytes(&self) -> usize {
        match self {
            DType::F32 | DType::I32 => 4,
            DType::F64 | DType::I64 => 8,
            DType::F16 | DType::BF16 => 2,
            // I4 is packed 2/byte; size_bytes is per-element so we round
            // up to 1. Callers needing exact packed size use ceil(n/2).
            DType::I8 | DType::I4 | DType::U8 | DType::Bool => 1,
        }
    }
}

/// Abstract tensor handle.
///
/// This is a backend-agnostic handle to tensor data.
/// The actual data may reside on CPU or GPU memory.
#[derive(Debug, Clone)]
pub struct TensorHandle {
    /// Unique identifier
    pub id: u64,
    /// Shape
    pub shape: Vec<usize>,
    /// Data type
    pub dtype: DType,
    /// Backend type
    pub backend: BackendType,
    /// Size in bytes
    pub size_bytes: usize,
}

impl TensorHandle {
    /// Get number of elements.
    #[inline]
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Get number of dimensions.
    #[inline]
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }
}

/// Compute backend trait.
///
/// All backends implement this trait to provide a uniform interface
/// for tensor operations.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Get backend type.
    fn backend_type(&self) -> BackendType;

    /// Get device info.
    fn device_info(&self) -> &DeviceInfo;

    /// Check if backend is available.
    fn is_available(&self) -> bool;

    // ==================== Memory Management ====================

    /// Allocate a tensor.
    fn allocate(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;

    /// Free a tensor.
    fn free(&self, handle: &TensorHandle) -> Result<()>;

    /// Copy data from host to device.
    fn copy_to_device(&self, handle: &TensorHandle, data: &[u8]) -> Result<()>;

    /// Copy data from device to host.
    fn copy_to_host(&self, handle: &TensorHandle) -> Result<Vec<u8>>;

    /// Copy between tensors on the same device.
    fn copy(&self, src: &TensorHandle, dst: &TensorHandle) -> Result<()>;

    // ==================== Tensor Creation ====================

    /// Create a tensor filled with zeros.
    fn zeros(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;

    /// Create a tensor filled with ones.
    fn ones(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;

    /// Create a tensor with random values from uniform distribution.
    fn rand(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;

    /// Create a tensor with random values from normal distribution.
    fn randn(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle>;

    /// Create a tensor from host data.
    #[allow(clippy::wrong_self_convention)]
    fn from_slice_f32(&self, data: &[f32], shape: &[usize]) -> Result<TensorHandle>;

    // ==================== Arithmetic Operations ====================

    /// Element-wise addition: c = a + b
    fn add(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;

    /// Element-wise subtraction: c = a - b
    fn sub(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;

    /// Element-wise multiplication: c = a * b
    fn mul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;

    /// Element-wise division: c = a / b
    fn div(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;

    /// Matrix multiplication: c = a @ b
    fn matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle>;

    /// Quantized matrix multiplication: `c = a @ b` computed via INT8.
    ///
    /// `a` (activations) and `b` (weights) are F32; the implementation
    /// may quantize them internally (e.g. per-tensor activations,
    /// per-channel weights) and accumulate in INT32, returning an F32
    /// result. This is an *optimization*: the default implementation
    /// just calls [`Self::matmul`], which is always numerically correct,
    /// so backends without INT8 support transparently fall back to F32.
    fn quantized_matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        self.matmul(a, b)
    }

    /// Quantized matmul with a *calibrated* activation scale `a_scale`
    /// (per-tensor, from offline calibration). Avoids a per-call
    /// activation-range reduction, so on INT8-capable backends the whole
    /// op stays on-device (no host sync) — the fast end-to-end path.
    /// Default ignores the scale and defers to [`Self::quantized_matmul`].
    #[allow(unused_variables)]
    fn quantized_matmul_calibrated(
        &self,
        a: &TensorHandle,
        a_scale: f32,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        self.quantized_matmul(a, b)
    }

    /// Calibrated quantized matmul with **INT4 weights** (W4A8): INT8
    /// activation at the calibrated scale × packed 4-bit per-channel
    /// weights. Halves weight memory vs INT8 (8× vs F32) at some accuracy
    /// cost. Default defers to the INT8 calibrated path.
    #[allow(unused_variables)]
    fn quantized_matmul_w4_calibrated(
        &self,
        a: &TensorHandle,
        a_scale: f32,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        self.quantized_matmul_calibrated(a, a_scale, b)
    }

    /// Quantized matmul with a calibrated **asymmetric** activation range
    /// `[a_lo, a_hi]` (zero-point quantization — uses the full int8 range
    /// even for one-sided distributions like post-ReLU activations).
    /// Default ignores the range and defers to [`Self::quantized_matmul`].
    #[allow(unused_variables)]
    fn quantized_matmul_asym_calibrated(
        &self,
        a: &TensorHandle,
        a_lo: f32,
        a_hi: f32,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        self.quantized_matmul(a, b)
    }

    /// Scalar multiplication: b = a * scalar
    fn scale(&self, a: &TensorHandle, scalar: f64) -> Result<TensorHandle>;

    // ==================== Reduction Operations ====================

    /// Sum all elements.
    fn sum(&self, a: &TensorHandle) -> Result<f64>;

    /// Sum along axis.
    fn sum_axis(&self, a: &TensorHandle, axis: usize) -> Result<TensorHandle>;

    /// Mean of all elements.
    fn mean(&self, a: &TensorHandle) -> Result<f64>;

    /// Mean along axis.
    fn mean_axis(&self, a: &TensorHandle, axis: usize) -> Result<TensorHandle>;

    /// Max of all elements.
    fn max(&self, a: &TensorHandle) -> Result<f64>;

    /// Min of all elements.
    fn min(&self, a: &TensorHandle) -> Result<f64>;

    // ==================== Activation Functions ====================

    /// ReLU activation.
    fn relu(&self, a: &TensorHandle) -> Result<TensorHandle>;

    /// GELU activation.
    fn gelu(&self, a: &TensorHandle) -> Result<TensorHandle>;

    /// Sigmoid activation.
    fn sigmoid(&self, a: &TensorHandle) -> Result<TensorHandle>;

    /// Tanh activation.
    fn tanh(&self, a: &TensorHandle) -> Result<TensorHandle>;

    /// Softmax along axis.
    fn softmax(&self, a: &TensorHandle, axis: i32) -> Result<TensorHandle>;

    // ==================== Convolution ====================

    /// 2-D convolution (cross-correlation), NCHW layout.
    ///
    /// - `input`: `[N, C_in, H, W]`
    /// - `weight`: `[C_out, C_in, KH, KW]`
    /// - `stride`, `padding`, `dilation`: symmetric (applied to both
    ///   spatial dims). `dilation = 1` is an ordinary convolution.
    /// - no bias
    ///
    /// Output: `[N, C_out, H_out, W_out]` where
    /// `H_out = (H + 2*padding - dilation*(KH-1) - 1) / stride + 1`
    /// (and likewise for W). The dilated kernel's effective extent is
    /// `dilation*(KH-1) + 1`.
    ///
    /// Default implementation returns an error; backends that support
    /// convolution (CPU reference, CUDA via im2col+GEMM) override it.
    #[allow(unused_variables)]
    fn conv2d(
        &self,
        input: &TensorHandle,
        weight: &TensorHandle,
        stride: usize,
        padding: usize,
        dilation: usize,
    ) -> Result<TensorHandle> {
        Err(RmiError::compute_simple(
            "conv2d not supported by this backend",
        ))
    }

    // ==================== Dtype Conversion ====================

    /// Cast a tensor to another dtype, returning a new tensor.
    ///
    /// Casting to the same dtype is an identity copy. The reference
    /// support set is F32 ↔ {F16, BF16, F64}; backends may support more
    /// or fewer. Half-precision casts go through F32 as the pivot.
    ///
    /// Default implementation returns an error; CPU and CUDA backends
    /// override it.
    #[allow(unused_variables)]
    fn cast(&self, a: &TensorHandle, target: DType) -> Result<TensorHandle> {
        Err(RmiError::compute_simple(
            "cast not supported by this backend",
        ))
    }

    // ==================== Shape Operations ====================

    /// Reshape tensor.
    fn reshape(&self, a: &TensorHandle, new_shape: &[usize]) -> Result<TensorHandle>;

    /// Transpose tensor.
    fn transpose(&self, a: &TensorHandle, axes: &[usize]) -> Result<TensorHandle>;

    /// Concatenate tensors along axis.
    fn concat(&self, tensors: &[&TensorHandle], axis: usize) -> Result<TensorHandle>;

    /// Split tensor along axis.
    fn split(&self, a: &TensorHandle, axis: usize, sections: usize) -> Result<Vec<TensorHandle>>;

    // ==================== Synchronization ====================

    /// Synchronize all pending operations.
    fn synchronize(&self) -> Result<()>;
}

/// Get the best available backend.
///
/// Priority order: CUDA > WebGPU > Vulkan > Metal > CPU
pub fn get_backend() -> Arc<dyn Backend> {
    #[cfg(feature = "cuda")]
    {
        if let Ok(cuda) = cuda::CudaBackend::new() {
            if cuda.is_available() {
                return Arc::new(cuda);
            }
        }
    }

    // Try GPU backends in preference order
    if let Ok(wgpu) = webgpu::WebGpuBackend::new() {
        if wgpu.is_available() {
            return Arc::new(wgpu);
        }
    }

    if let Ok(vk) = vulkan::VulkanBackend::default_device() {
        if vk.is_available() {
            return Arc::new(vk);
        }
    }

    if let Ok(mtl) = metal::MetalBackend::new() {
        if mtl.is_available() {
            return Arc::new(mtl);
        }
    }

    if let Ok(ane) = apple_ane::AppleAneBackend::new() {
        if ane.is_available() {
            return Arc::new(ane);
        }
    }

    if let Ok(qc) = qualcomm::QualcommBackend::new() {
        if qc.is_available() {
            return Arc::new(qc);
        }
    }

    Arc::new(cpu::CpuBackend::new())
}

/// Get a specific backend by type.
pub fn get_backend_by_type(backend_type: BackendType) -> Result<Arc<dyn Backend>> {
    match backend_type {
        BackendType::Cpu => Ok(Arc::new(cpu::CpuBackend::new())),
        #[cfg(feature = "cuda")]
        BackendType::Cuda => {
            let cuda = cuda::CudaBackend::new()?;
            if cuda.is_available() {
                Ok(Arc::new(cuda))
            } else {
                Err(RmiError::compute_simple("CUDA not available"))
            }
        }
        #[cfg(not(feature = "cuda"))]
        BackendType::Cuda => Err(RmiError::compute_simple("CUDA support not compiled")),
        BackendType::WebGpu => {
            let wgpu = webgpu::WebGpuBackend::new()?;
            if wgpu.is_available() {
                Ok(Arc::new(wgpu))
            } else {
                Err(RmiError::compute_simple("WebGPU not available"))
            }
        }
        BackendType::Metal => {
            let mtl = metal::MetalBackend::new()?;
            if mtl.is_available() {
                Ok(Arc::new(mtl))
            } else {
                Err(RmiError::compute_simple("Metal not available"))
            }
        }
        BackendType::Vulkan => {
            let vk = vulkan::VulkanBackend::default_device()?;
            if vk.is_available() {
                Ok(Arc::new(vk))
            } else {
                Err(RmiError::compute_simple("Vulkan not available"))
            }
        }
        BackendType::AppleAne => {
            let ane = apple_ane::AppleAneBackend::new()?;
            if ane.is_available() {
                Ok(Arc::new(ane))
            } else {
                Err(RmiError::compute_simple("Apple ANE not available"))
            }
        }
        BackendType::Qualcomm => {
            let qc = qualcomm::QualcommBackend::new()?;
            if qc.is_available() {
                Ok(Arc::new(qc))
            } else {
                Err(RmiError::compute_simple("Qualcomm Hexagon not available"))
            }
        }
    }
}

/// Re-exports for convenience
pub use apple_ane::AppleAneBackend;
pub use cpu::CpuBackend;
pub use metal::MetalBackend;
pub use qualcomm::QualcommBackend;
pub use vulkan::VulkanBackend;
pub use webgpu::WebGpuBackend;

#[cfg(feature = "cuda")]
pub use cuda::CudaBackend;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtype_size_bytes() {
        assert_eq!(DType::F32.size_bytes(), 4);
        assert_eq!(DType::F64.size_bytes(), 8);
        assert_eq!(DType::F16.size_bytes(), 2);
        assert_eq!(DType::BF16.size_bytes(), 2);
        assert_eq!(DType::I32.size_bytes(), 4);
        assert_eq!(DType::I64.size_bytes(), 8);
        assert_eq!(DType::U8.size_bytes(), 1);
        assert_eq!(DType::Bool.size_bytes(), 1);
    }

    #[test]
    fn tensor_handle_numel_ndim() {
        let h = TensorHandle {
            id: 0,
            shape: vec![3, 4, 5],
            dtype: DType::F32,
            backend: BackendType::Cpu,
            size_bytes: 3 * 4 * 5 * 4,
        };
        assert_eq!(h.numel(), 60);
        assert_eq!(h.ndim(), 3);
    }

    #[test]
    fn tensor_handle_scalar() {
        let h = TensorHandle {
            id: 1,
            shape: vec![],
            dtype: DType::F64,
            backend: BackendType::Cpu,
            size_bytes: 8,
        };
        assert_eq!(h.numel(), 1);
        assert_eq!(h.ndim(), 0);
    }

    #[test]
    fn backend_type_eq() {
        assert_eq!(BackendType::Cpu, BackendType::Cpu);
        assert_ne!(BackendType::Cpu, BackendType::Cuda);
        assert_ne!(BackendType::Metal, BackendType::Vulkan);
        assert_ne!(BackendType::AppleAne, BackendType::Metal);
        assert_ne!(BackendType::Qualcomm, BackendType::Vulkan);
    }

    #[test]
    fn get_backend_returns_available() {
        let backend = get_backend();
        assert!(backend.is_available());
    }

    #[test]
    fn get_backend_by_type_cpu() {
        let backend = get_backend_by_type(BackendType::Cpu).unwrap();
        assert_eq!(backend.backend_type(), BackendType::Cpu);
    }

    #[test]
    fn get_backend_by_type_stubs() {
        let webgpu = get_backend_by_type(BackendType::WebGpu).unwrap();
        assert_eq!(webgpu.backend_type(), BackendType::WebGpu);

        // Metal is only available on macOS/iOS
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            let metal = get_backend_by_type(BackendType::Metal).unwrap();
            assert_eq!(metal.backend_type(), BackendType::Metal);
        } else {
            assert!(get_backend_by_type(BackendType::Metal).is_err());
        }

        let vulkan = get_backend_by_type(BackendType::Vulkan).unwrap();
        assert_eq!(vulkan.backend_type(), BackendType::Vulkan);

        // Apple ANE is only available on macOS/iOS
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            let ane = get_backend_by_type(BackendType::AppleAne).unwrap();
            assert_eq!(ane.backend_type(), BackendType::AppleAne);
        } else {
            assert!(get_backend_by_type(BackendType::AppleAne).is_err());
        }

        // Qualcomm is only available on Android/Linux
        if cfg!(any(target_os = "android", target_os = "linux")) {
            let qc = get_backend_by_type(BackendType::Qualcomm).unwrap();
            assert_eq!(qc.backend_type(), BackendType::Qualcomm);
        } else {
            assert!(get_backend_by_type(BackendType::Qualcomm).is_err());
        }
    }

    #[test]
    fn device_info_fields() {
        let backend = get_backend();
        let info = backend.device_info();
        assert!(!info.name.is_empty());
        assert!(info.total_memory > 0);
        assert!(info.compute_units > 0);
    }
}
