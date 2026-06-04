//! CUDA Compute Backend
//!
//! GPU-accelerated tensor operations using CUDA via cudarc.
//! This module requires the `cuda` feature and a compatible NVIDIA GPU.
//!
//! # Architecture
//!
//! The CUDA backend provides:
//! - Device memory management with automatic cleanup
//! - cuBLAS integration for optimized BLAS operations  
//! - Custom CUDA kernels for elementwise and reduction ops
//! - Async stream support for overlapping compute and transfer
//! - Multi-GPU support through device selection
//!
//! # Example
//!
//! ```ignore
//! use mig::compute::cuda::CudaBackend;
//! use mig::compute::{Backend, DType};
//!
//! let backend = CudaBackend::new()?;
//! let a = backend.rand(&[1024, 1024], DType::F32)?;
//! let b = backend.rand(&[1024, 1024], DType::F32)?;
//! let c = backend.matmul(&a, &b)?;
//! backend.synchronize()?;
//! ```

#![cfg(feature = "cuda")]

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, RwLock};

use cudarc::cublas::{sys::cublasOperation_t, CudaBlas, GemmConfig};
use cudarc::driver::{
    CudaDevice, CudaSlice, CudaStream, DevicePtr, DeviceRepr, LaunchAsync, LaunchConfig,
};
use cudarc::nvrtc::Ptx;

use super::{Backend, BackendType, DType, DeviceInfo, TensorHandle};
use crate::error::{RmiError, Result};

// ============================================================================
// Constants
// ============================================================================

/// Default number of threads per block for 1D kernels
const THREADS_PER_BLOCK: u32 = 256;

/// Tile size for matrix operations
const TILE_SIZE: u32 = 16;

/// Maximum shared memory per block (48KB typical)
const MAX_SHARED_MEMORY: usize = 48 * 1024;

// ============================================================================
// Memory Management
// ============================================================================

/// CUDA memory allocation tracking
struct MemoryPool {
    /// Device reference
    device: Arc<CudaDevice>,
    /// Allocated tensors by handle ID
    allocations: HashMap<u64, CudaAllocation>,
    /// Next allocation ID
    next_id: u64,
    /// Total allocated bytes
    total_allocated: usize,
    /// Peak allocated bytes
    peak_allocated: usize,
}

/// Single CUDA allocation
struct CudaAllocation {
    /// Device memory slice (type-erased)
    ptr: u64,
    /// Size in bytes
    size_bytes: usize,
    /// Shape
    shape: Vec<usize>,
    /// Data type
    dtype: DType,
}

impl MemoryPool {
    fn new(device: Arc<CudaDevice>) -> Self {
        Self {
            device,
            allocations: HashMap::new(),
            next_id: 1,
            total_allocated: 0,
            peak_allocated: 0,
        }
    }

    fn allocate(&mut self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        let numel: usize = shape.iter().product();
        let size_bytes = numel * dtype.size_bytes();

        // Allocate device memory
        // SAFETY: FFI call to CUDA driver API; result is checked for success immediately.
        let ptr = unsafe {
            let mut ptr: u64 = 0;
            let result = cudarc::driver::sys::cuMemAlloc_v2(&mut ptr as *mut u64, size_bytes);
            if result != cudarc::driver::sys::CUresult::CUDA_SUCCESS {
                return Err(RmiError::Compute(format!(
                    "CUDA allocation failed: {:?}",
                    result
                )));
            }
            ptr
        };

        let id = self.next_id;
        self.next_id += 1;

        self.allocations.insert(
            id,
            CudaAllocation {
                ptr,
                size_bytes,
                shape: shape.to_vec(),
                dtype,
            },
        );

        self.total_allocated += size_bytes;
        self.peak_allocated = self.peak_allocated.max(self.total_allocated);

        Ok(TensorHandle {
            id,
            shape: shape.to_vec(),
            dtype,
            backend: BackendType::Cuda,
            size_bytes,
        })
    }

    fn free(&mut self, handle: &TensorHandle) -> Result<()> {
        if let Some(alloc) = self.allocations.remove(&handle.id) {
            // SAFETY: FFI call; `alloc.ptr` was obtained from cuMemAlloc_v2.
            unsafe {
                cudarc::driver::sys::cuMemFree_v2(alloc.ptr);
            }
            self.total_allocated -= alloc.size_bytes;
        }
        Ok(())
    }

    fn get_ptr(&self, handle: &TensorHandle) -> Result<u64> {
        self.allocations
            .get(&handle.id)
            .map(|a| a.ptr)
            .ok_or_else(|| RmiError::Compute("Tensor not found".to_string()))
    }
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        // Free all remaining allocations
        for (_, alloc) in self.allocations.drain() {
            // SAFETY: FFI call; each `alloc.ptr` was obtained from cuMemAlloc_v2.
            unsafe {
                cudarc::driver::sys::cuMemFree_v2(alloc.ptr);
            }
        }
    }
}

// ============================================================================
// CUDA Kernels (PTX)
// ============================================================================

/// PTX source for custom kernels
const KERNELS_PTX: &str = r#"
.version 7.0
.target sm_70
.address_size 64

// Elementwise addition: out[i] = a[i] + b[i]
.visible .entry add_f32(
    .param .u64 a,
    .param .u64 b, 
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<4>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<10>;
    
    ld.param.u64 %rd1, [a];
    ld.param.u64 %rd2, [b];
    ld.param.u64 %rd3, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd4, %r2;
    shl.b64 %rd5, %rd4, 2;
    
    add.s64 %rd6, %rd1, %rd5;
    add.s64 %rd7, %rd2, %rd5;
    add.s64 %rd8, %rd3, %rd5;
    
    ld.global.f32 %f1, [%rd6];
    ld.global.f32 %f2, [%rd7];
    add.f32 %f3, %f1, %f2;
    st.global.f32 [%rd8], %f3;
    
END:
    ret;
}

// Elementwise subtraction: out[i] = a[i] - b[i]
.visible .entry sub_f32(
    .param .u64 a,
    .param .u64 b,
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<4>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<10>;
    
    ld.param.u64 %rd1, [a];
    ld.param.u64 %rd2, [b];
    ld.param.u64 %rd3, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd4, %r2;
    shl.b64 %rd5, %rd4, 2;
    
    add.s64 %rd6, %rd1, %rd5;
    add.s64 %rd7, %rd2, %rd5;
    add.s64 %rd8, %rd3, %rd5;
    
    ld.global.f32 %f1, [%rd6];
    ld.global.f32 %f2, [%rd7];
    sub.f32 %f3, %f1, %f2;
    st.global.f32 [%rd8], %f3;
    
END:
    ret;
}

