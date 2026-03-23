//! Contract syntax and checking for the Redox compiler.
//!
//! Provides preconditions, postconditions, and invariants with both
//! compile-time (static analysis) and runtime (dynamic assertion) verification modes.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Contract expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ContractExpr {
    /// Boolean literal
    BoolLit(bool),
    /// Integer literal
    IntLit(i64),
    /// Float literal
    FloatLit(f64),
    /// String literal
    StrLit(String),
    /// Variable reference (parameter name, `result`, `old(x)`, etc.)
    Var(String),
    /// Old value of a variable (for postconditions)
    Old(Box<ContractExpr>),
    /// Result of the function (for postconditions)
    Result,
    /// Field access: expr.field
    Field(Box<ContractExpr>, String),
    /// Index access: expr[index]
    Index(Box<ContractExpr>, Box<ContractExpr>),
    /// Binary operation
    BinOp(Box<ContractExpr>, BinOp, Box<ContractExpr>),
    /// Unary operation
    UnOp(UnOp, Box<ContractExpr>),
    /// Function call within contract: f(args...)
    Call(String, Vec<ContractExpr>),
    /// Universal quantifier: forall x in collection => predicate
    ForAll(String, Box<ContractExpr>, Box<ContractExpr>),
    /// Existential quantifier: exists x in collection => predicate
    Exists(String, Box<ContractExpr>, Box<ContractExpr>),
    /// Implication: lhs ==> rhs
    Implies(Box<ContractExpr>, Box<ContractExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::And => "&&",
            BinOp::Or => "||",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Not,
    Neg,
}

impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            UnOp::Not => "!",
            UnOp::Neg => "-",
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for ContractExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractExpr::BoolLit(b) => write!(f, "{b}"),
            ContractExpr::IntLit(n) => write!(f, "{n}"),
            ContractExpr::FloatLit(v) => write!(f, "{v}"),
            ContractExpr::StrLit(s) => write!(f, "\"{s}\""),
            ContractExpr::Var(name) => write!(f, "{name}"),
            ContractExpr::Old(inner) => write!(f, "old({inner})"),
            ContractExpr::Result => write!(f, "result"),
            ContractExpr::Field(expr, field) => write!(f, "{expr}.{field}"),
            ContractExpr::Index(expr, idx) => write!(f, "{expr}[{idx}]"),
            ContractExpr::BinOp(l, op, r) => write!(f, "({l} {op} {r})"),
            ContractExpr::UnOp(op, e) => write!(f, "{op}{e}"),
            ContractExpr::Call(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(f, "{name}({})", args_str.join(", "))
            }
            ContractExpr::ForAll(var, coll, pred) => {
                write!(f, "forall {var} in {coll} => {pred}")
            }
            ContractExpr::Exists(var, coll, pred) => {
                write!(f, "exists {var} in {coll} => {pred}")
            }
            ContractExpr::Implies(lhs, rhs) => write!(f, "({lhs} ==> {rhs})"),
        }
    }
}

