//! Neural Network Optimizers Module
//!
//! Provides standard optimization algorithms for training neural networks.

use std::collections::HashMap;
use uuid::Uuid;

/// Trait for all optimizers
pub trait Optimizer {
    /// Update parameters using their gradients
    fn step(&mut self, params: &mut [(Uuid, Vec<f32>)], grads: &HashMap<Uuid, Vec<f32>>);
    /// Reset optimizer state
    fn zero_grad(&mut self);
    /// Get learning rate
    fn get_lr(&self) -> f32;
    /// Set learning rate
    fn set_lr(&mut self, lr: f32);
}

/// SGD optimizer with momentum
pub struct SGD {
    lr: f32,
    momentum: f32,
    weight_decay: f32,
    dampening: f32,
    nesterov: bool,
    velocities: HashMap<Uuid, Vec<f32>>,
}

impl SGD {
    /// Create a new SGD optimizer
    pub fn new(lr: f32) -> Self {
        Self { lr, momentum: 0.0, weight_decay: 0.0, dampening: 0.0, nesterov: false, velocities: HashMap::new() }
    }
    /// Set momentum factor
    pub fn momentum(mut self, momentum: f32) -> Self { self.momentum = momentum; self }
    /// Set weight decay
    pub fn weight_decay(mut self, weight_decay: f32) -> Self { self.weight_decay = weight_decay; self }
    /// Set dampening factor
    pub fn dampening(mut self, dampening: f32) -> Self { self.dampening = dampening; self }
    /// Enable Nesterov momentum
    pub fn nesterov(mut self, nesterov: bool) -> Self { self.nesterov = nesterov; self }
}

impl Optimizer for SGD {
    fn step(&mut self, params: &mut [(Uuid, Vec<f32>)], grads: &HashMap<Uuid, Vec<f32>>) {
        for (id, param) in params.iter_mut() {
            if let Some(grad) = grads.get(id) {
                let mut d_p: Vec<f32> = grad.clone();
                if self.weight_decay != 0.0 {
                    for (d, p) in d_p.iter_mut().zip(param.iter()) { *d += self.weight_decay * p; }
                }
                if self.momentum != 0.0 {
                    let v = self.velocities.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                    for (vi, di) in v.iter_mut().zip(d_p.iter()) { *vi = self.momentum * *vi + (1.0 - self.dampening) * di; }
                    if self.nesterov { for (d, vi) in d_p.iter_mut().zip(v.iter()) { *d += self.momentum * vi; } } 
                    else { d_p = v.clone(); }
                }
                for (p, d) in param.iter_mut().zip(d_p.iter()) { *p -= self.lr * d; }
            }
        }
    }
    fn zero_grad(&mut self) {}
    fn get_lr(&self) -> f32 { self.lr }
    fn set_lr(&mut self, lr: f32) { self.lr = lr; }
}

/// Adam optimizer
pub struct Adam {
    lr: f32, beta1: f32, beta2: f32, eps: f32, weight_decay: f32, amsgrad: bool,
    m: HashMap<Uuid, Vec<f32>>, v: HashMap<Uuid, Vec<f32>>, v_max: HashMap<Uuid, Vec<f32>>, t: u64,
}

impl Adam {
    /// Create a new Adam optimizer
    pub fn new(lr: f32) -> Self {
        Self { lr, beta1: 0.9, beta2: 0.999, eps: 1e-8, weight_decay: 0.0, amsgrad: false,
               m: HashMap::new(), v: HashMap::new(), v_max: HashMap::new(), t: 0 }
    }
    /// Set beta parameters
    pub fn betas(mut self, beta1: f32, beta2: f32) -> Self { self.beta1 = beta1; self.beta2 = beta2; self }
    /// Set epsilon
    pub fn eps(mut self, eps: f32) -> Self { self.eps = eps; self }
    /// Set weight decay
    pub fn weight_decay(mut self, weight_decay: f32) -> Self { self.weight_decay = weight_decay; self }
    /// Enable AMSGrad
    pub fn amsgrad(mut self, amsgrad: bool) -> Self { self.amsgrad = amsgrad; self }
}

