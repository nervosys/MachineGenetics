//! PGO (Profile-Guided Optimization) training workload.
//!
//! This binary exercises the hottest code paths in the RMI crate so that
//! LLVM's PGO infrastructure can collect accurate branch / call-site
//! frequency data.  The resulting profile is then fed back into a second
//! compilation pass to produce a faster release binary.
//!
//! # Usage
//!
//! ```bash
//! # 1. Build with instrumentation
//! RUSTFLAGS="-Cprofile-generate=./target/pgo" \
//!     cargo build --profile pgo-gen --example pgo_training
//!
//! # 2. Run the training workload (this binary)
//! ./target/pgo-gen/examples/pgo_training
//!
//! # 3. Merge raw profiles  (requires llvm-profdata on PATH)
//! llvm-profdata merge -o ./target/pgo/merged.profdata ./target/pgo
//!
//! # 4. Rebuild with the merged profile
//! RUSTFLAGS="-Cprofile-use=$(pwd)/target/pgo/merged.profdata" \
//!     cargo build --profile pgo-use
//! ```

use rmi::core::codegen::{
    ActivationKind, BinaryOpKind, FunctionBuilder, IRType, IRValue, PrimitiveType, Program,
    UnaryOpKind,
};
use rmi::core::optimization::{OptimizationLevel, OptimizationPipeline};
use rmi::core::verification::Verifier;
use rmi::lang::codec::{Decoder, Encoder};
use rmi::lang::sym::SymbolTable;
use rmi::lang::vm::Vm;
use rmi::lang::{Expr, Op};
use rmi::neural::{GradientTape, Layer, Linear, Loss, MSELoss, Variable};
use rmi::symbolic::inference::{InferenceConfig, InferenceEngine};
use rmi::symbolic::logic::{KnowledgeBase, Predicate, Term};

const ITERS: usize = 200;

