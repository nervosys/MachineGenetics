//! Property-based tests for the IR (Intermediate Representation)
//!
//! Uses proptest to verify structural invariants of the code generation IR.

use proptest::prelude::*;
use rmi::core::{
    ActivationKind, BinaryOpKind, Function, FunctionBuilder, IROperation, IRPrimitiveType, IRType,
    IRValue, Program, ReduceOpKind, UnaryOpKind,
};

// ============================================================================
// Strategies for generating arbitrary IR types
// ============================================================================

fn arb_primitive_type() -> impl Strategy<Value = IRPrimitiveType> {
    prop_oneof![
        Just(IRPrimitiveType::Void),
        Just(IRPrimitiveType::Bool),
        Just(IRPrimitiveType::I8),
        Just(IRPrimitiveType::I16),
        Just(IRPrimitiveType::I32),
        Just(IRPrimitiveType::I64),
        Just(IRPrimitiveType::U8),
        Just(IRPrimitiveType::U16),
        Just(IRPrimitiveType::U32),
        Just(IRPrimitiveType::U64),
        Just(IRPrimitiveType::F16),
        Just(IRPrimitiveType::F32),
        Just(IRPrimitiveType::F64),
        Just(IRPrimitiveType::BF16),
    ]
}

fn arb_ir_type() -> impl Strategy<Value = IRType> {
    prop_oneof![
        arb_primitive_type().prop_map(IRType::Primitive),
        (
            arb_primitive_type(),
            prop::collection::vec(1usize..64, 1..4)
        )
            .prop_map(|(dtype, shape)| IRType::tensor(dtype, shape)),
    ]
}

fn _arb_ir_value() -> impl Strategy<Value = IRValue> {
    prop_oneof![
        any::<bool>().prop_map(IRValue::Bool),
        any::<i64>().prop_map(IRValue::I64),
        any::<u64>().prop_map(IRValue::U64),
        (-1e10f64..1e10f64).prop_map(IRValue::F64),
        "[a-z]{1,16}".prop_map(IRValue::String),
    ]
}

fn arb_binary_op() -> impl Strategy<Value = BinaryOpKind> {
    prop_oneof![
        Just(BinaryOpKind::Add),
        Just(BinaryOpKind::Sub),
        Just(BinaryOpKind::Mul),
        Just(BinaryOpKind::Div),
        Just(BinaryOpKind::Pow),
        Just(BinaryOpKind::Min),
        Just(BinaryOpKind::Max),
        Just(BinaryOpKind::And),
        Just(BinaryOpKind::Or),
        Just(BinaryOpKind::Eq),
        Just(BinaryOpKind::Lt),
        Just(BinaryOpKind::Gt),
    ]
}

fn arb_unary_op() -> impl Strategy<Value = UnaryOpKind> {
    prop_oneof![
        Just(UnaryOpKind::Neg),
        Just(UnaryOpKind::Abs),
        Just(UnaryOpKind::Sqrt),
        Just(UnaryOpKind::Exp),
        Just(UnaryOpKind::Log),
        Just(UnaryOpKind::Sin),
        Just(UnaryOpKind::Cos),
        Just(UnaryOpKind::Tanh),
    ]
}

fn arb_activation() -> impl Strategy<Value = ActivationKind> {
    prop_oneof![
        Just(ActivationKind::ReLU),
        Just(ActivationKind::LeakyReLU),
        Just(ActivationKind::GeLU),
        Just(ActivationKind::SiLU),
        Just(ActivationKind::Sigmoid),
        Just(ActivationKind::Tanh),
        Just(ActivationKind::Softmax),
    ]
}

fn arb_reduce_op() -> impl Strategy<Value = ReduceOpKind> {
    prop_oneof![
        Just(ReduceOpKind::Sum),
        Just(ReduceOpKind::Mean),
        Just(ReduceOpKind::Max),
        Just(ReduceOpKind::Min),
        Just(ReduceOpKind::Prod),
    ]
}

// ============================================================================
// Property: IRType compatibility is reflexive
// ============================================================================

