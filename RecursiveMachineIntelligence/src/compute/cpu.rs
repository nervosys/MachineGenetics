//! CPU Compute Backend
//!
//! Provides CPU-based tensor operations using ndarray and rayon
//! for parallelization.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use half::{bf16, f16};
use ndarray::{ArrayD, Axis, IxDyn};
use rand::distributions::{Distribution, Uniform};
use rand_distr::StandardNormal;

use super::{Backend, BackendType, DType, DeviceInfo, TensorHandle};
use crate::error::{Result, RmiError};

/// CPU backend implementation.
pub struct CpuBackend {
    device_info: DeviceInfo,
    next_id: AtomicU64,
    /// Storage for tensor data
    storage: RwLock<HashMap<u64, TensorData>>,
}

struct TensorData {
    data: Arc<Vec<u8>>,
    #[allow(dead_code)]
    shape: Vec<usize>,
    #[allow(dead_code)]
    dtype: DType,
}

impl CpuBackend {
    /// Create a new CPU backend.
    pub fn new() -> Self {
        let num_cpus = num_cpus::get() as u32;
        // Default memory value (8GB) - sys_info removed as it's not a dependency
        let total_memory: u64 = 8 * 1024 * 1024 * 1024;

        Self {
            device_info: DeviceInfo {
                name: "CPU".to_string(),
                backend_type: BackendType::Cpu,
                total_memory,
                available_memory: total_memory / 2, // Conservative estimate
                compute_capability: None,
                compute_units: num_cpus,
            },
            next_id: AtomicU64::new(1),
            storage: RwLock::new(HashMap::new()),
        }
    }

    #[inline]
    fn get_next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    fn get_data(&self, handle: &TensorHandle) -> Result<Arc<Vec<u8>>> {
        let storage = self.storage.read().unwrap();
        storage
            .get(&handle.id)
            .map(|td| td.data.clone()) // Arc clone = O(1) ref count increment
            .ok_or_else(|| RmiError::compute_simple(format!("Tensor {} not found", handle.id)))
    }

    fn get_f32_array(&self, handle: &TensorHandle) -> Result<ArrayD<f32>> {
        let data = self.get_data(handle)?;
        let values: Vec<f32> = data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        ArrayD::from_shape_vec(IxDyn(&handle.shape), values)
            .map_err(|e| RmiError::compute_simple(e.to_string()))
    }

    fn store_f32_array(&self, array: &ArrayD<f32>) -> Result<TensorHandle> {
        let id = self.get_next_id();
        let shape = array.shape().to_vec();
        let numel: usize = shape.iter().product();

        let data: Vec<u8> = array.iter().flat_map(|f| f.to_le_bytes()).collect();

        let handle = TensorHandle {
            id,
            shape: shape.clone(),
            dtype: DType::F32,
            backend: BackendType::Cpu,
            size_bytes: numel * 4,
        };

        self.storage.write().unwrap().insert(
            id,
            TensorData {
                data: Arc::new(data),
                shape,
                dtype: DType::F32,
            },
        );

        Ok(handle)
    }

    /// Store raw bytes as a tensor of the given shape and dtype.
    fn store_bytes(&self, data: Vec<u8>, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        let id = self.get_next_id();
        let numel: usize = shape.iter().product();
        let handle = TensorHandle {
            id,
            shape: shape.to_vec(),
            dtype,
            backend: BackendType::Cpu,
            size_bytes: numel * dtype.size_bytes(),
        };
        self.storage.write().unwrap().insert(
            id,
            TensorData {
                data: Arc::new(data),
                shape: shape.to_vec(),
                dtype,
            },
        );
        Ok(handle)
    }

