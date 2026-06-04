//! Neural Network Loss Functions Module
//!
//! Provides standard loss functions for training neural networks.

/// Reduction mode for loss functions
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reduction {
    /// No reduction applied
    None,
    /// Mean reduction
    Mean,
    /// Sum reduction
    Sum,
}

/// Trait for all loss functions
pub trait Loss {
    /// Compute the loss
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32>;
    /// Compute gradient of loss w.r.t. input
    fn backward(&self, input: &[f32], target: &[f32]) -> Vec<f32>;
}

/// Mean Squared Error Loss
pub struct MSELoss {
    reduction: Reduction,
}

impl MSELoss {
    /// Create a new MSELoss
    pub fn new() -> Self {
        Self {
            reduction: Reduction::Mean,
        }
    }
    /// Set reduction mode
    pub fn reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }
}

impl Default for MSELoss {
    fn default() -> Self {
        Self::new()
    }
}

impl Loss for MSELoss {
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let losses: Vec<f32> = input
            .iter()
            .zip(target.iter())
            .map(|(i, t)| (i - t).powi(2))
            .collect();
        match self.reduction {
            Reduction::None => losses,
            Reduction::Mean => vec![losses.iter().sum::<f32>() / losses.len() as f32],
            Reduction::Sum => vec![losses.iter().sum()],
        }
    }
    fn backward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let n = input.len() as f32;
        input
            .iter()
            .zip(target.iter())
            .map(|(i, t)| match self.reduction {
                Reduction::None => 2.0 * (i - t),
                Reduction::Mean => 2.0 * (i - t) / n,
                Reduction::Sum => 2.0 * (i - t),
            })
            .collect()
    }
}

/// L1 Loss (Mean Absolute Error)
pub struct L1Loss {
    reduction: Reduction,
}

impl L1Loss {
    /// Create a new L1Loss
    pub fn new() -> Self {
        Self {
            reduction: Reduction::Mean,
        }
    }
    /// Set reduction mode
    pub fn reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }
}

impl Default for L1Loss {
    fn default() -> Self {
        Self::new()
    }
}

impl Loss for L1Loss {
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let losses: Vec<f32> = input
            .iter()
            .zip(target.iter())
            .map(|(i, t)| (i - t).abs())
            .collect();
        match self.reduction {
            Reduction::None => losses,
            Reduction::Mean => vec![losses.iter().sum::<f32>() / losses.len() as f32],
            Reduction::Sum => vec![losses.iter().sum()],
        }
    }
    fn backward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let n = input.len() as f32;
        input
            .iter()
            .zip(target.iter())
            .map(|(i, t)| {
                let sign = if i > t {
                    1.0
                } else if i < t {
                    -1.0
                } else {
                    0.0
                };
                match self.reduction {
                    Reduction::None => sign,
                    Reduction::Mean => sign / n,
                    Reduction::Sum => sign,
                }
            })
            .collect()
    }
}

/// Smooth L1 Loss (Huber Loss)
pub struct SmoothL1Loss {
    beta: f32,
    reduction: Reduction,
}

impl SmoothL1Loss {
    /// Create a new SmoothL1Loss
    pub fn new() -> Self {
        Self {
            beta: 1.0,
            reduction: Reduction::Mean,
        }
    }
    /// Set beta threshold
    pub fn beta(mut self, beta: f32) -> Self {
        self.beta = beta;
        self
    }
    /// Set reduction mode
    pub fn reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }
}

impl Default for SmoothL1Loss {
    fn default() -> Self {
        Self::new()
    }
}

impl Loss for SmoothL1Loss {
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let losses: Vec<f32> = input
            .iter()
            .zip(target.iter())
            .map(|(i, t)| {
                let diff = (i - t).abs();
                if diff < self.beta {
                    0.5 * diff * diff / self.beta
                } else {
                    diff - 0.5 * self.beta
                }
            })
            .collect();
        match self.reduction {
            Reduction::None => losses,
            Reduction::Mean => vec![losses.iter().sum::<f32>() / losses.len() as f32],
            Reduction::Sum => vec![losses.iter().sum()],
        }
    }
    fn backward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let n = input.len() as f32;
        input
            .iter()
            .zip(target.iter())
            .map(|(i, t)| {
                let diff = i - t;
                let g = if diff.abs() < self.beta {
                    diff / self.beta
                } else {
                    diff.signum()
                };
                match self.reduction {
                    Reduction::None => g,
                    Reduction::Mean => g / n,
                    Reduction::Sum => g,
                }
            })
            .collect()
    }
}

