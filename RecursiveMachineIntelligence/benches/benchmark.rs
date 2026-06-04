//! Performance Benchmarks for RMI Framework
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rmi::core::codegen::{
    ActivationKind, BinaryOpKind, FunctionBuilder, IRType, IRValue, PrimitiveType, Program,
    UnaryOpKind,
};
use rmi::core::optimization::{OptimizationLevel, OptimizationPipeline};
use rmi::core::verification::Verifier;
use rmi::neural::{GradientTape, Layer, Linear, Loss, MSELoss, Optimizer, Variable, SGD};
use std::collections::HashMap;
use uuid::Uuid;

/// Build a synthetic program with `num_functions` functions, each containing
/// ~12 IR nodes, suitable for optimization and verification benchmarks.
fn build_benchmark_program(num_functions: usize) -> Program {
    let mut program = Program::new("bench_program");
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
        // Arithmetic chain: a = x * 2.0
        let c2 = fb.constant(IRValue::F64(2.0), IRType::Primitive(PrimitiveType::F64));
        let a = fb.binary_op(BinaryOpKind::Mul, x, c2);
        // b = y + 1.0
        let c1 = fb.constant(IRValue::F64(1.0), IRType::Primitive(PrimitiveType::F64));
        let b = fb.binary_op(BinaryOpKind::Add, y, c1);
        // c = a + b
        let c = fb.binary_op(BinaryOpKind::Add, a, b);
        // d = sqrt(c)
        let d = fb.unary_op(UnaryOpKind::Sqrt, c);
        // e = relu(d)
        let e = fb.activation(ActivationKind::ReLU, d);
        // f = e * e (CSE candidate)
        let f = fb.binary_op(BinaryOpKind::Mul, e, e);
        // g = x + 0.0 (algebraic simplification candidate: identity add)
        let c0 = fb.constant(IRValue::F64(0.0), IRType::Primitive(PrimitiveType::F64));
        let g = fb.binary_op(BinaryOpKind::Add, x, c0);
        // h = f + g
        let h = fb.binary_op(BinaryOpKind::Add, f, g);
        fb.ret(h);
        program.add_function(fb.build());
    }
    program
}

fn matmul_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("matmul");
    for size in [64, 128, 256] {
        let a: Vec<f32> = (0..size * size).map(|i| (i % 100) as f32 * 0.01).collect();
        let b: Vec<f32> = (0..size * size).map(|i| (i % 100) as f32 * 0.01).collect();
        group.bench_with_input(BenchmarkId::new("naive", size), &size, |bench, &s| {
            bench.iter(|| {
                let mut result = vec![0.0f32; s * s];
                for i in 0..s {
                    for j in 0..s {
                        for k in 0..s {
                            result[i * s + j] += a[i * s + k] * b[k * s + j];
                        }
                    }
                }
                black_box(result)
            })
        });
    }
    group.finish();
}

fn forward_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("forward");
    for (in_f, out_f) in [(64, 32), (256, 128), (512, 256)] {
        let layer = Linear::new(in_f, out_f);
        let input = Variable::new(vec![0.1; in_f], vec![1, in_f], false);
        group.bench_with_input(
            BenchmarkId::new("linear", format!("{}x{}", in_f, out_f)),
            &(),
            |bench, _| {
                let mut tape = GradientTape::new();
                bench.iter(|| black_box(layer.forward(&[&input], &mut tape)))
            },
        );
    }
    group.finish();
}

fn autodiff_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("autodiff");
    for size in [64, 256, 1024] {
        let data: Vec<f32> = (0..size).map(|i| (i % 100) as f32 * 0.01).collect();
        group.bench_with_input(BenchmarkId::new("backward_mul", size), &size, |bench, _| {
            bench.iter(|| {
                let mut tape = GradientTape::new();
                let a = Variable::new(data.clone(), vec![size], true);
                let b = Variable::new(data.clone(), vec![size], true);
                let a_id = tape.register(a);
                let b_id = tape.register(b);
                let c_id = tape.mul(a_id, b_id);
                let loss_id = tape.sum(c_id, None, false);
                tape.backward(loss_id);
                black_box(())
            })
        });
    }
    group.finish();
}

