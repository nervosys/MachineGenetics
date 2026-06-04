//! Tensor runtime for the RMIL VM.
//!
//! Provides CPU-based tensor operations that integrate with the VM's [`Val::Tensor`]
//! type. Operations include element-wise arithmetic, reductions, shape manipulation,
//! and basic linear algebra.
//!
//! # Design
//!
//! Tensors are stored as contiguous `Vec<u8>` buffers with a [`Dtype`] and shape.
//! All compute happens on the CPU via Rust iterators — no external BLAS dependency.
//! This module can be used as a fallback when no GPU backend is available.
//!
//! # Examples
//!
//! ```
//! use rmi::lang::tensor_rt::{TensorOps, TensorData};
//! use rmi::lang::ty::Dtype;
//!
//! let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
//! let b = TensorData::from_f32(&[5.0, 6.0, 7.0, 8.0], &[2, 2]);
//! let c = TensorOps::add(&a, &b).unwrap();
//! assert_eq!(c.to_f32(), vec![6.0, 8.0, 10.0, 12.0]);
//! ```

use crate::lang::expr::Val;
use crate::lang::ty::Dtype;

// ── TensorData ───────────────────────────────────────────────────────────────

/// A CPU tensor with typed buffer, dtype, and shape.
#[derive(Debug, Clone, PartialEq)]
pub struct TensorData {
    /// Element data type.
    pub dtype: Dtype,
    /// Shape dimensions.
    pub shape: Vec<usize>,
    /// Raw byte buffer (little-endian, element-packed).
    pub data: Vec<u8>,
}

impl TensorData {
    /// Create a tensor from f32 data.
    pub fn from_f32(values: &[f32], shape: &[usize]) -> Self {
        assert_eq!(values.len(), shape.iter().product::<usize>());
        let data: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        Self {
            dtype: Dtype::F32,
            shape: shape.to_vec(),
            data,
        }
    }

    /// Create a tensor from f64 data.
    pub fn from_f64(values: &[f64], shape: &[usize]) -> Self {
        assert_eq!(values.len(), shape.iter().product::<usize>());
        let data: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        Self {
            dtype: Dtype::F64,
            shape: shape.to_vec(),
            data,
        }
    }

    /// Create a tensor from i64 data.
    pub fn from_i64(values: &[i64], shape: &[usize]) -> Self {
        assert_eq!(values.len(), shape.iter().product::<usize>());
        let data: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        Self {
            dtype: Dtype::I64,
            shape: shape.to_vec(),
            data,
        }
    }

    /// Create an f32 tensor filled with zeros.
    pub fn zeros_f32(shape: &[usize]) -> Self {
        let numel: usize = shape.iter().product();
        Self::from_f32(&vec![0.0f32; numel], shape)
    }

    /// Create an f32 tensor filled with ones.
    pub fn ones_f32(shape: &[usize]) -> Self {
        let numel: usize = shape.iter().product();
        Self::from_f32(&vec![1.0f32; numel], shape)
    }

    /// Create a scalar f32 tensor.
    pub fn scalar_f32(value: f32) -> Self {
        Self::from_f32(&[value], &[1])
    }

    /// Number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Number of dimensions.
    pub fn ndim(&self) -> usize {
        self.shape.len()
    }

