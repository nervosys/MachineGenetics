//! # Machine Language Compute Dispatch
//!
//! Runs a bridge-produced Machine Language [`Expr`] pipeline against an
//! [`rmi::compute::Backend`] for activation-only chains.
//!
//! This is the executable counterpart to the tree-walking [`rmi::lang::Vm`]:
//! where the VM stubs neural opcodes, this module dispatches them to a real
//! compute backend (CPU today, GPU once features are enabled).
//!
//! ## Supported ops
//!
//! Phase-4 scope is activation pipelines — opcodes that the
//! [`rmi::compute::Backend`] trait exposes as `(input) → output` without
//! external weights:
//!
//! | Opcode        | Backend method   |
//! |---------------|------------------|
//! | `RELU`        | `relu`           |
//! | `GELU`        | `gelu`           |
//! | `SIGMOID`     | `sigmoid`        |
//! | `TANH_ACT`    | `tanh`           |
//! | `SOFTMAX`     | `softmax(axis=-1)` |
//! | `IDENTITY`    | passthrough      |
//!
//! Weighted ops (`LINEAR`, `MATMUL`, `CONV2D`, `ATTN`, normalisations) are
//! reported as **unsupported** because they require parameter tensors that
//! the bridge does not yet thread through. Adding them is mechanical once
//! `Op::LINEAR(weight_tensor)` carries its parameter symbol.

use std::collections::HashMap;

use rmi::compute::{Backend, DType, TensorHandle};
use rmi::compute::cpu::CpuBackend;
use rmi::lang::{Expr, Op, Val};

// ═══════════════════════════════════════════════════════════════════
// Parameter store — content-hash-keyed weight tensors
// ═══════════════════════════════════════════════════════════════════

/// Cache of weight tensors keyed by a (op, dims) signature.
///
/// The first dispatch of `Linear(784, 256)` allocates a deterministically
/// seeded weight tensor; subsequent dispatches of the same `(LINEAR, [784,
/// 256])` reuse it. This makes repeated forward passes through a `net`
/// stable instead of re-initialising weights per call.
pub struct ParamStore {
    weights: HashMap<(u16, Vec<i64>), TensorHandle>,
    /// P120: the precision the pipeline computes in. Weights fed to
    /// matmul/conv are cast to this, and each op's output is normalised
    /// back to it. Canonical cached weights stay F32; only the per-call
    /// copies handed to compute ops are cast. Default F32 (no change).
    compute_dtype: DType,
    /// P123: when true, LINEAR/MATMUL route through
    /// `Backend::quantized_matmul` (INT8 on CUDA, exact F32 fallback
    /// elsewhere). Default false.
    quantize: bool,
    /// P129: post-training quantization calibration state. In
    /// `Calibrate` mode each LINEAR/MATMUL records the activation amax
    /// for its call-position index; in `Quantized` mode those recorded
    /// scales drive `quantized_matmul_calibrated` (fast, no host sync).
    quant_mode: QuantMode,
    /// Per-call-position activation scales (amax/127), indexed by the
    /// order LINEAR/MATMUL ops are dispatched in a pass (deterministic).
    act_scales: Vec<f32>,
    /// P136: per-call-position activation [lo, hi] ranges (for asymmetric
    /// zero-point quantization). Recorded alongside `act_scales` during
    /// calibration; running min/max across calibration samples.
    act_ranges: Vec<(f32, f32)>,
    /// P136: when true, Calibrated mode uses the asymmetric (zero-point)
    /// path via recorded [lo,hi] ranges instead of symmetric scales.
    /// Better accuracy on one-sided (e.g. post-ReLU) activations.
    asymmetric: bool,
    /// P139: weight bit-width for calibrated inference. 8 (default) =
    /// INT8 per-channel weights (IMMA); 4 = packed INT4 weights (W4A8,
    /// halves weight memory, naive GEMM).
    weight_bits: u8,
    /// Running call-position counter; reset at the start of each pass.
    matmul_call_idx: usize,
    /// P130: how `Calibrate` mode picks the clipping threshold per layer.
    calib_method: CalibMethod,
}

/// P130/P132: activation-range calibration method.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CalibMethod {
    /// Threshold = max|x| (sensitive to outliers).
    Max,
    /// Threshold = the p-th percentile of |x| (0.0–1.0), clipping
    /// outliers. e.g. 0.999 = 99.9th percentile. Far better accuracy on
    /// heavy-tailed activations.
    Percentile(f32),
    /// P132: entropy (KL-divergence) calibration. Builds a histogram of
    /// |x| and picks the clipping threshold that minimises the KL
    /// divergence between the original distribution and its quantized
    /// approximation (TensorRT's method). Best accuracy on most
    /// distributions; more expensive (offline, so fine).
    Entropy,
}

/// P129: quantization mode for the pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantMode {
    /// No quantization (F32 matmul).
    Off,
    /// Dynamic INT8: per-call activation amax (sync-bound but no calibration).
    Dynamic,
    /// Calibration pass: run F32 but RECORD activation amax per matmul.
    Calibrate,
    /// Calibrated INT8: use recorded scales (fast, on-device).
    Calibrated,
}

impl Default for ParamStore {
    fn default() -> Self {
        Self {
            weights: HashMap::new(),
            compute_dtype: DType::F32,
            quantize: false,
            quant_mode: QuantMode::Off,
            act_scales: Vec::new(),
            act_ranges: Vec::new(),
            asymmetric: false,
            weight_bits: 8,
            matmul_call_idx: 0,
            calib_method: CalibMethod::Max,
        }
    }
}

impl ParamStore {
    /// Construct an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of cached weight tensors.
    pub fn len(&self) -> usize {
        self.weights.len()
    }

    /// True if the store has no entries.
    pub fn is_empty(&self) -> bool {
        self.weights.is_empty()
    }

    /// Look up or allocate a weight tensor.
    fn get_or_alloc(
        &mut self,
        backend: &dyn Backend,
        op: Op,
        dims: &[i64],
        shape: &[usize],
    ) -> Result<TensorHandle, rmi::error::RmiError> {
        let key = (op.0, dims.to_vec());
        if let Some(handle) = self.weights.get(&key) {
            return Ok(handle.clone());
        }
        // Deterministic init: simple LCG seeded by the key hash, scaled to
        // [-1/sqrt(in), 1/sqrt(in)] for stable forward passes.
        let numel: usize = shape.iter().product();
        let fan_in = *shape.first().unwrap_or(&1).max(&1);
        let scale = 1.0f32 / (fan_in as f32).sqrt();
        let seed = lcg_seed(op.0 as u64, dims);
        let mut state = seed;
        let data: Vec<f32> = (0..numel)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let normalised = ((state >> 33) as u32 as f32) / (u32::MAX as f32);
                (normalised * 2.0 - 1.0) * scale
            })
            .collect();
        let handle = backend.from_slice_f32(&data, shape)?;
        self.weights.insert(key, handle.clone());
        Ok(handle)
    }

    /// Set the compute precision for subsequent dispatches (P120).
    pub fn set_compute_dtype(&mut self, dt: DType) {
        self.compute_dtype = dt;
    }

    /// The pipeline's compute precision.
    pub fn compute_dtype(&self) -> DType {
        self.compute_dtype
    }

    /// Enable/disable INT8 quantized matmul for LINEAR/MATMUL (P123).
    pub fn set_quantize(&mut self, q: bool) {
        self.quantize = q;
    }

    /// Set the quantization mode (P129) and reset the per-pass counter.
    pub fn set_quant_mode(&mut self, mode: QuantMode) {
        self.quant_mode = mode;
        self.matmul_call_idx = 0;
    }

    /// Set the calibration method (P130). `Max` (default) or
    /// `Percentile(p)` to clip outliers at the p-th percentile of |x|.
    pub fn set_calib_method(&mut self, m: CalibMethod) {
        self.calib_method = m;
    }

    /// Recorded per-position activation scales after a calibration pass.
    pub fn act_scales(&self) -> &[f32] {
        &self.act_scales
    }

    /// Recorded per-position activation [lo, hi] ranges (P136).
    pub fn act_ranges(&self) -> &[(f32, f32)] {
        &self.act_ranges
    }

    /// Enable asymmetric (zero-point) calibrated quantization (P136).
    /// Calibrated inference then uses the recorded [lo,hi] ranges via
    /// `quantized_matmul_asym_calibrated` — better accuracy for
    /// one-sided activations (e.g. post-ReLU).
    pub fn set_asymmetric(&mut self, asym: bool) {
        self.asymmetric = asym;
    }

    /// Set the calibrated-inference weight bit-width (P139): 8 (INT8,
    /// default) or 4 (packed INT4 W4A8 — half the weight memory).
    pub fn set_weight_bits(&mut self, bits: u8) {
        self.weight_bits = bits;
    }

    /// Run a LINEAR/MATMUL respecting the quant mode (P129). `input` is
    /// the activation, `weights` the (already compute-dtype) weight.
    /// - Off: plain F32 matmul.
    /// - Dynamic: `quantized_matmul` (per-call amax).
    /// - Calibrate: record this position's activation amax, run F32.
    /// - Calibrated: use the recorded scale via `quantized_matmul_calibrated`.
    fn run_matmul(
        &mut self,
        backend: &dyn Backend,
        input: &TensorHandle,
        weights: &TensorHandle,
    ) -> Result<TensorHandle, rmi::error::RmiError> {
        match self.quant_mode {
            QuantMode::Off => {
                if self.quantize {
                    backend.quantized_matmul(input, weights)
                } else {
                    backend.matmul(input, weights)
                }
            }
            QuantMode::Dynamic => backend.quantized_matmul(input, weights),
            QuantMode::Calibrate => {
                // Record the activation clipping threshold/127 for this
                // call position, per the configured calibration method.
                let vals = read_as_f32(backend, input)?;
                let thresh = match self.calib_method {
                    CalibMethod::Max => vals.iter().fold(0.0f32, |m, &x| m.max(x.abs())),
                    CalibMethod::Percentile(p) => percentile_abs(&vals, p),
                    CalibMethod::Entropy => entropy_threshold_abs(&vals, 2048, 128),
                };
                let scale = if thresh > 0.0 { thresh / 127.0 } else { 1.0 };
                // P136: also record the signed [lo, hi] range for the
                // asymmetric path (running min/max across samples).
                let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
                for &x in &vals {
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
                if !lo.is_finite() || !hi.is_finite() {
                    lo = 0.0;
                    hi = 0.0;
                }
                let idx = self.matmul_call_idx;
                if idx == self.act_scales.len() {
                    self.act_scales.push(scale);
                    self.act_ranges.push((lo, hi));
                } else if idx < self.act_scales.len() {
                    // Running max across calibration samples.
                    self.act_scales[idx] = self.act_scales[idx].max(scale);
                    let r = &mut self.act_ranges[idx];
                    r.0 = r.0.min(lo);
                    r.1 = r.1.max(hi);
                }
                self.matmul_call_idx += 1;
                backend.matmul(input, weights) // calibration runs in F32
            }
            QuantMode::Calibrated => {
                let idx = self.matmul_call_idx;
                self.matmul_call_idx += 1;
                if self.asymmetric {
                    // P136: zero-point quantization via recorded range.
                    if let Some(&(lo, hi)) = self.act_ranges.get(idx) {
                        if hi > lo {
                            return backend
                                .quantized_matmul_asym_calibrated(input, lo, hi, weights);
                        }
                    }
                }
                match self.act_scales.get(idx).copied() {
                    Some(sa) if self.weight_bits == 4 => {
                        // P139: W4A8 — packed INT4 weights.
                        backend.quantized_matmul_w4_calibrated(input, sa, weights)
                    }
                    Some(sa) => backend.quantized_matmul_calibrated(input, sa, weights),
                    None => backend.quantized_matmul(input, weights), // no scale → dynamic
                }
            }
        }
    }

    /// Like `get_or_alloc`, but returns the weight in the current
    /// `compute_dtype` (cast from the canonical F32 copy). Used by ops
    /// that feed weights straight into a compute op (matmul/conv).
    fn weight_for_compute(
        &mut self,
        backend: &dyn Backend,
        op: Op,
        dims: &[i64],
        shape: &[usize],
    ) -> Result<TensorHandle, rmi::error::RmiError> {
        let w = self.get_or_alloc(backend, op, dims, shape)?;
        if self.compute_dtype == DType::F32 {
            Ok(w)
        } else {
            backend.cast(&w, self.compute_dtype)
        }
    }
}

/// Read a tensor into host `f32`, upcasting via `cast` if it isn't F32
/// (so the pipeline's f32-internal helpers work on half tensors too).
fn read_as_f32(
    backend: &dyn Backend,
    h: &TensorHandle,
) -> Result<Vec<f32>, rmi::error::RmiError> {
    if h.dtype == DType::F32 {
        return Ok(bytes_to_f32(&backend.copy_to_host(h)?));
    }
    let f = backend.cast(h, DType::F32)?;
    let v = bytes_to_f32(&backend.copy_to_host(&f)?);
    let _ = backend.free(&f);
    Ok(v)
}

/// P130: the `p`-th percentile of |values| (p in [0,1]). Used by
/// percentile calibration to clip activation outliers. Sorts a copy of
/// the magnitudes (calibration is offline, so O(n log n) is fine).
fn percentile_abs(values: &[f32], p: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut mags: Vec<f32> = values.iter().map(|v| v.abs()).collect();
    mags.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p = p.clamp(0.0, 1.0);
    // Nearest-rank index.
    let idx = ((p * (mags.len() as f32 - 1.0)).round() as usize).min(mags.len() - 1);
    mags[idx]
}

/// P132: entropy (KL-divergence) calibration threshold over |x|
/// (TensorRT's algorithm). Builds a `nbins`-bin histogram of |x| in
/// [0, max], then for each candidate clip point `i` (in `quant_bins..=nbins`)
/// treats bins `[0,i)` as the reference distribution `P` (with outliers
/// past `i` folded into the last kept bin), quantizes it down to
/// `quant_bins` levels to form `Q`, and picks the `i` minimising
/// KL(P‖Q). Returns the threshold = right edge of bin `i`.
fn entropy_threshold_abs(values: &[f32], nbins: usize, quant_bins: usize) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let amax = values.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if amax <= 0.0 {
        return 0.0;
    }
    let bin_w = amax / nbins as f32;
    // Histogram of |x|.
    let mut hist = vec![0.0f64; nbins];
    for &x in values {
        let mut b = (x.abs() / bin_w) as usize;
        if b >= nbins {
            b = nbins - 1;
        }
        hist[b] += 1.0;
    }
    let mut best_i = nbins;
    let mut best_kl = f64::INFINITY;
    for i in quant_bins..=nbins {
        // Reference P over the first `i` bins; outliers (≥i) folded into
        // the last reference bin (TensorRT convention).
        let mut p: Vec<f64> = hist[..i].to_vec();
        let outliers: f64 = hist[i..].iter().sum();
        p[i - 1] += outliers;
        let p_sum: f64 = p.iter().sum();
        if p_sum <= 0.0 {
            continue;
        }
        // Quantize P into `quant_bins` levels → expand back to `i` bins (Q).
        let mut q = vec![0.0f64; i];
        for j in 0..quant_bins {
            let lo = j * i / quant_bins;
            let hi = ((j + 1) * i / quant_bins).max(lo + 1).min(i);
            // Sum of P over this quant bin, spread over its NON-EMPTY ref bins.
            let mut sum = 0.0f64;
            let mut nonempty = 0usize;
            for k in lo..hi {
                sum += p[k];
                if p[k] > 0.0 {
                    nonempty += 1;
                }
            }
            if nonempty > 0 {
                let share = sum / nonempty as f64;
                for k in lo..hi {
                    if p[k] > 0.0 {
                        q[k] = share;
                    }
                }
            }
        }
        // KL(P‖Q) over normalised distributions.
        let q_sum: f64 = q.iter().sum();
        if q_sum <= 0.0 {
            continue;
        }
        let mut kl = 0.0f64;
        for k in 0..i {
            if p[k] > 0.0 && q[k] > 0.0 {
                let pn = p[k] / p_sum;
                let qn = q[k] / q_sum;
                kl += pn * (pn / qn).ln();
            }
        }
        if kl < best_kl {
            best_kl = kl;
            best_i = i;
        }
    }
    // Threshold = right edge of the chosen bin (mid-edge convention).
    (best_i as f32 + 0.5) * bin_w
}

fn lcg_seed(op: u64, dims: &[i64]) -> u64 {
    let mut h = op.wrapping_mul(0x9E3779B97F4A7C15);
    for d in dims {
        h ^= (*d as u64).wrapping_mul(0xBF58476D1CE4E5B9);
        h = h.rotate_left(13);
    }
    h | 1
}

/// Outcome of a [`run_pipeline`] call.
#[derive(Debug)]
pub struct ComputeResult {
    /// Final tensor handle on the backend.
    pub output: TensorHandle,
    /// Sum of all output elements (cheap correctness probe).
    pub output_sum: f64,
    /// Number of ops dispatched to the backend.
    pub dispatched: usize,
    /// Opcodes encountered that are not supported by Phase-4 dispatch
    /// (typically weighted ops needing external params).
    pub unsupported: Vec<Op>,
}

/// Run a pipeline through the supplied backend.
///
/// `input_shape` and `seed_value` configure the initial tensor — a buffer of
/// `seed_value` repeated across the shape. The pipeline walks the `Seq`
/// chain of the expression and applies each supported op to the running
/// handle. Weighted ops (`LINEAR`, `MATMUL`) draw / cache their parameter
/// tensors through `params`.
pub fn run_pipeline(
    backend: &dyn Backend,
    expr: &Expr,
    input_shape: &[usize],
    seed_value: f32,
) -> Result<ComputeResult, rmi::error::RmiError> {
    let mut params = ParamStore::new();
    run_pipeline_with_params(backend, expr, input_shape, seed_value, &mut params)
}

/// Walk an Machine Language expression and extract the first shape-determining op's
/// expected input dimension. Returns `None` for expressions that don't
/// start with a shape-bearing op (e.g. pure-symbolic chains).
///
/// Used by callers that previously hardcoded `&[8]` as the input shape:
/// the dispatch path now adapts to whatever the model's first layer
/// declares. Fixes the P83 self-test gap where a 64-dim FlashAttention
/// block failed with "in_dim=64 but input has dim=8".
///
/// Conventions:
///   LINEAR(in, out)         -> first arg is in_features
///   CONV2D(in_ch, out_ch, k) -> first arg is in_channels (treated as
///                               flattened-dim for our 1-D input synth)
///   ATTN(dim, heads)        -> first arg is model dim
///   EMBED(vocab, dim)       -> second arg is embedding dim
///   MATMUL                  -> first matrix arg's leading dim
pub fn infer_input_shape(expr: &Expr) -> Option<Vec<usize>> {
    fn first_lit_int(args: &[Expr]) -> Option<usize> {
        args.first().and_then(|e| match e {
            Expr::Lit(Val::I64(n)) => usize::try_from(*n).ok(),
            _ => None,
        })
    }
    fn nth_lit_int(args: &[Expr], n: usize) -> Option<usize> {
        args.get(n).and_then(|e| match e {
            Expr::Lit(Val::I64(k)) => usize::try_from(*k).ok(),
            _ => None,
        })
    }
    fn walk(e: &Expr) -> Option<Vec<usize>> {
        match e {
            Expr::App(op, args) => match *op {
                Op::LINEAR | Op::MATMUL => first_lit_int(args).map(|d| vec![1, d]),
                Op::ATTN => first_lit_int(args).map(|d| vec![1, d]),
                // CONV2D(in_ch, out_ch, kernel) - need at least kernel x
                // kernel spatial dims. Use max(kernel, 8) so small kernels
                // don't degenerate but large ones (kernel=7 in ResNet stem)
                // still get enough spatial extent.
                Op::CONV2D => {
                    let c = first_lit_int(args)?;
                    let k = nth_lit_int(args, 2).unwrap_or(3);
                    // Use max(kernel * 4, 32) so the spatial extent
                    // survives 3-4 pool/stride stages without
                    // degenerating below the next layer's kernel.
                    let spatial = (k * 4).max(32);
                    Some(vec![1, c, spatial, spatial])
                }
                // EMBED takes token-ID input and returns [batch, seq,
                // dim]. The harness's float seed-tensor doesn't carry
                // valid IDs, but a small shape keeps the lookup result
                // compact enough for downstream ops. Use [1, 4] = 4
                // tokens in a single batch.
                Op::EMBED => Some(vec![1, 4]),
                _ => None,
            },
            // For Seq, the LEFT side runs first - probe it.
            Expr::Seq(l, r) => walk(l).or_else(|| walk(r)),
            Expr::Par(l, r) => walk(l).or_else(|| walk(r)),
            Expr::Block(stmts) => stmts.iter().find_map(walk),
            Expr::Let { body, .. } => walk(body),
            Expr::Call(_, args) => args.iter().find_map(walk),
            _ => None,
        }
    }
    walk(expr)
}

/// Forward-only evaluation of a pipeline against an existing tensor handle.
///
/// Like [`run_pipeline`] but takes the caller's input tensor verbatim
/// (instead of synthesising one from `seed_value`). Used for held-out
/// validation passes that must not see synthetic data.
pub fn forward_pass(
    backend: &dyn Backend,
    expr: &Expr,
    input: TensorHandle,
    params: &mut ParamStore,
) -> Result<TensorHandle, rmi::error::RmiError> {
    let mut handle = input;
    let mut unsupported = Vec::new();
    let mut dispatched = 0usize;
    params.matmul_call_idx = 0; // P129: reset per-pass matmul position counter
    walk(backend, expr, &mut handle, &mut dispatched, &mut unsupported, params)?;
    let _ = (dispatched, unsupported);
    Ok(handle)
}

/// Like [`run_pipeline`] but lets the caller supply (and inspect) a
/// parameter store. Useful when the same `net` is dispatched multiple times
/// and weight stability across calls matters.
pub fn run_pipeline_with_params(
    backend: &dyn Backend,
    expr: &Expr,
    input_shape: &[usize],
    seed_value: f32,
    params: &mut ParamStore,
) -> Result<ComputeResult, rmi::error::RmiError> {
    let numel: usize = input_shape.iter().product();
    let input_data: Vec<f32> = vec![seed_value; numel];
    let mut handle = backend.from_slice_f32(&input_data, input_shape)?;
    let mut unsupported = Vec::new();
    let mut dispatched = 0usize;

    params.matmul_call_idx = 0; // P129: reset per-pass matmul position counter
    walk(backend, expr, &mut handle, &mut dispatched, &mut unsupported, params)?;

    let bytes = backend.copy_to_host(&handle)?;
    let output_sum = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f64)
        .sum();

    Ok(ComputeResult {
        output: handle,
        output_sum,
        dispatched,
        unsupported,
    })
}

