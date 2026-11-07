//! The entropy-chunked learner (plan L1), a small BLT-like byte model.
//!
//! ## Chunks, from the n-gram's own uncertainty
//!
//! The hashed n-gram [`Learner`] gives each position `t` a next-byte
//! distribution from `bytes[..t]`. Its entropy `H(t)`, in bits, marks where a
//! chunk may end: a boundary falls before byte `t` when `H(t) > θ`. `H(t)`
//! depends only on the bytes before `t`, so the segmentation is causal, and a
//! decoder that has read `bytes[..t]` can recompute every boundary before `t`.
//!
//! `θ` is the `(1 − 1/L)` quantile of `H` over the training batch, where `L` is
//! the configured target mean chunk length. It is taken after the batch's
//! update, so it describes the model that will code the next batch, and each
//! call codes its batch with the `θ` the previous call left behind. That is the
//! same `θ` a fresh coder would use, so training and scoring agree. `H` does not
//! depend on the chunking, so the boundary rate on the batch is `1/L` and the
//! mean chunk length is `L` up to ties.
//!
//! ## Prediction: a two-expert mixture
//!
//! Both experts are proper distributions over the 256 byte values.
//!
//! * **(a)** the byte-level n-gram, `P_a`, with the same orders and buckets as
//!   the plain `ng` learner.
//! * **(b)** the chunk-conditioned predictor: a hashed table of byte counts,
//!   keyed by `(hash of the previous complete chunk, bytes so far in the current
//!   chunk)`. Its distribution is a Dirichlet back-off to the n-gram,
//!   `P_b(y) = (n_y + β·P_a(y)) / (N + β)` with `β = 1`, where `N` is the key's
//!   total count. An unseen key therefore predicts exactly what (a) does. A
//!   uniform back-off was tried first, and on the baseline's final held-out
//!   figures it was 0.9 to 1.4 bits per byte worse: an unseen key then costs the
//!   mixture a constant penalty that one global weight cannot avoid.
//!
//! The mixture is linear, `P = w_a P_a + w_b P_b`, which is a proper
//! distribution whenever the weights sum to one. Linear rather than log-linear,
//! because the log-domain mixture would need a renormalising sum over the 256
//! symbols at every byte anyway, and the linear form keeps each expert's
//! probability for the actual byte in view. The weights are a Bayesian posterior
//! over the two experts, updated once per byte by `w_i ← w_i·P_i(y)`,
//! renormalised, then mixed 2% toward uniform (a fixed share), so an expert that
//! loses early keeps at least 1% and can recover.
//!
//! The weights start from their value at the end of training, and keep moving
//! on the bytes they are scored on, in [`EntropyLearner::bits_per_byte`] and in
//! the predictor. Each update uses only the bytes before it, so a decoder can
//! replay it, and the coding cost stays honest. Frozen weights were measured to
//! fail out of distribution: trained on the self-play pool, the chunk table
//! held 99% of the weight and scored README at 9.52 bits per byte, above uniform.
//! With the weights moving, the same run scores README at 8.00, against the
//! n-gram's 7.99.
//!
//! The n-gram's rows are trained by Adam as in [`Learner`]. The chunk counts are
//! not gradient-trained, so [`ByteLearner::progress`] reports the n-gram's
//! learning progress only.
//!
//! ## Honest cost
//!
//! [`ByteLearner::bits_per_byte`] is the mean of `−log2 P(byte)` under the
//! mixture, with each position's chunk state built from the bytes before it, so
//! it is the cost a causal coder with the same model would pay.

use crate::learner::{ByteLearner, EntropyConfig, Learner, LearnerConfig, Predictor};

