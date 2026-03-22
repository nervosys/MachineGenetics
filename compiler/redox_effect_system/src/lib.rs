//! # Effect Type System
//!
//! Implements algebraic effects for the Redox language:
//!
//! - **Effect declarations**: named effect types with operation signatures
//! - **Effect inference**: propagate effects through the call graph
//! - **Effect polymorphism**: functions generic over effect sets
//! - **Effect handling**: handler blocks that intercept and resume effects
//!
//! Design follows Koka / Eff style:
//! ```text
//! effect State<S> {
//!     get() -> S,
//!     set(s: S) -> (),
//! }
//!
//! fn counter(): <State<i32>> i32 {
//!     let x = get();
//!     set(x + 1);
//!     x
//! }
//!
//! handle counter() {
//!     get() -> resume(42),
//!     set(s) -> resume(()),
//! }
//! ```
//!
//! (ROADMAP Step 68)

use std::collections::{HashMap, HashSet};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Effect Declarations
// ═══════════════════════════════════════════════════════════════════════════

/// A unique effect identifier.
pub type EffectId = String;

/// A type in the effect system (simplified).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffType {
    Unit,
    Bool,
    Int,
    Float,
    Str,
    Named(String),
    TypeVar(String),
    Tuple(Vec<EffType>),
    Function(Vec<EffType>, Box<EffType>, EffectSet),
}

impl fmt::Display for EffType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EffType::Unit => write!(f, "()"),
            EffType::Bool => write!(f, "bool"),
            EffType::Int => write!(f, "int"),
            EffType::Float => write!(f, "float"),
            EffType::Str => write!(f, "str"),
            EffType::Named(n) => write!(f, "{n}"),
            EffType::TypeVar(v) => write!(f, "'{v}"),
            EffType::Tuple(ts) => {
                write!(f, "(")?;
                for (i, t) in ts.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{t}")?;
                }
                write!(f, ")")
            }
            EffType::Function(params, ret, effs) => {
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {ret}")?;
                if !effs.is_empty() {
                    write!(f, " / {effs}")?;
                }
                Ok(())
            }
        }
    }
}

/// An operation declared within an effect.
#[derive(Debug, Clone)]
pub struct EffectOperation {
    pub name: String,
    pub params: Vec<(String, EffType)>,
    pub return_type: EffType,
}

/// An effect declaration (like an interface for effects).
#[derive(Debug, Clone)]
pub struct EffectDecl {
    pub name: EffectId,
    pub type_params: Vec<String>,
    pub operations: Vec<EffectOperation>,
}

impl EffectDecl {
    pub fn new(name: &str) -> Self {
        EffectDecl {
            name: name.to_string(),
            type_params: Vec::new(),
            operations: Vec::new(),
        }
    }

    pub fn with_type_param(mut self, param: &str) -> Self {
        self.type_params.push(param.to_string());
        self
    }

    pub fn with_op(mut self, name: &str, params: Vec<(String, EffType)>, ret: EffType) -> Self {
        self.operations.push(EffectOperation {
            name: name.to_string(),
            params,
            return_type: ret,
        });
        self
    }

    pub fn lookup_op(&self, name: &str) -> Option<&EffectOperation> {
        self.operations.iter().find(|op| op.name == name)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Effect Sets & Polymorphism
// ═══════════════════════════════════════════════════════════════════════════

/// A set of effects (row-polymorphic).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectSet {
    /// Concrete effects in this set.
    pub effects: HashSet<EffectId>,
    /// An optional effect type variable (for polymorphism).
    pub tail: Option<String>,
}

impl EffectSet {
    pub fn empty() -> Self {
        EffectSet { effects: HashSet::new(), tail: None }
    }

    pub fn single(eff: &str) -> Self {
        let mut effects = HashSet::new();
        effects.insert(eff.to_string());
        EffectSet { effects, tail: None }
    }

    pub fn from_effects(effs: &[&str]) -> Self {
        EffectSet {
            effects: effs.iter().map(|e| e.to_string()).collect(),
            tail: None,
        }
    }

    pub fn with_tail(mut self, var: &str) -> Self {
        self.tail = Some(var.to_string());
        self
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty() && self.tail.is_none()
    }

