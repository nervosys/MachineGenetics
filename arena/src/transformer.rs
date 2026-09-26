//! A byte-level causal transformer learner, on candle.
//!
//! Plan task 4.2. The n-gram learner's measured failure is that its frontier
//! *is* constant runs, so the reward pays generators for constants however the
//! novelty is keyed (`ARENA.md`, three exploits in one day). Self-play
//! pretraining's result depends on a learner with in-context mechanisms —
//! copying, counting, composition — and this is the smallest one that has them.
//!
//! ## Why candle, measured
//!
//! Task 4.1 compared the two paths on this machine (2× RTX 3090 Ti). The
//! repository's own ABL training path computes attention in scalar Rust loops
//! over host memory, with 65 host round-trips in `abl_compute.rs`, and no MAGE
//! source has ever trained an attention net through it. candle trained a
//! 3.4M-parameter, 4-layer causal model (context 512, batch 16) at **20 ms per
//! step, ~408k tokens/s** on one GPU — 134× its own CPU backend on the same
//! model.
//!
//! Building with `--features gpu` on Windows needs MSVC's environment
//! (`vcvars64.bat`), `CUDA_PATH` at a toolkit, and
//! `NVCC_APPEND_FLAGS=-Xcompiler /Zc:preprocessor`, because CUDA 13's headers
//! refuse MSVC's traditional preprocessor. `--features transformer` builds the
//! same model on candle's CPU backend, which is what CI tests.
//!
//! ## The learning-progress reward, exactly
//!
//! `|⟨∇θ L(y), P ⊙ δθ⟩| / |y|`, with `P = 1 / (√v̂ + ε)` from **this learner's
//! own Adam state**. That is why it does not use `candle_nn::AdamW`: the reward
//! needs the preconditioner, and AdamW keeps it private. `δθ` is the
//! displacement from a reference snapshot that lags between `e/4` and `e/2`
//! steps, as in the n-gram learner and the paper.
//!
//! ## Every byte is predicted
//!
//! Each window is fed as `[BOS, b₀, …, bₙ₋₂]` to predict `[b₀, …, bₙ₋₁]`, with
//! BOS a 257th embedding row, so bits per byte counts every byte, exactly as
//! the n-gram learner's does. The two are then comparable on one front.

use crate::learner::{ByteLearner, TransformerConfig};
use candle_core::{DType, Device, Module, Tensor, Var, D};
use candle_nn::{embedding, layer_norm, linear, Embedding, LayerNorm, Linear, VarBuilder, VarMap};

const BOS: u32 = 256;
/// Seed for weight initialisation (see `TransformerLearner::new`).
const INIT_SEED: u64 = 0x5EED_A12E;
const BETA1: f64 = 0.9;
const BETA2: f64 = 0.999;
const EPS: f64 = 1e-8;

struct Block {
    ln1: LayerNorm,
    qkv: Linear,
    proj: Linear,
    ln2: LayerNorm,
    fc1: Linear,
    fc2: Linear,
    heads: usize,
}

impl Block {
    fn new(vb: VarBuilder, d: usize, heads: usize) -> candle_core::Result<Self> {
        Ok(Block {
            ln1: layer_norm(d, 1e-5, vb.pp("ln1"))?,
            qkv: linear(d, 3 * d, vb.pp("qkv"))?,
            proj: linear(d, d, vb.pp("proj"))?,
            ln2: layer_norm(d, 1e-5, vb.pp("ln2"))?,
            fc1: linear(d, 4 * d, vb.pp("fc1"))?,
            fc2: linear(4 * d, d, vb.pp("fc2"))?,
            heads,
        })
    }

    fn forward(&self, x: &Tensor, mask: &Tensor) -> candle_core::Result<Tensor> {
        let (b, t, d) = x.dims3()?;
        let hd = d / self.heads;
        let h = self.ln1.forward(x)?;
        let qkv = self.qkv.forward(&h)?.reshape((b, t, 3, self.heads, hd))?;
        let part = |i: usize| -> candle_core::Result<Tensor> {
            qkv.narrow(2, i, 1)?.squeeze(2)?.transpose(1, 2)?.contiguous()
        };
        let (q, k, v) = (part(0)?, part(1)?, part(2)?);
        let att = (q.matmul(&k.t()?)? / (hd as f64).sqrt())?;
        let att = candle_nn::ops::softmax_last_dim(&att.broadcast_add(mask)?)?;
        let y = att.matmul(&v)?.transpose(1, 2)?.reshape((b, t, d))?;
        let x = (x + self.proj.forward(&y)?)?;
        let h = self.fc2.forward(&self.fc1.forward(&self.ln2.forward(&x)?)?.gelu()?)?;
        x + h
    }
}