    /// Decode a tensor's raw bytes into `f32` values, interpreting them
    /// per the handle's dtype. Supports F32/F64/F16/BF16.
    fn decode_to_f32(&self, handle: &TensorHandle) -> Result<Vec<f32>> {
        let bytes = self.get_data(handle)?;
        let v = match handle.dtype {
            DType::F32 => bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
            DType::F64 => bytes
                .chunks_exact(8)
                .map(|c| {
                    f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) as f32
                })
                .collect(),
            DType::F16 => bytes
                .chunks_exact(2)
                .map(|c| f16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
                .collect(),
            DType::BF16 => bytes
                .chunks_exact(2)
                .map(|c| bf16::from_bits(u16::from_le_bytes([c[0], c[1]])).to_f32())
                .collect(),
            other => {
                return Err(RmiError::compute_simple(format!(
                    "cast: source dtype {other:?} not supported"
                )))
            }
        };
        Ok(v)
    }
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for CpuBackend {
    #[inline]
    fn backend_type(&self) -> BackendType {
        BackendType::Cpu
    }

    #[inline]
    fn device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    #[inline]
    fn is_available(&self) -> bool {
        true
    }

    // ==================== Memory Management ====================

    fn allocate(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        let id = self.get_next_id();
        let numel: usize = shape.iter().product();
        let size_bytes = numel * dtype.size_bytes();

        let handle = TensorHandle {
            id,
            shape: shape.to_vec(),
            dtype,
            backend: BackendType::Cpu,
            size_bytes,
        };

        self.storage.write().unwrap().insert(
            id,
            TensorData {
                data: Arc::new(vec![0u8; size_bytes]),
                shape: shape.to_vec(),
                dtype,
            },
        );

        Ok(handle)
    }

    fn free(&self, handle: &TensorHandle) -> Result<()> {
        self.storage.write().unwrap().remove(&handle.id);
        Ok(())
    }

    fn copy_to_device(&self, handle: &TensorHandle, data: &[u8]) -> Result<()> {
        let mut storage = self.storage.write().unwrap();
        if let Some(td) = storage.get_mut(&handle.id) {
            if data.len() <= td.data.len() {
                let buf = Arc::make_mut(&mut td.data);
                buf[..data.len()].copy_from_slice(data);
                Ok(())
            } else {
                Err(RmiError::compute_simple("Data too large"))
            }
        } else {
            Err(RmiError::compute_simple(format!(
                "Tensor {} not found",
                handle.id
            )))
        }
    }

    fn copy_to_host(&self, handle: &TensorHandle) -> Result<Vec<u8>> {
        let storage = self.storage.read().unwrap();
        storage
            .get(&handle.id)
            .map(|td| (*td.data).clone())
            .ok_or_else(|| RmiError::compute_simple(format!("Tensor {} not found", handle.id)))
    }

    fn copy(&self, src: &TensorHandle, dst: &TensorHandle) -> Result<()> {
        let data = self.get_data(src)?;
        self.copy_to_device(dst, &data)
    }

    // ==================== Tensor Creation ====================

    fn zeros(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        self.allocate(shape, dtype)
    }

    fn ones(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        let handle = self.allocate(shape, dtype)?;

        match dtype {
            DType::F32 => {
                let numel: usize = shape.iter().product();
                let data: Vec<u8> = std::iter::repeat(1.0f32.to_le_bytes())
                    .take(numel)
                    .flatten()
                    .collect();
                self.copy_to_device(&handle, &data)?;
            }
            DType::F64 => {
                let numel: usize = shape.iter().product();
                let data: Vec<u8> = std::iter::repeat(1.0f64.to_le_bytes())
                    .take(numel)
                    .flatten()
                    .collect();
                self.copy_to_device(&handle, &data)?;
            }
            _ => {
                // For other types, fill with 1s
                let mut storage = self.storage.write().unwrap();
                if let Some(td) = storage.get_mut(&handle.id) {
                    Arc::make_mut(&mut td.data).fill(1);
                }
            }
        }

        Ok(handle)
    }

    fn rand(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        let handle = self.allocate(shape, dtype)?;
        let numel: usize = shape.iter().product();

        let mut rng = rand::thread_rng();
        let dist = Uniform::new(0.0f32, 1.0f32);

        let data: Vec<u8> = (0..numel)
            .map(|_| dist.sample(&mut rng))
            .flat_map(|f| f.to_le_bytes())
            .collect();

        self.copy_to_device(&handle, &data)?;
        Ok(handle)
    }

    fn randn(&self, shape: &[usize], dtype: DType) -> Result<TensorHandle> {
        let handle = self.allocate(shape, dtype)?;
        let numel: usize = shape.iter().product();

        let mut rng = rand::thread_rng();

        let data: Vec<u8> = (0..numel)
            .map(|_| {
                let sample: f64 = StandardNormal.sample(&mut rng);
                sample as f32
            })
            .flat_map(|f| f.to_le_bytes())
            .collect();

        self.copy_to_device(&handle, &data)?;
        Ok(handle)
    }

    fn from_slice_f32(&self, data: &[f32], shape: &[usize]) -> Result<TensorHandle> {
        let numel: usize = shape.iter().product();
        if data.len() != numel {
            return Err(RmiError::shape_mismatch_simple(format!(
                "expected {} elements for shape {:?}, got {}",
                numel,
                shape,
                data.len()
            )));
        }

        let handle = self.allocate(shape, DType::F32)?;
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.copy_to_device(&handle, &bytes)?;
        Ok(handle)
    }

    // ==================== Arithmetic Operations ====================

    fn add(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let b_arr = self.get_f32_array(b)?;
        let result = &a_arr + &b_arr;
        self.store_f32_array(&result)
    }

    fn sub(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let b_arr = self.get_f32_array(b)?;
        let result = &a_arr - &b_arr;
        self.store_f32_array(&result)
    }

    fn mul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let b_arr = self.get_f32_array(b)?;
        let result = &a_arr * &b_arr;
        self.store_f32_array(&result)
    }

    fn div(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let b_arr = self.get_f32_array(b)?;
        let result = &a_arr / &b_arr;
        self.store_f32_array(&result)
    }

    fn matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let b_arr = self.get_f32_array(b)?;