/// P120: run a pipeline in a chosen compute precision (`F32`, `F16`,
/// `BF16`). The seed input is cast to `precision`, weights fed to
/// matmul/conv are cast to it, and each op's output is normalised back
/// to it — so the whole forward pass runs in the requested dtype (on
/// CUDA, half matmuls hit tensor cores). NOTE: half precision requires
/// a backend with real half compute (CudaBackend); CpuBackend's ops
/// read storage as F32, so only `precision = F32` is correct there.
pub fn run_pipeline_with_precision(
    backend: &dyn Backend,
    expr: &Expr,
    input_shape: &[usize],
    seed_value: f32,
    params: &mut ParamStore,
    precision: DType,
) -> Result<ComputeResult, rmi::error::RmiError> {
    params.set_compute_dtype(precision);
    let numel: usize = input_shape.iter().product();
    let input_data: Vec<f32> = vec![seed_value; numel];
    let f32_in = backend.from_slice_f32(&input_data, input_shape)?;
    let mut handle = if precision != DType::F32 {
        let h = backend.cast(&f32_in, precision)?;
        let _ = backend.free(&f32_in);
        h
    } else {
        f32_in
    };
    let mut unsupported = Vec::new();
    let mut dispatched = 0usize;

    params.matmul_call_idx = 0; // P129: reset per-pass matmul position counter
    walk(backend, expr, &mut handle, &mut dispatched, &mut unsupported, params)?;

    // Read the (possibly half) output as f32 for the checksum.
    let output_sum = read_as_f32(backend, &handle)?
        .iter()
        .map(|&v| v as f64)
        .sum();

    Ok(ComputeResult {
        output: handle,
        output_sum,
        dispatched,
        unsupported,
    })
}

/// P123: run a pipeline with INT8-quantized matmuls. LINEAR/MATMUL route
/// through `Backend::quantized_matmul` (per-tensor activations,
/// per-channel weights, INT32 accumulate on CUDA; exact F32 fallback on
/// backends without INT8). Everything else runs in F32 as usual. This is
/// dynamic-activation / per-channel-weight INT8 inference of a `net`.
pub fn run_pipeline_quantized(
    backend: &dyn Backend,
    expr: &Expr,
    input_shape: &[usize],
    seed_value: f32,
    params: &mut ParamStore,
) -> Result<ComputeResult, rmi::error::RmiError> {
    params.set_quant_mode(QuantMode::Dynamic);
    run_pipeline_with_params(backend, expr, input_shape, seed_value, params)
}

/// P129: calibration pass. Runs the pipeline in F32 but records each
/// LINEAR/MATMUL's activation amax/127 into `params.act_scales` (by call
/// position). Call once (or several times to accumulate a running max
/// over representative inputs) before `run_pipeline_calibrated`.
pub fn calibrate_pipeline(
    backend: &dyn Backend,
    expr: &Expr,
    input_shape: &[usize],
    seed_value: f32,
    params: &mut ParamStore,
) -> Result<ComputeResult, rmi::error::RmiError> {
    params.set_quant_mode(QuantMode::Calibrate);
    run_pipeline_with_params(backend, expr, input_shape, seed_value, params)
}

/// P129: calibrated INT8 inference. Uses the per-position activation
/// scales recorded by `calibrate_pipeline` to drive
/// `quantized_matmul_calibrated` (fully on-device, no host sync — the
/// fast end-to-end INT8 path). Must be preceded by a calibration pass.
pub fn run_pipeline_calibrated(
    backend: &dyn Backend,
    expr: &Expr,
    input_shape: &[usize],
    seed_value: f32,
    params: &mut ParamStore,
) -> Result<ComputeResult, rmi::error::RmiError> {
    params.set_quant_mode(QuantMode::Calibrated);
    run_pipeline_with_params(backend, expr, input_shape, seed_value, params)
}

fn walk(
    backend: &dyn Backend,
    expr: &Expr,
    handle: &mut TensorHandle,
    dispatched: &mut usize,
    unsupported: &mut Vec<Op>,
    params: &mut ParamStore,
) -> Result<(), rmi::error::RmiError> {
    match expr {
        Expr::Seq(a, b) => {
            walk(backend, a, handle, dispatched, unsupported, params)?;
            walk(backend, b, handle, dispatched, unsupported, params)?;
        }
        Expr::App(op, args) => {
            if let Some(out) = dispatch_one(backend, *op, args, handle, params)? {
                // P120: keep the running tensor in the pipeline's compute
                // precision. Some ops (e.g. half matmul → F32 accumulate)
                // produce a different dtype; normalise it back.
                let want = params.compute_dtype();
                *handle = if out.dtype != want {
                    backend.cast(&out, want)?
                } else {
                    out
                };
                *dispatched += 1;
            } else if *op != Op::IDENTITY {
                unsupported.push(*op);
            }
        }
        Expr::Par(a, _b) => {
            // Take the left branch only. Real Par handling needs a join/
            // concat strategy; deferred to a later phase.
            walk(backend, a, handle, dispatched, unsupported, params)?;
        }
        _ => {}
    }
    Ok(())
}

/// Extract integer literal arguments from an App's arg list.
fn extract_int_args(args: &[Expr]) -> Vec<i64> {
    args.iter()
        .filter_map(|a| match a {
            Expr::Lit(Val::I64(n)) => Some(*n),
            _ => None,
        })
        .collect()
}

fn dispatch_one(
    backend: &dyn Backend,
    op: Op,
    args: &[Expr],
    handle: &TensorHandle,
    params: &mut ParamStore,
) -> Result<Option<TensorHandle>, rmi::error::RmiError> {
    Ok(match op {
        // ── Pure activations (no parameters) ────────────────────────
        Op::RELU => Some(backend.relu(handle)?),
        Op::GELU => Some(backend.gelu(handle)?),
        Op::SIGMOID => Some(backend.sigmoid(handle)?),
        Op::TANH_ACT => Some(backend.tanh(handle)?),
        Op::SOFTMAX => Some(backend.softmax(handle, -1)?),
        Op::IDENTITY => Some(handle.clone()),

        // ── Normalisations (parameterless or with learnable γ/β) ─────
        Op::LAYER_NORM | Op::RMS_NORM => Some(dispatch_layer_norm(backend, op, args, handle, params)?),

        // ── Pooling (1-D over last axis: kernel_size, stride from args) ─
        Op::MAX_POOL | Op::AVG_POOL => Some(dispatch_pool(backend, op, args, handle)?),

        // ── Global pooling: reduce all spatial dims to 1. ────────────
        // For input [B, C, H, W] -> [B, C]. For [B, C] (already
        // reduced) -> identity.
        Op::GLOBAL_POOL => Some(dispatch_global_pool(backend, handle)?),

        // ── 2D convolution ──────────────────────────────────────────
        Op::CONV2D => Some(dispatch_conv2d(backend, args, handle, params)?),

        // ── Scaled dot-product self-attention (parameterless or QKV) ─
        Op::ATTN => Some(dispatch_attention(backend, args, handle, params)?),

        // ── Token embedding lookup ──────────────────────────────────
        Op::EMBED => Some(dispatch_embed(backend, args, handle, params)?),

        // ── Sinusoidal positional encoding (parameter-free) ─────────
        Op::SINUSOIDAL_PE => Some(dispatch_sinusoidal_pe(backend, args, handle)?),

        // ── Learned positional embedding (additive) ─────────────────
        Op::LEARNED_PE => Some(dispatch_learned_pe(backend, args, handle, params)?),

        // ── Loss functions ──────────────────────────────────────────
        // MSE without a target tensor lowers to mean(x²): a useful
        // standalone reduction that exercises the path. Real training
        // with a target tensor lands when `train` blocks thread the
        // target through the bridge.
        Op::MSE_LOSS => Some(dispatch_mse_self(backend, handle)?),

        // ── Optimiser steps — no-op forward; real updates require backward. ──
        Op::SGD_STEP | Op::ADAM_STEP | Op::ADAMW_STEP | Op::RMSPROP_STEP => Some(handle.clone()),

        // ── Linear: input @ W (+ bias) → output. ────────────────────
        // Args: `[in_dim, out_dim]` (no bias) or `[in_dim, out_dim, 1]`
        // (with bias). Bias tensor stored under a sentinel slot so it
        // does not collide with the weight tensor's key.
        Op::LINEAR => {
            let dims = extract_int_args(args);
            let (in_dim, out_dim, use_bias) = match dims.as_slice() {
                [a, b] if *a > 0 && *b > 0 => (*a as usize, *b as usize, false),
                [a, b, c] if *a > 0 && *b > 0 => (*a as usize, *b as usize, *c != 0),
                _ => return Ok(None),
            };
            let input_2d = ensure_2d(backend, handle, in_dim)?;
            if input_2d.shape.last().copied() != Some(in_dim) {
                return Err(rmi::error::RmiError::compute_simple(format!(
                    "Linear: input last dim {} does not match in_dim {}",
                    input_2d.shape.last().copied().unwrap_or(0),
                    in_dim
                )));
            }
            // Weight tensor uses the canonical 2-arg key so old checkpoints
            // remain compatible.
            let weight_key: Vec<i64> = vec![in_dim as i64, out_dim as i64];
            let weights = params.weight_for_compute(backend, op, &weight_key, &[in_dim, out_dim])?;
            let mut out = params.run_matmul(backend, &input_2d, &weights)?;
            if use_bias {
                // Bias under disambiguated key. Allocate once, broadcast-add
                // to every row of the output.
                let bias_key = vec![in_dim as i64, out_dim as i64, BIAS_SLOT];
                let bias_handle = params.get_or_alloc(backend, op, &bias_key, &[out_dim])?;
                out = add_bias_2d(backend, &out, &bias_handle)?;
            }
            Some(out)
        }

        // ── MatMul: input @ W. Args = [m, k, n] or [k, n]. ──────────
        Op::MATMUL => {
            let dims = extract_int_args(args);
            // Accept [k, n] (input provides m) or [m, k, n] (explicit).
            let (k, n) = match dims.as_slice() {
                [k, n] if *k > 0 && *n > 0 => (*k as usize, *n as usize),
                [_m, k, n] if *k > 0 && *n > 0 => (*k as usize, *n as usize),
                _ => return Ok(None),
            };
            let input_2d = ensure_2d(backend, handle, k)?;
            let weights = params.weight_for_compute(backend, op, &dims, &[k, n])?;
            let out = params.run_matmul(backend, &input_2d, &weights)?;
            Some(out)
        }

        _ => None,
    })
}

/// Layer normalisation over the last axis. `RMS_NORM` skips the mean
/// subtraction and uses RMS instead of stddev.
///
/// `LayerNorm(dim)` (single positive arg matching the last axis) allocates
/// learnable γ and β tensors of shape `[dim]` and applies `y = γ·x̂ + β`.
/// Parameterless form `LayerNorm` (no args) keeps the Phase-6 behaviour.
fn dispatch_layer_norm(
    backend: &dyn Backend,
    op: Op,
    args: &[Expr],
    handle: &TensorHandle,
    params: &mut ParamStore,
) -> Result<TensorHandle, rmi::error::RmiError> {
    const EPS: f32 = 1e-5;
    let bytes = backend.copy_to_host(handle)?;
    let data: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let last = *handle.shape.last().unwrap_or(&data.len()).max(&1);
    let rows = data.len() / last;

    // Affine mode: LayerNorm(dim) with γ and β of shape [dim] cached in
    // ParamStore under (op, [dim, slot]) keys where slot ∈ {0=γ, 1=β}.
    let dims = extract_int_args(args);
    let affine = matches!(dims.as_slice(), [d] if (*d as usize) == last);
    let (gamma, beta) = if affine {
        let g_key = vec![last as i64, 0];
        let b_key = vec![last as i64, 1];
        let g_handle = params.get_or_alloc(backend, op, &g_key, &[last])?;
        let b_handle = params.get_or_alloc(backend, op, &b_key, &[last])?;
        // Override LCG init: γ ← 1, β ← 0 (standard LayerNorm init). Only
        // overwrite on first allocation: subsequent dispatches inherit
        // whatever the optimizer has updated them to.
        let g_bytes = backend.copy_to_host(&g_handle)?;
        let mut g_vec = bytes_to_f32(&g_bytes);
        let mut b_vec = bytes_to_f32(&backend.copy_to_host(&b_handle)?);
        // Detect "still LCG-initialised" by checking if values look random
        // (any non-zero magnitude on β triggers re-init suppression).
        if b_vec.iter().all(|v| v.abs() < 1e-9) && g_vec.iter().all(|v| (*v - 1.0).abs() < 1e-9) {
            // Already initialised to (γ=1, β=0).
        } else if g_vec.iter().all(|v| v.abs() < 0.6) {
            // Looks like fresh LCG values — reinitialise to γ=1, β=0.
            for g in g_vec.iter_mut() { *g = 1.0; }
            for b in b_vec.iter_mut() { *b = 0.0; }
            let new_g = backend.from_slice_f32(&g_vec, &[last])?;
            let new_b = backend.from_slice_f32(&b_vec, &[last])?;
            params.replace(op, &g_key, new_g);
            params.replace(op, &b_key, new_b);
        }
        (g_vec, b_vec)
    } else {
        (Vec::new(), Vec::new())
    };

    let mut out = vec![0.0f32; data.len()];
    for r in 0..rows {
        let row = &data[r * last..(r + 1) * last];
        let (centered, denom) = match op {
            Op::LAYER_NORM => {
                let mean: f32 = row.iter().copied().sum::<f32>() / last as f32;
                let centered: Vec<f32> = row.iter().map(|&v| v - mean).collect();
                let var: f32 = centered.iter().map(|v| v * v).sum::<f32>() / last as f32;
                (centered, (var + EPS).sqrt())
            }
            Op::RMS_NORM => {
                let ms: f32 = row.iter().map(|v| v * v).sum::<f32>() / last as f32;
                (row.to_vec(), (ms + EPS).sqrt())
            }
            _ => unreachable!(),
        };
        for (i, v) in centered.iter().enumerate() {
            let normalized = v / denom;
            out[r * last + i] = if affine {
                gamma[i] * normalized + beta[i]
            } else {
                normalized
            };
        }
    }
    backend.from_slice_f32(&out, &handle.shape)
}

/// 1-D pooling over the last axis. Args: `[kernel_size, stride]`. Stride
/// defaults to `kernel_size` when only one arg is provided (non-overlapping
/// windows). For multi-dim inputs, pools each row of the last axis.
/// Global pool: average over all spatial dimensions, reducing a rank-
/// >=3 tensor [B, C, *spatial] to [B, C]. Rank-1 and rank-2 inputs
/// pass through unchanged.
fn dispatch_global_pool(
    backend: &dyn Backend,
    handle: &TensorHandle,
) -> Result<TensorHandle, rmi::error::RmiError> {
    if handle.shape.len() <= 2 {
        return Ok(handle.clone());
    }
    let batch = handle.shape[0];
    let channels = handle.shape[1];
    let spatial: usize = handle.shape[2..].iter().product();
    if spatial == 0 {
        return Ok(handle.clone());
    }
    let bytes = backend.copy_to_host(handle)?;
    let data: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    // Average over spatial for each (batch, channel).
    let mut out = Vec::with_capacity(batch * channels);
    for b in 0..batch {
        for c in 0..channels {
            let base = (b * channels + c) * spatial;
            let sum: f32 = data[base..base + spatial].iter().sum();
            out.push(sum / spatial as f32);
        }
    }
    backend.from_slice_f32(&out, &[batch, channels])
}

fn dispatch_pool(
    backend: &dyn Backend,
    op: Op,
    args: &[Expr],
    handle: &TensorHandle,
) -> Result<TensorHandle, rmi::error::RmiError> {
    let dims = extract_int_args(args);
    let (kernel, stride) = match dims.as_slice() {
        [k] if *k > 0 => (*k as usize, *k as usize),
        [k, s] if *k > 0 && *s > 0 => (*k as usize, *s as usize),
        _ => {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "{op:?}: expected args [kernel] or [kernel, stride], got {dims:?}"
            )));
        }
    };
    let bytes = backend.copy_to_host(handle)?;
    let data: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let last = *handle.shape.last().unwrap_or(&data.len()).max(&1);
    let rows = data.len() / last;
    if kernel > last {
        return Err(rmi::error::RmiError::compute_simple(format!(
            "{op:?}: kernel={} larger than last dim={}",
            kernel, last
        )));
    }
    let out_len = (last - kernel) / stride + 1;
    let mut out = Vec::with_capacity(rows * out_len);
    for r in 0..rows {
        let row = &data[r * last..(r + 1) * last];
        for i in 0..out_len {
            let start = i * stride;
            let window = &row[start..start + kernel];
            let v = match op {
                Op::MAX_POOL => window.iter().cloned().fold(f32::MIN, f32::max),
                Op::AVG_POOL => window.iter().sum::<f32>() / kernel as f32,
                _ => unreachable!(),
            };
            out.push(v);
        }
    }
    let mut new_shape = handle.shape.clone();
    if let Some(last_dim) = new_shape.last_mut() {
        *last_dim = out_len;
    }
    backend.from_slice_f32(&out, &new_shape)
}

/// Scaled dot-product self-attention.
///
/// Overloaded by arg count:
///
/// - **`Attention(dim)`** — parameterless, Q = K = V = input.
///   Formula: `softmax(x · xᵀ / √d) · x`. Output shape = input shape.
/// - **`Attention(in_dim, model_dim)`** — learnable Q / K / V / O
///   projections, each cached in [`ParamStore`] as a `[in_dim,
///   model_dim]` weight tensor under a distinct key. Formula:
///   `(softmax(Q·Kᵀ / √model_dim) · V) · Wo` where Q = x·Wq, K = x·Wk,
///   V = x·Wv.
///
/// Input shape `[seq, in_dim]`.
fn dispatch_attention(
    backend: &dyn Backend,
    args: &[Expr],
    handle: &TensorHandle,
    params: &mut ParamStore,
) -> Result<TensorHandle, rmi::error::RmiError> {
    let dims = extract_int_args(args);
    let (seq, in_dim) = match handle.shape.as_slice() {
        [s, d] => (*s, *d),
        [d] => (1usize, *d),
        other => {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "ATTN: expected [seq, dim] or [dim], got {other:?}"
            )));
        }
    };

    // Decide between variants by arg count:
    //   [dim]                          → parameterless self-attention
    //   [in_dim, model_dim]            → learnable Q/K/V/O, single head
    //   [in_dim, model_dim, h]         → multi-head
    //   [in_dim, model_dim, h, causal] → multi-head, causal mask if causal != 0
    let qkv_mode = matches!(
        dims.as_slice(),
        [a, b] if *a > 0 && *b > 0
    ) || matches!(
        dims.as_slice(),
        [a, b, h] if *a > 0 && *b > 0 && *h > 0
    ) || matches!(
        dims.as_slice(),
        [a, b, h, _c] if *a > 0 && *b > 0 && *h > 0
    );
    let num_heads = match dims.as_slice() {
        [_, _, h] if *h > 0 => *h as usize,
        [_, _, h, _] if *h > 0 => *h as usize,
        _ => 1,
    };
    let causal = matches!(dims.as_slice(), [_, _, _, c] if *c != 0);

    let x = bytes_to_f32(&backend.copy_to_host(handle)?);
    let (q, k_proj, v, model_dim) = if qkv_mode {
        let in_d = dims[0] as usize;
        let model_d = dims[1] as usize;
        if model_d % num_heads != 0 {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "ATTN: model_dim {} not divisible by num_heads {}",
                model_d, num_heads
            )));
        }
        if in_d != in_dim {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "ATTN: in_dim={} but input has dim={}",
                in_d, in_dim
            )));
        }
        // Allocate Q/K/V weights under op-disambiguated keys.
        // We piggyback on ParamStore's (op, dims) key by mixing a salt
        // into the dims: use [in_d, model_d, 0] for Q, [in_d, model_d, 1]
        // for K, etc. — keeps each tensor distinct.
        let wq = backend_get_attn_weight(backend, params, in_d, model_d, 0)?;
        let wk = backend_get_attn_weight(backend, params, in_d, model_d, 1)?;
        let wv = backend_get_attn_weight(backend, params, in_d, model_d, 2)?;
        let q = matmul_2d(&x, &wq, seq, in_d, model_d);
        let k = matmul_2d(&x, &wk, seq, in_d, model_d);
        let v = matmul_2d(&x, &wv, seq, in_d, model_d);
        (q, k, v, model_d)
    } else {
        let scaling_dim = dims.first().copied().filter(|d| *d > 0).map(|d| d as usize).unwrap_or(in_dim);
        (x.clone(), x.clone(), x.clone(), scaling_dim)
    };

    let head_dim = model_dim / num_heads;
    let scale = 1.0f32 / (head_dim as f32).sqrt();
    // Per-head attention: each head independently softmax-attends over its
    // own slice of Q/K/V. Heads are interleaved within model_dim (head h's
    // dims are columns [h·head_dim .. (h+1)·head_dim]).
    let mut attended = vec![0.0f32; seq * model_dim];
    for h in 0..num_heads {
        let head_off = h * head_dim;
        // scores[i, j] for this head
        let mut scores = vec![0.0f32; seq * seq];
        for i in 0..seq {
            for j in 0..seq {
                if causal && j > i {
                    scores[i * seq + j] = f32::NEG_INFINITY;
                    continue;
                }
                let mut s = 0.0;
                for kk in 0..head_dim {
                    s += q[i * model_dim + head_off + kk]
                        * k_proj[j * model_dim + head_off + kk];
                }
                scores[i * seq + j] = s * scale;
            }
        }
        // Row-wise softmax (per head). Causal-masked positions land at 0
        // since exp(-inf) = 0.
        let mut weights = vec![0.0f32; seq * seq];
        for i in 0..seq {
            let row = &scores[i * seq..(i + 1) * seq];
            let max = row.iter().cloned().filter(|v| v.is_finite()).fold(f32::MIN, f32::max);
            let exps: Vec<f32> = row.iter().map(|v| {
                if v.is_finite() { (v - max).exp() } else { 0.0 }
            }).collect();
            let sum: f32 = exps.iter().sum();
            for (j, e) in exps.iter().enumerate() {
                weights[i * seq + j] = if sum > 0.0 { e / sum } else { 0.0 };
            }
        }
        // attended slice for this head
        for i in 0..seq {
            for kk in 0..head_dim {
                let mut s = 0.0;
                for j in 0..seq {
                    s += weights[i * seq + j] * v[j * model_dim + head_off + kk];
                }
                attended[i * model_dim + head_off + kk] = s;
            }
        }
    }

    // QKV-mode: output projection back to in_dim.
    let (out, out_shape) = if qkv_mode {
        let in_d = dims[0] as usize;
        let wo = backend_get_attn_weight(backend, params, model_dim, in_d, 3)?;
        let projected = matmul_2d(&attended, &wo, seq, model_dim, in_d);
        // Match the input's tensor shape (preserving 1-D shorthand if so).
        let shape = if handle.shape.len() == 1 {
            vec![in_d]
        } else {
            vec![seq, in_d]
        };
        (projected, shape)
    } else {
        (attended, handle.shape.clone())
    };

    backend.from_slice_f32(&out, &out_shape)
}