struct Model {
    emb: Embedding,
    pos: Embedding,
    blocks: Vec<Block>,
    ln: LayerNorm,
    head: Linear,
}

impl Model {
    fn forward(&self, ids: &Tensor, mask: &Tensor) -> candle_core::Result<Tensor> {
        let (_, t) = ids.dims2()?;
        let pos = Tensor::arange(0u32, t as u32, ids.device())?;
        let mut x = self.emb.forward(ids)?.broadcast_add(&self.pos.forward(&pos)?)?;
        for blk in &self.blocks {
            x = blk.forward(&x, mask)?;
        }
        self.head.forward(&self.ln.forward(&x)?)
    }
}

/// Redraw every parameter deterministically, from its *shape*: matrices
/// uniform with standard deviation `1/sqrt(fan_in)` (fan-in is the last
/// dimension), vectors that candle initialised randomly (biases) to zero, and
/// constant tensors (layer-norm ones) left alone. Parameters are visited in
/// sorted-name order, because the VarMap is a hash map.
///
/// The first version scaled each tensor by the extremes of candle's own
/// unseeded draw. That was neither reproducible (the bound varied with the
/// draw) nor sane: the maximum of a normal sample is about 4 sigma, so the
/// redrawn weights were ~2.3x too wide and an untrained model scored 16
/// bits/byte.
fn reseed(varmap: &VarMap, seed: u64) -> candle_core::Result<()> {
    let data = varmap.data().lock().expect("varmap lock");
    let mut names: Vec<&String> = data.keys().collect();
    names.sort();
    let mut rng = crate::grammar::Rng(seed);
    for name in names {
        let var = &data[name];
        let t = var.as_tensor();
        let dims = t.dims().to_vec();
        let host: Vec<f32> = t.flatten_all()?.to_dtype(DType::F32)?.to_vec1()?;
        let (lo, hi) = host.iter().fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        if hi - lo < 1e-12 {
            continue;
        }
        let fresh: Vec<f32> = if dims.len() >= 2 {
            let fan_in = *dims.last().expect("rank >= 2") as f64;
            let bound = (3.0 / fan_in).sqrt();
            (0..host.len()).map(|_| ((rng.next_f64() * 2.0 - 1.0) * bound) as f32).collect()
        } else {
            vec![0.0; host.len()]
        };
        var.set(&Tensor::from_vec(fresh, t.shape(), t.device())?.to_dtype(t.dtype())?)?;
    }
    Ok(())
}

pub struct TransformerLearner {
    cfg: TransformerConfig,
    device: Device,
    vars: Vec<Var>,
    model: Model,
    m: Vec<Tensor>,
    v: Vec<Tensor>,
    reference: Vec<Tensor>,
    candidate: Vec<Tensor>,
    candidate_step: u64,
    step: u64,
    bytes_seen: u64,
}

fn fail(e: candle_core::Error) -> String {
    format!("candle: {e}")
}