const V: usize = 256;
/// The fixed-share rate: each weight is mixed toward uniform by this much, so
/// neither expert's weight falls below `SHARE / 2`.
const SHARE: f64 = 0.02;
/// The Dirichlet pseudo-count that backs the chunk table off to the n-gram.
const BETA: f64 = 1.0;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// FNV-1a, continued from state `h`.
fn fnv(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Entropy of `p` in bits.
fn entropy_bits(p: &[f64; V]) -> f64 {
    -p.iter().filter(|&&q| q > 0.0).map(|&q| q * q.log2()).sum::<f64>()
}

/// Where the current chunk starts, and the hash of the chunk before it.
#[derive(Default)]
struct Segmenter {
    prev: u64,
    start: usize,
}

impl Segmenter {
    /// Begin byte `t`, closing the open chunk first if `cut` and it is nonempty.
    fn advance(&mut self, bytes: &[u8], t: usize, cut: bool) {
        if cut && t > self.start {
            self.prev = fnv(FNV_OFFSET, &bytes[self.start..t]);
            self.start = t;
        }
    }
}

/// What the model knows at one position, once that position's chunk is decided.
struct Step {
    ngram: [f64; V],
    bucket: usize,
}

pub struct EntropyLearner {
    pub cfg: EntropyConfig,
    ngram: Learner,
    /// Byte counts per chunk-table bucket, `buckets × 256`.
    counts: Vec<u32>,
    /// Total count per bucket.
    totals: Vec<u64>,
    /// Entropy threshold `θ` in bits, from the last train call. `+∞` until
    /// trained, so the untrained model never cuts a chunk.
    threshold: f64,
    /// Mixture weights of (n-gram, chunk predictor); they sum to one.
    weight: [f64; 2],
    bytes_seen: u64,
}

impl EntropyLearner {
    pub fn new(cfg: EntropyConfig) -> EntropyLearner {
        let buckets = 1usize << cfg.log2_buckets;
        EntropyLearner {
            cfg,
            ngram: Learner::new(LearnerConfig {
                orders: cfg.orders,
                log2_buckets: cfg.log2_buckets,
                lr: cfg.lr,
            }),
            counts: vec![0; buckets * V],
            totals: vec![0; buckets],
            threshold: f64::INFINITY,
            weight: [0.5, 0.5],
            bytes_seen: 0,
        }
    }

    fn buckets(&self) -> usize {
        self.totals.len()
    }

    fn bucket(&self, prev: u64, partial: &[u8]) -> usize {
        // The previous chunk's hash is fixed-width, so the two parts cannot blur.
        let h = fnv(fnv(FNV_OFFSET, &prev.to_le_bytes()), partial);
        (h as usize) & (self.buckets() - 1)
    }

    /// Expert (b) at byte `y`, backing off to the n-gram's `pa_y` (see the docs).
    fn chunk_prob(&self, bucket: usize, y: usize, pa_y: f64) -> f64 {
        (self.counts[bucket * V + y] as f64 + BETA * pa_y) / (self.totals[bucket] as f64 + BETA)
    }

    /// Advance `seg` to position `t` and return what is known there.
    fn step(&self, seg: &mut Segmenter, bytes: &[u8], t: usize, threshold: f64) -> Step {
        let ngram = self.ngram.distribution(bytes, t);
        seg.advance(bytes, t, entropy_bits(&ngram) > threshold);
        let bucket = self.bucket(seg.prev, &bytes[seg.start..t]);
        Step { ngram, bucket }
    }

    /// The mixture at byte `y` under weights `w`.
    fn mix(&self, w: [f64; 2], s: &Step, y: usize) -> f64 {
        w[0] * s.ngram[y] + w[1] * self.chunk_prob(s.bucket, y, s.ngram[y])
    }

    /// The weights after byte `y`, which `s` predicted: the Bayes posterior
    /// `w_i ← w_i·P_i(y)`, renormalised, then mixed toward uniform by the fixed share.
    fn posterior(&self, w: [f64; 2], s: &Step, y: usize) -> [f64; 2] {
        let a = w[0] * s.ngram[y];
        let b = w[1] * self.chunk_prob(s.bucket, y, s.ngram[y]);
        let z = a + b;
        if !(z > 0.0) {
            return w;
        }
        [(1.0 - SHARE) * a / z + SHARE / 2.0, (1.0 - SHARE) * b / z + SHARE / 2.0]
    }

    /// The `θ` that puts a fraction `1/L` of positions above it, over `batch`.
    fn quantile_threshold(&self, batch: &[&[u8]]) -> f64 {
        let mut hs: Vec<f64> = Vec::new();
        for seq in batch {
            for t in 1..seq.len() {
                hs.push(entropy_bits(&self.ngram.distribution(seq, t)));
            }
        }
        if hs.is_empty() {
            return self.threshold;
        }
        hs.sort_by(f64::total_cmp);
        let k = (hs.len() as f64 * (1.0 - 1.0 / self.cfg.target_len)).floor() as usize;
        hs[k.min(hs.len() - 1)]
    }

    /// Mean negative log-likelihood of `bytes`, in bits per byte.
    pub fn bits_per_byte(&self, bytes: &[u8]) -> f64 {
        if bytes.is_empty() {
            return f64::NAN;
        }
        let mut seg = Segmenter::default();
        let mut w = self.weight;
        let mut nll = 0.0;
        for t in 0..bytes.len() {
            let s = self.step(&mut seg, bytes, t, self.threshold);
            let y = bytes[t] as usize;
            nll -= self.mix(w, &s, y).max(1e-300).log2();
            w = self.posterior(w, &s, y);
        }
        nll / bytes.len() as f64
    }

    /// One train call: code `batch` with the current `θ`, taking a posterior
    /// step on the mixture weights and counting each byte as it is seen; then
    /// one Adam step on the n-gram; then set `θ` for the next call from the
    /// updated model.
    pub fn train(&mut self, batch: &[&[u8]]) {
        let theta = self.threshold;
        for seq in batch {
            let mut seg = Segmenter::default();
            for t in 0..seq.len() {
                let s = self.step(&mut seg, seq, t, theta);
                let y = seq[t] as usize;
                self.weight = self.posterior(self.weight, &s, y);
                let i = s.bucket * V + y;
                self.counts[i] += 1;
                self.totals[s.bucket] += 1;
            }
            self.bytes_seen += seq.len() as u64;
        }
        self.ngram.train(batch);
        self.threshold = self.quantile_threshold(batch);
    }

    /// The full mixture at position `t`, replayed from the start of `bytes`.
    #[cfg(test)]
    fn prediction(&self, bytes: &[u8], t: usize) -> [f64; V] {
        let mut seg = Segmenter::default();
        let mut w = self.weight;
        let mut last = None;
        for u in 0..=t {
            let s = self.step(&mut seg, bytes, u, self.threshold);
            if u < t {
                w = self.posterior(w, &s, bytes[u] as usize);
            }
            last = Some(s);
        }
        let s = last.expect("t is a position of bytes");
        let mut out = [0.0; V];
        for (y, o) in out.iter_mut().enumerate() {
            *o = self.mix(w, &s, y);
        }
        out
    }

    /// Mean chunk length over `bytes` under the current threshold.
    #[cfg(test)]
    fn mean_chunk_len(&self, bytes: &[u8]) -> f64 {
        let mut seg = Segmenter::default();
        let mut cuts = 0usize;
        for t in 0..bytes.len() {
            let before = seg.start;
            self.step(&mut seg, bytes, t, self.threshold);
            if seg.start != before {
                cuts += 1;
            }
        }
        bytes.len() as f64 / (cuts + 1) as f64
    }
}

impl ByteLearner for EntropyLearner {
    fn bits_per_byte(&self, bytes: &[u8]) -> f64 {
        EntropyLearner::bits_per_byte(self, bytes)
    }
    fn progress(&self, bytes: &[u8]) -> f64 {
        self.ngram.progress(bytes)
    }
    fn train(&mut self, batch: &[&[u8]]) {
        EntropyLearner::train(self, batch)
    }
    fn parameters(&self) -> usize {
        self.ngram.parameters() + self.counts.len()
    }
    fn bytes_seen(&self) -> u64 {
        self.bytes_seen
    }
    fn describe(&self) -> String {
        format!(
            "entropy {}×2^{} L{} lr{}",
            self.cfg.orders, self.cfg.log2_buckets, self.cfg.target_len, self.cfg.lr
        )
    }
    fn predictor(&self) -> Option<Box<dyn Predictor + '_>> {
        Some(Box::new(EntropyPredictor::new(self)))
    }
}

