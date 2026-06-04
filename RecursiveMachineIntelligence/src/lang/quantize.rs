//! Quantization passes — dtype conversion for RMIL tensor data.
//!
//! Provides:
//! - **F32 → F16** (half-precision) with round-to-nearest-even
//! - **F32 → BF16** (brain float) truncation
//! - **F32 → I8** (symmetric per-tensor quantization with scale)
//! - **F32 → I16** (symmetric quantization)
//! - **Batch quantization** of multiple tensors
//! - **Dequantization** (I8/I16 → F32 with scale)
//! - **Expr-level** quantization pass (rewrites tensor literals)
//!
//! # Examples
//!
//! ```
//! use rmi::lang::quantize::{quantize_f32_to_i8, dequantize_i8_to_f32};
//!
//! let data = vec![1.0_f32, -0.5, 2.0, 0.0];
//! let (quantized, scale) = quantize_f32_to_i8(&data);
//! let restored = dequantize_i8_to_f32(&quantized, scale);
//! for (orig, rest) in data.iter().zip(restored.iter()) {
//!     assert!((orig - rest).abs() < 0.02);
//! }
//! ```

use crate::lang::expr::{Expr, Val};
use crate::lang::ty::Dtype;

// ── F16 conversion ───────────────────────────────────────────────────────────

/// Convert an f32 to IEEE 754 half-precision (f16) stored as u16.
///
/// Uses round-to-nearest-even.
pub fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> 31) & 1;
    let exp = ((bits >> 23) & 0xFF) as i32;
    let frac = bits & 0x007F_FFFF;

    if exp == 0xFF {
        // Inf or NaN
        if frac == 0 {
            // Infinity
            return ((sign << 15) | 0x7C00) as u16;
        } else {
            // NaN — preserve some mantissa bits
            return ((sign << 15) | 0x7C00 | (frac >> 13).max(1)) as u16;
        }
    }

    let new_exp = exp - 127 + 15;

    if new_exp >= 31 {
        // Overflow → Infinity
        return ((sign << 15) | 0x7C00) as u16;
    }

    if new_exp <= 0 {
        // Denorm or zero
        if new_exp < -10 {
            // Too small → zero
            return (sign << 15) as u16;
        }
        let frac_with_hidden = frac | 0x0080_0000;
        let shift = 1 - new_exp;
        let shifted = frac_with_hidden >> (13 + shift);
        return ((sign << 15) | shifted) as u16;
    }

    // Normal number
    let f16_frac = frac >> 13;
    let round_bit = (frac >> 12) & 1;
    let sticky = frac & 0x0FFF;

    let mut result = ((sign << 15) | ((new_exp as u32) << 10) | f16_frac) as u16;

    // Round to nearest even
    if round_bit == 1 && (sticky != 0 || (f16_frac & 1) != 0) {
        result = result.wrapping_add(1);
    }

    result
}

/// Convert an f16 (u16) back to f32.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let frac = (h & 0x03FF) as u32;

    if exp == 0x1F {
        if frac == 0 {
            return f32::from_bits((sign << 31) | 0x7F80_0000);
        } else {
            return f32::from_bits((sign << 31) | 0x7FC0_0000 | (frac << 13));
        }
    }

    if exp == 0 {
        if frac == 0 {
            return f32::from_bits(sign << 31); // signed zero
        }
        // Denormalized: convert to normalized f32
        let mut e: i32 = exp as i32;
        let mut f = frac;
        while (f & 0x0400) == 0 {
            f <<= 1;
            e -= 1;
        }
        f &= 0x03FF;
        let f32_exp = (127 - 15 + e + 1) as u32;
        return f32::from_bits((sign << 31) | (f32_exp << 23) | (f << 13));
    }

    let f32_exp = exp + 127 - 15;
    f32::from_bits((sign << 31) | (f32_exp << 23) | (frac << 13))
}

// ── BF16 conversion ──────────────────────────────────────────────────────────

/// Convert f32 to BF16 (brain float 16) stored as u16.
///
/// BF16 truncates the lower 16 bits of the mantissa.
pub fn f32_to_bf16(value: f32) -> u16 {
    let bits = value.to_bits();
    // Round to nearest even
    let round_bit = (bits >> 15) & 1;
    let sticky = bits & 0x7FFF;
    let mut truncated = bits >> 16;
    if round_bit == 1 && (sticky != 0 || (truncated & 1) != 0) {
        truncated += 1;
    }
    truncated as u16
}