        // Handle 2D matrix multiplication
        if a_arr.ndim() == 2 && b_arr.ndim() == 2 {
            let a_2d = a_arr
                .into_dimensionality::<ndarray::Ix2>()
                .map_err(|e| RmiError::compute_simple(e.to_string()))?;
            let b_2d = b_arr
                .into_dimensionality::<ndarray::Ix2>()
                .map_err(|e| RmiError::compute_simple(e.to_string()))?;

            // For larger matrices, use BLAS (f64 tiled) for higher quality
            let m = a_2d.nrows();
            let k = a_2d.ncols();
            let n = b_2d.ncols();
            if m >= 32 && n >= 32 {
                use crate::compute::blas::{BlasMatrix, BlasOps};
                let a_blas = BlasMatrix::from_vec(m, k, a_2d.iter().map(|&v| v as f64).collect());
                let b_blas = BlasMatrix::from_vec(k, n, b_2d.iter().map(|&v| v as f64).collect());
                let c_blas = BlasOps::matmul(&a_blas, &b_blas)
                    .map_err(|e| RmiError::compute_simple(e.to_string()))?;
                let c_f32: Vec<f32> = c_blas.data.iter().map(|&v| v as f32).collect();
                let result = ArrayD::from_shape_vec(IxDyn(&[m, n]), c_f32)
                    .map_err(|e| RmiError::compute_simple(e.to_string()))?;
                return self.store_f32_array(&result);
            }

            let result = a_2d.dot(&b_2d);
            let result_dyn = result.into_dyn();
            self.store_f32_array(&result_dyn)
        } else {
            Err(RmiError::compute_simple(
                "Only 2D matmul currently supported",
            ))
        }
    }

    fn quantized_matmul(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        // P133: real INT8 quantized matmul on CPU. Symmetric per-tensor
        // activation, symmetric per-column weight, INT32 accumulate,
        // dequantize. Mirrors the CUDA math so CPU is a faithful oracle
        // (and quantization isn't CUDA-only). 2-D F32 only; else exact F32.
        if a.dtype != DType::F32 || b.dtype != DType::F32 || a.shape.len() != 2 || b.shape.len() != 2
        {
            return self.matmul(a, b);
        }
        let (m, k) = (a.shape[0], a.shape[1]);
        let (k2, n) = (b.shape[0], b.shape[1]);
        if k != k2 {
            return self.matmul(a, b);
        }
        let av = self.get_f32_array(a)?.into_raw_vec();
        let bv = self.get_f32_array(b)?.into_raw_vec();
        // Per-tensor activation scale.
        let a_amax = av.iter().fold(0.0f32, |mx, &x| mx.max(x.abs()));
        let sa = if a_amax > 0.0 { a_amax / 127.0 } else { 1.0 };
        // Per-column weight scales.
        let mut sb = vec![1.0f32; n];
        for (c, s) in sb.iter_mut().enumerate() {
            let mut amax = 0.0f32;
            for r in 0..k {
                amax = amax.max(bv[r * n + c].abs());
            }
            if amax > 0.0 {
                *s = amax / 127.0;
            }
        }
        // Quantize.
        let qa: Vec<i32> = av.iter().map(|&x| (x / sa).round().clamp(-127.0, 127.0) as i32).collect();
        let mut qb = vec![0i32; k * n];
        for r in 0..k {
            for c in 0..n {
                qb[r * n + c] = (bv[r * n + c] / sb[c]).round().clamp(-127.0, 127.0) as i32;
            }
        }
        // INT32-accumulate GEMM, dequant per column.
        let mut out = vec![0.0f32; m * n];
        for r in 0..m {
            for c in 0..n {
                let mut acc = 0i32;
                for kk in 0..k {
                    acc += qa[r * k + kk] * qb[kk * n + c];
                }
                out[r * n + c] = acc as f32 * sa * sb[c];
            }
        }
        let arr = ArrayD::from_shape_vec(IxDyn(&[m, n]), out)
            .map_err(|e| RmiError::compute_simple(e.to_string()))?;
        self.store_f32_array(&arr)
    }

    fn quantized_matmul_asym_calibrated(
        &self,
        a: &TensorHandle,
        a_lo: f32,
        a_hi: f32,
        b: &TensorHandle,
    ) -> Result<TensorHandle> {
        // P138: real asymmetric (zero-point) INT8 on CPU. Activation
        // quantized over the calibrated [lo,hi] (full int8 range even
        // when one-sided), symmetric per-column weights, INT32
        // accumulate with the exact zero-point correction
        // A·B = sa·sb·(Σqa·qb − za·Σqb). Mirrors the CUDA P134 math.
        if a.dtype != DType::F32 || b.dtype != DType::F32 || a.shape.len() != 2 || b.shape.len() != 2
            || a_hi <= a_lo
        {
            return self.quantized_matmul(a, b);
        }
        let (m, k) = (a.shape[0], a.shape[1]);
        let (k2, n) = (b.shape[0], b.shape[1]);
        if k != k2 {
            return self.quantized_matmul(a, b);
        }
        let av = self.get_f32_array(a)?.into_raw_vec();
        let bv = self.get_f32_array(b)?.into_raw_vec();
        // Asymmetric activation quant: sa = range/255, za maps lo→-128.
        let sa = (a_hi - a_lo).max(1e-12) / 255.0;
        let za = (-128.0 - (a_lo / sa).round()) as i32;
        let qa: Vec<i32> = av
            .iter()
            .map(|&x| ((x / sa).round() as i32 + za).clamp(-128, 127))
            .collect();
        // Symmetric per-column weight quant + column sums.
        let mut sb = vec![1.0f32; n];
        for (c, s) in sb.iter_mut().enumerate() {
            let mut amax = 0.0f32;
            for r in 0..k {
                amax = amax.max(bv[r * n + c].abs());
            }
            if amax > 0.0 {
                *s = amax / 127.0;
            }
        }
        let mut qb = vec![0i32; k * n];
        let mut colsum = vec![0i32; n];
        for r in 0..k {
            for c in 0..n {
                let q = (bv[r * n + c] / sb[c]).round().clamp(-127.0, 127.0) as i32;
                qb[r * n + c] = q;
                colsum[c] += q;
            }
        }
        let mut out = vec![0.0f32; m * n];
        for r in 0..m {
            for c in 0..n {
                let mut acc = 0i32;
                for kk in 0..k {
                    acc += qa[r * k + kk] * qb[kk * n + c];
                }
                out[r * n + c] = sa * sb[c] * (acc - za * colsum[c]) as f32;
            }
        }
        let arr = ArrayD::from_shape_vec(IxDyn(&[m, n]), out)
            .map_err(|e| RmiError::compute_simple(e.to_string()))?;
        self.store_f32_array(&arr)
    }

    fn scale(&self, a: &TensorHandle, scalar: f64) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let result = &a_arr * (scalar as f32);
        self.store_f32_array(&result)
    }

    // ==================== Reduction Operations ====================

    fn sum(&self, a: &TensorHandle) -> Result<f64> {
        let a_arr = self.get_f32_array(a)?;
        Ok(a_arr.sum() as f64)
    }

    fn sum_axis(&self, a: &TensorHandle, axis: usize) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let result = a_arr.sum_axis(Axis(axis));
        self.store_f32_array(&result.into_dyn())
    }

    fn mean(&self, a: &TensorHandle) -> Result<f64> {
        let a_arr = self.get_f32_array(a)?;
        Ok(a_arr.mean().unwrap_or(0.0) as f64)
    }

    fn mean_axis(&self, a: &TensorHandle, axis: usize) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let result = a_arr
            .mean_axis(Axis(axis))
            .ok_or_else(|| RmiError::compute_simple("Mean failed"))?;
        self.store_f32_array(&result.into_dyn())
    }

    fn max(&self, a: &TensorHandle) -> Result<f64> {
        let a_arr = self.get_f32_array(a)?;
        Ok(*a_arr
            .iter()
            .max_by(|x, y| x.partial_cmp(y).expect("NaN in tensor during max()"))
            .unwrap_or(&0.0) as f64)
    }

    fn min(&self, a: &TensorHandle) -> Result<f64> {
        let a_arr = self.get_f32_array(a)?;
        Ok(*a_arr
            .iter()
            .min_by(|x, y| x.partial_cmp(y).expect("NaN in tensor during min()"))
            .unwrap_or(&0.0) as f64)
    }

    // ==================== Activation Functions ====================

    fn relu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let result = a_arr.mapv(|x| x.max(0.0));
        self.store_f32_array(&result)
    }

    fn gelu(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        // Approximate GELU: 0.5 * x * (1 + tanh(sqrt(2/π) * (x + 0.044715 * x^3)))
        let sqrt_2_pi = (2.0f32 / std::f32::consts::PI).sqrt();
        let result =
            a_arr.mapv(|x| 0.5 * x * (1.0 + (sqrt_2_pi * (x + 0.044715 * x.powi(3))).tanh()));
        self.store_f32_array(&result)
    }

    fn sigmoid(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let result = a_arr.mapv(|x| 1.0 / (1.0 + (-x).exp()));
        self.store_f32_array(&result)
    }

    fn tanh(&self, a: &TensorHandle) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let result = a_arr.mapv(|x| x.tanh());
        self.store_f32_array(&result)
    }

    fn softmax(&self, a: &TensorHandle, axis: i32) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let axis_usize = if axis < 0 {
            (a_arr.ndim() as i32 + axis) as usize
        } else {
            axis as usize
        };

        // Compute softmax along axis
        let max_vals = a_arr.map_axis(Axis(axis_usize), |lane| {
            *lane
                .iter()
                .max_by(|x, y| x.partial_cmp(y).expect("NaN in softmax lane"))
                .expect("empty lane in softmax max reduction")
        });

        // Subtract max for numerical stability and compute exp.
        // `lanes_mut(Axis(k))` yields mutable 1-D lanes PARALLEL to axis k —
        // i.e. for `[3,5]` with k=1 it yields 3 lanes of length 5, one
        // per row. `max_vals` from `map_axis(Axis(k), ...)` is shaped to
        // match exactly (one max per lane).
        let mut result = a_arr.clone();
        let max_flat = max_vals
            .as_slice_memory_order()
            .expect("softmax max_vals must be contiguous");
        for (i, mut lane) in result.lanes_mut(Axis(axis_usize)).into_iter().enumerate() {
            let max_val = max_flat[i];
            lane.mapv_inplace(|x| (x - max_val).exp());
        }

        // Normalize.
        let sums = result.sum_axis(Axis(axis_usize));
        let sum_flat = sums
            .as_slice_memory_order()
            .expect("softmax sums must be contiguous");
        for (i, mut lane) in result.lanes_mut(Axis(axis_usize)).into_iter().enumerate() {
            let sum = sum_flat[i];
            lane.mapv_inplace(|x| x / sum);
        }

        self.store_f32_array(&result)
    }

    // ==================== Shape Operations ====================

    fn reshape(&self, a: &TensorHandle, new_shape: &[usize]) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let result = a_arr
            .into_shape(IxDyn(new_shape))
            .map_err(|e| RmiError::compute_simple(e.to_string()))?;
        self.store_f32_array(&result)
    }

    fn transpose(&self, a: &TensorHandle, axes: &[usize]) -> Result<TensorHandle> {
        let a_arr = self.get_f32_array(a)?;
        let result = a_arr.permuted_axes(IxDyn(axes));
        self.store_f32_array(&result.into_owned())
    }

    fn conv2d(
        &self,
        input: &TensorHandle,
        weight: &TensorHandle,
        stride: usize,
        padding: usize,
        dilation: usize,
    ) -> Result<TensorHandle> {
        // Reference 2-D convolution (cross-correlation), NCHW. Naive
        // nested loops with zero-padding + dilation — correctness over
        // speed; this is the oracle the GPU im2col+GEMM path is checked
        // against.
        if input.shape.len() != 4 || weight.shape.len() != 4 {
            return Err(RmiError::compute_simple(format!(
                "conv2d expects 4-D input [N,C,H,W] and weight [O,C,KH,KW], got {:?} and {:?}",
                input.shape, weight.shape
            )));
        }
        if stride == 0 || dilation == 0 {
            return Err(RmiError::compute_simple(
                "conv2d stride and dilation must be >= 1",
            ));
        }
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
        // Effective (dilated) kernel extent.
        let eff_h = dilation * (kh - 1) + 1;
        let eff_w = dilation * (kw - 1) + 1;
        if h + 2 * padding < eff_h || w + 2 * padding < eff_w {
            return Err(RmiError::compute_simple(
                "conv2d dilated kernel larger than padded input",
            ));
        }
        let hout = (h + 2 * padding - eff_h) / stride + 1;
        let wout = (w + 2 * padding - eff_w) / stride + 1;

        let x = self.get_f32_array(input)?;
        let wt = self.get_f32_array(weight)?;
        let mut out = ArrayD::<f32>::zeros(IxDyn(&[n, cout, hout, wout]));

        for ni in 0..n {
            for co in 0..cout {
                for ho in 0..hout {
                    for wo in 0..wout {
                        let mut acc = 0.0f32;
                        for ci in 0..cin {
                            for ky in 0..kh {
                                let hi = ho * stride + ky * dilation;
                                if hi < padding || hi - padding >= h {
                                    continue;
                                }
                                let hh = hi - padding;
                                for kx in 0..kw {
                                    let wi = wo * stride + kx * dilation;
                                    if wi < padding || wi - padding >= w {
                                        continue;
                                    }
                                    let ww = wi - padding;
                                    acc += x[[ni, ci, hh, ww]] * wt[[co, ci, ky, kx]];
                                }
                            }
                        }
                        out[[ni, co, ho, wo]] = acc;
                    }
                }
            }
        }
        self.store_f32_array(&out)
    }

    fn cast(&self, a: &TensorHandle, target: DType) -> Result<TensorHandle> {
        // Same dtype → identity copy.
        if a.dtype == target {
            let bytes = self.get_data(a)?;
            return self.store_bytes((*bytes).clone(), &a.shape, target);
        }
        // Decode source → f32, then encode to the target dtype. F32 is
        // the pivot for all half/float conversions.
        let vals = self.decode_to_f32(a)?;
        let bytes: Vec<u8> = match target {
            DType::F32 => vals.iter().flat_map(|v| v.to_le_bytes()).collect(),
            DType::F64 => vals.iter().flat_map(|v| (*v as f64).to_le_bytes()).collect(),
            DType::F16 => vals
                .iter()
                .flat_map(|v| f16::from_f32(*v).to_bits().to_le_bytes())
                .collect(),
            DType::BF16 => vals
                .iter()
                .flat_map(|v| bf16::from_f32(*v).to_bits().to_le_bytes())
                .collect(),
            other => {
                return Err(RmiError::compute_simple(format!(
                    "cast: target dtype {other:?} not supported"
                )))
            }
        };
        self.store_bytes(bytes, &a.shape, target)
    }

    fn concat(&self, tensors: &[&TensorHandle], axis: usize) -> Result<TensorHandle> {
        if tensors.is_empty() {
            return Err(RmiError::compute_simple("No tensors to concatenate"));
        }

        let arrays: Vec<ArrayD<f32>> = tensors
            .iter()
            .map(|h| self.get_f32_array(h))
            .collect::<Result<Vec<_>>>()?;

        let views: Vec<_> = arrays.iter().map(|a| a.view()).collect();
        let result = ndarray::concatenate(Axis(axis), &views)
            .map_err(|e| RmiError::compute_simple(e.to_string()))?;

        self.store_f32_array(&result)
    }

    fn split(&self, a: &TensorHandle, axis: usize, sections: usize) -> Result<Vec<TensorHandle>> {
        let a_arr = self.get_f32_array(a)?;
        let axis_len = a_arr.shape()[axis];

        if axis_len % sections != 0 {
            return Err(RmiError::compute_simple(
                "Array length must be divisible by sections",
            ));
        }

        let section_size = axis_len / sections;
        let mut results = Vec::with_capacity(sections);

        for i in 0..sections {
            let start = i * section_size;
            let end = start + section_size;
            let slice = a_arr.slice_axis(Axis(axis), ndarray::Slice::from(start..end));
            results.push(self.store_f32_array(&slice.into_owned())?);
        }

        Ok(results)
    }

    // ==================== Synchronization ====================

    fn synchronize(&self) -> Result<()> {
        // CPU is synchronous, nothing to do
        Ok(())
    }
}