/// Binary Cross Entropy Loss (requires sigmoid-activated inputs)
pub struct BCELoss {
    reduction: Reduction,
}

impl BCELoss {
    /// Create a new BCELoss
    pub fn new() -> Self {
        Self {
            reduction: Reduction::Mean,
        }
    }
    /// Set reduction mode
    pub fn reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }
}

impl Default for BCELoss {
    fn default() -> Self {
        Self::new()
    }
}

impl Loss for BCELoss {
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let eps = 1e-7;
        let losses: Vec<f32> = input
            .iter()
            .zip(target.iter())
            .map(|(i, t)| {
                let p = i.clamp(eps, 1.0 - eps);
                -(t * p.ln() + (1.0 - t) * (1.0 - p).ln())
            })
            .collect();
        match self.reduction {
            Reduction::None => losses,
            Reduction::Mean => vec![losses.iter().sum::<f32>() / losses.len() as f32],
            Reduction::Sum => vec![losses.iter().sum()],
        }
    }
    fn backward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let eps = 1e-7;
        let n = input.len() as f32;
        input
            .iter()
            .zip(target.iter())
            .map(|(i, t)| {
                let p = i.clamp(eps, 1.0 - eps);
                let g = -t / p + (1.0 - t) / (1.0 - p);
                match self.reduction {
                    Reduction::None => g,
                    Reduction::Mean => g / n,
                    Reduction::Sum => g,
                }
            })
            .collect()
    }
}

/// BCE with Logits Loss (applies sigmoid internally)
pub struct BCEWithLogitsLoss {
    reduction: Reduction,
}

impl BCEWithLogitsLoss {
    /// Create a new BCEWithLogitsLoss
    pub fn new() -> Self {
        Self {
            reduction: Reduction::Mean,
        }
    }
    /// Set reduction mode
    pub fn reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }
}

impl Default for BCEWithLogitsLoss {
    fn default() -> Self {
        Self::new()
    }
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

impl Loss for BCEWithLogitsLoss {
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let losses: Vec<f32> = input
            .iter()
            .zip(target.iter())
            .map(|(x, t)| x.max(0.0) - x * t + (1.0 + (-x.abs()).exp()).ln())
            .collect();
        match self.reduction {
            Reduction::None => losses,
            Reduction::Mean => vec![losses.iter().sum::<f32>() / losses.len() as f32],
            Reduction::Sum => vec![losses.iter().sum()],
        }
    }
    fn backward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let n = input.len() as f32;
        input
            .iter()
            .zip(target.iter())
            .map(|(x, t)| {
                let g = sigmoid(*x) - t;
                match self.reduction {
                    Reduction::None => g,
                    Reduction::Mean => g / n,
                    Reduction::Sum => g,
                }
            })
            .collect()
    }
}

/// Cross Entropy Loss (for classification)
pub struct CrossEntropyLoss {
    label_smoothing: f32,
    reduction: Reduction,
}

impl CrossEntropyLoss {
    /// Create a new CrossEntropyLoss
    pub fn new() -> Self {
        Self {
            label_smoothing: 0.0,
            reduction: Reduction::Mean,
        }
    }
    /// Set label smoothing
    pub fn label_smoothing(mut self, label_smoothing: f32) -> Self {
        self.label_smoothing = label_smoothing;
        self
    }
    /// Set reduction mode
    pub fn reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }

    /// Compute softmax probabilities
    pub fn softmax(logits: &[f32]) -> Vec<f32> {
        let max_val = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exps: Vec<f32> = logits.iter().map(|x| (x - max_val).exp()).collect();
        let sum: f32 = exps.iter().sum();
        exps.iter().map(|e| e / sum).collect()
    }

    /// Compute cross entropy for batch (logits: `[batch, classes]`, targets: `[batch]` as class indices)
    pub fn forward_batch(
        &self,
        logits: &[f32],
        targets: &[usize],
        batch_size: usize,
        num_classes: usize,
    ) -> Vec<f32> {
        let mut losses = Vec::with_capacity(batch_size);
        #[allow(clippy::needless_range_loop)]
        for b in 0..batch_size {
            let start = b * num_classes;
            let batch_logits = &logits[start..start + num_classes];
            let probs = Self::softmax(batch_logits);
            let target = targets[b];
            let smoothed_target: Vec<f32> = (0..num_classes)
                .map(|c| {
                    if c == target {
                        1.0 - self.label_smoothing + self.label_smoothing / num_classes as f32
                    } else {
                        self.label_smoothing / num_classes as f32
                    }
                })
                .collect();
            let loss: f32 = probs
                .iter()
                .zip(smoothed_target.iter())
                .map(|(p, t)| -t * p.ln())
                .sum();
            losses.push(loss);
        }
        match self.reduction {
            Reduction::None => losses,
            Reduction::Mean => vec![losses.iter().sum::<f32>() / losses.len() as f32],
            Reduction::Sum => vec![losses.iter().sum()],
        }
    }

    /// Compute gradients for batch
    pub fn backward_batch(
        &self,
        logits: &[f32],
        targets: &[usize],
        batch_size: usize,
        num_classes: usize,
    ) -> Vec<f32> {
        let n = batch_size as f32;
        let mut grads = vec![0.0; logits.len()];
        #[allow(clippy::needless_range_loop)]
        for b in 0..batch_size {
            let start = b * num_classes;
            let batch_logits = &logits[start..start + num_classes];
            let probs = Self::softmax(batch_logits);
            let target = targets[b];
            for c in 0..num_classes {
                let smoothed_t = if c == target {
                    1.0 - self.label_smoothing + self.label_smoothing / num_classes as f32
                } else {
                    self.label_smoothing / num_classes as f32
                };
                let g = probs[c] - smoothed_t;
                grads[start + c] = match self.reduction {
                    Reduction::None => g,
                    Reduction::Mean => g / n,
                    Reduction::Sum => g,
                };
            }
        }
        grads
    }
}

