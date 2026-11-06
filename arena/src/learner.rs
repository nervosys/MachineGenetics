//! The learner: a byte-level predictor with exact gradients.
//!
//! ## What it is, and what it is not
//!
//! Self-play pretraining trains a Llama-style transformer. This is a
//! **hashed-context softmax model** — for each order `k`, the previous `k`
//! bytes select a row of logits, and the rows are summed — trained by Adam with
//! exact gradients. It learns order-`k` statistics and nothing longer-range: it
//! cannot copy across a gap or run a counter, which is the regularity a
//! transformer would pick up from generated data. It is the smallest learner
//! for which every quantity the arena needs is exact and cheap on a CPU —
//! gradients, the learning-progress reward, held-out bits per byte — so the
//! loop can be built, tested and measured end to end before a GPU learner is
//! swapped in behind the same interface.
//!
//! ## The learning-progress reward
//!
//! From Cowsik et al.: a sequence is worth `|⟨∇θ L(y), P ⊙ δθ⟩|`, the alignment
//! between the gradient it would produce and the direction the learner is
//! actually moving, with `P` the optimiser's diagonal preconditioner. Rewarding
//! *difficulty* instead collapses into noise — random bytes are maximally hard
//! and teach nothing — while this rewards data on the frontier of what the
//! learner is currently learning.
//!
//! `δθ` is the parameter movement over a lookback window of about half the
//! training so far — the paper's `⌊e/2⌋`. Two snapshots implement it: the
//! reference sits between steps `e/4` and `e/2`, and is replaced by the newer
//! snapshot each time training doubles in length.
//!
//! **The window length is load-bearing, and the first version got it wrong.**
//! It used an exponential moving average with decay 0.9, so the reference
//! trailed the parameters by a handful of steps and `δθ` was the *last few*
//! updates. The measured result, on the first real run (2026-09-25): the best
//! programs were constant byte runs — `[129; 6]`, `range(41).map(|v| 232)`.
//! A short window makes the reward self-confirming: whatever the learner just
//! saw dominates its recent movement, so more of the same aligns best with it,
//! and the generator collapses onto one byte. A long window measures what the
//! learner has been learning *persistently*, which one round cannot dominate.
//!
//! The reward is taken **per byte**, so a program cannot raise its score by
//! printing more of the same.

use serde::{Deserialize, Serialize};

/// Learner hyperparameters — the continuous, convex-ish part of the search.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LearnerConfig {
    /// Context orders used, `1..=orders`.
    pub orders: usize,
    /// Rows per order, as a power of two.
    pub log2_buckets: u32,
    pub lr: f64,
}

impl Default for LearnerConfig {
    fn default() -> Self {
        LearnerConfig { orders: 3, log2_buckets: 12, lr: 0.02 }
    }
}

/// What the arena needs of any learner: an exact score, the learning-progress
/// reward, and a training step. The n-gram [`Learner`] and the transformer
/// (`crate::transformer`, behind the `transformer` feature) both implement it,
/// so they compete on one front.
pub trait ByteLearner {
    /// Mean negative log-likelihood of `bytes`, in bits per byte.
    fn bits_per_byte(&self, bytes: &[u8]) -> f64;
    /// `|⟨∇L(bytes), P ⊙ δθ⟩| / |bytes|` — see the module docs.
    fn progress(&self, bytes: &[u8]) -> f64;
    /// One optimiser step over `batch`.
    fn train(&mut self, batch: &[&[u8]]);
    fn parameters(&self) -> usize;
    fn bytes_seen(&self) -> u64;
    fn describe(&self) -> String;
    /// [`ByteLearner::bits_per_byte`] of many sequences. The default loops;
    /// a GPU learner overrides it to score them in a few batched passes,
    /// because compression progress scores every program's probe twice a
    /// round and one small pass each left the GPU mostly idle.
    fn bits_per_byte_many(&self, seqs: &[&[u8]]) -> Vec<f64> {
        seqs.iter().map(|s| self.bits_per_byte(s)).collect()
    }
}

/// A causal byte transformer's shape and optimiser settings.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TransformerConfig {
    pub d: usize,
    pub layers: usize,
    pub heads: usize,
    pub ctx: usize,
    pub lr: f64,
    /// Run on CUDA device 0 (needs `--features gpu`) rather than the CPU.
    pub gpu: bool,
}

/// Which learner to build.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LearnerSpec {
    Ngram(LearnerConfig),
    Transformer(TransformerConfig),
}

impl Default for LearnerSpec {
    fn default() -> Self {
        LearnerSpec::Ngram(LearnerConfig::default())
    }
}

/// Build a learner, or say why this binary cannot.
pub fn build(spec: LearnerSpec) -> Result<Box<dyn ByteLearner>, String> {
    match spec {
        LearnerSpec::Ngram(c) => Ok(Box::new(Learner::new(c))),
        #[cfg(feature = "transformer")]
        LearnerSpec::Transformer(c) => Ok(Box::new(crate::transformer::TransformerLearner::new(c)?)),
        #[cfg(not(feature = "transformer"))]
        LearnerSpec::Transformer(_) => {
            Err("this arena was built without the `transformer` feature (add --features transformer, or gpu)".into())
        }
    }
}