/// The entropy learner's predictor (plan P6). It keeps the chunk segmentation
/// as it goes, so each byte costs one step, and it reads the same mixture
/// [`EntropyLearner::bits_per_byte`] does.
struct EntropyPredictor<'a> {
    learner: &'a EntropyLearner,
    history: Vec<u8>,
    seg: Segmenter,
    /// The step at the current position, `history.len()`.
    cur: Step,
    /// The mixture weights after the bytes so far, as [`EntropyLearner::bits_per_byte`] has them.
    weight: [f64; 2],
}

impl<'a> EntropyPredictor<'a> {
    fn new(learner: &'a EntropyLearner) -> EntropyPredictor<'a> {
        let mut seg = Segmenter::default();
        let cur = learner.step(&mut seg, &[], 0, learner.threshold);
        EntropyPredictor { learner, history: Vec::new(), seg, cur, weight: learner.weight }
    }
}

impl Predictor for EntropyPredictor<'_> {
    fn distribution(&self) -> [f64; 256] {
        let mut out = [0.0; V];
        for (y, o) in out.iter_mut().enumerate() {
            *o = self.learner.mix(self.weight, &self.cur, y);
        }
        out
    }
    fn push(&mut self, byte: u8) {
        self.weight = self.learner.posterior(self.weight, &self.cur, byte as usize);
        self.history.push(byte);
        let t = self.history.len();
        self.cur = self.learner.step(&mut self.seg, &self.history, t, self.learner.threshold);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const README: &[u8] = include_bytes!("../../README.md");

    fn tiny(target_len: f64) -> EntropyLearner {
        EntropyLearner::new(EntropyConfig {
            orders: 2,
            log2_buckets: 8,
            target_len,
            lr: 0.05,
        })
    }

    /// Text chunks of `len` bytes from the README, the way the baseline trains.
    fn chunks(len: usize, n: usize) -> Vec<&'static [u8]> {
        (0..n).map(|i| &README[i * len..(i + 1) * len]).collect()
    }