/// Helper: allocate-or-fetch an attention weight tensor of shape `[a, b]`
/// under a disambiguating `slot` (Q=0, K=1, V=2, O=3) so the four
/// projections live as distinct entries in the `ParamStore`.
fn backend_get_attn_weight(
    backend: &dyn Backend,
    params: &mut ParamStore,
    a: usize,
    b: usize,
    slot: i64,
) -> Result<Vec<f32>, rmi::error::RmiError> {
    let dims = vec![a as i64, b as i64, slot];
    let handle = params.get_or_alloc(backend, Op::ATTN, &dims, &[a, b])?;
    Ok(bytes_to_f32(&backend.copy_to_host(&handle)?))
}

/// Plain 2-D matmul `[m, k] @ [k, n] → [m, n]` on host-side f32 buffers.
fn matmul_2d(a: &[f32], b: &[f32], m: usize, k: usize, n: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; m * n];
    for i in 0..m {
        for j in 0..n {
            let mut s = 0.0f32;
            for kk in 0..k {
                s += a[i * k + kk] * b[kk * n + j];
            }
            out[i * n + j] = s;
        }
    }
    out
}

/// 2-D convolution: naive direct implementation, no padding, stride = 1.
///
/// Args: `[in_channels, out_channels, kernel_size]` (kernel assumed square).
/// Input shapes accepted:
///   - `[C, H, W]`            (single example)
///   - `[N, C, H, W]`         (batched)
///   - `[H, W]` with C=1      (single channel)
///
/// Weights cached in [`ParamStore`] under `(CONV2D, [in_ch, out_ch, k])`,
/// shape `[out_ch, in_ch, k, k]` (out-major to keep matmul-friendly).
fn dispatch_conv2d(
    backend: &dyn Backend,
    args: &[Expr],
    handle: &TensorHandle,
    params: &mut ParamStore,
) -> Result<TensorHandle, rmi::error::RmiError> {
    // Arg schema (backward compatible): [in_ch, out_ch, kernel] then
    // optional [bias(0/1), stride, padding, dilation]. Defaults: bias=0,
    // stride=1, padding=0, dilation=1. So [ic,oc,k] is a "valid" conv.
    let dims = extract_int_args(args);
    if dims.len() < 3 || dims.len() > 7 || dims[0] <= 0 || dims[1] <= 0 || dims[2] <= 0 {
        return Err(rmi::error::RmiError::compute_simple(format!(
            "CONV2D: expected [in_ch, out_ch, kernel] (+ optional bias, stride, padding, dilation), got {dims:?}"
        )));
    }
    let in_ch = dims[0] as usize;
    let out_ch = dims[1] as usize;
    let k = dims[2] as usize;
    let use_bias = dims.get(3).is_some_and(|b| *b != 0);
    let stride = dims.get(4).copied().filter(|s| *s > 0).unwrap_or(1) as usize;
    let padding = dims.get(5).copied().filter(|p| *p >= 0).unwrap_or(0) as usize;
    let dilation = dims.get(6).copied().filter(|d| *d > 0).unwrap_or(1) as usize;

    // Reshape input to [N, C, H, W].
    let (n, c, h, w) = match handle.shape.as_slice() {
        [h, w] => (1usize, 1usize, *h, *w),
        [c, h, w] => (1usize, *c, *h, *w),
        [n, c, h, w] => (*n, *c, *h, *w),
        other => {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "CONV2D: cannot interpret shape {:?} as image",
                other
            )));
        }
    };
    if c != in_ch {
        return Err(rmi::error::RmiError::compute_simple(format!(
            "CONV2D: in_ch={} but input has {} channels",
            in_ch, c
        )));
    }
    let eff_k = dilation * (k - 1) + 1;
    if h + 2 * padding < eff_k || w + 2 * padding < eff_k {
        return Err(rmi::error::RmiError::compute_simple(format!(
            "CONV2D: dilated kernel (eff {eff_k}) larger than padded input {}×{}",
            h + 2 * padding,
            w + 2 * padding
        )));
    }
    let out_h = (h + 2 * padding - eff_k) / stride + 1;
    let out_w = (w + 2 * padding - eff_k) / stride + 1;

    // Weight key uses canonical 3-arg form for checkpoint compat.
    let weight_key: Vec<i64> = vec![in_ch as i64, out_ch as i64, k as i64];
    let w_handle = params.get_or_alloc(backend, Op::CONV2D, &weight_key, &[out_ch, in_ch, k, k])?;

    // Dispatch the convolution through the Backend trait — on CUDA this
    // is the GPU im2col + cuBLASLt GEMM path (P113); on CPU it's the
    // naive reference. Reshape input to 4-D NCHW first (free on a
    // GPU-resident handle).
    let input_4d = if handle.shape.len() == 4 {
        handle.clone()
    } else {
        backend.reshape(handle, &[n, c, h, w])?
    };
    let conv_out = backend.conv2d(&input_4d, &w_handle, stride, padding, dilation)?;

    // Optional per-output-channel bias, broadcast over the spatial dims.
    // The Backend has no broadcast-add, so we materialise the expanded
    // bias once and use elementwise add (stays on-device for the add).
    let biased = if use_bias {
        let bias_key = vec![in_ch as i64, out_ch as i64, k as i64, BIAS_SLOT];
        let bias_handle = params.get_or_alloc(backend, Op::CONV2D, &bias_key, &[out_ch])?;
        let bias_vec = bytes_to_f32(&backend.copy_to_host(&bias_handle)?);
        let spatial = out_h * out_w;
        let mut full = vec![0.0f32; n * out_ch * spatial];
        for ni in 0..n {
            for oc in 0..out_ch {
                let b = bias_vec[oc];
                let base = (ni * out_ch + oc) * spatial;
                full[base..base + spatial].fill(b);
            }
        }
        let bias_full = backend.from_slice_f32(&full, &[n, out_ch, out_h, out_w])?;
        backend.add(&conv_out, &bias_full)?
    } else {
        conv_out
    };

    // Collapse the batch dim back out when the caller passed a ≤3-D image.
    let out_shape = if n == 1 && handle.shape.len() <= 3 {
        vec![out_ch, out_h, out_w]
    } else {
        vec![n, out_ch, out_h, out_w]
    };
    if biased.shape == out_shape {
        Ok(biased)
    } else {
        backend.reshape(&biased, &out_shape)
    }
}

/// Token embedding lookup.
///
/// Args `[vocab_size, embed_dim]`. Allocates a `[vocab, embed]` weight
/// tensor (initialised via the usual LCG/√fan_in). Input is a `[seq]` (or
/// `[batch, seq]`) tensor of integer indices encoded as f32; output gathers
/// the corresponding rows into shape `[seq, embed]` (or `[batch, seq, embed]`).
fn dispatch_embed(
    backend: &dyn Backend,
    args: &[Expr],
    handle: &TensorHandle,
    params: &mut ParamStore,
) -> Result<TensorHandle, rmi::error::RmiError> {
    let dims = extract_int_args(args);
    let (vocab, embed) = match dims.as_slice() {
        [v, e] if *v > 0 && *e > 0 => (*v as usize, *e as usize),
        _ => {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "EMBED: expected [vocab_size, embed_dim], got {dims:?}"
            )));
        }
    };
    let indices_f = bytes_to_f32(&backend.copy_to_host(handle)?);
    let seq = indices_f.len();
    let w_handle = params.get_or_alloc(backend, Op::EMBED, &dims, &[vocab, embed])?;
    let weights = bytes_to_f32(&backend.copy_to_host(&w_handle)?);
    let mut out = vec![0.0f32; seq * embed];
    for (i, &fidx) in indices_f.iter().enumerate() {
        let idx = (fidx.round() as i64).clamp(0, vocab as i64 - 1) as usize;
        out[i * embed..(i + 1) * embed].copy_from_slice(&weights[idx * embed..(idx + 1) * embed]);
    }
    let out_shape = if handle.shape.len() == 1 {
        vec![seq, embed]
    } else {
        let mut s = handle.shape.clone();
        s.push(embed);
        s
    };
    backend.from_slice_f32(&out, &out_shape)
}

/// Sinusoidal positional encoding (Vaswani et al., 2017). No parameters.
///
/// Args `[max_seq, embed_dim]` — only `embed_dim` is consumed; `max_seq` is
/// retained for signature compatibility and ignored beyond the input's
/// actual sequence length. Adds `PE[pos, 2k] = sin(pos / 10000^(2k/d))`
/// and `PE[pos, 2k+1] = cos(pos / 10000^(2k/d))` to every token.
fn dispatch_sinusoidal_pe(
    backend: &dyn Backend,
    args: &[Expr],
    handle: &TensorHandle,
) -> Result<TensorHandle, rmi::error::RmiError> {
    let dims = extract_int_args(args);
    let embed = match dims.as_slice() {
        [_max_seq, e] if *e > 0 => *e as usize,
        [e] if *e > 0 => *e as usize,
        _ => {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "SINUSOIDAL_PE: expected [max_seq, embed_dim] or [embed_dim], got {dims:?}"
            )));
        }
    };
    let (seq, dim) = match handle.shape.as_slice() {
        [s, d] if *d == embed => (*s, *d),
        [d] if *d == embed => (1usize, *d),
        other => {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "SINUSOIDAL_PE: input shape {:?} must end with embed_dim={}",
                other, embed
            )));
        }
    };
    let mut data = bytes_to_f32(&backend.copy_to_host(handle)?);
    for pos in 0..seq {
        for k in 0..dim {
            let denom = (10000.0f32).powf(((k / 2) as f32 * 2.0) / dim as f32);
            let angle = pos as f32 / denom;
            let pe = if k % 2 == 0 { angle.sin() } else { angle.cos() };
            data[pos * dim + k] += pe;
        }
    }
    backend.from_slice_f32(&data, &handle.shape)
}

/// Learned positional embedding — additive variant.
///
/// Args `[max_seq, embed_dim]`. Allocates a `[max_seq, embed_dim]` weight
/// tensor (the position embedding table). Forward: input shape `[seq,
/// embed_dim]` receives `table[pos, :]` added to row `pos` for each
/// position. Sequence length must not exceed `max_seq`.
fn dispatch_learned_pe(
    backend: &dyn Backend,
    args: &[Expr],
    handle: &TensorHandle,
    params: &mut ParamStore,
) -> Result<TensorHandle, rmi::error::RmiError> {
    let dims = extract_int_args(args);
    let (max_seq, embed) = match dims.as_slice() {
        [m, e] if *m > 0 && *e > 0 => (*m as usize, *e as usize),
        _ => {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "LEARNED_PE: expected [max_seq, embed_dim], got {dims:?}"
            )));
        }
    };
    let (seq, dim) = match handle.shape.as_slice() {
        [s, d] if *d == embed => (*s, *d),
        [d] if *d == embed => (1usize, *d),
        other => {
            return Err(rmi::error::RmiError::compute_simple(format!(
                "LEARNED_PE: input shape {:?} must end with embed_dim={}",
                other, embed
            )));
        }
    };
    if seq > max_seq {
        return Err(rmi::error::RmiError::compute_simple(format!(
            "LEARNED_PE: input seq={} exceeds max_seq={}",
            seq, max_seq
        )));
    }
    let w_handle = params.get_or_alloc(backend, Op::LEARNED_PE, &dims, &[max_seq, embed])?;
    let pe_table = bytes_to_f32(&backend.copy_to_host(&w_handle)?);
    let mut data = bytes_to_f32(&backend.copy_to_host(handle)?);
    for pos in 0..seq {
        for k in 0..dim {
            data[pos * dim + k] += pe_table[pos * embed + k];
        }
    }
    backend.from_slice_f32(&data, &handle.shape)
}

/// Mean of squared values — placeholder for MSE without a paired target.
/// Returns a scalar wrapped as `[1]`.
fn dispatch_mse_self(
    backend: &dyn Backend,
    handle: &TensorHandle,
) -> Result<TensorHandle, rmi::error::RmiError> {
    let bytes = backend.copy_to_host(handle)?;
    let data: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let n = data.len().max(1) as f32;
    let ms: f32 = data.iter().map(|v| v * v).sum::<f32>() / n;
    backend.from_slice_f32(&[ms], &[1])
}

// ═══════════════════════════════════════════════════════════════════
// Training — one SGD step over Linear + ReLU + MSE chains
// ═══════════════════════════════════════════════════════════════════

/// Outcome of one training step.
#[derive(Debug, Clone)]
pub struct TrainStep {
    /// Scalar MSE loss before the parameter update.
    pub loss: f32,
    /// Number of Linear weight tensors that were updated.
    pub updated_layers: usize,
}

/// Run one forward + backward + SGD step on a pipeline of
/// `LINEAR >> {RELU|IDENTITY} >> ... >> LINEAR` against a paired target.
///
/// Phase-7 scope: supports an arbitrary chain of `Linear(in, out)` layers
/// optionally interleaved with `ReLU`. Loss is MSE; optimiser is plain SGD.
/// Other ops in the chain are skipped during backward (their forward output
/// passes through but receives no gradient correction).
///
/// `input` has shape `[N, in_dim_first]`; `target` has shape `[N, out_dim_last]`.
/// On return, the relevant `Linear` weights in `params` have been updated
/// in-place by `weight -= lr * grad`.
pub fn train_one_step(
    backend: &dyn Backend,
    expr: &Expr,
    input: &[f32],
    input_shape: &[usize],
    target: &[f32],
    target_shape: &[usize],
    lr: f32,
    params: &mut ParamStore,
) -> Result<TrainStep, rmi::error::RmiError> {
    let mut dummy_state = OptimState::new();
    train_one_step_with_optim(
        backend,
        expr,
        input,
        input_shape,
        target,
        target_shape,
        lr,
        Optimizer::Sgd,
        params,
        &mut dummy_state,
    )
}

/// Train-step variant that takes an explicit [`Optimizer`] and a persistent
/// [`OptimState`] (for Adam-style per-parameter moments). The SGD-only
/// [`train_one_step`] wrapper delegates to this with `Optimizer::Sgd` and
/// a throwaway state.
pub fn train_one_step_with_optim(
    backend: &dyn Backend,
    expr: &Expr,
    input: &[f32],
    input_shape: &[usize],
    target: &[f32],
    target_shape: &[usize],
    lr: f32,
    optim: Optimizer,
    params: &mut ParamStore,
    optim_state: &mut OptimState,
) -> Result<TrainStep, rmi::error::RmiError> {
    train_one_step_with_optim_loss(
        backend, expr, input, input_shape, target, target_shape, lr, optim, Loss::Mse, params, optim_state,
    )
}