impl TransformerLearner {
    pub fn new(cfg: TransformerConfig) -> Result<Self, String> {
        if cfg.d % cfg.heads != 0 {
            return Err(format!("model width {} is not divisible by {} heads", cfg.d, cfg.heads));
        }
        let device = if cfg.gpu {
            Device::new_cuda(0).map_err(|e| format!("no CUDA device (build with --features gpu): {e}"))?
        } else {
            Device::Cpu
        };
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let blocks = (0..cfg.layers)
            .map(|i| Block::new(vb.pp(format!("b{i}")), cfg.d, cfg.heads))
            .collect::<candle_core::Result<Vec<_>>>()
            .map_err(fail)?;
        let model = Model {
            emb: embedding(257, cfg.d, vb.pp("emb")).map_err(fail)?,
            pos: embedding(cfg.ctx, cfg.d, vb.pp("pos")).map_err(fail)?,
            blocks,
            ln: layer_norm(cfg.d, 1e-5, vb.pp("ln")).map_err(fail)?,
            head: linear(cfg.d, 256, vb.pp("head")).map_err(fail)?,
        };
        // Seeded, so a run's initial weights are a function of its
        // configuration. Unseeded, the untrained model's bits/byte varied from
        // run to run (8.3, then 9.9), which made a test pass or fail by chance,
        // and the arena's promise that a run is reproducible from its seed was
        // quietly false for this learner. candle's CPU backend refuses
        // `set_seed`, so the weights are redrawn here from the arena's own
        // generator, identically on CPU and GPU.
        reseed(&varmap, INIT_SEED).map_err(fail)?;
        let vars = varmap.all_vars();
        let zeros = |vars: &[Var]| -> Result<Vec<Tensor>, String> {
            vars.iter().map(|v| v.as_tensor().zeros_like().map_err(fail)).collect()
        };
        let snapshot = |vars: &[Var]| -> Result<Vec<Tensor>, String> {
            vars.iter().map(|v| v.as_tensor().copy().map_err(fail)).collect()
        };
        Ok(TransformerLearner {
            m: zeros(&vars)?,
            v: zeros(&vars)?,
            reference: snapshot(&vars)?,
            candidate: snapshot(&vars)?,
            candidate_step: 0,
            step: 0,
            bytes_seen: 0,
            cfg,
            device,
            vars,
            model,
        })
    }

    fn mask(&self, t: usize) -> candle_core::Result<Tensor> {
        let m: Vec<f32> =
            (0..t * t).map(|i| if i % t > i / t { f32::NEG_INFINITY } else { 0.0 }).collect();
        Tensor::from_vec(m, (t, t), &self.device)
    }

