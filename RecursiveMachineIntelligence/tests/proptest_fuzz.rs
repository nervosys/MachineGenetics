//! Fuzz-style property-based tests for optimization, verification, emitters, and mutation.
//!
//! These tests ensure the optimization pipeline, verification system, code emitters,
//! and mutation/crossover operators never panic on arbitrary well-formed IR inputs.

use proptest::prelude::*;
use rmi::core::codegen::{
    BinaryOpKind, CodeEmitter, Crossover, FunctionBuilder, IRType, IRValue, Mutator, PrimitiveType,
    Program, RustEmitter, UnaryOpKind,
};
use rmi::core::emitters::{CudaEmitter, MlirEmitter, OnnxEmitter};
use rmi::core::optimization::{OptimizationLevel, OptimizationPipeline};
use rmi::core::verification::Verifier;

// ============================================================================
// Strategies for generating arbitrary programs
// ============================================================================

fn arb_opt_level() -> impl Strategy<Value = OptimizationLevel> {
    prop_oneof![
        Just(OptimizationLevel::O0),
        Just(OptimizationLevel::O1),
        Just(OptimizationLevel::O2),
        Just(OptimizationLevel::O3),
    ]
}

/// Build a random program with chained unary + binary operations.
/// `ops` is a list of (is_binary, op_index) pairs used to generate operations.
fn build_arb_program(name: &str, num_funcs: usize, ops: &[(bool, usize)]) -> Program {
    let mut program = Program::new(name);
    let ty = IRType::Primitive(PrimitiveType::F64);
    for (i, &(is_binary, op_idx)) in ops.iter().enumerate().take(num_funcs) {
        let mut fb = FunctionBuilder::new(
            format!("f_{}", i),
            vec![("x".into(), ty.clone()), ("y".into(), ty.clone())],
            ty.clone(),
        );
        let x = fb.param(0);
        let y = fb.param(1);

        // Add a constant (constant folding candidate)
        let c = fb.constant(IRValue::F64(2.0), ty.clone());

        let result = if is_binary {
            let bin_ops = [
                BinaryOpKind::Add,
                BinaryOpKind::Sub,
                BinaryOpKind::Mul,
                BinaryOpKind::Div,
                BinaryOpKind::Min,
                BinaryOpKind::Max,
            ];
            let op = bin_ops[op_idx % bin_ops.len()];
            let a = fb.binary_op(op, x, c);
            fb.binary_op(BinaryOpKind::Add, a, y)
        } else {
            let un_ops = [
                UnaryOpKind::Neg,
                UnaryOpKind::Abs,
                UnaryOpKind::Sqrt,
                UnaryOpKind::Exp,
                UnaryOpKind::Log,
                UnaryOpKind::Sin,
                UnaryOpKind::Cos,
                UnaryOpKind::Tanh,
            ];
            let op = un_ops[op_idx % un_ops.len()];
            let a = fb.unary_op(op, x);
            let _cse = fb.unary_op(op, x); // CSE candidate
            fb.binary_op(BinaryOpKind::Add, a, y)
        };
        fb.ret(result);
        program.add_function(fb.build());
    }
    program
}

// ============================================================================
// Fuzz: Optimization pipeline never panics on arbitrary valid programs
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn optimization_never_panics(
        level in arb_opt_level(),
        num_funcs in 1usize..4,
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..4),
    ) {
        let program = build_arb_program("fuzz_opt", num_funcs, &ops);
        let pipeline = OptimizationPipeline::level(level);
        let optimized = pipeline.optimize(program);
        prop_assert!(!optimized.functions.is_empty() || num_funcs == 0);
    }
}

// ============================================================================
// Fuzz: Optimization idempotence
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn optimization_idempotent_at_fixpoint(
        level in arb_opt_level(),
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("idem", 1, &ops);
        let pipeline = OptimizationPipeline::level(level);
        let opt1 = pipeline.optimize(program);
        let opt2 = pipeline.optimize(opt1.clone());
        prop_assert_eq!(opt1.functions.len(), opt2.functions.len());
    }
}

// ============================================================================
// Fuzz: Verification never panics on arbitrary programs
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn verification_never_panics(
        num_funcs in 1usize..4,
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..4),
    ) {
        let program = build_arb_program("fuzz_ver", num_funcs, &ops);
        let verifier = Verifier::new();
        let _report = verifier.verify(&program);
    }
}

// ============================================================================
// Fuzz: Verification on optimized programs never panics
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn verify_after_optimize_never_panics(
        level in arb_opt_level(),
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("fuzz_vo", 2, &ops);
        let pipeline = OptimizationPipeline::level(level);
        let optimized = pipeline.optimize(program);
        let verifier = Verifier::new();
        let _report = verifier.verify(&optimized);
    }
}

