//! Comprehensive Neural Module Tests
use rmi::neural::{
    Adam, CosineAnnealingLR, CrossEntropyLoss, GradientTape, L1Loss, LRScheduler, Layer, Linear,
    Loss, MSELoss, Optimizer, Reduction, StepLR, Variable, SGD,
};
use std::collections::HashMap;
use uuid::Uuid;

// ========================================
// Variable Tests
// ========================================

#[test]
fn test_variable_creation() {
    let v = Variable::new(vec![1.0, 2.0, 3.0], vec![3], true);
    assert_eq!(v.data.len(), 3);
    assert_eq!(v.shape, vec![3]);
    assert!(v.requires_grad);
}

#[test]
fn test_variable_zeros() {
    let v = Variable::zeros(&[2, 3], false);
    assert_eq!(v.data.len(), 6);
    assert_eq!(v.shape, vec![2, 3]);
    assert!(v.data.iter().all(|&x| x == 0.0));
}

#[test]
fn test_variable_ones() {
    let v = Variable::ones(&[4, 5], true);
    assert_eq!(v.data.len(), 20);
    assert!(v.data.iter().all(|&x| x == 1.0));
}

#[test]
fn test_variable_scalar() {
    let v = Variable::scalar(std::f32::consts::PI, true);
    assert_eq!(v.numel(), 1);
    assert_eq!(v.data[0], std::f32::consts::PI);
}

#[test]
fn test_variable_zero_grad() {
    let mut v = Variable::new(vec![1.0, 2.0], vec![2], true);
    v.zero_grad();
    assert!(v.grad.is_some());
    assert_eq!(v.grad.as_ref().unwrap(), &vec![0.0, 0.0]);
}

#[test]
fn test_variable_accumulate_grad() {
    let mut v = Variable::new(vec![1.0, 2.0], vec![2], true);
    v.zero_grad();
    v.accumulate_grad(&[0.5, 0.5]);
    v.accumulate_grad(&[0.5, 0.5]);
    assert_eq!(v.grad.as_ref().unwrap(), &vec![1.0, 1.0]);
}

// ========================================
// GradientTape Forward Tests
// ========================================

#[test]
fn test_tape_add() {
    let mut tape = GradientTape::new();
    let a = Variable::new(vec![1.0, 2.0], vec![2], true);
    let b = Variable::new(vec![3.0, 4.0], vec![2], true);
    let a_id = tape.register(a);
    let b_id = tape.register(b);
    let c_id = tape.add(a_id, b_id);
    let c = tape.get(c_id).unwrap();
    assert_eq!(c.data, vec![4.0, 6.0]);
}

#[test]
fn test_tape_mul() {
    let mut tape = GradientTape::new();
    let a = Variable::new(vec![2.0, 3.0], vec![2], true);
    let b = Variable::new(vec![4.0, 5.0], vec![2], true);
    let a_id = tape.register(a);
    let b_id = tape.register(b);
    let c_id = tape.mul(a_id, b_id);
    let c = tape.get(c_id).unwrap();
    assert_eq!(c.data, vec![8.0, 15.0]);
}

#[test]
fn test_tape_relu() {
    let mut tape = GradientTape::new();
    let a = Variable::new(vec![-1.0, 0.0, 1.0, 2.0], vec![4], true);
    let a_id = tape.register(a);
    let b_id = tape.relu(a_id);
    let b = tape.get(b_id).unwrap();
    assert_eq!(b.data, vec![0.0, 0.0, 1.0, 2.0]);
}

#[test]
fn test_tape_sigmoid() {
    let mut tape = GradientTape::new();
    let a = Variable::new(vec![0.0], vec![1], true);
    let a_id = tape.register(a);
    let b_id = tape.sigmoid(a_id);
    let b = tape.get(b_id).unwrap();
    assert!((b.data[0] - 0.5).abs() < 1e-6);
}