fn loss_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("loss");
    for size in [64, 256, 1024] {
        let pred: Vec<f32> = (0..size).map(|i| (i % 100) as f32 * 0.01).collect();
        let target: Vec<f32> = (0..size).map(|i| ((i + 10) % 100) as f32 * 0.01).collect();
        let mse = MSELoss::new();
        group.bench_with_input(BenchmarkId::new("mse", size), &size, |bench, _| {
            bench.iter(|| black_box(mse.forward(&pred, &target)))
        });
    }
    group.finish();
}

fn optimizer_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimizer");
    for num_params in [100, 1000, 10000] {
        let id = Uuid::new_v4();
        let params: Vec<f32> = (0..num_params).map(|i| (i % 100) as f32 * 0.01).collect();
        let grads: Vec<f32> = (0..num_params)
            .map(|i| ((i + 5) % 100) as f32 * 0.001)
            .collect();
        group.bench_with_input(
            BenchmarkId::new("sgd", num_params),
            &num_params,
            |bench, _| {
                let mut opt = SGD::new(0.01);
                let mut p = vec![(id, params.clone())];
                let mut g = HashMap::new();
                g.insert(id, grads.clone());
                bench.iter(|| {
                    let _: () = opt.step(&mut p, &g);
                    black_box(())
                })
            },
        );
    }
    group.finish();
}

fn activation_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("activation");
    for size in [256, 1024, 4096] {
        let data: Vec<f32> = (0..size)
            .map(|i| (i as f32 - (size / 2) as f32) * 0.01)
            .collect();
        group.bench_with_input(BenchmarkId::new("relu", size), &size, |bench, _| {
            bench.iter(|| {
                let mut tape = GradientTape::new();
                let x = Variable::new(data.clone(), vec![size], true);
                let x_id = tape.register(x);
                black_box(tape.relu(x_id))
            })
        });
    }
    group.finish();
}

fn optimization_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimization");
    for num_funcs in [1, 4, 16] {
        let program = build_benchmark_program(num_funcs);
        for level in [
            ("O0", OptimizationLevel::O0),
            ("O1", OptimizationLevel::O1),
            ("O2", OptimizationLevel::O2),
            ("O3", OptimizationLevel::O3),
        ] {
            let pipeline = OptimizationPipeline::level(level.1);
            group.bench_with_input(
                BenchmarkId::new(level.0, format!("{}funcs", num_funcs)),
                &num_funcs,
                |bench, _| bench.iter(|| black_box(pipeline.optimize(program.clone()))),
            );
        }
    }
    group.finish();
}

fn verification_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("verification");
    let verifier = Verifier::new();
    for num_funcs in [1, 4, 16] {
        let program = build_benchmark_program(num_funcs);
        group.bench_with_input(
            BenchmarkId::new("full", format!("{}funcs", num_funcs)),
            &num_funcs,
            |bench, _| bench.iter(|| black_box(verifier.verify(&program))),
        );
    }
    // Also benchmark verification after optimization (O3-optimized programs)
    for num_funcs in [1, 4, 16] {
        let program = build_benchmark_program(num_funcs);
        let pipeline = OptimizationPipeline::level(OptimizationLevel::O3);
        let optimized = pipeline.optimize(program);
        group.bench_with_input(
            BenchmarkId::new("post_opt", format!("{}funcs", num_funcs)),
            &num_funcs,
            |bench, _| bench.iter(|| black_box(verifier.verify(&optimized))),
        );
    }
    group.finish();
}

// ── BLAS benchmarks ──────────────────────────────────────────────────────────