// ============================================================================
// Fuzz: RustEmitter never panics
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn rust_emitter_never_panics(
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("emit_rs", 1, &ops);
        let emitter = RustEmitter;
        let result = emitter.emit(&program);
        prop_assert!(result.is_ok(), "RustEmitter should not fail on valid IR");
        let code = result.unwrap();
        prop_assert!(!code.is_empty());
    }
}

// ============================================================================
// Fuzz: CudaEmitter never panics
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn cuda_emitter_never_panics(
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("emit_cuda", 1, &ops);
        let emitter = CudaEmitter::new();
        let result = emitter.emit(&program);
        prop_assert!(result.is_ok(), "CudaEmitter should not fail: {:?}", result.err());
    }
}

// ============================================================================
// Fuzz: MlirEmitter never panics
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn mlir_emitter_never_panics(
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("emit_mlir", 1, &ops);
        let emitter = MlirEmitter::new();
        let result = emitter.emit(&program);
        prop_assert!(result.is_ok(), "MlirEmitter should not fail: {:?}", result.err());
    }
}

// ============================================================================
// Fuzz: OnnxEmitter never panics
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn onnx_emitter_never_panics(
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("emit_onnx", 1, &ops);
        let emitter = OnnxEmitter::new();
        let result = emitter.emit(&program);
        prop_assert!(result.is_ok(), "OnnxEmitter should not fail: {:?}", result.err());
    }
}

// ============================================================================
// Fuzz: All emitters on optimized program
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn all_emitters_on_optimized(
        level in arb_opt_level(),
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("emit_all", 1, &ops);
        let pipeline = OptimizationPipeline::level(level);
        let optimized = pipeline.optimize(program);

        let emitters: Vec<Box<dyn CodeEmitter>> = vec![
            Box::new(RustEmitter),
            Box::new(CudaEmitter::new()),
            Box::new(MlirEmitter::new()),
            Box::new(OnnxEmitter::new()),
        ];

        for emitter in &emitters {
            let result = emitter.emit(&optimized);
            prop_assert!(
                result.is_ok(),
                "Emitter {:?} failed on optimized program: {:?}",
                emitter.target(),
                result.err()
            );
        }
    }
}

// ============================================================================
// Fuzz: Mutation never panics on arbitrary programs
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn mutation_never_panics(
        seed in any::<u64>(),
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let mut program = build_arb_program("fuzz_mut", 1, &ops);
        let mutator = Mutator::new(seed);
        let mutation = mutator.random_mutation();
        let _ = mutator.apply_mutation(&mut program, &mutation);
    }
}

// ============================================================================
// Fuzz: Crossover never panics
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn crossover_never_panics(
        seed in any::<u64>(),
        ops_a in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
        ops_b in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let parent_a = build_arb_program("parent_a", 1, &ops_a);
        let parent_b = build_arb_program("parent_b", 1, &ops_b);
        let crossover = Crossover::new(seed);
        let _ = crossover.single_point(&parent_a, &parent_b);
        let _ = crossover.uniform(&parent_a, &parent_b);
    }
}

// ============================================================================
// Fuzz: Mutated programs can still be optimized without panic
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn optimize_after_mutation_never_panics(
        seed in any::<u64>(),
        level in arb_opt_level(),
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let mut program = build_arb_program("mut_opt", 1, &ops);
        let mutator = Mutator::new(seed);
        let mutation = mutator.random_mutation();
        let _ = mutator.apply_mutation(&mut program, &mutation);
        let pipeline = OptimizationPipeline::level(level);
        let _optimized = pipeline.optimize(program);
    }
}

// ============================================================================
// Fuzz: End-to-end pipeline (build -> optimize -> verify -> emit)
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn end_to_end_pipeline_never_panics(
        level in arb_opt_level(),
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("e2e", 1, &ops);
        let pipeline = OptimizationPipeline::level(level);
        let optimized = pipeline.optimize(program);
        let verifier = Verifier::new();
        let _report = verifier.verify(&optimized);
        let emitter = RustEmitter;
        let _code = emitter.emit(&optimized);
    }
}

// ============================================================================
// Property: O0 preserves function count (identity transform)
// ============================================================================

proptest! {
    #[test]
    fn o0_preserves_function_count(
        num_funcs in 1usize..5,
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..5),
    ) {
        let program = build_arb_program("o0_id", num_funcs, &ops);
        let original_count = program.functions.len();
        let pipeline = OptimizationPipeline::level(OptimizationLevel::O0);
        let optimized = pipeline.optimize(program);
        prop_assert_eq!(original_count, optimized.functions.len(),
            "O0 should preserve function count");
    }
}