impl ByteLearner for Learner {
    fn bits_per_byte(&self, bytes: &[u8]) -> f64 {
        Learner::bits_per_byte(self, bytes)
    }
    fn progress(&self, bytes: &[u8]) -> f64 {
        Learner::progress(self, bytes)
    }
    fn train(&mut self, batch: &[&[u8]]) {
        Learner::train(self, batch)
    }
    fn parameters(&self) -> usize {
        Learner::parameters(self)
    }
    fn bytes_seen(&self) -> u64 {
        self.bytes_seen
    }
    fn describe(&self) -> String {
        format!("ngram {}×2^{} lr{}", self.cfg.orders, self.cfg.log2_buckets, self.cfg.lr)
    }
}

const V: usize = 256;
const BETA1: f64 = 0.9;
const BETA2: f64 = 0.999;
const EPS: f64 = 1e-8;

pub struct Learner {
    pub cfg: LearnerConfig,
    /// Order-major: `orders × buckets × 256`, then a 256-wide bias.
    theta: Vec<f32>,
    m: Vec<f32>,
    v: Vec<f32>,
    /// Parameters at `reference_step`, the origin of `δθ`.
    reference: Vec<f32>,
    reference_step: u64,
    /// The next reference, taken when training last doubled.
    candidate: Vec<f32>,
    candidate_step: u64,
    step: u64,
    pub bytes_seen: u64,
}

