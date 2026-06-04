//! RMI Framework Performance Benchmarks
//!
//! This example measures performance of core neural operations
//! and compares conceptually to PyTorch equivalents.

use rmi::neural::{Adam, GradientTape, Layer, Linear, Loss, MSELoss, Optimizer, Variable, SGD};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

fn bench<F: FnMut()>(name: &str, iterations: usize, mut f: F) {
    // Warmup
    for _ in 0..10 {
        f();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();

    let per_iter = elapsed.as_nanos() as f64 / iterations as f64;
    let throughput = 1_000_000_000.0 / per_iter;

    println!(
        "{:40} {:>10.2} ns/iter  ({:.0} ops/sec)",
        name, per_iter, throughput
    );
}

fn main() {
    println!("============================================================");
    println!("RMI Framework Performance Benchmarks");
    println!("============================================================");
    println!();

    // Matrix multiplication benchmarks
    println!("Matrix Multiplication (naive):");
    println!("------------------------------------------------------------");
    for size in [64, 128, 256] {
        let a: Vec<f32> = (0..size * size).map(|i| (i % 100) as f32 * 0.01).collect();
        let b: Vec<f32> = (0..size * size).map(|i| (i % 100) as f32 * 0.01).collect();

        bench(&format!("  {}x{} matmul", size, size), 100, || {
            let mut result = vec![0.0f32; size * size];
            for i in 0..size {
                for j in 0..size {
                    for k in 0..size {
                        result[i * size + j] += a[i * size + k] * b[k * size + j];
                    }
                }
            }
            std::hint::black_box(result);
        });
    }
    println!();

    // Forward pass benchmarks
    println!("Linear Layer Forward Pass:");
    println!("------------------------------------------------------------");
    for (in_f, out_f) in [(64, 32), (256, 128), (512, 256)] {
        let layer = Linear::new(in_f, out_f);
        let input = Variable::new(vec![0.1; in_f], vec![1, in_f], false);

        bench(&format!("  Linear {}x{}", in_f, out_f), 1000, || {
            let mut tape = GradientTape::new();
            std::hint::black_box(layer.forward(&[&input], &mut tape));
        });
    }
    println!();

    // Autodiff backward pass benchmarks
    println!("Autodiff Backward Pass:");
    println!("------------------------------------------------------------");
    for size in [64, 256, 1024] {
        let data: Vec<f32> = (0..size).map(|i| (i % 100) as f32 * 0.01).collect();

        bench(&format!("  backward mul {}", size), 1000, || {
            let mut tape = GradientTape::new();
            let a = Variable::new(data.clone(), vec![size], true);
            let b = Variable::new(data.clone(), vec![size], true);
            let a_id = tape.register(a);
            let b_id = tape.register(b);
            let c_id = tape.mul(a_id, b_id);
            let loss_id = tape.sum(c_id, None, false);
            tape.backward(loss_id);
            std::hint::black_box(());
        });
    }
    println!();

    // Loss function benchmarks
    println!("Loss Functions:");
    println!("------------------------------------------------------------");
    for size in [64, 256, 1024] {
        let pred: Vec<f32> = (0..size).map(|i| (i % 100) as f32 * 0.01).collect();
        let target: Vec<f32> = (0..size).map(|i| ((i + 10) % 100) as f32 * 0.01).collect();
        let mse = MSELoss::new();

        bench(&format!("  MSE loss {}", size), 10000, || {
            std::hint::black_box(mse.forward(&pred, &target));
        });
    }
    println!();

    // Optimizer benchmarks
    println!("Optimizer Step:");
    println!("------------------------------------------------------------");
    for num_params in [100, 1000, 10000] {
        let id = Uuid::new_v4();
        let params: Vec<f32> = (0..num_params).map(|i| (i % 100) as f32 * 0.01).collect();
        let grads: Vec<f32> = (0..num_params)
            .map(|i| ((i + 5) % 100) as f32 * 0.001)
            .collect();

        bench(&format!("  SGD {} params", num_params), 1000, || {
            let mut opt = SGD::new(0.01);
            let mut p = vec![(id, params.clone())];
            let mut g = HashMap::new();
            g.insert(id, grads.clone());
            let _: () = opt.step(&mut p, &g);
            std::hint::black_box(());
        });

        bench(&format!("  Adam {} params", num_params), 1000, || {
            let mut opt = Adam::new(0.001);
            let mut p = vec![(id, params.clone())];
            let mut g = HashMap::new();
            g.insert(id, grads.clone());
            let _: () = opt.step(&mut p, &g);
            std::hint::black_box(());
        });
    }
    println!();

    // Activation function benchmarks
    println!("Activation Functions:");
    println!("------------------------------------------------------------");
    for size in [256, 1024, 4096] {
        let data: Vec<f32> = (0..size)
            .map(|i| (i as f32 - (size / 2) as f32) * 0.01)
            .collect();

        bench(&format!("  ReLU {}", size), 5000, || {
            let mut tape = GradientTape::new();
            let x = Variable::new(data.clone(), vec![size], true);
            let x_id = tape.register(x);
            std::hint::black_box(tape.relu(x_id));
        });

        bench(&format!("  Sigmoid {}", size), 5000, || {
            let mut tape = GradientTape::new();
            let x = Variable::new(data.clone(), vec![size], true);
            let x_id = tape.register(x);
            std::hint::black_box(tape.sigmoid(x_id));
        });
    }
    println!();

    // MLP training step benchmark
    println!("MLP Training Step (simulated):");
    println!("------------------------------------------------------------");
    let layer1 = Linear::new(64, 32);
    let layer2 = Linear::new(32, 10);
    let loss_fn = MSELoss::new();
    let input = Variable::new(vec![0.1; 64], vec![1, 64], false);
    let target = vec![0.0; 10];

    bench("  Forward + Loss (64->32->10)", 1000, || {
        let mut tape = GradientTape::new();
        let h = layer1.forward(&[&input], &mut tape);
        let h_id = tape.register(h);
        let h_relu = tape.relu(h_id);
        let h_var = tape.get(h_relu).unwrap().clone();
        let out = layer2.forward(&[&h_var], &mut tape);
        let loss = loss_fn.forward(&out.data, &target);
        std::hint::black_box(loss);
    });
    println!();

    println!("============================================================");
    println!("Benchmark Complete");
    println!("============================================================");
}