// Elementwise multiplication: out[i] = a[i] * b[i]
.visible .entry mul_f32(
    .param .u64 a,
    .param .u64 b,
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<4>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<10>;
    
    ld.param.u64 %rd1, [a];
    ld.param.u64 %rd2, [b];
    ld.param.u64 %rd3, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd4, %r2;
    shl.b64 %rd5, %rd4, 2;
    
    add.s64 %rd6, %rd1, %rd5;
    add.s64 %rd7, %rd2, %rd5;
    add.s64 %rd8, %rd3, %rd5;
    
    ld.global.f32 %f1, [%rd6];
    ld.global.f32 %f2, [%rd7];
    mul.f32 %f3, %f1, %f2;
    st.global.f32 [%rd8], %f3;
    
END:
    ret;
}

// Elementwise division: out[i] = a[i] / b[i]
.visible .entry div_f32(
    .param .u64 a,
    .param .u64 b,
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<4>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<10>;
    
    ld.param.u64 %rd1, [a];
    ld.param.u64 %rd2, [b];
    ld.param.u64 %rd3, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd4, %r2;
    shl.b64 %rd5, %rd4, 2;
    
    add.s64 %rd6, %rd1, %rd5;
    add.s64 %rd7, %rd2, %rd5;
    add.s64 %rd8, %rd3, %rd5;
    
    ld.global.f32 %f1, [%rd6];
    ld.global.f32 %f2, [%rd7];
    div.approx.f32 %f3, %f1, %f2;
    st.global.f32 [%rd8], %f3;
    
END:
    ret;
}

// ReLU activation: out[i] = max(0, x[i])
.visible .entry relu_f32(
    .param .u64 x,
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<3>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<8>;
    
    ld.param.u64 %rd1, [x];
    ld.param.u64 %rd2, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd3, %r2;
    shl.b64 %rd4, %rd3, 2;
    
    add.s64 %rd5, %rd1, %rd4;
    add.s64 %rd6, %rd2, %rd4;
    
    ld.global.f32 %f1, [%rd5];
    mov.f32 %f2, 0f00000000;
    max.f32 %f1, %f1, %f2;
    st.global.f32 [%rd6], %f1;
    
END:
    ret;
}

// GELU activation (approximate): out[i] = 0.5 * x * (1 + tanh(sqrt(2/pi) * (x + 0.044715 * x^3)))
.visible .entry gelu_f32(
    .param .u64 x,
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<12>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<8>;
    
    ld.param.u64 %rd1, [x];
    ld.param.u64 %rd2, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd3, %r2;
    shl.b64 %rd4, %rd3, 2;
    
    add.s64 %rd5, %rd1, %rd4;
    add.s64 %rd6, %rd2, %rd4;
    
    ld.global.f32 %f1, [%rd5];          // x
    mul.f32 %f2, %f1, %f1;              // x^2
    mul.f32 %f3, %f2, %f1;              // x^3
    mov.f32 %f4, 0f3D372713;            // 0.044715
    mul.f32 %f5, %f4, %f3;              // 0.044715 * x^3
    add.f32 %f6, %f1, %f5;              // x + 0.044715 * x^3
    mov.f32 %f7, 0f3F4C422A;            // sqrt(2/pi) ≈ 0.7978845608
    mul.f32 %f8, %f7, %f6;              // sqrt(2/pi) * (x + 0.044715 * x^3)
    // Approximate tanh using exp
    mul.f32 %f9, %f8, 0fC0000000;       // -2 * arg
    ex2.approx.f32 %f9, %f9;            // exp(-2*arg) via 2^x approx
    add.f32 %f10, %f9, 0f3F800000;      // 1 + exp(-2*arg)
    rcp.approx.f32 %f10, %f10;          // 1 / (1 + exp(-2*arg))
    mul.f32 %f10, %f10, 0f40000000;     // 2 / (1 + exp(-2*arg))
    sub.f32 %f10, %f10, 0f3F800000;     // tanh ≈ 2/(1+exp(-2x)) - 1
    add.f32 %f10, %f10, 0f3F800000;     // 1 + tanh(...)
    mul.f32 %f11, %f1, %f10;            // x * (1 + tanh(...))
    mul.f32 %f11, %f11, 0f3F000000;     // 0.5 * x * (1 + tanh(...))
    st.global.f32 [%rd6], %f11;
    
END:
    ret;
}

// Sigmoid activation: out[i] = 1 / (1 + exp(-x[i]))
.visible .entry sigmoid_f32(
    .param .u64 x,
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<5>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<8>;
    
    ld.param.u64 %rd1, [x];
    ld.param.u64 %rd2, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd3, %r2;
    shl.b64 %rd4, %rd3, 2;
    
    add.s64 %rd5, %rd1, %rd4;
    add.s64 %rd6, %rd2, %rd4;
    
    ld.global.f32 %f1, [%rd5];
    neg.f32 %f2, %f1;                   // -x
    mul.f32 %f2, %f2, 0f3FB8AA3B;       // -x * log2(e) for exp via exp2
    ex2.approx.f32 %f2, %f2;            // exp(-x)
    add.f32 %f3, %f2, 0f3F800000;       // 1 + exp(-x)
    rcp.approx.f32 %f4, %f3;            // 1 / (1 + exp(-x))
    st.global.f32 [%rd6], %f4;
    
END:
    ret;
}

// Tanh activation: out[i] = tanh(x[i])
.visible .entry tanh_f32(
    .param .u64 x,
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<6>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<8>;
    
    ld.param.u64 %rd1, [x];
    ld.param.u64 %rd2, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd3, %r2;
    shl.b64 %rd4, %rd3, 2;
    
    add.s64 %rd5, %rd1, %rd4;
    add.s64 %rd6, %rd2, %rd4;
    
    ld.global.f32 %f1, [%rd5];
    mul.f32 %f2, %f1, 0fC0000000;       // -2x
    mul.f32 %f2, %f2, 0f3FB8AA3B;       // -2x * log2(e)
    ex2.approx.f32 %f2, %f2;            // exp(-2x)
    add.f32 %f3, %f2, 0f3F800000;       // 1 + exp(-2x)
    rcp.approx.f32 %f4, %f3;            // 1 / (1 + exp(-2x))
    mul.f32 %f4, %f4, 0f40000000;       // 2 / (1 + exp(-2x))
    sub.f32 %f5, %f4, 0f3F800000;       // tanh = 2/(1+exp(-2x)) - 1
    st.global.f32 [%rd6], %f5;
    
END:
    ret;
}