impl Optimizer for Adam {
    fn step(&mut self, params: &mut [(Uuid, Vec<f32>)], grads: &HashMap<Uuid, Vec<f32>>) {
        self.t += 1;
        let bc1 = 1.0 - self.beta1.powi(self.t as i32);
        let bc2 = 1.0 - self.beta2.powi(self.t as i32);
        for (id, param) in params.iter_mut() {
            if let Some(grad) = grads.get(id) {
                let mut g = grad.clone();
                if self.weight_decay != 0.0 { for (gi, pi) in g.iter_mut().zip(param.iter()) { *gi += self.weight_decay * pi; } }
                let m = self.m.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                let v = self.v.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                for (mi, gi) in m.iter_mut().zip(g.iter()) { *mi = self.beta1 * *mi + (1.0 - self.beta1) * gi; }
                for (vi, gi) in v.iter_mut().zip(g.iter()) { *vi = self.beta2 * *vi + (1.0 - self.beta2) * gi * gi; }
                let v_eff = if self.amsgrad {
                    let vm = self.v_max.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                    for (vmi, vi) in vm.iter_mut().zip(v.iter()) { *vmi = vmi.max(*vi); }
                    vm.clone()
                } else { v.clone() };
                for ((pi, mi), vi) in param.iter_mut().zip(m.iter()).zip(v_eff.iter()) {
                    *pi -= self.lr * (mi / bc1) / ((vi / bc2).sqrt() + self.eps);
                }
            }
        }
    }
    fn zero_grad(&mut self) {}
    fn get_lr(&self) -> f32 { self.lr }
    fn set_lr(&mut self, lr: f32) { self.lr = lr; }
}

/// AdamW optimizer
pub struct AdamW { inner: Adam }

impl AdamW {
    /// Create a new AdamW optimizer
    pub fn new(lr: f32) -> Self { Self { inner: Adam::new(lr) } }
    /// Set beta parameters
    pub fn betas(mut self, beta1: f32, beta2: f32) -> Self { self.inner = self.inner.betas(beta1, beta2); self }
    /// Set epsilon
    pub fn eps(mut self, eps: f32) -> Self { self.inner = self.inner.eps(eps); self }
    /// Set weight decay
    pub fn weight_decay(mut self, weight_decay: f32) -> Self { self.inner = self.inner.weight_decay(weight_decay); self }
}

impl Optimizer for AdamW {
    fn step(&mut self, params: &mut [(Uuid, Vec<f32>)], grads: &HashMap<Uuid, Vec<f32>>) {
        self.inner.t += 1;
        let bc1 = 1.0 - self.inner.beta1.powi(self.inner.t as i32);
        let bc2 = 1.0 - self.inner.beta2.powi(self.inner.t as i32);
        for (id, param) in params.iter_mut() {
            if let Some(grad) = grads.get(id) {
                if self.inner.weight_decay != 0.0 { for pi in param.iter_mut() { *pi *= 1.0 - self.inner.lr * self.inner.weight_decay; } }
                let m = self.inner.m.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                let v = self.inner.v.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                for (mi, gi) in m.iter_mut().zip(grad.iter()) { *mi = self.inner.beta1 * *mi + (1.0 - self.inner.beta1) * gi; }
                for (vi, gi) in v.iter_mut().zip(grad.iter()) { *vi = self.inner.beta2 * *vi + (1.0 - self.inner.beta2) * gi * gi; }
                for ((pi, mi), vi) in param.iter_mut().zip(m.iter()).zip(v.iter()) {
                    *pi -= self.inner.lr * (mi / bc1) / ((vi / bc2).sqrt() + self.inner.eps);
                }
            }
        }
    }
    fn zero_grad(&mut self) {}
    fn get_lr(&self) -> f32 { self.inner.lr }
    fn set_lr(&mut self, lr: f32) { self.inner.lr = lr; }
}

/// RMSprop optimizer
pub struct RMSprop {
    lr: f32, alpha: f32, eps: f32, weight_decay: f32, momentum: f32, centered: bool,
    v: HashMap<Uuid, Vec<f32>>, buf: HashMap<Uuid, Vec<f32>>, grad_avg: HashMap<Uuid, Vec<f32>>,
}

impl RMSprop {
    /// Create a new RMSprop optimizer
    pub fn new(lr: f32) -> Self {
        Self { lr, alpha: 0.99, eps: 1e-8, weight_decay: 0.0, momentum: 0.0, centered: false,
               v: HashMap::new(), buf: HashMap::new(), grad_avg: HashMap::new() }
    }
    /// Set alpha
    pub fn alpha(mut self, alpha: f32) -> Self { self.alpha = alpha; self }
    /// Set epsilon
    pub fn eps(mut self, eps: f32) -> Self { self.eps = eps; self }
    /// Set weight decay
    pub fn weight_decay(mut self, weight_decay: f32) -> Self { self.weight_decay = weight_decay; self }
    /// Set momentum
    pub fn momentum(mut self, momentum: f32) -> Self { self.momentum = momentum; self }
    /// Enable centered
    pub fn centered(mut self, centered: bool) -> Self { self.centered = centered; self }
}