#[test]
fn test_tape_matmul() {
    let mut tape = GradientTape::new();
    // [2,3] @ [3,2] = [2,2]
    let a = Variable::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3], true);
    let b = Variable::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![3, 2], true);
    let a_id = tape.register(a);
    let b_id = tape.register(b);
    let c_id = tape.matmul(a_id, b_id);
    let c = tape.get(c_id).unwrap();
    assert_eq!(c.shape, vec![2, 2]);
    assert_eq!(c.data, vec![22.0, 28.0, 49.0, 64.0]);
}

// ========================================
// GradientTape Backward Tests
// ========================================

#[test]
fn test_tape_backward_add() {
    let mut tape = GradientTape::new();
    let a = Variable::new(vec![1.0, 2.0], vec![2], true);
    let b = Variable::new(vec![3.0, 4.0], vec![2], true);
    let a_id = tape.register(a);
    let b_id = tape.register(b);
    let c_id = tape.add(a_id, b_id);
    let loss_id = tape.sum(c_id, None, false);
    tape.backward(loss_id);
    let a_grad = tape.get(a_id).unwrap().grad.as_ref().unwrap();
    let b_grad = tape.get(b_id).unwrap().grad.as_ref().unwrap();
    assert_eq!(a_grad, &vec![1.0, 1.0]);
    assert_eq!(b_grad, &vec![1.0, 1.0]);
}

#[test]
fn test_tape_backward_mul() {
    let mut tape = GradientTape::new();
    let a = Variable::new(vec![2.0, 3.0], vec![2], true);
    let b = Variable::new(vec![4.0, 5.0], vec![2], true);
    let a_id = tape.register(a);
    let b_id = tape.register(b);
    let c_id = tape.mul(a_id, b_id);
    let loss_id = tape.sum(c_id, None, false);
    tape.backward(loss_id);
    // d(xy)/dx = y, d(xy)/dy = x
    let a_grad = tape.get(a_id).unwrap().grad.as_ref().unwrap();
    let b_grad = tape.get(b_id).unwrap().grad.as_ref().unwrap();
    assert_eq!(a_grad, &vec![4.0, 5.0]);
    assert_eq!(b_grad, &vec![2.0, 3.0]);
}

