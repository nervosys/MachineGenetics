//! # `cuda-bench` — CPU-vs-GPU wall-clock harness for the P-storage win
//!
//! Quantifies the payoff of **P-storage** (GPU-resident F32 tensors,
//! `cuda_backend.rs`): once intermediate tensors live in device memory,
//! a multi-op chain runs end-to-end on the GPU with **zero per-op
//! host↔device roundtrips**. Before P-storage every op paid
//! `dtoh → cpu_op → htod`; now `matmul → add → relu → scale` keeps all
//! intermediates resident and only the final result is copied back.
//!
//! This bin times that exact chain on `CpuBackend` vs `CudaBackend` for
//! a sweep of matrix sizes and reports wall-clock + speedup. It also
//! prints the backend's observability counters (`matmul_gpu_count`,
//! `elementwise_gpu_count`, `bounce_count`) so a reader can confirm the
//! chain stayed on the GPU (`bounce_count == 0`) rather than silently
//! falling back to CPU.
//!
//! ## Build & run
//!
//! ```text
//! cargo run --release --features cuda --bin cuda-bench
//! cargo run --release --features cuda --bin cuda-bench -- 1024 200   # dim iters
//! ```
//!
//! Without `--features cuda` the bin compiles to a stub that explains
//! how to enable it. With the feature but no NVIDIA driver present,
//! `CudaBackend::new()` errors and the harness reports CPU-only numbers
//! (so CI on a driverless box still exits 0).

// The CUDA backend module is itself `#![cfg(feature = "cuda")]`, so its
// `CudaBackend` symbol only exists under the feature. Gate the whole
// real harness accordingly and provide a no-feature stub `main`.
#[cfg(feature = "cuda")]
#[path = "../cuda_backend.rs"]
mod cuda_backend;

#[cfg(not(feature = "cuda"))]
fn main() {
    eprintln!("cuda-bench: built without the `cuda` feature — nothing to measure.");
    eprintln!("Re-run with:  cargo run --release --features cuda --bin cuda-bench");
}

#[cfg(feature = "cuda")]
fn main() {
    real_main();
}