// Scalar multiplication: out[i] = a[i] * scalar
.visible .entry scale_f32(
    .param .u64 a,
    .param .u64 out,
    .param .f32 scalar,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<4>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<8>;
    
    ld.param.u64 %rd1, [a];
    ld.param.u64 %rd2, [out];
    ld.param.f32 %f1, [scalar];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd3, %r2;
    shl.b64 %rd4, %rd3, 2;
    
    add.s64 %rd5, %rd1, %rd4;
    add.s64 %rd6, %rd2, %rd4;
    
    ld.global.f32 %f2, [%rd5];
    mul.f32 %f3, %f2, %f1;
    st.global.f32 [%rd6], %f3;
    
END:
    ret;
}

// Fill tensor with value: out[i] = value
.visible .entry fill_f32(
    .param .u64 out,
    .param .f32 value,
    .param .u32 n
) {
    .reg .pred %p<2>;
    .reg .f32 %f<2>;
    .reg .b32 %r<5>;
    .reg .b64 %rd<4>;
    
    ld.param.u64 %rd1, [out];
    ld.param.f32 %f1, [value];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    mad.lo.s32 %r2, %r2, %r3, %r4;
    
    setp.ge.u32 %p1, %r2, %r1;
    @%p1 bra END;
    
    cvt.u64.u32 %rd2, %r2;
    shl.b64 %rd3, %rd2, 2;
    add.s64 %rd1, %rd1, %rd3;
    
    st.global.f32 [%rd1], %f1;
    
END:
    ret;
}

// Sum reduction (first pass - block-level)
.visible .entry sum_reduce_f32(
    .param .u64 x,
    .param .u64 out,
    .param .u32 n
) {
    .reg .pred %p<3>;
    .reg .f32 %f<4>;
    .reg .b32 %r<10>;
    .reg .b64 %rd<6>;
    
    .shared .align 4 .f32 sdata[256];
    
    ld.param.u64 %rd1, [x];
    ld.param.u64 %rd2, [out];
    ld.param.u32 %r1, [n];
    
    mov.u32 %r2, %ctaid.x;
    mov.u32 %r3, %ntid.x;
    mov.u32 %r4, %tid.x;
    
    // Global index
    mad.lo.s32 %r5, %r2, %r3, %r4;
    shl.b32 %r6, %r5, 1;  // Each thread loads 2 elements
    
    // Load first element
    mov.f32 %f1, 0f00000000;
    setp.lt.u32 %p1, %r6, %r1;
    @!%p1 bra LOAD2;
    cvt.u64.u32 %rd3, %r6;
    shl.b64 %rd4, %rd3, 2;
    add.s64 %rd5, %rd1, %rd4;
    ld.global.f32 %f1, [%rd5];
    
LOAD2:
    // Load second element
    add.u32 %r7, %r6, 1;
    setp.lt.u32 %p2, %r7, %r1;
    @!%p2 bra STORE_SHARED;
    cvt.u64.u32 %rd3, %r7;
    shl.b64 %rd4, %rd3, 2;
    add.s64 %rd5, %rd1, %rd4;
    ld.global.f32 %f2, [%rd5];
    add.f32 %f1, %f1, %f2;
    
STORE_SHARED:
    // Store to shared memory
    shl.b32 %r8, %r4, 2;
    mov.u32 %r9, sdata;
    add.s32 %r9, %r9, %r8;
    st.shared.f32 [%r9], %f1;
    
    bar.sync 0;
    
    // Reduction in shared memory
    mov.u32 %r7, 128;
REDUCE_LOOP:
    setp.ge.u32 %p1, %r4, %r7;
    @%p1 bra REDUCE_DONE;
    
    shl.b32 %r8, %r4, 2;
    mov.u32 %r9, sdata;
    add.s32 %r9, %r9, %r8;
    ld.shared.f32 %f1, [%r9];
    
    shl.b32 %r8, %r7, 2;
    add.s32 %r9, %r9, %r8;
    ld.shared.f32 %f2, [%r9];
    
    add.f32 %f1, %f1, %f2;
    
    shl.b32 %r8, %r4, 2;
    mov.u32 %r9, sdata;
    add.s32 %r9, %r9, %r8;
    st.shared.f32 [%r9], %f1;
    
    bar.sync 0;
    shr.u32 %r7, %r7, 1;
    setp.gt.u32 %p1, %r7, 0;
    @%p1 bra REDUCE_LOOP;
    
REDUCE_DONE:
    // Thread 0 writes result
    setp.ne.u32 %p1, %r4, 0;
    @%p1 bra END;
    
    mov.u32 %r9, sdata;
    ld.shared.f32 %f1, [%r9];
    
    cvt.u64.u32 %rd3, %r2;
    shl.b64 %rd4, %rd3, 2;
    add.s64 %rd5, %rd2, %rd4;
    st.global.f32 [%rd5], %f1;
    
END:
    ret;
}
"#;

// ============================================================================
// CUDA Backend Implementation
// ============================================================================

/// CUDA compute backend for GPU-accelerated operations.
///
/// This backend uses cudarc for CUDA runtime/driver API access
/// and provides optimized implementations of tensor operations
/// using cuBLAS and custom CUDA kernels.
pub struct CudaBackend {
    /// CUDA device
    device: Arc<CudaDevice>,
    /// cuBLAS handle
    blas: Arc<CudaBlas>,
    /// Memory pool (thread-safe)
    memory: Arc<Mutex<MemoryPool>>,
    /// Device info
    device_info: DeviceInfo,
    /// Loaded PTX modules
    kernels_loaded: bool,
}

impl CudaBackend {
    /// Create a new CUDA backend on device 0.
    pub fn new() -> Result<Self> {
        Self::with_device(0)
    }

    /// Create a new CUDA backend on a specific device.
    pub fn with_device(device_id: usize) -> Result<Self> {
        // Initialize CUDA device
        let device = CudaDevice::new(device_id)
            .map_err(|e| RmiError::Compute(format!("CUDA init failed: {}", e)))?;
        let device = Arc::new(device);

        // Create cuBLAS handle
        let blas = CudaBlas::new(device.clone())
            .map_err(|e| RmiError::Compute(format!("cuBLAS init failed: {}", e)))?;
        let blas = Arc::new(blas);

        // Query device properties
        let device_info = Self::query_device_info(&device, device_id)?;

        // Create memory pool
        let memory = Arc::new(Mutex::new(MemoryPool::new(device.clone())));

        // Load custom kernels
        let mut backend = Self {
            device,
            blas,
            memory,
            device_info,
            kernels_loaded: false,
        };

        backend.load_kernels()?;

        Ok(backend)
    }