// ── BLAS-backed linear algebra convenience methods ────────────────────────────

impl CpuBackend {
    /// Solve Ax = b using BLAS LU factorization.
    ///
    /// Both `a` (square matrix) and `b` (vector) must be f32 tensors.
    /// Returns the solution vector x as a 1-D tensor handle.
    pub fn solve(&self, a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle> {
        use crate::compute::blas::{BlasMatrix, BlasOps};

        let a_arr = self.get_f32_array(a)?;
        let b_arr = self.get_f32_array(b)?;
        let n = a.shape[0];

        let a_blas = BlasMatrix::from_vec(n, n, a_arr.iter().map(|&v| v as f64).collect());
        let b_f64: Vec<f64> = b_arr.iter().map(|&v| v as f64).collect();

        let x =
            BlasOps::solve(&a_blas, &b_f64).map_err(|e| RmiError::compute_simple(e.to_string()))?;
        let x_f32: Vec<f32> = x.iter().map(|&v| v as f32).collect();
        self.from_slice_f32(&x_f32, &[n])
    }

    /// Compute the determinant of a square matrix using BLAS LU.
    pub fn det(&self, a: &TensorHandle) -> Result<f64> {
        use crate::compute::blas::{BlasMatrix, BlasOps};

        let a_arr = self.get_f32_array(a)?;
        let n = a.shape[0];
        let a_blas = BlasMatrix::from_vec(n, n, a_arr.iter().map(|&v| v as f64).collect());
        BlasOps::det(&a_blas).map_err(|e| RmiError::compute_simple(e.to_string()))
    }

    /// Compute the matrix inverse using BLAS.
    pub fn inv(&self, a: &TensorHandle) -> Result<TensorHandle> {
        use crate::compute::blas::{BlasMatrix, BlasOps};

        let a_arr = self.get_f32_array(a)?;
        let n = a.shape[0];
        let a_blas = BlasMatrix::from_vec(n, n, a_arr.iter().map(|&v| v as f64).collect());
        let inv = BlasOps::inv(&a_blas).map_err(|e| RmiError::compute_simple(e.to_string()))?;
        let inv_f32: Vec<f32> = inv.data.iter().map(|&v| v as f32).collect();
        self.from_slice_f32(&inv_f32, &[n, n])
    }

    /// Cholesky decomposition (for symmetric positive-definite matrices).
    pub fn cholesky(&self, a: &TensorHandle) -> Result<TensorHandle> {
        use crate::compute::blas::{BlasMatrix, BlasOps};

        let a_arr = self.get_f32_array(a)?;
        let n = a.shape[0];
        let a_blas = BlasMatrix::from_vec(n, n, a_arr.iter().map(|&v| v as f64).collect());
        let l = BlasOps::cholesky(&a_blas).map_err(|e| RmiError::compute_simple(e.to_string()))?;
        let l_f32: Vec<f32> = l.data.iter().map(|&v| v as f32).collect();
        self.from_slice_f32(&l_f32, &[n, n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_asym_quantized_matmul_beats_symmetric_on_relu() {
        // P138: on all-positive (post-ReLU) activations, asymmetric
        // zero-point quant uses the full int8 range and beats symmetric.
        let be = CpuBackend::new();
        let (m, k, n) = (4usize, 8usize, 5usize);
        let av: Vec<f32> = (0..m * k).map(|i| ((i as f32 * 0.31).sin() * 0.5 + 0.5) * 4.0).collect();
        let bv: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.09).cos() * 0.8).collect();
        let a = be.from_slice_f32(&av, &[m, k]).unwrap();
        let w = be.from_slice_f32(&bv, &[k, n]).unwrap();
        let to_f32 = |h: &TensorHandle| -> Vec<f32> {
            be.copy_to_host(h)
                .unwrap()
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        };
        let want = to_f32(&be.matmul(&a, &w).unwrap());
        let (lo, hi) = av.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), &x| {
            (l.min(x), h.max(x))
        });
        let got_sym = to_f32(&be.quantized_matmul(&a, &w).unwrap());
        let got_asym = to_f32(&be.quantized_matmul_asym_calibrated(&a, lo, hi, &w).unwrap());
        let rel = |got: &[f32]| -> f32 {
            let (mut nu, mut de) = (0.0f32, 0.0f32);
            for (w0, g) in want.iter().zip(got) {
                nu += (w0 - g).abs();
                de += w0.abs();
            }
            nu / de.max(1e-9)
        };
        let (es, ea) = (rel(&got_sym), rel(&got_asym));
        assert!(ea < es, "asym ({ea}) should beat symmetric ({es}) on post-ReLU");
        assert!(ea < 0.02, "asym rel err {ea} too high");
    }