/// Convert BF16 (u16) back to f32.
pub fn bf16_to_f32(b: u16) -> f32 {
    f32::from_bits((b as u32) << 16)
}

// ── I8 quantization ──────────────────────────────────────────────────────────

/// Symmetric per-tensor quantization: F32 → I8.
///
/// Returns (quantized values, scale factor).
/// The scale maps [-127, 127] to [-max_abs, max_abs].
pub fn quantize_f32_to_i8(data: &[f32]) -> (Vec<i8>, f32) {
    if data.is_empty() {
        return (Vec::new(), 1.0);
    }
    let max_abs = data.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
    if max_abs == 0.0 {
        return (vec![0i8; data.len()], 1.0);
    }
    let scale = max_abs / 127.0;
    let quantized: Vec<i8> = data
        .iter()
        .map(|v| (v / scale).round().clamp(-127.0, 127.0) as i8)
        .collect();
    (quantized, scale)
}

/// Dequantize I8 → F32 using the given scale.
pub fn dequantize_i8_to_f32(data: &[i8], scale: f32) -> Vec<f32> {
    data.iter().map(|v| (*v as f32) * scale).collect()
}

// ── I16 quantization ─────────────────────────────────────────────────────────

/// Symmetric per-tensor quantization: F32 → I16.
///
/// Returns (quantized values, scale factor).
pub fn quantize_f32_to_i16(data: &[f32]) -> (Vec<i16>, f32) {
    if data.is_empty() {
        return (Vec::new(), 1.0);
    }
    let max_abs = data.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
    if max_abs == 0.0 {
        return (vec![0i16; data.len()], 1.0);
    }
    let scale = max_abs / 32767.0;
    let quantized: Vec<i16> = data
        .iter()
        .map(|v| (v / scale).round().clamp(-32767.0, 32767.0) as i16)
        .collect();
    (quantized, scale)
}

/// Dequantize I16 → F32 using the given scale.
pub fn dequantize_i16_to_f32(data: &[i16], scale: f32) -> Vec<f32> {
    data.iter().map(|v| (*v as f32) * scale).collect()
}

// ── Batch conversion ─────────────────────────────────────────────────────────

/// Convert a slice of f32 values to f16, returning the bytes.
pub fn batch_f32_to_f16(data: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    for v in data {
        let h = f32_to_f16(*v);
        out.extend_from_slice(&h.to_le_bytes());
    }
    out
}

/// Convert f16 bytes back to f32 values.
pub fn batch_f16_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|c| {
            let h = u16::from_le_bytes([c[0], c[1]]);
            f16_to_f32(h)
        })
        .collect()
}

/// Convert a slice of f32 values to bf16 bytes.
pub fn batch_f32_to_bf16(data: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    for v in data {
        let b = f32_to_bf16(*v);
        out.extend_from_slice(&b.to_le_bytes());
    }
    out
}

/// Convert bf16 bytes back to f32 values.
pub fn batch_bf16_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|c| {
            let b = u16::from_le_bytes([c[0], c[1]]);
            bf16_to_f32(b)
        })
        .collect()
}

// ── Quantized tensor wrapper ─────────────────────────────────────────────────

/// A quantized tensor with metadata for dequantization.
#[derive(Debug, Clone)]
pub struct QuantizedTensor {
    /// Raw quantized bytes.
    pub data: Vec<u8>,
    /// Target dtype (I8, I16, F16, BF16).
    pub dtype: Dtype,
    /// Shape.
    pub shape: Vec<usize>,
    /// Scale factor (for I8/I16; 1.0 for F16/BF16).
    pub scale: f32,
    /// Original dtype (before quantization).
    pub original_dtype: Dtype,
}

impl QuantizedTensor {
    /// Dequantize back to f32 values.
    pub fn dequantize(&self) -> Vec<f32> {
        match self.dtype {
            Dtype::I8 => {
                let i8_data: Vec<i8> = self.data.iter().map(|b| *b as i8).collect();
                dequantize_i8_to_f32(&i8_data, self.scale)
            }
            Dtype::I16 => {
                let i16_data: Vec<i16> = self
                    .data
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
                dequantize_i16_to_f32(&i16_data, self.scale)
            }
            Dtype::F16 => batch_f16_to_f32(&self.data),
            Dtype::BF16 => batch_bf16_to_f32(&self.data),
            _ => Vec::new(),
        }
    }

