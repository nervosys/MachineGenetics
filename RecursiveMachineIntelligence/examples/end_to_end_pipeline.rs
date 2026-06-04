//! End-to-End IR Pipeline Example
//!
//! Demonstrates the full RMI compilation pipeline:
//! 1. **Build** an IR program using `FunctionBuilder`
//! 2. **Verify** correctness with the composite `Verifier`
//! 3. **Optimize** with the multi-pass `OptimizationPipeline`
//! 4. **Emit** target code via `CudaEmitter` and `MlirEmitter`
//!
//! This is the canonical workflow for using RMI as an ML compiler IR.

use rmi::core::codegen::{
    ActivationKind, BinaryOpKind, CodeEmitter, FunctionBuilder, IRType, IRValue, NormalizeKind,
    PrimitiveType, Program,
};
use rmi::core::emitters::{CudaEmitter, MlirEmitter};
use rmi::core::optimization::{OptimizationLevel, OptimizationPipeline};
use rmi::core::verification::Verifier;

fn main() {
    println!("═══════════════════════════════════════════════════════════");
    println!("  RMI End-to-End IR Pipeline");
    println!("═══════════════════════════════════════════════════════════\n");

    // ── Step 1: Build the IR ────────────────────────────────────────

    println!("Step 1: Building IR program...\n");

    let batch = 32;
    let features = 784;
    let hidden = 256;
    let classes = 10;

    let input_ty = IRType::tensor(PrimitiveType::F32, vec![batch, features]);
    let w1_ty = IRType::tensor(PrimitiveType::F32, vec![features, hidden]);
    let b1_ty = IRType::tensor(PrimitiveType::F32, vec![hidden]);
    let w2_ty = IRType::tensor(PrimitiveType::F32, vec![hidden, classes]);
    let b2_ty = IRType::tensor(PrimitiveType::F32, vec![classes]);
    let output_ty = IRType::tensor(PrimitiveType::F32, vec![batch, classes]);

    // Build a two-layer MLP: input -> Linear -> LayerNorm -> ReLU -> Linear -> Softmax
    let mut fb = FunctionBuilder::new(
        "mlp_forward",
        vec![
            ("x".to_string(), input_ty),
            ("w1".to_string(), w1_ty),
            ("b1".to_string(), b1_ty),
            ("w2".to_string(), w2_ty),
            ("b2".to_string(), b2_ty),
        ],
        output_ty,
    );

    // Layer 1: x @ w1 + b1
    let x = fb.param(0);
    let w1 = fb.param(1);
    let b1 = fb.param(2);
    let w2 = fb.param(3);
    let b2 = fb.param(4);

    let h = fb.matmul(x, w1, false, false);
    let h = fb.binary_op(BinaryOpKind::Add, h, b1);

    // LayerNorm + ReLU activation
    let h = fb.normalize(NormalizeKind::LayerNorm, h, 1e-5);
    let h = fb.activation(ActivationKind::ReLU, h);

    // Layer 2: h @ w2 + b2
    let logits = fb.matmul(h, w2, false, false);
    let logits = fb.binary_op(BinaryOpKind::Add, logits, b2);

    // Softmax output
    let output = fb.activation(ActivationKind::Softmax, logits);
    fb.ret(output);

    let func = fb.build();

    // Also build a small constant-foldable helper to show optimization
    let mut fb2 = FunctionBuilder::new(
        "scale_factor",
        vec![],
        IRType::Primitive(PrimitiveType::F32),
    );
    let two = fb2.constant(IRValue::F64(2.0), IRType::Primitive(PrimitiveType::F32));
    let three = fb2.constant(IRValue::F64(3.0), IRType::Primitive(PrimitiveType::F32));
    let six = fb2.binary_op(BinaryOpKind::Mul, two, three);
    fb2.ret(six);
    let func2 = fb2.build();

    let mut program = Program::new("mnist_mlp");
    program.add_function(func);
    program.add_function(func2);

    println!(
        "  Program '{}': {} functions, {} nodes total",
        program.name,
        program.functions.len(),
        program
            .functions
            .iter()
            .map(|f| f.nodes.len())
            .sum::<usize>(),
    );
    for f in &program.functions {
        println!("    - {}() with {} nodes", f.name, f.nodes.len());
    }

    // ── Step 2: Verify ──────────────────────────────────────────────

    println!("\nStep 2: Verifying IR...\n");

    let verifier = Verifier::new();
    let report = verifier.verify(&program);

    println!("  Passes run: {:?}", verifier.pass_names());
    println!(
        "  Result: {} errors, {} warnings",
        report.errors().len(),
        report.warnings().len(),
    );

    if !report.is_ok() {
        for diag in report.errors() {
            println!("  ERROR {}: {}", diag.code, diag.message);
        }
    }

    // Proceed even with warnings
    for diag in report.warnings() {
        println!("  WARN  {}: {}", diag.code, diag.message);
    }

    // ── Step 3: Optimize ────────────────────────────────────────────

    println!("\nStep 3: Optimizing IR (O2 pipeline)...\n");

    let pipeline = OptimizationPipeline::level(OptimizationLevel::O2);
    println!("  Pipeline passes: {:?}", pipeline.pass_names());

    let optimized = pipeline.optimize(program.clone());
    let original_nodes: usize = program.functions.iter().map(|f| f.nodes.len()).sum();
    let optimized_nodes: usize = optimized.functions.iter().map(|f| f.nodes.len()).sum();

    println!("  Before: {} nodes", original_nodes);
    println!("  After:  {} nodes", optimized_nodes);
    if original_nodes > 0 {
        let reduction = (1.0 - optimized_nodes as f64 / original_nodes as f64) * 100.0;
        println!("  Reduction: {:.1}%", reduction);
    }

    // ── Step 4: Emit Code ───────────────────────────────────────────

    println!("\nStep 4: Emitting CUDA/PTX code...\n");

    let emitter = CudaEmitter::new();
    match emitter.emit(&optimized) {
        Ok(code) => {
            // Show first 40 lines
            let lines: Vec<&str> = code.lines().collect();
            let show = lines.len().min(40);
            for line in &lines[..show] {
                println!("  {}", line);
            }
            if lines.len() > show {
                println!("  ... ({} more lines)", lines.len() - show);
            }
            println!(
                "\n  Total output: {} lines, {} bytes",
                lines.len(),
                code.len()
            );
        }
        Err(e) => {
            println!("  Emission error: {}", e);
        }
    }

    // ── Step 5: Also emit MLIR ──────────────────────────────────────

    println!("\nStep 5: Emitting MLIR dialect...\n");

    let mlir_emitter = MlirEmitter::new();
    match mlir_emitter.emit(&optimized) {
        Ok(code) => {
            let lines: Vec<&str> = code.lines().collect();
            let show = lines.len().min(20);
            for line in &lines[..show] {
                println!("  {}", line);
            }
            if lines.len() > show {
                println!("  ... ({} more lines)", lines.len() - show);
            }
        }
        Err(e) => {
            println!("  Emission error: {}", e);
        }
    }

    println!("\n═══════════════════════════════════════════════════════════");
    println!("  Pipeline complete!");
    println!("═══════════════════════════════════════════════════════════");
}