// ============================================================================
// Property: Well-formed programs pass verification
// ============================================================================

proptest! {
    #[test]
    fn well_formed_programs_verify_clean(
        ops in prop::collection::vec((any::<bool>(), 0usize..8), 1..3),
    ) {
        let program = build_arb_program("clean", 1, &ops);
        let verifier = Verifier::new();
        let result = verifier.check(&program);
        prop_assert!(result.is_ok(),
            "Well-formed program should pass verification: {:?}", result);
    }
}


// ============================================================================
// Property: VM eval never panics on simple expressions
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn vm_eval_arith_never_panics(
        a in -1e6f32..1e6f32,
        b in -1e6f32..1e6f32,
        op_idx in 0usize..4,
    ) {
        use rmi::lang::{Vm, Expr, Op, Val};
        let mut vm = Vm::new_no_jit();
        let ops = [Op::ADD, Op::SUB, Op::MUL, Op::DIV];
        let expr = Expr::App(
            ops[op_idx],
            vec![Expr::Lit(Val::f64(a as f64)), Expr::Lit(Val::f64(b as f64))],
        );
        // Should never panic, may return error for div-by-zero etc.
        let _ = vm.eval(&expr);
    }
}

// ============================================================================
// Property: VM eval nested let-bindings never panic
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn vm_eval_nested_lets_never_panics(
        depth in 1usize..6,
        val in -100.0f32..100.0f32,
    ) {
        use rmi::lang::{Vm, Expr, Val, SymbolTable};
        let mut vm = Vm::new_no_jit();
        let mut syms = SymbolTable::new();

        // Build nested let: let x0 = val in let x1 = x0 in ... xN
        let mut body = Expr::Ref(syms.intern(&format!("x{}", depth - 1)));
        for i in (0..depth).rev() {
            let sym = syms.intern(&format!("x{}", i));
            let v = if i == 0 {
                Expr::Lit(Val::f64(val as f64))
            } else {
                Expr::Ref(syms.intern(&format!("x{}", i - 1)))
            };
            body = Expr::bind(sym, v, body);
        }
        let _ = vm.eval(&body);
    }
}

// ============================================================================
// Property: Symbolic unification never panics on arbitrary terms
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn unification_never_panics(
        n_args in 0usize..4,
        func_name_idx in 0usize..3,
        var_name_idx in 0usize..3,
    ) {
        use rmi::symbolic::unification::unify;
        use rmi::symbolic::logic::Term;

        let func_names = ["f", "g", "h"];
        let var_names = ["X", "Y", "Z"];

        let args1: Vec<Term> = (0..n_args)
            .map(|i| if i % 2 == 0 {
                Term::var(var_names[i % var_names.len()])
            } else {
                Term::constant("a")
            })
            .collect();

        let args2: Vec<Term> = (0..n_args)
            .map(|i| if i % 2 == 0 {
                Term::constant("b")
            } else {
                Term::var(var_names[(i + 1) % var_names.len()])
            })
            .collect();

        let t1 = Term::func(func_names[func_name_idx], args1);
        let t2 = Term::func(func_names[var_name_idx], args2);
        // Should never panic — may succeed or fail
        let _ = unify(&t1, &t2);
    }
}

// ============================================================================
// Property: Unification occurs-check prevents infinite terms
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn unification_occurs_check_detected(
        var_name in "[A-Z][a-z]{0,3}",
    ) {
        use rmi::symbolic::unification::unify;
        use rmi::symbolic::logic::Term;

        // X = f(X) should fail occurs check
        let x = Term::var(var_name.clone());
        let fx = Term::func("f", vec![Term::var(var_name)]);
        let result = unify(&x, &fx);
        prop_assert!(result.is_err(), "Occurs check should catch X = f(X)");
    }
}

// ============================================================================
// Property: Frame encode/decode roundtrip for arbitrary payloads
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn frame_encode_decode_roundtrip(
        payload in prop::collection::vec(any::<u8>(), 0..512),
    ) {
        use rmi::distributed::transport::Frame;
        let frame = Frame::data(payload.clone());
        let encoded = frame.encode();
        let (decoded, consumed) = Frame::decode(&encoded).unwrap();
        prop_assert_eq!(consumed, encoded.len());
        prop_assert_eq!(decoded.payload, payload);
    }
}

// ============================================================================
// Property: Frame decode never panics on arbitrary bytes
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn frame_decode_arbitrary_bytes_never_panics(
        data in prop::collection::vec(any::<u8>(), 0..256),
    ) {
        use rmi::distributed::transport::Frame;
        // Should never panic — may return Ok or Err
        let _ = Frame::decode(&data);
    }
}