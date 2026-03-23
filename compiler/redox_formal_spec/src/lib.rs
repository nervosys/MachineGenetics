//! # Formal Specification Syntax
//!
//! `spec` blocks with `@req`/`@ens`/`@perf`/`@fx` clauses for
//! machine-verifiable function specifications.
//!
//! ```text
//! spec sort<T: Ord>(xs: &mut [T]) {
//!     @req xs.len() > 0
//!     @ens result.is_sorted()
//!     @ens result.len() == old(xs.len())
//!     @perf O(n * log(n))
//!     @fx pure
//! }
//! ```

use std::fmt;

// ── Specification Expressions ────────────────────────────────────────

/// Expression in a spec clause.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpecExpr {
    /// Boolean literal.
    BoolLit(bool),
    /// Integer literal.
    IntLit(i64),
    /// Variable reference.
    Var(String),
    /// `old(expr)` — value before function execution.
    Old(Box<SpecExpr>),
    /// `result` — the function's return value.
    Result,
    /// Field access: `expr.field`.
    Field(Box<SpecExpr>, String),
    /// Method call: `expr.method(args)`.
    MethodCall(Box<SpecExpr>, String, Vec<SpecExpr>),
    /// Function call: `f(args)`.
    Call(String, Vec<SpecExpr>),
    /// Binary operation.
    BinOp(Box<SpecExpr>, BinOp, Box<SpecExpr>),
    /// Unary operation.
    UnOp(UnOp, Box<SpecExpr>),
    /// Universal quantifier: `forall x in collection: predicate`.
    ForAll(String, Box<SpecExpr>, Box<SpecExpr>),
    /// Existential quantifier: `exists x in collection: predicate`.
    Exists(String, Box<SpecExpr>, Box<SpecExpr>),
    /// Implication: `a ==> b`.
    Implies(Box<SpecExpr>, Box<SpecExpr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnOp {
    Not,
    Neg,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "+"),
            Self::Sub => write!(f, "-"),
            Self::Mul => write!(f, "*"),
            Self::Div => write!(f, "/"),
            Self::Mod => write!(f, "%"),
            Self::Eq => write!(f, "=="),
            Self::Ne => write!(f, "!="),
            Self::Lt => write!(f, "<"),
            Self::Le => write!(f, "<="),
            Self::Gt => write!(f, ">"),
            Self::Ge => write!(f, ">="),
            Self::And => write!(f, "&&"),
            Self::Or => write!(f, "||"),
        }
    }
}

impl fmt::Display for UnOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Not => write!(f, "!"),
            Self::Neg => write!(f, "-"),
        }
    }
}

impl fmt::Display for SpecExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoolLit(b) => write!(f, "{b}"),
            Self::IntLit(n) => write!(f, "{n}"),
            Self::Var(v) => write!(f, "{v}"),
            Self::Old(e) => write!(f, "old({e})"),
            Self::Result => write!(f, "result"),
            Self::Field(e, name) => write!(f, "{e}.{name}"),
            Self::MethodCall(e, name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                write!(f, "{e}.{name}({})", args_str.join(", "))
            }
            Self::Call(name, args) => {
                let args_str: Vec<String> = args.iter().map(|a| format!("{a}")).collect();
                write!(f, "{name}({})", args_str.join(", "))
            }
            Self::BinOp(l, op, r) => write!(f, "({l} {op} {r})"),
            Self::UnOp(op, e) => write!(f, "{op}{e}"),
            Self::ForAll(v, coll, pred) => write!(f, "forall {v} in {coll}: {pred}"),
            Self::Exists(v, coll, pred) => write!(f, "exists {v} in {coll}: {pred}"),
            Self::Implies(a, b) => write!(f, "({a} ==> {b})"),
        }
    }
}

// ── Spec Clauses ─────────────────────────────────────────────────────

/// Complexity class for `@perf` clauses.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Complexity {
    O1,
    OLogN,
    ON,
    ONLogN,
    ON2,
    ON3,
    O2N,
    Custom(String),
}

impl fmt::Display for Complexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::O1 => write!(f, "O(1)"),
            Self::OLogN => write!(f, "O(log(n))"),
            Self::ON => write!(f, "O(n)"),
            Self::ONLogN => write!(f, "O(n * log(n))"),
            Self::ON2 => write!(f, "O(n^2)"),
            Self::ON3 => write!(f, "O(n^3)"),
            Self::O2N => write!(f, "O(2^n)"),
            Self::Custom(s) => write!(f, "O({s})"),
        }
    }
}

/// Effect kind for `@fx` clauses.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EffectKind {
    Pure,
    Io,
    Alloc,
    Diverge,
    Panic,
    Unsafe,
    Custom(String),
}

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pure => write!(f, "pure"),
            Self::Io => write!(f, "io"),
            Self::Alloc => write!(f, "alloc"),
            Self::Diverge => write!(f, "diverge"),
            Self::Panic => write!(f, "panic"),
            Self::Unsafe => write!(f, "unsafe"),
            Self::Custom(s) => write!(f, "{s}"),
        }
    }
}