#[cfg(feature = "cuda")]
fn real_main() {
    use crate::cuda_backend::CudaBackend;
    use rmi::compute::cpu::CpuBackend;
    use rmi::compute::Backend;
    use std::time::Instant;

    // ---- args: optional `dim` and `iters`; otherwise sweep defaults ----
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let sweep: Vec<(usize, usize)> = if argv.len() >= 2 {
        let dim: usize = argv[0].parse().unwrap_or(512);
        let iters: usize = argv[1].parse().unwrap_or(100);
        vec![(dim, iters)]
    } else {
        // (square dim, chain iterations). Small dims exaggerate the
        // per-op roundtrip overhead that P-storage removes; large dims
        // are matmul-bound where the GPU wins on raw FLOPs.
        vec![(64, 500), (128, 400), (256, 300), (512, 200), (1024, 100)]
    };

    println!("# cuda-bench — P-storage CPU-vs-GPU chain (matmul → add → relu → scale)");

    // Try to bring up the GPU. If the driver is absent we still print
    // CPU numbers and exit 0 (driverless CI must not fail).
    let gpu = match CudaBackend::new() {
        Ok(g) => {
            println!(
                "# GPU: {} (device {})",
                g.device_info().name,
                g.device_id()
            );
            Some(g)
        }
        Err(e) => {
            println!("# GPU: unavailable ({e}) — reporting CPU-only");
            None
        }
    };
    let cpu = CpuBackend::new();

    // One chain step: D = scale(relu(A·B + C), 0.5). Intermediates stay
    // resident on whichever backend `b` is. Returns the final handle's
    // checksum (sum of all elements) so we can cross-check correctness
    // and so the optimizer can't elide the work.
    fn chain<B: Backend>(
        b: &B,
        a: &rmi::compute::TensorHandle,
        bm: &rmi::compute::TensorHandle,
        c: &rmi::compute::TensorHandle,
        iters: usize,
    ) -> f64 {
        let mut acc = 0.0f64;
        for _ in 0..iters {
            let p = b.matmul(a, bm).expect("matmul");
            let q = b.add(&p, c).expect("add");
            let r = b.relu(&q).expect("relu");
            let s = b.scale(&r, 0.5).expect("scale");
            acc += b.sum(&s).expect("sum"); // sum() forces a host-visible sync
        }
        acc
    }

    println!(
        "\n{:>6} {:>7} {:>12} {:>12} {:>9}  notes",
        "dim", "iters", "cpu_ms", "gpu_ms", "speedup"
    );
    println!("{}", "-".repeat(72));

    for (dim, iters) in sweep {
        let n = dim * dim;
        // Deterministic, varied inputs (no Math.random in this env, and
        // we want reproducible numbers anyway).
        let a_data: Vec<f32> = (0..n).map(|i| ((i % 13) as f32 - 6.0) * 0.05).collect();
        let b_data: Vec<f32> = (0..n).map(|i| ((i % 7) as f32 - 3.0) * 0.05).collect();
        let c_data: Vec<f32> = (0..n).map(|i| ((i % 5) as f32 - 2.0) * 0.05).collect();
        let shape = [dim, dim];

        // ---- CPU ----
        let a = cpu.from_slice_f32(&a_data, &shape).expect("cpu a");
        let bm = cpu.from_slice_f32(&b_data, &shape).expect("cpu b");
        let c = cpu.from_slice_f32(&c_data, &shape).expect("cpu c");
        let _ = chain(&cpu, &a, &bm, &c, 1); // warmup
        let t = Instant::now();
        let cpu_sum = chain(&cpu, &a, &bm, &c, iters);
        let cpu_ms = t.elapsed().as_secs_f64() * 1e3;

        match &gpu {
            Some(g) => {
                let ga = g.from_slice_f32(&a_data, &shape).expect("gpu a");
                let gb = g.from_slice_f32(&b_data, &shape).expect("gpu b");
                let gc = g.from_slice_f32(&c_data, &shape).expect("gpu c");
                let bounce_before = g.bounce_count();
                let _ = chain(g, &ga, &gb, &gc, 1); // warmup (NVRTC compile + cuBLASLt init)
                let t = Instant::now();
                let gpu_sum = chain(g, &ga, &gb, &gc, iters);
                let gpu_ms = t.elapsed().as_secs_f64() * 1e3;
                let bounced = g.bounce_count() - bounce_before;

                let rel = (cpu_sum - gpu_sum).abs() / cpu_sum.abs().max(1e-9);
                let ok = rel < 1e-3;
                let speedup = cpu_ms / gpu_ms.max(1e-9);
                println!(
                    "{:>6} {:>7} {:>12.2} {:>12.2} {:>8.2}x  bounce={} {}",
                    dim,
                    iters,
                    cpu_ms,
                    gpu_ms,
                    speedup,
                    bounced,
                    if ok { "✓" } else { "✗ MISMATCH" }
                );
            }
            None => {
                println!(
                    "{:>6} {:>7} {:>12.2} {:>12} {:>9}  (cpu sum={:.4})",
                    dim, iters, cpu_ms, "-", "-", cpu_sum
                );
            }
        }
    }

    if let Some(g) = &gpu {
        println!(
            "\n# counters: matmul_gpu={} elementwise_gpu={} bounce={} gpu_storage_len={}",
            g.matmul_gpu_count(),
            g.elementwise_gpu_count(),
            g.bounce_count(),
            g.gpu_storage_len(),
        );
        println!("# bounce=0 across the sweep ⇒ the whole chain stayed GPU-resident (P-storage working).");
    }

    // ════════════════════════════════════════════════════════════════
    // INT8 quantized matmul (P121-124) vs F32 cuBLASLt. Realistic
    // inference: the weight is quantized ONCE (P124 cache, after warmup);
    // each iter dynamically quantizes the activation + runs the custom
    // INT8 GEMM. INT8 operands are 4× smaller; this measures whether the
    // (naive, non-tensor-core) INT8 kernel also wins on TIME vs the
    // tensor-core F32 path.
    // ════════════════════════════════════════════════════════════════
    if let Some(g) = &gpu {
        println!("\n# INT8 GEMM (cuBLASLt IMMA tensor cores) vs F32 cuBLASLt — GEMM only");
        println!(
            "{:>6} {:>7} {:>12} {:>12} {:>9} {:>12} {:>8}",
            "dim", "iters", "f32_ms", "imma_ms", "gemm_sp", "cal_ms", "cal_sp"
        );
        println!("{}", "-".repeat(76));
        for (dim, iters) in [(512usize, 200usize), (1024, 200), (2048, 100)] {
            let nsq = dim * dim;
            let av: Vec<f32> = (0..nsq).map(|i| ((i % 13) as f32 - 6.0) * 0.05).collect();
            let wv: Vec<f32> = (0..nsq).map(|i| ((i % 7) as f32 - 3.0) * 0.05).collect();
            let a = g.from_slice_f32(&av, &[dim, dim]).expect("a");
            let w = g.from_slice_f32(&wv, &[dim, dim]).expect("w");

            // F32 baseline (tensor cores via cuBLASLt). Sync once at the
            // end of the loop (cheap stream sync, not a reduction) so the
            // timing reflects GEMM throughput, not the sync op itself.
            let _ = g.matmul(&a, &w).map(|y| g.free(&y));
            let _ = g.synchronize();
            let t = Instant::now();
            for _ in 0..iters {
                let y = g.matmul(&a, &w).expect("matmul");
                let _ = g.free(&y);
            }
            let _ = g.synchronize();
            let f32_ms = t.elapsed().as_secs_f64() * 1e3;

            // INT8 IMMA — quantize ONCE (static), time the GEMM only.
            let (aq, sa) = g.quantize_i8(&a).expect("quant a");
            let (bq_t, sb) = g.quantize_i8_perchannel_t(&w).expect("quant w");
            let _ = g.matmul_i8_immma(&aq, sa, &bq_t, &sb).map(|y| g.free(&y));
            let _ = g.synchronize();
            let t = Instant::now();
            for _ in 0..iters {
                let y = g.matmul_i8_immma(&aq, sa, &bq_t, &sb).expect("imma");
                let _ = g.free(&y);
            }
            let _ = g.synchronize();
            let imma_ms = t.elapsed().as_secs_f64() * 1e3;

            // Calibrated end-to-end (P128): known activation scale, no
            // amax, no host sync — fully on-device quantize→IMMA→dequant.
            let sa = av.iter().fold(0.0f32, |m, &x| m.max(x.abs())) / 127.0;
            let _ = g.quantized_matmul_calibrated(&a, sa, &w).map(|y| g.free(&y)); // warm cache
            let _ = g.synchronize();
            let t = Instant::now();
            for _ in 0..iters {
                let y = g.quantized_matmul_calibrated(&a, sa, &w).expect("qmm_cal");
                let _ = g.free(&y);
            }
            let _ = g.synchronize();
            let e2e_ms = t.elapsed().as_secs_f64() * 1e3;

            let _ = g.free(&a);
            let _ = g.free(&w);
            let _ = g.free(&aq);
            let _ = g.free(&bq_t);

            println!(
                "{:>6} {:>7} {:>12.2} {:>12.2} {:>8.2}x {:>12.2} {:>7.2}x",
                dim, iters, f32_ms, imma_ms,
                f32_ms / imma_ms.max(1e-9),
                e2e_ms, f32_ms / e2e_ms.max(1e-9)
            );
        }
        println!(
            "# IMMA confirmed via quant_imma_count={}. imma_ms=GEMM-only; cal_ms=calibrated e2e (P128, no host sync).",
            g.quant_imma_count()
        );
    }

    // ════════════════════════════════════════════════════════════════
    // conv2d (P113) — GPU im2col+cuBLASLt GEMM vs CPU naive nested loops.
    // The CPU reference is O(N·Cout·Cin·H·W·K²); the GPU collapses it to
    // one im2col + one GEMM, so the gap widens fast with channels/size.
    // ════════════════════════════════════════════════════════════════
    println!("\n# conv2d — CPU naive vs GPU im2col+GEMM (NCHW, stride 1, pad 1)");
    println!(
        "{:>4} {:>5} {:>5} {:>5} {:>4} {:>5} {:>12} {:>12} {:>9}  notes",
        "N", "Cin", "Cout", "HW", "K", "iters", "cpu_ms", "gpu_ms", "speedup"
    );
    println!("{}", "-".repeat(82));

    // (N, Cin, Cout, H=W, K, iters)
    let conv_cases = [
        (1usize, 3usize, 16usize, 32usize, 3usize, 20usize),
        (1, 16, 32, 32, 3, 20),
        (8, 16, 32, 28, 3, 10),
        (1, 64, 64, 56, 3, 5),
    ];
    for (n, cin, cout, hw, k, iters) in conv_cases {
        let pad = 1usize;
        let stride = 1usize;
        let xn = n * cin * hw * hw;
        let wn = cout * cin * k * k;
        // All-positive so the output checksum is well away from zero
        // (a zero-centered sum cancels and makes the relative-error
        // cross-check meaningless — element-wise correctness is covered
        // by the gpu_tests, this is just a timing sanity gate).
        let x: Vec<f32> = (0..xn).map(|i| (i % 17) as f32 * 0.03 + 0.1).collect();
        let wt: Vec<f32> = (0..wn).map(|i| (i % 11) as f32 * 0.01 + 0.05).collect();
        let xshape = [n, cin, hw, hw];
        let wshape = [cout, cin, k, k];

        let conv_chain = |b: &dyn Backend, iters: usize| -> f64 {
            let xh = b.from_slice_f32(&x, &xshape).expect("x");
            let wh = b.from_slice_f32(&wt, &wshape).expect("w");
            let mut acc = 0.0f64;
            for _ in 0..iters {
                let y = b.conv2d(&xh, &wh, stride, pad, 1).expect("conv2d");
                acc += b.sum(&y).expect("sum"); // host-visible sync point
            }
            acc
        };

        let _ = conv_chain(&cpu, 1); // warmup
        let t = Instant::now();
        let cpu_sum = conv_chain(&cpu, iters);
        let cpu_ms = t.elapsed().as_secs_f64() * 1e3;

        match &gpu {
            Some(g) => {
                let _ = conv_chain(g, 1); // warmup (NVRTC + cuBLASLt)
                let t = Instant::now();
                let gpu_sum = conv_chain(g, iters);
                let gpu_ms = t.elapsed().as_secs_f64() * 1e3;
                let rel = (cpu_sum - gpu_sum).abs() / cpu_sum.abs().max(1e-9);
                let ok = rel < 2e-3;
                println!(
                    "{:>4} {:>5} {:>5} {:>5} {:>4} {:>5} {:>12.2} {:>12.2} {:>8.2}x  {}",
                    n, cin, cout, hw, k, iters, cpu_ms, gpu_ms,
                    cpu_ms / gpu_ms.max(1e-9),
                    if ok { "✓" } else { "✗ MISMATCH" }
                );
            }
            None => {
                println!(
                    "{:>4} {:>5} {:>5} {:>5} {:>4} {:>5} {:>12.2} {:>12} {:>9}  (cpu sum={:.2})",
                    n, cin, cout, hw, k, iters, cpu_ms, "-", "-", cpu_sum
                );
            }
        }
    }
}
