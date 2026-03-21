//! # MIR → Redox MLIR Dialect Translation Layer
//!
//! Translates MIR (Mid-level Intermediate Representation) constructs into
//! Redox MLIR dialect operations. This is a thin boundary layer where MIR
//! semantics map directly to dialect operations.
//!
//! ## Simplified MIR Representation
//!
//! This module defines a simplified MIR model (`MirBody`, `BasicBlock`,
//! `Statement`, `Terminator`, `Place`, `Operand`, `Rvalue`) that mirrors
//! the actual `redox_middle::mir` types. In production, these would be
//! constructed from the real MIR; here they serve as the translation input.
//!
//! ## Translation Rules
//!
//! | MIR Construct            | Redox MLIR Operation           |
//! | ------------------------ | ------------------------------ |
//! | `Assign(place, Use(op))` | `redox.move` or `redox.copy`   |
//! | `Assign(_, Ref(..borrow))`| `redox.borrow`                |
//! | `Drop(place)`            | `redox.drop`                   |
//! | `Call { func, .. }`      | `redox.effect.perform` (if effectful) or plain call |
//! | `Goto { target }`        | control-flow (no dialect op)   |
//! | `SwitchInt { discr, .. }`| control-flow (no dialect op)   |
//! | `Return`                 | control-flow (no dialect op)   |

use crate::dialect::*;
use std::fmt;

// ===========================================================================
// Simplified MIR types
// ===========================================================================

/// A MIR function body.
#[derive(Debug, Clone)]
pub struct MirBody {
    pub name: String,
    pub blocks: Vec<BasicBlock>,
    pub local_types: Vec<LocalType>,
}

/// Type information for a MIR local variable.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalType {
    pub local: Local,
    pub type_name: String,
    pub is_copy: bool,
}

/// A MIR local variable index.
pub type Local = usize;

/// A MIR basic block.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: String,
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

/// A MIR statement.
#[derive(Debug, Clone)]
pub enum Statement {
    /// `place = rvalue`
    Assign(Place, Rvalue),
    /// `StorageLive(local)` — allocate stack slot.
    StorageLive(Local),
    /// `StorageDead(local)` — deallocate stack slot.
    StorageDead(Local),
    /// No-op.
    Nop,
}

/// A MIR terminator.
#[derive(Debug, Clone)]
pub enum Terminator {
    /// Unconditional branch.
    Goto { target: usize },
    /// Switch on an integer discriminant.
    SwitchInt { discr: Operand, targets: Vec<usize>, otherwise: usize },
    /// Return from function.
    Return,
    /// Unreachable code path.
    Unreachable,
    /// Drop a place, then branch.
    Drop { place: Place, target: usize },
    /// Function call.
    Call {
        func: String,
        args: Vec<Operand>,
        destination: Place,
        target: Option<usize>,
    },
}

/// A MIR place (local + projections).
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub local: Local,
    pub projections: Vec<Projection>,
}

impl Place {
    pub fn local(l: Local) -> Self {
        Place { local: l, projections: vec![] }
    }

    pub fn field(l: Local, field: usize) -> Self {
        Place { local: l, projections: vec![Projection::Field(field)] }
    }

    pub fn deref(l: Local) -> Self {
        Place { local: l, projections: vec![Projection::Deref] }
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_{}", self.local)?;
        for proj in &self.projections {
            match proj {
                Projection::Deref => write!(f, ".*")?,
                Projection::Field(i) => write!(f, ".{i}")?,
                Projection::Index(l) => write!(f, "[_{}]", l)?,
            }
        }
        Ok(())
    }
}

/// A place projection element.
#[derive(Debug, Clone, PartialEq)]
pub enum Projection {
    Deref,
    Field(usize),
    Index(Local),
}

/// A MIR operand.
#[derive(Debug, Clone)]
pub enum Operand {
    /// Copy the value from a place (place remains valid).
    Copy(Place),
    /// Move the value from a place (place becomes invalid).
    Move(Place),
    /// A constant value.
    Constant(String),
}