fn blas_benchmarks(c: &mut Criterion) {
    use rmi::compute::blas::{BlasMatrix, BlasOps};

    let mut group = c.benchmark_group("blas");

    // Matrix multiply at several sizes
    for size in [32, 64, 128, 256] {
        let a = BlasMatrix::random(size, size, 42);
        let b = BlasMatrix::random(size, size, 43);
        group.bench_with_input(BenchmarkId::new("matmul", size), &size, |bench, _| {
            bench.iter(|| black_box(BlasOps::matmul(&a, &b).unwrap()))
        });
    }

    // LU decomposition
    for size in [32, 64, 128] {
        let a = BlasMatrix::random(size, size, 44);
        group.bench_with_input(BenchmarkId::new("lu", size), &size, |bench, _| {
            bench.iter(|| black_box(BlasOps::lu(&a).unwrap()))
        });
    }

    // Cholesky (needs symmetric positive-definite matrix: AᵀA + I)
    for size in [32, 64, 128] {
        let r = BlasMatrix::random(size, size, 45);
        let rt = r.transpose();
        let mut spd = BlasOps::matmul(&rt, &r).unwrap();
        // Add I to ensure positive definiteness
        for i in 0..size {
            spd.data[i * size + i] += 1.0;
        }
        group.bench_with_input(BenchmarkId::new("cholesky", size), &size, |bench, _| {
            bench.iter(|| black_box(BlasOps::cholesky(&spd).unwrap()))
        });
    }

    // QR decomposition
    for size in [32, 64, 128] {
        let a = BlasMatrix::random(size, size, 46);
        group.bench_with_input(BenchmarkId::new("qr", size), &size, |bench, _| {
            bench.iter(|| black_box(BlasOps::qr(&a).unwrap()))
        });
    }

    // Solve Ax = b
    for size in [32, 64, 128] {
        let a = BlasMatrix::random(size, size, 47);
        let b: Vec<f64> = (0..size).map(|i| i as f64 * 0.1).collect();
        group.bench_with_input(BenchmarkId::new("solve", size), &size, |bench, _| {
            bench.iter(|| black_box(BlasOps::solve(&a, &b).unwrap()))
        });
    }

    group.finish();
}

// ── JIT benchmarks ──────────────────────────────────────────────────────────

fn jit_benchmarks(c: &mut Criterion) {
    use rmi::lang::jit::{JitCompiler, JitConfig};
    use rmi::lang::{Expr, Op};

    let mut group = c.benchmark_group("jit");

    // Build progressively deeper expression chains
    for depth in [4, 8, 16, 32] {
        let mut expr = Expr::float(1.0);
        for _ in 0..depth {
            expr = expr.then(Expr::op1(Op::RELU));
        }
        let compiler = JitCompiler::new(JitConfig::default());
        group.bench_with_input(BenchmarkId::new("compile", depth), &depth, |bench, _| {
            bench.iter(|| black_box(compiler.compile(&expr).unwrap()))
        });
    }

    // JIT execution throughput
    let mut expr = Expr::op1(Op::RELU);
    for _ in 0..4 {
        expr = expr.then(Expr::op1(Op::RELU));
    }
    let compiler = JitCompiler::new(JitConfig::default());
    let func = compiler.compile(&expr).unwrap();
    group.bench_function("call_f64_chain5", |bench| {
        bench.iter(|| black_box(func.call_f64(&[1.0]).unwrap()))
    });

    // Compile + execute (cold path)
    let arith = Expr::op2(Op::ADD, Expr::float(2.0), Expr::float(3.0));
    group.bench_function("compile_and_call", |bench| {
        let compiler = JitCompiler::new(JitConfig::default());
        bench.iter(|| {
            let f = compiler.compile(&arith).unwrap();
            black_box(f.call_f64(&[]).unwrap())
        })
    });

    // Cached compilation
    group.bench_function("compile_cached", |bench| {
        let mut compiler = JitCompiler::new(JitConfig::default());
        let arith = Expr::op2(Op::ADD, Expr::float(2.0), Expr::float(3.0));
        bench.iter(|| {
            let _ = compiler.compile_cached(&arith).unwrap();
            black_box(compiler.cache_size())
        })
    });

    group.finish();
}

// ── Fusion benchmarks ───────────────────────────────────────────────────────