    #[test]
    fn untrained_is_uniform_eight_bits() {
        let l = tiny(4.0);
        assert!((l.bits_per_byte(b"hello world") - 8.0).abs() < 1e-9);
    }

    #[test]
    fn the_prediction_is_a_distribution_over_256_bytes() {
        let mut l = tiny(4.0);
        let text = &README[..1024];
        assert!((l.prediction(text, 0).iter().sum::<f64>() - 1.0).abs() < 1e-9);
        for _ in 0..3 {
            l.train(&[&README[..256], &README[256..512]]);
        }
        for t in [0usize, 1, 10, 100, 500, 1023] {
            let sum: f64 = l.prediction(text, t).iter().sum();
            assert!((sum - 1.0).abs() < 1e-9, "position {t} sums to {sum}");
        }
        let w = l.weight;
        assert!(
            (w[0] + w[1] - 1.0).abs() < 1e-12 && w[0] >= SHARE / 2.0 && w[1] >= SHARE / 2.0,
            "weights {w:?}"
        );
    }

    #[test]
    fn bits_per_byte_is_the_mean_cost_of_the_replayed_prediction() {
        let mut l = tiny(4.0);
        for _ in 0..4 {
            l.train(&[&README[..256], &README[256..512]]);
        }
        let text = &README[..200];
        let replayed: f64 = (0..text.len())
            .map(|t| -l.prediction(text, t)[text[t] as usize].log2())
            .sum::<f64>()
            / text.len() as f64;
        assert!(
            (l.bits_per_byte(text) - replayed).abs() < 1e-9,
            "{} vs {replayed}",
            l.bits_per_byte(text)
        );
    }

    #[test]
    fn same_input_same_bits_per_byte() {
        let run = || {
            let mut l = tiny(4.0);
            for _ in 0..3 {
                l.train(&[&README[..256], &README[256..512]]);
            }
            (l.bits_per_byte(&README[..512]), l.bits_per_byte(&README[..512]))
        };
        let (a, b) = run();
        let (c, d) = run();
        assert_eq!(a.to_bits(), b.to_bits());
        assert_eq!(a.to_bits(), c.to_bits());
        assert_eq!(a.to_bits(), d.to_bits());
    }

    #[test]
    fn the_learned_threshold_gives_the_target_mean_chunk_length() {
        let target = 4.0;
        let mut l = tiny(target);
        for _ in 0..8 {
            for c in chunks(256, 16) {
                l.train(&[c]);
            }
        }
        let sample = &README[..4096];
        let mean = l.mean_chunk_len(sample);
        eprintln!("target {target}: mean chunk length {mean:.3} on the README's first 4 KiB");
        assert!(
            (mean - target).abs() <= 0.25 * target,
            "mean chunk length {mean}, target {target}"
        );
    }