    #[test]
    fn cpu_quantized_matmul_matches_f32() {
        // P133: CPU INT8 quantized matmul tracks the F32 matmul within
        // quantization error — quantization is no longer CUDA-only.
        let b = CpuBackend::new();
        let (m, k, n) = (4usize, 6usize, 5usize);
        let av: Vec<f32> = (0..m * k).map(|i| ((i as f32) * 0.13).sin() * 0.8).collect();
        let bv: Vec<f32> = (0..k * n).map(|i| ((i as f32) * 0.09).cos() * 0.8).collect();
        let a = b.from_slice_f32(&av, &[m, k]).unwrap();
        let w = b.from_slice_f32(&bv, &[k, n]).unwrap();

        let want_h = b.matmul(&a, &w).unwrap();
        let want: Vec<f32> = b
            .copy_to_host(&want_h)
            .unwrap()
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        let got_h = b.quantized_matmul(&a, &w).unwrap();
        let got: Vec<f32> = b
            .copy_to_host(&got_h)
            .unwrap()
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(got.len(), m * n);
        let (mut num, mut den) = (0.0f32, 0.0f32);
        for (w0, g) in want.iter().zip(&got) {
            num += (w0 - g).abs();
            den += w0.abs();
        }
        assert!(num / den.max(1e-9) < 0.05, "CPU INT8 rel err too high");
    }