fn fusion_benchmarks(c: &mut Criterion) {
    use rmi::compute::fusion::{FusionConfig, FusionPass};
    use rmi::lang::{Expr, Op};

    let mut group = c.benchmark_group("fusion");

    // Elementwise chain fusion
    for chain_len in [4, 8, 16, 32] {
        let mut expr = Expr::op1(Op::RELU);
        for _ in 1..chain_len {
            expr = expr.then(Expr::op1(Op::GELU));
        }
        let pass = FusionPass::new(FusionConfig::default());
        group.bench_with_input(
            BenchmarkId::new("elementwise_chain", chain_len),
            &chain_len,
            |bench, _| bench.iter(|| black_box(pass.fuse(&expr))),
        );
    }

    // MatMul + activation fusion
    let matmul_relu = Expr::op1(Op::MATMUL).then(Expr::op1(Op::RELU));
    let pass = FusionPass::new(FusionConfig::default());
    group.bench_function("matmul_activation", |bench| {
        bench.iter(|| black_box(pass.fuse(&matmul_relu)))
    });

    // Mixed pipeline: linear → norm → relu → dropout
    let pipeline = Expr::op1(Op::LINEAR)
        .then(Expr::op1(Op::LAYER_NORM))
        .then(Expr::op1(Op::RELU))
        .then(Expr::op1(Op::DROP));
    group.bench_function("mixed_pipeline", |bench| {
        bench.iter(|| black_box(pass.fuse(&pipeline)))
    });

    group.finish();
}

// ── FFI benchmarks ──────────────────────────────────────────────────────────

fn ffi_benchmarks(c: &mut Criterion) {
    use rmi::lang::ffi::{register_math_prelude, FfiRegistry, FfiSignature};
    use rmi::lang::Val;

    let mut group = c.benchmark_group("ffi");

    // Call overhead — identity function
    let mut reg = FfiRegistry::new();
    reg.register_fn("id", FfiSignature::unary_f64(), |args| Ok(args[0].clone()));
    group.bench_function("call_identity", |bench| {
        let arg = Val::f64(42.0);
        bench.iter(|| black_box(reg.call("id", std::slice::from_ref(&arg)).unwrap()))
    });

    // call_unchecked overhead
    group.bench_function("call_unchecked_identity", |bench| {
        let arg = Val::f64(42.0);
        bench.iter(|| {
            black_box(
                reg.call_unchecked("id", std::slice::from_ref(&arg))
                    .unwrap(),
            )
        })
    });

    // Math prelude (sqrt)
    let mut math_reg = FfiRegistry::new();
    register_math_prelude(&mut math_reg);
    group.bench_function("call_sqrt", |bench| {
        let arg = Val::f64(144.0);
        bench.iter(|| {
            black_box(
                math_reg
                    .call("ffi_sqrt", std::slice::from_ref(&arg))
                    .unwrap(),
            )
        })
    });

    // Batch calls — simulate calling a function many times
    for batch in [100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("batch_sqrt", batch),
            &batch,
            |bench, &n| {
                bench.iter(|| {
                    for i in 0..n {
                        black_box(math_reg.call("ffi_sqrt", &[Val::f64(i as f64)]).unwrap());
                    }
                })
            },
        );
    }

    group.finish();
}

// ── VM benchmarks ───────────────────────────────────────────────────────────