fn main() {
    eprintln!("[pgo] Starting PGO training workload …");

    // ── 1. Neural forward / backward ────────────────────────────────────
    eprintln!("[pgo]  1/7  Neural forward + backward");
    for _ in 0..ITERS {
        let layer = Linear::new(128, 64);
        let input = Variable::new(vec![0.1; 128], vec![1, 128], true);
        let mut tape = GradientTape::new();
        let out = layer.forward(&[&input], &mut tape);
        let target = Variable::new(vec![0.5; 64], vec![1, 64], false);
        let loss_fn = MSELoss::new();
        let loss_val = loss_fn.forward(&out.data, &target.data);
        let grad = loss_fn.backward(&out.data, &target.data);
        std::hint::black_box((&loss_val, &grad));
    }

    // ── 2. Matmul (hot inner loop) ──────────────────────────────────────
    eprintln!("[pgo]  2/7  Matrix multiply");
    let size = 128;
    let a: Vec<f32> = (0..size * size).map(|i| (i % 100) as f32 * 0.01).collect();
    let b: Vec<f32> = (0..size * size).map(|i| (i % 100) as f32 * 0.01).collect();
    for _ in 0..ITERS / 10 {
        let mut result = vec![0.0f32; size * size];
        for i in 0..size {
            for j in 0..size {
                for k in 0..size {
                    result[i * size + j] += a[i * size + k] * b[k * size + j];
                }
            }
        }
        std::hint::black_box(&result);
    }

    // ── 3. IR optimization pipeline ─────────────────────────────────────
    eprintln!("[pgo]  3/7  IR optimization pipeline");
    let program = build_program(8);
    for _ in 0..ITERS {
        for &level in &[
            OptimizationLevel::O0,
            OptimizationLevel::O1,
            OptimizationLevel::O2,
            OptimizationLevel::O3,
        ] {
            let pipeline = OptimizationPipeline::level(level);
            let _ = pipeline.optimize(program.clone());
        }
    }

    // ── 4. IR verification ──────────────────────────────────────────────
    eprintln!("[pgo]  4/7  IR verification");
    let verifier = Verifier::new();
    for _ in 0..ITERS {
        let _ = verifier.verify(&program);
    }

    // ── 5. VM eval + JIT ────────────────────────────────────────────────
    eprintln!("[pgo]  5/7  VM eval / JIT");
    let expr = build_deep_expr(32);
    for _ in 0..ITERS {
        let mut vm = Vm::new_no_jit();
        let _ = vm.eval(&expr);
    }
    for _ in 0..ITERS {
        let mut vm = Vm::new();
        let _ = vm.eval_jit(&expr);
    }

    // ── 6. Codec encode / decode ────────────────────────────────────────
    eprintln!("[pgo]  6/7  Codec roundtrip");
    let sym_table = SymbolTable::new();
    for depth in [4, 8, 16, 32] {
        let expr = build_deep_expr(depth);
        for _ in 0..ITERS {
            let encoded = Encoder::encode_expr_only(&expr);
            let _ = Decoder::decode_expr_only(&encoded);
            let prog_enc = Encoder::encode_program(&expr, &sym_table);
            let _ = Decoder::decode_program(&prog_enc);
        }
    }

    // ── 7. Symbolic inference ───────────────────────────────────────────
    eprintln!("[pgo]  7/7  Symbolic inference");
    let mut kb = KnowledgeBase::new();
    for i in 0..100 {
        kb.add_fact(
            "parent",
            vec![
                Term::constant(format!("p{}", i)),
                Term::constant(format!("p{}", i + 1)),
            ],
        );
    }
    kb.add_rule(
        Predicate::new(
            "ancestor",
            vec![
                Term::var("X"),
                Term::var("Z"),
            ],
        ),
        vec![Predicate::new(
            "parent",
            vec![
                Term::var("X"),
                Term::var("Z"),
            ],
        )],
    );
    for _ in 0..ITERS {
        let mut engine = InferenceEngine::new(InferenceConfig::default());
        let q = Predicate::new(
            "parent",
            vec![
                Term::constant("p0"),
                Term::constant("p1"),
            ],
        );
        let _ = engine.query(&kb, &q);
        let q2 = Predicate::new(
            "ancestor",
            vec![
                Term::constant("p0"),
                Term::constant("p1"),
            ],
        );
        let _ = engine.query(&kb, &q2);
    }

    eprintln!("[pgo] Done — profile data written to ./target/pgo/");
}

/// Build a synthetic IR program.
fn build_program(num_functions: usize) -> Program {
    let mut program = Program::new("pgo_program");
    for i in 0..num_functions {
        let mut fb = FunctionBuilder::new(
            format!("func_{}", i),
            vec![
                ("x".into(), IRType::Primitive(PrimitiveType::F64)),
                ("y".into(), IRType::Primitive(PrimitiveType::F64)),
            ],
            IRType::Primitive(PrimitiveType::F64),
        );
        let x = fb.param(0);
        let y = fb.param(1);
        let c2 = fb.constant(IRValue::F64(2.0), IRType::Primitive(PrimitiveType::F64));
        let a = fb.binary_op(BinaryOpKind::Mul, x, c2);
        let c1 = fb.constant(IRValue::F64(1.0), IRType::Primitive(PrimitiveType::F64));
        let b = fb.binary_op(BinaryOpKind::Add, y, c1);
        let c = fb.binary_op(BinaryOpKind::Add, a, b);
        let d = fb.unary_op(UnaryOpKind::Sqrt, c);
        let e = fb.activation(ActivationKind::ReLU, d);
        let f = fb.binary_op(BinaryOpKind::Mul, e, e);
        let c0 = fb.constant(IRValue::F64(0.0), IRType::Primitive(PrimitiveType::F64));
        let g = fb.binary_op(BinaryOpKind::Add, x, c0);
        let h = fb.binary_op(BinaryOpKind::Add, f, g);
        fb.ret(h);
        program.add_function(fb.build());
    }
    program
}

/// Build a deep arithmetic expression tree.
fn build_deep_expr(depth: usize) -> Expr {
    let mut expr = Expr::float(1.0);
    for _ in 0..depth {
        expr = Expr::op2(Op::ADD, expr, Expr::float(0.5));
    }
    expr
}