    /// Windows of at most `ctx` bytes, each fed as `[BOS, …]`.
    fn windows<'a>(&self, bytes: &'a [u8]) -> Vec<&'a [u8]> {
        bytes.chunks(self.cfg.ctx).collect()
    }

    /// Summed negative log-likelihood (nats) of `windows`, and their byte count,
    /// as a differentiable scalar.
    fn nll(&self, windows: &[&[u8]]) -> candle_core::Result<(Tensor, usize)> {
        let t = windows.iter().map(|w| w.len()).max().unwrap_or(0);
        let b = windows.len();
        let mut ids = vec![0u32; b * t];
        let mut tgt = vec![0u32; b * t];
        let mut keep = vec![0f32; b * t];
        for (i, w) in windows.iter().enumerate() {
            for (j, &byte) in w.iter().enumerate() {
                ids[i * t + j] = if j == 0 { BOS } else { w[j - 1] as u32 };
                tgt[i * t + j] = byte as u32;
                keep[i * t + j] = 1.0;
            }
        }
        let ids = Tensor::from_vec(ids, (b, t), &self.device)?;
        let tgt = Tensor::from_vec(tgt, (b * t, 1), &self.device)?;
        let keep = Tensor::from_vec(keep, (b * t,), &self.device)?;
        let logits = self.model.forward(&ids, &self.mask(t)?)?.reshape((b * t, 256))?;
        let logp = candle_nn::ops::log_softmax(&logits, D::Minus1)?;
        let picked = logp.gather(&tgt, D::Minus1)?.squeeze(D::Minus1)?;
        let n: usize = windows.iter().map(|w| w.len()).sum();
        Ok(((picked * keep)?.sum_all()?.neg()?, n))
    }

    /// Per-window summed negative log-likelihood (nats), not differentiated.
    fn nll_rows(&self, windows: &[&[u8]]) -> candle_core::Result<Vec<f32>> {
        let t = windows.iter().map(|w| w.len()).max().unwrap_or(0);
        let b = windows.len();
        let mut ids = vec![0u32; b * t];
        let mut tgt = vec![0u32; b * t];
        let mut keep = vec![0f32; b * t];
        for (i, w) in windows.iter().enumerate() {
            for (j, &byte) in w.iter().enumerate() {
                ids[i * t + j] = if j == 0 { BOS } else { w[j - 1] as u32 };
                tgt[i * t + j] = byte as u32;
                keep[i * t + j] = 1.0;
            }
        }
        let ids = Tensor::from_vec(ids, (b, t), &self.device)?;
        let tgt = Tensor::from_vec(tgt, (b * t, 1), &self.device)?;
        let keep = Tensor::from_vec(keep, (b, t), &self.device)?;
        let logits = self.model.forward(&ids, &self.mask(t)?)?.reshape((b * t, 256))?;
        let logp = candle_nn::ops::log_softmax(&logits, D::Minus1)?;
        let picked = logp.gather(&tgt, D::Minus1)?.reshape((b, t))?;
        (picked * keep)?.sum(D::Minus1)?.neg()?.to_vec1::<f32>()
    }

    fn adam_step(&mut self, loss: &Tensor) -> candle_core::Result<()> {
        let grads = loss.backward()?;
        self.step += 1;
        let (bc1, bc2) = (1.0 - BETA1.powi(self.step as i32), 1.0 - BETA2.powi(self.step as i32));
        for (i, var) in self.vars.iter().enumerate() {
            let Some(g) = grads.get(var.as_tensor()) else { continue };
            self.m[i] = ((&self.m[i] * BETA1)? + (g * (1.0 - BETA1))?)?;
            self.v[i] = ((&self.v[i] * BETA2)? + (g.sqr()? * (1.0 - BETA2))?)?;
            let mhat = (&self.m[i] / bc1)?;
            let denom = ((&self.v[i] / bc2)?.sqrt()? + EPS)?;
            let update = ((mhat / denom)? * self.cfg.lr)?;
            var.set(&(var.as_tensor() - update)?)?;
        }
        // The reference lags between step/4 and step/2 (see module docs).
        if self.step >= 2 * self.candidate_step {
            std::mem::swap(&mut self.reference, &mut self.candidate);
            self.candidate = self.vars.iter().map(|v| v.as_tensor().copy()).collect::<candle_core::Result<_>>()?;
            self.candidate_step = self.step;
        }
        Ok(())
    }

    fn progress_inner(&self, bytes: &[u8]) -> candle_core::Result<f64> {
        let windows = self.windows(bytes);
        let (sum, n) = self.nll(&windows)?;
        let loss = (sum / n.max(1) as f64)?;
        let grads = loss.backward()?;
        let bc2 = 1.0 - BETA2.powi(self.step.max(1) as i32);
        let mut dot = 0.0f64;
        for (i, var) in self.vars.iter().enumerate() {
            let Some(g) = grads.get(var.as_tensor()) else { continue };
            let precond = ((&self.v[i] / bc2)?.sqrt()? + 1e-3)?.recip()?;
            let delta = (var.as_tensor() - &self.reference[i])?;
            dot += (g * precond)?.mul(&delta)?.sum_all()?.to_dtype(DType::F64)?.to_scalar::<f64>()?;
        }
        Ok(dot.abs())
    }
}

impl ByteLearner for TransformerLearner {
    fn bits_per_byte(&self, bytes: &[u8]) -> f64 {
        if bytes.is_empty() {
            return f64::NAN;
        }
        let windows = self.windows(bytes);
        // Batched in groups so a long corpus does not become one huge tensor.
        let mut nats = 0.0;
        for group in windows.chunks(16) {
            match self.nll(group).and_then(|(s, _)| s.to_scalar::<f32>()) {
                Ok(s) => nats += s as f64,
                Err(_) => return f64::NAN,
            }
        }
        nats / bytes.len() as f64 / std::f64::consts::LN_2
    }

    fn bits_per_byte_many(&self, seqs: &[&[u8]]) -> Vec<f64> {
        // Every window of every sequence, tagged with its sequence, scored in
        // groups of 64 windows rather than one pass per sequence.
        let mut tagged: Vec<(usize, &[u8])> = Vec::new();
        for (si, s) in seqs.iter().enumerate() {
            for w in self.windows(s) {
                tagged.push((si, w));
            }
        }
        let mut nats = vec![0.0f64; seqs.len()];
        for group in tagged.chunks(64) {
            let windows: Vec<&[u8]> = group.iter().map(|(_, w)| *w).collect();
            match self.nll_rows(&windows) {
                Ok(rows) => {
                    for ((si, _), v) in group.iter().zip(rows) {
                        nats[*si] += v as f64;
                    }
                }
                Err(_) => return vec![f64::NAN; seqs.len()],
            }
        }
        seqs.iter()
            .zip(nats)
            .map(|(s, n)| if s.is_empty() { f64::NAN } else { n / s.len() as f64 / std::f64::consts::LN_2 })
            .collect()
    }