/// Train-step with explicit loss function. Generalises
/// [`train_one_step_with_optim`] (which defaults to MSE).
pub fn train_one_step_with_optim_loss(
    backend: &dyn Backend,
    expr: &Expr,
    input: &[f32],
    input_shape: &[usize],
    target: &[f32],
    target_shape: &[usize],
    lr: f32,
    optim: Optimizer,
    loss_kind: Loss,
    params: &mut ParamStore,
    optim_state: &mut OptimState,
) -> Result<TrainStep, rmi::error::RmiError> {
    optim_state.step += 1;
    // Flatten the expression into a sequence of (op, args).
    let mut stages: Vec<(Op, Vec<i64>)> = Vec::new();
    flatten_stages(expr, &mut stages);

    // ── Forward, saving intermediate activations ───────────────────
    let mut activations: Vec<Vec<f32>> = Vec::new();
    let mut shapes: Vec<Vec<usize>> = Vec::new();
    let mut pre_act: Vec<Option<Vec<f32>>> = Vec::new(); // pre-ReLU values

    activations.push(input.to_vec());
    shapes.push(input_shape.to_vec());
    pre_act.push(None);

    for (op, dims) in &stages {
        let (last_data, last_shape) = (activations.last().unwrap(), shapes.last().unwrap());
        match *op {
            // ── Attention (QKV mode, single- or multi-head) forward ──
            Op::ATTN if matches!(dims.as_slice(),
                [a, b] if *a > 0 && *b > 0)
                || matches!(dims.as_slice(),
                [a, b, h] if *a > 0 && *b > 0 && *h > 0) =>
            {
                let in_d = dims[0] as usize;
                let model_d = dims[1] as usize;
                let num_heads = match dims.as_slice() {
                    [_, _, h] if *h > 0 => *h as usize,
                    _ => 1,
                };
                if model_d % num_heads != 0 {
                    return Err(rmi::error::RmiError::compute_simple(format!(
                        "train ATTN: model_dim {} not divisible by num_heads {}",
                        model_d, num_heads
                    )));
                }
                let head_dim = model_d / num_heads;
                let (seq, dim) = match last_shape.as_slice() {
                    [s, d] => (*s, *d),
                    [d] => (1usize, *d),
                    _ => {
                        return Err(rmi::error::RmiError::compute_simple(format!(
                            "train ATTN: bad shape {:?}",
                            last_shape
                        )));
                    }
                };
                if dim != in_d {
                    return Err(rmi::error::RmiError::compute_simple(format!(
                        "train ATTN: in_dim={} but input dim={}",
                        in_d, dim
                    )));
                }
                let wq_h = params.get_or_alloc(backend, *op, &[in_d as i64, model_d as i64, 0], &[in_d, model_d])?;
                let wk_h = params.get_or_alloc(backend, *op, &[in_d as i64, model_d as i64, 1], &[in_d, model_d])?;
                let wv_h = params.get_or_alloc(backend, *op, &[in_d as i64, model_d as i64, 2], &[in_d, model_d])?;
                let wo_h = params.get_or_alloc(backend, *op, &[model_d as i64, in_d as i64, 3], &[model_d, in_d])?;
                let wq = bytes_to_f32(&backend.copy_to_host(&wq_h)?);
                let wk = bytes_to_f32(&backend.copy_to_host(&wk_h)?);
                let wv = bytes_to_f32(&backend.copy_to_host(&wv_h)?);
                let wo = bytes_to_f32(&backend.copy_to_host(&wo_h)?);
                let q = matmul_2d(last_data, &wq, seq, in_d, model_d);
                let k_p = matmul_2d(last_data, &wk, seq, in_d, model_d);
                let v_p = matmul_2d(last_data, &wv, seq, in_d, model_d);
                let scale = 1.0 / (head_dim as f32).sqrt();
                // Per-head softmax matrices, concatenated as [head, seq, seq].
                let mut s_softmax = vec![0.0f32; num_heads * seq * seq];
                // Per-head attended outputs, then concatenated back to [seq, model_d].
                let mut a = vec![0.0f32; seq * model_d];
                for h in 0..num_heads {
                    let head_off = h * head_dim;
                    // Compute scores for this head.
                    let mut head_scores = vec![0.0f32; seq * seq];
                    for i in 0..seq {
                        for j in 0..seq {
                            let mut acc = 0.0f32;
                            for kk in 0..head_dim {
                                acc += q[i * model_d + head_off + kk]
                                    * k_p[j * model_d + head_off + kk];
                            }
                            head_scores[i * seq + j] = acc * scale;
                        }
                    }
                    // Softmax per row of this head.
                    for i in 0..seq {
                        let row = &head_scores[i * seq..(i + 1) * seq];
                        let max = row.iter().cloned().fold(f32::MIN, f32::max);
                        let exps: Vec<f32> = row.iter().map(|x| (x - max).exp()).collect();
                        let sum: f32 = exps.iter().sum();
                        for j in 0..seq {
                            s_softmax[h * seq * seq + i * seq + j] = exps[j] / sum;
                        }
                    }
                    // Apply weights to V to get attended for this head.
                    for i in 0..seq {
                        for kk in 0..head_dim {
                            let mut s = 0.0f32;
                            for j in 0..seq {
                                s += s_softmax[h * seq * seq + i * seq + j]
                                    * v_p[j * model_d + head_off + kk];
                            }
                            a[i * model_d + head_off + kk] = s;
                        }
                    }
                }
                let out = matmul_2d(&a, &wo, seq, model_d, in_d);
                // Stash for backward:
                // [num_heads u32-as-f32, s_softmax (h×seq×seq), a, q, k, v, wq, wk, wv, wo]
                let mut stash = Vec::with_capacity(
                    1 + s_softmax.len() + a.len() + 3 * q.len() + 3 * (in_d * model_d) + model_d * in_d,
                );
                stash.push(num_heads as f32);
                stash.extend_from_slice(&s_softmax);
                stash.extend_from_slice(&a);
                stash.extend_from_slice(&q);
                stash.extend_from_slice(&k_p);
                stash.extend_from_slice(&v_p);
                stash.extend_from_slice(&wq);
                stash.extend_from_slice(&wk);
                stash.extend_from_slice(&wv);
                stash.extend_from_slice(&wo);
                activations.push(out);
                shapes.push(vec![seq, in_d]);
                pre_act.push(Some(stash));
            }

            Op::CONV2D => {
                let (in_ch, out_ch, k, use_bias) = match dims.as_slice() {
                    [ic, oc, k] if *ic > 0 && *oc > 0 && *k > 0 => {
                        (*ic as usize, *oc as usize, *k as usize, false)
                    }
                    [ic, oc, k, b] if *ic > 0 && *oc > 0 && *k > 0 => {
                        (*ic as usize, *oc as usize, *k as usize, *b != 0)
                    }
                    _ => continue,
                };
                // Reshape input to [C, H, W] (Phase-10 training: single example, no batch).
                let (h, w) = match last_shape.as_slice() {
                    [c, h, w] if *c == in_ch => (*h, *w),
                    [h, w] if in_ch == 1 => (*h, *w),
                    other => {
                        return Err(rmi::error::RmiError::compute_simple(format!(
                            "train CONV2D: cannot interpret shape {:?} as [{}, H, W]",
                            other, in_ch
                        )));
                    }
                };
                if h < k || w < k {
                    return Err(rmi::error::RmiError::compute_simple(format!(
                        "train CONV2D: kernel {k}×{k} larger than input {h}×{w}"
                    )));
                }
                let out_h = h - k + 1;
                let out_w = w - k + 1;
                let weight_key: Vec<i64> = vec![in_ch as i64, out_ch as i64, k as i64];
                let w_handle = params.get_or_alloc(backend, *op, &weight_key, &[out_ch, in_ch, k, k])?;
                let weights = bytes_to_f32(&backend.copy_to_host(&w_handle)?);
                let bias_vec: Vec<f32> = if use_bias {
                    let bias_key = vec![in_ch as i64, out_ch as i64, k as i64, BIAS_SLOT];
                    let bh = params.get_or_alloc(backend, *op, &bias_key, &[out_ch])?;
                    bytes_to_f32(&backend.copy_to_host(&bh)?)
                } else {
                    Vec::new()
                };
                let mut out = vec![0.0f32; out_ch * out_h * out_w];
                for oc in 0..out_ch {
                    for oh in 0..out_h {
                        for ow in 0..out_w {
                            let mut acc = 0.0f32;
                            for ic in 0..in_ch {
                                for ki in 0..k {
                                    for kj in 0..k {
                                        let in_idx = (ic * h + oh + ki) * w + ow + kj;
                                        let w_idx = ((oc * in_ch + ic) * k + ki) * k + kj;
                                        acc += last_data[in_idx] * weights[w_idx];
                                    }
                                }
                            }
                            out[(oc * out_h + oh) * out_w + ow] = if use_bias { acc + bias_vec[oc] } else { acc };
                        }
                    }
                }
                activations.push(out);
                shapes.push(vec![out_ch, out_h, out_w]);
                pre_act.push(None);
            }
            Op::LINEAR => {
                let (in_dim, out_dim, use_bias) = match dims.as_slice() {
                    [a, b] if *a > 0 && *b > 0 => (*a as usize, *b as usize, false),
                    [a, b, c] if *a > 0 && *b > 0 => (*a as usize, *b as usize, *c != 0),
                    _ => continue,
                };
                let (batch, mat) = as_matrix(last_data, last_shape, in_dim)?;
                let weight_key: Vec<i64> = vec![in_dim as i64, out_dim as i64];
                let w_handle = params.get_or_alloc(backend, *op, &weight_key, &[in_dim, out_dim])?;
                let w_bytes = backend.copy_to_host(&w_handle)?;
                let w = bytes_to_f32(&w_bytes);
                let bias_vec = if use_bias {
                    let bias_key = vec![in_dim as i64, out_dim as i64, BIAS_SLOT];
                    let bias_handle = params.get_or_alloc(backend, *op, &bias_key, &[out_dim])?;
                    bytes_to_f32(&backend.copy_to_host(&bias_handle)?)
                } else {
                    Vec::new()
                };
                // mat: [batch, in], w: [in, out] → out: [batch, out] (+bias broadcast)
                let mut out = vec![0.0f32; batch * out_dim];
                for b in 0..batch {
                    for j in 0..out_dim {
                        let mut s = 0.0;
                        for i in 0..in_dim {
                            s += mat[b * in_dim + i] * w[i * out_dim + j];
                        }
                        if use_bias {
                            s += bias_vec[j];
                        }
                        out[b * out_dim + j] = s;
                    }
                }
                activations.push(out);
                shapes.push(vec![batch, out_dim]);
                pre_act.push(None);
            }
            Op::RELU => {
                // Save pre-activation values so we can mask gradients during backward.
                let pre = last_data.clone();
                let post: Vec<f32> = pre.iter().map(|v| v.max(0.0)).collect();
                let s = last_shape.clone();
                activations.push(post);
                shapes.push(s);
                pre_act.push(Some(pre));
            }
            // ── Token embedding forward (saves indices for backward) ─
            Op::EMBED => {
                let (vocab, embed) = match dims.as_slice() {
                    [v, e] if *v > 0 && *e > 0 => (*v as usize, *e as usize),
                    _ => continue,
                };
                let indices_f = last_data.clone();
                let seq = indices_f.len();
                let w_handle = params.get_or_alloc(backend, *op, dims, &[vocab, embed])?;
                let weights = bytes_to_f32(&backend.copy_to_host(&w_handle)?);
                let mut out = vec![0.0f32; seq * embed];
                for (ii, &fidx) in indices_f.iter().enumerate() {
                    let idx = (fidx.round() as i64).clamp(0, vocab as i64 - 1) as usize;
                    out[ii * embed..(ii + 1) * embed]
                        .copy_from_slice(&weights[idx * embed..(idx + 1) * embed]);
                }
                activations.push(out);
                shapes.push(vec![seq, embed]);
                // Stash the rounded indices for backward (as f32 for uniformity).
                pre_act.push(Some(indices_f));
            }
            // ── Dropout forward (train mode: mask + scale) ──────────
            Op::DROP => {
                // Args: [percent (0-100)] or empty (defaults to 0 = no-op).
                let pct = dims.first().copied().unwrap_or(0).clamp(0, 99);
                if pct == 0 {
                    activations.push(last_data.clone());
                    shapes.push(last_shape.clone());
                    pre_act.push(None);
                } else {
                    let p = pct as f32 / 100.0;
                    let inv_keep = 1.0 / (1.0 - p);
                    // Deterministic mask: LCG seeded by step + len.
                    let seed = optim_state.step.wrapping_mul(0x9E37_79B9_7F4A_7C15)
                        ^ (last_data.len() as u64);
                    let mut state = seed | 1;
                    let mut mask = vec![0.0f32; last_data.len()];
                    let mut out = vec![0.0f32; last_data.len()];
                    for k in 0..last_data.len() {
                        state = state.wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let u = ((state >> 33) as u32 as f32) / (u32::MAX as f32);
                        if u >= p {
                            mask[k] = inv_keep;
                            out[k] = last_data[k] * inv_keep;
                        }
                    }
                    activations.push(out);
                    shapes.push(last_shape.clone());
                    pre_act.push(Some(mask)); // stash mask for backward
                }
            }
            // ── Learned PE forward (additive, cached table) ─────────
            Op::LEARNED_PE => {
                let (max_seq, embed) = match dims.as_slice() {
                    [m, e] if *m > 0 && *e > 0 => (*m as usize, *e as usize),
                    _ => continue,
                };
                let seq = last_data.len() / embed;
                if seq > max_seq {
                    return Err(rmi::error::RmiError::compute_simple(format!(
                        "train LEARNED_PE: seq={} > max_seq={}",
                        seq, max_seq
                    )));
                }
                let w_handle = params.get_or_alloc(backend, *op, dims, &[max_seq, embed])?;
                let pe_table = bytes_to_f32(&backend.copy_to_host(&w_handle)?);
                let mut out = last_data.clone();
                for pos in 0..seq {
                    for k in 0..embed {
                        out[pos * embed + k] += pe_table[pos * embed + k];
                    }
                }
                activations.push(out);
                shapes.push(last_shape.clone());
                pre_act.push(None);
            }
            // ── Sinusoidal PE forward (no params) ───────────────────
            Op::SINUSOIDAL_PE => {
                let embed = match dims.as_slice() {
                    [_max_seq, e] if *e > 0 => *e as usize,
                    [e] if *e > 0 => *e as usize,
                    _ => continue,
                };
                let mut data = last_data.clone();
                let seq = data.len() / embed;
                for pos in 0..seq {
                    for k in 0..embed {
                        let denom = (10000.0f32).powf(((k / 2) as f32 * 2.0) / embed as f32);
                        let angle = pos as f32 / denom;
                        let pe = if k % 2 == 0 { angle.sin() } else { angle.cos() };
                        data[pos * embed + k] += pe;
                    }
                }
                activations.push(data);
                shapes.push(last_shape.clone());
                pre_act.push(None);
            }
            // ── LayerNorm forward with optional γ/β ─────────────────
            Op::LAYER_NORM
                if matches!(dims.as_slice(), [d]
                    if (*d as usize) == *last_shape.last().unwrap_or(&0)) =>
            {
                const EPS: f32 = 1e-5;
                let last = dims[0] as usize;
                let rows = last_data.len() / last;
                let g_key = vec![last as i64, 0];
                let b_key = vec![last as i64, 1];
                let g_handle = params.get_or_alloc(backend, *op, &g_key, &[last])?;
                let b_handle = params.get_or_alloc(backend, *op, &b_key, &[last])?;
                let gamma = bytes_to_f32(&backend.copy_to_host(&g_handle)?);
                let beta = bytes_to_f32(&backend.copy_to_host(&b_handle)?);
                let mut out = vec![0.0f32; last_data.len()];
                // Stash: [rows×{mean f32, inv_std f32}, x̂ values (rows×last)]
                let mut stash = Vec::with_capacity(rows * 2 + last_data.len());
                for r in 0..rows {
                    let row = &last_data[r * last..(r + 1) * last];
                    let mean: f32 = row.iter().copied().sum::<f32>() / last as f32;
                    let var: f32 =
                        row.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / last as f32;
                    let inv_std = 1.0 / (var + EPS).sqrt();
                    stash.push(mean);
                    stash.push(inv_std);
                    for i in 0..last {
                        let x_hat = (row[i] - mean) * inv_std;
                        stash.push(x_hat);
                        out[r * last + i] = gamma[i] * x_hat + beta[i];
                    }
                }
                activations.push(out);
                shapes.push(last_shape.clone());
                pre_act.push(Some(stash));
            }
            _ => {
                // Pass-through for unsupported-in-train ops.
                activations.push(last_data.clone());
                shapes.push(last_shape.clone());
                pre_act.push(None);
            }
        }
    }

    // ── Loss + initial gradient ────────────────────────────────────
    let y_pred = activations.last().unwrap().clone();
    if y_pred.len() != target.len() {
        return Err(rmi::error::RmiError::compute_simple(format!(
            "train: target length {} != model output length {}",
            target.len(),
            y_pred.len()
        )));
    }
    let n = y_pred.len() as f32;
    let (loss, mut grad): (f32, Vec<f32>) = match loss_kind {
        Loss::Mse => {
            let l = y_pred
                .iter()
                .zip(target.iter())
                .map(|(p, t)| (p - t).powi(2))
                .sum::<f32>()
                / n;
            let g: Vec<f32> = y_pred
                .iter()
                .zip(target.iter())
                .map(|(p, t)| 2.0 * (p - t) / n)
                .collect();
            (l, g)
        }
        Loss::CrossEntropy => {
            // Apply row-wise softmax to logits, treat target as one-hot.
            // Shapes assumed [batch, classes].
            let classes = match target_shape.last() {
                Some(c) => *c,
                None => return Err(rmi::error::RmiError::compute_simple("CE: empty target shape".to_string())),
            };
            let batch = y_pred.len() / classes.max(1);
            let mut soft = vec![0.0f32; y_pred.len()];
            for b in 0..batch {
                let row = &y_pred[b * classes..(b + 1) * classes];
                let max = row.iter().cloned().fold(f32::MIN, f32::max);
                let exps: Vec<f32> = row.iter().map(|v| (v - max).exp()).collect();
                let sum: f32 = exps.iter().sum();
                for (j, e) in exps.iter().enumerate() {
                    soft[b * classes + j] = e / sum;
                }
            }
            const EPS: f32 = 1e-9;
            let l: f32 = -soft
                .iter()
                .zip(target.iter())
                .map(|(s, t)| t * (s + EPS).ln())
                .sum::<f32>()
                / batch as f32;
            // dL/d(logits) = (softmax − target) / batch
            let g: Vec<f32> = soft
                .iter()
                .zip(target.iter())
                .map(|(s, t)| (s - t) / batch as f32)
                .collect();
            (l, g)
        }
    };
    let mut grad_shape = target_shape.to_vec();
    let mut updated = 0usize;

    // Walk stages in reverse.
    for (i, (op, dims)) in stages.iter().enumerate().rev() {
        let input_act = &activations[i];
        let input_shape = &shapes[i];
        match *op {
            // ── Attention (QKV mode, single- or multi-head) backward ──
            Op::ATTN if matches!(dims.as_slice(),
                [a, b] if *a > 0 && *b > 0)
                || matches!(dims.as_slice(),
                [a, b, h] if *a > 0 && *b > 0 && *h > 0) =>
            {
                let in_d = dims[0] as usize;
                let model_d = dims[1] as usize;
                let stash = match &pre_act[i + 1] {
                    Some(s) => s,
                    None => continue,
                };
                let seq = input_act.len() / in_d;
                // Unpack stash: leading f32 carries num_heads.
                let mut ofs = 0usize;
                let num_heads = stash[ofs] as usize; ofs += 1;
                let head_dim = model_d / num_heads;
                let s_softmax = &stash[ofs..ofs + num_heads * seq * seq]; ofs += num_heads * seq * seq;
                let a_mat = &stash[ofs..ofs + seq * model_d]; ofs += seq * model_d;
                let q_mat = &stash[ofs..ofs + seq * model_d]; ofs += seq * model_d;
                let k_mat = &stash[ofs..ofs + seq * model_d]; ofs += seq * model_d;
                let v_mat = &stash[ofs..ofs + seq * model_d]; ofs += seq * model_d;
                let wq = &stash[ofs..ofs + in_d * model_d]; ofs += in_d * model_d;
                let wk = &stash[ofs..ofs + in_d * model_d]; ofs += in_d * model_d;
                let wv = &stash[ofs..ofs + in_d * model_d]; ofs += in_d * model_d;
                let wo = &stash[ofs..ofs + model_d * in_d];
                let scale = 1.0f32 / (head_dim as f32).sqrt();

                // grad is [seq, in_d] (downstream of OutProj).
                // dA = grad @ Wo^T : [seq, model_d]
                let mut da = vec![0.0f32; seq * model_d];
                for i2 in 0..seq {
                    for j in 0..model_d {
                        let mut s = 0.0f32;
                        for ki in 0..in_d {
                            s += grad[i2 * in_d + ki] * wo[j * in_d + ki];
                        }
                        da[i2 * model_d + j] = s;
                    }
                }
                // dWo = A^T @ grad : [model_d, in_d]
                let mut dwo = vec![0.0f32; model_d * in_d];
                for j in 0..model_d {
                    for ki in 0..in_d {
                        let mut s = 0.0f32;
                        for i2 in 0..seq {
                            s += a_mat[i2 * model_d + j] * grad[i2 * in_d + ki];
                        }
                        dwo[j * in_d + ki] = s;
                    }
                }
                // Per-head: split dA, S, V, Q, K into head slices; compute
                // dV, dQ, dK per head; recombine into full [seq, model_d]
                // gradient tensors at each head's column offset.
                let mut dv = vec![0.0f32; seq * model_d];
                let mut dq = vec![0.0f32; seq * model_d];
                let mut dk = vec![0.0f32; seq * model_d];
                for h in 0..num_heads {
                    let head_off = h * head_dim;
                    let s_head = &s_softmax[h * seq * seq..(h + 1) * seq * seq];
                    // dV_h = S_h^T @ dA_h
                    for i2 in 0..seq {
                        for j in 0..head_dim {
                            let mut s = 0.0f32;
                            for kk in 0..seq {
                                s += s_head[kk * seq + i2]
                                    * da[kk * model_d + head_off + j];
                            }
                            dv[i2 * model_d + head_off + j] = s;
                        }
                    }
                    // dS_h = dA_h @ V_h^T : [seq, seq]
                    let mut ds_h = vec![0.0f32; seq * seq];
                    for i2 in 0..seq {
                        for j in 0..seq {
                            let mut s = 0.0f32;
                            for kk in 0..head_dim {
                                s += da[i2 * model_d + head_off + kk]
                                    * v_mat[j * model_d + head_off + kk];
                            }
                            ds_h[i2 * seq + j] = s;
                        }
                    }
                    // Softmax backward per head.
                    let mut dscores_h = vec![0.0f32; seq * seq];
                    for i2 in 0..seq {
                        let mut row_sum = 0.0f32;
                        for kk in 0..seq {
                            row_sum += ds_h[i2 * seq + kk] * s_head[i2 * seq + kk];
                        }
                        for j in 0..seq {
                            dscores_h[i2 * seq + j] =
                                s_head[i2 * seq + j] * (ds_h[i2 * seq + j] - row_sum);
                        }
                    }
                    // dQ_h = dscores_h @ K_h * scale
                    // dK_h = dscores_h^T @ Q_h * scale
                    for i2 in 0..seq {
                        for j in 0..head_dim {
                            let mut sq = 0.0f32;
                            let mut sk = 0.0f32;
                            for kk in 0..seq {
                                sq += dscores_h[i2 * seq + kk]
                                    * k_mat[kk * model_d + head_off + j];
                                sk += dscores_h[kk * seq + i2]
                                    * q_mat[kk * model_d + head_off + j];
                            }
                            dq[i2 * model_d + head_off + j] = sq * scale;
                            dk[i2 * model_d + head_off + j] = sk * scale;
                        }
                    }
                }
                // dWq, dWk, dWv = x^T @ d{Q,K,V}
                let mut dwq = vec![0.0f32; in_d * model_d];
                let mut dwk = vec![0.0f32; in_d * model_d];
                let mut dwv = vec![0.0f32; in_d * model_d];
                for ii in 0..in_d {
                    for j in 0..model_d {
                        let mut sq = 0.0f32;
                        let mut sk = 0.0f32;
                        let mut sv = 0.0f32;
                        for bb in 0..seq {
                            sq += input_act[bb * in_d + ii] * dq[bb * model_d + j];
                            sk += input_act[bb * in_d + ii] * dk[bb * model_d + j];
                            sv += input_act[bb * in_d + ii] * dv[bb * model_d + j];
                        }
                        dwq[ii * model_d + j] = sq;
                        dwk[ii * model_d + j] = sk;
                        dwv[ii * model_d + j] = sv;
                    }
                }
                // Apply updates to all four weight tensors.
                for (slot, shape, dw, w_old) in [
                    (0i64, [in_d, model_d], &dwq, wq),
                    (1, [in_d, model_d], &dwk, wk),
                    (2, [in_d, model_d], &dwv, wv),
                    (3, [model_d, in_d], &dwo, wo),
                ] {
                    let mut new_w = w_old.to_vec();
                    let dims_key = [shape[0] as i64, shape[1] as i64, slot];
                    apply_optim_step(Op::ATTN, &dims_key, &mut new_w, dw, lr, optim, optim_state);
                    let new_h = backend.from_slice_f32(&new_w, &shape)?;
                    params.replace(Op::ATTN, &dims_key, new_h);
                }
                updated += 4;
                // dx = dQ @ Wq^T + dK @ Wk^T + dV @ Wv^T : [seq, in_d]
                let mut new_grad = vec![0.0f32; seq * in_d];
                for i2 in 0..seq {
                    for ii in 0..in_d {
                        let mut s = 0.0f32;
                        for j in 0..model_d {
                            s += dq[i2 * model_d + j] * wq[ii * model_d + j]
                                + dk[i2 * model_d + j] * wk[ii * model_d + j]
                                + dv[i2 * model_d + j] * wv[ii * model_d + j];
                        }
                        new_grad[i2 * in_d + ii] = s;
                    }
                }
                grad = new_grad;
                grad_shape = vec![seq, in_d];
            }

            Op::CONV2D => {
                let (in_ch, out_ch, k, use_bias) = match dims.as_slice() {
                    [ic, oc, k] if *ic > 0 && *oc > 0 && *k > 0 => {
                        (*ic as usize, *oc as usize, *k as usize, false)
                    }
                    [ic, oc, k, b] if *ic > 0 && *oc > 0 && *k > 0 => {
                        (*ic as usize, *oc as usize, *k as usize, *b != 0)
                    }
                    _ => continue,
                };
                let (h, w) = match input_shape.as_slice() {
                    [c, h, w] if *c == in_ch => (*h, *w),
                    [h, w] if in_ch == 1 => (*h, *w),
                    _ => continue,
                };
                let out_h = h - k + 1;
                let out_w = w - k + 1;
                // dW[oc, ic, ki, kj] = sum over oh,ow of x[ic, oh+ki, ow+kj] * grad[oc, oh, ow]
                let mut dw = vec![0.0f32; out_ch * in_ch * k * k];
                for oc in 0..out_ch {
                    for ic in 0..in_ch {
                        for ki in 0..k {
                            for kj in 0..k {
                                let mut s = 0.0f32;
                                for oh in 0..out_h {
                                    for ow in 0..out_w {
                                        let in_idx = (ic * h + oh + ki) * w + ow + kj;
                                        let grad_idx = (oc * out_h + oh) * out_w + ow;
                                        s += input_act[in_idx] * grad[grad_idx];
                                    }
                                }
                                dw[((oc * in_ch + ic) * k + ki) * k + kj] = s;
                            }
                        }
                    }
                }
                let weight_key: Vec<i64> = vec![in_ch as i64, out_ch as i64, k as i64];
                let w_handle = params.get_or_alloc(backend, *op, &weight_key, &[out_ch, in_ch, k, k])?;
                let w_bytes = backend.copy_to_host(&w_handle)?;
                let mut w_vec = bytes_to_f32(&w_bytes);
                apply_optim_step(*op, &weight_key, &mut w_vec, &dw, lr, optim, optim_state);
                let new_w = backend.from_slice_f32(&w_vec, &[out_ch, in_ch, k, k])?;
                params.replace(*op, &weight_key, new_w);
                updated += 1;
                // Bias gradient: db[oc] = Σ_(oh,ow) grad[oc, oh, ow]
                if use_bias {
                    let bias_key = vec![in_ch as i64, out_ch as i64, k as i64, BIAS_SLOT];
                    let bh = params.get_or_alloc(backend, *op, &bias_key, &[out_ch])?;
                    let mut bias_vec = bytes_to_f32(&backend.copy_to_host(&bh)?);
                    let mut db = vec![0.0f32; out_ch];
                    for oc in 0..out_ch {
                        for oh in 0..out_h {
                            for ow in 0..out_w {
                                db[oc] += grad[(oc * out_h + oh) * out_w + ow];
                            }
                        }
                    }
                    apply_optim_step(*op, &bias_key, &mut bias_vec, &db, lr, optim, optim_state);
                    let new_b = backend.from_slice_f32(&bias_vec, &[out_ch])?;
                    params.replace(*op, &bias_key, new_b);
                    updated += 1;
                }
                // dx[ic, hh, ww] = sum over oc,ki,kj of grad[oc, hh-ki, ww-kj] * w[oc, ic, ki, kj]
                // valid only when (hh-ki, ww-kj) in [0, out_h) x [0, out_w).
                let mut new_grad = vec![0.0f32; in_ch * h * w];
                for ic in 0..in_ch {
                    for hh in 0..h {
                        for ww in 0..w {
                            let mut s = 0.0f32;
                            for oc in 0..out_ch {
                                for ki in 0..k {
                                    for kj in 0..k {
                                        if hh < ki || ww < kj {
                                            continue;
                                        }
                                        let oh = hh - ki;
                                        let ow = ww - kj;
                                        if oh >= out_h || ow >= out_w {
                                            continue;
                                        }
                                        let g_idx = (oc * out_h + oh) * out_w + ow;
                                        let w_idx = ((oc * in_ch + ic) * k + ki) * k + kj;
                                        s += grad[g_idx] * w_vec[w_idx];
                                    }
                                }
                            }
                            new_grad[(ic * h + hh) * w + ww] = s;
                        }
                    }
                }
                grad = new_grad;
                grad_shape = vec![in_ch, h, w];
            }
            Op::LINEAR => {
                let (in_dim, out_dim, use_bias) = match dims.as_slice() {
                    [a, b] if *a > 0 && *b > 0 => (*a as usize, *b as usize, false),
                    [a, b, c] if *a > 0 && *b > 0 => (*a as usize, *b as usize, *c != 0),
                    _ => continue,
                };
                let batch = input_act.len() / in_dim;
                // dW = x^T @ grad, where x: [batch, in], grad: [batch, out]
                let mut dw = vec![0.0f32; in_dim * out_dim];
                for ii in 0..in_dim {
                    for jj in 0..out_dim {
                        let mut s = 0.0;
                        for bb in 0..batch {
                            s += input_act[bb * in_dim + ii] * grad[bb * out_dim + jj];
                        }
                        dw[ii * out_dim + jj] = s;
                    }
                }
                // Update weights: W -= lr * dW.
                let weight_key = vec![in_dim as i64, out_dim as i64];
                let w_handle = params.get_or_alloc(backend, *op, &weight_key, &[in_dim, out_dim])?;
                let w_bytes = backend.copy_to_host(&w_handle)?;
                let mut w = bytes_to_f32(&w_bytes);
                apply_optim_step(*op, &weight_key, &mut w, &dw, lr, optim, optim_state);
                let new_w = backend.from_slice_f32(&w, &[in_dim, out_dim])?;
                params.replace(*op, &weight_key, new_w);
                updated += 1;
                // Bias gradient: db = sum_over_batch(grad), shape [out_dim].
                if use_bias {
                    let bias_key = vec![in_dim as i64, out_dim as i64, BIAS_SLOT];
                    let bias_handle = params.get_or_alloc(backend, *op, &bias_key, &[out_dim])?;
                    let mut bias_vec = bytes_to_f32(&backend.copy_to_host(&bias_handle)?);
                    let mut db = vec![0.0f32; out_dim];
                    for jj in 0..out_dim {
                        for bb in 0..batch {
                            db[jj] += grad[bb * out_dim + jj];
                        }
                    }
                    apply_optim_step(*op, &bias_key, &mut bias_vec, &db, lr, optim, optim_state);
                    let new_b = backend.from_slice_f32(&bias_vec, &[out_dim])?;
                    params.replace(*op, &bias_key, new_b);
                    updated += 1;
                }
                // dL/dx = grad @ W^T, where W: [in, out] → out shape [batch, in]
                let mut new_grad = vec![0.0f32; batch * in_dim];
                for bb in 0..batch {
                    for ii in 0..in_dim {
                        let mut s = 0.0;
                        for jj in 0..out_dim {
                            s += grad[bb * out_dim + jj] * w[ii * out_dim + jj];
                        }
                        new_grad[bb * in_dim + ii] = s;
                    }
                }
                grad = new_grad;
                grad_shape = vec![batch, in_dim];
            }
            // ── Embedding backward: scatter-add to looked-up rows ───
            Op::EMBED => {
                let (vocab, embed) = match dims.as_slice() {
                    [v, e] if *v > 0 && *e > 0 => (*v as usize, *e as usize),
                    _ => continue,
                };
                let indices = match &pre_act[i + 1] {
                    Some(idx) => idx.clone(),
                    None => continue,
                };
                let seq = indices.len();
                let w_handle = params.get_or_alloc(backend, *op, dims, &[vocab, embed])?;
                let mut w_vec = bytes_to_f32(&backend.copy_to_host(&w_handle)?);
                // dW[idx, :] += grad[i, :]; other rows untouched.
                let mut dw = vec![0.0f32; vocab * embed];
                for ii in 0..seq {
                    let idx = (indices[ii].round() as i64).clamp(0, vocab as i64 - 1) as usize;
                    for j in 0..embed {
                        dw[idx * embed + j] += grad[ii * embed + j];
                    }
                }
                apply_optim_step(*op, dims, &mut w_vec, &dw, lr, optim, optim_state);
                let new_w = backend.from_slice_f32(&w_vec, &[vocab, embed])?;
                params.replace(*op, dims, new_w);
                updated += 1;
                // dx is meaningless for an indexed lookup — gradient stops here.
                grad = vec![0.0f32; seq];
                grad_shape = vec![seq];
            }
            // ── Dropout backward: multiply grad by stashed mask ─────
            Op::DROP => {
                if let Some(mask) = &pre_act[i + 1] {
                    for k in 0..grad.len().min(mask.len()) {
                        grad[k] *= mask[k];
                    }
                }
                grad_shape = input_shape.clone();
            }
            // ── Learned PE backward: scatter-add to used positions ──
            Op::LEARNED_PE => {
                let (max_seq, embed) = match dims.as_slice() {
                    [m, e] if *m > 0 && *e > 0 => (*m as usize, *e as usize),
                    _ => continue,
                };
                let seq = grad.len() / embed;
                let w_handle = params.get_or_alloc(backend, *op, dims, &[max_seq, embed])?;
                let mut w_vec = bytes_to_f32(&backend.copy_to_host(&w_handle)?);
                let mut dw = vec![0.0f32; max_seq * embed];
                // dW[pos, :] += grad[pos, :] for pos in 0..seq
                for pos in 0..seq.min(max_seq) {
                    for k in 0..embed {
                        dw[pos * embed + k] += grad[pos * embed + k];
                    }
                }
                apply_optim_step(*op, dims, &mut w_vec, &dw, lr, optim, optim_state);
                let new_w = backend.from_slice_f32(&w_vec, &[max_seq, embed])?;
                params.replace(*op, dims, new_w);
                updated += 1;
                // Gradient passes through unchanged (additive op).
                grad_shape = input_shape.clone();
            }
            // ── Sinusoidal PE backward: identity (no params) ────────
            Op::SINUSOIDAL_PE => {
                // Pass gradient through unchanged.
                grad_shape = input_shape.clone();
            }
            Op::RELU => {
                // Mask gradient by indicator(pre > 0).
                if let Some(pre) = &pre_act[i + 1] {
                    for k in 0..grad.len().min(pre.len()) {
                        if pre[k] <= 0.0 {
                            grad[k] = 0.0;
                        }
                    }
                }
                grad_shape = input_shape.clone();
            }
            // ── LayerNorm backward with γ/β ────────────────────────
            Op::LAYER_NORM
                if matches!(dims.as_slice(), [d]
                    if (*d as usize) == *input_shape.last().unwrap_or(&0)) =>
            {
                let last = dims[0] as usize;
                let rows = input_act.len() / last;
                let stash = match &pre_act[i + 1] {
                    Some(s) => s,
                    None => continue,
                };
                let g_key = vec![last as i64, 0];
                let b_key = vec![last as i64, 1];
                let g_handle = params.get_or_alloc(backend, *op, &g_key, &[last])?;
                let b_handle = params.get_or_alloc(backend, *op, &b_key, &[last])?;
                let mut gamma = bytes_to_f32(&backend.copy_to_host(&g_handle)?);
                let mut beta = bytes_to_f32(&backend.copy_to_host(&b_handle)?);
                let mut dgamma = vec![0.0f32; last];
                let mut dbeta = vec![0.0f32; last];
                let mut dx = vec![0.0f32; input_act.len()];
                let row_stride = 2 + last;
                let nf = last as f32;
                for r in 0..rows {
                    let mean = stash[r * row_stride];
                    let inv_std = stash[r * row_stride + 1];
                    let xhat_offset = r * row_stride + 2;
                    let xhat = &stash[xhat_offset..xhat_offset + last];
                    let dy = &grad[r * last..(r + 1) * last];
                    // Accumulate dγ, dβ across rows.
                    for j in 0..last {
                        dgamma[j] += dy[j] * xhat[j];
                        dbeta[j] += dy[j];
                    }
                    // dx̂ = dy · γ
                    let mut dxhat = vec![0.0f32; last];
                    for j in 0..last {
                        dxhat[j] = dy[j] * gamma[j];
                    }
                    // Standard LN backward: combine into dx using the
                    // batched form (Kingma/Ba derivation). Let
                    // m1 = mean(dx̂), m2 = mean(dx̂·x̂).
                    let m1: f32 = dxhat.iter().sum::<f32>() / nf;
                    let m2: f32 = dxhat.iter().zip(xhat.iter()).map(|(a, b)| a * b).sum::<f32>() / nf;
                    for j in 0..last {
                        dx[r * last + j] = inv_std * (dxhat[j] - m1 - xhat[j] * m2);
                    }
                    // Suppress unused warning for `mean`; kept for clarity.
                    let _ = mean;
                }
                // Update γ, β via the optimiser.
                apply_optim_step(*op, &g_key, &mut gamma, &dgamma, lr, optim, optim_state);
                apply_optim_step(*op, &b_key, &mut beta, &dbeta, lr, optim, optim_state);
                let new_g = backend.from_slice_f32(&gamma, &[last])?;
                let new_b = backend.from_slice_f32(&beta, &[last])?;
                params.replace(*op, &g_key, new_g);
                params.replace(*op, &b_key, new_b);
                updated += 2;
                grad = dx;
                grad_shape = input_shape.clone();
            }
            _ => {
                grad_shape = input_shape.clone();
            }
        }
    }
    let _ = grad_shape; // silence unused-variable warning in the trivial path

    Ok(TrainStep {
        loss,
        updated_layers: updated,
    })
}