/// A MIR rvalue — the right-hand side of an assignment.
#[derive(Debug, Clone)]
pub enum Rvalue {
    /// Use an operand as-is.
    Use(Operand),
    /// Create a reference: `&place` or `&mut place`.
    Ref { mutable: bool, place: Place },
    /// Binary operation.
    BinaryOp(BinOp, Operand, Operand),
    /// Unary operation.
    UnaryOp(UnOp, Operand),
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Rem,
    Eq, Ne, Lt, Le, Gt, Ge,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

// ===========================================================================
// Translation context
// ===========================================================================

/// Context for translating a MIR body to Redox MLIR dialect operations.
pub struct TranslationContext {
    /// Known effects for function calls (function name → effect names).
    pub effectful_functions: Vec<(String, Vec<String>)>,
}

impl TranslationContext {
    pub fn new() -> Self {
        Self { effectful_functions: vec![] }
    }

    /// Register a function as having algebraic effects.
    pub fn register_effectful(&mut self, func: impl Into<String>, effects: Vec<String>) {
        self.effectful_functions.push((func.into(), effects));
    }

    /// Look up effects for a function call.
    fn effects_of(&self, func: &str) -> Option<&[String]> {
        self.effectful_functions
            .iter()
            .find(|(name, _)| name == func)
            .map(|(_, effects)| effects.as_slice())
    }
}

impl Default for TranslationContext {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Translation: MirBody → Vec<RedoxOp>
// ===========================================================================

/// Translate an entire MIR body into a sequence of Redox MLIR dialect operations.
pub fn translate_body(body: &MirBody, ctx: &TranslationContext) -> Vec<TranslatedBlock> {
    body.blocks
        .iter()
        .map(|bb| translate_block(bb, body, ctx))
        .collect()
}

/// A translated basic block.
#[derive(Debug, Clone)]
pub struct TranslatedBlock {
    pub label: String,
    pub ops: Vec<RedoxOp>,
    pub terminator_ops: Vec<RedoxOp>,
}

/// Translate a single basic block.
fn translate_block(
    bb: &BasicBlock,
    body: &MirBody,
    ctx: &TranslationContext,
) -> TranslatedBlock {
    let mut ops = Vec::new();

    for stmt in &bb.statements {
        ops.extend(translate_statement(stmt, body));
    }

    let terminator_ops = translate_terminator(&bb.terminator, body, ctx);

    TranslatedBlock {
        label: bb.label.clone(),
        ops,
        terminator_ops,
    }
}

/// Translate a MIR statement to zero or more Redox MLIR ops.
fn translate_statement(stmt: &Statement, body: &MirBody) -> Vec<RedoxOp> {
    match stmt {
        Statement::Assign(place, rvalue) => translate_assign(place, rvalue, body),
        Statement::StorageLive(_) | Statement::StorageDead(_) | Statement::Nop => vec![],
    }
}

/// Translate an assignment statement.
fn translate_assign(place: &Place, rvalue: &Rvalue, body: &MirBody) -> Vec<RedoxOp> {
    match rvalue {
        Rvalue::Use(Operand::Move(src)) => {
            let ty = type_for_local(src.local, body);
            vec![RedoxOp::Move(MoveOp {
                source_type: ty.clone(),
                result_type: ty,
            })]
        }
        Rvalue::Use(Operand::Copy(src)) => {
            let ty = type_for_local(src.local, body);
            vec![RedoxOp::Copy(CopyOp { source_type: ty })]
        }
        Rvalue::Use(Operand::Constant(_)) => {
            // Constants don't generate ownership ops
            vec![]
        }
        Rvalue::Ref { mutable, place: src } => {
            let mode = if *mutable { BorrowMode::Exclusive } else { BorrowMode::Shared };
            let ty = type_for_local(src.local, body);
            vec![RedoxOp::Borrow(BorrowOp {
                source_type: ty,
                mode,
                region: region_type(format!("'_{}", place.local)),
            })]
        }
        Rvalue::BinaryOp(_, _, _) | Rvalue::UnaryOp(_, _) => {
            // Arithmetic ops don't generate ownership dialect ops
            vec![]
        }
    }
}

/// Translate a MIR terminator to dialect ops.
fn translate_terminator(
    term: &Terminator,
    body: &MirBody,
    ctx: &TranslationContext,
) -> Vec<RedoxOp> {
    match term {
        Terminator::Drop { place, .. } => {
            let ty = type_for_local(place.local, body);
            vec![RedoxOp::Drop(DropOp { value_type: ty })]
        }
        Terminator::Call { func, args, .. } => {
            if let Some(effects) = ctx.effects_of(func) {
                // Effectful call → effect.perform for each effect
                effects
                    .iter()
                    .map(|eff| {
                        RedoxOp::EffectPerform(EffectPerformOp {
                            effect: effect_type(eff),
                            arg_types: args
                                .iter()
                                .filter_map(|a| match a {
                                    Operand::Copy(p) | Operand::Move(p) => {
                                        Some(type_for_local(p.local, body))
                                    }
                                    Operand::Constant(_) => None,
                                })
                                .collect(),
                            result_type: None,
                        })
                    })
                    .collect()
            } else {
                // Non-effectful calls handled by standard MLIR func dialect
                vec![]
            }
        }
        // Control flow terminators don't produce dialect ops — they remain
        // as MLIR control flow (cf.br, cf.cond_br, cf.switch, func.return).
        Terminator::Goto { .. }
        | Terminator::SwitchInt { .. }
        | Terminator::Return
        | Terminator::Unreachable => vec![],
    }
}

/// Get the Redox MLIR type for a local variable.
fn type_for_local(local: Local, body: &MirBody) -> RedoxType {
    body.local_types
        .iter()
        .find(|lt| lt.local == local)
        .map(|lt| owned_type(&lt.type_name))
        .unwrap_or_else(|| owned_type("unknown"))
}

// ===========================================================================
// Convenience: collect all dialect ops from a translated body
// ===========================================================================

/// Flatten all ops from translated blocks into a single vec.
pub fn all_ops(blocks: &[TranslatedBlock]) -> Vec<RedoxOp> {
    blocks
        .iter()
        .flat_map(|b| b.ops.iter().chain(b.terminator_ops.iter()))
        .cloned()
        .collect()
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::{verify_ops, BorrowMode, RedoxOp};

    // -- Helper: build a minimal MIR body ------------------------------------

    fn local_types_simple() -> Vec<LocalType> {
        vec![
            LocalType { local: 0, type_name: "i32".into(), is_copy: true },
            LocalType { local: 1, type_name: "i32".into(), is_copy: true },
            LocalType { local: 2, type_name: "String".into(), is_copy: false },
            LocalType { local: 3, type_name: "Vec<u8>".into(), is_copy: false },
        ]
    }

    // -- Assignment: move ----------------------------------------------------

    #[test]
    fn translate_assign_move() {
        let body = MirBody {
            name: "test_move".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![Statement::Assign(
                    Place::local(0),
                    Rvalue::Use(Operand::Move(Place::local(2))),
                )],
                terminator: Terminator::Return,
            }],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].ops.len(), 1);