    pub fn is_pure(&self) -> bool {
        self.effects.is_empty() && self.tail.is_none()
    }

    pub fn contains(&self, eff: &str) -> bool {
        self.effects.contains(eff)
    }

    /// Union two effect sets.
    pub fn union(&self, other: &EffectSet) -> EffectSet {
        let mut effects = self.effects.clone();
        effects.extend(other.effects.iter().cloned());
        let tail = self.tail.clone().or_else(|| other.tail.clone());
        EffectSet { effects, tail }
    }

    /// Remove an effect (handled).
    pub fn without(&self, eff: &str) -> EffectSet {
        let mut effects = self.effects.clone();
        effects.remove(eff);
        EffectSet { effects, tail: self.tail.clone() }
    }

    /// Check if this set is a subset of another (ignoring tails).
    pub fn is_subset_of(&self, other: &EffectSet) -> bool {
        self.effects.is_subset(&other.effects)
    }
}

impl fmt::Display for EffectSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<")?;
        let mut effs: Vec<&str> = self.effects.iter().map(|e| e.as_str()).collect();
        effs.sort();
        for (i, e) in effs.iter().enumerate() {
            if i > 0 { write!(f, ", ")?; }
            write!(f, "{e}")?;
        }
        if let Some(ref t) = self.tail {
            if !effs.is_empty() { write!(f, ", ")?; }
            write!(f, "..{t}")?;
        }
        write!(f, ">")
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Effect Inference
// ═══════════════════════════════════════════════════════════════════════════

/// A function signature with effect annotation.
#[derive(Debug, Clone)]
pub struct FnSignature {
    pub name: String,
    pub params: Vec<(String, EffType)>,
    pub return_type: EffType,
    pub effects: EffectSet,
    /// Type parameters for effect polymorphism.
    pub effect_params: Vec<String>,
}

/// The effect inference engine.
pub struct EffectInferencer {
    /// Known effect declarations.
    pub effects: HashMap<EffectId, EffectDecl>,
    /// Known function signatures.
    pub signatures: HashMap<String, FnSignature>,
}

/// Expression for effect inference.
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(String),
    Var(String),
    Call(String, Vec<Expr>),
    EffectOp(EffectId, String, Vec<Expr>),
    Handle(Box<Expr>, Vec<Handler>),
    Block(Vec<Expr>),
    Lambda(Vec<String>, Box<Expr>),
}

/// A handler clause.
#[derive(Debug, Clone)]
pub struct Handler {
    pub effect: EffectId,
    pub operation: String,
    pub params: Vec<String>,
    pub body: Expr,
}

/// Result of effect inference.
#[derive(Debug, Clone)]
pub struct InferResult {
    pub inferred_type: EffType,
    pub inferred_effects: EffectSet,
}

impl EffectInferencer {
    pub fn new() -> Self {
        EffectInferencer {
            effects: HashMap::new(),
            signatures: HashMap::new(),
        }
    }

    pub fn declare_effect(&mut self, decl: EffectDecl) {
        self.effects.insert(decl.name.clone(), decl);
    }

    pub fn declare_fn(&mut self, sig: FnSignature) {
        self.signatures.insert(sig.name.clone(), sig);
    }