/// A single clause in a spec block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecClause {
    /// `@req expr` — precondition.
    Requires(SpecExpr),
    /// `@req label: expr` — labeled precondition.
    RequiresLabeled(String, SpecExpr),
    /// `@ens expr` — postcondition.
    Ensures(SpecExpr),
    /// `@ens label: expr` — labeled postcondition.
    EnsuresLabeled(String, SpecExpr),
    /// `@perf complexity` — performance bound.
    Performance(Complexity),
    /// `@fx effect` — effect declaration.
    Effect(EffectKind),
    /// `@inv expr` — loop/type invariant.
    Invariant(SpecExpr),
    /// `@inv label: expr` — labeled invariant.
    InvariantLabeled(String, SpecExpr),
}

impl fmt::Display for SpecClause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Requires(e) => write!(f, "@req {e}"),
            Self::RequiresLabeled(l, e) => write!(f, "@req {l}: {e}"),
            Self::Ensures(e) => write!(f, "@ens {e}"),
            Self::EnsuresLabeled(l, e) => write!(f, "@ens {l}: {e}"),
            Self::Performance(c) => write!(f, "@perf {c}"),
            Self::Effect(e) => write!(f, "@fx {e}"),
            Self::Invariant(e) => write!(f, "@inv {e}"),
            Self::InvariantLabeled(l, e) => write!(f, "@inv {l}: {e}"),
        }
    }
}

// ── Parameter and Type ───────────────────────────────────────────────

/// A parameter in a spec signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecParam {
    pub name: String,
    pub ty: String,
    pub mutable: bool,
}

impl fmt::Display for SpecParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mutable {
            write!(f, "{}: &mut {}", self.name, self.ty)
        } else {
            write!(f, "{}: {}", self.name, self.ty)
        }
    }
}

/// A generic type parameter with optional bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParam {
    pub name: String,
    pub bounds: Vec<String>,
}

impl fmt::Display for GenericParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.bounds.is_empty() {
            write!(f, ": {}", self.bounds.join(" + "))?;
        }
        Ok(())
    }
}

// ── Spec Block ───────────────────────────────────────────────────────

/// A complete `spec` block for a function.
#[derive(Debug, Clone)]
pub struct SpecBlock {
    pub name: String,
    pub generics: Vec<GenericParam>,
    pub params: Vec<SpecParam>,
    pub return_type: Option<String>,
    pub clauses: Vec<SpecClause>,
}

impl SpecBlock {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            generics: Vec::new(),
            params: Vec::new(),
            return_type: None,
            clauses: Vec::new(),
        }
    }

    pub fn generic(mut self, name: impl Into<String>, bounds: Vec<String>) -> Self {
        self.generics.push(GenericParam { name: name.into(), bounds });
        self
    }

    pub fn param(mut self, name: impl Into<String>, ty: impl Into<String>) -> Self {
        self.params.push(SpecParam { name: name.into(), ty: ty.into(), mutable: false });
        self
    }

    pub fn param_mut(mut self, name: impl Into<String>, ty: impl Into<String>) -> Self {
        self.params.push(SpecParam { name: name.into(), ty: ty.into(), mutable: true });
        self
    }

    pub fn returns(mut self, ty: impl Into<String>) -> Self {
        self.return_type = Some(ty.into());
        self
    }

    pub fn requires(mut self, expr: SpecExpr) -> Self {
        self.clauses.push(SpecClause::Requires(expr));
        self
    }

    pub fn requires_labeled(mut self, label: impl Into<String>, expr: SpecExpr) -> Self {
        self.clauses.push(SpecClause::RequiresLabeled(label.into(), expr));
        self
    }

    pub fn ensures(mut self, expr: SpecExpr) -> Self {
        self.clauses.push(SpecClause::Ensures(expr));
        self
    }

    pub fn ensures_labeled(mut self, label: impl Into<String>, expr: SpecExpr) -> Self {
        self.clauses.push(SpecClause::EnsuresLabeled(label.into(), expr));
        self
    }

    pub fn perf(mut self, complexity: Complexity) -> Self {
        self.clauses.push(SpecClause::Performance(complexity));
        self
    }

    pub fn effect(mut self, effect: EffectKind) -> Self {
        self.clauses.push(SpecClause::Effect(effect));
        self
    }

    pub fn invariant(mut self, expr: SpecExpr) -> Self {
        self.clauses.push(SpecClause::Invariant(expr));
        self
    }

    // ── Queries ──────────────────────────────────────────────────

    pub fn preconditions(&self) -> Vec<&SpecExpr> {
        self.clauses
            .iter()
            .filter_map(|c| match c {
                SpecClause::Requires(e) | SpecClause::RequiresLabeled(_, e) => Some(e),
                _ => None,
            })
            .collect()
    }

    pub fn postconditions(&self) -> Vec<&SpecExpr> {
        self.clauses
            .iter()
            .filter_map(|c| match c {
                SpecClause::Ensures(e) | SpecClause::EnsuresLabeled(_, e) => Some(e),
                _ => None,
            })
            .collect()
    }

    pub fn performance_bounds(&self) -> Vec<&Complexity> {
        self.clauses
            .iter()
            .filter_map(|c| match c {
                SpecClause::Performance(p) => Some(p),
                _ => None,
            })
            .collect()
    }

    pub fn effects(&self) -> Vec<&EffectKind> {
        self.clauses
            .iter()
            .filter_map(|c| match c {
                SpecClause::Effect(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    pub fn invariants(&self) -> Vec<&SpecExpr> {
        self.clauses
            .iter()
            .filter_map(|c| match c {
                SpecClause::Invariant(e) | SpecClause::InvariantLabeled(_, e) => Some(e),
                _ => None,
            })
            .collect()
    }

    pub fn is_pure(&self) -> bool {
        self.effects().iter().any(|e| **e == EffectKind::Pure)
    }
}

impl fmt::Display for SpecBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "spec {}", self.name)?;
        if !self.generics.is_empty() {
            let gs: Vec<String> = self.generics.iter().map(|g| format!("{g}")).collect();
            write!(f, "<{}>", gs.join(", "))?;
        }
        let ps: Vec<String> = self.params.iter().map(|p| format!("{p}")).collect();
        write!(f, "({})", ps.join(", "))?;
        if let Some(ref ret) = self.return_type {
            write!(f, " -> {ret}")?;
        }
        writeln!(f, " {{")?;
        for c in &self.clauses {
            writeln!(f, "    {c}")?;
        }
        write!(f, "}}")
    }
}