proptest! {
    #[test]
    fn ir_type_reflexive_compatibility(ty in arb_ir_type()) {
        prop_assert!(ty.is_compatible(&ty),
            "Every type should be compatible with itself: {:?}", ty);
    }
}

// ============================================================================
// Property: Function verification catches invalid references
// ============================================================================

proptest! {
    #[test]
    fn function_verify_detects_dangling_inputs(
        bad_ref in 90000u64..99999u64,
    ) {
        let mut func = Function::new(
            "test",
            vec![("x".into(), IRType::Primitive(IRPrimitiveType::F32))],
            IRType::Primitive(IRPrimitiveType::F32),
        );
        // Add a valid parameter node
        let param_id = func.add_node(
            IROperation::Parameter { index: 0, name: "x".into() },
            IRType::Primitive(IRPrimitiveType::F32),
            vec![],
        );
        // Add a node referencing a non-existent input
        func.add_node(
            IROperation::UnaryOp { op: UnaryOpKind::Neg },
            IRType::Primitive(IRPrimitiveType::F32),
            vec![bad_ref],
        );
        let result = func.verify();
        prop_assert!(result.is_err(),
            "Function with dangling input {} should fail verification", bad_ref);
        let _ = param_id; // used above
    }
}

// ============================================================================
// Property: Valid function graphs always pass verification
// ============================================================================

proptest! {
    #[test]
    fn valid_function_passes_verify(
        op in arb_unary_op(),
        dtype in arb_primitive_type(),
    ) {
        let ty = IRType::Primitive(dtype);
        let mut func = Function::new("f", vec![("x".into(), ty.clone())], ty.clone());
        let param = func.add_node(
            IROperation::Parameter { index: 0, name: "x".into() },
            ty.clone(),
            vec![],
        );
        let result = func.add_node(
            IROperation::UnaryOp { op },
            ty.clone(),
            vec![param],
        );
        func.set_return(result);
        prop_assert!(func.verify().is_ok(),
            "Valid unary function should verify");
    }
}

// ============================================================================
// Property: Binary op functions with valid wiring pass verification
// ============================================================================

proptest! {
    #[test]
    fn valid_binary_function_passes_verify(op in arb_binary_op()) {
        let ty = IRType::Primitive(IRPrimitiveType::F32);
        let mut func = Function::new(
            "binop",
            vec![("a".into(), ty.clone()), ("b".into(), ty.clone())],
            ty.clone(),
        );
        let a = func.add_node(
            IROperation::Parameter { index: 0, name: "a".into() },
            ty.clone(),
            vec![],
        );
        let b = func.add_node(
            IROperation::Parameter { index: 1, name: "b".into() },
            ty.clone(),
            vec![],
        );
        let result = func.add_node(
            IROperation::BinaryOp { op },
            ty.clone(),
            vec![a, b],
        );
        func.set_return(result);
        prop_assert!(func.verify().is_ok());
    }
}

// ============================================================================
// Property: Program structural hash is deterministic
// ============================================================================

proptest! {
    #[test]
    fn structural_hash_computed(
        name in "[a-z]{1,8}",
        op in arb_unary_op(),
    ) {
        // Verify that structural hash is always computed (not None) for
        // a non-trivial program.
        let ty = IRType::Primitive(IRPrimitiveType::F32);
        let mut func = Function::new(&name, vec![("x".into(), ty.clone())], ty.clone());
        let p = func.add_node(
            IROperation::Parameter { index: 0, name: "x".into() },
            ty.clone(),
            vec![],
        );
        let r = func.add_node(
            IROperation::UnaryOp { op },
            ty.clone(),
            vec![p],
        );
        func.set_return(r);

        let mut prog = Program::new("test");
        prog.add_function(func);
        prog.compute_structural_hash();

        prop_assert!(prog.metadata.structural_hash.is_some(),
            "Structural hash should be computed");

        // Hash should be stable when re-computed on the same program
        let h1 = prog.metadata.structural_hash;
        prog.compute_structural_hash();
        let h2 = prog.metadata.structural_hash;
        prop_assert_eq!(h1, h2, "Hash should be stable on same program");
    }
}