    /// Infer effects for an expression.
    pub fn infer(&self, expr: &Expr) -> InferResult {
        match expr {
            Expr::Literal(_) => InferResult {
                inferred_type: EffType::Int,
                inferred_effects: EffectSet::empty(),
            },
            Expr::Var(_) => InferResult {
                inferred_type: EffType::Int,
                inferred_effects: EffectSet::empty(),
            },
            Expr::Call(name, args) => {
                // Collect effects from arguments
                let mut effects = EffectSet::empty();
                for arg in args {
                    let r = self.infer(arg);
                    effects = effects.union(&r.inferred_effects);
                }
                // Add effects from the called function
                if let Some(sig) = self.signatures.get(name) {
                    effects = effects.union(&sig.effects);
                    InferResult {
                        inferred_type: sig.return_type.clone(),
                        inferred_effects: effects,
                    }
                } else {
                    InferResult {
                        inferred_type: EffType::Unit,
                        inferred_effects: effects,
                    }
                }
            }
            Expr::EffectOp(eff_id, op_name, args) => {
                let mut effects = EffectSet::single(eff_id);
                for arg in args {
                    let r = self.infer(arg);
                    effects = effects.union(&r.inferred_effects);
                }
                let return_type = self.effects.get(eff_id)
                    .and_then(|d| d.lookup_op(op_name))
                    .map(|op| op.return_type.clone())
                    .unwrap_or(EffType::Unit);
                InferResult {
                    inferred_type: return_type,
                    inferred_effects: effects,
                }
            }
            Expr::Handle(body, handlers) => {
                let body_result = self.infer(body);
                let mut remaining = body_result.inferred_effects.clone();
                for handler in handlers {
                    remaining = remaining.without(&handler.effect);
                }
                InferResult {
                    inferred_type: body_result.inferred_type,
                    inferred_effects: remaining,
                }
            }
            Expr::Block(exprs) => {
                let mut effects = EffectSet::empty();
                let mut last_type = EffType::Unit;
                for e in exprs {
                    let r = self.infer(e);
                    effects = effects.union(&r.inferred_effects);
                    last_type = r.inferred_type;
                }
                InferResult {
                    inferred_type: last_type,
                    inferred_effects: effects,
                }
            }
            Expr::Lambda(_params, body) => {
                let r = self.infer(body);
                InferResult {
                    inferred_type: EffType::Function(vec![], Box::new(r.inferred_type), r.inferred_effects.clone()),
                    inferred_effects: EffectSet::empty(), // lambda itself is pure
                }
            }
        }
    }

    /// Check if a function body's inferred effects match its declared effects.
    pub fn check_effects(&self, sig: &FnSignature, body: &Expr) -> Vec<EffectError> {
        let inferred = self.infer(body);
        let mut errors = Vec::new();

        // Check for undeclared effects
        for eff in &inferred.inferred_effects.effects {
            if !sig.effects.contains(eff) && sig.effect_params.is_empty() {
                errors.push(EffectError::UndeclaredEffect {
                    function: sig.name.clone(),
                    effect: eff.clone(),
                    declared: sig.effects.clone(),
                });
            }
        }

        errors
    }
}

/// Errors from effect checking.
#[derive(Debug, Clone)]
pub enum EffectError {
    UndeclaredEffect {
        function: String,
        effect: EffectId,
        declared: EffectSet,
    },
    UnhandledEffect {
        effect: EffectId,
        at: String,
    },
    UnknownEffect {
        effect: EffectId,
    },
    UnknownOperation {
        effect: EffectId,
        operation: String,
    },
}