// ── Parsing ──────────────────────────────────────────────────────────

/// Parse error for spec syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at {}:{}: {}", self.line, self.col, self.message)
    }
}

/// Parse a complexity string like "O(n * log(n))".
pub fn parse_complexity(s: &str) -> Result<Complexity, String> {
    let s = s.trim();
    match s {
        "O(1)" => Ok(Complexity::O1),
        "O(log(n))" | "O(log n)" => Ok(Complexity::OLogN),
        "O(n)" => Ok(Complexity::ON),
        "O(n * log(n))" | "O(n*log(n))" | "O(n log n)" => Ok(Complexity::ONLogN),
        "O(n^2)" | "O(n*n)" => Ok(Complexity::ON2),
        "O(n^3)" => Ok(Complexity::ON3),
        "O(2^n)" => Ok(Complexity::O2N),
        _ => {
            if s.starts_with("O(") && s.ends_with(')') {
                let inner = &s[2..s.len() - 1];
                Ok(Complexity::Custom(inner.to_string()))
            } else {
                Err(format!("invalid complexity: {s}"))
            }
        }
    }
}

/// Parse an effect string.
pub fn parse_effect(s: &str) -> Result<EffectKind, String> {
    match s.trim() {
        "pure" => Ok(EffectKind::Pure),
        "io" => Ok(EffectKind::Io),
        "alloc" => Ok(EffectKind::Alloc),
        "diverge" => Ok(EffectKind::Diverge),
        "panic" => Ok(EffectKind::Panic),
        "unsafe" => Ok(EffectKind::Unsafe),
        other => Ok(EffectKind::Custom(other.to_string())),
    }
}

/// Simple expression parser for spec clause bodies.
/// Supports variables, int literals, bool literals, `old(...)`, `result`,
/// binary ops, and field access.
pub fn parse_expr(s: &str) -> Result<SpecExpr, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty expression".into());
    }

    // Try to parse implications: `a ==> b`
    if let Some(pos) = s.find("==>") {
        let left = s[..pos].trim();
        let right = s[pos + 3..].trim();
        if !left.is_empty() && !right.is_empty() {
            return Ok(SpecExpr::Implies(
                Box::new(parse_expr(left)?),
                Box::new(parse_expr(right)?),
            ));
        }
    }

    // Try to parse logical: `a && b`, `a || b`
    for (op_str, op) in &[("&&", BinOp::And), ("||", BinOp::Or)] {
        if let Some(pos) = find_top_level_op(s, op_str) {
            let left = s[..pos].trim();
            let right = s[pos + op_str.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                return Ok(SpecExpr::BinOp(
                    Box::new(parse_expr(left)?),
                    *op,
                    Box::new(parse_expr(right)?),
                ));
            }
        }
    }

    // Comparison: ==, !=, <=, >=, <, >
    for (op_str, op) in &[
        ("==", BinOp::Eq),
        ("!=", BinOp::Ne),
        ("<=", BinOp::Le),
        (">=", BinOp::Ge),
        ("<", BinOp::Lt),
        (">", BinOp::Gt),
    ] {
        if let Some(pos) = find_top_level_op(s, op_str) {
            let left = s[..pos].trim();
            let right = s[pos + op_str.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                return Ok(SpecExpr::BinOp(
                    Box::new(parse_expr(left)?),
                    *op,
                    Box::new(parse_expr(right)?),
                ));
            }
        }
    }

    // Arithmetic: +, -, *, /, %
    for (op_str, op) in &[("+", BinOp::Add), ("-", BinOp::Sub)] {
        if let Some(pos) = find_top_level_op(s, op_str) {
            let left = s[..pos].trim();
            let right = s[pos + op_str.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                return Ok(SpecExpr::BinOp(
                    Box::new(parse_expr(left)?),
                    *op,
                    Box::new(parse_expr(right)?),
                ));
            }
        }
    }

    for (op_str, op) in &[("*", BinOp::Mul), ("/", BinOp::Div), ("%", BinOp::Mod)] {
        if let Some(pos) = find_top_level_op(s, op_str) {
            let left = s[..pos].trim();
            let right = s[pos + op_str.len()..].trim();
            if !left.is_empty() && !right.is_empty() {
                return Ok(SpecExpr::BinOp(
                    Box::new(parse_expr(left)?),
                    *op,
                    Box::new(parse_expr(right)?),
                ));
            }
        }
    }

    // Unary not
    if s.starts_with('!') {
        return Ok(SpecExpr::UnOp(UnOp::Not, Box::new(parse_expr(&s[1..])?)));
    }

    // Parenthesized expression
    if s.starts_with('(') && s.ends_with(')') {
        return parse_expr(&s[1..s.len() - 1]);
    }

    // `old(...)`
    if s.starts_with("old(") && s.ends_with(')') {
        return Ok(SpecExpr::Old(Box::new(parse_expr(&s[4..s.len() - 1])?)));
    }

    // `result`
    if s == "result" {
        return Ok(SpecExpr::Result);
    }

    // Bool literal
    if s == "true" {
        return Ok(SpecExpr::BoolLit(true));
    }
    if s == "false" {
        return Ok(SpecExpr::BoolLit(false));
    }

    // Int literal
    if let Ok(n) = s.parse::<i64>() {
        return Ok(SpecExpr::IntLit(n));
    }

    // Field access or method call: e.g. `xs.len()`, `xs.is_sorted`
    if let Some(dot_pos) = s.rfind('.') {
        let left = s[..dot_pos].trim();
        let right = s[dot_pos + 1..].trim();
        if !left.is_empty() && !right.is_empty() {
            let base = parse_expr(left)?;
            if right.ends_with("()") {
                let method = &right[..right.len() - 2];
                return Ok(SpecExpr::MethodCall(Box::new(base), method.to_string(), vec![]));
            } else {
                return Ok(SpecExpr::Field(Box::new(base), right.to_string()));
            }
        }
    }

    // Variable / identifier
    if s.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Ok(SpecExpr::Var(s.to_string()));
    }

    Err(format!("cannot parse expression: {s}"))
}