impl Optimizer for RMSprop {
    fn step(&mut self, params: &mut [(Uuid, Vec<f32>)], grads: &HashMap<Uuid, Vec<f32>>) {
        for (id, param) in params.iter_mut() {
            if let Some(grad) = grads.get(id) {
                let mut g = grad.clone();
                if self.weight_decay != 0.0 { for (gi, pi) in g.iter_mut().zip(param.iter()) { *gi += self.weight_decay * pi; } }
                let v = self.v.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                for (vi, gi) in v.iter_mut().zip(g.iter()) { *vi = self.alpha * *vi + (1.0 - self.alpha) * gi * gi; }
                let avg: Vec<f32> = if self.centered {
                    let ga = self.grad_avg.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                    for (gai, gi) in ga.iter_mut().zip(g.iter()) { *gai = self.alpha * *gai + (1.0 - self.alpha) * gi; }
                    v.iter().zip(ga.iter()).map(|(vi, gai)| (vi - gai * gai).sqrt() + self.eps).collect()
                } else { v.iter().map(|vi| vi.sqrt() + self.eps).collect() };
                if self.momentum > 0.0 {
                    let buf = self.buf.entry(*id).or_insert_with(|| vec![0.0; param.len()]);
                    for ((bi, gi), ai) in buf.iter_mut().zip(g.iter()).zip(avg.iter()) { *bi = self.momentum * *bi + gi / ai; }
                    for (pi, bi) in param.iter_mut().zip(buf.iter()) { *pi -= self.lr * bi; }
                } else { for ((pi, gi), ai) in param.iter_mut().zip(g.iter()).zip(avg.iter()) { *pi -= self.lr * gi / ai; } }
            }
        }
    }
    fn zero_grad(&mut self) {}
    fn get_lr(&self) -> f32 { self.lr }
    fn set_lr(&mut self, lr: f32) { self.lr = lr; }
}

/// LR scheduler trait
pub trait LRScheduler { 
    /// Get learning rate
    fn get_lr(&self) -> f32;
    /// Step the scheduler
    fn step(&mut self);
}

/// Step LR scheduler
pub struct StepLR { base_lr: f32, current_lr: f32, step_size: usize, gamma: f32, current_step: usize }

impl StepLR {
    /// Create a new StepLR scheduler
    pub fn new(base_lr: f32, step_size: usize, gamma: f32) -> Self {
        Self { base_lr, current_lr: base_lr, step_size, gamma, current_step: 0 }
    }
}

impl LRScheduler for StepLR {
    fn get_lr(&self) -> f32 { self.current_lr }
    fn step(&mut self) { self.current_step += 1; self.current_lr = self.base_lr * self.gamma.powi((self.current_step / self.step_size) as i32); }
}

/// Cosine annealing LR scheduler
pub struct CosineAnnealingLR { base_lr: f32, min_lr: f32, t_max: usize, current_step: usize }

impl CosineAnnealingLR {
    /// Create a new CosineAnnealingLR scheduler
    pub fn new(base_lr: f32, t_max: usize) -> Self { Self { base_lr, min_lr: 0.0, t_max, current_step: 0 } }
    /// Set minimum LR
    pub fn min_lr(mut self, min_lr: f32) -> Self { self.min_lr = min_lr; self }
}

impl LRScheduler for CosineAnnealingLR {
    fn get_lr(&self) -> f32 { self.min_lr + (self.base_lr - self.min_lr) * (1.0 + (std::f32::consts::PI * self.current_step as f32 / self.t_max as f32).cos()) / 2.0 }
    fn step(&mut self) { self.current_step = (self.current_step + 1).min(self.t_max); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sgd_basic() {
        let mut opt = SGD::new(0.1);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0, 2.0, 3.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![0.1, 0.2, 0.3]);
        opt.step(&mut params, &grads);
        assert!((params[0].1[0] - 0.99).abs() < 1e-6);
    }