    /// Size in bytes.
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }

    /// Element size in bytes.
    pub fn element_size(&self) -> usize {
        match self.dtype {
            Dtype::Bool | Dtype::I8 | Dtype::U8 => 1,
            Dtype::F16 | Dtype::BF16 | Dtype::I16 | Dtype::U16 => 2,
            Dtype::F32 | Dtype::I32 | Dtype::U32 => 4,
            Dtype::F64 | Dtype::I64 | Dtype::U64 => 8,
        }
    }

    /// Read data as f32 slice.
    pub fn to_f32(&self) -> Vec<f32> {
        assert_eq!(self.dtype, Dtype::F32);
        self.data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    /// Read data as f64 slice.
    pub fn to_f64(&self) -> Vec<f64> {
        assert_eq!(self.dtype, Dtype::F64);
        self.data
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
            .collect()
    }

    /// Read data as i64 slice.
    pub fn to_i64(&self) -> Vec<i64> {
        assert_eq!(self.dtype, Dtype::I64);
        self.data
            .chunks_exact(8)
            .map(|c| i64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
            .collect()
    }

    /// Convert to a Val::Tensor.
    pub fn to_val(&self) -> Val {
        Val::Tensor {
            dtype: self.dtype,
            shape: self.shape.clone(),
            data: self.data.clone(),
        }
    }

    /// Create from a Val::Tensor.
    pub fn from_val(val: &Val) -> Option<Self> {
        match val {
            Val::Tensor { dtype, shape, data } => Some(Self {
                dtype: *dtype,
                shape: shape.clone(),
                data: data.clone(),
            }),
            _ => None,
        }
    }
}

// ── TensorOps ────────────────────────────────────────────────────────────────

/// Tensor operation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TensorError {
    /// Shape mismatch for binary op.
    ShapeMismatch {
        /// Shape of first operand.
        a: Vec<usize>,
        /// Shape of second operand.
        b: Vec<usize>,
    },
    /// Dtype mismatch for binary op.
    DtypeMismatch {
        /// Dtype of first operand.
        a: Dtype,
        /// Dtype of second operand.
        b: Dtype,
    },
    /// Invalid shape for operation.
    InvalidShape(String),
    /// Unsupported dtype.
    UnsupportedDtype(Dtype),
}

impl std::fmt::Display for TensorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ShapeMismatch { a, b } => write!(f, "shape mismatch: {a:?} vs {b:?}"),
            Self::DtypeMismatch { a, b } => write!(f, "dtype mismatch: {a:?} vs {b:?}"),
            Self::InvalidShape(s) => write!(f, "invalid shape: {s}"),
            Self::UnsupportedDtype(d) => write!(f, "unsupported dtype: {d:?}"),
        }
    }
}

impl std::error::Error for TensorError {}

/// CPU tensor operations.
pub struct TensorOps;

impl TensorOps {
    /// Check that two tensors have matching shape and dtype.
    fn check_binary(a: &TensorData, b: &TensorData) -> Result<(), TensorError> {
        if a.dtype != b.dtype {
            return Err(TensorError::DtypeMismatch {
                a: a.dtype,
                b: b.dtype,
            });
        }
        if a.shape != b.shape {
            return Err(TensorError::ShapeMismatch {
                a: a.shape.clone(),
                b: b.shape.clone(),
            });
        }
        Ok(())
    }

    /// Element-wise binary operation on f32 tensors.
    fn binary_f32(
        a: &TensorData,
        b: &TensorData,
        op: impl Fn(f32, f32) -> f32,
    ) -> Result<TensorData, TensorError> {
        Self::check_binary(a, b)?;
        let va = a.to_f32();
        let vb = b.to_f32();
        let result: Vec<f32> = va.iter().zip(vb.iter()).map(|(x, y)| op(*x, *y)).collect();
        Ok(TensorData::from_f32(&result, &a.shape))
    }