    /// Query device information.
    fn query_device_info(device: &CudaDevice, device_id: usize) -> Result<DeviceInfo> {
        // Use cudarc to query device properties
        let name = format!("CUDA Device {}", device_id);

        // Get memory info
        // SAFETY: FFI call to CUDA driver API; out-params are valid local variables.
        let (free, total) = unsafe {
            let mut free: usize = 0;
            let mut total: usize = 0;
            cudarc::driver::sys::cuMemGetInfo_v2(&mut free, &mut total);
            (free, total)
        };

        // Get compute capability and SM count
        // SAFETY: FFI calls to CUDA driver API with valid out-param pointers.
        let (major, minor, sm_count) = unsafe {
            let mut major: i32 = 0;
            let mut minor: i32 = 0;
            let mut sm_count: i32 = 0;

            cudarc::driver::sys::cuDeviceGetAttribute(
                &mut major,
                cudarc::driver::sys::CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MAJOR,
                device_id as i32,
            );
            cudarc::driver::sys::cuDeviceGetAttribute(
                &mut minor,
                cudarc::driver::sys::CUdevice_attribute::CU_DEVICE_ATTRIBUTE_COMPUTE_CAPABILITY_MINOR,
                device_id as i32,
            );
            cudarc::driver::sys::cuDeviceGetAttribute(
                &mut sm_count,
                cudarc::driver::sys::CUdevice_attribute::CU_DEVICE_ATTRIBUTE_MULTIPROCESSOR_COUNT,
                device_id as i32,
            );

            (major as u32, minor as u32, sm_count as u32)
        };

        Ok(DeviceInfo {
            name,
            backend_type: BackendType::Cuda,
            total_memory: total as u64,
            available_memory: free as u64,
            compute_capability: Some((major, minor)),
            compute_units: sm_count,
        })
    }

    /// Load PTX kernels.
    fn load_kernels(&mut self) -> Result<()> {
        // Load the PTX module
        self.device
            .load_ptx(
                Ptx::from_src(KERNELS_PTX),
                "air_kernels",
                &[
                    "add_f32",
                    "sub_f32",
                    "mul_f32",
                    "div_f32",
                    "relu_f32",
                    "gelu_f32",
                    "sigmoid_f32",
                    "tanh_f32",
                    "scale_f32",
                    "fill_f32",
                    "sum_reduce_f32",
                ],
            )
            .map_err(|e| RmiError::Compute(format!("PTX load failed: {}", e)))?;

        self.kernels_loaded = true;
        Ok(())
    }

    /// Calculate grid dimensions for a given number of elements.
    fn grid_1d(&self, n: usize) -> (u32, u32, u32) {
        let blocks = ((n as u32) + THREADS_PER_BLOCK - 1) / THREADS_PER_BLOCK;
        (blocks, 1, 1)
    }

    /// Launch an elementwise binary kernel.
    fn launch_binary_kernel(
        &self,
        kernel_name: &str,
        a: &TensorHandle,
        b: &TensorHandle,
        out: &TensorHandle,
    ) -> Result<()> {
        let memory = self.memory.lock().unwrap();
        let a_ptr = memory.get_ptr(a)?;
        let b_ptr = memory.get_ptr(b)?;
        let out_ptr = memory.get_ptr(out)?;
        let n = a.numel() as u32;
        drop(memory);

        let grid = self.grid_1d(a.numel());
        let block = (THREADS_PER_BLOCK, 1, 1);

        let func = self
            .device
            .get_func("air_kernels", kernel_name)
            .ok_or_else(|| RmiError::Compute(format!("Kernel {} not found", kernel_name)))?;

        // SAFETY: kernel parameters (pointers, n) are valid; grid/block dims computed from element count.
        unsafe {
            func.launch(
                LaunchConfig {
                    grid_dim: grid,
                    block_dim: block,
                    shared_mem_bytes: 0,
                },
                (a_ptr, b_ptr, out_ptr, n),
            )
        }
        .map_err(|e| RmiError::Compute(format!("Kernel launch failed: {}", e)))?;

        Ok(())
    }

    /// Launch an elementwise unary kernel.
    fn launch_unary_kernel(
        &self,
        kernel_name: &str,
        x: &TensorHandle,
        out: &TensorHandle,
    ) -> Result<()> {
        let memory = self.memory.lock().unwrap();
        let x_ptr = memory.get_ptr(x)?;
        let out_ptr = memory.get_ptr(out)?;
        let n = x.numel() as u32;
        drop(memory);

        let grid = self.grid_1d(x.numel());
        let block = (THREADS_PER_BLOCK, 1, 1);

        let func = self
            .device
            .get_func("air_kernels", kernel_name)
            .ok_or_else(|| RmiError::Compute(format!("Kernel {} not found", kernel_name)))?;

        // SAFETY: kernel parameters (pointers, n) are valid; grid/block dims computed from element count.
        unsafe {
            func.launch(
                LaunchConfig {
                    grid_dim: grid,
                    block_dim: block,
                    shared_mem_bytes: 0,
                },
                (x_ptr, out_ptr, n),
            )
        }
        .map_err(|e| RmiError::Compute(format!("Kernel launch failed: {}", e)))?;

        Ok(())
    }
}