impl Default for CrossEntropyLoss {
    fn default() -> Self {
        Self::new()
    }
}

impl Loss for CrossEntropyLoss {
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let probs = Self::softmax(input);
        let loss: f32 = probs
            .iter()
            .zip(target.iter())
            .map(|(p, t)| -t * p.max(1e-7).ln())
            .sum();
        match self.reduction {
            Reduction::None => vec![loss],
            Reduction::Mean => vec![loss],
            Reduction::Sum => vec![loss],
        }
    }
    fn backward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let probs = Self::softmax(input);
        probs
            .iter()
            .zip(target.iter())
            .map(|(p, t)| p - t)
            .collect()
    }
}

/// Negative Log Likelihood Loss
pub struct NLLLoss {
    reduction: Reduction,
}

impl NLLLoss {
    /// Create a new NLLLoss
    pub fn new() -> Self {
        Self {
            reduction: Reduction::Mean,
        }
    }
    /// Set reduction mode
    pub fn reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }

    /// Forward for batch
    pub fn forward_batch(
        &self,
        log_probs: &[f32],
        targets: &[usize],
        batch_size: usize,
        num_classes: usize,
    ) -> Vec<f32> {
        let losses: Vec<f32> = (0..batch_size)
            .map(|b| -log_probs[b * num_classes + targets[b]])
            .collect();
        match self.reduction {
            Reduction::None => losses,
            Reduction::Mean => vec![losses.iter().sum::<f32>() / losses.len() as f32],
            Reduction::Sum => vec![losses.iter().sum()],
        }
    }

    /// Backward for batch
    pub fn backward_batch(
        &self,
        targets: &[usize],
        batch_size: usize,
        num_classes: usize,
    ) -> Vec<f32> {
        let n = batch_size as f32;
        let mut grads = vec![0.0; batch_size * num_classes];
        for b in 0..batch_size {
            let g = match self.reduction {
                Reduction::None => -1.0,
                Reduction::Mean => -1.0 / n,
                Reduction::Sum => -1.0,
            };
            grads[b * num_classes + targets[b]] = g;
        }
        grads
    }
}

impl Default for NLLLoss {
    fn default() -> Self {
        Self::new()
    }
}

impl Loss for NLLLoss {
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let loss: f32 = input.iter().zip(target.iter()).map(|(lp, t)| -t * lp).sum();
        match self.reduction {
            Reduction::None => vec![loss],
            Reduction::Mean => vec![loss / input.len() as f32],
            Reduction::Sum => vec![loss],
        }
    }
    fn backward(&self, _input: &[f32], target: &[f32]) -> Vec<f32> {
        let n = target.len() as f32;
        target
            .iter()
            .map(|t| match self.reduction {
                Reduction::None => -*t,
                Reduction::Mean => -*t / n,
                Reduction::Sum => -*t,
            })
            .collect()
    }
}

/// KL Divergence Loss
pub struct KLDivLoss {
    reduction: Reduction,
    log_target: bool,
}