    #[test]
    fn cast_f32_half_roundtrip_and_dtype() {
        let b = CpuBackend::new();
        let v: Vec<f32> = (0..16).map(|i| i as f32 * 0.1 - 0.8).collect();
        let f = b.from_slice_f32(&v, &[4, 4]).unwrap();

        for (dt, tol) in [(DType::F16, 2e-3f32), (DType::BF16, 1e-2f32)] {
            let h = b.cast(&f, dt).unwrap();
            assert_eq!(h.dtype, dt, "cast result dtype");
            assert_eq!(h.shape, vec![4, 4]);
            assert_eq!(h.size_bytes, 16 * 2, "half is 2 bytes/elem");
            // half handle's raw payload is numel*2 bytes.
            assert_eq!(b.copy_to_host(&h).unwrap().len(), 32);

            // Round-trip back to F32 and compare.
            let back = b.cast(&h, DType::F32).unwrap();
            assert_eq!(back.dtype, DType::F32);
            let rb = b.copy_to_host(&back).unwrap();
            let rv: Vec<f32> = rb
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            for (a, r) in v.iter().zip(&rv) {
                assert!((a - r).abs() <= tol, "{dt:?} roundtrip {a} vs {r}");
            }
        }

        // F64 cast widens then narrows losslessly for these values.
        let d = b.cast(&f, DType::F64).unwrap();
        assert_eq!(d.dtype, DType::F64);
        assert_eq!(d.size_bytes, 16 * 8);
        let back = b.cast(&d, DType::F32).unwrap();
        let rv: Vec<f32> = b
            .copy_to_host(&back)
            .unwrap()
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(rv, v, "f32→f64→f32 is lossless");
    }