    fn progress(&self, bytes: &[u8]) -> f64 {
        if bytes.is_empty() || self.step == 0 {
            return 0.0;
        }
        self.progress_inner(bytes).unwrap_or(0.0)
    }

    fn train(&mut self, batch: &[&[u8]]) {
        let windows: Vec<&[u8]> = batch.iter().flat_map(|b| self.windows(b)).collect();
        if windows.is_empty() {
            return;
        }
        let n: usize = windows.iter().map(|w| w.len()).sum();
        let step = self.nll(&windows).and_then(|(s, n)| s / n.max(1) as f64);
        if let Ok(loss) = step {
            // A failed step leaves the parameters as they were; the arena sees
            // no improvement rather than a crash.
            if self.adam_step(&loss).is_ok() {
                self.bytes_seen += n as u64;
            }
        }
    }

    fn parameters(&self) -> usize {
        self.vars.iter().map(|v| v.elem_count()).sum()
    }

    fn bytes_seen(&self) -> u64 {
        self.bytes_seen
    }

    fn describe(&self) -> String {
        format!(
            "transformer d{} L{} h{} ctx{} lr{} on {}",
            self.cfg.d,
            self.cfg.layers,
            self.cfg.heads,
            self.cfg.ctx,
            self.cfg.lr,
            if self.cfg.gpu { "cuda:0" } else { "cpu" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> TransformerLearner {
        TransformerLearner::new(TransformerConfig { d: 32, layers: 1, heads: 2, ctx: 32, lr: 3e-3, gpu: false })
            .expect("builds on cpu")
    }

    #[test]
    fn untrained_is_near_uniform_counts_every_byte_and_is_reproducible() {
        let text = b"hello world, hello world";
        let (a, b) = (tiny().bits_per_byte(text), tiny().bits_per_byte(text));
        // Seeded initialisation: two builds agree exactly.
        assert_eq!(a.to_bits(), b.to_bits(), "{a} vs {b}");
        // Random init is not uniform; measured spread across unseeded inits
        // reached 9.9 bits, so the band is what init actually guarantees.
        assert!((a - 8.0).abs() < 2.5, "{a}");
    }

    #[test]
    fn training_learns_a_pattern_it_can_only_learn_in_context() {
        // A period-5 pattern: predicting it needs the previous byte at least,
        // which this model sees through attention, not a hashed table.
        let data: Vec<u8> = b"vwxyz".iter().cycle().take(64).cloned().collect();
        let mut l = tiny();
        let before = l.bits_per_byte(&data);
        for _ in 0..150 {
            l.train(&[&data]);
        }
        let after = l.bits_per_byte(&data);
        assert!(after < before * 0.3, "{before} -> {after}");
    }

    #[test]
    fn progress_is_zero_before_training_and_positive_after() {
        let mut l = tiny();
        assert_eq!(l.progress(b"abcabc"), 0.0);
        let data = b"abcabcabcabc".to_vec();
        for _ in 0..10 {
            l.train(&[&data]);
        }
        assert!(l.progress(&data) > 0.0);
    }

    #[test]
    fn batched_scoring_agrees_with_one_at_a_time() {
        let mut l = tiny();
        let data: Vec<u8> = b"vwxyz".iter().cycle().take(80).cloned().collect();
        for _ in 0..20 {
            l.train(&[&data]);
        }
        let seqs: Vec<Vec<u8>> = vec![data.clone(), b"hello".to_vec(), (0u8..70).collect()];
        let refs: Vec<&[u8]> = seqs.iter().map(|s| s.as_slice()).collect();
        let many = l.bits_per_byte_many(&refs);
        for (s, m) in refs.iter().zip(many) {
            let one = l.bits_per_byte(s);
            assert!((one - m).abs() < 1e-3, "{one} vs {m}");
        }
    }

    #[test]
    fn a_bad_shape_is_refused_not_panicked() {
        let e = TransformerLearner::new(TransformerConfig { d: 30, layers: 1, heads: 4, ctx: 8, lr: 1e-3, gpu: false });
        assert!(e.is_err());
    }
}