#[test]
fn test_tape_backward_relu() {
    let mut tape = GradientTape::new();
    let x = Variable::new(vec![-1.0, 0.0, 1.0, 2.0], vec![4], true);
    let x_id = tape.register(x);
    let y_id = tape.relu(x_id);
    let loss_id = tape.sum(y_id, None, false);
    tape.backward(loss_id);
    let x_grad = tape.get(x_id).unwrap().grad.as_ref().unwrap();
    // ReLU gradient: 0 where x <= 0, 1 where x > 0
    assert_eq!(x_grad, &vec![0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn test_tape_backward_chain() {
    let mut tape = GradientTape::new();
    let x = Variable::new(vec![1.0, 2.0], vec![2], true);
    let x_id = tape.register(x);
    // y = relu(x) + x  -> dy/dx = 1 + relu'(x)
    let relu_id = tape.relu(x_id);
    let sum_id = tape.add(relu_id, x_id);
    let loss_id = tape.sum(sum_id, None, false);
    tape.backward(loss_id);
    let x_grad = tape.get(x_id).unwrap().grad.as_ref().unwrap();
    // For positive x, gradient = 1 (add) + 1 (relu) = 2
    assert_eq!(x_grad, &vec![2.0, 2.0]);
}

// ========================================
// Linear Layer Tests
// ========================================

#[test]
fn test_linear_layer() {
    let layer = Linear::new(10, 5);
    let input = Variable::new(vec![0.1; 20], vec![2, 10], false);
    let mut tape = GradientTape::new();
    let output = layer.forward(&[&input], &mut tape);
    assert_eq!(output.shape, vec![2, 5]);
}

#[test]
fn test_linear_layer_shapes() {
    for (in_feat, out_feat) in [(8, 4), (16, 8), (32, 16), (64, 32)] {
        let layer = Linear::new(in_feat, out_feat);
        let input = Variable::new(vec![0.1; in_feat], vec![1, in_feat], false);
        let mut tape = GradientTape::new();
        let output = layer.forward(&[&input], &mut tape);
        assert_eq!(output.shape, vec![1, out_feat]);
    }
}

#[test]
fn test_batch_processing() {
    let layer = Linear::new(10, 5);
    for batch_size in [1, 4, 8, 16] {
        let input = Variable::new(vec![0.1; batch_size * 10], vec![batch_size, 10], false);
        let mut tape = GradientTape::new();
        let output = layer.forward(&[&input], &mut tape);
        assert_eq!(output.shape, vec![batch_size, 5]);
    }
}

// ========================================
// Loss Function Tests
// ========================================

#[test]
fn test_mse_loss_perfect() {
    let loss = MSELoss::new();
    let pred = vec![1.0, 2.0, 3.0];
    let target = vec![1.0, 2.0, 3.0];
    let l = loss.forward(&pred, &target);
    assert!((l[0] - 0.0).abs() < 1e-6);
}

#[test]
fn test_mse_loss_value() {
    let loss = MSELoss::new();
    let pred = vec![0.0, 0.0];
    let target = vec![1.0, 1.0];
    let l = loss.forward(&pred, &target);
    // MSE = (1+1)/2 = 1
    assert!((l[0] - 1.0).abs() < 1e-6);
}

#[test]
fn test_mse_loss_gradient() {
    let loss = MSELoss::new();
    let pred = vec![2.0, 3.0];
    let target = vec![1.0, 1.0];
    let grad = loss.backward(&pred, &target);
    // grad = (pred - target) = [1, 2]
    assert!((grad[0] - 1.0).abs() < 1e-6);
    assert!((grad[1] - 2.0).abs() < 1e-6);
}

#[test]
fn test_l1_loss() {
    let loss = L1Loss::new();
    let pred = vec![1.0, 2.0, 3.0];
    let target = vec![0.0, 2.0, 5.0];
    let l = loss.forward(&pred, &target);
    // L1 = (|1-0| + |2-2| + |3-5|) / 3 = (1+0+2)/3 = 1
    assert!((l[0] - 1.0).abs() < 1e-6);
}

#[test]
fn test_cross_entropy_softmax() {
    let probs = CrossEntropyLoss::softmax(&[1.0, 2.0, 3.0]);
    let sum: f32 = probs.iter().sum();
    assert!((sum - 1.0).abs() < 1e-6);
    assert!(probs[2] > probs[1]);
    assert!(probs[1] > probs[0]);
}

#[test]
fn test_cross_entropy_batch() {
    let loss = CrossEntropyLoss::new();
    let logits = vec![2.0, 1.0, 0.1, 0.1, 2.0, 1.0];
    let targets = vec![0, 1];
    let l = loss.forward_batch(&logits, &targets, 2, 3);
    assert!(l[0] < 1.5);
}

#[test]
fn test_reduction_modes() {
    let loss_mean = MSELoss::new().reduction(Reduction::Mean);
    let loss_sum = MSELoss::new().reduction(Reduction::Sum);
    let loss_none = MSELoss::new().reduction(Reduction::None);
    let pred = vec![0.0, 0.0, 0.0];
    let target = vec![1.0, 2.0, 3.0];
    let l_none = loss_none.forward(&pred, &target);
    let l_mean = loss_mean.forward(&pred, &target);
    let l_sum = loss_sum.forward(&pred, &target);
    assert_eq!(l_none.len(), 3);
    assert_eq!(l_mean.len(), 1);
    assert_eq!(l_sum.len(), 1);
}

// ========================================
// Optimizer Tests
// ========================================

#[test]
fn test_sgd_decreases_params() {
    let mut opt = SGD::new(0.1);
    let id = Uuid::new_v4();
    let mut params = vec![(id, vec![1.0, 1.0, 1.0])];
    let mut grads = HashMap::new();
    grads.insert(id, vec![1.0, 1.0, 1.0]);
    let initial = params[0].1.clone();
    opt.step(&mut params, &grads);
    for (p, i) in params[0].1.iter().zip(initial.iter()) {
        assert!(p < i);
    }
}

#[test]
fn test_sgd_with_momentum() {
    let mut opt = SGD::new(0.1).momentum(0.9);
    let id = Uuid::new_v4();
    let mut params = vec![(id, vec![1.0])];
    let mut grads = HashMap::new();
    grads.insert(id, vec![1.0]);
    // Multiple steps - momentum should accumulate
    for _ in 0..5 {
        opt.step(&mut params, &grads);
    }
    assert!(params[0].1[0] < 0.5);
}

#[test]
fn test_adam_convergence() {
    let mut opt = Adam::new(0.5); // Larger LR for faster convergence
    let id = Uuid::new_v4();
    let mut params = vec![(id, vec![10.0])];
    for _ in 0..200 {
        let mut grads = HashMap::new();
        grads.insert(id, vec![2.0 * params[0].1[0]]);
        opt.step(&mut params, &grads);
    }
    assert!(params[0].1[0].abs() < 2.0);
}

#[test]
fn test_adam_with_weight_decay() {
    let mut opt = Adam::new(0.01).weight_decay(0.1);
    let id = Uuid::new_v4();
    let mut params = vec![(id, vec![5.0])];
    let mut grads = HashMap::new();
    grads.insert(id, vec![0.0]); // Zero grad, only weight decay
    opt.step(&mut params, &grads);
    assert!(params[0].1[0] < 5.0);
}

// ========================================
// Learning Rate Scheduler Tests
// ========================================

#[test]
fn test_step_lr_decay() {
    let mut sched = StepLR::new(0.1, 10, 0.1);
    assert!((sched.get_lr() - 0.1).abs() < 1e-6);
    for _ in 0..10 {
        sched.step();
    }
    assert!((sched.get_lr() - 0.01).abs() < 1e-6);
}

#[test]
fn test_step_lr_multiple_decays() {
    let mut sched = StepLR::new(1.0, 5, 0.5);
    for _ in 0..5 {
        sched.step();
    }
    assert!((sched.get_lr() - 0.5).abs() < 1e-6);
    for _ in 0..5 {
        sched.step();
    }
    assert!((sched.get_lr() - 0.25).abs() < 1e-6);
}

#[test]
fn test_cosine_lr_endpoints() {
    let mut sched = CosineAnnealingLR::new(0.1, 100);
    assert!((sched.get_lr() - 0.1).abs() < 1e-6);
    for _ in 0..100 {
        sched.step();
    }
    assert!((sched.get_lr() - 0.0).abs() < 1e-6);
}

#[test]
fn test_cosine_lr_midpoint() {
    let mut sched = CosineAnnealingLR::new(1.0, 100);
    for _ in 0..50 {
        sched.step();
    }
    // At midpoint, cosine should be around 0.5
    assert!((sched.get_lr() - 0.5).abs() < 0.1);
}

// ========================================
// Integration Tests
// ========================================

#[test]
fn test_simple_training_step() {
    // Simulate a simple training step
    let layer = Linear::new(4, 2);
    let _loss_fn = MSELoss::new();
    let _opt = SGD::new(0.01);

    // Forward pass
    let input = Variable::new(vec![0.5; 4], vec![1, 4], false);
    let mut tape = GradientTape::new();
    let output = layer.forward(&[&input], &mut tape);

    // Verify output is valid (finite and non-empty)
    assert!(!output.data.is_empty());
    assert!(output.data.iter().all(|v| v.is_finite()));
}

#[test]
fn test_mlp_forward() {
    let layer1 = Linear::new(10, 8);
    let layer2 = Linear::new(8, 4);
    let layer3 = Linear::new(4, 2);

    let input = Variable::new(vec![0.1; 10], vec![1, 10], false);
    let mut tape = GradientTape::new();

    let h1 = layer1.forward(&[&input], &mut tape);
    // Apply ReLU through tape
    let h1_id = tape.register(h1);
    let h1_relu_id = tape.relu(h1_id);
    let h1_relu = tape.get(h1_relu_id).unwrap().clone();

    let h2 = layer2.forward(&[&h1_relu], &mut tape);
    let h2_id = tape.register(h2);
    let h2_relu_id = tape.relu(h2_id);
    let h2_relu = tape.get(h2_relu_id).unwrap().clone();

    let output = layer3.forward(&[&h2_relu], &mut tape);
    assert_eq!(output.shape, vec![1, 2]);
}