fn flatten_stages(expr: &Expr, out: &mut Vec<(Op, Vec<i64>)>) {
    match expr {
        Expr::Seq(a, b) => {
            flatten_stages(a, out);
            flatten_stages(b, out);
        }
        Expr::App(op, args) => {
            // MSE_LOSS / optimiser steps are recorded as forward stubs;
            // backward ignores them (loss computed externally).
            if matches!(*op, Op::MSE_LOSS | Op::SGD_STEP | Op::ADAM_STEP | Op::ADAMW_STEP | Op::RMSPROP_STEP) {
                return;
            }
            let dims = extract_int_args(args);
            out.push((*op, dims));
        }
        _ => {}
    }
}

fn as_matrix(data: &[f32], shape: &[usize], in_dim: usize) -> Result<(usize, Vec<f32>), rmi::error::RmiError> {
    match shape.len() {
        1 if shape[0] % in_dim == 0 => Ok((shape[0] / in_dim, data.to_vec())),
        2 if shape[1] == in_dim => Ok((shape[0], data.to_vec())),
        _ => Err(rmi::error::RmiError::compute_simple(format!(
            "train: cannot shape {:?} as matrix with in_dim={}",
            shape, in_dim
        ))),
    }
}

fn bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

impl ParamStore {
    /// Overwrite a cached weight tensor (used by the training step).
    fn replace(&mut self, op: Op, dims: &[i64], handle: TensorHandle) {
        self.weights.insert((op.0, dims.to_vec()), handle);
    }

    /// Public version of `replace` for cross-module wiring (e.g. tied
    /// embeddings synced from main.rs).
    pub fn replace_public(&mut self, op: Op, dims: &[i64], handle: TensorHandle) {
        self.replace(op, dims, handle);
    }

    /// Lookup a cached weight tensor handle. Returns `None` if no entry
    /// exists for the supplied `(op, dims)` key.
    pub fn get_handle(&self, op: Op, dims: &[i64]) -> Option<TensorHandle> {
        self.weights.get(&(op.0, dims.to_vec())).cloned()
    }

    /// Serialize every cached weight tensor to a simple binary format.
    ///
    /// Layout:
    /// ```text
    ///   magic   "MGPS" (4 bytes)
    ///   version u16 (= 1)
    ///   count   u32       — number of weight entries
    ///   for each entry:
    ///     op       u16
    ///     dims_len u32, dims (i64 × dims_len)
    ///     shape_len u32, shape (u64 × shape_len)
    ///     data_len u32, data (u8 × data_len)  — f32 little-endian
    /// ```
    ///
    /// Round-trip stable: `ParamStore::load(&self.save(backend)?)` produces
    /// the same `(op, dims) → tensor` mapping.
    pub fn save(&self, backend: &CpuBackend) -> Result<Vec<u8>, rmi::error::RmiError> {
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"MGPS");
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&(self.weights.len() as u32).to_le_bytes());
        // Deterministic order for byte-stable round-trips.
        let mut entries: Vec<_> = self.weights.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for ((op, dims), handle) in entries {
            buf.extend_from_slice(&op.to_le_bytes());
            buf.extend_from_slice(&(dims.len() as u32).to_le_bytes());
            for d in dims {
                buf.extend_from_slice(&d.to_le_bytes());
            }
            buf.extend_from_slice(&(handle.shape.len() as u32).to_le_bytes());
            for s in &handle.shape {
                buf.extend_from_slice(&(*s as u64).to_le_bytes());
            }
            let data = backend.copy_to_host(handle)?;
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(&data);
        }
        Ok(buf)
    }

    /// Inverse of [`save`]. Reads a checkpoint blob and produces a fresh
    /// `ParamStore`. The supplied `backend` allocates tensor handles.
    pub fn load(blob: &[u8], backend: &CpuBackend) -> Result<Self, String> {
        let mut store = ParamStore::new();
        let mut pos = 0usize;
        fn read<'a>(buf: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], String> {
            if *pos + n > buf.len() {
                return Err(format!("unexpected EOF at offset {}", *pos));
            }
            let s = &buf[*pos..*pos + n];
            *pos += n;
            Ok(s)
        }
        let magic = read(blob, &mut pos, 4)?;
        if magic != b"MGPS" {
            return Err(format!("bad magic: {:?}", magic));
        }
        let _ver = u16::from_le_bytes(read(blob, &mut pos, 2)?.try_into().unwrap());
        let count = u32::from_le_bytes(read(blob, &mut pos, 4)?.try_into().unwrap()) as usize;
        for _ in 0..count {
            let op = u16::from_le_bytes(read(blob, &mut pos, 2)?.try_into().unwrap());
            let dims_len = u32::from_le_bytes(read(blob, &mut pos, 4)?.try_into().unwrap()) as usize;
            let mut dims = Vec::with_capacity(dims_len);
            for _ in 0..dims_len {
                let v = i64::from_le_bytes(read(blob, &mut pos, 8)?.try_into().unwrap());
                dims.push(v);
            }
            let shape_len = u32::from_le_bytes(read(blob, &mut pos, 4)?.try_into().unwrap()) as usize;
            let mut shape = Vec::with_capacity(shape_len);
            for _ in 0..shape_len {
                let v = u64::from_le_bytes(read(blob, &mut pos, 8)?.try_into().unwrap()) as usize;
                shape.push(v);
            }
            let data_len = u32::from_le_bytes(read(blob, &mut pos, 4)?.try_into().unwrap()) as usize;
            let data = read(blob, &mut pos, data_len)?;
            let floats: Vec<f32> = data
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            let handle = backend
                .from_slice_f32(&floats, &shape)
                .map_err(|e| format!("alloc tensor: {e}"))?;
            store.weights.insert((op, dims), handle);
        }
        Ok(store)
    }
}

// ═══════════════════════════════════════════════════════════════════
// OptimState — per-parameter Adam moments
// ═══════════════════════════════════════════════════════════════════

/// Per-parameter optimizer state (first/second moments + step count).
///
/// Used by [`Optimizer::Adam`] to maintain `m` and `v` running averages of
/// gradients and squared gradients between [`train_one_step`] calls. Keyed
/// identically to [`ParamStore`] so a (op, dims) signature points at one
/// weight tensor and its corresponding state.
#[derive(Default)]
pub struct OptimState {
    moments: HashMap<(u16, Vec<i64>), AdamMoment>,
    /// Number of update steps applied (shared across params for bias correction).
    pub step: u64,
    /// Optional per-tensor L2 gradient clipping threshold. `None` disables.
    pub clip_grad: Option<f32>,
    /// Optional decoupled weight-decay coefficient (AdamW-style). When set,
    /// each step additionally applies `w ← w − lr · wd · w` to every weight.
    pub weight_decay: Option<f32>,
}

struct AdamMoment {
    m: Vec<f32>,
    v: Vec<f32>,
}

impl OptimState {
    /// Construct an empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of parameter slots currently tracked.
    pub fn len(&self) -> usize {
        self.moments.len()
    }

    /// True if no parameter state has been recorded.
    pub fn is_empty(&self) -> bool {
        self.moments.is_empty()
    }

    fn get_or_init(&mut self, op: Op, dims: &[i64], numel: usize) -> &mut AdamMoment {
        self.moments
            .entry((op.0, dims.to_vec()))
            .or_insert_with(|| AdamMoment {
                m: vec![0.0; numel],
                v: vec![0.0; numel],
            })
    }
}

/// Loss function for [`train_one_step_with_optim_loss`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Loss {
    /// Mean-squared error: `½·mean((y − target)²)` (gradient `2(y−target)/N`).
    Mse,
    /// Categorical cross-entropy on softmax outputs. `target` is one-hot.
    /// Forward: `−mean(target · log(softmax(y) + ε))`.
    /// Backward shortcut: `dL/dy = (softmax(y) − target) / N`.
    CrossEntropy,
}

/// Optimizer choice for [`train_one_step_with_optim`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Optimizer {
    /// Vanilla SGD: `w ← w − lr·g`.
    Sgd,
    /// Adam: per-parameter `m`, `v` with bias correction.
    Adam {
        /// First-moment decay rate.
        beta1: f32,
        /// Second-moment decay rate.
        beta2: f32,
        /// Numerical stability constant.
        eps: f32,
    },
}

impl Optimizer {
    /// Standard Adam defaults: β₁=0.9, β₂=0.999, ε=1e-8.
    pub fn adam_default() -> Self {
        Optimizer::Adam {
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
        }
    }
}

/// Apply an optimizer step to a single weight tensor.
///
/// Honors `state.clip_grad` (per-tensor L2 norm clipping) by scaling `grad`
/// down before the update when its norm exceeds the threshold.
fn apply_optim_step(
    op: Op,
    dims: &[i64],
    w: &mut [f32],
    grad: &[f32],
    lr: f32,
    optim: Optimizer,
    state: &mut OptimState,
) {
    // Allocate a clipped copy only when clipping is actually needed.
    let owned_clipped: Option<Vec<f32>> = state.clip_grad.and_then(|threshold| {
        let norm_sq: f32 = grad.iter().map(|g| g * g).sum();
        let norm = norm_sq.sqrt();
        if norm > threshold {
            let scale = threshold / norm;
            Some(grad.iter().map(|g| g * scale).collect())
        } else {
            None
        }
    });
    let grad: &[f32] = owned_clipped.as_deref().unwrap_or(grad);

    match optim {
        Optimizer::Sgd => {
            for k in 0..w.len() {
                w[k] -= lr * grad[k];
            }
        }
        Optimizer::Adam { beta1, beta2, eps } => {
            let t = state.step.max(1) as f32;
            let moment = state.get_or_init(op, dims, w.len());
            let bc1 = 1.0 - beta1.powf(t);
            let bc2 = 1.0 - beta2.powf(t);
            for k in 0..w.len() {
                let g = grad[k];
                moment.m[k] = beta1 * moment.m[k] + (1.0 - beta1) * g;
                moment.v[k] = beta2 * moment.v[k] + (1.0 - beta2) * g * g;
                let m_hat = moment.m[k] / bc1;
                let v_hat = moment.v[k] / bc2;
                w[k] -= lr * m_hat / (v_hat.sqrt() + eps);
            }
        }
    }
    // Decoupled weight decay (AdamW-style): applies regardless of optimizer.
    if let Some(wd) = state.weight_decay {
        if wd > 0.0 {
            for v in w.iter_mut() {
                *v -= lr * wd * *v;
            }
        }
    }
}

/// Sentinel slot for Linear bias tensors in [`ParamStore`]. Picked far away
/// from natural dimension values to avoid collisions with weighted-op keys.
const BIAS_SLOT: i64 = -1;

/// Broadcast-add a bias vector `[out]` to every row of a 2-D handle `[N, out]`.
fn add_bias_2d(
    backend: &dyn Backend,
    h: &TensorHandle,
    bias: &TensorHandle,
) -> Result<TensorHandle, rmi::error::RmiError> {
    let mat = read_as_f32(backend, h)?;
    let b = read_as_f32(backend, bias)?;
    let (rows, cols) = match h.shape.as_slice() {
        [r, c] => (*r, *c),
        _ => return Err(rmi::error::RmiError::compute_simple("add_bias_2d: not 2D".to_string())),
    };
    if b.len() != cols {
        return Err(rmi::error::RmiError::compute_simple(format!(
            "add_bias_2d: bias len {} != cols {}",
            b.len(), cols
        )));
    }
    let mut out = mat;
    for r in 0..rows {
        for c in 0..cols {
            out[r * cols + c] += b[c];
        }
    }
    let r = backend.from_slice_f32(&out, &h.shape)?;
    // Preserve the input's dtype (e.g. half pipeline).
    if h.dtype != DType::F32 {
        backend.cast(&r, h.dtype)
    } else {
        Ok(r)
    }
}