    #[test]
    fn test_cpu_backend_creation() {
        let backend = CpuBackend::new();
        assert!(backend.is_available());
        assert_eq!(backend.backend_type(), BackendType::Cpu);
    }

    #[test]
    fn test_tensor_creation() {
        let backend = CpuBackend::new();

        let zeros = backend.zeros(&[2, 3], DType::F32).unwrap();
        assert_eq!(zeros.shape, vec![2, 3]);
        assert_eq!(zeros.numel(), 6);

        let ones = backend.ones(&[2, 3], DType::F32).unwrap();
        let sum = backend.sum(&ones).unwrap();
        assert!((sum - 6.0).abs() < 1e-5);
    }

    #[test]
    fn test_arithmetic() {
        let backend = CpuBackend::new();

        let a = backend
            .from_slice_f32(&[1.0, 2.0, 3.0, 4.0], &[2, 2])
            .unwrap();
        let b = backend
            .from_slice_f32(&[5.0, 6.0, 7.0, 8.0], &[2, 2])
            .unwrap();

        let sum = backend.add(&a, &b).unwrap();
        let result = backend.copy_to_host(&sum).unwrap();
        let values: Vec<f32> = result
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        assert_eq!(values, vec![6.0, 8.0, 10.0, 12.0]);
    }