fn vm_benchmarks(c: &mut Criterion) {
    use rmi::lang::sym::{Sym, SymbolTable};
    use rmi::lang::vm::Vm;
    use rmi::lang::{Expr, Op};

    let mut group = c.benchmark_group("vm");

    // Arithmetic expression evaluation
    for depth in [4, 8, 16, 32] {
        let mut expr = Expr::float(1.0);
        for _ in 0..depth {
            expr = Expr::op2(Op::ADD, expr, Expr::float(0.1));
        }
        group.bench_with_input(BenchmarkId::new("eval_arith", depth), &depth, |bench, _| {
            let mut vm = Vm::new_no_jit();
            bench.iter(|| black_box(vm.eval(&expr).unwrap()))
        });
    }

    // Nested let bindings
    for depth in [4, 8, 16] {
        let mut sym_table = SymbolTable::new();
        let syms: Vec<Sym> = (0..depth)
            .map(|i| sym_table.intern(&format!("x{}", i)))
            .collect();
        let mut body = Expr::Ref(syms[0]);
        for i in (0..depth).rev() {
            body = Expr::bind(
                syms[i],
                Expr::op2(Op::ADD, Expr::float(i as f32), Expr::float(1.0)),
                body,
            );
        }
        group.bench_with_input(
            BenchmarkId::new("eval_let_chain", depth),
            &depth,
            |bench, _| {
                let mut vm = Vm::new_no_jit();
                bench.iter(|| black_box(vm.eval(&body).unwrap()))
            },
        );
    }

    // JIT vs interpreter comparison
    let expr = Expr::op2(Op::ADD, Expr::float(2.0), Expr::float(3.0));
    group.bench_function("eval_interp_add", |bench| {
        let mut vm = Vm::new_no_jit();
        bench.iter(|| black_box(vm.eval(&expr).unwrap()))
    });
    group.bench_function("eval_jit_add", |bench| {
        let mut vm = Vm::new();
        bench.iter(|| black_box(vm.eval_jit(&expr).unwrap()))
    });

    group.finish();
}

// ── Codec benchmarks ────────────────────────────────────────────────────────

fn codec_benchmarks(c: &mut Criterion) {
    use rmi::lang::codec::{Decoder, Encoder};
    use rmi::lang::sym::SymbolTable;
    use rmi::lang::{Expr, Op};

    let mut group = c.benchmark_group("codec");

    // Encode/decode at various expression sizes
    for depth in [4, 8, 16, 32] {
        let mut expr = Expr::float(1.0);
        for _ in 0..depth {
            expr = Expr::op2(Op::ADD, expr, Expr::float(0.5));
        }

        group.bench_with_input(BenchmarkId::new("encode", depth), &depth, |bench, _| {
            bench.iter(|| black_box(Encoder::encode_expr_only(&expr)))
        });

        let encoded = Encoder::encode_expr_only(&expr);
        group.bench_with_input(BenchmarkId::new("decode", depth), &depth, |bench, _| {
            bench.iter(|| black_box(Decoder::decode_expr_only(&encoded).unwrap()))
        });

        // Round-trip
        group.bench_with_input(BenchmarkId::new("roundtrip", depth), &depth, |bench, _| {
            bench.iter(|| {
                let enc = Encoder::encode_expr_only(&expr);
                black_box(Decoder::decode_expr_only(&enc).unwrap())
            })
        });
    }

    // Program encode/decode with symbol table
    let sym_table = SymbolTable::new();
    let expr = Expr::op2(Op::MUL, Expr::float(3.15), Expr::float(2.0));
    group.bench_function("encode_program", |bench| {
        bench.iter(|| black_box(Encoder::encode_program(&expr, &sym_table)))
    });

    let prog_encoded = Encoder::encode_program(&expr, &sym_table);
    group.bench_function("decode_program", |bench| {
        bench.iter(|| black_box(Decoder::decode_program(&prog_encoded).unwrap()))
    });

    group.finish();
}

// ── Protocol benchmarks ─────────────────────────────────────────────────────

fn protocol_benchmarks(c: &mut Criterion) {
    use rmi::core::protocol::{Message, MessageFlags, MessageHeader, MessageType};

    let mut group = c.benchmark_group("protocol");

    // Header serialization
    let header = MessageHeader::new(MessageType::TaskRequest, MessageFlags::empty(), 1024);
    group.bench_function("header_to_bytes", |bench| {
        bench.iter(|| black_box(header.to_bytes()))
    });

    let header_bytes = header.to_bytes();
    group.bench_function("header_from_bytes", |bench| {
        bench.iter(|| black_box(MessageHeader::from_bytes(&header_bytes).unwrap()))
    });

    // Message serialization at various payload sizes
    for payload_size in [64, 256, 1024, 4096] {
        let payload: Vec<u8> = (0..payload_size).map(|i| (i % 256) as u8).collect();

        let msg = Message::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            MessageType::TaskRequest,
            payload.clone(),
        );

        group.bench_with_input(
            BenchmarkId::new("msg_to_binary", payload_size),
            &payload_size,
            |bench, _| bench.iter(|| black_box(msg.to_binary())),
        );

        let binary = msg.to_binary();
        group.bench_with_input(
            BenchmarkId::new("msg_from_binary", payload_size),
            &payload_size,
            |bench, _| bench.iter(|| black_box(Message::from_binary(&binary).unwrap())),
        );
    }

    group.finish();
}

