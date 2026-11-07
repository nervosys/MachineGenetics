//! The diagonal state-space learner (plan L2): a small linear-recurrent byte model.
//!
//! ## The model
//!
//! Each byte `x` has an embedding `E[x] ∈ ℝ^d`. A bank of `d` independent linear
//! channels carries a state `h ∈ ℝ^d` along the sequence:
//!
//! ```text
//! h_t = a ⊙ h_{t−1} + b ⊙ (W_in E[x_t])          diagonal recurrence
//! y_t = (W_out h_t) ⊙ σ(W_g E[x_t])               gated output
//! z_t = W_head y_t + c,    p_t = softmax(z_t)     next-byte distribution
//! ```
//!
//! **Stability, and the ODE view.** The decay is `a = exp(−exp(ℓ))`, which lies in
//! (0, 1) for every real `ℓ`, so no setting of the parameters makes the recurrence
//! grow without bound. This is the discretised-ODE view of a state-space model: the
//! continuous system `ẋ = Λx + Bu` with diagonal `Λ = −exp(ℓ) < 0`, zero-order-hold
//! discretised at unit step, has transition `a = exp(Λ)`. Each channel is therefore
//! one stable mode with a memory of about `1/(1 − a)` bytes, and the state is the
//! sum of those modes' responses to the byte stream. The input gain `b` plays the
//! role of the discretised `B̄`, but it is learned freely rather than tied to `Λ`.
//! Physical trajectories are sums of damped modes, which is why this family is
//! worth testing on them.
//!
//! ## Prediction, and the first byte
//!
//! The output at position `t` predicts byte `t` from the state after `bytes[..t]`
//! and the gate input `E[bytes[t−1]]`. Byte 0 has no context: its output is the
//! bias alone (`y_0 = 0`). Predictions are causal, and
//! [`ByteLearner::bits_per_byte`] is the mean of `−log₂ p_t(bytes[t])` over every
//! byte, which is the cost a causal coder with this model would pay.
//!
//! The output layer starts at zero, so the untrained model is exactly uniform
//! (8 bits per byte). Gradients to the other tensors are zero at the first step
//! and grow once `W_head` has moved.
//!
//! ## Training
//!
//! The backward pass is written by hand. Each train call sums the gradient of the
//! negative log-likelihood over its sequences, divides by the byte count, and
//! takes one Adam step on every parameter. The tests check the gradient of every
//! tensor against central differences.
//!
//! Backpropagation through the recurrence is truncated. With `bptt = k > 0` the
//! gradient carried back through the state is cut at every multiple of `k`, so a
//! block of `k` bytes never sends gradient into the block before it. The state
//! still carries forward through the whole chunk; only its gradient is cut. With
//! `bptt = 0` nothing is cut, and that is the objective the gradient check tests.
//!
//! ## Learning progress
//!
//! [`ByteLearner::progress`] is the n-gram's `|⟨∇L(y), P ⊙ δθ⟩| / |y|` (see
//! `crate::learner`), with `P` the bias-corrected Adam preconditioner as in
//! `crate::transformer`, and `δθ` measured from a reference snapshot that lags
//! between step/4 and step/2.

use crate::grammar::Rng;
use crate::learner::{ByteLearner, SsmConfig};

const V: usize = 256;
const BETA1: f64 = 0.9;
const BETA2: f64 = 0.999;
const EPS: f64 = 1e-8;
/// Seed of the deterministic initialisation: a given `d` always starts from the same parameters.
const SEED: u64 = 0x55D1_5EED;
/// The truncation length of backpropagation used when `ssm:d:lr` omits it, in bytes.
pub const DEFAULT_BPTT: usize = 64;

/// Offsets of each tensor in the flat parameter vector, for a model of width `d`.
#[derive(Debug, Clone, Copy)]
struct Layout {
    d: usize,
}