    #[test]
    fn a_repeated_excerpt_is_not_coded_worse_than_the_plain_ngram() {
        // A README excerpt, repeated: the chunk table has real repeats to learn.
        let excerpt = &README[..512];
        let repeated: Vec<u8> = excerpt.repeat(8);
        let cfg = LearnerConfig {
            orders: 3,
            log2_buckets: 10,
            lr: 0.05,
        };
        let mut ent = EntropyLearner::new(EntropyConfig {
            orders: cfg.orders,
            log2_buckets: cfg.log2_buckets,
            target_len: 4.0,
            lr: cfg.lr,
        });
        let mut ng = Learner::new(cfg);
        for _ in 0..4 {
            for c in repeated.chunks(256) {
                ent.train(&[c]);
                ng.train(&[c]);
            }
        }
        let (e, n) = (ent.bits_per_byte(&repeated), ng.bits_per_byte(&repeated));
        eprintln!("repeated excerpt: entropy {e:.4} bits/byte, ngram {n:.4} bits/byte");
        assert!(e <= n + 0.05, "entropy {e} worse than ngram {n} by more than 0.05");
    }

    #[test]
    fn the_chunk_expert_is_a_distribution_for_every_key() {
        // P_b(y) = (n_y + β·P_a(y)) / (N + β) sums to one over y, but only if the
        // back-off is normalised too. Checked for keys the pool has seen often
        // and for text it never contains, at every position.
        let mut l = tiny(4.0);
        for _ in 0..8 {
            l.train(&[&README[..256], &README[256..512]]);
        }
        let pool_key = &README[..512];
        let unseen = b"zq#~ zq#~ zq#~ zq#~ zq#~ zq#~ zq#~ zq#~".as_slice();
        let mut keys = 0usize;
        for text in [pool_key, unseen] {
            let mut seg = Segmenter::default();
            for t in 0..text.len() {
                let s = l.step(&mut seg, text, t, l.threshold);
                let sum_a: f64 = s.ngram.iter().sum();
                let sum_b: f64 = (0..V).map(|y| l.chunk_prob(s.bucket, y, s.ngram[y])).sum();
                assert!((sum_a - 1.0).abs() < 1e-9, "n-gram sums to {sum_a} at {t}");
                assert!((sum_b - 1.0).abs() < 1e-9, "chunk expert sums to {sum_b} at {t}");
                keys += 1;
            }
        }
        assert_eq!(keys, pool_key.len() + unseen.len());
    }

    #[test]
    fn out_of_distribution_the_mixture_follows_the_ngram() {
        // A chunk table that has memorised a pool gives each of its keys a large
        // count N, and an unseen byte in such a key costs log2(N + 1) bits over
        // the n-gram. Scored on text the pool never contains, a mixture whose
        // weights stayed frozen would pay that on every byte. The weights keep
        // moving on the bytes they score, so the mixture should cost what the
        // n-gram costs, and not more.
        let pool: Vec<u8> = b"0123456789".repeat(40);
        let mut ent = EntropyLearner::new(EntropyConfig { orders: 2, log2_buckets: 8, target_len: 4.0, lr: 0.05 });
        let mut ng = Learner::new(LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 });
        for _ in 0..4 {
            for c in pool.chunks(64) {
                ent.train(&[c]);
                ng.train(&[c]);
            }
        }
        let text = b"the quick brown fox jumps over the lazy dog. ".repeat(4);
        let (e, n) = (ent.bits_per_byte(&text), ng.bits_per_byte(&text));
        eprintln!("out of distribution: entropy {e:.4} bits/byte, ngram {n:.4} bits/byte");
        assert!(e <= n + 0.1, "entropy {e} above the ngram {n} by more than 0.1 out of distribution");
    }

    #[test]
    fn the_predictor_scores_what_bits_per_byte_scores_as_the_weights_move() {
        // Generation runs the predictor one byte at a time; its weights must move
        // on each pushed byte exactly as `bits_per_byte` moves them while scoring.
        let mut l = tiny(4.0);
        for _ in 0..4 {
            l.train(&[&README[..256], &README[256..512]]);
        }
        let text = b"0123456789 zyx 0123456789 zyx";
        let mut p = ByteLearner::predictor(&l).expect("the entropy learner has a predictor");
        let mut nll = 0.0;
        for &y in text.iter() {
            nll -= p.distribution()[y as usize].log2();
            p.push(y);
        }
        let mean = nll / text.len() as f64;
        assert!((mean - l.bits_per_byte(text)).abs() < 1e-9, "predictor {mean} vs scored {}", l.bits_per_byte(text));
    }

    #[test]
    fn the_learner_reports_its_kind_and_shape() {
        let l = tiny(4.0);
        let d = ByteLearner::describe(&l);
        assert!(d.starts_with("entropy 2×2^8 L4"), "{d}");
        assert!(ByteLearner::parameters(&l) > 0);
    }
}