impl Backend for CudaBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::Cuda
    }

    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    fn is_available(&self) -> bool {
        self.kernels_loaded
    }

    // ==================== Memory Management ====================

    fn allocate(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        self.memory.lock().unwrap().allocate(shape, dtype)
    }

    fn free(&self, handle: &TensorHandle) -> Result<()> {
        self.memory.lock().unwrap().free(handle)
    }

    fn copy_to_device(&self, handle: &TensorHandle, data: &[u8]) -> Result<()> {
        let memory = self.memory.lock().unwrap();
        let ptr = memory.get_ptr(handle)?;

        if data.len() != handle.size_bytes {
            return Err(RmiError::Compute(format!(
                "Data size mismatch: {} vs {}",
                data.len(),
                handle.size_bytes
            )));
        }

        // SAFETY: FFI call; `ptr` is a valid device pointer, `data` is a valid host buffer of correct size.
        unsafe {
            cudarc::driver::sys::cuMemcpyHtoD_v2(ptr, data.as_ptr() as *const c_void, data.len());
        }

        Ok(())
    }

    fn copy_to_host(&self, handle: &TensorHandle) -> Result<Vec<u8>> {
        let memory = self.memory.lock().unwrap();
        let ptr = memory.get_ptr(handle)?;

        let mut data = vec![0u8; handle.size_bytes];

        // SAFETY: FFI call; `ptr` is a valid device pointer, `data` is a host buffer of matching size.
        unsafe {
            cudarc::driver::sys::cuMemcpyDtoH_v2(
                data.as_mut_ptr() as *mut c_void,
                ptr,
                handle.size_bytes,
            );
        }

        Ok(data)
    }

    fn copy(&self, src: &TensorHandle, dst: &TensorHandle) -> Result<()> {
        let memory = self.memory.lock().unwrap();
        let src_ptr = memory.get_ptr(src)?;
        let dst_ptr = memory.get_ptr(dst)?;

        if src.size_bytes != dst.size_bytes {
            return Err(RmiError::Compute("Size mismatch".to_string()));
        }

        // SAFETY: FFI call; both device pointers are valid and sizes match (verified above).
        unsafe {
            cudarc::driver::sys::cuMemcpyDtoD_v2(dst_ptr, src_ptr, src.size_bytes);
        }

        Ok(())
    }

    // ==================== Tensor Creation ====================

    fn zeros(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        let handle = self.allocate(shape, dtype)?;

        // Launch fill kernel with 0
        let memory = self.memory.lock().unwrap();
        let ptr = memory.get_ptr(&handle)?;
        let n = handle.numel() as u32;
        drop(memory);

        let func = self
            .device
            .get_func("air_kernels", "fill_f32")
            .ok_or_else(|| RmiError::Compute("fill kernel not found".to_string()))?;

        let grid = self.grid_1d(handle.numel());
        let block = (THREADS_PER_BLOCK, 1, 1);

        // SAFETY: kernel launch with valid device pointer; grid/block dims computed from element count.
        unsafe {
            func.launch(
                LaunchConfig {
                    grid_dim: grid,
                    block_dim: block,
                    shared_mem_bytes: 0,
                },
                (ptr, 0.0f32, n),
            )
        }
        .map_err(|e| RmiError::Compute(format!("Fill failed: {}", e)))?;

        Ok(handle)
    }

    fn ones(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        let handle = self.allocate(shape, dtype)?;

        let memory = self.memory.lock().unwrap();
        let ptr = memory.get_ptr(&handle)?;
        let n = handle.numel() as u32;
        drop(memory);

        let func = self
            .device
            .get_func("air_kernels", "fill_f32")
            .ok_or_else(|| RmiError::Compute("fill kernel not found".to_string()))?;

        let grid = self.grid_1d(handle.numel());
        let block = (THREADS_PER_BLOCK, 1, 1);

        // SAFETY: kernel launch with valid device pointer; grid/block dims computed from element count.
        unsafe {
            func.launch(
                LaunchConfig {
                    grid_dim: grid,
                    block_dim: block,
                    shared_mem_bytes: 0,
                },
                (ptr, 1.0f32, n),
            )
        }
        .map_err(|e| RmiError::Compute(format!("Fill failed: {}", e)))?;

        Ok(handle)
    }

    fn rand(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        // Generate random data on CPU and copy to GPU
        // (For production, use cuRAND)
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let numel: usize = shape.iter().product();
        let data: Vec<f32> = (0..numel).map(|_| rng.gen()).collect();
        let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();

        let handle = self.allocate(shape, dtype)?;
        self.copy_to_device(&handle, &bytes)?;

        Ok(handle)
    }

    fn randn(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        use rand_distr::{Distribution, StandardNormal};
        let mut rng = rand::thread_rng();

        let numel: usize = shape.iter().product();
        let data: Vec<f32> = (0..numel)
            .map(|_| StandardNormal.sample(&mut rng) as f32)
            .collect();
        let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();

        let handle = self.allocate(shape, dtype)?;
        self.copy_to_device(&handle, &bytes)?;

        Ok(handle)
    }

    fn from_slice_f32(&self, data: &[f32], shape: &[usize]) -> Result<TensorHandle> {
        let expected_numel: usize = shape.iter().product();
        if data.len() != expected_numel {
            return Err(RmiError::Compute(format!(
                "Data length {} doesn't match shape {:?}",
                data.len(),
                shape
            )));
        }

        let bytes: Vec<u8> = data.iter().flat_map(|x| x.to_le_bytes()).collect();
        let handle = self.allocate(shape, DType::F32)?;
        self.copy_to_device(&handle, &bytes)?;

        Ok(handle)
    }

    // ==================== Arithmetic Operations ====================

    fn add(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        if a.shape != b.shape {
            return Err(RmiError::Compute(
                "Shape mismatch for add".to_string(),
            ));
        }

        let out = self.allocate(&a.shape, a.dtype)?;
        self.launch_binary_kernel("add_f32", a, b, &out)?;
        Ok(out)
    }

    fn sub(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        if a.shape != b.shape {
            return Err(RmiError::Compute(
                "Shape mismatch for sub".to_string(),
            ));
        }

        let out = self.allocate(&a.shape, a.dtype)?;
        self.launch_binary_kernel("sub_f32", a, b, &out)?;
        Ok(out)
    }

    fn mul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        if a.shape != b.shape {
            return Err(RmiError::Compute(
                "Shape mismatch for mul".to_string(),
            ));
        }

        let out = self.allocate(&a.shape, a.dtype)?;
        self.launch_binary_kernel("mul_f32", a, b, &out)?;
        Ok(out)
    }

    fn div(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        if a.shape != b.shape {
            return Err(RmiError::Compute(
                "Shape mismatch for div".to_string(),
            ));
        }

        let out = self.allocate(&a.shape, a.dtype)?;
        self.launch_binary_kernel("div_f32", a, b, &out)?;
        Ok(out)
    }

    fn matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        // Use cuBLAS SGEMM
        if a.shape.len() != 2 || b.shape.len() != 2 {
            return Err(RmiError::Compute(
                "Matmul requires 2D tensors".to_string(),
            ));
        }

        let m = a.shape[0];
        let k = a.shape[1];
        let n = b.shape[1];

        if k != b.shape[0] {
            return Err(RmiError::Compute(format!(
                "Matmul dimension mismatch: {:?} @ {:?}",
                a.shape, b.shape
            )));
        }

        let out = self.allocate(&[m, n], DType::F32)?;

        let memory = self.memory.lock().unwrap();
        let a_ptr = memory.get_ptr(a)?;
        let b_ptr = memory.get_ptr(b)?;
        let c_ptr = memory.get_ptr(&out)?;
        drop(memory);

        // cuBLAS uses column-major, so we compute C^T = B^T @ A^T
        // which gives us C in row-major
        // SAFETY: FFI call to cuBLAS; all device pointers and dimensions are validated above.
        unsafe {
            let alpha: f32 = 1.0;
            let beta: f32 = 0.0;

            cudarc::cublas::sys::cublasSgemm_v2(
                self.blas.handle() as *mut _,
                cublasOperation_t::CUBLAS_OP_N,
                cublasOperation_t::CUBLAS_OP_N,
                n as i32,
                m as i32,
                k as i32,
                &alpha,
                b_ptr as *const f32,
                n as i32,
                a_ptr as *const f32,
                k as i32,
                &beta,
                c_ptr as *mut f32,
                n as i32,
            );
        }

        Ok(out)
    }

    fn scale(&self, a: &TensorHandle, scalar: f64) -> Result<TensorHandle> {
        let out = self.allocate(&a.shape, a.dtype)?;

        let memory = self.memory.lock().unwrap();
        let a_ptr = memory.get_ptr(a)?;
        let out_ptr = memory.get_ptr(&out)?;
        let n = a.numel() as u32;
        drop(memory);

        let func = self
            .device
            .get_func("air_kernels", "scale_f32")
            .ok_or_else(|| RmiError::Compute("scale kernel not found".to_string()))?;

        let grid = self.grid_1d(a.numel());
        let block = (THREADS_PER_BLOCK, 1, 1);

        // SAFETY: kernel launch with valid device pointers; grid/block dims computed from element count.
        unsafe {
            func.launch(
                LaunchConfig {
                    grid_dim: grid,
                    block_dim: block,
                    shared_mem_bytes: 0,
                },
                (a_ptr, out_ptr, scalar as f32, n),
            )
        }
        .map_err(|e| RmiError::Compute(format!("Scale failed: {}", e)))?;

        Ok(out)
    }

    // ==================== Reduction Operations ====================

    fn sum(&self, a: &TensorHandle) -> Result<f64> {
        // Multi-pass reduction
        let n = a.numel();
        let blocks = (n + THREADS_PER_BLOCK as usize * 2 - 1) / (THREADS_PER_BLOCK as usize * 2);

        // First pass: reduce to block sums
        let partial = self.allocate(&[blocks], DType::F32)?;

        let memory = self.memory.lock().unwrap();
        let a_ptr = memory.get_ptr(a)?;
        let partial_ptr = memory.get_ptr(&partial)?;
        drop(memory);

        let func = self
            .device
            .get_func("air_kernels", "sum_reduce_f32")
            .ok_or_else(|| RmiError::Compute("sum kernel not found".to_string()))?;

        // SAFETY: kernel launch with valid device pointers; shared memory size matches block size.
        unsafe {
            func.launch(
                LaunchConfig {
                    grid_dim: (blocks as u32, 1, 1),
                    block_dim: (THREADS_PER_BLOCK, 1, 1),
                    shared_mem_bytes: THREADS_PER_BLOCK * 4,
                },
                (a_ptr, partial_ptr, n as u32),
            )
        }
        .map_err(|e| RmiError::Compute(format!("Sum reduce failed: {}", e)))?;

        // Copy partial results to host and sum
        let partial_data = self.copy_to_host(&partial)?;
        let partial_floats: Vec<f32> = partial_data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        self.free(&partial)?;

        Ok(partial_floats.iter().map(|&x| x as f64).sum())
    }

    fn sum_axis(&self, a: &TensorHandle, axis: usize) -> Result<TensorHandle> {
        // For simplicity, fall back to CPU for axis reduction
        // A full implementation would have specialized kernels
        let data = self.copy_to_host(a)?;
        let floats: Vec<f32> = data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        let shape = &a.shape;
        let mut out_shape = shape.clone();
        out_shape.remove(axis);

        if out_shape.is_empty() {
            out_shape.push(1);
        }

        let out_numel: usize = out_shape.iter().product();
        let mut result = vec![0.0f32; out_numel];

        // Compute strides
        let mut strides = vec![1usize; shape.len()];
        for i in (0..shape.len() - 1).rev() {
            strides[i] = strides[i + 1] * shape[i + 1];
        }

        for i in 0..floats.len() {
            let mut idx = i;
            let mut out_idx = 0;
            let mut out_stride = 1;

            for d in (0..shape.len()).rev() {
                let coord = idx / strides[d];
                idx %= strides[d];

                if d != axis {
                    out_idx += coord * out_stride;
                    out_stride *= shape[d];
                }
            }

            result[out_idx] += floats[i];
        }

        self.from_slice_f32(&result, &out_shape)
    }

    fn mean(&self, a: &TensorHandle) -> Result<f64> {
        let sum = self.sum(a)?;
        Ok(sum / a.numel() as f64)
    }

    fn mean_axis(&self, a: &TensorHandle, axis: usize) -> Result<TensorHandle> {
        let sum = self.sum_axis(a, axis)?;
        let divisor = a.shape[axis] as f64;
        self.scale(&sum, 1.0 / divisor)
    }

    fn max(&self, a: &TensorHandle) -> Result<f64> {
        let data = self.copy_to_host(a)?;
        let floats: Vec<f32> = data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        floats
            .iter()
            .cloned()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|x| x as f64)
            .ok_or_else(|| RmiError::Compute("Empty tensor".to_string()))
    }

    fn min(&self, a: &TensorHandle) -> Result<f64> {
        let data = self.copy_to_host(a)?;
        let floats: Vec<f32> = data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        floats
            .iter()
            .cloned()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|x| x as f64)
            .ok_or_else(|| RmiError::Compute("Empty tensor".to_string()))
    }

    // ==================== Activation Functions ====================

    fn relu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let out = self.allocate(&a.shape, a.dtype)?;
        self.launch_unary_kernel("relu_f32", a, &out)?;
        Ok(out)
    }

    fn gelu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let out = self.allocate(&a.shape, a.dtype)?;
        self.launch_unary_kernel("gelu_f32", a, &out)?;
        Ok(out)
    }

    fn sigmoid(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let out = self.allocate(&a.shape, a.dtype)?;
        self.launch_unary_kernel("sigmoid_f32", a, &out)?;
        Ok(out)
    }

    fn tanh(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let out = self.allocate(&a.shape, a.dtype)?;
        self.launch_unary_kernel("tanh_f32", a, &out)?;
        Ok(out)
    }

    fn softmax(&self, a: &TensorHandle, axis: i32) -> Result<TensorHandle> {
        // Softmax: exp(x - max(x)) / sum(exp(x - max(x)))
        // For simplicity, compute on CPU (production would use cuDNN)
        let data = self.copy_to_host(a)?;
        let floats: Vec<f32> = data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        let axis_idx = if axis < 0 {
            (a.shape.len() as i32 + axis) as usize
        } else {
            axis as usize
        };

        let axis_size = a.shape[axis_idx];
        let outer_size: usize = a.shape[..axis_idx].iter().product();
        let inner_size: usize = a.shape[axis_idx + 1..].iter().product();

        let mut result = vec![0.0f32; floats.len()];

        for outer in 0..outer_size.max(1) {
            for inner in 0..inner_size.max(1) {
                // Find max
                let mut max_val = f32::NEG_INFINITY;
                for a_idx in 0..axis_size {
                    let idx = outer * axis_size * inner_size + a_idx * inner_size + inner;
                    max_val = max_val.max(floats[idx]);
                }

                // Compute exp and sum
                let mut sum = 0.0f32;
                for a_idx in 0..axis_size {
                    let idx = outer * axis_size * inner_size + a_idx * inner_size + inner;
                    let exp_val = (floats[idx] - max_val).exp();
                    result[idx] = exp_val;
                    sum += exp_val;
                }

                // Normalize
                for a_idx in 0..axis_size {
                    let idx = outer * axis_size * inner_size + a_idx * inner_size + inner;
                    result[idx] /= sum;
                }
            }
        }

        self.from_slice_f32(&result, &a.shape)
    }

    // ==================== Shape Operations ====================

    fn reshape(&self, a: &TensorHandle, new_shape: &[usize]) -> Result<TensorHandle> {
        let old_numel = a.numel();
        let new_numel: usize = new_shape.iter().product();

        if old_numel != new_numel {
            return Err(RmiError::Compute(format!(
                "Cannot reshape {} elements to {:?}",
                old_numel, new_shape
            )));
        }

        // Reshape is just a view change - copy with new shape
        let out = self.allocate(new_shape, a.dtype)?;
        self.copy(a, &out)?;
        Ok(out)
    }

    fn transpose(&self, a: &TensorHandle, axes: &[usize]) -> Result<TensorHandle> {
        // General transpose - CPU fallback for now
        let data = self.copy_to_host(a)?;
        let floats: Vec<f32> = data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        let old_shape = &a.shape;
        let new_shape: Vec<usize> = axes.iter().map(|&i| old_shape[i]).collect();

        // Compute strides
        let mut old_strides = vec![1usize; old_shape.len()];
        for i in (0..old_shape.len() - 1).rev() {
            old_strides[i] = old_strides[i + 1] * old_shape[i + 1];
        }

        let mut new_strides = vec![1usize; new_shape.len()];
        for i in (0..new_shape.len() - 1).rev() {
            new_strides[i] = new_strides[i + 1] * new_shape[i + 1];
        }

        let numel = floats.len();
        let mut result = vec![0.0f32; numel];

        for i in 0..numel {
            // Compute old indices
            let mut remaining = i;
            let mut old_indices = vec![0usize; old_shape.len()];
            for d in 0..old_shape.len() {
                old_indices[d] = remaining / old_strides[d];
                remaining %= old_strides[d];
            }

            // Map to new indices
            let new_indices: Vec<usize> = axes.iter().map(|&a| old_indices[a]).collect();

            // Compute new linear index
            let new_idx: usize = new_indices
                .iter()
                .zip(new_strides.iter())
                .map(|(&i, &s)| i * s)
                .sum();

            result[new_idx] = floats[i];
        }

        self.from_slice_f32(&result, &new_shape)
    }

    fn concat(&self, tensors: &[&TensorHandle], axis: usize) -> Result<TensorHandle> {
        if tensors.is_empty() {
            return Err(RmiError::Compute(
                "No tensors to concatenate".to_string(),
            ));
        }

        let first = tensors[0];

        // Validate shapes match except on concat axis
        for t in tensors.iter().skip(1) {
            if t.shape.len() != first.shape.len() {
                return Err(RmiError::Compute(
                    "Rank mismatch in concat".to_string(),
                ));
            }
            for (d, (&a, &b)) in first.shape.iter().zip(t.shape.iter()).enumerate() {
                if d != axis && a != b {
                    return Err(RmiError::Compute(format!(
                        "Shape mismatch at dim {}: {} vs {}",
                        d, a, b
                    )));
                }
            }
        }

        // Compute output shape
        let mut out_shape = first.shape.clone();
        out_shape[axis] = tensors.iter().map(|t| t.shape[axis]).sum();

        // Gather all data
        let mut all_floats: Vec<Vec<f32>> = Vec::new();
        for t in tensors {
            let data = self.copy_to_host(t)?;
            let floats: Vec<f32> = data
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            all_floats.push(floats);
        }

        // Simple concat for 1D/2D
        let out_numel: usize = out_shape.iter().product();
        let mut result = vec![0.0f32; out_numel];

        // For each position in output, find source tensor and index
        let mut out_strides = vec![1usize; out_shape.len()];
        for i in (0..out_shape.len() - 1).rev() {
            out_strides[i] = out_strides[i + 1] * out_shape[i + 1];
        }

        for out_idx in 0..out_numel {
            // Compute coordinates
            let mut remaining = out_idx;
            let mut coords = vec![0usize; out_shape.len()];
            for d in 0..out_shape.len() {
                coords[d] = remaining / out_strides[d];
                remaining %= out_strides[d];
            }

            // Find which tensor and local index
            let axis_coord = coords[axis];
            let mut tensor_idx = 0;
            let mut offset = 0;
            for (i, t) in tensors.iter().enumerate() {
                if axis_coord < offset + t.shape[axis] {
                    tensor_idx = i;
                    coords[axis] = axis_coord - offset;
                    break;
                }
                offset += t.shape[axis];
            }

            // Compute source linear index
            let src_shape = &tensors[tensor_idx].shape;
            let mut src_strides = vec![1usize; src_shape.len()];
            for i in (0..src_shape.len() - 1).rev() {
                src_strides[i] = src_strides[i + 1] * src_shape[i + 1];
            }

            let src_idx: usize = coords
                .iter()
                .zip(src_strides.iter())
                .map(|(&c, &s)| c * s)
                .sum();

            result[out_idx] = all_floats[tensor_idx][src_idx];
        }

        self.from_slice_f32(&result, &out_shape)
    }

    fn split(&self, a: &TensorHandle, axis: usize, sections: usize) -> Result<Vec<TensorHandle>> {
        if a.shape[axis] % sections != 0 {
            return Err(RmiError::Compute(format!(
                "Cannot split {} into {} equal parts",
                a.shape[axis], sections
            )));
        }

        let section_size = a.shape[axis] / sections;
        let mut results = Vec::new();

        let data = self.copy_to_host(a)?;
        let floats: Vec<f32> = data
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        let mut out_shape = a.shape.clone();
        out_shape[axis] = section_size;

        for section in 0..sections {
            let start = section * section_size;
            let out_numel: usize = out_shape.iter().product();
            let mut result = vec![0.0f32; out_numel];

            // Copy section
            let mut out_strides = vec![1usize; out_shape.len()];
            for i in (0..out_shape.len() - 1).rev() {
                out_strides[i] = out_strides[i + 1] * out_shape[i + 1];
            }

            let mut in_strides = vec![1usize; a.shape.len()];
            for i in (0..a.shape.len() - 1).rev() {
                in_strides[i] = in_strides[i + 1] * a.shape[i + 1];
            }

            for out_idx in 0..out_numel {
                let mut remaining = out_idx;
                let mut coords = vec![0usize; out_shape.len()];
                for d in 0..out_shape.len() {
                    coords[d] = remaining / out_strides[d];
                    remaining %= out_strides[d];
                }

                coords[axis] += start;

                let in_idx: usize = coords
                    .iter()
                    .zip(in_strides.iter())
                    .map(|(&c, &s)| c * s)
                    .sum();

                result[out_idx] = floats[in_idx];
            }

            results.push(self.from_slice_f32(&result, &out_shape)?);
        }

        Ok(results)
    }

    // ==================== Synchronization ====================

    fn synchronize(&self) -> Result<()> {
        // SAFETY: FFI call to CUDA driver API; result is checked for success.
        unsafe {
            let result = cudarc::driver::sys::cuCtxSynchronize();
            if result != cudarc::driver::sys::CUresult::CUDA_SUCCESS {
                return Err(RmiError::Compute(format!(
                    "CUDA synchronize failed: {:?}",
                    result
                )));
            }
        }
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // These tests require CUDA hardware to run
    // They are ignored by default

    #[test]
    #[ignore]
    fn test_cuda_backend_creation() {
        let backend = CudaBackend::new().unwrap();
        assert!(backend.is_available());
        assert_eq!(backend.backend_type(), BackendType::Cuda);
    }

    #[test]
    #[ignore]
    fn test_cuda_tensor_allocation() {
        let backend = CudaBackend::new().unwrap();
        let handle = backend.allocate(&[1024, 1024], DType::F32).unwrap();
        assert_eq!(handle.shape, vec![1024, 1024]);
        assert_eq!(handle.numel(), 1024 * 1024);
        backend.free(&handle).unwrap();
    }

    #[test]
    #[ignore]
    fn test_cuda_elementwise_add() {
        let backend = CudaBackend::new().unwrap();

        let a_data: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let b_data: Vec<f32> = (0..100).map(|i| (i * 2) as f32).collect();

        let a = backend.from_slice_f32(&a_data, &[10, 10]).unwrap();
        let b = backend.from_slice_f32(&b_data, &[10, 10]).unwrap();

        let c = backend.add(&a, &b).unwrap();
        backend.synchronize().unwrap();

        let c_host = backend.copy_to_host(&c).unwrap();
        let c_floats: Vec<f32> = c_host
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        for i in 0..100 {
            assert!((c_floats[i] - (i as f32 * 3.0)).abs() < 1e-5);
        }
    }

    #[test]
    #[ignore]
    fn test_cuda_matmul() {
        let backend = CudaBackend::new().unwrap();

        // 2x3 @ 3x2 = 2x2
        let a_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let b_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];

        let a = backend.from_slice_f32(&a_data, &[2, 3]).unwrap();
        let b = backend.from_slice_f32(&b_data, &[3, 2]).unwrap();

        let c = backend.matmul(&a, &b).unwrap();
        backend.synchronize().unwrap();

        assert_eq!(c.shape, vec![2, 2]);

        // Expected: [[22, 28], [49, 64]]
        let c_host = backend.copy_to_host(&c).unwrap();
        let c_floats: Vec<f32> = c_host
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        assert!((c_floats[0] - 22.0).abs() < 1e-4);
        assert!((c_floats[1] - 28.0).abs() < 1e-4);
        assert!((c_floats[2] - 49.0).abs() < 1e-4);
        assert!((c_floats[3] - 64.0).abs() < 1e-4);
    }

    #[test]
    #[ignore]
    fn test_cuda_relu() {
        let backend = CudaBackend::new().unwrap();

        let data: Vec<f32> = vec![-2.0, -1.0, 0.0, 1.0, 2.0];
        let x = backend.from_slice_f32(&data, &[5]).unwrap();

        let y = backend.relu(&x).unwrap();
        backend.synchronize().unwrap();

        let y_host = backend.copy_to_host(&y).unwrap();
        let y_floats: Vec<f32> = y_host
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        assert!((y_floats[0] - 0.0).abs() < 1e-5);
        assert!((y_floats[1] - 0.0).abs() < 1e-5);
        assert!((y_floats[2] - 0.0).abs() < 1e-5);
        assert!((y_floats[3] - 1.0).abs() < 1e-5);
        assert!((y_floats[4] - 2.0).abs() < 1e-5);
    }

    #[test]
    #[ignore]
    fn test_cuda_sum_reduction() {
        let backend = CudaBackend::new().unwrap();

        let n = 10000;
        let data: Vec<f32> = (0..n).map(|i| 1.0).collect();
        let x = backend.from_slice_f32(&data, &[n]).unwrap();

        let sum = backend.sum(&x).unwrap();
        backend.synchronize().unwrap();

        assert!((sum - n as f64).abs() < 1.0);
    }
}