    /// Element-wise unary operation on f32 tensors.
    fn unary_f32(a: &TensorData, op: impl Fn(f32) -> f32) -> Result<TensorData, TensorError> {
        if a.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }
        let va = a.to_f32();
        let result: Vec<f32> = va.iter().map(|x| op(*x)).collect();
        Ok(TensorData::from_f32(&result, &a.shape))
    }

    // ── Arithmetic ───────────────────────────────────────────────────────

    /// Element-wise addition.
    pub fn add(a: &TensorData, b: &TensorData) -> Result<TensorData, TensorError> {
        Self::binary_f32(a, b, |x, y| x + y)
    }

    /// Element-wise subtraction.
    pub fn sub(a: &TensorData, b: &TensorData) -> Result<TensorData, TensorError> {
        Self::binary_f32(a, b, |x, y| x - y)
    }

    /// Element-wise multiplication.
    pub fn mul(a: &TensorData, b: &TensorData) -> Result<TensorData, TensorError> {
        Self::binary_f32(a, b, |x, y| x * y)
    }

    /// Element-wise division.
    pub fn div(a: &TensorData, b: &TensorData) -> Result<TensorData, TensorError> {
        Self::binary_f32(a, b, |x, y| x / y)
    }

    // ── Unary ops ────────────────────────────────────────────────────────

    /// Element-wise negation.
    pub fn neg(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| -x)
    }

    /// Element-wise absolute value.
    pub fn abs(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| x.abs())
    }

    /// Element-wise exp.
    pub fn exp(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| x.exp())
    }

    /// Element-wise log.
    pub fn log(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| x.ln())
    }

    /// Element-wise sqrt.
    pub fn sqrt(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| x.sqrt())
    }

    /// Element-wise sin.
    pub fn sin(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| x.sin())
    }

    /// Element-wise cos.
    pub fn cos(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| x.cos())
    }

    // ── Activations ──────────────────────────────────────────────────────

    /// Element-wise ReLU.
    pub fn relu(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| if x > 0.0 { x } else { 0.0 })
    }

    /// Element-wise sigmoid.
    pub fn sigmoid(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| 1.0 / (1.0 + (-x).exp()))
    }

    /// Element-wise tanh.
    pub fn tanh(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| x.tanh())
    }

    /// Element-wise GELU (approximate).
    pub fn gelu(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| {
            0.5 * x * (1.0 + (0.797_884_6 * (x + 0.044715 * x * x * x)).tanh())
        })
    }

    /// Element-wise SiLU (Swish).
    pub fn silu(a: &TensorData) -> Result<TensorData, TensorError> {
        Self::unary_f32(a, |x| x * (1.0 / (1.0 + (-x).exp())))
    }

    // ── Reductions ───────────────────────────────────────────────────────

    /// Sum all elements.
    pub fn sum(a: &TensorData) -> Result<TensorData, TensorError> {
        if a.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }
        let total: f32 = a.to_f32().iter().sum();
        Ok(TensorData::scalar_f32(total))
    }

    /// Mean of all elements.
    pub fn mean(a: &TensorData) -> Result<TensorData, TensorError> {
        if a.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }
        let vals = a.to_f32();
        let total: f32 = vals.iter().sum();
        Ok(TensorData::scalar_f32(total / vals.len() as f32))
    }

    /// Max of all elements.
    pub fn max(a: &TensorData) -> Result<TensorData, TensorError> {
        if a.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }
        let vals = a.to_f32();
        let m = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        Ok(TensorData::scalar_f32(m))
    }

    /// Min of all elements.
    pub fn min(a: &TensorData) -> Result<TensorData, TensorError> {
        if a.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }
        let vals = a.to_f32();
        let m = vals.iter().cloned().fold(f32::INFINITY, f32::min);
        Ok(TensorData::scalar_f32(m))
    }

    // ── Shape manipulation ───────────────────────────────────────────────

    /// Reshape tensor to new shape (total elements must match).
    pub fn reshape(a: &TensorData, new_shape: &[usize]) -> Result<TensorData, TensorError> {
        let new_numel: usize = new_shape.iter().product();
        if new_numel != a.numel() {
            return Err(TensorError::InvalidShape(format!(
                "cannot reshape {:?} to {:?}",
                a.shape, new_shape
            )));
        }
        Ok(TensorData {
            dtype: a.dtype,
            shape: new_shape.to_vec(),
            data: a.data.clone(),
        })
    }

    /// Transpose a 2D tensor.
    pub fn transpose_2d(a: &TensorData) -> Result<TensorData, TensorError> {
        if a.ndim() != 2 {
            return Err(TensorError::InvalidShape(format!(
                "transpose requires 2D, got {:?}",
                a.shape
            )));
        }
        if a.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }
        let (rows, cols) = (a.shape[0], a.shape[1]);
        let vals = a.to_f32();
        let mut result = vec![0.0f32; vals.len()];
        for r in 0..rows {
            for c in 0..cols {
                result[c * rows + r] = vals[r * cols + c];
            }
        }
        Ok(TensorData::from_f32(&result, &[cols, rows]))
    }

    /// Matrix multiply two 2D tensors.
    pub fn matmul(a: &TensorData, b: &TensorData) -> Result<TensorData, TensorError> {
        if a.ndim() != 2 || b.ndim() != 2 {
            return Err(TensorError::InvalidShape(
                "matmul requires 2D tensors".into(),
            ));
        }
        if a.shape[1] != b.shape[0] {
            return Err(TensorError::ShapeMismatch {
                a: a.shape.clone(),
                b: b.shape.clone(),
            });
        }
        if a.dtype != Dtype::F32 || b.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }

        let (m, k) = (a.shape[0], a.shape[1]);
        let n = b.shape[1];
        let va = a.to_f32();
        let vb = b.to_f32();
        let mut result = vec![0.0f32; m * n];

        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0f32;
                for p in 0..k {
                    sum += va[i * k + p] * vb[p * n + j];
                }
                result[i * n + j] = sum;
            }
        }
        Ok(TensorData::from_f32(&result, &[m, n]))
    }

    /// Softmax over the last dimension.
    pub fn softmax(a: &TensorData) -> Result<TensorData, TensorError> {
        if a.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }
        let vals = a.to_f32();
        if a.ndim() == 1 || (a.ndim() == 2 && a.shape[0] == 1) {
            let max_val = vals.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exps: Vec<f32> = vals.iter().map(|x| (x - max_val).exp()).collect();
            let sum: f32 = exps.iter().sum();
            let result: Vec<f32> = exps.iter().map(|x| x / sum).collect();
            Ok(TensorData::from_f32(&result, &a.shape))
        } else if a.ndim() == 2 {
            let (rows, cols) = (a.shape[0], a.shape[1]);
            let mut result = vec![0.0f32; rows * cols];
            for r in 0..rows {
                let row = &vals[r * cols..(r + 1) * cols];
                let max_val = row.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let exps: Vec<f32> = row.iter().map(|x| (x - max_val).exp()).collect();
                let sum: f32 = exps.iter().sum();
                for c in 0..cols {
                    result[r * cols + c] = exps[c] / sum;
                }
            }
            Ok(TensorData::from_f32(&result, &a.shape))
        } else {
            Err(TensorError::InvalidShape(
                "softmax supports 1D and 2D".into(),
            ))
        }
    }

    /// Layer normalization over the last dimension.
    pub fn layer_norm(a: &TensorData, eps: f32) -> Result<TensorData, TensorError> {
        if a.dtype != Dtype::F32 {
            return Err(TensorError::UnsupportedDtype(a.dtype));
        }
        let vals = a.to_f32();
        let n = vals.len() as f32;
        let mean: f32 = vals.iter().sum::<f32>() / n;
        let var: f32 = vals.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
        let std_inv = 1.0 / (var + eps).sqrt();
        let result: Vec<f32> = vals.iter().map(|x| (x - mean) * std_inv).collect();
        Ok(TensorData::from_f32(&result, &a.shape))
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: &[f32], b: &[f32], tol: f32) -> bool {
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < tol)
    }

    #[test]
    fn from_f32_roundtrip() {
        let vals = vec![1.0, 2.0, 3.0, 4.0];
        let t = TensorData::from_f32(&vals, &[2, 2]);
        assert_eq!(t.to_f32(), vals);
        assert_eq!(t.numel(), 4);
        assert_eq!(t.ndim(), 2);
    }

    #[test]
    fn from_f64_roundtrip() {
        let vals = vec![1.0, 2.5, 3.7];
        let t = TensorData::from_f64(&vals, &[3]);
        assert_eq!(t.to_f64(), vals);
    }

    #[test]
    fn from_i64_roundtrip() {
        let vals = vec![10, 20, 30];
        let t = TensorData::from_i64(&vals, &[3]);
        assert_eq!(t.to_i64(), vals);
    }

    #[test]
    fn zeros_and_ones() {
        let z = TensorData::zeros_f32(&[3, 3]);
        assert_eq!(z.numel(), 9);
        assert!(z.to_f32().iter().all(|&x| x == 0.0));

        let o = TensorData::ones_f32(&[2, 2]);
        assert!(o.to_f32().iter().all(|&x| x == 1.0));
    }

    #[test]
    fn scalar() {
        let s = TensorData::scalar_f32(3.15);
        assert_eq!(s.numel(), 1);
        assert!((s.to_f32()[0] - 3.15).abs() < 1e-6);
    }

    #[test]
    fn add() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0], &[3]);
        let b = TensorData::from_f32(&[4.0, 5.0, 6.0], &[3]);
        let c = TensorOps::add(&a, &b).unwrap();
        assert_eq!(c.to_f32(), vec![5.0, 7.0, 9.0]);
    }

    #[test]
    fn sub() {
        let a = TensorData::from_f32(&[5.0, 3.0], &[2]);
        let b = TensorData::from_f32(&[2.0, 1.0], &[2]);
        let c = TensorOps::sub(&a, &b).unwrap();
        assert_eq!(c.to_f32(), vec![3.0, 2.0]);
    }

    #[test]
    fn mul() {
        let a = TensorData::from_f32(&[2.0, 3.0], &[2]);
        let b = TensorData::from_f32(&[4.0, 5.0], &[2]);
        let c = TensorOps::mul(&a, &b).unwrap();
        assert_eq!(c.to_f32(), vec![8.0, 15.0]);
    }

    #[test]
    fn div() {
        let a = TensorData::from_f32(&[10.0, 9.0], &[2]);
        let b = TensorData::from_f32(&[2.0, 3.0], &[2]);
        let c = TensorOps::div(&a, &b).unwrap();
        assert_eq!(c.to_f32(), vec![5.0, 3.0]);
    }

    #[test]
    fn neg() {
        let a = TensorData::from_f32(&[1.0, -2.0], &[2]);
        let c = TensorOps::neg(&a).unwrap();
        assert_eq!(c.to_f32(), vec![-1.0, 2.0]);
    }

    #[test]
    fn abs() {
        let a = TensorData::from_f32(&[-3.0, 4.0, -5.0], &[3]);
        let c = TensorOps::abs(&a).unwrap();
        assert_eq!(c.to_f32(), vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn exp_log() {
        let a = TensorData::from_f32(&[0.0, 1.0], &[2]);
        let e = TensorOps::exp(&a).unwrap();
        assert!(approx_eq(&e.to_f32(), &[1.0, std::f32::consts::E], 1e-5));

        let l = TensorOps::log(&e).unwrap();
        assert!(approx_eq(&l.to_f32(), &[0.0, 1.0], 1e-5));
    }

    #[test]
    fn sqrt_test() {
        let a = TensorData::from_f32(&[4.0, 9.0, 16.0], &[3]);
        let c = TensorOps::sqrt(&a).unwrap();
        assert!(approx_eq(&c.to_f32(), &[2.0, 3.0, 4.0], 1e-5));
    }

    #[test]
    fn sin_cos() {
        let a = TensorData::from_f32(&[0.0, std::f32::consts::FRAC_PI_2], &[2]);
        let s = TensorOps::sin(&a).unwrap();
        assert!(approx_eq(&s.to_f32(), &[0.0, 1.0], 1e-5));

        let c = TensorOps::cos(&a).unwrap();
        assert!(approx_eq(&c.to_f32(), &[1.0, 0.0], 1e-5));
    }

    #[test]
    fn relu_test() {
        let a = TensorData::from_f32(&[-2.0, -1.0, 0.0, 1.0, 2.0], &[5]);
        let c = TensorOps::relu(&a).unwrap();
        assert_eq!(c.to_f32(), vec![0.0, 0.0, 0.0, 1.0, 2.0]);
    }

    #[test]
    fn sigmoid_test() {
        let a = TensorData::from_f32(&[0.0], &[1]);
        let c = TensorOps::sigmoid(&a).unwrap();
        assert!((c.to_f32()[0] - 0.5).abs() < 1e-5);
    }

    #[test]
    fn tanh_test() {
        let a = TensorData::from_f32(&[0.0], &[1]);
        let c = TensorOps::tanh(&a).unwrap();
        assert!((c.to_f32()[0]).abs() < 1e-5);
    }

    #[test]
    fn gelu_test() {
        let a = TensorData::from_f32(&[0.0, 1.0], &[2]);
        let c = TensorOps::gelu(&a).unwrap();
        assert!((c.to_f32()[0]).abs() < 1e-5);
        assert!(c.to_f32()[1] > 0.5); // gelu(1) ≈ 0.841
    }

    #[test]
    fn silu_test() {
        let a = TensorData::from_f32(&[0.0], &[1]);
        let c = TensorOps::silu(&a).unwrap();
        assert!((c.to_f32()[0]).abs() < 1e-5);
    }

    #[test]
    fn sum_test() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
        let s = TensorOps::sum(&a).unwrap();
        assert!((s.to_f32()[0] - 10.0).abs() < 1e-5);
    }

    #[test]
    fn mean_test() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0], &[4]);
        let m = TensorOps::mean(&a).unwrap();
        assert!((m.to_f32()[0] - 2.5).abs() < 1e-5);
    }

    #[test]
    fn max_min_test() {
        let a = TensorData::from_f32(&[3.0, 1.0, 4.0, 1.0, 5.0], &[5]);
        let mx = TensorOps::max(&a).unwrap();
        assert!((mx.to_f32()[0] - 5.0).abs() < 1e-5);

        let mn = TensorOps::min(&a).unwrap();
        assert!((mn.to_f32()[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn reshape_test() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let b = TensorOps::reshape(&a, &[3, 2]).unwrap();
        assert_eq!(b.shape, vec![3, 2]);
        assert_eq!(b.to_f32(), a.to_f32()); // same data

        // Invalid reshape
        assert!(TensorOps::reshape(&a, &[4, 4]).is_err());
    }

    #[test]
    fn transpose_test() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let t = TensorOps::transpose_2d(&a).unwrap();
        assert_eq!(t.shape, vec![3, 2]);
        assert_eq!(t.to_f32(), vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn matmul_test() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
        let b = TensorData::from_f32(&[5.0, 6.0, 7.0, 8.0], &[2, 2]);
        let c = TensorOps::matmul(&a, &b).unwrap();
        // [1*5+2*7, 1*6+2*8, 3*5+4*7, 3*6+4*8] = [19, 22, 43, 50]
        assert_eq!(c.to_f32(), vec![19.0, 22.0, 43.0, 50.0]);
    }

    #[test]
    fn matmul_non_square() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[2, 3]);
        let b = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[3, 2]);
        let c = TensorOps::matmul(&a, &b).unwrap();
        assert_eq!(c.shape, vec![2, 2]);
        // [1*1+2*3+3*5, 1*2+2*4+3*6, 4*1+5*3+6*5, 4*2+5*4+6*6]
        assert_eq!(c.to_f32(), vec![22.0, 28.0, 49.0, 64.0]);
    }

    #[test]
    fn softmax_test() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0], &[3]);
        let s = TensorOps::softmax(&a).unwrap();
        let vals = s.to_f32();
        let sum: f32 = vals.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5);
        assert!(vals[2] > vals[1]);
        assert!(vals[1] > vals[0]);
    }

    #[test]
    fn softmax_2d() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
        let s = TensorOps::softmax(&a).unwrap();
        let vals = s.to_f32();
        // Each row should sum to 1
        assert!((vals[0] + vals[1] - 1.0).abs() < 1e-5);
        assert!((vals[2] + vals[3] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn layer_norm_test() {
        let a = TensorData::from_f32(&[1.0, 2.0, 3.0, 4.0], &[4]);
        let n = TensorOps::layer_norm(&a, 1e-5).unwrap();
        let vals = n.to_f32();
        // Should be approximately zero mean, unit variance
        let mean: f32 = vals.iter().sum::<f32>() / vals.len() as f32;
        assert!(mean.abs() < 1e-5);
    }

    #[test]
    fn shape_mismatch_error() {
        let a = TensorData::from_f32(&[1.0, 2.0], &[2]);
        let b = TensorData::from_f32(&[1.0, 2.0, 3.0], &[3]);
        assert!(TensorOps::add(&a, &b).is_err());
    }

    #[test]
    fn dtype_mismatch_error() {
        let a = TensorData::from_f32(&[1.0], &[1]);
        let b = TensorData::from_f64(&[1.0], &[1]);
        // Will fail due to dtype mismatch
        let result = TensorOps::add(
            &a,
            &TensorData {
                dtype: Dtype::F64,
                shape: vec![1],
                data: b.data.clone(),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn val_roundtrip() {
        let t = TensorData::from_f32(&[1.0, 2.0, 3.0], &[3]);
        let v = t.to_val();
        let t2 = TensorData::from_val(&v).unwrap();
        assert_eq!(t, t2);
    }

    #[test]
    fn from_val_non_tensor() {
        assert!(TensorData::from_val(&Val::I64(42)).is_none());
    }

    #[test]
    fn byte_size() {
        let t = TensorData::from_f32(&[1.0, 2.0, 3.0], &[3]);
        assert_eq!(t.byte_size(), 12);
        assert_eq!(t.element_size(), 4);
    }

    #[test]
    fn error_display() {
        let e = TensorError::ShapeMismatch {
            a: vec![2, 3],
            b: vec![3, 2],
        };
        let s = format!("{e}");
        assert!(s.contains("shape mismatch"));
    }
}