impl Layout {
    fn e(self) -> usize {
        0
    }
    fn w_in(self) -> usize {
        V * self.d
    }
    fn b(self) -> usize {
        self.w_in() + self.d * self.d
    }
    fn lam(self) -> usize {
        self.b() + self.d
    }
    fn w_out(self) -> usize {
        self.lam() + self.d
    }
    fn w_g(self) -> usize {
        self.w_out() + self.d * self.d
    }
    fn w_head(self) -> usize {
        self.w_g() + self.d * self.d
    }
    fn bias(self) -> usize {
        self.w_head() + V * self.d
    }
    fn total(self) -> usize {
        self.bias() + V
    }
    #[cfg(test)]
    fn named(self) -> [(&'static str, std::ops::Range<usize>); 8] {
        [
            ("E", self.e()..self.w_in()),
            ("W_in", self.w_in()..self.b()),
            ("b", self.b()..self.lam()),
            ("log_lambda", self.lam()..self.w_out()),
            ("W_out", self.w_out()..self.w_g()),
            ("W_g", self.w_g()..self.w_head()),
            ("W_head", self.w_head()..self.bias()),
            ("bias", self.bias()..self.total()),
        ]
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Everything the backward pass needs from one forward pass over a sequence.
struct Trace {
    /// `h[t]` is the state after `bytes[..t]`, so `h[0]` is the zero state.
    h: Vec<f64>,
    /// `v[t] = W_out h[t]`; zero at `t = 0`.
    v: Vec<f64>,
    /// `g[t] = σ(W_g E[bytes[t−1]])`; zero at `t = 0`.
    g: Vec<f64>,
    /// `y[t] = v[t] ⊙ g[t]`; zero at `t = 0`.
    y: Vec<f64>,
    /// `p[t]` is the distribution of byte `t`, `V` values per position.
    p: Vec<f64>,
    /// Summed negative log-likelihood, in nats.
    nll: f64,
}

pub struct SsmLearner {
    pub cfg: SsmConfig,
    /// Every parameter, in the order of [`Layout`].
    theta: Vec<f64>,
    m: Vec<f64>,
    v: Vec<f64>,
    /// Parameters at the reference snapshot, the origin of `δθ`.
    reference: Vec<f64>,
    /// The next reference, taken when training last doubled.
    candidate: Vec<f64>,
    candidate_step: u64,
    step: u64,
    bytes_seen: u64,
}

/// The initial parameters. The decays are spread over `[0.5, 0.99)`, so the channels
/// start with memories from a few bytes to about a hundred; `b = 1 − a` keeps a unit
/// input at unit steady-state size; the output layer is zero (see the module docs).
fn initial(d: usize) -> Vec<f64> {
    let lay = Layout { d };
    let mut rng = Rng(SEED);
    let mut theta = vec![0.0; lay.total()];
    let scale = 1.0 / (d as f64).sqrt();
    let mut draw = |s: f64| (rng.next_f64() * 2.0 - 1.0) * s;
    for x in &mut theta[lay.e()..lay.w_in()] {
        *x = draw(0.5);
    }
    for x in &mut theta[lay.w_in()..lay.b()] {
        *x = draw(scale);
    }
    for i in 0..d {
        let frac = if d > 1 { i as f64 / (d - 1) as f64 } else { 0.0 };
        let a = 0.5 + 0.49 * frac;
        // a = exp(−e) with e = exp(ℓ), so ℓ = ln(−ln a).
        theta[lay.lam() + i] = (-a.ln()).ln();
        theta[lay.b() + i] = 1.0 - a;
    }
    for x in &mut theta[lay.w_out()..lay.w_head()] {
        *x = draw(scale);
    }
    theta
}

impl SsmLearner {
    pub fn new(cfg: SsmConfig) -> SsmLearner {
        let theta = initial(cfg.d);
        let n = theta.len();
        SsmLearner {
            cfg,
            m: vec![0.0; n],
            v: vec![0.0; n],
            reference: theta.clone(),
            candidate: theta.clone(),
            theta,
            candidate_step: 0,
            step: 0,
            bytes_seen: 0,
        }
    }

    /// Every parameter drawn at random, so that no gradient is trivially zero.
    #[cfg(test)]
    fn randomised(d: usize, seed: u64) -> SsmLearner {
        let mut l = SsmLearner::new(SsmConfig { d, lr: 0.02, bptt: 0 });
        let mut rng = Rng(seed);
        let lam = Layout { d }.lam();
        for i in 0..l.theta.len() {
            l.theta[i] = if (lam..lam + d).contains(&i) {
                -2.0 + 2.5 * rng.next_f64()
            } else {
                rng.next_f64() - 0.5
            };
        }
        l
    }

    fn embed(&self, byte: u8) -> &[f64] {
        let d = self.cfg.d;
        let s = Layout { d }.e() + byte as usize * d;
        &self.theta[s..s + d]
    }

    /// The per-channel decays `a = exp(−exp(ℓ))`.
    fn decay(&self) -> Vec<f64> {
        let lam = Layout { d: self.cfg.d }.lam();
        (0..self.cfg.d).map(|i| (-self.theta[lam + i].exp()).exp()).collect()
    }

    fn forward(&self, bytes: &[u8]) -> Trace {
        let d = self.cfg.d;
        let lay = Layout { d };
        let th = &self.theta;
        let a = self.decay();
        let n = bytes.len();
        let mut h = vec![0.0; (n + 1) * d];
        for s in 0..n {
            let x = self.embed(bytes[s]);
            for i in 0..d {
                let u = dot(&th[lay.w_in() + i * d..][..d], x);
                h[(s + 1) * d + i] = a[i] * h[s * d + i] + th[lay.b() + i] * u;
            }
        }
        let mut tr = Trace {
            h,
            v: vec![0.0; n * d],
            g: vec![0.0; n * d],
            y: vec![0.0; n * d],
            p: vec![0.0; n * V],
            nll: 0.0,
        };
        for t in 0..n {
            if t > 0 {
                let x = self.embed(bytes[t - 1]);
                for i in 0..d {
                    let v = dot(&th[lay.w_out() + i * d..][..d], &tr.h[t * d..(t + 1) * d]);
                    let g = sigmoid(dot(&th[lay.w_g() + i * d..][..d], x));
                    tr.v[t * d + i] = v;
                    tr.g[t * d + i] = g;
                    tr.y[t * d + i] = v * g;
                }
            }
            let y = &tr.y[t * d..(t + 1) * d];
            let z: Vec<f64> = (0..V).map(|k| th[lay.bias() + k] + dot(&th[lay.w_head() + k * d..][..d], y)).collect();
            let m = z.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let lse = m + z.iter().map(|&q| (q - m).exp()).sum::<f64>().ln();
            for k in 0..V {
                tr.p[t * V + k] = (z[k] - lse).exp();
            }
            tr.nll += lse - z[bytes[t] as usize];
        }
        tr
    }

    /// The gradient of the summed negative log-likelihood (in nats) of `bytes`,
    /// with the recurrence's gradient cut at every `bptt`-th state (see the module docs).
    fn backprop(&self, bytes: &[u8], tr: &Trace) -> Vec<f64> {
        let d = self.cfg.d;
        let lay = Layout { d };
        let th = &self.theta;
        let a = self.decay();
        let n = bytes.len();
        let bptt = self.cfg.bptt;
        let mut grad = vec![0.0; lay.total()];
        // Accumulated dL/da, one per channel, summed over every state.
        let mut da = vec![0.0; d];
        // The gradient carried from state s+1 back into state s by the recurrence.
        let mut carry = vec![0.0; d];
        for s in (0..=n).rev() {
            // dL/dh[s]: the recurrence's carry, plus the output at position s.
            let mut gs = carry.clone();
            if s < n {
                // dL/dz_s = p_s − onehot(bytes[s]).
                let mut dz = tr.p[s * V..(s + 1) * V].to_vec();
                dz[bytes[s] as usize] -= 1.0;
                let y = &tr.y[s * d..(s + 1) * d];
                let mut dy = vec![0.0; d];
                for k in 0..V {
                    grad[lay.bias() + k] += dz[k];
                    let row = lay.w_head() + k * d;
                    for j in 0..d {
                        grad[row + j] += dz[k] * y[j];
                        dy[j] += th[row + j] * dz[k];
                    }
                }
                // The gated output needs a gate input, which byte 0 does not have.
                if s >= 1 {
                    let gate_byte = bytes[s - 1] as usize;
                    let x = self.embed(bytes[s - 1]);
                    let hs = &tr.h[s * d..(s + 1) * d];
                    for i in 0..d {
                        let gi = tr.g[s * d + i];
                        let dv = dy[i] * gi;
                        let dq = dy[i] * tr.v[s * d + i] * gi * (1.0 - gi);
                        let (ro, rg) = (lay.w_out() + i * d, lay.w_g() + i * d);
                        for j in 0..d {
                            grad[ro + j] += dv * hs[j];
                            gs[j] += th[ro + j] * dv;
                            grad[rg + j] += dq * x[j];
                            grad[lay.e() + gate_byte * d + j] += th[rg + j] * dq;
                        }
                    }
                }
            }
            // h[s] = a ⊙ h[s−1] + b ⊙ W_in E[bytes[s−1]].
            if s >= 1 {
                let byte = bytes[s - 1] as usize;
                let x = self.embed(bytes[s - 1]);
                let hprev = &tr.h[(s - 1) * d..s * d];
                for i in 0..d {
                    let row = lay.w_in() + i * d;
                    let u = dot(&th[row..row + d], x);
                    grad[lay.b() + i] += gs[i] * u;
                    da[i] += gs[i] * hprev[i];
                    let du = gs[i] * th[lay.b() + i];
                    for j in 0..d {
                        grad[row + j] += du * x[j];
                        grad[lay.e() + byte * d + j] += th[row + j] * du;
                    }
                }
                carry = if bptt > 0 && s % bptt == 0 {
                    vec![0.0; d]
                } else {
                    (0..d).map(|i| a[i] * gs[i]).collect()
                };
            }
        }
        // a = exp(−e) with e = exp(ℓ), so da/dℓ = −a·e.
        for i in 0..d {
            grad[lay.lam() + i] = -da[i] * a[i] * th[lay.lam() + i].exp();
        }
        grad
    }

    /// Mean negative log-likelihood of `bytes`, in bits per byte.
    pub fn bits_per_byte(&self, bytes: &[u8]) -> f64 {
        if bytes.is_empty() {
            return f64::NAN;
        }
        self.forward(bytes).nll / bytes.len() as f64 / std::f64::consts::LN_2
    }

    /// Learning progress per byte: `|⟨∇L(y), P ⊙ (θ − θ̄)⟩| / |y|`, with `P` the
    /// bias-corrected Adam preconditioner, as in `crate::transformer`.
    pub fn progress(&self, bytes: &[u8]) -> f64 {
        if bytes.is_empty() || self.step == 0 {
            return 0.0;
        }
        let tr = self.forward(bytes);
        let g = self.backprop(bytes, &tr);
        let bc2 = 1.0 - BETA2.powi(self.step as i32);
        let dot: f64 = (0..g.len())
            .map(|i| {
                let precond = 1.0 / ((self.v[i] / bc2).sqrt() + 1e-3);
                g[i] * precond * (self.theta[i] - self.reference[i])
            })
            .sum();
        dot.abs() / bytes.len() as f64
    }

    /// One Adam step on the mean gradient over every sequence in `batch`.
    pub fn train(&mut self, batch: &[&[u8]]) {
        let total = self.theta.len();
        let mut grad = vec![0.0; total];
        let mut count = 0usize;
        for &bytes in batch {
            if bytes.is_empty() {
                continue;
            }
            let tr = self.forward(bytes);
            for (acc, dg) in grad.iter_mut().zip(self.backprop(bytes, &tr)) {
                *acc += dg;
            }
            count += bytes.len();
            self.bytes_seen += bytes.len() as u64;
        }
        if count == 0 {
            return;
        }
        self.step += 1;
        let scale = 1.0 / count as f64;
        let bc1 = 1.0 - BETA1.powi(self.step as i32);
        let bc2 = 1.0 - BETA2.powi(self.step as i32);
        for i in 0..total {
            let g = grad[i] * scale;
            self.m[i] = BETA1 * self.m[i] + (1.0 - BETA1) * g;
            self.v[i] = BETA2 * self.v[i] + (1.0 - BETA2) * g * g;
            self.theta[i] -= self.cfg.lr * (self.m[i] / bc1) / ((self.v[i] / bc2).sqrt() + EPS);
        }
        // The reference lags between step/4 and step/2, as in `crate::learner`.
        if self.step >= 2 * self.candidate_step {
            std::mem::swap(&mut self.reference, &mut self.candidate);
            self.candidate.copy_from_slice(&self.theta);
            self.candidate_step = self.step;
        }
    }
}

impl ByteLearner for SsmLearner {
    fn bits_per_byte(&self, bytes: &[u8]) -> f64 {
        SsmLearner::bits_per_byte(self, bytes)
    }
    fn progress(&self, bytes: &[u8]) -> f64 {
        SsmLearner::progress(self, bytes)
    }
    fn train(&mut self, batch: &[&[u8]]) {
        SsmLearner::train(self, batch)
    }
    fn parameters(&self) -> usize {
        self.theta.len()
    }
    fn bytes_seen(&self) -> u64 {
        self.bytes_seen
    }
    fn describe(&self) -> String {
        format!("ssm d{} bptt{} lr{}", self.cfg.d, self.cfg.bptt, self.cfg.lr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> SsmLearner {
        SsmLearner::new(SsmConfig { d: 4, lr: 0.02, bptt: 0 })
    }

    #[test]
    fn untrained_is_uniform_eight_bits() {
        let l = tiny();
        assert!((l.bits_per_byte(b"hello world") - 8.0).abs() < 1e-9);
    }

    #[test]
    fn every_distribution_sums_to_one_and_is_causal() {
        let l = SsmLearner::randomised(4, 11);
        let text = b"the quick brown fox";
        let tr = l.forward(text);
        for t in 0..text.len() {
            let sum: f64 = tr.p[t * V..(t + 1) * V].iter().sum();
            assert!((sum - 1.0).abs() < 1e-12, "position {t} sums to {sum}");
        }
        // Changing bytes from 10 on must not move the predictions for 0..=10.
        let mut changed = text.to_vec();
        changed[10..].fill(b'z');
        let tr2 = l.forward(&changed);
        for t in 0..=10 {
            assert_eq!(tr.p[t * V..(t + 1) * V], tr2.p[t * V..(t + 1) * V], "position {t} sees the future");
        }
    }

    #[test]
    fn analytic_gradient_matches_central_differences_on_every_tensor() {
        let d = 4;
        let mut l = SsmLearner::randomised(d, 7);
        let bytes = b"abcbca".to_vec();
        let tr = l.forward(&bytes);
        let g = l.backprop(&bytes, &tr);
        let eps = 1e-5;
        let mut worst = 0.0f64;
        let lay = Layout { d };
        for (name, range) in lay.named() {
            let mut worst_here = 0.0f64;
            for i in range {
                let base = l.theta[i];
                l.theta[i] = base + eps;
                let up = l.forward(&bytes).nll;
                l.theta[i] = base - eps;
                let down = l.forward(&bytes).nll;
                l.theta[i] = base;
                let numeric = (up - down) / (2.0 * eps);
                let rel = (g[i] - numeric).abs() / g[i].abs().max(numeric.abs()).max(1e-5);
                worst_here = worst_here.max(rel);
                assert!(rel < 1e-4, "{name}[{i}]: analytic {} vs numeric {numeric} (rel {rel:e})", g[i]);
            }
            eprintln!("gradient check, {name}: worst relative error {worst_here:e}");
            worst = worst.max(worst_here);
        }
        eprintln!("gradient check, all tensors: worst relative error {worst:e}");
    }

    #[test]
    fn truncation_is_exact_when_the_block_covers_the_sequence_and_not_otherwise() {
        let bytes = b"abcbcaabcbca".to_vec();
        let full = SsmLearner::randomised(4, 5);
        let mut whole = SsmLearner::randomised(4, 5);
        whole.cfg.bptt = 1000;
        let mut cut = SsmLearner::randomised(4, 5);
        cut.cfg.bptt = 3;
        let g_full = full.backprop(&bytes, &full.forward(&bytes));
        let g_whole = whole.backprop(&bytes, &whole.forward(&bytes));
        let g_cut = cut.backprop(&bytes, &cut.forward(&bytes));
        assert_eq!(g_full, g_whole);
        assert!(g_full.iter().zip(&g_cut).any(|(a, b)| (a - b).abs() > 1e-9), "a 3-byte block changed nothing");
    }

    #[test]
    fn training_reduces_bits_per_byte_on_a_repetitive_sequence() {
        let mut l = SsmLearner::new(SsmConfig { d: 8, lr: 0.02, bptt: 0 });
        let data: Vec<u8> = b"abc".repeat(12);
        let before = l.bits_per_byte(&data);
        for _ in 0..150 {
            l.train(&[&data]);
        }
        let after = l.bits_per_byte(&data);
        eprintln!("repetitive sequence: {before:.3} -> {after:.3} bits per byte");
        assert!(after < 0.5 * before, "{before} -> {after}");
    }

    #[test]
    fn same_input_same_bits_per_byte() {
        let run = || {
            let mut l = SsmLearner::new(SsmConfig { d: 6, lr: 0.02, bptt: 16 });
            for _ in 0..5 {
                l.train(&[b"hello there, general kenobi", b"hello again"]);
            }
            l.bits_per_byte(b"hello there")
        };
        assert_eq!(run().to_bits(), run().to_bits());
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
    fn the_learner_reports_its_kind_and_shape() {
        let l = tiny();
        let d = ByteLearner::describe(&l);
        assert!(d.starts_with("ssm d4 bptt0 lr0.02"), "{d}");
        assert_eq!(ByteLearner::parameters(&l), Layout { d: 4 }.total());
    }
}