        match &blocks[0].ops[0] {
            RedoxOp::Move(m) => {
                assert_eq!(m.source_type, owned_type("String"));
                assert_eq!(m.result_type, owned_type("String"));
            }
            other => panic!("expected Move, got {:?}", other),
        }
    }

    // -- Assignment: copy ----------------------------------------------------

    #[test]
    fn translate_assign_copy() {
        let body = MirBody {
            name: "test_copy".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![Statement::Assign(
                    Place::local(0),
                    Rvalue::Use(Operand::Copy(Place::local(1))),
                )],
                terminator: Terminator::Return,
            }],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        assert_eq!(blocks[0].ops.len(), 1);
        assert_eq!(blocks[0].ops[0].op_name(), "redox.copy");
    }

    // -- Assignment: borrow (shared) -----------------------------------------

    #[test]
    fn translate_assign_shared_borrow() {
        let body = MirBody {
            name: "test_borrow".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![Statement::Assign(
                    Place::local(0),
                    Rvalue::Ref { mutable: false, place: Place::local(2) },
                )],
                terminator: Terminator::Return,
            }],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        assert_eq!(blocks[0].ops.len(), 1);

        match &blocks[0].ops[0] {
            RedoxOp::Borrow(b) => {
                assert_eq!(b.source_type, owned_type("String"));
                assert_eq!(b.mode, BorrowMode::Shared);
            }
            other => panic!("expected Borrow, got {:?}", other),
        }
    }

    // -- Assignment: mutable borrow ------------------------------------------

    #[test]
    fn translate_assign_mut_borrow() {
        let body = MirBody {
            name: "test_mut_borrow".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![Statement::Assign(
                    Place::local(0),
                    Rvalue::Ref { mutable: true, place: Place::local(3) },
                )],
                terminator: Terminator::Return,
            }],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);

        match &blocks[0].ops[0] {
            RedoxOp::Borrow(b) => {
                assert_eq!(b.mode, BorrowMode::Exclusive);
                assert_eq!(b.source_type, owned_type("Vec<u8>"));
            }
            other => panic!("expected Borrow, got {:?}", other),
        }
    }

    // -- Terminator: drop ----------------------------------------------------

    #[test]
    fn translate_drop_terminator() {
        let body = MirBody {
            name: "test_drop".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![],
                terminator: Terminator::Drop {
                    place: Place::local(3),
                    target: 1,
                },
            }],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        assert_eq!(blocks[0].terminator_ops.len(), 1);
        assert_eq!(blocks[0].terminator_ops[0].op_name(), "redox.drop");

        match &blocks[0].terminator_ops[0] {
            RedoxOp::Drop(d) => assert_eq!(d.value_type, owned_type("Vec<u8>")),
            other => panic!("expected Drop, got {:?}", other),
        }
    }

    // -- Terminator: effectful call ------------------------------------------

    #[test]
    fn translate_effectful_call() {
        let body = MirBody {
            name: "test_call".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![],
                terminator: Terminator::Call {
                    func: "read_file".into(),
                    args: vec![Operand::Move(Place::local(2))],
                    destination: Place::local(0),
                    target: Some(1),
                },
            }],
            local_types: local_types_simple(),
        };
        let mut ctx = TranslationContext::new();
        ctx.register_effectful("read_file", vec!["IO".into()]);

        let blocks = translate_body(&body, &ctx);
        assert_eq!(blocks[0].terminator_ops.len(), 1);
        assert_eq!(blocks[0].terminator_ops[0].op_name(), "redox.effect.perform");
    }

    // -- Terminator: non-effectful call → no dialect ops ---------------------

    #[test]
    fn translate_non_effectful_call() {
        let body = MirBody {
            name: "test_call".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![],
                terminator: Terminator::Call {
                    func: "add".into(),
                    args: vec![
                        Operand::Copy(Place::local(0)),
                        Operand::Copy(Place::local(1)),
                    ],
                    destination: Place::local(0),
                    target: Some(1),
                },
            }],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();

        let blocks = translate_body(&body, &ctx);
        assert!(blocks[0].terminator_ops.is_empty());
    }

    // -- Control flow terminators produce no dialect ops ----------------------

    #[test]
    fn translate_control_flow_no_ops() {
        let body = MirBody {
            name: "test_cf".into(),
            blocks: vec![
                BasicBlock {
                    label: "bb0".into(),
                    statements: vec![],
                    terminator: Terminator::Goto { target: 1 },
                },
                BasicBlock {
                    label: "bb1".into(),
                    statements: vec![],
                    terminator: Terminator::SwitchInt {
                        discr: Operand::Copy(Place::local(0)),
                        targets: vec![2, 3],
                        otherwise: 4,
                    },
                },
                BasicBlock {
                    label: "bb2".into(),
                    statements: vec![],
                    terminator: Terminator::Return,
                },
                BasicBlock {
                    label: "bb3".into(),
                    statements: vec![],
                    terminator: Terminator::Unreachable,
                },
            ],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);

        for block in &blocks {
            assert!(block.ops.is_empty());
            assert!(block.terminator_ops.is_empty());
        }
    }

    // -- Constants don't generate ownership ops ------------------------------

    #[test]
    fn translate_constant_no_ops() {
        let body = MirBody {
            name: "test_const".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![Statement::Assign(
                    Place::local(0),
                    Rvalue::Use(Operand::Constant("42".into())),
                )],
                terminator: Terminator::Return,
            }],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        assert!(blocks[0].ops.is_empty());
    }

    // -- Arithmetic doesn't generate ownership ops ---------------------------

    #[test]
    fn translate_binop_no_ownership_ops() {
        let body = MirBody {
            name: "test_binop".into(),
            blocks: vec![BasicBlock {
                label: "bb0".into(),
                statements: vec![Statement::Assign(
                    Place::local(0),
                    Rvalue::BinaryOp(
                        BinOp::Add,
                        Operand::Copy(Place::local(0)),
                        Operand::Copy(Place::local(1)),
                    ),
                )],
                terminator: Terminator::Return,
            }],
            local_types: local_types_simple(),
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        assert!(blocks[0].ops.is_empty());
    }

    // -- Integration: multi-block function -----------------------------------

    #[test]
    fn translate_multi_block_function() {
        // Simulates:
        //   fn example(s: String) -> String {
        //     let r = &s;        // bb0: borrow
        //     let t = s;         // bb0: move
        //     drop(r);           // bb1: drop (terminator)
        //     return t;          // bb2: return
        //   }
        let body = MirBody {
            name: "example".into(),
            blocks: vec![
                BasicBlock {
                    label: "bb0".into(),
                    statements: vec![
                        Statement::StorageLive(1),
                        Statement::Assign(
                            Place::local(1),
                            Rvalue::Ref { mutable: false, place: Place::local(2) },
                        ),
                        Statement::Assign(
                            Place::local(0),
                            Rvalue::Use(Operand::Move(Place::local(2))),
                        ),
                    ],
                    terminator: Terminator::Goto { target: 1 },
                },
                BasicBlock {
                    label: "bb1".into(),
                    statements: vec![
                        Statement::StorageDead(1),
                    ],
                    terminator: Terminator::Drop {
                        place: Place::local(3),
                        target: 2,
                    },
                },
                BasicBlock {
                    label: "bb2".into(),
                    statements: vec![],
                    terminator: Terminator::Return,
                },
            ],
            local_types: vec![
                LocalType { local: 0, type_name: "String".into(), is_copy: false },
                LocalType { local: 1, type_name: "String".into(), is_copy: false },
                LocalType { local: 2, type_name: "String".into(), is_copy: false },
                LocalType { local: 3, type_name: "Vec<u8>".into(), is_copy: false },
            ],
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        assert_eq!(blocks.len(), 3);

        // bb0: borrow + move
        assert_eq!(blocks[0].ops.len(), 2);
        assert_eq!(blocks[0].ops[0].op_name(), "redox.borrow");
        assert_eq!(blocks[0].ops[1].op_name(), "redox.move");

        // bb1: drop terminator
        assert_eq!(blocks[1].terminator_ops.len(), 1);
        assert_eq!(blocks[1].terminator_ops[0].op_name(), "redox.drop");

        // bb2: return → no dialect ops
        assert!(blocks[2].ops.is_empty());
        assert!(blocks[2].terminator_ops.is_empty());

        // All generated ops should verify
        let all = all_ops(&blocks);
        let errors = verify_ops(&all);
        assert!(errors.is_empty(), "verification errors: {errors:?}");
    }

    // -- all_ops flattening --------------------------------------------------

    #[test]
    fn all_ops_flattens_correctly() {
        let body = MirBody {
            name: "flatten_test".into(),
            blocks: vec![
                BasicBlock {
                    label: "bb0".into(),
                    statements: vec![
                        Statement::Assign(
                            Place::local(0),
                            Rvalue::Use(Operand::Move(Place::local(1))),
                        ),
                    ],
                    terminator: Terminator::Drop { place: Place::local(1), target: 1 },
                },
                BasicBlock {
                    label: "bb1".into(),
                    statements: vec![],
                    terminator: Terminator::Return,
                },
            ],
            local_types: vec![
                LocalType { local: 0, type_name: "i32".into(), is_copy: true },
                LocalType { local: 1, type_name: "i32".into(), is_copy: true },
            ],
        };
        let ctx = TranslationContext::new();
        let blocks = translate_body(&body, &ctx);
        let ops = all_ops(&blocks);
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].op_name(), "redox.move");
        assert_eq!(ops[1].op_name(), "redox.drop");
    }

    // -- Place display -------------------------------------------------------

    #[test]
    fn place_display() {
        assert_eq!(Place::local(0).to_string(), "_0");
        assert_eq!(Place::field(1, 3).to_string(), "_1.3");
        assert_eq!(Place::deref(2).to_string(), "_2.*");
        assert_eq!(
            Place {
                local: 0,
                projections: vec![
                    Projection::Deref,
                    Projection::Field(1),
                    Projection::Index(3),
                ],
            }
            .to_string(),
            "_0.*.1[_3]"
        );
    }

    // -- TranslationContext effects lookup ------------------------------------

    #[test]
    fn translation_context_effects() {
        let mut ctx = TranslationContext::new();
        assert!(ctx.effects_of("read_file").is_none());

        ctx.register_effectful("read_file", vec!["IO".into()]);
        ctx.register_effectful("spawn", vec!["Async".into(), "IO".into()]);

        assert_eq!(ctx.effects_of("read_file").unwrap(), &["IO"]);
        assert_eq!(
            ctx.effects_of("spawn").unwrap(),
            &["Async", "IO"]
        );
        assert!(ctx.effects_of("add").is_none());
    }
}