/// Find a top-level operator (not inside parentheses).
fn find_top_level_op(s: &str, op: &str) -> Option<usize> {
    let mut depth = 0i32;
    let bytes = s.as_bytes();
    let op_bytes = op.as_bytes();
    // Search from right to left for left-associative ops
    let mut i = s.len();
    while i > 0 {
        i -= 1;
        if bytes[i] == b')' {
            depth += 1;
        } else if bytes[i] == b'(' {
            depth -= 1;
        } else if depth == 0 && i + op_bytes.len() <= s.len() {
            // Don't match == inside ==> or != inside ==>
            if op == "==" && i + 3 <= s.len() && &bytes[i..i + 3] == b"==>" {
                continue;
            }
            if &bytes[i..i + op_bytes.len()] == op_bytes {
                // Avoid matching <= inside ==>
                if op == "<" && i > 0 && (bytes[i - 1] == b'=' || bytes[i - 1] == b'!') {
                    continue;
                }
                if op == ">" && i >= 2 && bytes[i - 1] == b'=' && bytes[i - 2] == b'=' {
                    continue;
                }
                return Some(i);
            }
        }
    }
    None
}

/// Parse a clause line like `@req x > 0` or `@perf O(n)`.
pub fn parse_clause(line: &str) -> Result<SpecClause, String> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("@req") {
        let rest = rest.trim();
        if let Some(colon_pos) = rest.find(':') {
            let label = rest[..colon_pos].trim();
            let expr_str = rest[colon_pos + 1..].trim();
            if !label.is_empty() && label.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Ok(SpecClause::RequiresLabeled(label.to_string(), parse_expr(expr_str)?));
            }
        }
        Ok(SpecClause::Requires(parse_expr(rest)?))
    } else if let Some(rest) = line.strip_prefix("@ens") {
        let rest = rest.trim();
        if let Some(colon_pos) = rest.find(':') {
            let label = rest[..colon_pos].trim();
            let expr_str = rest[colon_pos + 1..].trim();
            if !label.is_empty() && label.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Ok(SpecClause::EnsuresLabeled(label.to_string(), parse_expr(expr_str)?));
            }
        }
        Ok(SpecClause::Ensures(parse_expr(rest)?))
    } else if let Some(rest) = line.strip_prefix("@perf") {
        Ok(SpecClause::Performance(parse_complexity(rest.trim())?))
    } else if let Some(rest) = line.strip_prefix("@fx") {
        Ok(SpecClause::Effect(parse_effect(rest.trim())?))
    } else if let Some(rest) = line.strip_prefix("@inv") {
        let rest = rest.trim();
        if let Some(colon_pos) = rest.find(':') {
            let label = rest[..colon_pos].trim();
            let expr_str = rest[colon_pos + 1..].trim();
            if !label.is_empty() && label.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Ok(SpecClause::InvariantLabeled(label.to_string(), parse_expr(expr_str)?));
            }
        }
        Ok(SpecClause::Invariant(parse_expr(rest)?))
    } else {
        Err(format!("unknown clause: {line}"))
    }
}

// ── Validation ───────────────────────────────────────────────────────