fn hash(ctx: &[u8], order: usize, buckets: usize) -> usize {
    // FNV-1a over the context, salted by order so orders do not collide by
    // construction.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ (order as u64).wrapping_mul(0x9E37_79B9);
    for &b in ctx {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    (h as usize) & (buckets - 1)
}

impl Learner {
    pub fn new(cfg: LearnerConfig) -> Learner {
        let n = cfg.orders * (1usize << cfg.log2_buckets) * V + V;
        Learner {
            cfg,
            theta: vec![0.0; n],
            m: vec![0.0; n],
            v: vec![0.0; n],
            reference: vec![0.0; n],
            reference_step: 0,
            candidate: vec![0.0; n],
            candidate_step: 0,
            step: 0,
            bytes_seen: 0,
        }
    }

    pub fn parameters(&self) -> usize {
        self.theta.len()
    }

    fn buckets(&self) -> usize {
        1usize << self.cfg.log2_buckets
    }

    fn bias_base(&self) -> usize {
        self.cfg.orders * self.buckets() * V
    }

    /// Row offsets feeding the prediction of `bytes[t]`: one per order whose
    /// full context exists, plus the bias.
    fn rows(&self, bytes: &[u8], t: usize) -> Vec<usize> {
        let mut rows = Vec::with_capacity(self.cfg.orders + 1);
        for k in 1..=self.cfg.orders {
            if t >= k {
                let h = hash(&bytes[t - k..t], k, self.buckets());
                rows.push(((k - 1) * self.buckets() + h) * V);
            }
        }
        rows.push(self.bias_base());
        rows
    }

    fn probs(&self, rows: &[usize]) -> [f64; V] {
        let mut logits = [0.0f64; V];
        for &r in rows {
            for (y, l) in logits.iter_mut().enumerate() {
                *l += self.theta[r + y] as f64;
            }
        }
        let m = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mut z = 0.0;
        for l in logits.iter_mut() {
            *l = (*l - m).exp();
            z += *l;
        }
        for l in logits.iter_mut() {
            *l /= z;
        }
        logits
    }

    /// Mean negative log-likelihood of `bytes`, in bits per byte.
    pub fn bits_per_byte(&self, bytes: &[u8]) -> f64 {
        if bytes.is_empty() {
            return f64::NAN;
        }
        let mut nll = 0.0;
        for t in 0..bytes.len() {
            let p = self.probs(&self.rows(bytes, t));
            nll -= p[bytes[t] as usize].max(1e-300).ln();
        }
        nll / bytes.len() as f64 / std::f64::consts::LN_2
    }

    /// Learning progress per byte: `|⟨∇L(y), P ⊙ (θ − θ̄)⟩| / |y|`.
    pub fn progress(&self, bytes: &[u8]) -> f64 {
        if bytes.is_empty() {
            return 0.0;
        }
        let mut dot = 0.0f64;
        for t in 0..bytes.len() {
            let rows = self.rows(bytes, t);
            let p = self.probs(&rows);
            for &r in &rows {
                for (y, &py) in p.iter().enumerate() {
                    let g = py - if y == bytes[t] as usize { 1.0 } else { 0.0 };
                    let i = r + y;
                    let precond = 1.0 / ((self.v[i] as f64).sqrt() + 1e-3);
                    dot += g * precond * (self.theta[i] - self.reference[i]) as f64;
                }
            }
        }
        dot.abs() / bytes.len() as f64
    }

    /// One Adam step on the summed loss of every sequence in `batch`.
    pub fn train(&mut self, batch: &[&[u8]]) {
        use std::collections::HashMap;
        let mut grad: HashMap<usize, f64> = HashMap::new();
        let mut count = 0usize;
        for bytes in batch {
            for t in 0..bytes.len() {
                let rows = self.rows(bytes, t);
                let p = self.probs(&rows);
                for &r in &rows {
                    for (y, &py) in p.iter().enumerate() {
                        let g = py - if y == bytes[t] as usize { 1.0 } else { 0.0 };
                        *grad.entry(r + y).or_insert(0.0) += g;
                    }
                }
                count += 1;
            }
            self.bytes_seen += bytes.len() as u64;
        }
        if count == 0 {
            return;
        }
        self.step += 1;
        let bc1 = 1.0 - BETA1.powi(self.step as i32);
        let bc2 = 1.0 - BETA2.powi(self.step as i32);
        let scale = 1.0 / count as f64;
        // Sparse Adam: only rows this batch touched move, which is what makes
        // the step cost proportional to the data rather than the table.
        for (i, g) in grad {
            let g = g * scale;
            let m = BETA1 * self.m[i] as f64 + (1.0 - BETA1) * g;
            let v = BETA2 * self.v[i] as f64 + (1.0 - BETA2) * g * g;
            self.m[i] = m as f32;
            self.v[i] = v as f32;
            let update = self.cfg.lr * (m / bc1) / ((v / bc2).sqrt() + EPS);
            self.theta[i] -= update as f32;
        }
        // Each time training doubles, the candidate becomes the reference and
        // the current parameters become the candidate — so the reference always
        // lies between step/4 and step/2. Dense copies, but only O(log steps)
        // of them over a whole run.
        if self.step >= 2 * self.candidate_step {
            std::mem::swap(&mut self.reference, &mut self.candidate);
            self.reference_step = self.candidate_step;
            self.candidate.copy_from_slice(&self.theta);
            self.candidate_step = self.step;
        }
    }

    /// Loss gradient at one parameter by finite differences, for testing.
    #[cfg(test)]
    fn numeric_grad(&mut self, bytes: &[u8], i: usize) -> f64 {
        let h = 1e-3f32;
        let loss = |l: &Learner| l.bits_per_byte(bytes) * std::f64::consts::LN_2 * bytes.len() as f64;
        self.theta[i] += h;
        let up = loss(self);
        self.theta[i] -= 2.0 * h;
        let down = loss(self);
        self.theta[i] += h;
        (up - down) / (2.0 * h as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> Learner {
        Learner::new(LearnerConfig { orders: 2, log2_buckets: 6, lr: 0.05 })
    }

    #[test]
    fn untrained_is_uniform_eight_bits() {
        let l = tiny();
        assert!((l.bits_per_byte(b"hello world") - 8.0).abs() < 1e-9);
    }

    #[test]
    fn training_lowers_bits_per_byte_on_structure() {
        let mut l = tiny();
        let data: Vec<u8> = b"abcabcabcabcabcabcabcabc".to_vec();
        let before = l.bits_per_byte(&data);
        for _ in 0..200 {
            l.train(&[&data]);
        }
        let after = l.bits_per_byte(&data);
        assert!(after < before * 0.25, "{before} -> {after}");
    }

    #[test]
    fn analytic_gradient_matches_finite_differences() {
        let mut l = tiny();
        let data = b"xyzzyxy".to_vec();
        for _ in 0..5 {
            l.train(&[&data]);
        }
        // The analytic gradient of the summed NLL at the bias of byte 'x'.
        let i = l.bias_base() + b'x' as usize;
        let mut analytic = 0.0;
        for t in 0..data.len() {
            let rows = l.rows(&data, t);
            let p = l.probs(&rows);
            analytic += p[b'x' as usize] - if data[t] == b'x' { 1.0 } else { 0.0 };
        }
        let numeric = l.numeric_grad(&data, i);
        assert!((analytic - numeric).abs() < 1e-2, "analytic {analytic} vs numeric {numeric}");
    }

    #[test]
    fn the_reference_lags_between_a_quarter_and_a_half_of_training() {
        let mut l = tiny();
        for _ in 0..100 {
            l.train(&[b"abc"]);
            let (e, r) = (l.step, l.reference_step);
            assert!(r * 2 <= e && (e <= 2 || r * 4 >= e), "step {e}, reference {r}");
        }
    }

    #[test]
    fn progress_is_zero_before_the_learner_has_moved() {
        let l = tiny();
        assert_eq!(l.progress(b"anything"), 0.0);
    }

    #[test]
    fn progress_favours_what_the_learner_is_learning_over_noise() {
        let mut l = tiny();
        let pattern: Vec<u8> = b"0101010101010101".to_vec();
        for _ in 0..20 {
            l.train(&[&pattern]);
        }
        let on = l.progress(&pattern);
        let mut rng = crate::grammar::Rng(3);
        let noise: Vec<u8> = (0..16).map(|_| rng.next_u64() as u8).collect();
        let off = l.progress(&noise);
        assert!(on > off, "on-frontier {on} vs noise {off}");
    }
}