impl fmt::Display for EffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EffectError::UndeclaredEffect { function, effect, declared } => {
                write!(f, "function `{function}` performs effect `{effect}` but only declares {declared}")
            }
            EffectError::UnhandledEffect { effect, at } => {
                write!(f, "unhandled effect `{effect}` at {at}")
            }
            EffectError::UnknownEffect { effect } => {
                write!(f, "unknown effect `{effect}`")
            }
            EffectError::UnknownOperation { effect, operation } => {
                write!(f, "unknown operation `{operation}` on effect `{effect}`")
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn state_effect() -> EffectDecl {
        EffectDecl::new("State")
            .with_type_param("S")
            .with_op("get", vec![], EffType::TypeVar("S".to_string()))
            .with_op("set", vec![("s".to_string(), EffType::TypeVar("S".to_string()))], EffType::Unit)
    }

    fn io_effect() -> EffectDecl {
        EffectDecl::new("IO")
            .with_op("print", vec![("msg".to_string(), EffType::Str)], EffType::Unit)
            .with_op("read", vec![], EffType::Str)
    }

    fn setup_inferencer() -> EffectInferencer {
        let mut inf = EffectInferencer::new();
        inf.declare_effect(state_effect());
        inf.declare_effect(io_effect());
        inf.declare_fn(FnSignature {
            name: "get_state".to_string(),
            params: vec![],
            return_type: EffType::Int,
            effects: EffectSet::single("State"),
            effect_params: vec![],
        });
        inf.declare_fn(FnSignature {
            name: "do_print".to_string(),
            params: vec![("msg".to_string(), EffType::Str)],
            return_type: EffType::Unit,
            effects: EffectSet::single("IO"),
            effect_params: vec![],
        });
        inf.declare_fn(FnSignature {
            name: "pure_add".to_string(),
            params: vec![("a".to_string(), EffType::Int), ("b".to_string(), EffType::Int)],
            return_type: EffType::Int,
            effects: EffectSet::empty(),
            effect_params: vec![],
        });
        inf
    }

    // ── EffType Display ──────────────────────────────────────────────────

    #[test]
    fn type_display_unit() {
        assert_eq!(EffType::Unit.to_string(), "()");
    }

    #[test]
    fn type_display_function() {
        let t = EffType::Function(
            vec![EffType::Int, EffType::Bool],
            Box::new(EffType::Str),
            EffectSet::single("IO"),
        );
        let s = t.to_string();
        assert!(s.contains("int"));
        assert!(s.contains("bool"));
        assert!(s.contains("str"));
        assert!(s.contains("IO"));
    }

    // ── EffectDecl ───────────────────────────────────────────────────────

    #[test]
    fn effect_decl_ops() {
        let state = state_effect();
        assert_eq!(state.operations.len(), 2);
        assert!(state.lookup_op("get").is_some());
        assert!(state.lookup_op("set").is_some());
        assert!(state.lookup_op("delete").is_none());
    }

    #[test]
    fn effect_decl_type_params() {
        let state = state_effect();
        assert_eq!(state.type_params, vec!["S"]);
    }

    // ── EffectSet ────────────────────────────────────────────────────────

    #[test]
    fn effect_set_empty() {
        let s = EffectSet::empty();
        assert!(s.is_empty());
        assert!(s.is_pure());
    }

    #[test]
    fn effect_set_single() {
        let s = EffectSet::single("IO");
        assert!(!s.is_pure());
        assert!(s.contains("IO"));
        assert!(!s.contains("State"));
    }

    #[test]
    fn effect_set_union() {
        let a = EffectSet::single("IO");
        let b = EffectSet::single("State");
        let c = a.union(&b);
        assert!(c.contains("IO"));
        assert!(c.contains("State"));
        assert_eq!(c.effects.len(), 2);
    }

    #[test]
    fn effect_set_without() {
        let s = EffectSet::from_effects(&["IO", "State"]);
        let r = s.without("IO");
        assert!(!r.contains("IO"));
        assert!(r.contains("State"));
    }

    #[test]
    fn effect_set_subset() {
        let a = EffectSet::single("IO");
        let b = EffectSet::from_effects(&["IO", "State"]);
        assert!(a.is_subset_of(&b));
        assert!(!b.is_subset_of(&a));
    }

    #[test]
    fn effect_set_display() {
        let s = EffectSet::from_effects(&["IO", "State"]);
        let d = s.to_string();
        assert!(d.starts_with("<"));
        assert!(d.ends_with(">"));
        assert!(d.contains("IO"));
        assert!(d.contains("State"));
    }

    #[test]
    fn effect_set_with_tail() {
        let s = EffectSet::single("IO").with_tail("e");
        let d = s.to_string();
        assert!(d.contains("..e"));
    }

    // ── Effect Inference ─────────────────────────────────────────────────

    #[test]
    fn infer_literal_pure() {
        let inf = setup_inferencer();
        let r = inf.infer(&Expr::Literal("42".to_string()));
        assert!(r.inferred_effects.is_pure());
    }

    #[test]
    fn infer_pure_call() {
        let inf = setup_inferencer();
        let r = inf.infer(&Expr::Call(
            "pure_add".to_string(),
            vec![Expr::Literal("1".to_string()), Expr::Literal("2".to_string())],
        ));
        assert!(r.inferred_effects.is_pure());
        assert_eq!(r.inferred_type, EffType::Int);
    }

    #[test]
    fn infer_effectful_call() {
        let inf = setup_inferencer();
        let r = inf.infer(&Expr::Call("get_state".to_string(), vec![]));
        assert!(r.inferred_effects.contains("State"));
        assert!(!r.inferred_effects.contains("IO"));
    }

    #[test]
    fn infer_effect_op() {
        let inf = setup_inferencer();
        let r = inf.infer(&Expr::EffectOp(
            "State".to_string(),
            "get".to_string(),
            vec![],
        ));
        assert!(r.inferred_effects.contains("State"));
    }

    #[test]
    fn infer_block_accumulates() {
        let inf = setup_inferencer();
        let block = Expr::Block(vec![
            Expr::Call("get_state".to_string(), vec![]),
            Expr::Call("do_print".to_string(), vec![Expr::Literal("hi".to_string())]),
        ]);
        let r = inf.infer(&block);
        assert!(r.inferred_effects.contains("State"));
        assert!(r.inferred_effects.contains("IO"));
    }

    #[test]
    fn infer_handle_removes_effect() {
        let inf = setup_inferencer();
        let body = Expr::EffectOp("State".to_string(), "get".to_string(), vec![]);
        let handled = Expr::Handle(
            Box::new(body),
            vec![Handler {
                effect: "State".to_string(),
                operation: "get".to_string(),
                params: vec![],
                body: Expr::Literal("42".to_string()),
            }],
        );
        let r = inf.infer(&handled);
        assert!(!r.inferred_effects.contains("State"));
    }

    #[test]
    fn infer_handle_keeps_unhandled() {
        let inf = setup_inferencer();
        let body = Expr::Block(vec![
            Expr::EffectOp("State".to_string(), "get".to_string(), vec![]),
            Expr::EffectOp("IO".to_string(), "print".to_string(), vec![Expr::Literal("hi".to_string())]),
        ]);
        let handled = Expr::Handle(
            Box::new(body),
            vec![Handler {
                effect: "State".to_string(),
                operation: "get".to_string(),
                params: vec![],
                body: Expr::Literal("42".to_string()),
            }],
        );
        let r = inf.infer(&handled);
        assert!(!r.inferred_effects.contains("State"));
        assert!(r.inferred_effects.contains("IO"));
    }

    #[test]
    fn infer_lambda_pure() {
        let inf = setup_inferencer();
        let lam = Expr::Lambda(
            vec!["x".to_string()],
            Box::new(Expr::Literal("1".to_string())),
        );
        let r = inf.infer(&lam);
        assert!(r.inferred_effects.is_pure());
    }

    // ── Effect Checking ──────────────────────────────────────────────────

    #[test]
    fn check_matching_effects() {
        let inf = setup_inferencer();
        let sig = FnSignature {
            name: "stateful".to_string(),
            params: vec![],
            return_type: EffType::Int,
            effects: EffectSet::single("State"),
            effect_params: vec![],
        };
        let body = Expr::Call("get_state".to_string(), vec![]);
        let errors = inf.check_effects(&sig, &body);
        assert!(errors.is_empty(), "declared effects match inferred");
    }

    #[test]
    fn check_undeclared_effect() {
        let inf = setup_inferencer();
        let sig = FnSignature {
            name: "supposedly_pure".to_string(),
            params: vec![],
            return_type: EffType::Int,
            effects: EffectSet::empty(),
            effect_params: vec![],
        };
        let body = Expr::Call("get_state".to_string(), vec![]);
        let errors = inf.check_effects(&sig, &body);
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            EffectError::UndeclaredEffect { effect, .. } => assert_eq!(effect, "State"),
            _ => panic!("expected UndeclaredEffect"),
        }
    }

    #[test]
    fn check_polymorphic_skips_check() {
        let inf = setup_inferencer();
        let sig = FnSignature {
            name: "polymorphic".to_string(),
            params: vec![],
            return_type: EffType::Int,
            effects: EffectSet::empty(),
            effect_params: vec!["e".to_string()],
        };
        let body = Expr::Call("get_state".to_string(), vec![]);
        let errors = inf.check_effects(&sig, &body);
        assert!(errors.is_empty(), "polymorphic functions skip concrete checks");
    }

    // ── Error Display ────────────────────────────────────────────────────

    #[test]
    fn error_display() {
        let e = EffectError::UndeclaredEffect {
            function: "foo".to_string(),
            effect: "State".to_string(),
            declared: EffectSet::empty(),
        };
        let s = e.to_string();
        assert!(s.contains("foo"));
        assert!(s.contains("State"));
    }

    #[test]
    fn error_unknown_op() {
        let e = EffectError::UnknownOperation {
            effect: "IO".to_string(),
            operation: "delete".to_string(),
        };
        assert!(e.to_string().contains("delete"));
    }
}