/// Validation error for a spec block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub message: String,
    pub clause_index: Option<usize>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(idx) = self.clause_index {
            write!(f, "clause {}: {}", idx, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

/// Collect free variables from an expression.
pub fn free_vars(expr: &SpecExpr) -> Vec<String> {
    let mut vars = Vec::new();
    collect_free_vars(expr, &mut vars);
    vars.sort();
    vars.dedup();
    vars
}

fn collect_free_vars(expr: &SpecExpr, vars: &mut Vec<String>) {
    match expr {
        SpecExpr::BoolLit(_) | SpecExpr::IntLit(_) | SpecExpr::Result => {}
        SpecExpr::Var(v) => vars.push(v.clone()),
        SpecExpr::Old(e) | SpecExpr::UnOp(_, e) => collect_free_vars(e, vars),
        SpecExpr::Field(e, _) => collect_free_vars(e, vars),
        SpecExpr::MethodCall(e, _, args) => {
            collect_free_vars(e, vars);
            for a in args {
                collect_free_vars(a, vars);
            }
        }
        SpecExpr::Call(_, args) => {
            for a in args {
                collect_free_vars(a, vars);
            }
        }
        SpecExpr::BinOp(l, _, r) | SpecExpr::Implies(l, r) => {
            collect_free_vars(l, vars);
            collect_free_vars(r, vars);
        }
        SpecExpr::ForAll(v, coll, pred) | SpecExpr::Exists(v, coll, pred) => {
            collect_free_vars(coll, vars);
            let mut inner = Vec::new();
            collect_free_vars(pred, &mut inner);
            inner.retain(|x| x != v);
            vars.extend(inner);
        }
    }
}

/// Validate a spec block for common errors.
pub fn validate(spec: &SpecBlock) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let param_names: Vec<&str> = spec.params.iter().map(|p| p.name.as_str()).collect();

    for (i, clause) in spec.clauses.iter().enumerate() {
        match clause {
            SpecClause::Requires(e) | SpecClause::RequiresLabeled(_, e) => {
                // Preconditions should not reference `result`
                if uses_result(e) {
                    errors.push(ValidationError {
                        message: "precondition cannot reference `result`".into(),
                        clause_index: Some(i),
                    });
                }
                // Check free vars are in params
                for v in free_vars(e) {
                    if !param_names.contains(&v.as_str()) {
                        errors.push(ValidationError {
                            message: format!("unknown variable `{v}` in precondition"),
                            clause_index: Some(i),
                        });
                    }
                }
            }
            SpecClause::Ensures(e) | SpecClause::EnsuresLabeled(_, e) => {
                // Postconditions can reference `result` and `old()` — just check vars
                let fv = free_vars(e);
                for v in &fv {
                    if !param_names.contains(&v.as_str()) {
                        errors.push(ValidationError {
                            message: format!("unknown variable `{v}` in postcondition"),
                            clause_index: Some(i),
                        });
                    }
                }
            }
            SpecClause::Performance(_) | SpecClause::Effect(_) => {}
            SpecClause::Invariant(e) | SpecClause::InvariantLabeled(_, e) => {
                for v in free_vars(e) {
                    if !param_names.contains(&v.as_str()) {
                        errors.push(ValidationError {
                            message: format!("unknown variable `{v}` in invariant"),
                            clause_index: Some(i),
                        });
                    }
                }
            }
        }
    }

    // Check for conflicting effects
    let effects = spec.effects();
    if effects.iter().any(|e| **e == EffectKind::Pure) && effects.len() > 1 {
        errors.push(ValidationError {
            message: "function declared `pure` cannot have other effects".into(),
            clause_index: None,
        });
    }

    errors
}

/// Check if an expression references `result`.
fn uses_result(expr: &SpecExpr) -> bool {
    match expr {
        SpecExpr::Result => true,
        SpecExpr::BoolLit(_) | SpecExpr::IntLit(_) | SpecExpr::Var(_) => false,
        SpecExpr::Old(e) | SpecExpr::UnOp(_, e) | SpecExpr::Field(e, _) => uses_result(e),
        SpecExpr::MethodCall(e, _, args) => uses_result(e) || args.iter().any(|a| uses_result(a)),
        SpecExpr::Call(_, args) => args.iter().any(|a| uses_result(a)),
        SpecExpr::BinOp(l, _, r) | SpecExpr::Implies(l, r) => uses_result(l) || uses_result(r),
        SpecExpr::ForAll(_, c, p) | SpecExpr::Exists(_, c, p) => uses_result(c) || uses_result(p),
    }
}

// ── Runtime Check Generation ─────────────────────────────────────────

/// A generated runtime check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCheck {
    pub kind: CheckKind,
    pub label: Option<String>,
    pub expression: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    Precondition,
    Postcondition,
    Invariant,
}

impl fmt::Display for CheckKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Precondition => write!(f, "precondition"),
            Self::Postcondition => write!(f, "postcondition"),
            Self::Invariant => write!(f, "invariant"),
        }
    }
}