    /// Number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Size in bytes.
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }

    /// Compression ratio vs original f32.
    pub fn compression_ratio(&self) -> f32 {
        let orig_size = self.numel() * 4; // f32 = 4 bytes
        if self.data.is_empty() {
            return 1.0;
        }
        orig_size as f32 / self.data.len() as f32
    }
}

/// Quantize f32 data to a target dtype.
pub fn quantize_tensor(data: &[f32], shape: &[usize], target_dtype: Dtype) -> QuantizedTensor {
    match target_dtype {
        Dtype::I8 => {
            let (quantized, scale) = quantize_f32_to_i8(data);
            QuantizedTensor {
                data: quantized.iter().map(|v| *v as u8).collect(),
                dtype: Dtype::I8,
                shape: shape.to_vec(),
                scale,
                original_dtype: Dtype::F32,
            }
        }
        Dtype::I16 => {
            let (quantized, scale) = quantize_f32_to_i16(data);
            let mut bytes = Vec::with_capacity(quantized.len() * 2);
            for v in &quantized {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
            QuantizedTensor {
                data: bytes,
                dtype: Dtype::I16,
                shape: shape.to_vec(),
                scale,
                original_dtype: Dtype::F32,
            }
        }
        Dtype::F16 => QuantizedTensor {
            data: batch_f32_to_f16(data),
            dtype: Dtype::F16,
            shape: shape.to_vec(),
            scale: 1.0,
            original_dtype: Dtype::F32,
        },
        Dtype::BF16 => QuantizedTensor {
            data: batch_f32_to_bf16(data),
            dtype: Dtype::BF16,
            shape: shape.to_vec(),
            scale: 1.0,
            original_dtype: Dtype::F32,
        },
        _ => QuantizedTensor {
            data: Vec::new(),
            dtype: target_dtype,
            shape: shape.to_vec(),
            scale: 1.0,
            original_dtype: Dtype::F32,
        },
    }
}

// ── Expr-level quantization pass ─────────────────────────────────────────────

/// Quantize all tensor literals in an expression tree to the target dtype.
///
/// Only affects `Val::Tensor` nodes whose dtype is F32.
/// Returns the rewritten expression.
pub fn quantize_expr(expr: &Expr, target: Dtype) -> Expr {
    match expr {
        Expr::Lit(Val::Tensor { dtype, shape, data }) => {
            if *dtype == Dtype::F32 {
                // Interpret data as f32
                let f32_data: Vec<f32> = data
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                let qt = quantize_tensor(&f32_data, shape, target);
                Expr::Lit(Val::Tensor {
                    dtype: qt.dtype,
                    shape: qt.shape,
                    data: qt.data,
                })
            } else {
                expr.clone()
            }
        }
        Expr::App(op, args) => {
            let new_args: Vec<Expr> = args.iter().map(|a| quantize_expr(a, target)).collect();
            Expr::App(*op, new_args)
        }
        Expr::Seq(a, b) => Expr::Seq(
            Box::new(quantize_expr(a, target)),
            Box::new(quantize_expr(b, target)),
        ),
        Expr::Par(a, b) => Expr::Par(
            Box::new(quantize_expr(a, target)),
            Box::new(quantize_expr(b, target)),
        ),
        Expr::Cond { pred, yes, no } => Expr::Cond {
            pred: Box::new(quantize_expr(pred, target)),
            yes: Box::new(quantize_expr(yes, target)),
            no: Box::new(quantize_expr(no, target)),
        },
        Expr::Let { name, val, body } => Expr::Let {
            name: *name,
            val: Box::new(quantize_expr(val, target)),
            body: Box::new(quantize_expr(body, target)),
        },
        Expr::Lam { params, body } => Expr::Lam {
            params: params.clone(),
            body: Box::new(quantize_expr(body, target)),
        },
        Expr::Call(func, args) => {
            let new_args: Vec<Expr> = args.iter().map(|a| quantize_expr(a, target)).collect();
            Expr::Call(Box::new(quantize_expr(func, target)), new_args)
        }
        Expr::Block(exprs) => {
            let new_exprs: Vec<Expr> = exprs.iter().map(|e| quantize_expr(e, target)).collect();
            Expr::Block(new_exprs)
        }
        _ => expr.clone(),
    }
}

/// Compute the memory savings from quantizing an expression's tensors.
pub fn estimate_savings(expr: &Expr, target: Dtype) -> QuantizationStats {
    let mut stats = QuantizationStats::default();
    estimate_savings_inner(expr, target, &mut stats);
    stats
}

/// Quantization statistics.
#[derive(Debug, Clone, Default)]
pub struct QuantizationStats {
    /// Number of tensor literals found.
    pub tensors_found: usize,
    /// Number of tensors quantized (F32 tensors only).
    pub tensors_quantized: usize,
    /// Original total bytes (for quantizable tensors).
    pub original_bytes: usize,
    /// Quantized total bytes.
    pub quantized_bytes: usize,
}

impl QuantizationStats {
    /// Compression ratio.
    pub fn compression_ratio(&self) -> f32 {
        if self.quantized_bytes == 0 {
            return 1.0;
        }
        self.original_bytes as f32 / self.quantized_bytes as f32
    }

    /// Bytes saved.
    pub fn bytes_saved(&self) -> usize {
        self.original_bytes.saturating_sub(self.quantized_bytes)
    }
}

fn estimate_savings_inner(expr: &Expr, target: Dtype, stats: &mut QuantizationStats) {
    match expr {
        Expr::Lit(Val::Tensor { dtype, shape, data }) => {
            stats.tensors_found += 1;
            if *dtype == Dtype::F32 {
                stats.tensors_quantized += 1;
                stats.original_bytes += data.len();
                let numel: usize = shape.iter().product();
                stats.quantized_bytes += numel * target.size();
            }
        }
        Expr::App(_, args) => {
            for a in args {
                estimate_savings_inner(a, target, stats);
            }
        }
        Expr::Seq(a, b) | Expr::Par(a, b) => {
            estimate_savings_inner(a, target, stats);
            estimate_savings_inner(b, target, stats);
        }
        Expr::Cond { pred, yes, no } => {
            estimate_savings_inner(pred, target, stats);
            estimate_savings_inner(yes, target, stats);
            estimate_savings_inner(no, target, stats);
        }
        Expr::Let { val, body, .. } => {
            estimate_savings_inner(val, target, stats);
            estimate_savings_inner(body, target, stats);
        }
        Expr::Lam { body, .. } => estimate_savings_inner(body, target, stats),
        Expr::Call(func, args) => {
            estimate_savings_inner(func, target, stats);
            for a in args {
                estimate_savings_inner(a, target, stats);
            }
        }
        Expr::Block(exprs) => {
            for e in exprs {
                estimate_savings_inner(e, target, stats);
            }
        }
        _ => {}
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::op::Op;

    // ── F16 ──────────────────────────────────────────────────────────────

    #[test]
    fn f16_zero() {
        assert_eq!(f32_to_f16(0.0), 0x0000);
        assert_eq!(f16_to_f32(0x0000), 0.0);
    }

    #[test]
    fn f16_one() {
        let h = f32_to_f16(1.0);
        assert_eq!(h, 0x3C00);
        let back = f16_to_f32(h);
        assert!((back - 1.0).abs() < 1e-6);
    }

    #[test]
    fn f16_negative() {
        let h = f32_to_f16(-1.0);
        let back = f16_to_f32(h);
        assert!((back - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn f16_roundtrip() {
        let values = [0.0, 1.0, -1.0, 0.5, -0.5, 2.0, 100.0];
        for v in values {
            let h = f32_to_f16(v);
            let back = f16_to_f32(h);
            assert!(
                (back - v).abs() < 0.01 * v.abs().max(0.01),
                "f16 roundtrip failed for {v}: got {back}"
            );
        }
    }

    #[test]
    fn f16_infinity() {
        let h = f32_to_f16(f32::INFINITY);
        assert_eq!(h, 0x7C00);
        assert!(f16_to_f32(h).is_infinite());
    }

    #[test]
    fn f16_nan() {
        let h = f32_to_f16(f32::NAN);
        assert!(f16_to_f32(h).is_nan());
    }

    // ── BF16 ─────────────────────────────────────────────────────────────

    #[test]
    fn bf16_zero() {
        assert_eq!(f32_to_bf16(0.0), 0x0000);
        assert_eq!(bf16_to_f32(0x0000), 0.0);
    }

    #[test]
    fn bf16_one() {
        let b = f32_to_bf16(1.0);
        assert_eq!(bf16_to_f32(b), 1.0);
    }

    #[test]
    fn bf16_roundtrip() {
        let values = [0.0, 1.0, -1.0, 3.15, -2.0, 100.0];
        for v in values {
            let b = f32_to_bf16(v);
            let back = bf16_to_f32(b);
            assert!(
                (back - v).abs() < 0.02 * v.abs().max(0.02),
                "bf16 roundtrip failed for {v}: got {back}"
            );
        }
    }

    // ── I8 quantization ──────────────────────────────────────────────────

    #[test]
    fn i8_quantize_basic() {
        let data = vec![1.0, -1.0, 0.5, 0.0];
        let (q, scale) = quantize_f32_to_i8(&data);
        assert_eq!(q.len(), 4);
        assert!(scale > 0.0);
        assert_eq!(q[0], 127); // max positive
        assert_eq!(q[1], -127); // max negative
    }

    #[test]
    fn i8_roundtrip() {
        let data = vec![1.0_f32, -0.5, 2.0, 0.0, -1.5];
        let (q, scale) = quantize_f32_to_i8(&data);
        let restored = dequantize_i8_to_f32(&q, scale);
        for (orig, rest) in data.iter().zip(restored.iter()) {
            assert!((orig - rest).abs() < 0.02, "i8 roundtrip: {orig} → {rest}");
        }
    }

    #[test]
    fn i8_empty() {
        let (q, scale) = quantize_f32_to_i8(&[]);
        assert!(q.is_empty());
        assert_eq!(scale, 1.0);
    }

    #[test]
    fn i8_all_zeros() {
        let data = vec![0.0; 10];
        let (q, scale) = quantize_f32_to_i8(&data);
        assert!(q.iter().all(|v| *v == 0));
        assert_eq!(scale, 1.0);
    }

    // ── I16 quantization ─────────────────────────────────────────────────

    #[test]
    fn i16_quantize_basic() {
        let data = vec![1.0, -1.0, 0.0];
        let (q, scale) = quantize_f32_to_i16(&data);
        assert_eq!(q.len(), 3);
        assert!(scale > 0.0);
        assert_eq!(q[0], 32767); // max positive
        assert_eq!(q[1], -32767);
    }

    #[test]
    fn i16_roundtrip() {
        let data = vec![1.0_f32, -0.5, 3.15, 0.0];
        let (q, scale) = quantize_f32_to_i16(&data);
        let restored = dequantize_i16_to_f32(&q, scale);
        for (orig, rest) in data.iter().zip(restored.iter()) {
            assert!(
                (orig - rest).abs() < 0.001,
                "i16 roundtrip: {orig} → {rest}"
            );
        }
    }

    // ── Batch conversion ─────────────────────────────────────────────────

    #[test]
    fn batch_f16_roundtrip() {
        let data = vec![1.0, 2.0, -3.0, 0.5];
        let bytes = batch_f32_to_f16(&data);
        assert_eq!(bytes.len(), 8); // 4 floats * 2 bytes
        let restored = batch_f16_to_f32(&bytes);
        for (orig, rest) in data.iter().zip(restored.iter()) {
            assert!(
                (orig - rest).abs() < 0.01,
                "batch f16 roundtrip: {orig} → {rest}"
            );
        }
    }

    #[test]
    fn batch_bf16_roundtrip() {
        let data = vec![1.0, -1.0, 0.0, 42.0];
        let bytes = batch_f32_to_bf16(&data);
        assert_eq!(bytes.len(), 8);
        let restored = batch_bf16_to_f32(&bytes);
        for (orig, rest) in data.iter().zip(restored.iter()) {
            assert!(
                (orig - rest).abs() < 0.5,
                "batch bf16 roundtrip: {orig} → {rest}"
            );
        }
    }

    // ── QuantizedTensor ──────────────────────────────────────────────────

    #[test]
    fn quantized_tensor_i8() {
        let data = vec![1.0, -1.0, 0.5, -0.5];
        let qt = quantize_tensor(&data, &[2, 2], Dtype::I8);
        assert_eq!(qt.dtype, Dtype::I8);
        assert_eq!(qt.shape, vec![2, 2]);
        assert_eq!(qt.numel(), 4);
        assert_eq!(qt.byte_size(), 4); // 4 * 1 byte

        let restored = qt.dequantize();
        for (orig, rest) in data.iter().zip(restored.iter()) {
            assert!((orig - rest).abs() < 0.02);
        }
    }

    #[test]
    fn quantized_tensor_f16() {
        let data = vec![1.0, 2.0, 3.0];
        let qt = quantize_tensor(&data, &[3], Dtype::F16);
        assert_eq!(qt.dtype, Dtype::F16);
        assert_eq!(qt.byte_size(), 6); // 3 * 2 bytes
        assert_eq!(qt.compression_ratio(), 2.0); // 12 / 6

        let restored = qt.dequantize();
        for (orig, rest) in data.iter().zip(restored.iter()) {
            assert!((orig - rest).abs() < 0.01);
        }
    }

    #[test]
    fn quantized_tensor_bf16() {
        let data = vec![1.0, -1.0];
        let qt = quantize_tensor(&data, &[2], Dtype::BF16);
        assert_eq!(qt.dtype, Dtype::BF16);
        assert_eq!(qt.compression_ratio(), 2.0);
    }

    #[test]
    fn quantized_tensor_i16() {
        let data = vec![1.0, -1.0, 0.0, 0.5];
        let qt = quantize_tensor(&data, &[4], Dtype::I16);
        assert_eq!(qt.dtype, Dtype::I16);
        assert_eq!(qt.byte_size(), 8); // 4 * 2 bytes
        let restored = qt.dequantize();
        for (orig, rest) in data.iter().zip(restored.iter()) {
            assert!((orig - rest).abs() < 0.001);
        }
    }

    // ── Expr-level quantization ──────────────────────────────────────────

    #[test]
    fn quantize_expr_tensor_literal() {
        let f32_bytes: Vec<u8> = [1.0_f32, 2.0, 3.0, 4.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let expr = Expr::Lit(Val::Tensor {
            dtype: Dtype::F32,
            shape: vec![2, 2],
            data: f32_bytes,
        });
        let quantized = quantize_expr(&expr, Dtype::F16);
        if let Expr::Lit(Val::Tensor { dtype, shape, data }) = &quantized {
            assert_eq!(*dtype, Dtype::F16);
            assert_eq!(*shape, vec![2, 2]);
            assert_eq!(data.len(), 8); // 4 elements * 2 bytes
        } else {
            panic!("expected tensor literal");
        }
    }

    #[test]
    fn quantize_expr_non_tensor_unchanged() {
        let expr = Expr::int(42);
        let result = quantize_expr(&expr, Dtype::I8);
        assert_eq!(result, expr);
    }

    #[test]
    fn quantize_expr_nested() {
        let f32_bytes: Vec<u8> = [1.0_f32].iter().flat_map(|v| v.to_le_bytes()).collect();
        let tensor = Expr::Lit(Val::Tensor {
            dtype: Dtype::F32,
            shape: vec![1],
            data: f32_bytes,
        });
        let expr = Expr::op2(Op::ADD, tensor, Expr::int(1));
        let quantized = quantize_expr(&expr, Dtype::F16);
        if let Expr::App(_, args) = &quantized {
            if let Expr::Lit(Val::Tensor { dtype, .. }) = &args[0] {
                assert_eq!(*dtype, Dtype::F16);
            } else {
                panic!("expected tensor in first arg");
            }
        } else {
            panic!("expected App");
        }
    }

    // ── Savings estimation ───────────────────────────────────────────────

    #[test]
    fn estimate_savings_no_tensors() {
        let expr = Expr::int(42);
        let stats = estimate_savings(&expr, Dtype::I8);
        assert_eq!(stats.tensors_found, 0);
        assert_eq!(stats.tensors_quantized, 0);
    }

    #[test]
    fn estimate_savings_with_tensor() {
        let f32_bytes: Vec<u8> = vec![1.0_f32; 100]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let expr = Expr::Lit(Val::Tensor {
            dtype: Dtype::F32,
            shape: vec![100],
            data: f32_bytes,
        });
        let stats = estimate_savings(&expr, Dtype::I8);
        assert_eq!(stats.tensors_found, 1);
        assert_eq!(stats.tensors_quantized, 1);
        assert_eq!(stats.original_bytes, 400); // 100 * 4
        assert_eq!(stats.quantized_bytes, 100); // 100 * 1
        assert_eq!(stats.compression_ratio(), 4.0);
        assert_eq!(stats.bytes_saved(), 300);
    }

    #[test]
    fn f16_small_denorm() {
        // Very small number should become f16 denorm or zero
        let tiny = 1e-8_f32;
        let h = f32_to_f16(tiny);
        let back = f16_to_f32(h);
        assert!(back.abs() < 1e-4);
    }

    #[test]
    fn f16_negative_zero() {
        let h = f32_to_f16(-0.0);
        let back = f16_to_f32(h);
        assert_eq!(back, 0.0); // negative zero is zero
    }
}