    #[test]
    fn test_sgd_momentum() {
        let mut opt = SGD::new(0.1).momentum(0.9);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![1.0]);
        opt.step(&mut params, &grads);
        assert!((params[0].1[0] - 0.9).abs() < 1e-6);
        opt.step(&mut params, &grads);
        assert!((params[0].1[0] - 0.71).abs() < 1e-6);
    }

    #[test]
    fn test_adam_basic() {
        let mut opt = Adam::new(0.001);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0, 2.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![0.1, 0.2]);
        opt.step(&mut params, &grads);
        assert!(params[0].1[0] < 1.0);
    }

    #[test]
    fn test_step_lr() {
        let mut sched = StepLR::new(0.1, 5, 0.5);
        assert!((sched.get_lr() - 0.1).abs() < 1e-6);
        for _ in 0..5 { sched.step(); }
        assert!((sched.get_lr() - 0.05).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_lr() {
        let mut sched = CosineAnnealingLR::new(0.1, 10);
        for _ in 0..10 { sched.step(); }
        assert!((sched.get_lr() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_sgd_weight_decay() {
        let mut opt = SGD::new(0.1).weight_decay(0.01);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0, 2.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![0.0, 0.0]);
        opt.step(&mut params, &grads);
        // With zero grads but weight_decay, params should shrink
        assert!(params[0].1[0] < 1.0);
        assert!(params[0].1[1] < 2.0);
    }

    #[test]
    fn test_sgd_nesterov() {
        let mut opt = SGD::new(0.1).momentum(0.9).nesterov(true);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![1.0]);
        opt.step(&mut params, &grads);
        // Nesterov should move differently than standard momentum
        assert!(params[0].1[0] < 1.0);
    }

    #[test]
    fn test_sgd_get_set_lr() {
        let mut opt = SGD::new(0.01);
        assert!((opt.get_lr() - 0.01).abs() < 1e-9);
        opt.set_lr(0.05);
        assert!((opt.get_lr() - 0.05).abs() < 1e-9);
    }

    #[test]
    fn test_sgd_dampening() {
        let mut opt = SGD::new(0.1).momentum(0.9).dampening(0.5);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![1.0]);
        opt.step(&mut params, &grads);
        opt.step(&mut params, &grads);
        // With dampening, velocity update is scaled differently
        assert!(params[0].1[0] < 1.0);
    }

    #[test]
    fn test_sgd_no_matching_grad() {
        let mut opt = SGD::new(0.1);
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let mut params = vec![(id1, vec![1.0, 2.0])];
        let mut grads = HashMap::new();
        grads.insert(id2, vec![0.5, 0.5]);
        opt.step(&mut params, &grads);
        // Param unchanged since no grad matches
        assert!((params[0].1[0] - 1.0).abs() < 1e-9);
        assert!((params[0].1[1] - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_adam_multiple_steps() {
        let mut opt = Adam::new(0.1);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![5.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![1.0]);
        let initial = params[0].1[0];
        for _ in 0..10 {
            opt.step(&mut params, &grads);
        }
        assert!(params[0].1[0] < initial);
    }

    #[test]
    fn test_adam_amsgrad() {
        let mut opt = Adam::new(0.001).amsgrad(true);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0, 2.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![0.1, 0.2]);
        opt.step(&mut params, &grads);
        assert!(params[0].1[0] < 1.0);
        assert!(params[0].1[1] < 2.0);
    }

    #[test]
    fn test_adam_get_set_lr() {
        let mut opt = Adam::new(0.001);
        assert!((opt.get_lr() - 0.001).abs() < 1e-9);
        opt.set_lr(0.01);
        assert!((opt.get_lr() - 0.01).abs() < 1e-9);
    }

    #[test]
    fn test_adam_custom_betas() {
        let mut opt = Adam::new(0.001).betas(0.8, 0.99).eps(1e-7);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![0.5]);
        opt.step(&mut params, &grads);
        assert!(params[0].1[0] < 1.0);
    }

    #[test]
    fn test_adamw_basic() {
        let mut opt = AdamW::new(0.001);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0, 2.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![0.1, 0.2]);
        opt.step(&mut params, &grads);
        assert!(params[0].1[0] < 1.0);
        assert!(params[0].1[1] < 2.0);
    }

    #[test]
    fn test_adamw_weight_decay() {
        let mut opt = AdamW::new(0.001).weight_decay(0.1);
        let id = Uuid::new_v4();
        let mut params_wd = vec![(id, vec![1.0])];
        let mut params_no = vec![(id, vec![1.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![0.0]);
        opt.step(&mut params_wd, &grads);
        let mut opt2 = AdamW::new(0.001);
        opt2.step(&mut params_no, &grads);
        // With weight decay, params should be smaller
        assert!(params_wd[0].1[0] < params_no[0].1[0]);
    }

    #[test]
    fn test_rmsprop_basic() {
        let mut opt = RMSprop::new(0.01);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0, 2.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![0.5, 0.5]);
        opt.step(&mut params, &grads);
        assert!(params[0].1[0] < 1.0);
        assert!(params[0].1[1] < 2.0);
    }

    #[test]
    fn test_rmsprop_with_momentum() {
        let mut opt = RMSprop::new(0.01).momentum(0.9);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![1.0]);
        opt.step(&mut params, &grads);
        opt.step(&mut params, &grads);
        assert!(params[0].1[0] < 1.0);
    }

    #[test]
    fn test_rmsprop_centered() {
        let mut opt = RMSprop::new(0.01).centered(true);
        let id = Uuid::new_v4();
        let mut params = vec![(id, vec![1.0])];
        let mut grads = HashMap::new();
        grads.insert(id, vec![1.0]);
        opt.step(&mut params, &grads);
        assert!(params[0].1[0] < 1.0);
    }

    #[test]
    fn test_cosine_lr_with_min_lr() {
        let mut sched = CosineAnnealingLR::new(0.1, 10).min_lr(0.01);
        assert!((sched.get_lr() - 0.1).abs() < 1e-6);
        for _ in 0..10 { sched.step(); }
        assert!((sched.get_lr() - 0.01).abs() < 1e-6);
    }

}