// ---------------------------------------------------------------------------
// Contract kinds
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Precondition {
    pub label: Option<String>,
    pub expr: ContractExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Postcondition {
    pub label: Option<String>,
    pub expr: ContractExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Invariant {
    pub label: Option<String>,
    pub expr: ContractExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContractKind {
    Pre(Precondition),
    Post(Postcondition),
    Inv(Invariant),
}

impl ContractKind {
    pub fn label(&self) -> Option<&str> {
        match self {
            ContractKind::Pre(p) => p.label.as_deref(),
            ContractKind::Post(p) => p.label.as_deref(),
            ContractKind::Inv(i) => i.label.as_deref(),
        }
    }

    pub fn expr(&self) -> &ContractExpr {
        match self {
            ContractKind::Pre(p) => &p.expr,
            ContractKind::Post(p) => &p.expr,
            ContractKind::Inv(i) => &i.expr,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            ContractKind::Pre(_) => "precondition",
            ContractKind::Post(_) => "postcondition",
            ContractKind::Inv(_) => "invariant",
        }
    }
}

impl fmt::Display for ContractKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label_str = match self.label() {
            Some(l) => format!(" \"{l}\""),
            None => String::new(),
        };
        write!(f, "{}{}: {}", self.kind_name(), label_str, self.expr())
    }
}

// ---------------------------------------------------------------------------
// Function contract
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionContract {
    pub fn_name: String,
    pub params: Vec<(String, String)>, // (name, type_name)
    pub return_type: Option<String>,
    pub preconditions: Vec<Precondition>,
    pub postconditions: Vec<Postcondition>,
}

impl FunctionContract {
    pub fn new(name: String) -> Self {
        Self {
            fn_name: name,
            params: Vec::new(),
            return_type: None,
            preconditions: Vec::new(),
            postconditions: Vec::new(),
        }
    }

    pub fn param(mut self, name: String, ty: String) -> Self {
        self.params.push((name, ty));
        self
    }

    pub fn returns(mut self, ty: String) -> Self {
        self.return_type = Some(ty);
        self
    }

    pub fn pre(mut self, expr: ContractExpr) -> Self {
        self.preconditions.push(Precondition { label: None, expr });
        self
    }

    pub fn pre_labeled(mut self, label: String, expr: ContractExpr) -> Self {
        self.preconditions.push(Precondition { label: Some(label), expr });
        self
    }

    pub fn post(mut self, expr: ContractExpr) -> Self {
        self.postconditions.push(Postcondition { label: None, expr });
        self
    }

    pub fn post_labeled(mut self, label: String, expr: ContractExpr) -> Self {
        self.postconditions.push(Postcondition { label: Some(label), expr });
        self
    }

    pub fn all_contracts(&self) -> Vec<ContractKind> {
        let mut out = Vec::new();
        for p in &self.preconditions {
            out.push(ContractKind::Pre(p.clone()));
        }
        for p in &self.postconditions {
            out.push(ContractKind::Post(p.clone()));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Type invariant
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct TypeInvariant {
    pub type_name: String,
    pub fields: Vec<(String, String)>, // (name, type_name)
    pub invariants: Vec<Invariant>,
}

impl TypeInvariant {
    pub fn new(name: String) -> Self {
        Self { type_name: name, fields: Vec::new(), invariants: Vec::new() }
    }

    pub fn field(mut self, name: String, ty: String) -> Self {
        self.fields.push((name, ty));
        self
    }

    pub fn invariant(mut self, expr: ContractExpr) -> Self {
        self.invariants.push(Invariant { label: None, expr });
        self
    }

    pub fn invariant_labeled(mut self, label: String, expr: ContractExpr) -> Self {
        self.invariants.push(Invariant { label: Some(label), expr });
        self
    }
}

// ---------------------------------------------------------------------------
// Verification mode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationMode {
    /// Static analysis at compile time
    CompileTime,
    /// Dynamic assertion injection at runtime
    Runtime,
    /// Both compile-time and runtime
    Both,
}

// ---------------------------------------------------------------------------
// Static analysis — known value environment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum StaticValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Unknown,
}

impl StaticValue {
    pub fn is_known(&self) -> bool {
        !matches!(self, StaticValue::Unknown)
    }
}

/// Environment mapping variable names to optional static values.
pub type StaticEnv = HashMap<String, StaticValue>;

/// Attempt to statically evaluate a contract expression.
pub fn static_eval(expr: &ContractExpr, env: &StaticEnv) -> StaticValue {
    match expr {
        ContractExpr::BoolLit(b) => StaticValue::Bool(*b),
        ContractExpr::IntLit(n) => StaticValue::Int(*n),
        ContractExpr::FloatLit(v) => StaticValue::Float(*v),
        ContractExpr::StrLit(s) => StaticValue::Str(s.clone()),
        ContractExpr::Var(name) => env.get(name).cloned().unwrap_or(StaticValue::Unknown),
        ContractExpr::Result => StaticValue::Unknown,
        ContractExpr::Old(_) => StaticValue::Unknown,
        ContractExpr::BinOp(l, op, r) => {
            let lv = static_eval(l, env);
            let rv = static_eval(r, env);
            eval_binop(&lv, *op, &rv)
        }
        ContractExpr::UnOp(op, inner) => {
            let v = static_eval(inner, env);
            eval_unop(*op, &v)
        }
        ContractExpr::Implies(lhs, rhs) => {
            let lv = static_eval(lhs, env);
            let rv = static_eval(rhs, env);
            match (&lv, &rv) {
                (StaticValue::Bool(false), _) => StaticValue::Bool(true),
                (StaticValue::Bool(true), StaticValue::Bool(b)) => StaticValue::Bool(*b),
                _ => StaticValue::Unknown,
            }
        }
        _ => StaticValue::Unknown,
    }
}

fn eval_binop(lv: &StaticValue, op: BinOp, rv: &StaticValue) -> StaticValue {
    match (lv, op, rv) {
        // Integer arithmetic
        (StaticValue::Int(a), BinOp::Add, StaticValue::Int(b)) => StaticValue::Int(a + b),
        (StaticValue::Int(a), BinOp::Sub, StaticValue::Int(b)) => StaticValue::Int(a - b),
        (StaticValue::Int(a), BinOp::Mul, StaticValue::Int(b)) => StaticValue::Int(a * b),
        (StaticValue::Int(a), BinOp::Div, StaticValue::Int(b)) if *b != 0 => {
            StaticValue::Int(a / b)
        }
        (StaticValue::Int(a), BinOp::Mod, StaticValue::Int(b)) if *b != 0 => {
            StaticValue::Int(a % b)
        }
        // Integer comparison
        (StaticValue::Int(a), BinOp::Eq, StaticValue::Int(b)) => StaticValue::Bool(a == b),
        (StaticValue::Int(a), BinOp::Ne, StaticValue::Int(b)) => StaticValue::Bool(a != b),
        (StaticValue::Int(a), BinOp::Lt, StaticValue::Int(b)) => StaticValue::Bool(a < b),
        (StaticValue::Int(a), BinOp::Le, StaticValue::Int(b)) => StaticValue::Bool(a <= b),
        (StaticValue::Int(a), BinOp::Gt, StaticValue::Int(b)) => StaticValue::Bool(a > b),
        (StaticValue::Int(a), BinOp::Ge, StaticValue::Int(b)) => StaticValue::Bool(a >= b),
        // Boolean logic
        (StaticValue::Bool(a), BinOp::And, StaticValue::Bool(b)) => StaticValue::Bool(*a && *b),
        (StaticValue::Bool(a), BinOp::Or, StaticValue::Bool(b)) => StaticValue::Bool(*a || *b),
        (StaticValue::Bool(a), BinOp::Eq, StaticValue::Bool(b)) => StaticValue::Bool(a == b),
        (StaticValue::Bool(a), BinOp::Ne, StaticValue::Bool(b)) => StaticValue::Bool(a != b),
        _ => StaticValue::Unknown,
    }
}

fn eval_unop(op: UnOp, v: &StaticValue) -> StaticValue {
    match (op, v) {
        (UnOp::Not, StaticValue::Bool(b)) => StaticValue::Bool(!b),
        (UnOp::Neg, StaticValue::Int(n)) => StaticValue::Int(-n),
        (UnOp::Neg, StaticValue::Float(f)) => StaticValue::Float(-f),
        _ => StaticValue::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Contract errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ContractError {
    /// Precondition references a parameter that doesn't exist
    UnknownParam { contract_label: Option<String>, param: String, fn_name: String },
    /// Postcondition references `result` but function has no return type
    ResultWithoutReturn { contract_label: Option<String>, fn_name: String },
    /// Postcondition uses `old(x)` where x is not a parameter
    OldNonParam { contract_label: Option<String>, var: String, fn_name: String },
    /// Invariant references a field not in the type
    UnknownField { contract_label: Option<String>, field: String, type_name: String },
    /// Static evaluation determined the contract is always false
    AlwaysFalse { kind: String, label: Option<String>, location: String },
    /// Static evaluation determined the contract is trivially true (warning)
    AlwaysTrue { kind: String, label: Option<String>, location: String },
    /// Contract expression references a variable not in scope
    UndefinedVar { var: String, location: String },
}

impl fmt::Display for ContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContractError::UnknownParam { param, fn_name, .. } => {
                write!(f, "precondition references unknown parameter '{param}' in fn {fn_name}")
            }
            ContractError::ResultWithoutReturn { fn_name, .. } => {
                write!(f, "postcondition references 'result' but fn {fn_name} has no return type")
            }
            ContractError::OldNonParam { var, fn_name, .. } => {
                write!(f, "old({var}) references non-parameter in fn {fn_name}")
            }
            ContractError::UnknownField { field, type_name, .. } => {
                write!(f, "invariant references unknown field '{field}' in type {type_name}")
            }
            ContractError::AlwaysFalse { kind, location, .. } => {
                write!(f, "{kind} is always false in {location}")
            }
            ContractError::AlwaysTrue { kind, location, .. } => {
                write!(f, "{kind} is always true in {location} (trivial)")
            }
            ContractError::UndefinedVar { var, location } => {
                write!(f, "undefined variable '{var}' in contract at {location}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Free variable extraction
// ---------------------------------------------------------------------------

fn free_vars(expr: &ContractExpr, vars: &mut Vec<String>) {
    match expr {
        ContractExpr::Var(name) => vars.push(name.clone()),
        ContractExpr::Old(inner) => free_vars(inner, vars),
        ContractExpr::Result => {}
        ContractExpr::BoolLit(_)
        | ContractExpr::IntLit(_)
        | ContractExpr::FloatLit(_)
        | ContractExpr::StrLit(_) => {}
        ContractExpr::Field(base, _) => free_vars(base, vars),
        ContractExpr::Index(base, idx) => {
            free_vars(base, vars);
            free_vars(idx, vars);
        }
        ContractExpr::BinOp(l, _, r) => {
            free_vars(l, vars);
            free_vars(r, vars);
        }
        ContractExpr::UnOp(_, inner) => free_vars(inner, vars),
        ContractExpr::Call(_, args) => {
            for a in args {
                free_vars(a, vars);
            }
        }
        ContractExpr::ForAll(bound, coll, pred) | ContractExpr::Exists(bound, coll, pred) => {
            free_vars(coll, vars);
            // bound variable is not free in predicate
            let mut pred_vars = Vec::new();
            free_vars(pred, &mut pred_vars);
            for v in pred_vars {
                if v != *bound {
                    vars.push(v);
                }
            }
        }
        ContractExpr::Implies(l, r) => {
            free_vars(l, vars);
            free_vars(r, vars);
        }
    }
}

fn old_vars(expr: &ContractExpr, vars: &mut Vec<String>) {
    match expr {
        ContractExpr::Old(inner) => {
            if let ContractExpr::Var(name) = inner.as_ref() {
                vars.push(name.clone());
            }
        }
        ContractExpr::BinOp(l, _, r) | ContractExpr::Implies(l, r) => {
            old_vars(l, vars);
            old_vars(r, vars);
        }
        ContractExpr::UnOp(_, inner) | ContractExpr::Field(inner, _) => old_vars(inner, vars),
        ContractExpr::Index(base, idx) => {
            old_vars(base, vars);
            old_vars(idx, vars);
        }
        ContractExpr::Call(_, args) => {
            for a in args {
                old_vars(a, vars);
            }
        }
        ContractExpr::ForAll(_, coll, pred) | ContractExpr::Exists(_, coll, pred) => {
            old_vars(coll, vars);
            old_vars(pred, vars);
        }
        _ => {}
    }
}

fn has_result(expr: &ContractExpr) -> bool {
    match expr {
        ContractExpr::Result => true,
        ContractExpr::BinOp(l, _, r) | ContractExpr::Implies(l, r) => {
            has_result(l) || has_result(r)
        }
        ContractExpr::UnOp(_, inner) | ContractExpr::Field(inner, _) | ContractExpr::Old(inner) => {
            has_result(inner)
        }
        ContractExpr::Index(base, idx) => has_result(base) || has_result(idx),
        ContractExpr::Call(_, args) => args.iter().any(|a| has_result(a)),
        ContractExpr::ForAll(_, coll, pred) | ContractExpr::Exists(_, coll, pred) => {
            has_result(coll) || has_result(pred)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Contract checker
// ---------------------------------------------------------------------------

pub struct ContractChecker {
    mode: VerificationMode,
    errors: Vec<ContractError>,
    warnings: Vec<ContractError>,
}

impl ContractChecker {
    pub fn new(mode: VerificationMode) -> Self {
        Self { mode, errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn errors(&self) -> &[ContractError] {
        &self.errors
    }

    pub fn warnings(&self) -> &[ContractError] {
        &self.warnings
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Check a function contract for well-formedness.
    pub fn check_function(&mut self, contract: &FunctionContract) {
        let param_names: Vec<&str> = contract.params.iter().map(|(n, _)| n.as_str()).collect();

        // Check preconditions
        for pre in &contract.preconditions {
            let mut vars = Vec::new();
            free_vars(&pre.expr, &mut vars);
            for v in &vars {
                if !param_names.contains(&v.as_str()) {
                    self.errors.push(ContractError::UnknownParam {
                        contract_label: pre.label.clone(),
                        param: v.clone(),
                        fn_name: contract.fn_name.clone(),
                    });
                }
            }
            // Preconditions should not reference `result`
            if has_result(&pre.expr) {
                self.errors.push(ContractError::ResultWithoutReturn {
                    contract_label: pre.label.clone(),
                    fn_name: contract.fn_name.clone(),
                });
            }

            self.static_check_expr(
                &pre.expr,
                "precondition",
                pre.label.clone(),
                &contract.fn_name,
                &param_names,
            );
        }

        // Check postconditions
        for post in &contract.postconditions {
            if has_result(&post.expr) && contract.return_type.is_none() {
                self.errors.push(ContractError::ResultWithoutReturn {
                    contract_label: post.label.clone(),
                    fn_name: contract.fn_name.clone(),
                });
            }

            let mut olds = Vec::new();
            old_vars(&post.expr, &mut olds);
            for v in &olds {
                if !param_names.contains(&v.as_str()) {
                    self.errors.push(ContractError::OldNonParam {
                        contract_label: post.label.clone(),
                        var: v.clone(),
                        fn_name: contract.fn_name.clone(),
                    });
                }
            }

            self.static_check_expr(
                &post.expr,
                "postcondition",
                post.label.clone(),
                &contract.fn_name,
                &param_names,
            );
        }
    }

    /// Check a type invariant for well-formedness.
    pub fn check_type_invariant(&mut self, ti: &TypeInvariant) {
        let field_names: Vec<&str> = ti.fields.iter().map(|(n, _)| n.as_str()).collect();

        for inv in &ti.invariants {
            let mut vars = Vec::new();
            free_vars(&inv.expr, &mut vars);
            for v in &vars {
                if v != "self" && !field_names.contains(&v.as_str()) {
                    self.errors.push(ContractError::UnknownField {
                        contract_label: inv.label.clone(),
                        field: v.clone(),
                        type_name: ti.type_name.clone(),
                    });
                }
            }

            // Static check with field values unknown
            if self.mode == VerificationMode::CompileTime || self.mode == VerificationMode::Both {
                let env: StaticEnv = HashMap::new();
                let val = static_eval(&inv.expr, &env);
                match val {
                    StaticValue::Bool(false) => {
                        self.errors.push(ContractError::AlwaysFalse {
                            kind: "invariant".to_string(),
                            label: inv.label.clone(),
                            location: ti.type_name.clone(),
                        });
                    }
                    StaticValue::Bool(true) => {
                        self.warnings.push(ContractError::AlwaysTrue {
                            kind: "invariant".to_string(),
                            label: inv.label.clone(),
                            location: ti.type_name.clone(),
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    fn static_check_expr(
        &mut self,
        expr: &ContractExpr,
        kind: &str,
        label: Option<String>,
        location: &str,
        _scope_vars: &[&str],
    ) {
        if self.mode == VerificationMode::Runtime {
            return;
        }
        let env: StaticEnv = HashMap::new();
        let val = static_eval(expr, &env);
        match val {
            StaticValue::Bool(false) => {
                self.errors.push(ContractError::AlwaysFalse {
                    kind: kind.to_string(),
                    label,
                    location: location.to_string(),
                });
            }
            StaticValue::Bool(true) => {
                self.warnings.push(ContractError::AlwaysTrue {
                    kind: kind.to_string(),
                    label,
                    location: location.to_string(),
                });
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime assertion generation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeAssertion {
    pub kind: String,
    pub label: Option<String>,
    pub assertion_code: String,
    pub message: String,
}

/// Generate runtime assertions for a function contract.
pub fn generate_runtime_assertions(contract: &FunctionContract) -> Vec<RuntimeAssertion> {
    let mut assertions = Vec::new();

    for pre in &contract.preconditions {
        let code = expr_to_code(&pre.expr);
        let msg = match &pre.label {
            Some(l) => format!("precondition '{}' violated in fn {}", l, contract.fn_name),
            None => format!("precondition violated in fn {}", contract.fn_name),
        };
        assertions.push(RuntimeAssertion {
            kind: "precondition".to_string(),
            label: pre.label.clone(),
            assertion_code: format!("assert!({code}, \"{msg}\");"),
            message: msg,
        });
    }

    for post in &contract.postconditions {
        let code = expr_to_code(&post.expr);
        let msg = match &post.label {
            Some(l) => format!("postcondition '{}' violated in fn {}", l, contract.fn_name),
            None => format!("postcondition violated in fn {}", contract.fn_name),
        };
        assertions.push(RuntimeAssertion {
            kind: "postcondition".to_string(),
            label: post.label.clone(),
            assertion_code: format!("assert!({code}, \"{msg}\");"),
            message: msg,
        });
    }

    assertions
}

/// Generate runtime assertions for a type invariant.
pub fn generate_invariant_assertions(ti: &TypeInvariant) -> Vec<RuntimeAssertion> {
    let mut assertions = Vec::new();

    for inv in &ti.invariants {
        let code = expr_to_code(&inv.expr);
        let msg = match &inv.label {
            Some(l) => format!("invariant '{}' violated in type {}", l, ti.type_name),
            None => format!("invariant violated in type {}", ti.type_name),
        };
        assertions.push(RuntimeAssertion {
            kind: "invariant".to_string(),
            label: inv.label.clone(),
            assertion_code: format!("assert!({code}, \"{msg}\");"),
            message: msg,
        });
    }

    assertions
}

fn expr_to_code(expr: &ContractExpr) -> String {
    match expr {
        ContractExpr::BoolLit(b) => format!("{b}"),
        ContractExpr::IntLit(n) => format!("{n}"),
        ContractExpr::FloatLit(v) => format!("{v:.1}"),
        ContractExpr::StrLit(s) => format!("\"{s}\""),
        ContractExpr::Var(name) => name.clone(),
        ContractExpr::Old(inner) => format!("__old_{}", expr_to_code(inner)),
        ContractExpr::Result => "__result".to_string(),
        ContractExpr::Field(base, field) => format!("{}.{field}", expr_to_code(base)),
        ContractExpr::Index(base, idx) => {
            format!("{}[{}]", expr_to_code(base), expr_to_code(idx))
        }
        ContractExpr::BinOp(l, op, r) => {
            format!("({} {} {})", expr_to_code(l), op, expr_to_code(r))
        }
        ContractExpr::UnOp(op, e) => format!("{op}{}", expr_to_code(e)),
        ContractExpr::Call(name, args) => {
            let args_str: Vec<String> = args.iter().map(|a| expr_to_code(a)).collect();
            format!("{name}({})", args_str.join(", "))
        }
        ContractExpr::ForAll(var, coll, pred) => {
            format!("{}.iter().all(|{var}| {})", expr_to_code(coll), expr_to_code(pred))
        }
        ContractExpr::Exists(var, coll, pred) => {
            format!("{}.iter().any(|{var}| {})", expr_to_code(coll), expr_to_code(pred))
        }
        ContractExpr::Implies(l, r) => {
            format!("(!{} || {})", expr_to_code(l), expr_to_code(r))
        }
    }
}

// ---------------------------------------------------------------------------
// Full verification pipeline
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct VerificationReport {
    pub mode: VerificationMode,
    pub errors: Vec<ContractError>,
    pub warnings: Vec<ContractError>,
    pub runtime_assertions: Vec<RuntimeAssertion>,
}

impl VerificationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn summary(&self) -> String {
        format!(
            "Verification (mode={:?}): {} errors, {} warnings, {} runtime assertions",
            self.mode,
            self.errors.len(),
            self.warnings.len(),
            self.runtime_assertions.len(),
        )
    }
}

/// Run full contract verification for a function contract.
pub fn verify_function(contract: &FunctionContract, mode: VerificationMode) -> VerificationReport {
    let mut checker = ContractChecker::new(mode);
    checker.check_function(contract);

    let runtime_assertions = if mode == VerificationMode::Runtime || mode == VerificationMode::Both
    {
        generate_runtime_assertions(contract)
    } else {
        Vec::new()
    };

    VerificationReport {
        mode,
        errors: checker.errors().to_vec(),
        warnings: checker.warnings().to_vec(),
        runtime_assertions,
    }
}

/// Run full contract verification for a type invariant.
pub fn verify_type_invariant(ti: &TypeInvariant, mode: VerificationMode) -> VerificationReport {
    let mut checker = ContractChecker::new(mode);
    checker.check_type_invariant(ti);

    let runtime_assertions = if mode == VerificationMode::Runtime || mode == VerificationMode::Both
    {
        generate_invariant_assertions(ti)
    } else {
        Vec::new()
    };

    VerificationReport {
        mode,
        errors: checker.errors().to_vec(),
        warnings: checker.warnings().to_vec(),
        runtime_assertions,
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Expression construction helpers --

    fn var(name: &str) -> ContractExpr {
        ContractExpr::Var(name.to_string())
    }

    fn int_lit(n: i64) -> ContractExpr {
        ContractExpr::IntLit(n)
    }

    fn bool_lit(b: bool) -> ContractExpr {
        ContractExpr::BoolLit(b)
    }

    fn gt(l: ContractExpr, r: ContractExpr) -> ContractExpr {
        ContractExpr::BinOp(Box::new(l), BinOp::Gt, Box::new(r))
    }

    fn lt(l: ContractExpr, r: ContractExpr) -> ContractExpr {
        ContractExpr::BinOp(Box::new(l), BinOp::Lt, Box::new(r))
    }

    fn eq(l: ContractExpr, r: ContractExpr) -> ContractExpr {
        ContractExpr::BinOp(Box::new(l), BinOp::Eq, Box::new(r))
    }

    fn and(l: ContractExpr, r: ContractExpr) -> ContractExpr {
        ContractExpr::BinOp(Box::new(l), BinOp::And, Box::new(r))
    }

    fn add(l: ContractExpr, r: ContractExpr) -> ContractExpr {
        ContractExpr::BinOp(Box::new(l), BinOp::Add, Box::new(r))
    }

    fn not(e: ContractExpr) -> ContractExpr {
        ContractExpr::UnOp(UnOp::Not, Box::new(e))
    }

    fn implies(l: ContractExpr, r: ContractExpr) -> ContractExpr {
        ContractExpr::Implies(Box::new(l), Box::new(r))
    }

    // -- Expression tests --

    #[test]
    fn test_expr_display() {
        let e = gt(var("x"), int_lit(0));
        assert_eq!(e.to_string(), "(x > 0)");
    }

    #[test]
    fn test_nested_expr_display() {
        let e = and(gt(var("x"), int_lit(0)), lt(var("x"), int_lit(100)));
        assert_eq!(e.to_string(), "((x > 0) && (x < 100))");
    }

    #[test]
    fn test_forall_display() {
        let e = ContractExpr::ForAll(
            "i".to_string(),
            Box::new(var("arr")),
            Box::new(gt(var("i"), int_lit(0))),
        );
        assert_eq!(e.to_string(), "forall i in arr => (i > 0)");
    }

    #[test]
    fn test_old_display() {
        let e = ContractExpr::Old(Box::new(var("x")));
        assert_eq!(e.to_string(), "old(x)");
    }

    #[test]
    fn test_implies_display() {
        let e = implies(gt(var("x"), int_lit(0)), lt(var("y"), int_lit(10)));
        assert_eq!(e.to_string(), "((x > 0) ==> (y < 10))");
    }

    // -- Static evaluation tests --

    #[test]
    fn test_static_eval_literal() {
        let env = StaticEnv::new();
        assert_eq!(static_eval(&int_lit(42), &env), StaticValue::Int(42));
        assert_eq!(static_eval(&bool_lit(true), &env), StaticValue::Bool(true));
    }

    #[test]
    fn test_static_eval_arithmetic() {
        let env = StaticEnv::new();
        let e = add(int_lit(3), int_lit(4));
        assert_eq!(static_eval(&e, &env), StaticValue::Int(7));
    }

    #[test]
    fn test_static_eval_comparison() {
        let env = StaticEnv::new();
        let e = gt(int_lit(5), int_lit(3));
        assert_eq!(static_eval(&e, &env), StaticValue::Bool(true));
    }

    #[test]
    fn test_static_eval_with_env() {
        let mut env = StaticEnv::new();
        env.insert("x".to_string(), StaticValue::Int(10));
        let e = gt(var("x"), int_lit(5));
        assert_eq!(static_eval(&e, &env), StaticValue::Bool(true));
    }

    #[test]
    fn test_static_eval_unknown_var() {
        let env = StaticEnv::new();
        let e = gt(var("x"), int_lit(0));
        assert_eq!(static_eval(&e, &env), StaticValue::Unknown);
    }

    #[test]
    fn test_static_eval_implies_false_antecedent() {
        let env = StaticEnv::new();
        let e = implies(bool_lit(false), bool_lit(false));
        assert_eq!(static_eval(&e, &env), StaticValue::Bool(true));
    }

    #[test]
    fn test_static_eval_not() {
        let env = StaticEnv::new();
        let e = not(bool_lit(true));
        assert_eq!(static_eval(&e, &env), StaticValue::Bool(false));
    }

    // -- Function contract tests --

    #[test]
    fn test_function_contract_valid() {
        let contract = FunctionContract::new("sqrt".to_string())
            .param("x".to_string(), "f64".to_string())
            .returns("f64".to_string())
            .pre(ContractExpr::BinOp(
                Box::new(var("x")),
                BinOp::Ge,
                Box::new(ContractExpr::FloatLit(0.0)),
            ))
            .post(ContractExpr::BinOp(
                Box::new(ContractExpr::Result),
                BinOp::Ge,
                Box::new(ContractExpr::FloatLit(0.0)),
            ));

        let report = verify_function(&contract, VerificationMode::CompileTime);
        assert!(report.is_ok());
    }

    #[test]
    fn test_function_contract_unknown_param() {
        let contract = FunctionContract::new("foo".to_string())
            .param("x".to_string(), "i32".to_string())
            .pre(gt(var("y"), int_lit(0)));

        let report = verify_function(&contract, VerificationMode::CompileTime);
        assert!(!report.is_ok());
        assert!(matches!(
            &report.errors[0],
            ContractError::UnknownParam { param, .. } if param == "y"
        ));
    }

    #[test]
    fn test_function_contract_result_without_return() {
        let contract = FunctionContract::new("bar".to_string())
            .param("x".to_string(), "i32".to_string())
            .post(gt(ContractExpr::Result, int_lit(0)));

        let report = verify_function(&contract, VerificationMode::CompileTime);
        assert!(!report.is_ok());
        assert!(matches!(&report.errors[0], ContractError::ResultWithoutReturn { .. }));
    }

    #[test]
    fn test_function_contract_old_non_param() {
        let contract = FunctionContract::new("inc".to_string())
            .param("x".to_string(), "i32".to_string())
            .returns("i32".to_string())
            .post(eq(ContractExpr::Result, add(ContractExpr::Old(Box::new(var("z"))), int_lit(1))));

        let report = verify_function(&contract, VerificationMode::CompileTime);
        assert!(!report.is_ok());
        assert!(matches!(
            &report.errors[0],
            ContractError::OldNonParam { var, .. } if var == "z"
        ));
    }

    #[test]
    fn test_function_contract_always_false() {
        let contract = FunctionContract::new("bad".to_string())
            .param("x".to_string(), "i32".to_string())
            .pre(bool_lit(false));

        let report = verify_function(&contract, VerificationMode::CompileTime);
        assert!(!report.is_ok());
        assert!(matches!(&report.errors[0], ContractError::AlwaysFalse { .. }));
    }

    #[test]
    fn test_function_contract_always_true_warning() {
        let contract = FunctionContract::new("trivial".to_string())
            .param("x".to_string(), "i32".to_string())
            .pre(bool_lit(true));

        let report = verify_function(&contract, VerificationMode::CompileTime);
        assert!(report.is_ok());
        assert_eq!(report.warnings.len(), 1);
        assert!(matches!(&report.warnings[0], ContractError::AlwaysTrue { .. }));
    }

    #[test]
    fn test_precondition_with_result_is_error() {
        let contract = FunctionContract::new("bad".to_string())
            .param("x".to_string(), "i32".to_string())
            .returns("i32".to_string())
            .pre(gt(ContractExpr::Result, int_lit(0)));

        let report = verify_function(&contract, VerificationMode::CompileTime);
        assert!(!report.is_ok());
    }

    // -- Type invariant tests --

    #[test]
    fn test_type_invariant_valid() {
        let ti = TypeInvariant::new("PositiveInt".to_string())
            .field("value".to_string(), "i32".to_string())
            .invariant(gt(var("value"), int_lit(0)));

        let report = verify_type_invariant(&ti, VerificationMode::CompileTime);
        assert!(report.is_ok());
    }

    #[test]
    fn test_type_invariant_unknown_field() {
        let ti = TypeInvariant::new("Bounded".to_string())
            .field("lo".to_string(), "i32".to_string())
            .field("hi".to_string(), "i32".to_string())
            .invariant(lt(var("lo"), var("maximum")));

        let report = verify_type_invariant(&ti, VerificationMode::CompileTime);
        assert!(!report.is_ok());
        assert!(matches!(
            &report.errors[0],
            ContractError::UnknownField { field, .. } if field == "maximum"
        ));
    }

    #[test]
    fn test_type_invariant_always_false() {
        let ti = TypeInvariant::new("Bad".to_string())
            .field("x".to_string(), "i32".to_string())
            .invariant(bool_lit(false));

        let report = verify_type_invariant(&ti, VerificationMode::CompileTime);
        assert!(!report.is_ok());
    }

    #[test]
    fn test_type_invariant_self_allowed() {
        let ti = TypeInvariant::new("Wrapper".to_string())
            .field("val".to_string(), "i32".to_string())
            .invariant(gt(var("self"), int_lit(0)));

        let report = verify_type_invariant(&ti, VerificationMode::CompileTime);
        // `self` should not be flagged as unknown field
        let field_errors: Vec<_> = report
            .errors
            .iter()
            .filter(|e| matches!(e, ContractError::UnknownField { .. }))
            .collect();
        assert!(field_errors.is_empty());
    }

    // -- Runtime assertion generation tests --

    #[test]
    fn test_runtime_assertions_precondition() {
        let contract = FunctionContract::new("div".to_string())
            .param("a".to_string(), "i32".to_string())
            .param("b".to_string(), "i32".to_string())
            .returns("i32".to_string())
            .pre_labeled(
                "divisor non-zero".to_string(),
                ContractExpr::BinOp(Box::new(var("b")), BinOp::Ne, Box::new(int_lit(0))),
            );

        let assertions = generate_runtime_assertions(&contract);
        assert_eq!(assertions.len(), 1);
        assert_eq!(assertions[0].kind, "precondition");
        assert!(assertions[0].assertion_code.contains("assert!"));
        assert!(assertions[0].message.contains("divisor non-zero"));
    }

    #[test]
    fn test_runtime_assertions_postcondition() {
        let contract = FunctionContract::new("abs".to_string())
            .param("x".to_string(), "i32".to_string())
            .returns("i32".to_string())
            .post(ContractExpr::BinOp(
                Box::new(ContractExpr::Result),
                BinOp::Ge,
                Box::new(int_lit(0)),
            ));

        let assertions = generate_runtime_assertions(&contract);
        assert_eq!(assertions.len(), 1);
        assert_eq!(assertions[0].kind, "postcondition");
        assert!(assertions[0].assertion_code.contains("__result"));
    }

    #[test]
    fn test_runtime_invariant_assertion() {
        let ti = TypeInvariant::new("NonEmpty".to_string())
            .field("len".to_string(), "usize".to_string())
            .invariant_labeled("must be non-empty".to_string(), gt(var("len"), int_lit(0)));

        let assertions = generate_invariant_assertions(&ti);
        assert_eq!(assertions.len(), 1);
        assert!(assertions[0].message.contains("must be non-empty"));
    }

    // -- Verification mode tests --

    #[test]
    fn test_compile_time_mode_no_runtime_assertions() {
        let contract = FunctionContract::new("f".to_string())
            .param("x".to_string(), "i32".to_string())
            .pre(gt(var("x"), int_lit(0)));

        let report = verify_function(&contract, VerificationMode::CompileTime);
        assert!(report.runtime_assertions.is_empty());
    }

    #[test]
    fn test_runtime_mode_skips_static_analysis() {
        // `false` as precondition should not be flagged in runtime-only mode
        let contract = FunctionContract::new("f".to_string())
            .param("x".to_string(), "i32".to_string())
            .pre(bool_lit(false));

        let report = verify_function(&contract, VerificationMode::Runtime);
        // No AlwaysFalse error in runtime mode (static analysis skipped)
        let always_false: Vec<_> = report
            .errors
            .iter()
            .filter(|e| matches!(e, ContractError::AlwaysFalse { .. }))
            .collect();
        assert!(always_false.is_empty());
        assert_eq!(report.runtime_assertions.len(), 1);
    }

    #[test]
    fn test_both_mode_has_static_and_runtime() {
        let contract = FunctionContract::new("f".to_string())
            .param("x".to_string(), "i32".to_string())
            .returns("i32".to_string())
            .pre(gt(var("x"), int_lit(0)))
            .post(gt(ContractExpr::Result, int_lit(0)));

        let report = verify_function(&contract, VerificationMode::Both);
        assert_eq!(report.runtime_assertions.len(), 2);
    }

    // -- Code generation tests --

    #[test]
    fn test_expr_to_code_forall() {
        let e = ContractExpr::ForAll(
            "i".to_string(),
            Box::new(var("arr")),
            Box::new(gt(var("i"), int_lit(0))),
        );
        let code = expr_to_code(&e);
        assert_eq!(code, "arr.iter().all(|i| (i > 0))");
    }

    #[test]
    fn test_expr_to_code_implies() {
        let e = implies(var("a"), var("b"));
        let code = expr_to_code(&e);
        assert_eq!(code, "(!a || b)");
    }

    // -- Report summary --

    #[test]
    fn test_report_summary() {
        let report = VerificationReport {
            mode: VerificationMode::Both,
            errors: vec![],
            warnings: vec![ContractError::AlwaysTrue {
                kind: "precondition".to_string(),
                label: None,
                location: "f".to_string(),
            }],
            runtime_assertions: vec![RuntimeAssertion {
                kind: "precondition".to_string(),
                label: None,
                assertion_code: "assert!(true);".to_string(),
                message: "ok".to_string(),
            }],
        };
        let s = report.summary();
        assert!(s.contains("0 errors"));
        assert!(s.contains("1 warnings"));
        assert!(s.contains("1 runtime assertions"));
    }

    // -- Contract kind tests --

    #[test]
    fn test_contract_kind_display() {
        let ck = ContractKind::Pre(Precondition {
            label: Some("positive".to_string()),
            expr: gt(var("x"), int_lit(0)),
        });
        let s = ck.to_string();
        assert!(s.contains("precondition"));
        assert!(s.contains("positive"));
    }

    #[test]
    fn test_all_contracts() {
        let contract = FunctionContract::new("f".to_string())
            .param("x".to_string(), "i32".to_string())
            .pre(gt(var("x"), int_lit(0)))
            .post(gt(ContractExpr::Result, int_lit(0)));
        let all = contract.all_contracts();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].kind_name(), "precondition");
        assert_eq!(all[1].kind_name(), "postcondition");
    }

    // -- Free vars and old vars --

    #[test]
    fn test_free_vars_forall_binds() {
        let e = ContractExpr::ForAll(
            "i".to_string(),
            Box::new(var("arr")),
            Box::new(and(gt(var("i"), int_lit(0)), lt(var("i"), var("max")))),
        );
        let mut vars = Vec::new();
        free_vars(&e, &mut vars);
        assert!(vars.contains(&"arr".to_string()));
        assert!(vars.contains(&"max".to_string()));
        assert!(!vars.contains(&"i".to_string()));
    }
}