/// Generate runtime assertion checks from a spec block.
pub fn generate_runtime_checks(spec: &SpecBlock) -> Vec<RuntimeCheck> {
    let mut checks = Vec::new();
    for clause in &spec.clauses {
        match clause {
            SpecClause::Requires(e) => {
                checks.push(RuntimeCheck {
                    kind: CheckKind::Precondition,
                    label: None,
                    expression: format!("{e}"),
                });
            }
            SpecClause::RequiresLabeled(l, e) => {
                checks.push(RuntimeCheck {
                    kind: CheckKind::Precondition,
                    label: Some(l.clone()),
                    expression: format!("{e}"),
                });
            }
            SpecClause::Ensures(e) => {
                checks.push(RuntimeCheck {
                    kind: CheckKind::Postcondition,
                    label: None,
                    expression: format!("{e}"),
                });
            }
            SpecClause::EnsuresLabeled(l, e) => {
                checks.push(RuntimeCheck {
                    kind: CheckKind::Postcondition,
                    label: Some(l.clone()),
                    expression: format!("{e}"),
                });
            }
            SpecClause::Invariant(e) => {
                checks.push(RuntimeCheck {
                    kind: CheckKind::Invariant,
                    label: None,
                    expression: format!("{e}"),
                });
            }
            SpecClause::InvariantLabeled(l, e) => {
                checks.push(RuntimeCheck {
                    kind: CheckKind::Invariant,
                    label: Some(l.clone()),
                    expression: format!("{e}"),
                });
            }
            SpecClause::Performance(_) | SpecClause::Effect(_) => {}
        }
    }
    checks
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_block_builder() {
        let spec = SpecBlock::new("sort")
            .generic("T", vec!["Ord".into()])
            .param_mut("xs", "[T]")
            .returns("Vec<T>")
            .requires(SpecExpr::BinOp(
                Box::new(SpecExpr::MethodCall(
                    Box::new(SpecExpr::Var("xs".into())),
                    "len".into(),
                    vec![],
                )),
                BinOp::Gt,
                Box::new(SpecExpr::IntLit(0)),
            ))
            .ensures(SpecExpr::MethodCall(Box::new(SpecExpr::Result), "is_sorted".into(), vec![]))
            .perf(Complexity::ONLogN)
            .effect(EffectKind::Pure);

        assert_eq!(spec.name, "sort");
        assert_eq!(spec.generics.len(), 1);
        assert_eq!(spec.params.len(), 1);
        assert!(spec.params[0].mutable);
        assert_eq!(spec.preconditions().len(), 1);
        assert_eq!(spec.postconditions().len(), 1);
        assert_eq!(spec.performance_bounds().len(), 1);
        assert!(spec.is_pure());
    }

    #[test]
    fn test_spec_block_display() {
        let spec = SpecBlock::new("abs")
            .param("x", "i32")
            .returns("i32")
            .requires(SpecExpr::BoolLit(true))
            .ensures(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Ge,
                Box::new(SpecExpr::IntLit(0)),
            ));
        let s = format!("{spec}");
        assert!(s.contains("spec abs"));
        assert!(s.contains("@req true"));
        assert!(s.contains("@ens (result >= 0)"));
    }

    #[test]
    fn test_parse_expr_int() {
        assert_eq!(parse_expr("42").unwrap(), SpecExpr::IntLit(42));
        assert_eq!(parse_expr("-5").unwrap(), SpecExpr::IntLit(-5));
    }

    #[test]
    fn test_parse_expr_bool() {
        assert_eq!(parse_expr("true").unwrap(), SpecExpr::BoolLit(true));
        assert_eq!(parse_expr("false").unwrap(), SpecExpr::BoolLit(false));
    }

    #[test]
    fn test_parse_expr_var() {
        assert_eq!(parse_expr("x").unwrap(), SpecExpr::Var("x".into()));
    }

    #[test]
    fn test_parse_expr_result() {
        assert_eq!(parse_expr("result").unwrap(), SpecExpr::Result);
    }

    #[test]
    fn test_parse_expr_old() {
        let expr = parse_expr("old(x)").unwrap();
        assert_eq!(expr, SpecExpr::Old(Box::new(SpecExpr::Var("x".into()))));
    }

    #[test]
    fn test_parse_expr_comparison() {
        let expr = parse_expr("x > 0").unwrap();
        assert_eq!(
            expr,
            SpecExpr::BinOp(
                Box::new(SpecExpr::Var("x".into())),
                BinOp::Gt,
                Box::new(SpecExpr::IntLit(0)),
            )
        );
    }

    #[test]
    fn test_parse_expr_field_access() {
        let expr = parse_expr("xs.len()").unwrap();
        assert_eq!(
            expr,
            SpecExpr::MethodCall(Box::new(SpecExpr::Var("xs".into())), "len".into(), vec![],)
        );
    }

    #[test]
    fn test_parse_expr_implies() {
        let expr = parse_expr("x > 0 ==> y > 0").unwrap();
        match expr {
            SpecExpr::Implies(_, _) => {}
            _ => panic!("expected Implies"),
        }
    }

    #[test]
    fn test_parse_expr_logical() {
        let expr = parse_expr("a && b").unwrap();
        assert_eq!(
            expr,
            SpecExpr::BinOp(
                Box::new(SpecExpr::Var("a".into())),
                BinOp::And,
                Box::new(SpecExpr::Var("b".into())),
            )
        );
    }

    #[test]
    fn test_parse_expr_arithmetic() {
        let expr = parse_expr("x + 1").unwrap();
        assert_eq!(
            expr,
            SpecExpr::BinOp(
                Box::new(SpecExpr::Var("x".into())),
                BinOp::Add,
                Box::new(SpecExpr::IntLit(1)),
            )
        );
    }

    #[test]
    fn test_parse_expr_not() {
        let expr = parse_expr("!x").unwrap();
        assert_eq!(expr, SpecExpr::UnOp(UnOp::Not, Box::new(SpecExpr::Var("x".into()))));
    }

    #[test]
    fn test_parse_clause_req() {
        let clause = parse_clause("@req x > 0").unwrap();
        match clause {
            SpecClause::Requires(_) => {}
            _ => panic!("expected Requires"),
        }
    }

    #[test]
    fn test_parse_clause_req_labeled() {
        let clause = parse_clause("@req positive: x > 0").unwrap();
        match clause {
            SpecClause::RequiresLabeled(l, _) => assert_eq!(l, "positive"),
            _ => panic!("expected RequiresLabeled"),
        }
    }

    #[test]
    fn test_parse_clause_ens() {
        let clause = parse_clause("@ens result >= 0").unwrap();
        match clause {
            SpecClause::Ensures(_) => {}
            _ => panic!("expected Ensures"),
        }
    }

    #[test]
    fn test_parse_clause_perf() {
        let clause = parse_clause("@perf O(n * log(n))").unwrap();
        assert_eq!(clause, SpecClause::Performance(Complexity::ONLogN));
    }

    #[test]
    fn test_parse_clause_fx() {
        let clause = parse_clause("@fx pure").unwrap();
        assert_eq!(clause, SpecClause::Effect(EffectKind::Pure));
    }

    #[test]
    fn test_parse_clause_inv() {
        let clause = parse_clause("@inv x > 0").unwrap();
        match clause {
            SpecClause::Invariant(_) => {}
            _ => panic!("expected Invariant"),
        }
    }

    #[test]
    fn test_parse_complexity_variants() {
        assert_eq!(parse_complexity("O(1)").unwrap(), Complexity::O1);
        assert_eq!(parse_complexity("O(n)").unwrap(), Complexity::ON);
        assert_eq!(parse_complexity("O(n^2)").unwrap(), Complexity::ON2);
        assert_eq!(parse_complexity("O(2^n)").unwrap(), Complexity::O2N);
        assert_eq!(parse_complexity("O(log(n))").unwrap(), Complexity::OLogN);
    }

    #[test]
    fn test_parse_complexity_custom() {
        let c = parse_complexity("O(n * m)").unwrap();
        assert_eq!(c, Complexity::Custom("n * m".into()));
    }

    #[test]
    fn test_parse_effect_variants() {
        assert_eq!(parse_effect("pure").unwrap(), EffectKind::Pure);
        assert_eq!(parse_effect("io").unwrap(), EffectKind::Io);
        assert_eq!(parse_effect("alloc").unwrap(), EffectKind::Alloc);
        assert_eq!(parse_effect("diverge").unwrap(), EffectKind::Diverge);
        assert_eq!(parse_effect("panic").unwrap(), EffectKind::Panic);
    }

    #[test]
    fn test_validate_ok() {
        let spec = SpecBlock::new("inc")
            .param("x", "i32")
            .returns("i32")
            .requires(SpecExpr::BinOp(
                Box::new(SpecExpr::Var("x".into())),
                BinOp::Gt,
                Box::new(SpecExpr::IntLit(0)),
            ))
            .ensures(SpecExpr::BinOp(
                Box::new(SpecExpr::Result),
                BinOp::Eq,
                Box::new(SpecExpr::BinOp(
                    Box::new(SpecExpr::Var("x".into())),
                    BinOp::Add,
                    Box::new(SpecExpr::IntLit(1)),
                )),
            ))
            .effect(EffectKind::Pure);
        let errors = validate(&spec);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_validate_result_in_precondition() {
        let spec = SpecBlock::new("bad").param("x", "i32").requires(SpecExpr::BinOp(
            Box::new(SpecExpr::Result),
            BinOp::Gt,
            Box::new(SpecExpr::IntLit(0)),
        ));
        let errors = validate(&spec);
        assert!(errors.iter().any(|e| e.message.contains("result")));
    }

    #[test]
    fn test_validate_unknown_var() {
        let spec = SpecBlock::new("bad").param("x", "i32").requires(SpecExpr::BinOp(
            Box::new(SpecExpr::Var("z".into())),
            BinOp::Gt,
            Box::new(SpecExpr::IntLit(0)),
        ));
        let errors = validate(&spec);
        assert!(errors.iter().any(|e| e.message.contains("z")));
    }

    #[test]
    fn test_validate_pure_conflict() {
        let spec = SpecBlock::new("bad").effect(EffectKind::Pure).effect(EffectKind::Io);
        let errors = validate(&spec);
        assert!(errors.iter().any(|e| e.message.contains("pure")));
    }

    #[test]
    fn test_free_vars() {
        let expr = SpecExpr::BinOp(
            Box::new(SpecExpr::Var("x".into())),
            BinOp::Add,
            Box::new(SpecExpr::Var("y".into())),
        );
        let fv = free_vars(&expr);
        assert_eq!(fv, vec!["x", "y"]);
    }

    #[test]
    fn test_free_vars_with_quantifier() {
        let expr = SpecExpr::ForAll(
            "i".into(),
            Box::new(SpecExpr::Var("xs".into())),
            Box::new(SpecExpr::BinOp(
                Box::new(SpecExpr::Var("i".into())),
                BinOp::Gt,
                Box::new(SpecExpr::IntLit(0)),
            )),
        );
        let fv = free_vars(&expr);
        assert_eq!(fv, vec!["xs"]);
        assert!(!fv.contains(&"i".to_string()));
    }

    #[test]
    fn test_generate_runtime_checks() {
        let spec = SpecBlock::new("test")
            .param("x", "i32")
            .requires(SpecExpr::BoolLit(true))
            .ensures(SpecExpr::BoolLit(true))
            .perf(Complexity::O1)
            .effect(EffectKind::Pure);
        let checks = generate_runtime_checks(&spec);
        assert_eq!(checks.len(), 2);
        assert_eq!(checks[0].kind, CheckKind::Precondition);
        assert_eq!(checks[1].kind, CheckKind::Postcondition);
    }

    #[test]
    fn test_runtime_check_with_label() {
        let spec = SpecBlock::new("test").requires_labeled("non_empty", SpecExpr::BoolLit(true));
        let checks = generate_runtime_checks(&spec);
        assert_eq!(checks[0].label.as_deref(), Some("non_empty"));
    }

    #[test]
    fn test_complexity_display() {
        assert_eq!(format!("{}", Complexity::O1), "O(1)");
        assert_eq!(format!("{}", Complexity::ONLogN), "O(n * log(n))");
        assert_eq!(format!("{}", Complexity::O2N), "O(2^n)");
        assert_eq!(format!("{}", Complexity::Custom("n * m".into())), "O(n * m)");
    }

    #[test]
    fn test_effect_display() {
        assert_eq!(format!("{}", EffectKind::Pure), "pure");
        assert_eq!(format!("{}", EffectKind::Io), "io");
        assert_eq!(format!("{}", EffectKind::Custom("net".into())), "net");
    }

    #[test]
    fn test_clause_display() {
        let c = SpecClause::Requires(SpecExpr::BoolLit(true));
        assert_eq!(format!("{c}"), "@req true");
        let c = SpecClause::Performance(Complexity::ON);
        assert_eq!(format!("{c}"), "@perf O(n)");
    }

    #[test]
    fn test_binop_display() {
        assert_eq!(format!("{}", BinOp::Add), "+");
        assert_eq!(format!("{}", BinOp::Le), "<=");
        assert_eq!(format!("{}", BinOp::And), "&&");
    }

    #[test]
    fn test_spec_expr_display() {
        let e = SpecExpr::ForAll(
            "i".into(),
            Box::new(SpecExpr::Var("xs".into())),
            Box::new(SpecExpr::BinOp(
                Box::new(SpecExpr::Var("i".into())),
                BinOp::Gt,
                Box::new(SpecExpr::IntLit(0)),
            )),
        );
        let s = format!("{e}");
        assert!(s.contains("forall i in xs"));
    }

    #[test]
    fn test_check_kind_display() {
        assert_eq!(format!("{}", CheckKind::Precondition), "precondition");
        assert_eq!(format!("{}", CheckKind::Postcondition), "postcondition");
    }

    #[test]
    fn test_parse_error_display() {
        let e = ParseError { message: "bad".into(), line: 1, col: 5 };
        assert_eq!(format!("{e}"), "parse error at 1:5: bad");
    }

    #[test]
    fn test_validation_error_display() {
        let e = ValidationError { message: "test".into(), clause_index: Some(2) };
        assert_eq!(format!("{e}"), "clause 2: test");
        let e = ValidationError { message: "global".into(), clause_index: None };
        assert_eq!(format!("{e}"), "global");
    }

    #[test]
    fn test_generic_param_display() {
        let g = GenericParam { name: "T".into(), bounds: vec!["Ord".into(), "Clone".into()] };
        assert_eq!(format!("{g}"), "T: Ord + Clone");
        let g = GenericParam { name: "U".into(), bounds: vec![] };
        assert_eq!(format!("{g}"), "U");
    }

    #[test]
    fn test_spec_param_display() {
        let p = SpecParam { name: "x".into(), ty: "i32".into(), mutable: false };
        assert_eq!(format!("{p}"), "x: i32");
        let p = SpecParam { name: "xs".into(), ty: "[T]".into(), mutable: true };
        assert_eq!(format!("{p}"), "xs: &mut [T]");
    }

    #[test]
    fn test_uses_result_detection() {
        assert!(uses_result(&SpecExpr::Result));
        assert!(!uses_result(&SpecExpr::Var("x".into())));
        assert!(uses_result(&SpecExpr::Old(Box::new(SpecExpr::Result))));
    }

    #[test]
    fn test_is_pure() {
        let spec = SpecBlock::new("f").effect(EffectKind::Pure);
        assert!(spec.is_pure());
        let spec = SpecBlock::new("g").effect(EffectKind::Io);
        assert!(!spec.is_pure());
    }

    #[test]
    fn test_invariants() {
        let spec = SpecBlock::new("f").param("x", "i32").invariant(SpecExpr::BinOp(
            Box::new(SpecExpr::Var("x".into())),
            BinOp::Gt,
            Box::new(SpecExpr::IntLit(0)),
        ));
        assert_eq!(spec.invariants().len(), 1);
    }
}