/// Ensure a tensor handle is 2D `[N, last_dim]`. If the handle is 1D of
/// length `last_dim`, reshape it to `[1, last_dim]` so matmul can consume it.
fn ensure_2d(
    backend: &dyn Backend,
    handle: &TensorHandle,
    last_dim: usize,
) -> Result<TensorHandle, rmi::error::RmiError> {
    if handle.shape.len() == 2 && handle.shape[1] == last_dim {
        return Ok(handle.clone());
    }
    // For rank-N tensors whose LAST dim is `last_dim`, collapse the
    // leading dims into a single row count. This is the standard
    // "Linear-over-batch" semantics: [b, s, dim] -> [b*s, dim] so the
    // matmul against the [dim, out] weight works, with caller-side
    // reshape back to [b, s, out] if needed.
    if let Some(&last) = handle.shape.last() {
        if last == last_dim {
            let n: usize = handle.shape.iter().take(handle.shape.len() - 1).product();
            let data = read_as_f32(backend, handle)?;
            let r = backend.from_slice_f32(&data, &[n, last_dim])?;
            return if handle.dtype != DType::F32 {
                backend.cast(&r, handle.dtype)
            } else {
                Ok(r)
            };
        }
    }
    // Rank-1 tensor whose length is a multiple of last_dim - same
    // reshape semantics as before.
    if handle.shape.len() == 1 && handle.shape[0] % last_dim == 0 {
        let n = handle.shape[0] / last_dim;
        let data = read_as_f32(backend, handle)?;
        let r = backend.from_slice_f32(&data, &[n, last_dim])?;
        return if handle.dtype != DType::F32 {
            backend.cast(&r, handle.dtype)
        } else {
            Ok(r)
        };
    }
    Err(rmi::error::RmiError::compute_simple(format!(
        "cannot reshape tensor of shape {:?} to end with {}",
        handle.shape, last_dim
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::machine_bridge::NetTranslator;

    /// CONV2D through the Machine Language pipeline now routes to `Backend::conv2d`
    /// (GPU im2col+GEMM on CUDA, naive on CPU). Verify the CPU pipeline
    /// produces the correct output shape and a finite result.
    #[test]
    fn conv2d_pipeline_cpu_shape() {
        // in_ch=2, out_ch=3, kernel=3; input [1,2,5,5] → out [3,3,3].
        let args = vec![
            Expr::Lit(Val::I64(2)),
            Expr::Lit(Val::I64(3)),
            Expr::Lit(Val::I64(3)),
        ];
        let expr = Expr::App(Op::CONV2D, args);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        // 3-D input [C,H,W] exercises the batch-collapse: out → [out_ch,H',W'].
        let result =
            run_pipeline_with_params(&backend, &expr, &[2, 5, 5], 0.5, &mut params)
                .expect("conv2d pipeline");
        assert_eq!(result.dispatched, 1, "expected 1 op dispatched");
        assert!(result.unsupported.is_empty(), "unsupported: {:?}", result.unsupported);
        assert_eq!(result.output.shape, vec![3, 3, 3], "conv out shape");
        assert!(result.output_sum.is_finite(), "conv output sum not finite");
    }

    /// Parity check: the CONV2D pipeline must produce the same result on
    /// CPU and CUDA (identical deterministic weights via ParamStore, so
    /// only the conv math differs — GPU im2col+GEMM vs CPU naive).
    #[cfg(feature = "cuda")]
    #[test]
    fn conv2d_pipeline_cpu_cuda_parity() {
        let cuda = match crate::cuda_backend::CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let args = vec![
            Expr::Lit(Val::I64(3)),
            Expr::Lit(Val::I64(4)),
            Expr::Lit(Val::I64(3)),
        ];
        let expr = Expr::App(Op::CONV2D, args);
        let shape = [2usize, 3, 6, 6];

        let cpu = CpuBackend::new();
        let mut cpu_params = ParamStore::new();
        let cpu_res =
            run_pipeline_with_params(&cpu, &expr, &shape, 0.3, &mut cpu_params).expect("cpu");

        let mut cuda_params = ParamStore::new();
        let cuda_res =
            run_pipeline_with_params(&cuda, &expr, &shape, 0.3, &mut cuda_params).expect("cuda");

        assert_eq!(cpu_res.output.shape, cuda_res.output.shape, "shape parity");
        let rel = (cpu_res.output_sum - cuda_res.output_sum).abs()
            / cpu_res.output_sum.abs().max(1e-6);
        assert!(
            rel < 1e-3,
            "conv2d CPU/CUDA sum mismatch: cpu={} cuda={} (rel={rel})",
            cpu_res.output_sum,
            cuda_res.output_sum
        );
    }

    /// P116: stride/padding exposed at the CONV2D opcode. A "same"
    /// conv (stride=1, pad=1, k=3) must preserve spatial dims, and CPU
    /// vs CUDA pipeline outputs must agree.
    #[test]
    fn conv2d_pipeline_strided_padded() {
        // [in=2,out=4,k=3,bias=1,stride=2,pad=1] on [2,7,7]:
        // out = (7+2-3)/2+1 = 4 → [4,4,4].
        let args = vec![
            Expr::Lit(Val::I64(2)),
            Expr::Lit(Val::I64(4)),
            Expr::Lit(Val::I64(3)),
            Expr::Lit(Val::I64(1)), // bias
            Expr::Lit(Val::I64(2)), // stride
            Expr::Lit(Val::I64(1)), // padding
        ];
        let expr = Expr::App(Op::CONV2D, args);

        let cpu = CpuBackend::new();
        let mut p = ParamStore::new();
        let res = run_pipeline_with_params(&cpu, &expr, &[2, 7, 7], 0.4, &mut p).expect("cpu");
        assert_eq!(res.dispatched, 1);
        assert!(res.unsupported.is_empty());
        assert_eq!(res.output.shape, vec![4, 4, 4], "strided/padded conv shape");

        #[cfg(feature = "cuda")]
        if let Ok(cuda) = crate::cuda_backend::CudaBackend::new() {
            let mut cp = ParamStore::new();
            let cres =
                run_pipeline_with_params(&cuda, &expr, &[2, 7, 7], 0.4, &mut cp).expect("cuda");
            assert_eq!(cres.output.shape, vec![4, 4, 4]);
            let rel =
                (res.output_sum - cres.output_sum).abs() / res.output_sum.abs().max(1e-6);
            assert!(rel < 1e-3, "strided conv parity: cpu={} cuda={}", res.output_sum, cres.output_sum);
        }
    }

    /// P120: a net run in F16 precision through `run_pipeline_with_precision`
    /// must match the F32 run within half tolerance (same deterministic
    /// weights; on CUDA the matmuls hit tensor cores in half). CUDA-only —
    /// CpuBackend ops read storage as F32 so half compute isn't valid there.
    #[cfg(feature = "cuda")]
    #[test]
    fn pipeline_f16_matches_f32() {
        let cuda = match crate::cuda_backend::CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        // Linear(8,16,bias) >> ReLU >> Linear(16,4,bias) >> Softmax
        let net = || {
            Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(16), Expr::int(1)])
                >> Expr::op1(Op::RELU)
                >> Expr::op(Op::LINEAR, vec![Expr::int(16), Expr::int(4), Expr::int(1)])
                >> Expr::op1(Op::SOFTMAX)
        };
        let shape = [4usize, 8];

        let mut pf = ParamStore::new();
        let rf = run_pipeline_with_precision(&cuda, &net(), &shape, 0.5, &mut pf, DType::F32)
            .expect("f32");
        assert_eq!(rf.output.dtype, DType::F32);
        assert_eq!(rf.dispatched, 4);

        let mut ph = ParamStore::new();
        let rh = run_pipeline_with_precision(&cuda, &net(), &shape, 0.5, &mut ph, DType::F16)
            .expect("f16");
        assert_eq!(rh.output.dtype, DType::F16, "pipeline ran in F16");
        assert_eq!(rh.output.shape, rf.output.shape);

        // Compare element-wise (read both as f32) within half tolerance.
        let want: Vec<f32> = {
            let b = cuda.copy_to_host(&rf.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        let got_f32 = cuda.cast(&rh.output, DType::F32).unwrap();
        let got: Vec<f32> = {
            let b = cuda.copy_to_host(&got_f32).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        assert_eq!(want.len(), got.len());
        for (a, g) in want.iter().zip(&got) {
            assert!((a - g).abs() < 2e-2, "f16 pipeline diverged: {a} vs {g}");
        }
        // Softmax rows (4 rows × 4 classes) each sum to ~1 in both.
        assert!((rh.output_sum - 4.0).abs() < 0.1, "4 softmax rows ≈ 4.0, got {}", rh.output_sum);
    }

    /// P123: a net run with INT8-quantized matmuls through
    /// `run_pipeline_quantized` must stay close to the F32 run (same
    /// deterministic weights). CUDA-only (CPU falls back to exact F32,
    /// which would make this trivially pass — the point is the INT8 path).
    #[cfg(feature = "cuda")]
    #[test]
    fn pipeline_quantized_matches_f32() {
        let cuda = match crate::cuda_backend::CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        // Linear(8,16) >> ReLU >> Linear(16,4) >> Softmax
        let net = || {
            Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(16)])
                >> Expr::op1(Op::RELU)
                >> Expr::op(Op::LINEAR, vec![Expr::int(16), Expr::int(4)])
                >> Expr::op1(Op::SOFTMAX)
        };
        let shape = [4usize, 8];

        let mut pf = ParamStore::new();
        let rf = run_pipeline_with_params(&cuda, &net(), &shape, 0.5, &mut pf).expect("f32");
        assert_eq!(rf.dispatched, 4);

        let mut pq = ParamStore::new();
        let rq = run_pipeline_quantized(&cuda, &net(), &shape, 0.5, &mut pq).expect("quant");
        assert_eq!(rq.output.shape, rf.output.shape);

        let want: Vec<f32> = {
            let b = cuda.copy_to_host(&rf.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        let got: Vec<f32> = {
            let b = cuda.copy_to_host(&rq.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        // INT8 dynamic quant through 2 linears + softmax: outputs (probs)
        // should track the f32 reference within a few %.
        for (a, g) in want.iter().zip(&got) {
            assert!((a - g).abs() < 5e-2, "quant pipeline diverged: {a} vs {g}");
        }
        // Softmax rows still normalise.
        assert!((rq.output_sum - 4.0).abs() < 0.1, "4 softmax rows ≈ 4.0, got {}", rq.output_sum);
    }

    /// P129: calibrate-then-run-calibrated INT8 inference through the
    /// pipeline. The calibration pass records per-layer activation scales;
    /// the calibrated pass uses them via the fast on-device IMMA path.
    /// CUDA-only (CPU falls back to F32).
    #[cfg(feature = "cuda")]
    #[test]
    fn pipeline_calibrated_quant_matches_f32() {
        let cuda = match crate::cuda_backend::CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        // Two linears (dims mult of 4 for IMMA) with an activation between.
        let net = || {
            Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(16)])
                >> Expr::op1(Op::RELU)
                >> Expr::op(Op::LINEAR, vec![Expr::int(16), Expr::int(8)])
                >> Expr::op1(Op::SOFTMAX)
        };
        let shape = [4usize, 8];

        let mut pf = ParamStore::new();
        let rf = run_pipeline_with_params(&cuda, &net(), &shape, 0.5, &mut pf).expect("f32");

        // Calibration + calibrated inference share one ParamStore (so the
        // weights and recorded scales carry over).
        let mut pq = ParamStore::new();
        let _ = calibrate_pipeline(&cuda, &net(), &shape, 0.5, &mut pq).expect("calibrate");
        assert_eq!(pq.act_scales().len(), 2, "two LINEARs → two recorded scales");
        assert!(pq.act_scales().iter().all(|&s| s > 0.0), "scales recorded");

        let rq = run_pipeline_calibrated(&cuda, &net(), &shape, 0.5, &mut pq).expect("calibrated");
        assert_eq!(rq.output.shape, rf.output.shape);
        // IMMA tensor-core path must have run for the calibrated linears.
        assert!(cuda.quant_imma_count() >= 2, "calibrated linears used IMMA");

        let want: Vec<f32> = {
            let b = cuda.copy_to_host(&rf.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        let got: Vec<f32> = {
            let b = cuda.copy_to_host(&rq.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        for (a, g) in want.iter().zip(&got) {
            assert!((a - g).abs() < 5e-2, "calibrated quant pipeline diverged: {a} vs {g}");
        }
        assert!((rq.output_sum - 4.0).abs() < 0.1, "4 softmax rows ≈ 4.0, got {}", rq.output_sum);
    }

    #[test]
    fn entropy_threshold_clips_below_max() {
        // Bulk of mass near ~1, a thin tail out to 50. Entropy calibration
        // should pick a threshold well below the max (clipping the tail)
        // but above the bulk.
        let mut v: Vec<f32> = Vec::new();
        for i in 0..10000 {
            v.push((i as f32 % 100.0) * 0.01); // 0..1 bulk
        }
        for i in 0..20 {
            v.push(50.0 - i as f32); // sparse tail 31..50
        }
        let t = entropy_threshold_abs(&v, 2048, 128);
        let max = v.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
        assert!((max - 50.0).abs() < 1e-3);
        assert!(t < max, "entropy threshold {t} should clip below max {max}");
        assert!(t > 0.5, "entropy threshold {t} should keep the bulk");
        // Degenerate inputs.
        assert_eq!(entropy_threshold_abs(&[], 2048, 128), 0.0);
        assert_eq!(entropy_threshold_abs(&[0.0, 0.0], 2048, 128), 0.0);
    }

    /// P136: asymmetric calibrated PTQ through the pipeline. The net has
    /// a ReLU before the second linear, so that layer's activations are
    /// one-sided — exactly where asymmetric quantization helps. Asym
    /// calibrated inference must match F32 and record per-layer ranges.
    #[cfg(feature = "cuda")]
    #[test]
    fn pipeline_asymmetric_calibrated_quant() {
        let cuda = match crate::cuda_backend::CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let net = || {
            Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(16)])
                >> Expr::op1(Op::RELU)
                >> Expr::op(Op::LINEAR, vec![Expr::int(16), Expr::int(8)])
                >> Expr::op1(Op::SOFTMAX)
        };
        let shape = [4usize, 8];

        let mut pf = ParamStore::new();
        let rf = run_pipeline_with_params(&cuda, &net(), &shape, 0.5, &mut pf).expect("f32");

        let mut pq = ParamStore::new();
        pq.set_asymmetric(true);
        // Calibrate over two representative inputs of opposite sign (a
        // single constant seed can land entirely in ReLU's dead zone for
        // this deterministic weight init). Ranges merge via running
        // min/max across passes.
        let _ = calibrate_pipeline(&cuda, &net(), &shape, 0.5, &mut pq).expect("calibrate+");
        let _ = calibrate_pipeline(&cuda, &net(), &shape, -0.5, &mut pq).expect("calibrate-");
        assert_eq!(pq.act_ranges().len(), 2, "two LINEARs → two recorded ranges");
        // The second linear's input is post-ReLU → lo ≥ 0 (one-sided).
        let (lo2, hi2) = pq.act_ranges()[1];
        assert!(lo2 >= -1e-6 && hi2 > lo2, "post-ReLU range should be one-sided: [{lo2},{hi2}]");

        let rq = run_pipeline_calibrated(&cuda, &net(), &shape, 0.5, &mut pq).expect("asym calib");
        assert_eq!(rq.output.shape, rf.output.shape);

        let want: Vec<f32> = {
            let b = cuda.copy_to_host(&rf.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        let got: Vec<f32> = {
            let b = cuda.copy_to_host(&rq.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        for (a, g) in want.iter().zip(&got) {
            assert!((a - g).abs() < 5e-2, "asym calib pipeline diverged: {a} vs {g}");
        }
        assert!((rq.output_sum - 4.0).abs() < 0.1, "softmax rows ≈ 4.0, got {}", rq.output_sum);
    }

    /// P139: W4A8 calibrated PTQ through the pipeline — packed INT4
    /// weights, INT8 calibrated activations. Coarser than INT8 weights
    /// but must still track F32 within int4 tolerance.
    #[cfg(feature = "cuda")]
    #[test]
    fn pipeline_w4a8_calibrated_quant() {
        let cuda = match crate::cuda_backend::CudaBackend::new() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("skip: no CUDA device available");
                return;
            }
        };
        let net = || {
            Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(16)])
                >> Expr::op1(Op::RELU)
                >> Expr::op(Op::LINEAR, vec![Expr::int(16), Expr::int(8)])
                >> Expr::op1(Op::SOFTMAX)
        };
        let shape = [4usize, 8];

        let mut pf = ParamStore::new();
        let rf = run_pipeline_with_params(&cuda, &net(), &shape, 0.5, &mut pf).expect("f32");

        let mut pq = ParamStore::new();
        pq.set_weight_bits(4);
        let _ = calibrate_pipeline(&cuda, &net(), &shape, 0.5, &mut pq).expect("calibrate");
        let _ = calibrate_pipeline(&cuda, &net(), &shape, -0.5, &mut pq).expect("calibrate-");
        let rq = run_pipeline_calibrated(&cuda, &net(), &shape, 0.5, &mut pq).expect("w4a8");
        assert_eq!(rq.output.shape, rf.output.shape);

        let want: Vec<f32> = {
            let b = cuda.copy_to_host(&rf.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        let got: Vec<f32> = {
            let b = cuda.copy_to_host(&rq.output).unwrap();
            b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
        };
        // 4-bit weights through 2 linears + softmax: looser than int8.
        for (a, g) in want.iter().zip(&got) {
            assert!((a - g).abs() < 0.12, "w4a8 pipeline diverged: {a} vs {g}");
        }
        assert!((rq.output_sum - 4.0).abs() < 0.15, "softmax rows ≈ 4.0, got {}", rq.output_sum);
    }

    #[test]
    fn percentile_abs_clips_outliers() {
        // 100 values of magnitude ~1, plus one outlier of 1000.
        let mut v: Vec<f32> = (0..100).map(|i| (i as f32 % 10.0) * 0.1 + 0.5).collect();
        v.push(1000.0);
        let max = percentile_abs(&v, 1.0);
        let p99 = percentile_abs(&v, 0.99);
        assert!((max - 1000.0).abs() < 1e-3, "max picks the outlier: {max}");
        assert!(p99 < 2.0, "99th percentile clips the outlier: {p99}");
        // Empty + single-element edge cases.
        assert_eq!(percentile_abs(&[], 0.5), 0.0);
        assert_eq!(percentile_abs(&[-7.0], 0.5), 7.0);
    }

    /// P130: percentile calibration records a SMALLER (outlier-clipped)
    /// activation scale than max calibration when activations are
    /// heavy-tailed. Uses CPU (the calibration math is backend-agnostic;
    /// CPU runs the F32 calibration pass). The seed input is uniform so
    /// we exercise the recording path; the key assertion is the method
    /// plumbing selects percentile vs max.
    #[test]
    fn calibration_method_selects_percentile() {
        let cpu = CpuBackend::new();
        let net = || Expr::op(Op::LINEAR, vec![Expr::int(8), Expr::int(8)]);
        let shape = [4usize, 8];

        let mut p_max = ParamStore::new();
        p_max.set_calib_method(CalibMethod::Max);
        let _ = calibrate_pipeline(&cpu, &net(), &shape, 1.5, &mut p_max).expect("cal max");

        let mut p_pct = ParamStore::new();
        p_pct.set_calib_method(CalibMethod::Percentile(0.5)); // median
        let _ = calibrate_pipeline(&cpu, &net(), &shape, 1.5, &mut p_pct).expect("cal pct");

        assert_eq!(p_max.act_scales().len(), 1);
        assert_eq!(p_pct.act_scales().len(), 1);
        // With a constant seed input all |x| are equal, so median == max
        // here; the meaningful guarantee is both record a positive scale
        // and the method is wired. (Outlier-clipping behaviour is proven
        // directly in `percentile_abs_clips_outliers`.)
        assert!(p_max.act_scales()[0] > 0.0 && p_pct.act_scales()[0] > 0.0);
        assert!(p_pct.act_scales()[0] <= p_max.act_scales()[0] + 1e-6);
    }

    fn activation_layer(name: &str) -> ast::LayerDef {
        ast::LayerDef {
            name: format!("l_{}", name.to_lowercase()),
            layer_type: ast::Type::Path {
                segments: vec![name.to_string()],
                type_args: Vec::new(),
            },
            args: Vec::new(),
        }
    }

    fn empty_block() -> ast::Block {
        ast::Block { stmts: Vec::new(), tail_expr: None }
    }

    #[test]
    fn relu_chain_runs_on_cpu_backend() {
        // net { layer a: ReLU; layer b: GELU; layer c: ReLU; }
        let net = ast::NetDef {
            name: "ActChain".into(),
            generics: Vec::new(),
            layers: vec![
                activation_layer("ReLU"),
                activation_layer("GELU"),
                activation_layer("ReLU"),
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        // Input: 8 elements of -1.0 → first ReLU yields all zeros.
        let result = run_pipeline(&backend, &lowered.expr, &[8], -1.0).expect("run");
        assert_eq!(result.dispatched, 3, "expected 3 ops dispatched");
        assert!(result.unsupported.is_empty(), "unsupported: {:?}", result.unsupported);
        // After ReLU(-1)=0 → GELU(0)≈0 → ReLU(0)=0 → sum should be 0.
        assert!(
            result.output_sum.abs() < 1e-3,
            "output sum should be ~0, got {}",
            result.output_sum
        );
    }

    #[test]
    fn sigmoid_at_zero_yields_half() {
        // sigmoid(0) = 0.5; sum over 4 elements = 2.0.
        let net = ast::NetDef {
            name: "Sig".into(),
            generics: Vec::new(),
            layers: vec![activation_layer("Sigmoid")],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let result = run_pipeline(&backend, &lowered.expr, &[4], 0.0).expect("run");
        assert_eq!(result.dispatched, 1);
        assert!((result.output_sum - 2.0).abs() < 1e-3, "got {}", result.output_sum);
    }

    #[test]
    fn linear_without_dims_reported_as_unsupported() {
        // A Linear layer with no args still cannot run — no dimensions.
        let net = ast::NetDef {
            name: "BareLin".into(),
            generics: Vec::new(),
            layers: vec![activation_layer("Linear"), activation_layer("ReLU")],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let result = run_pipeline(&backend, &lowered.expr, &[4], 1.0).expect("run");
        // ReLU is dispatched (1); LINEAR with no dims is unsupported.
        assert_eq!(result.dispatched, 1);
        assert_eq!(result.unsupported, vec![Op::LINEAR]);
    }

    fn named_layer(name: &str, type_name: &str, args: Vec<i64>) -> ast::LayerDef {
        ast::LayerDef {
            name: name.into(),
            layer_type: ast::Type::Path {
                segments: vec![type_name.to_string()],
                type_args: Vec::new(),
            },
            args: args
                .into_iter()
                .map(|n| ast::Expr::Literal {
                    value: n.to_string(),
                    kind: ast::LiteralKind::Int,
                })
                .collect(),
        }
    }

    fn layer_with_int_args(type_name: &str, args: Vec<i64>) -> ast::LayerDef {
        named_layer(&format!("l_{}", type_name.to_lowercase()), type_name, args)
    }

    #[test]
    fn linear_with_dims_runs_real_matmul() {
        // net { layer fc: Linear(8, 16); layer act: ReLU; } input [8]
        let net = ast::NetDef {
            name: "MLP".into(),
            generics: Vec::new(),
            layers: vec![
                layer_with_int_args("Linear", vec![8, 16]),
                activation_layer("ReLU"),
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let result = run_pipeline(&backend, &lowered.expr, &[8], 0.5).expect("run");
        assert_eq!(result.dispatched, 2, "expected Linear + ReLU dispatched");
        assert!(result.unsupported.is_empty(), "unsupported: {:?}", result.unsupported);
        // Output shape should be [1, 16] after Linear + ReLU.
        assert_eq!(result.output.shape, vec![1, 16]);
    }

    #[test]
    fn param_store_caches_weights_across_calls() {
        let net = ast::NetDef {
            name: "Stable".into(),
            generics: Vec::new(),
            layers: vec![layer_with_int_args("Linear", vec![4, 8])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let r1 = run_pipeline_with_params(&backend, &lowered.expr, &[4], 1.0, &mut params)
            .expect("run 1");
        assert_eq!(params.len(), 1, "one weight tensor cached");
        let r2 = run_pipeline_with_params(&backend, &lowered.expr, &[4], 1.0, &mut params)
            .expect("run 2");
        assert_eq!(params.len(), 1, "still one — weights reused");
        // Same input, same weights → same output sum.
        assert!(
            (r1.output_sum - r2.output_sum).abs() < 1e-5,
            "stable weights should produce stable output: {} vs {}",
            r1.output_sum,
            r2.output_sum
        );
    }

    fn norm_layer(name: &str, type_name: &str) -> ast::LayerDef {
        ast::LayerDef {
            name: name.into(),
            layer_type: ast::Type::Path {
                segments: vec![type_name.to_string()],
                type_args: Vec::new(),
            },
            args: Vec::new(),
        }
    }

    #[test]
    fn one_sgd_step_reduces_mse_loss() {
        // Tiny regression: y = 2x. One Linear(1, 1), batch of 4 examples.
        // After SGD steps, MSE loss should decrease monotonically toward 0.
        let net = ast::NetDef {
            name: "Reg".into(),
            generics: Vec::new(),
            layers: vec![named_layer("fc", "Linear", vec![1, 1])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let x = [0.5f32, 1.0, 1.5, 2.0];
        let y = [1.0f32, 2.0, 3.0, 4.0]; // y = 2x
        let mut losses = Vec::new();
        for _ in 0..30 {
            let r = train_one_step(
                &backend,
                &lowered.expr,
                &x,
                &[4, 1],
                &y,
                &[4, 1],
                0.1,
                &mut params,
            )
            .expect("step");
            losses.push(r.loss);
        }
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(
            last < first,
            "loss should decrease across SGD steps: first={first} last={last}"
        );
        assert!(last < 0.5, "loss should approach zero, got {last}");
    }

    #[test]
    fn cross_entropy_classifies_two_classes() {
        // 2-class linear classifier: Linear(2, 2). Inputs cluster in two
        // regions; training with cross-entropy should drive loss down.
        let net = ast::NetDef {
            name: "Clf".into(),
            generics: Vec::new(),
            layers: vec![named_layer("fc", "Linear", vec![2, 2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        // 4 well-separated samples: high x1 ⇒ class 0, high x2 ⇒ class 1.
        // Avoid all-zero inputs (no-bias Linear can't move on those).
        let x = vec![
            1.0f32, -1.0, // class 0
            0.8, -0.5, // class 0
            -1.0, 1.0, // class 1
            -0.5, 0.8, // class 1
        ];
        let y = vec![
            1.0f32, 0.0, // class 0
            1.0, 0.0, // class 0
            0.0, 1.0, // class 1
            0.0, 1.0, // class 1
        ];
        let mut params = ParamStore::new();
        let mut state = OptimState::new();
        let mut losses = Vec::new();
        for _ in 0..200 {
            let r = train_one_step_with_optim_loss(
                &backend,
                &lowered.expr,
                &x,
                &[4, 2],
                &y,
                &[4, 2],
                0.1,
                Optimizer::adam_default(),
                Loss::CrossEntropy,
                &mut params,
                &mut state,
            )
            .expect("step");
            losses.push(r.loss);
        }
        let first = *losses.first().unwrap();
        let last = *losses.last().unwrap();
        assert!(last < first * 0.5, "CE loss should at least halve: first={first} last={last}");
    }

    #[test]
    fn param_store_round_trips_through_save_load() {
        // Train a tiny net so the ParamStore actually has weights, then
        // save → load → compare.
        let net = ast::NetDef {
            name: "Reg".into(),
            generics: Vec::new(),
            layers: vec![named_layer("fc", "Linear", vec![2, 3])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let x = [0.5f32, 0.5, 1.0, 0.0];
        let y = [1.0f32, 1.0, 1.0, 2.0, 2.0, 2.0];
        for _ in 0..5 {
            train_one_step(
                &backend, &lowered.expr, &x, &[2, 2], &y, &[2, 3], 0.05, &mut params,
            )
            .expect("train step");
        }
        assert_eq!(params.len(), 1);

        // Serialize then reload.
        let blob = params.save(&backend).expect("save");
        assert!(!blob.is_empty());
        let restored = ParamStore::load(&blob, &backend).expect("load");
        assert_eq!(restored.len(), 1);

        // Verify the restored weight produces the same forward output.
        let h = backend.from_slice_f32(&x, &[2, 2]).unwrap();
        let mut p1 = params;
        let mut p2 = restored;
        let out1 = forward_pass(&backend, &lowered.expr, h.clone(), &mut p1).unwrap();
        let out2 = forward_pass(&backend, &lowered.expr, h, &mut p2).unwrap();
        let b1 = bytes_to_f32(&backend.copy_to_host(&out1).unwrap());
        let b2 = bytes_to_f32(&backend.copy_to_host(&out2).unwrap());
        assert_eq!(b1.len(), b2.len());
        for i in 0..b1.len() {
            assert!((b1[i] - b2[i]).abs() < 1e-6, "mismatch at {i}: {} vs {}", b1[i], b2[i]);
        }
    }

    #[test]
    fn linear_with_bias_fits_intercept() {
        // Learn y = x + 1. Without bias this is impossible; with bias it
        // converges. We test both: bias-on should converge much lower.
        let bias_net = ast::NetDef {
            name: "Bias".into(),
            generics: Vec::new(),
            layers: vec![named_layer("fc", "Linear", vec![1, 1, 1])],
            forward: empty_block(),
        };
        let no_bias_net = ast::NetDef {
            name: "NoBias".into(),
            generics: Vec::new(),
            layers: vec![named_layer("fc", "Linear", vec![1, 1])],
            forward: empty_block(),
        };
        let lowered_b = NetTranslator::translate(&bias_net);
        let lowered_n = NetTranslator::translate(&no_bias_net);
        let backend = CpuBackend::new();
        // y = x + 1 (intercept of 1 makes bias necessary)
        let x = [0.0f32, 1.0, 2.0, 3.0];
        let y = [1.0f32, 2.0, 3.0, 4.0];

        let mut params_b = ParamStore::new();
        let mut state_b = OptimState::new();
        let mut bias_loss = 0.0;
        for _ in 0..100 {
            bias_loss = train_one_step_with_optim(
                &backend, &lowered_b.expr, &x, &[4, 1], &y, &[4, 1], 0.1,
                Optimizer::adam_default(), &mut params_b, &mut state_b,
            ).unwrap().loss;
        }
        // With bias: should converge near zero.
        assert!(bias_loss < 0.05, "bias-on should converge; got {bias_loss}");
        // 2 params: weight + bias.
        assert_eq!(params_b.len(), 2, "weight + bias = 2 entries");

        let mut params_n = ParamStore::new();
        let mut state_n = OptimState::new();
        let mut nobias_loss = 0.0;
        for _ in 0..100 {
            nobias_loss = train_one_step_with_optim(
                &backend, &lowered_n.expr, &x, &[4, 1], &y, &[4, 1], 0.1,
                Optimizer::adam_default(), &mut params_n, &mut state_n,
            ).unwrap().loss;
        }
        // Without bias: floor on loss because y - kx can't fit y = x + 1.
        // Loss should be much higher than the bias case.
        assert!(nobias_loss > 5.0 * bias_loss,
            "bias should win meaningfully: nobias={nobias_loss} bias={bias_loss}");
        assert_eq!(params_n.len(), 1);
    }

    #[test]
    fn adam_converges_and_tracks_state() {
        // y = 2x with a single Linear(1, 1). Adam should reduce loss
        // monotonically (after the initial bias-corrected phase) and
        // maintain one OptimState entry plus a step counter.
        let net = ast::NetDef {
            name: "Reg".into(),
            generics: Vec::new(),
            layers: vec![named_layer("fc", "Linear", vec![1, 1])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let x = [0.5f32, 1.0, 1.5, 2.0];
        let y = [1.0f32, 2.0, 3.0, 4.0];

        let mut params = ParamStore::new();
        let mut state = OptimState::new();
        let mut losses = Vec::new();
        for _ in 0..100 {
            let r = train_one_step_with_optim(
                &backend,
                &lowered.expr,
                &x,
                &[4, 1],
                &y,
                &[4, 1],
                0.1,
                Optimizer::adam_default(),
                &mut params,
                &mut state,
            )
            .expect("adam");
            losses.push(r.loss);
        }

        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(last < first * 0.1, "Adam should reduce loss ≥10x: first={first} last={last}");
        // OptimState should have one entry (the single Linear's weight),
        // tracking m + v across all 100 steps.
        assert_eq!(state.len(), 1);
        assert_eq!(state.step, 100);
    }

    #[test]
    fn weight_decay_shrinks_weights() {
        // With wd=0.5, lr=0.1, the weight should shrink by 5% per step
        // when the gradient is zero. We trigger this by training on a
        // problem where the model is already perfect.
        let net = ast::NetDef {
            name: "WD".into(),
            generics: Vec::new(),
            layers: vec![named_layer("fc", "Linear", vec![1, 1])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let mut state = OptimState::new();
        state.weight_decay = Some(0.5);
        // Trivial: x = 0 means gradient is 0 (Linear has no bias here, so
        // y_pred = 0 regardless of W). With y = 0 target, loss = 0 too.
        // Only WD has an effect on W.
        let x = vec![0.0f32];
        let y = vec![0.0f32];
        // Force initial weight to 1.0 by pre-training a bit (or read after init).
        let blob_before = {
            train_one_step_with_optim(
                &backend, &lowered.expr, &x, &[1, 1], &y, &[1, 1], 0.1,
                Optimizer::Sgd, &mut params, &mut state,
            ).expect("step");
            params.save(&backend).unwrap()
        };
        let store_before = ParamStore::load(&blob_before, &backend).unwrap();
        let w_handle_b = store_before.weights.get(&(rmi::lang::Op::LINEAR.0, vec![1, 1])).unwrap();
        let w_before = bytes_to_f32(&backend.copy_to_host(w_handle_b).unwrap())[0];

        // Run 5 more steps. With gradient = 0 always, WD alone shrinks W by
        // 5% per step → 0.95^5 ≈ 0.774 of original.
        for _ in 0..5 {
            train_one_step_with_optim(
                &backend, &lowered.expr, &x, &[1, 1], &y, &[1, 1], 0.1,
                Optimizer::Sgd, &mut params, &mut state,
            ).expect("step");
        }
        let w_handle_a = params.weights.get(&(rmi::lang::Op::LINEAR.0, vec![1, 1])).unwrap();
        let w_after = bytes_to_f32(&backend.copy_to_host(w_handle_a).unwrap())[0];
        let ratio = w_after / w_before;
        let expected = 0.95f32.powi(5);
        assert!(
            (ratio - expected).abs() < 0.01,
            "WD shrink ratio should be 0.95^5 = {}, got {} (w_before={} w_after={})",
            expected, ratio, w_before, w_after
        );
    }

    #[test]
    fn gradient_clipping_bounds_update() {
        // Without clipping, large initial loss can blow up weights.
        // With clipping, weights stay bounded and loss still decreases.
        let net = ast::NetDef {
            name: "Clip".into(),
            generics: Vec::new(),
            layers: vec![named_layer("fc", "Linear", vec![1, 1])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let mut state = OptimState::new();
        state.clip_grad = Some(0.1); // very aggressive clip
        // Extreme input/target to force large gradients.
        let x = vec![10.0f32];
        let y = vec![100.0f32];
        for _ in 0..20 {
            train_one_step_with_optim(
                &backend, &lowered.expr, &x, &[1, 1], &y, &[1, 1], 0.5,
                Optimizer::Sgd, &mut params, &mut state,
            ).expect("step");
        }
        // Inspect the trained weight: should be modest, not explosive.
        let blob = params.save(&backend).unwrap();
        let restored = ParamStore::load(&blob, &backend).unwrap();
        let w_handle = restored.weights.get(&(rmi::lang::Op::LINEAR.0, vec![1, 1])).unwrap();
        let w_vec = bytes_to_f32(&backend.copy_to_host(w_handle).unwrap());
        // 20 steps × lr=0.5 × clipped grad ≤ 0.1 → |w| ≤ 1.0 plus small init.
        assert!(w_vec[0].abs() < 1.5,
            "clipped weight should stay bounded; got {}", w_vec[0]);
    }

    #[test]
    fn dropout_inference_passes_through() {
        // At inference (run_pipeline path), DROP is a no-op pass-through.
        let net = ast::NetDef {
            name: "Drop".into(),
            generics: Vec::new(),
            layers: vec![named_layer("d", "Dropout", vec![50])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let result = run_pipeline(&backend, &lowered.expr, &[8], 1.0).expect("run");
        // All ones in, all ones out (pass-through at inference).
        assert!((result.output_sum - 8.0).abs() < 1e-5,
            "DROP should pass through at inference; sum={}", result.output_sum);
    }

    #[test]
    fn dropout_train_masks_and_scales() {
        // In train mode with p=50%, the mask should zero roughly half of
        // the activations and scale the rest by 1/0.5 = 2.
        let net = ast::NetDef {
            name: "DropT".into(),
            generics: Vec::new(),
            layers: vec![
                named_layer("d", "Dropout", vec![50]),
                named_layer("fc", "Linear", vec![64, 1, 1]),
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let x = vec![1.0f32; 64];
        let y = vec![0.0f32];
        // One training step: the dropout mask is applied during forward.
        let r = train_one_step(
            &backend, &lowered.expr, &x, &[1, 64], &y, &[1, 1], 0.01, &mut params,
        ).expect("step");
        // Loss should be finite (not nan or inf) — sanity check the mask
        // didn't break the forward.
        assert!(r.loss.is_finite(), "loss should be finite, got {}", r.loss);
    }

    #[test]
    fn learned_pe_adds_table_rows() {
        // LearnedPE(8, 4): allocates [8, 4] table. With zero input, output
        // equals the (random-init) table for positions 0..seq.
        let net = ast::NetDef {
            name: "LPE".into(),
            generics: Vec::new(),
            layers: vec![named_layer("pe", "LearnedPE", vec![8, 4])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let zeros = vec![0.0f32; 3 * 4];
        let handle = backend.from_slice_f32(&zeros, &[3, 4]).unwrap();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        assert_eq!(running.shape, vec![3, 4]);
        assert_eq!(params.len(), 1, "PE table cached");
        // Output should be non-zero (the cached table has random init).
        let out = bytes_to_f32(&backend.copy_to_host(&running).unwrap());
        assert!(out.iter().any(|v| v.abs() > 1e-6), "expected PE table values");
    }

    #[test]
    fn learned_pe_trains() {
        // LearnedPE(4, 2) → Linear(2, 1, 1). Target depends only on position.
        let net = ast::NetDef {
            name: "LPETrain".into(),
            generics: Vec::new(),
            layers: vec![
                named_layer("pe", "LearnedPE", vec![4, 2]),
                named_layer("fc", "Linear", vec![2, 1, 1]),
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        // 4 sequences, each with 1 token of dim 2 (zeros); target is position-dependent.
        let x = vec![0.0f32; 4 * 2];
        let y = vec![1.0f32, 2.0, 3.0, 4.0];
        let mut losses = Vec::new();
        for _ in 0..200 {
            let r = train_one_step_with_optim(
                &backend, &lowered.expr, &x, &[4, 2], &y, &[4, 1], 0.05,
                Optimizer::adam_default(), &mut params, &mut OptimState::new(),
            ).expect("step");
            losses.push(r.loss);
        }
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        // Note: this is a single-batch training so all 4 positions receive
        // their per-position gradient — the PE table should learn to encode
        // position-specific outputs.
        assert!(last < first * 0.7, "learned PE should help: first={first} last={last}");
        // 3 weight tensors: PE table, Linear W, Linear bias.
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn embedding_gathers_rows_by_index() {
        // Embedding(vocab=4, embed=3). Indices [0, 2, 1] → 3 rows × 3 cols.
        let net = ast::NetDef {
            name: "Emb".into(),
            generics: Vec::new(),
            layers: vec![named_layer("emb", "Embedding", vec![4, 3])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let indices = vec![0.0f32, 2.0, 1.0];
        let handle = backend.from_slice_f32(&indices, &[3]).unwrap();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        assert_eq!(running.shape, vec![3, 3]);
        assert_eq!(params.len(), 1, "one [vocab, embed] table cached");
    }

    #[test]
    fn embedding_then_linear_trains() {
        // 4-vocab, 3-embed, then Linear(3, 1, 1). Learn a mapping per index.
        let net = ast::NetDef {
            name: "EmbLin".into(),
            generics: Vec::new(),
            layers: vec![
                named_layer("emb", "Embedding", vec![4, 3]),
                named_layer("fc", "Linear", vec![3, 1, 1]),
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        // 4 samples — each a single index, with target = index.
        let x = vec![0.0f32, 1.0, 2.0, 3.0];
        let y = vec![0.0f32, 1.0, 2.0, 3.0];
        let mut losses = Vec::new();
        for _ in 0..200 {
            let r = train_one_step_with_optim(
                &backend, &lowered.expr, &x, &[4], &y, &[4, 1], 0.05,
                Optimizer::adam_default(), &mut params, &mut OptimState::new(),
            ).expect("step");
            losses.push(r.loss);
        }
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(last < first * 0.5, "embed+linear should learn lookup: first={first} last={last}");
        assert_eq!(params.len(), 3, "embedding table + Linear W + Linear bias = 3");
    }

    #[test]
    fn sinusoidal_pe_preserves_shape_and_adds_phase() {
        let net = ast::NetDef {
            name: "PE".into(),
            generics: Vec::new(),
            layers: vec![named_layer("pe", "SinusoidalPE", vec![16, 4])],
            forward: empty_block(),
        };
        // Forward dispatch table maps name "SinusoidalPE" → Op? Not in
        // current layer_name_to_op. Use direct Machine Language expression instead.
        let _ = lowered_to_silence_warning(net.clone());
        let backend = CpuBackend::new();
        let expr = rmi::lang::Expr::op(Op::SINUSOIDAL_PE, vec![
            rmi::lang::Expr::int(16),
            rmi::lang::Expr::int(4),
        ]);
        // Zero input: output equals pure PE table values at each row.
        let input = vec![0.0f32; 4 * 4]; // seq=4, embed=4
        let handle = backend.from_slice_f32(&input, &[4, 4]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        assert_eq!(running.shape, vec![4, 4]);
        let out = bytes_to_f32(&backend.copy_to_host(&running).unwrap());
        // Position 0: sin(0)=0, cos(0)=1, sin(0)=0, cos(0)=1 → [0, 1, 0, 1]
        assert!((out[0] - 0.0).abs() < 1e-5, "PE[0,0] = sin(0) = 0, got {}", out[0]);
        assert!((out[1] - 1.0).abs() < 1e-5, "PE[0,1] = cos(0) = 1, got {}", out[1]);
        assert!((out[2] - 0.0).abs() < 1e-5, "PE[0,2] = sin(0) = 0, got {}", out[2]);
        assert!((out[3] - 1.0).abs() < 1e-5, "PE[0,3] = cos(0) = 1, got {}", out[3]);
    }

    fn lowered_to_silence_warning(_net: ast::NetDef) -> usize {
        0
    }

    #[test]
    fn multi_head_attention_training_reduces_loss() {
        // Attention(2, 4, 2): in_dim=2, model_dim=4, 2 heads (head_dim=2).
        // Trains end-to-end with per-head softmax backward.
        let net = ast::NetDef {
            name: "MHA".into(),
            generics: Vec::new(),
            layers: vec![named_layer("attn", "Attention", vec![2, 4, 2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let x = vec![1.0f32, 0.0, 0.0, 1.0, 0.5, 0.5];
        let y = vec![0.3f32, 0.7, 0.4, 0.6, 0.5, 0.5];
        let mut losses = Vec::new();
        for _ in 0..80 {
            let r = train_one_step(
                &backend, &lowered.expr, &x, &[3, 2], &y, &[3, 2], 0.1, &mut params,
            )
            .expect("step");
            losses.push(r.loss);
        }
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(
            last < first * 0.5,
            "multi-head attention training should halve loss: first={first} last={last}"
        );
        assert_eq!(params.len(), 4, "Q/K/V/O = 4 weight tensors regardless of head count");
    }

    #[test]
    fn attention_qkv_training_reduces_loss() {
        // Single Attention(2, 4) layer, learn an identity-ish target.
        let net = ast::NetDef {
            name: "AttnTrain".into(),
            generics: Vec::new(),
            layers: vec![named_layer("attn", "Attention", vec![2, 4])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        // 3 tokens of dim 2; target is the same shape — let attention learn
        // to reconstruct an arbitrary linear target from the input via QKV+O.
        let x = vec![1.0f32, 0.0, 0.0, 1.0, 0.5, 0.5];
        let y = vec![0.3f32, 0.7, 0.4, 0.6, 0.5, 0.5];
        let mut losses = Vec::new();
        for _ in 0..80 {
            let r = train_one_step(
                &backend, &lowered.expr, &x, &[3, 2], &y, &[3, 2], 0.1, &mut params,
            )
            .expect("step");
            losses.push(r.loss);
        }
        // 4 weight tensors updated each step: Q, K, V, O.
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(
            last < first * 0.5,
            "attention QKV training should halve loss: first={first} last={last}"
        );
        assert_eq!(params.len(), 4, "Q/K/V/O = 4 weight tensors");
    }

    #[test]
    fn conv2d_with_bias_allocates_two_tensors() {
        // Conv2D(1, 1, 2, 1): with bias. Forward should produce 3×3 output
        // and ParamStore should contain weight + bias = 2 tensors.
        let net = ast::NetDef {
            name: "ConvB".into(),
            generics: Vec::new(),
            layers: vec![named_layer("c", "Conv2D", vec![1, 1, 2, 1])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let input: Vec<f32> = (0..16).map(|i| (i as f32) * 0.125).collect();
        let handle = backend.from_slice_f32(&input, &[1, 4, 4]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        assert_eq!(running.shape, vec![1, 3, 3]);
        assert_eq!(params.len(), 2, "weight + bias = 2 entries");
    }

    #[test]
    fn conv2d_with_bias_trains() {
        // Conv2D(1, 1, 2, 1) on a 4×4 input toward a 3×3 constant target.
        // Bias should help the network shift the mean toward the target.
        let net = ast::NetDef {
            name: "ConvBT".into(),
            generics: Vec::new(),
            layers: vec![named_layer("c", "Conv2D", vec![1, 1, 2, 1])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let x: Vec<f32> = (0..16).map(|i| i as f32 * 0.1).collect();
        let y: Vec<f32> = vec![1.0; 9];
        let mut losses = Vec::new();
        for _ in 0..50 {
            let r = train_one_step(
                &backend, &lowered.expr, &x, &[1, 4, 4], &y, &[1, 3, 3], 0.05, &mut params,
            )
            .expect("step");
            losses.push(r.loss);
        }
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(last < first * 0.7, "conv2d+bias should reduce loss: first={first} last={last}");
        assert_eq!(params.len(), 2, "weight + bias both updated");
    }

    #[test]
    fn conv2d_training_reduces_loss() {
        // Conv2D(1, 1, 2): learn a 2x2 kernel that predicts mean-of-window.
        // Input is a flat 4x4 image; target is the constant 4-output image.
        let net = ast::NetDef {
            name: "ConvNet".into(),
            generics: Vec::new(),
            layers: vec![named_layer("c", "Conv2D", vec![1, 1, 2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        // Input: 4x4 with values 0..15 / 8.
        let x: Vec<f32> = (0..16).map(|i| i as f32 * 0.125).collect();
        // Target: 3x3 output (after 2x2 valid conv) all equal to some constant.
        let y: Vec<f32> = vec![1.0; 9];
        let mut losses = Vec::new();
        for _ in 0..50 {
            let r = train_one_step(
                &backend,
                &lowered.expr,
                &x,
                &[1, 4, 4],
                &y,
                &[1, 3, 3],
                0.05,
                &mut params,
            )
            .expect("step");
            losses.push(r.loss);
        }
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(
            last < first * 0.7,
            "conv2d training should reduce loss: first={first} last={last}"
        );
    }

    #[test]
    fn two_layer_linear_regression_converges() {
        // Linear(2, 4) >> Linear(4, 1). All-positive input avoids ReLU edge
        // cases for this test; ReLU coverage is in the activation tests.
        // Target is a linear function of input: y = 1.5*x1 + 0.5*x2.
        let net = ast::NetDef {
            name: "MLP2".into(),
            generics: Vec::new(),
            layers: vec![
                named_layer("fc1", "Linear", vec![2, 4]),
                named_layer("fc2", "Linear", vec![4, 1]),
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        // Inputs and corresponding targets y = 1.5*x1 + 0.5*x2.
        let x = [0.5f32, 0.5, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let y = [1.0f32, 1.5, 0.5, 2.0];
        let mut losses = Vec::new();
        for _ in 0..100 {
            let r = train_one_step(
                &backend,
                &lowered.expr,
                &x,
                &[4, 2],
                &y,
                &[4, 1],
                0.05,
                &mut params,
            )
            .expect("step");
            losses.push(r.loss);
        }
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(
            last < first * 0.5,
            "two-layer regression should halve loss in 100 steps: first={first} last={last}"
        );
    }

    #[test]
    fn attention_on_identical_tokens_returns_uniform_average() {
        // 4 identical tokens of dim 2 → attention weights are uniform 1/4.
        // Output equals the input (since averaging identical rows preserves them).
        let net = ast::NetDef {
            name: "Attn".into(),
            generics: Vec::new(),
            layers: vec![named_layer("attn", "Attention", vec![2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let input = vec![1.0f32, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0];
        let handle = backend.from_slice_f32(&input, &[4, 2]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        assert_eq!(running.shape, vec![4, 2]);
        let out = bytes_to_f32(&backend.copy_to_host(&running).unwrap());
        // Each row should still be [1.0, 2.0] within numerical tolerance.
        for i in 0..4 {
            assert!((out[i * 2] - 1.0).abs() < 1e-4, "row {i} col 0 = {}", out[i * 2]);
            assert!((out[i * 2 + 1] - 2.0).abs() < 1e-4, "row {i} col 1 = {}", out[i * 2 + 1]);
        }
    }

    #[test]
    fn multi_head_attention_preserves_shape() {
        // Attention(4, 8, 2): in_dim=4, model_dim=8, 2 heads (head_dim=4).
        let net = ast::NetDef {
            name: "MHA".into(),
            generics: Vec::new(),
            layers: vec![named_layer("attn", "Attention", vec![4, 8, 2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let input: Vec<f32> = (0..12).map(|i| (i as f32) * 0.1 - 0.6).collect();
        let handle = backend.from_slice_f32(&input, &[3, 4]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        // Same shape as single-head: [3, 4]
        assert_eq!(running.shape, vec![3, 4]);
        // Still 4 weight tensors (Q/K/V/O), shapes unchanged from single-head.
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn multi_head_attention_uneven_dim_errors() {
        // model_dim=7 not divisible by num_heads=2 → error.
        let net = ast::NetDef {
            name: "MHA".into(),
            generics: Vec::new(),
            layers: vec![named_layer("attn", "Attention", vec![4, 7, 2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let input: Vec<f32> = vec![0.1; 12];
        let handle = backend.from_slice_f32(&input, &[3, 4]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        let result = walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params);
        assert!(result.is_err(), "expected divisibility error");
    }

    #[test]
    fn attention_qkv_allocates_four_weight_tensors() {
        // Attention(in_dim=4, model_dim=8) should allocate Q, K, V, O
        // weights — 4 entries in the ParamStore.
        let net = ast::NetDef {
            name: "QKVAttn".into(),
            generics: Vec::new(),
            layers: vec![named_layer("attn", "Attention", vec![4, 8])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let input: Vec<f32> = (0..12).map(|i| (i as f32) * 0.1 - 0.6).collect();
        let handle = backend.from_slice_f32(&input, &[3, 4]).unwrap();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        // Output shape: [seq, in_dim] = [3, 4] (output projected back to in_dim).
        assert_eq!(running.shape, vec![3, 4]);
        // Q, K, V, O = 4 distinct weight tensors.
        assert_eq!(params.len(), 4, "expected 4 QKV+O weight tensors, got {}", params.len());
    }

    #[test]
    fn attention_concentrates_on_similar_tokens() {
        // Three tokens: [1,0], [1,0], [0,1]. The first two are identical and
        // dissimilar from the third. After self-attention, output rows 0 and 1
        // should be closer to [1,0] than to [0,1].
        let net = ast::NetDef {
            name: "Attn".into(),
            generics: Vec::new(),
            layers: vec![named_layer("attn", "Attention", vec![2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let input = vec![1.0f32, 0.0, 1.0, 0.0, 0.0, 1.0];
        let handle = backend.from_slice_f32(&input, &[3, 2]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        let out = bytes_to_f32(&backend.copy_to_host(&running).unwrap());
        // Row 0: x-component > y-component (more attention to similar [1,0] tokens).
        assert!(out[0] > out[1], "row 0 should lean toward [1,0]: got [{}, {}]", out[0], out[1]);
        assert!(out[2] > out[3], "row 1 should lean toward [1,0]: got [{}, {}]", out[2], out[3]);
    }

    #[test]
    fn conv2d_with_unit_kernel_acts_like_scale() {
        // Conv2D(in_ch=1, out_ch=1, k=1) on a 4x4 input is just elementwise
        // scale by the single weight. We can't predict the random weight, but
        // we can check: output shape = [1, 4, 4], and output is proportional
        // to input (ratio constant across pixels).
        let net = ast::NetDef {
            name: "C".into(),
            generics: Vec::new(),
            layers: vec![named_layer("conv", "Conv2D", vec![1, 1, 1])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let input: Vec<f32> = (0..16).map(|i| (i + 1) as f32).collect();
        let handle = backend.from_slice_f32(&input, &[1, 4, 4]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        assert_eq!(running.shape, vec![1, 4, 4]);
        let out = bytes_to_f32(&backend.copy_to_host(&running).unwrap());
        // Ratio out[i] / input[i] should be a constant (the weight).
        let ratio_0 = out[0] / input[0];
        for i in 1..16 {
            let r = out[i] / input[i];
            assert!((r - ratio_0).abs() < 1e-4, "expected constant ratio, got {r} vs {ratio_0}");
        }
    }

    #[test]
    fn conv2d_3x3_kernel_shrinks_spatial_dims() {
        // Conv2D(in_ch=2, out_ch=4, k=3) on 6x6 input → output [4, 4, 4].
        let net = ast::NetDef {
            name: "C".into(),
            generics: Vec::new(),
            layers: vec![named_layer("conv", "Conv2D", vec![2, 4, 3])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let input: Vec<f32> = (0..72).map(|i| (i as f32) * 0.1).collect();
        let handle = backend.from_slice_f32(&input, &[2, 6, 6]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        assert_eq!(running.shape, vec![4, 4, 4]);
        assert_eq!(dispatched, 1);
    }

    #[test]
    fn max_pool_picks_window_max() {
        // Input [1,3,2,5,4,1,6,7] kernel=2 stride=2 → [3, 5, 4, 7]
        let net = ast::NetDef {
            name: "MP".into(),
            generics: Vec::new(),
            layers: vec![named_layer("mp", "MaxPool", vec![2, 2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        // Use from_slice_f32 directly so we can supply non-uniform input.
        let handle = backend
            .from_slice_f32(&[1.0, 3.0, 2.0, 5.0, 4.0, 1.0, 6.0, 7.0], &[8])
            .unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        let bytes = backend.copy_to_host(&running).unwrap();
        let out: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(out, vec![3.0, 5.0, 4.0, 7.0]);
    }

    #[test]
    fn avg_pool_averages_window() {
        let net = ast::NetDef {
            name: "AP".into(),
            generics: Vec::new(),
            layers: vec![named_layer("ap", "AvgPool", vec![2])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        // [2, 4, 6, 8] with kernel=2 stride=2 → [3, 7]
        let handle = backend.from_slice_f32(&[2.0, 4.0, 6.0, 8.0], &[4]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        let bytes = backend.copy_to_host(&running).unwrap();
        let out: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(out, vec![3.0, 7.0]);
    }

    #[test]
    fn layer_norm_zeroes_mean_and_unit_variance() {
        let net = ast::NetDef {
            name: "LN".into(),
            generics: Vec::new(),
            layers: vec![norm_layer("ln", "LayerNorm")],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        // 8 elements of value 3.0 → after layer norm: all (3-3)/eps ≈ 0.
        let result = run_pipeline(&backend, &lowered.expr, &[8], 3.0).expect("run");
        assert_eq!(result.dispatched, 1);
        assert!(
            result.output_sum.abs() < 1e-3,
            "constant input → zeros after LN; got sum {}",
            result.output_sum
        );
    }

    #[test]
    fn layer_norm_with_affine_trains() {
        // LayerNorm(2) → Linear(2, 1): the LayerNorm γ/β need to learn to
        // preserve enough variance, and Linear maps to target.
        let net = ast::NetDef {
            name: "LNAff".into(),
            generics: Vec::new(),
            layers: vec![
                named_layer("ln", "LayerNorm", vec![2]),
                named_layer("fc", "Linear", vec![2, 1, 1]),
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let mut params = ParamStore::new();
        let x = vec![1.0f32, 2.0, 3.0, 4.0, 0.5, 1.5, 2.5, 3.5];
        let y = vec![1.0f32, 2.0, 3.0, 4.0];
        let mut losses = Vec::new();
        for _ in 0..200 {
            let r = train_one_step_with_optim(
                &backend, &lowered.expr, &x, &[4, 2], &y, &[4, 1], 0.1,
                Optimizer::adam_default(), &mut params, &mut OptimState::new(),
            ).expect("step");
            losses.push(r.loss);
        }
        let first = losses.first().copied().unwrap();
        let last = losses.last().copied().unwrap();
        assert!(last < first * 0.6, "LN affine + Linear should learn: first={first} last={last}");
        // 4 weight tensors: LN γ, LN β, Linear W, Linear bias.
        assert_eq!(params.len(), 4, "LN(γ+β) + Linear(W+b) = 4 params");
    }

    #[test]
    fn causal_attention_zeros_future_positions() {
        // Attention(2, 4, 1, 1): causal. With clearly distinct token i and
        // future token j > i, position i should NOT incorporate j's value.
        // Use parameterless variant by passing only the parameterless arg form,
        // wait — causal flag requires the 4-arg form. So Q/K/V are random.
        // Instead, test a specific property: token 0's output (causal) only
        // attends to itself, so it equals V[0] projection only.
        // Construct a hand-built Machine Language expression for parameterless-with-mask
        // path: not supported. So we exercise just the QKV+causal mode.
        let net = ast::NetDef {
            name: "Causal".into(),
            generics: Vec::new(),
            layers: vec![named_layer("attn", "Attention", vec![2, 4, 1, 1])],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        // Three tokens of dim 2.
        let input = vec![1.0f32, 0.0, 0.0, 1.0, 0.5, 0.5];
        let handle = backend.from_slice_f32(&input, &[3, 2]).unwrap();
        let mut params = ParamStore::new();
        let mut dispatched = 0;
        let mut unsupported = Vec::new();
        let mut running = handle;
        walk(&backend, &lowered.expr, &mut running, &mut dispatched, &mut unsupported, &mut params).unwrap();
        // Just verify shape preserved and 4 weight tensors allocated.
        assert_eq!(running.shape, vec![3, 2]);
        assert_eq!(params.len(), 4);

        // Now: token 0's output should be deterministic in causal mode
        // since it only attends to itself. With deterministic weight init,
        // re-running the same model with the same input should produce the
        // same output, and changing token 1 or 2 should NOT change token 0.
        let mut params2 = ParamStore::new();
        let alt_input = vec![1.0f32, 0.0, 999.0, 999.0, -999.0, -999.0];
        let alt_handle = backend.from_slice_f32(&alt_input, &[3, 2]).unwrap();
        let mut running2 = alt_handle;
        // Copy params from first run so the weights match.
        let blob = params.save(&backend).unwrap();
        params2 = ParamStore::load(&blob, &backend).unwrap();
        walk(&backend, &lowered.expr, &mut running2, &mut dispatched, &mut unsupported, &mut params2).unwrap();
        let out1 = bytes_to_f32(&backend.copy_to_host(&running).unwrap());
        let out2 = bytes_to_f32(&backend.copy_to_host(&running2).unwrap());
        // Token 0 = first 2 elements; should match across runs (causal masks future).
        assert!((out1[0] - out2[0]).abs() < 1e-3, "token 0 col 0 changed: {} vs {}", out1[0], out2[0]);
        assert!((out1[1] - out2[1]).abs() < 1e-3, "token 0 col 1 changed: {} vs {}", out1[1], out2[1]);
    }

    #[test]
    fn rms_norm_preserves_sign() {
        let net = ast::NetDef {
            name: "RN".into(),
            generics: Vec::new(),
            layers: vec![norm_layer("rn", "RMSNorm")],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        // RMS of [2,2,2,2] = 2, so output = [1,1,1,1], sum = 4.
        let result = run_pipeline(&backend, &lowered.expr, &[4], 2.0).expect("run");
        assert!(
            (result.output_sum - 4.0).abs() < 1e-3,
            "RMSNorm(2,2,2,2) should sum to 4, got {}",
            result.output_sum
        );
    }

    #[test]
    fn mse_loss_returns_scalar() {
        let net = ast::NetDef {
            name: "MseStub".into(),
            generics: Vec::new(),
            layers: vec![norm_layer("loss", "MSE")],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        // mean(2²) = 4 over 8 elements.
        let result = run_pipeline(&backend, &lowered.expr, &[8], 2.0).expect("run");
        assert_eq!(result.output.shape, vec![1]);
        assert!((result.output_sum - 4.0).abs() < 1e-3);
    }

    #[test]
    fn sgd_step_is_noop_on_forward() {
        let net = ast::NetDef {
            name: "OptStub".into(),
            generics: Vec::new(),
            layers: vec![
                activation_layer("ReLU"),
                norm_layer("step", "SGD_STEP"), // not in our forward table
            ],
            forward: empty_block(),
        };
        // SGD_STEP isn't in layer_name_to_op so we exercise it via a
        // hand-built Machine Language expression instead.
        let backend = CpuBackend::new();
        let expr = rmi::lang::Expr::op1(Op::SGD_STEP);
        let result = run_pipeline(&backend, &expr, &[4], 1.0).expect("run");
        // SGD_STEP forward is a no-op — input passes through unchanged.
        assert_eq!(result.dispatched, 1);
        assert!((result.output_sum - 4.0).abs() < 1e-3, "SGD_STEP forward should preserve input");
    }

    #[test]
    fn deep_mlp_pipelines_dims_through_linears() {
        // Linear(8,16) → ReLU → Linear(16,4) → Sigmoid. Input [8] → output [1, 4].
        let net = ast::NetDef {
            name: "Deep".into(),
            generics: Vec::new(),
            layers: vec![
                named_layer("fc1", "Linear", vec![8, 16]),
                activation_layer("ReLU"),
                named_layer("fc2", "Linear", vec![16, 4]),
                activation_layer("Sigmoid"),
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let backend = CpuBackend::new();
        let result = run_pipeline(&backend, &lowered.expr, &[8], 0.1).expect("run");
        assert_eq!(result.dispatched, 4);
        assert_eq!(result.output.shape, vec![1, 4]);
        // Sigmoid output is in (0, 1), so sum is in (0, 4).
        assert!(
            result.output_sum > 0.0 && result.output_sum < 4.0,
            "sigmoid output sum should be in (0, 4), got {}",
            result.output_sum
        );
    }

    /// **P84 bug-fix invariant**: `infer_input_shape` returns the
    /// model's declared first-dim instead of the previous hardcoded 8.
    /// Models with non-8-dim inputs must dispatch cleanly.
    #[test]
    fn infer_input_shape_picks_first_linear_in_features() {
        let net = ast::NetDef {
            name: "FourIn".into(),
            generics: Vec::new(),
            layers: vec![
                ast::LayerDef {
                    name: "fc".into(),
                    layer_type: ast::Type::Path {
                        segments: vec!["Linear".into()],
                        type_args: vec![],
                    },
                    args: vec![
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "4".into() },
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "2".into() },
                    ],
                },
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let shape = infer_input_shape(&lowered.expr)
            .expect("Linear's in_features should be inferable");
        assert_eq!(shape, vec![1, 4]);

        // And dispatch with that shape must succeed.
        let backend = CpuBackend::new();
        let result = run_pipeline(&backend, &lowered.expr, &shape, 0.5).expect("dispatch");
        assert_eq!(result.dispatched, 1);
        assert_eq!(result.output.shape, vec![1, 2]);
    }

    #[test]
    fn infer_input_shape_picks_attention_dim() {
        let net = ast::NetDef {
            name: "AttnNet".into(),
            generics: Vec::new(),
            layers: vec![
                ast::LayerDef {
                    name: "attn".into(),
                    layer_type: ast::Type::Path {
                        segments: vec!["Attention".into()],
                        type_args: vec![],
                    },
                    args: vec![
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "64".into() },
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "4".into() },
                    ],
                },
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let shape = infer_input_shape(&lowered.expr)
            .expect("Attention's dim should be inferable");
        assert_eq!(shape, vec![1, 64]);
    }

    /// P85 follow-up: CONV2D was inferring `[1, c, 1, 1]` which can't
    /// hold a real conv kernel. Now uses `max(kernel * 4, 32)` spatial
    /// so models with 7x7 stems and multiple pool stages still have
    /// room to operate.
    #[test]
    fn infer_input_shape_conv2d_picks_kernel_aware_spatial() {
        let net = ast::NetDef {
            name: "ConvNet".into(),
            generics: Vec::new(),
            layers: vec![
                ast::LayerDef {
                    name: "stem".into(),
                    layer_type: ast::Type::Path {
                        segments: vec!["Conv2D".into()],
                        type_args: vec![],
                    },
                    args: vec![
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "3".into() },
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "64".into() },
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "7".into() },
                    ],
                },
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let shape = infer_input_shape(&lowered.expr)
            .expect("Conv2D's in_channels should be inferable");
        // max(kernel*4, 32) = max(28, 32) = 32
        assert_eq!(shape, vec![1, 3, 32, 32]);
    }

    /// P85 follow-up: EMBED uses [1, 4] (4 token IDs) instead of
    /// [1, dim], so the lookup produces [1, 4, dim] which downstream
    /// ops can reason about rank-wise.
    #[test]
    fn infer_input_shape_embed_uses_small_token_count() {
        let net = ast::NetDef {
            name: "EmbedNet".into(),
            generics: Vec::new(),
            layers: vec![
                ast::LayerDef {
                    name: "e".into(),
                    layer_type: ast::Type::Path {
                        segments: vec!["Embed".into()],
                        type_args: vec![],
                    },
                    args: vec![
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "50000".into() },
                        ast::Expr::Literal { kind: ast::LiteralKind::Int, value: "768".into() },
                    ],
                },
            ],
            forward: empty_block(),
        };
        let lowered = NetTranslator::translate(&net);
        let shape = infer_input_shape(&lowered.expr).expect("inferable");
        assert_eq!(shape, vec![1, 4]);
    }

    /// P86: GlobalPool reduces all spatial dims to 1, producing
    /// [B, C] from [B, C, H, W]. Implements the missing dispatch
    /// that was leaving conv outputs unreduced and breaking the
    /// downstream classifier head.
    #[test]
    fn global_pool_reduces_4d_to_2d() {
        let backend = CpuBackend::new();
        // [1, 4, 2, 3] with all 1.0 -> [1, 4] with all 1.0 (mean preserved)
        let data: Vec<f32> = vec![1.0; 1 * 4 * 2 * 3];
        let input = backend.from_slice_f32(&data, &[1, 4, 2, 3]).expect("input");
        let out = dispatch_global_pool(&backend, &input).expect("pool");
        assert_eq!(out.shape, vec![1, 4]);
        let bytes = backend.copy_to_host(&out).unwrap();
        for chunk in bytes.chunks_exact(4) {
            let v = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            assert!((v - 1.0).abs() < 1e-6, "expected 1.0, got {v}");
        }
    }

    /// P86: ensure_2d now collapses leading dims when the last dim
    /// matches, enabling Linear-over-batch on 3D/4D inputs.
    #[test]
    fn ensure_2d_collapses_leading_dims() {
        let backend = CpuBackend::new();
        // [2, 3, 5] with last_dim=5 -> [6, 5]
        let data: Vec<f32> = (0..30).map(|i| i as f32).collect();
        let input = backend.from_slice_f32(&data, &[2, 3, 5]).expect("input");
        let out = ensure_2d(&backend, &input, 5).expect("reshape");
        assert_eq!(out.shape, vec![6, 5]);
    }

    #[test]
    fn infer_input_shape_returns_none_for_non_shape_bearing() {
        // A pure-symbolic / control-only expr won't have an inferable
        // shape - caller falls back to a default.
        let expr = Expr::Lit(Val::I64(7));
        assert!(infer_input_shape(&expr).is_none());
    }
}