// ── Symbolic inference benchmarks ───────────────────────────────────────────

fn inference_benchmarks(c: &mut Criterion) {
    use rmi::symbolic::inference::{InferenceConfig, InferenceEngine};
    use rmi::symbolic::logic::{KnowledgeBase, Predicate, Term};

    let mut group = c.benchmark_group("inference");

    // Build KB of various sizes
    for num_facts in [10, 50, 100] {
        let mut kb = KnowledgeBase::new();
        for i in 0..num_facts {
            kb.add_fact(
                "parent",
                vec![
                    Term::Constant(format!("person_{}", i)),
                    Term::Constant(format!("person_{}", i + 1)),
                ],
            );
        }
        // Add a rule: ancestor(X, Z) :- parent(X, Z)
        kb.add_rule(
            Predicate::new(
                "ancestor",
                vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            ),
            vec![Predicate::new(
                "parent",
                vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Z".to_string()),
                ],
            )],
        );

        // Query a known fact
        let query = Predicate::new(
            "parent",
            vec![
                Term::Constant("person_0".to_string()),
                Term::Constant("person_1".to_string()),
            ],
        );

        group.bench_with_input(
            BenchmarkId::new("query_fact", num_facts),
            &num_facts,
            |bench, _| {
                let mut engine = InferenceEngine::new(InferenceConfig::default());
                bench.iter(|| black_box(engine.query(&kb, &query)))
            },
        );

        // Query via rule
        let rule_query = Predicate::new(
            "ancestor",
            vec![
                Term::Constant("person_0".to_string()),
                Term::Constant("person_1".to_string()),
            ],
        );

        group.bench_with_input(
            BenchmarkId::new("query_rule", num_facts),
            &num_facts,
            |bench, _| {
                let mut engine = InferenceEngine::new(InferenceConfig::default());
                bench.iter(|| black_box(engine.query(&kb, &rule_query)))
            },
        );
    }

    group.finish();
}

// ── Incremental compilation benchmarks ──────────────────────────────────────

fn incremental_benchmarks(c: &mut Criterion) {
    use rmi::lang::incremental::IncrementalCache;
    use rmi::lang::{Expr, Op};

    let mut group = c.benchmark_group("incremental");

    // Cache miss (cold compile)
    for depth in [4, 8, 16] {
        let mut expr = Expr::float(1.0);
        for _ in 0..depth {
            expr = Expr::op2(Op::ADD, expr, Expr::float(0.5));
        }
        group.bench_with_input(
            BenchmarkId::new("cold_compile", depth),
            &depth,
            |bench, _| {
                bench.iter(|| {
                    let mut cache = IncrementalCache::new();
                    black_box(cache.compile(&expr).hash)
                })
            },
        );
    }

    // Cache hit (warm compile)
    for depth in [4, 8, 16] {
        let mut expr = Expr::float(1.0);
        for _ in 0..depth {
            expr = Expr::op2(Op::ADD, expr, Expr::float(0.5));
        }
        let mut cache = IncrementalCache::new();
        let _ = cache.compile(&expr); // warm up
        group.bench_with_input(
            BenchmarkId::new("warm_compile", depth),
            &depth,
            |bench, _| bench.iter_with_large_drop(|| cache.compile(&expr).hash),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    matmul_benchmarks,
    forward_benchmarks,
    autodiff_benchmarks,
    loss_benchmarks,
    optimizer_benchmarks,
    activation_benchmarks,
    optimization_benchmarks,
    verification_benchmarks,
    blas_benchmarks,
    jit_benchmarks,
    fusion_benchmarks,
    ffi_benchmarks,
    vm_benchmarks,
    codec_benchmarks,
    protocol_benchmarks,
    inference_benchmarks,
    incremental_benchmarks,
);
criterion_main!(benches);