impl KLDivLoss {
    /// Create a new KLDivLoss
    pub fn new() -> Self {
        Self {
            reduction: Reduction::Mean,
            log_target: false,
        }
    }
    /// Set reduction mode
    pub fn reduction(mut self, reduction: Reduction) -> Self {
        self.reduction = reduction;
        self
    }
    /// Set whether target is in log space
    pub fn log_target(mut self, log_target: bool) -> Self {
        self.log_target = log_target;
        self
    }
}

impl Default for KLDivLoss {
    fn default() -> Self {
        Self::new()
    }
}

impl Loss for KLDivLoss {
    fn forward(&self, input: &[f32], target: &[f32]) -> Vec<f32> {
        let losses: Vec<f32> = input
            .iter()
            .zip(target.iter())
            .map(|(log_p, t)| {
                let t_val = if self.log_target { t.exp() } else { *t };
                let t_log = if self.log_target { *t } else { t.ln() };
                t_val * (t_log - log_p)
            })
            .collect();
        match self.reduction {
            Reduction::None => losses,
            Reduction::Mean => vec![losses.iter().sum::<f32>() / losses.len() as f32],
            Reduction::Sum => vec![losses.iter().sum()],
        }
    }
    fn backward(&self, _input: &[f32], target: &[f32]) -> Vec<f32> {
        let n = target.len() as f32;
        target
            .iter()
            .map(|t| {
                let t_val = if self.log_target { t.exp() } else { *t };
                match self.reduction {
                    Reduction::None => -t_val,
                    Reduction::Mean => -t_val / n,
                    Reduction::Sum => -t_val,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mse_loss() {
        let loss = MSELoss::new();
        let input = vec![1.0, 2.0, 3.0];
        let target = vec![1.0, 2.0, 3.0];
        let l = loss.forward(&input, &target);
        assert!((l[0] - 0.0).abs() < 1e-6);
        let input2 = vec![0.0, 0.0, 0.0];
        let l2 = loss.forward(&input2, &target);
        assert!((l2[0] - 14.0 / 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_l1_loss() {
        let loss = L1Loss::new();
        let input = vec![1.0, 2.0, 3.0];
        let target = vec![0.0, 2.0, 5.0];
        let l = loss.forward(&input, &target);
        assert!((l[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_bce_loss() {
        let loss = BCELoss::new();
        let input = vec![0.5, 0.5];
        let target = vec![1.0, 0.0];
        let l = loss.forward(&input, &target);
        assert!(l[0] > 0.0);
    }

    #[test]
    fn test_cross_entropy() {
        let loss = CrossEntropyLoss::new();
        let logits = vec![2.0, 1.0, 0.1];
        let targets = vec![0usize];
        let l = loss.forward_batch(&logits, &targets, 1, 3);
        assert!(l[0] < 1.0);
    }

    #[test]
    fn test_softmax() {
        let logits = vec![1.0, 2.0, 3.0];
        let probs = CrossEntropyLoss::softmax(&logits);
        let sum: f32 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_smooth_l1_loss_forward() {
        let loss = SmoothL1Loss::new();
        let input = vec![1.5, 3.0];
        let target = vec![1.0, 1.0];
        let l = loss.forward(&input, &target);
        let expected = (0.125 + 1.5) / 2.0;
        assert!((l[0] - expected).abs() < 1e-6);
    }

    #[test]
    fn test_smooth_l1_loss_custom_beta() {
        let loss = SmoothL1Loss::new().beta(0.5);
        let input = vec![1.3];
        let target = vec![1.0];
        let l = loss.forward(&input, &target);
        let expected = 0.5 * 0.3_f32.powi(2) / 0.5;
        assert!((l[0] - expected).abs() < 1e-5);
    }

    #[test]
    fn test_smooth_l1_backward() {
        let loss = SmoothL1Loss::new();
        let input = vec![1.5, 3.0];
        let target = vec![1.0, 1.0];
        let grads = loss.backward(&input, &target);
        assert_eq!(grads.len(), 2);
        assert!((grads[0] - 0.5 / 2.0).abs() < 1e-6);
        assert!((grads[1] - 1.0 / 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_bce_with_logits_forward() {
        let loss = BCEWithLogitsLoss::new();
        let input = vec![0.0];
        let target = vec![1.0];
        let l = loss.forward(&input, &target);
        assert!((l[0] - 2.0_f32.ln()).abs() < 1e-5);
    }

    #[test]
    fn test_bce_with_logits_backward() {
        let loss = BCEWithLogitsLoss::new();
        let input = vec![0.0];
        let target = vec![1.0];
        let grads = loss.backward(&input, &target);
        assert!((grads[0] - (-0.5)).abs() < 1e-5);
    }

    #[test]
    fn test_cross_entropy_label_smoothing() {
        let loss = CrossEntropyLoss::new().label_smoothing(0.1);
        let logits = vec![2.0, 1.0, 0.1];
        let targets = vec![0usize];
        let l = loss.forward_batch(&logits, &targets, 1, 3);
        let loss_no_smooth = CrossEntropyLoss::new();
        let l2 = loss_no_smooth.forward_batch(&logits, &targets, 1, 3);
        assert!(l[0] > l2[0]);
    }

    #[test]
    fn test_cross_entropy_backward_batch() {
        let loss = CrossEntropyLoss::new();
        let logits = vec![2.0, 1.0, 0.1];
        let targets = vec![0usize];
        let grads = loss.backward_batch(&logits, &targets, 1, 3);
        assert_eq!(grads.len(), 3);
        assert!(grads[0] < 0.0);
        assert!(grads[1] > 0.0);
        assert!(grads[2] > 0.0);
        let sum: f32 = grads.iter().sum();
        assert!(sum.abs() < 1e-5);
    }

    #[test]
    fn test_nll_loss_forward() {
        let loss = NLLLoss::new();
        let log_probs = vec![0.7_f32.ln(), 0.2_f32.ln(), 0.1_f32.ln()];
        let target = vec![1.0, 0.0, 0.0];
        let l = loss.forward(&log_probs, &target);
        let expected = -(1.0 * 0.7_f32.ln()) / 3.0;
        assert!((l[0] - expected).abs() < 1e-5);
    }

    #[test]
    fn test_nll_loss_batch() {
        let loss = NLLLoss::new();
        let log_probs = vec![0.7_f32.ln(), 0.2_f32.ln(), 0.1_f32.ln(), 0.1_f32.ln(), 0.8_f32.ln(), 0.1_f32.ln()];
        let targets = vec![0usize, 1usize];
        let l = loss.forward_batch(&log_probs, &targets, 2, 3);
        let expected = (-0.7_f32.ln() + -0.8_f32.ln()) / 2.0;
        assert!((l[0] - expected).abs() < 1e-5);
    }

    #[test]
    fn test_nll_backward_batch() {
        let loss = NLLLoss::new();
        let targets = vec![0usize, 2usize];
        let grads = loss.backward_batch(&targets, 2, 3);
        assert_eq!(grads.len(), 6);
        assert!((grads[0] - (-0.5)).abs() < 1e-6);
        assert!((grads[1]).abs() < 1e-6);
        assert!((grads[5] - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn test_kl_div_loss_forward() {
        let loss = KLDivLoss::new();
        let log_q = vec![0.25_f32.ln(), 0.25_f32.ln(), 0.5_f32.ln()];
        let p = vec![0.25, 0.25, 0.5];
        let l = loss.forward(&log_q, &p);
        assert!(l[0].abs() < 1e-5);
    }

    #[test]
    fn test_kl_div_loss_log_target() {
        let loss = KLDivLoss::new().log_target(true);
        let log_q = vec![0.5_f32.ln(), 0.5_f32.ln()];
        let log_p = vec![0.5_f32.ln(), 0.5_f32.ln()];
        let l = loss.forward(&log_q, &log_p);
        assert!(l[0].abs() < 1e-5);
    }

    #[test]
    fn test_mse_reduction_none() {
        let loss = MSELoss::new().reduction(Reduction::None);
        let input = vec![1.0, 2.0, 3.0];
        let target = vec![0.0, 2.0, 5.0];
        let l = loss.forward(&input, &target);
        assert_eq!(l.len(), 3);
        assert!((l[0] - 1.0).abs() < 1e-6);
        assert!((l[1] - 0.0).abs() < 1e-6);
        assert!((l[2] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_mse_reduction_sum() {
        let loss = MSELoss::new().reduction(Reduction::Sum);
        let input = vec![1.0, 2.0, 3.0];
        let target = vec![0.0, 2.0, 5.0];
        let l = loss.forward(&input, &target);
        assert_eq!(l.len(), 1);
        assert!((l[0] - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_mse_backward() {
        let loss = MSELoss::new();
        let input = vec![1.0, 3.0];
        let target = vec![0.0, 1.0];
        let grads = loss.backward(&input, &target);
        assert_eq!(grads.len(), 2);
        assert!((grads[0] - 2.0 * 1.0 / 2.0).abs() < 1e-6);
        assert!((grads[1] - 2.0 * 2.0 / 2.0).abs() < 1e-6);
    }

}