// ============================================================================
// Property: IRValue roundtrips through serde
// ============================================================================

proptest! {
    #[test]
    fn ir_value_serde_roundtrip(val in prop_oneof![
        any::<bool>().prop_map(IRValue::Bool),
        any::<i64>().prop_map(IRValue::I64),
        any::<u64>().prop_map(IRValue::U64),
        "[a-z]{1,16}".prop_map(IRValue::String),
    ]) {
        let json = serde_json::to_string(&val).unwrap();
        let restored: IRValue = serde_json::from_str(&json).unwrap();
        let json2 = serde_json::to_string(&restored).unwrap();
        prop_assert_eq!(json, json2, "IRValue should roundtrip through JSON");
    }
}

// ============================================================================
// Property: IRType serde roundtrip
// ============================================================================

proptest! {
    #[test]
    fn ir_type_serde_roundtrip(ty in arb_ir_type()) {
        let json = serde_json::to_string(&ty).unwrap();
        let restored: IRType = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(ty, restored, "IRType should roundtrip through JSON");
    }
}

// ============================================================================
// Property: FunctionBuilder always produces valid functions
// ============================================================================

proptest! {
    #[test]
    fn function_builder_produces_valid_functions(
        num_ops in 1usize..5,
        ops in prop::collection::vec(arb_unary_op(), 1..5),
    ) {
        let ty = IRType::Primitive(IRPrimitiveType::F32);
        let mut fb = FunctionBuilder::new("chain", vec![("x".into(), ty.clone())], ty.clone());
        let mut current = fb.param(0);
        for op in ops.iter().take(num_ops) {
            current = fb.unary_op(*op, current);
        }
        fb.ret(current);
        let func = fb.build();
        prop_assert!(func.verify().is_ok(),
            "FunctionBuilder chain of {} ops should produce valid function", num_ops);
    }
}

// ============================================================================
// Property: Neural IR operations - activation always valid
// ============================================================================

proptest! {
    #[test]
    fn activation_node_valid(act in arb_activation()) {
        let ty = IRType::Primitive(IRPrimitiveType::F32);
        let mut func = Function::new("act", vec![("x".into(), ty.clone())], ty.clone());
        let p = func.add_node(
            IROperation::Parameter { index: 0, name: "x".into() },
            ty.clone(),
            vec![],
        );
        let r = func.add_node(
            IROperation::Activation { kind: act },
            ty.clone(),
            vec![p],
        );
        func.set_return(r);
        prop_assert!(func.verify().is_ok());
    }
}

// ============================================================================
// Property: Reduce operations valid with valid axes
// ============================================================================

proptest! {
    #[test]
    fn reduce_op_node_valid(
        op in arb_reduce_op(),
        axis in 0i32..3,
    ) {
        let ty = IRType::tensor(IRPrimitiveType::F32, vec![8, 4, 2]);
        let out_ty = IRType::Primitive(IRPrimitiveType::F32);
        let mut func = Function::new("reduce", vec![("t".into(), ty.clone())], out_ty.clone());
        let p = func.add_node(
            IROperation::Parameter { index: 0, name: "t".into() },
            ty,
            vec![],
        );
        let r = func.add_node(
            IROperation::Reduce { op, axes: vec![axis] },
            out_ty.clone(),
            vec![p],
        );
        func.set_return(r);
        prop_assert!(func.verify().is_ok());
    }
}

// ============================================================================
// Property: Empty program verifies
// ============================================================================

#[test]
fn empty_program_verifies() {
    let prog = Program::new("empty");
    assert!(prog.verify().is_ok());
}

// ============================================================================
// Property: function with return pointing to non-existent node fails
// ============================================================================

#[test]
fn function_bad_return_fails() {
    let ty = IRType::Primitive(IRPrimitiveType::F32);
    let mut func = Function::new("bad", vec![], ty);
    func.set_return(999999);
    assert!(func.verify().is_err());
}