    #[test]
    fn test_matmul() {
        let backend = CpuBackend::new();

        let a = backend
            .from_slice_f32(&[1.0, 2.0, 3.0, 4.0], &[2, 2])
            .unwrap();
        let b = backend
            .from_slice_f32(&[5.0, 6.0, 7.0, 8.0], &[2, 2])
            .unwrap();

        let result = backend.matmul(&a, &b).unwrap();
        assert_eq!(result.shape, vec![2, 2]);
    }

    #[test]
    fn test_activations() {
        let backend = CpuBackend::new();

        let a = backend
            .from_slice_f32(&[-1.0, 0.0, 1.0, 2.0], &[4])
            .unwrap();

        let relu = backend.relu(&a).unwrap();
        let relu_data = backend.copy_to_host(&relu).unwrap();
        let relu_values: Vec<f32> = relu_data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        assert_eq!(relu_values, vec![0.0, 0.0, 1.0, 2.0]);
    }

    // ── BLAS integration tests ───────────────────────────────────────────

    #[test]
    fn test_blas_matmul_large() {
        // Matrices ≥32x32 should use the BLAS path
        let backend = CpuBackend::new();
        let n = 32;
        let a_data: Vec<f32> = (0..n * n).map(|i| (i % 10) as f32 * 0.1).collect();
        let b_data: Vec<f32> = (0..n * n).map(|i| ((i + 3) % 10) as f32 * 0.1).collect();
        let a = backend.from_slice_f32(&a_data, &[n, n]).unwrap();
        let b = backend.from_slice_f32(&b_data, &[n, n]).unwrap();
        let c = backend.matmul(&a, &b).unwrap();
        assert_eq!(c.shape, vec![n, n]);
    }

    #[test]
    fn test_blas_solve() {
        let backend = CpuBackend::new();
        // A = [[2, 1], [1, 3]], b = [5, 7]
        // Solution: x = [8/5, 9/5] = [1.6, 1.8]
        let a = backend
            .from_slice_f32(&[2.0, 1.0, 1.0, 3.0], &[2, 2])
            .unwrap();
        let b = backend.from_slice_f32(&[5.0, 7.0], &[2]).unwrap();
        let x = backend.solve(&a, &b).unwrap();
        let x_data = backend.copy_to_host(&x).unwrap();
        let x_vals: Vec<f32> = x_data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert!((x_vals[0] - 1.6).abs() < 1e-4);
        assert!((x_vals[1] - 1.8).abs() < 1e-4);
    }

    #[test]
    fn test_blas_det() {
        let backend = CpuBackend::new();
        // det([[1, 2], [3, 4]]) = 1*4 - 2*3 = -2
        let a = backend
            .from_slice_f32(&[1.0, 2.0, 3.0, 4.0], &[2, 2])
            .unwrap();
        let d = backend.det(&a).unwrap();
        assert!((d - (-2.0)).abs() < 1e-6);
    }

    #[test]
    fn test_blas_inv() {
        let backend = CpuBackend::new();
        // inv([[1, 2], [3, 4]]) = [[-2, 1], [1.5, -0.5]]
        let a = backend
            .from_slice_f32(&[1.0, 2.0, 3.0, 4.0], &[2, 2])
            .unwrap();
        let a_inv = backend.inv(&a).unwrap();
        let inv_data = backend.copy_to_host(&a_inv).unwrap();
        let inv_vals: Vec<f32> = inv_data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert!((inv_vals[0] - (-2.0)).abs() < 1e-4);
        assert!((inv_vals[1] - 1.0).abs() < 1e-4);
        assert!((inv_vals[2] - 1.5).abs() < 1e-4);
        assert!((inv_vals[3] - (-0.5)).abs() < 1e-4);
    }

    #[test]
    fn test_blas_cholesky() {
        let backend = CpuBackend::new();
        // SPD matrix: [[4, 2], [2, 3]]
        let a = backend
            .from_slice_f32(&[4.0, 2.0, 2.0, 3.0], &[2, 2])
            .unwrap();
        let l = backend.cholesky(&a).unwrap();
        assert_eq!(l.shape, vec![2, 2]);
        let l_data = backend.copy_to_host(&l).unwrap();
        let l_vals: Vec<f32> = l_data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        // L[0,0] = sqrt(4) = 2.0
        assert!((l_vals[0] - 2.0).abs() < 1e-4);
    }
}
