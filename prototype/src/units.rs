//! Units of measure (plan C2).
//!
//! A float type may carry a dimension: `f64[m/s]`, `f32[kg*m/s^2]`. A
//! dimension is a vector of integer exponents over the seven SI base units
//! (`kg`, `m`, `s`, `A`, `K`, `mol`, `cd`); a bare `f64` is dimensionless.
//! The derived names `N`, `J`, `W`, `Pa` and `Hz` are accepted and expand to
//! base exponents, so `f64[N]` and `f64[kg*m/s^2]` are the same type.
//!
//! This pass runs after the type checker, from [`crate::types::check`], so a
//! unit error is a type error. Dimensions are erased before evaluation and
//! every other static pass sees the base float type.
//!
//! The rules:
//!
//! - `+`, `-`, `%`, the comparisons, `min`/`max`, `=`, `+=` and `-=` need the
//!   same dimension on both sides; the result keeps it.
//! - `*` adds exponents and `/` subtracts them. `*=` and `/=` need a
//!   dimensionless right-hand side, since they cannot change the target's unit.
//! - A numeric literal is **polymorphic**: it takes the unit of the other
//!   operand of `+`, `-` or a comparison (`x + 1.0`), and is a plain scalar
//!   under `*` and `/` (`2.0 * x` has the unit of `x`). A `val` bound to a
//!   literal without an annotation stays polymorphic; annotate it to fix it.
//! - Parameters, returns, struct fields, consts and annotated `val`s carry
//!   their declared dimension. A call checks each argument, and a function
//!   checks what it returns.
//! - `sqrt` halves every exponent and needs them all even. `abs`, `floor`,
//!   `ceil`, `round` and `trunc` keep the unit. `exp`, `ln`, `log`, `log2`,
//!   `log10`, the trigonometric and hyperbolic functions and `sigmoid` need a
//!   dimensionless argument and return one. Each may be called as `f(x)` or
//!   `x.f()`.
//! - `x as f64` drops a unit, and `x as f64[m]` asserts one: the deliberate
//!   escape hatch.
//! - A contract clause of a function is checked like its body, with the
//!   parameters in scope and `result` carrying the return dimension in an
//!   `@ens`. A clause `contract::collect` does not reach is not checked.
//!
//! Where the pass cannot tell a dimension (a method of a user type, a closure
//! parameter, an index into something untyped), it says nothing. A mismatch
//! is reported only between two dimensions it knows, so code without unit
//! annotations, where every known dimension is dimensionless, cannot fail it.
//!
//! A mismatch is **E0562** ([`DiagnosticCategory::UnitMismatch`]), and the
//! message names both dimensions: `cannot add m and s`. An unknown unit name
//! is E0412 (`UnresolvedType`).

use crate::ast::{self, Block, Expr, ItemKind, LiteralKind, Pattern, Stmt, Type};
use crate::hir::{Diagnostic, DiagnosticCategory, Severity};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// The base units, in display order.
pub const BASE_UNITS: [&str; 7] = ["kg", "m", "s", "A", "K", "mol", "cd"];

/// A dimension: one integer exponent per base unit, in [`BASE_UNITS`] order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dim(pub [i32; 7]);

impl Dim {
    /// The dimension of a pure number.
    pub const ONE: Dim = Dim([0; 7]);

    pub fn is_one(self) -> bool {
        self == Dim::ONE
    }

    fn base(i: usize) -> Dim {
        let mut e = [0; 7];
        e[i] = 1;
        Dim(e)
    }

    fn zip(self, other: Dim, f: impl Fn(i32, i32) -> i32) -> Dim {
        let mut e = [0; 7];
        for (i, x) in e.iter_mut().enumerate() {
            *x = f(self.0[i], other.0[i]);
        }
        Dim(e)
    }

    pub fn mul(self, other: Dim) -> Dim {
        self.zip(other, |a, b| a + b)
    }

    pub fn div(self, other: Dim) -> Dim {
        self.zip(other, |a, b| a - b)
    }

    pub fn pow(self, n: i32) -> Dim {
        self.zip(Dim::ONE, |a, _| a * n)
    }

    /// The square root, when every exponent is even.
    pub fn sqrt(self) -> Option<Dim> {
        if self.0.iter().all(|e| e % 2 == 0) {
            Some(self.zip(Dim::ONE, |a, _| a / 2))
        } else {
            None
        }
    }

    /// How a message names this dimension: its unit, or `dimensionless`.
    pub fn describe(self) -> String {
        if self.is_one() { "dimensionless".to_string() } else { self.to_string() }
    }
}

/// The unit syntax: `kg*m/s^2`, `1/s`, or `1` for a pure number. Parsing the
/// output gives the same dimension back.
impl fmt::Display for Dim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let factors: Vec<(String, i32)> = BASE_UNITS
            .iter()
            .zip(self.0)
            .map(|(n, e)| (n.to_string(), e))
            .collect();
        f.write_str(&canonical(&factors))
    }
}

/// The dimension of one unit name, base or derived.
pub fn unit_named(name: &str) -> Option<Dim> {
    if let Some(i) = BASE_UNITS.iter().position(|b| *b == name) {
        return Some(Dim::base(i));
    }
    let (kg, m, s) = (Dim::base(0), Dim::base(1), Dim::base(2));
    let newton = kg.mul(m).div(s.pow(2));
    Some(match name {
        "N" => newton,
        "J" => newton.mul(m),
        "W" => newton.mul(m).div(s),
        "Pa" => newton.div(m.pow(2)),
        "Hz" => Dim::ONE.div(s),
        _ => return None,
    })
}

/// The canonical spelling of a unit written as `(name, exponent)` factors:
/// repeated names merged, zero exponents dropped, numerator factors in order
/// of first appearance joined by `*`, then each denominator factor after a
/// `/`. Nothing left is `1`.
///
/// The parser stores this spelling, so both printers emit it and reparsing it
/// gives the same tree.
pub fn canonical(factors: &[(String, i32)]) -> String {
    let mut merged: Vec<(String, i32)> = Vec::new();
    for (name, e) in factors {
        match merged.iter_mut().find(|(n, _)| n == name) {
            Some(slot) => slot.1 += e,
            None => merged.push((name.clone(), *e)),
        }
    }
    merged.retain(|(_, e)| *e != 0);
    let term = |n: &str, e: i32| if e == 1 { n.to_string() } else { format!("{n}^{e}") };
    let num: Vec<String> =
        merged.iter().filter(|(_, e)| *e > 0).map(|(n, e)| term(n, *e)).collect();
    let mut out = if num.is_empty() { "1".to_string() } else { num.join("*") };
    for (n, e) in merged.iter().filter(|(_, e)| *e < 0) {
        out.push('/');
        out.push_str(&term(n, -e));
    }
    out
}

/// The dimension a canonical unit spelling stands for. An unknown unit name
/// is an error naming it.
pub fn parse_unit(text: &str) -> Result<Dim, String> {
    let mut dim = Dim::ONE;
    let mut sign = 1;
    let mut rest = text.trim();
    loop {
        let end = rest.find(['*', '/']).unwrap_or(rest.len());
        let term = rest[..end].trim();
        let (name, exp) = match term.split_once('^') {
            Some((n, e)) => (n.trim(), e.trim().parse::<i32>().map_err(|_| {
                format!("`{term}` in unit `{text}` has an exponent that is not an integer")
            })?),
            None => (term, 1),
        };
        if name != "1" {
            let d = unit_named(name).ok_or_else(|| {
                format!(
                    "unknown unit `{name}` in `{text}`: the units are kg, m, s, A, K, mol, cd, N, J, W, Pa and Hz"
                )
            })?;
            dim = dim.mul(d.pow(sign * exp));
        }
        if end == rest.len() {
            break;
        }
        sign = if rest.as_bytes()[end] == b'/' { -1 } else { 1 };
        rest = &rest[end + 1..];
    }
    Ok(dim)
}

// ── The pass ─────────────────────────────────────────────────────────

/// The dimension of a numeric value, as far as the pass can tell.
#[derive(Debug, Clone, Copy, PartialEq)]
enum D {
    Known(Dim),
    /// A literal: adopts the other operand's unit.
    Poly,
    /// Something the pass cannot see into. Never reported.
    Unknown,
}

/// What the pass knows about a value.
#[derive(Debug, Clone, PartialEq)]
enum V {
    Num(D),
    Struct(String),
    Seq(Box<V>),
    Other,
}

impl V {
    fn d(&self) -> D {
        match self {
            V::Num(d) => *d,
            _ => D::Unknown,
        }
    }
}

const KEEP: &[&str] = &["abs", "floor", "ceil", "round", "trunc"];
const PURE_NUMBER: &[&str] = &[
    "exp", "ln", "log", "log2", "log10", "sin", "cos", "tan", "asin", "acos", "atan", "sinh",
    "cosh", "tanh", "sigmoid",
];

/// What the pass found: its diagnostics, and the dimension each function's
/// body was inferred to have, where it knows one.
#[derive(Debug, Default)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub inferred: Vec<(String, Dim)>,
}

impl Report {
    /// The inferred dimension of `name`'s body, in unit syntax.
    pub fn inferred_of(&self, name: &str) -> Option<String> {
        self.inferred.iter().find(|(n, _)| n == name).map(|(_, d)| d.to_string())
    }
}

/// Check the units of a parsed module.
pub fn check(module: &ast::Module) -> Report {
    let mut c = Checker::default();
    c.collect(&module.items);
    for item in &module.items {
        c.item(item, None);
    }
    for (clause, f) in crate::contract::collect(module) {
        if let Some(f) = f {
            c.clause(&clause, f);
        }
    }
    Report { diagnostics: c.diagnostics, inferred: c.inferred }
}

#[derive(Default)]
struct Checker {
    structs: HashMap<String, Vec<(String, Type)>>,
    aliases: HashMap<String, Type>,
    fns: HashMap<String, (Vec<Type>, Option<Type>)>,
    consts: HashMap<String, Type>,
    scopes: Vec<HashMap<String, V>>,
    ret: Vec<(String, V)>,
    diagnostics: Vec<Diagnostic>,
    seen: HashSet<String>,
    inferred: Vec<(String, Dim)>,
}

impl Checker {
    fn report(&mut self, category: DiagnosticCategory, msg: String) {
        if self.seen.insert(msg.clone()) {
            self.diagnostics.push(Diagnostic::categorized(Severity::Error, msg, category, None));
        }
    }

    fn mismatch(&mut self, msg: String) {
        self.report(DiagnosticCategory::UnitMismatch, msg);
    }

    fn collect(&mut self, items: &[ast::Item]) {
        for item in items {
            match &item.kind {
                ItemKind::Struct(s) => {
                    let fields = s.fields.iter().map(|f| (f.name.clone(), f.ty.clone())).collect();
                    self.structs.insert(s.name.clone(), fields);
                }
                ItemKind::TypeAlias(a) => {
                    self.aliases.insert(a.name.clone(), a.ty.clone());
                }
                ItemKind::Function(f) => {
                    let params = f.params.iter().map(|p| p.ty.clone()).collect();
                    self.fns.insert(f.name.clone(), (params, f.return_type.clone()));
                }
                ItemKind::Const(k) => {
                    self.consts.insert(k.name.clone(), k.ty.clone());
                }
                ItemKind::Module(m) => {
                    if let Some(items) = &m.items {
                        self.collect(items);
                    }
                }
                _ => {}
            }
        }
    }

    /// What a declared type says about its values.
    fn of_type(&mut self, ty: &Type) -> V {
        self.of_type_at(ty, 0)
    }

    fn of_type_at(&mut self, ty: &Type, depth: u32) -> V {
        if depth > 16 {
            return V::Other;
        }
        match ty {
            Type::Dimensioned { unit, .. } => match parse_unit(unit) {
                Ok(d) => V::Num(D::Known(d)),
                Err(msg) => {
                    self.report(DiagnosticCategory::UnresolvedType, msg);
                    V::Num(D::Unknown)
                }
            },
            Type::Path { segments, type_args } if segments.len() == 1 => {
                let name = segments[0].as_str();
                match name {
                    "f32" | "f64" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8"
                    | "u16" | "u32" | "u64" | "u128" | "usize"
                        if type_args.is_empty() =>
                    {
                        V::Num(D::Known(Dim::ONE))
                    }
                    _ if self.structs.contains_key(name) => V::Struct(name.to_string()),
                    _ => match self.aliases.get(name).cloned() {
                        Some(t) => self.of_type_at(&t, depth + 1),
                        None => V::Other,
                    },
                }
            }
            Type::Slice { inner } | Type::Vec { inner } | Type::Array { inner, .. } => {
                V::Seq(Box::new(self.of_type_at(inner, depth + 1)))
            }
            Type::Reference { inner, .. } => self.of_type_at(inner, depth + 1),
            Type::Refined { base, .. } => self.of_type_at(base, depth + 1),
            _ => V::Other,
        }
    }

    fn lookup(&self, name: &str) -> Option<V> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn bind(&mut self, name: &str, v: V) {
        if let Some(s) = self.scopes.last_mut() {
            s.insert(name.to_string(), v);
        }
    }

    fn bind_pattern(&mut self, pattern: &Pattern, v: V) {
        match pattern {
            Pattern::Ident { name } => self.bind(name, v),
            Pattern::Ref { pattern } => self.bind_pattern(pattern, v),
            Pattern::Tuple { elements } | Pattern::Enum { elements, .. } => {
                for p in elements {
                    self.bind_pattern(p, V::Other);
                }
            }
            Pattern::Slice { elements, rest_name, .. } => {
                for p in elements {
                    self.bind_pattern(p, V::Other);
                }
                if let Some(n) = rest_name {
                    self.bind(n, V::Other);
                }
            }
            Pattern::Struct { fields, .. } => {
                for f in fields {
                    match &f.pattern {
                        Some(p) => self.bind_pattern(p, V::Other),
                        None => self.bind(&f.name, V::Other),
                    }
                }
            }
            Pattern::Or { patterns } => {
                for p in patterns {
                    self.bind_pattern(p, V::Other);
                }
            }
            Pattern::Literal { .. } | Pattern::Wildcard => {}
        }
    }

    // ── Items ────────────────────────────────────────────────────────

    fn item(&mut self, item: &ast::Item, self_ty: Option<&str>) {
        match &item.kind {
            ItemKind::Function(f) => self.function(f, self_ty),
            ItemKind::Const(k) => {
                let want = self.of_type(&k.ty);
                self.scopes.push(HashMap::new());
                let got = self.expr(&k.value);
                self.scopes.pop();
                if let (D::Known(w), D::Known(g)) = (want.d(), got.d())
                    && w != g
                {
                    self.mismatch(format!(
                        "const `{}` is declared {} but its value is {}",
                        k.name,
                        w.describe(),
                        g.describe()
                    ));
                }
            }
            ItemKind::Impl(b) => {
                let name = head(&b.self_type);
                for i in &b.items {
                    self.item(i, name.as_deref());
                }
            }
            ItemKind::Extend(b) => {
                let name = head(&b.target_type);
                for i in &b.items {
                    self.item(i, name.as_deref());
                }
            }
            ItemKind::Trait(t) => {
                for i in &t.items {
                    self.item(i, None);
                }
            }
            ItemKind::Module(m) => {
                if let Some(items) = &m.items {
                    for i in items {
                        self.item(i, None);
                    }
                }
            }
            _ => {}
        }
    }

    /// One contract clause of `f`. A clause that does not prepare is the
    /// type checker's to report.
    fn clause(&mut self, clause: &crate::contract::Clause, f: &ast::FunctionDef) {
        let params: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
        let Ok(expr) = crate::contract::prepare(clause, Some(&params)) else { return };
        let mut scope = self.signature_scope(f, None);
        if clause.kind == ast::ContractClauseKind::Ensures
            && let Some(t) = &f.return_type
        {
            let v = self.of_type(t);
            scope.insert("result".to_string(), v);
        }
        // `seen` is by message, and the clause's text goes on the message
        // only afterwards: `d + t` in a body must not hide it in a clause.
        let seen = std::mem::take(&mut self.seen);
        let before = self.diagnostics.len();
        self.scopes.push(scope);
        self.expr(&expr);
        self.scopes.pop();
        self.seen = seen;
        for d in &mut self.diagnostics[before..] {
            d.message =
                format!("{} in contract `{}` of `{}`", d.message, clause.display(), clause.function);
        }
    }

    fn signature_scope(&mut self, f: &ast::FunctionDef, self_ty: Option<&str>) -> HashMap<String, V> {
        let mut scope = HashMap::new();
        for (name, ty) in &self.consts.clone() {
            let v = self.of_type(ty);
            scope.insert(name.clone(), v);
        }
        for p in &f.params {
            let v = match (&p.ty, self_ty) {
                (Type::SelfType, Some(s)) if self.structs.contains_key(s) => {
                    V::Struct(s.to_string())
                }
                _ if p.name == "self" => match self_ty {
                    Some(s) if self.structs.contains_key(s) => V::Struct(s.to_string()),
                    _ => V::Other,
                },
                _ => self.of_type(&p.ty),
            };
            scope.insert(p.name.clone(), v);
        }
        scope
    }

    fn function(&mut self, f: &ast::FunctionDef, self_ty: Option<&str>) {
        let scope = self.signature_scope(f, self_ty);
        let want = match &f.return_type {
            Some(t) => self.of_type(t),
            None => V::Other,
        };
        self.scopes.push(scope);
        self.ret.push((f.name.clone(), want.clone()));
        let got = match &f.body_expr {
            Some(e) => self.expr(e),
            None => self.block(&f.body),
        };
        self.ret.pop();
        self.scopes.pop();
        self.check_return(&f.name, &want, &got);
        if let D::Known(d) = got.d() {
            self.inferred.push((f.name.clone(), d));
        }
    }

    fn check_return(&mut self, name: &str, want: &V, got: &V) {
        if let (D::Known(w), D::Known(g)) = (want.d(), got.d())
            && w != g
        {
            self.mismatch(format!(
                "`{name}` returns {}, but is declared to return {}",
                g.describe(),
                w.describe()
            ));
        }
    }

    // ── Blocks and statements ────────────────────────────────────────

    fn block(&mut self, b: &Block) -> V {
        self.scopes.push(HashMap::new());
        for s in &b.stmts {
            self.stmt(s);
        }
        let v = match &b.tail_expr {
            Some(e) => self.expr(e),
            None => V::Other,
        };
        self.scopes.pop();
        v
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Let { pattern, ty, value, .. } => {
                let got = self.expr(value);
                let v = match ty {
                    Some(t) => {
                        let want = self.of_type(t);
                        if let (D::Known(w), D::Known(g)) = (want.d(), got.d())
                            && w != g
                        {
                            let who = match pattern {
                                Pattern::Ident { name } => format!("`{name}`"),
                                _ => "a binding".to_string(),
                            };
                            self.mismatch(format!(
                                "{who} is declared {} but given {}",
                                w.describe(),
                                g.describe()
                            ));
                        }
                        if want == V::Other { got } else { want }
                    }
                    None => got,
                };
                self.bind_pattern(pattern, v);
            }
            Stmt::Expr { expr } | Stmt::Defer { expr } => {
                self.expr(expr);
            }
            Stmt::Guard { cond, else_block } => {
                self.expr(cond);
                self.block(else_block);
            }
            Stmt::Item { item } => {
                if let ItemKind::Function(f) = &item.kind {
                    let params = f.params.iter().map(|p| p.ty.clone()).collect();
                    self.fns.insert(f.name.clone(), (params, f.return_type.clone()));
                }
                self.item(item, None);
            }
        }
    }

    // ── Expressions ──────────────────────────────────────────────────

    /// Two dimensions that must be equal; `verb` says what was being done.
    fn same(&mut self, verb: &str, a: D, b: D) -> D {
        match (a, b) {
            (D::Known(x), D::Known(y)) => {
                if x != y {
                    self.mismatch(format!("cannot {verb} {} and {}", x.describe(), y.describe()));
                }
                D::Known(x)
            }
            (D::Known(x), _) | (_, D::Known(x)) => D::Known(x),
            (D::Poly, D::Poly) => D::Poly,
            _ => D::Unknown,
        }
    }

    fn mul(a: D, b: D, divide: bool) -> D {
        match (a, b) {
            (D::Known(x), D::Known(y)) => D::Known(if divide { x.div(y) } else { x.mul(y) }),
            (D::Known(x), D::Poly) => D::Known(x),
            (D::Poly, D::Known(y)) => D::Known(if divide { Dim::ONE.div(y) } else { y }),
            (D::Poly, D::Poly) => D::Poly,
            _ => D::Unknown,
        }
    }

    fn binary(&mut self, op: &str, l: V, r: V) -> V {
        let (a, b) = (l.d(), r.d());
        match op {
            "+" => V::Num(self.same("add", a, b)),
            "-" => V::Num(self.same("subtract", a, b)),
            "%" => V::Num(self.same("take the remainder of", a, b)),
            "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                self.same("compare", a, b);
                V::Other
            }
            "*" => V::Num(Self::mul(a, b, false)),
            "/" => V::Num(Self::mul(a, b, true)),
            "+=" | "-=" => {
                self.same(if op == "+=" { "add" } else { "subtract" }, a, b);
                l
            }
            "*=" | "/=" => {
                if let (D::Known(x), D::Known(y)) = (a, b)
                    && !y.is_one()
                {
                    self.mismatch(format!(
                        "`{op}` by {} would change the unit of a {} value; write `x = x {} y`",
                        y.describe(),
                        x.describe(),
                        &op[..1]
                    ));
                }
                l
            }
            "%=" => {
                self.same("take the remainder of", a, b);
                l
            }
            _ => V::Other,
        }
    }

    /// The dimension-checked result of a builtin numeric function, or `None`
    /// when `name` is not one.
    fn builtin(&mut self, name: &str, args: &[V]) -> Option<V> {
        let first = args.first().map(V::d).unwrap_or(D::Unknown);
        if name == "sqrt" && args.len() == 1 {
            return Some(V::Num(match first {
                D::Known(d) => match d.sqrt() {
                    Some(r) => D::Known(r),
                    None => {
                        self.mismatch(format!(
                            "cannot take the square root of {}: its exponents are not all even",
                            d.describe()
                        ));
                        D::Unknown
                    }
                },
                other => other,
            }));
        }
        if KEEP.contains(&name) && args.len() == 1 {
            return Some(V::Num(first));
        }
        if (name == "min" || name == "max") && args.len() == 2 {
            let verb = format!("take the {name} of");
            return Some(V::Num(self.same(&verb, first, args[1].d())));
        }
        if PURE_NUMBER.contains(&name) && args.len() == 1 {
            if let D::Known(d) = first
                && !d.is_one()
            {
                self.mismatch(format!(
                    "`{name}` needs a dimensionless argument, but was given {}",
                    d.describe()
                ));
            }
            return Some(V::Num(D::Known(Dim::ONE)));
        }
        None
    }

    fn call(&mut self, name: &str, args: &[V]) -> V {
        if let Some((params, ret)) = self.fns.get(name).cloned() {
            for (i, (arg, p)) in args.iter().zip(&params).enumerate() {
                let want = self.of_type(p);
                if let (D::Known(w), D::Known(g)) = (want.d(), arg.d())
                    && w != g
                {
                    self.mismatch(format!(
                        "argument {} of `{name}` is {}, but the parameter is {}",
                        i + 1,
                        g.describe(),
                        w.describe()
                    ));
                }
            }
            return match ret {
                Some(t) => self.of_type(&t),
                None => V::Other,
            };
        }
        self.builtin(name, args).unwrap_or(V::Num(D::Unknown))
    }

    fn join(&mut self, what: &str, vs: Vec<V>) -> V {
        let mut known: Option<(Dim, V)> = None;
        for v in &vs {
            if let D::Known(d) = v.d() {
                match &known {
                    Some((k, _)) if *k != d => {
                        self.mismatch(format!(
                            "the branches of {what} are {} and {}",
                            k.describe(),
                            d.describe()
                        ));
                    }
                    Some(_) => {}
                    None => known = Some((d, v.clone())),
                }
            }
        }
        match known {
            Some((_, v)) => v,
            None => vs.into_iter().next().unwrap_or(V::Other),
        }
    }

    fn expr(&mut self, e: &Expr) -> V {
        match e {
            Expr::Literal { kind: LiteralKind::Int | LiteralKind::Float, .. } => V::Num(D::Poly),
            Expr::Literal { .. } => V::Other,
            Expr::Ident { name } => self.lookup(name).unwrap_or(V::Other),
            Expr::Binary { op, left, right } => {
                let l = self.expr(left);
                let r = self.expr(right);
                self.binary(op, l, r)
            }
            Expr::Unary { op, operand } => {
                let v = self.expr(operand);
                if op == "!" { V::Other } else { v }
            }
            Expr::Call { func, args } => {
                let vs: Vec<V> = args.iter().map(|a| self.expr(a)).collect();
                match func.as_ref() {
                    Expr::Ident { name } if self.lookup(name).is_none() => self.call(name, &vs),
                    other => {
                        self.expr(other);
                        V::Num(D::Unknown)
                    }
                }
            }
            Expr::MethodCall { receiver, method, args, .. } => {
                let recv = self.expr(receiver);
                let mut vs = vec![recv.clone()];
                vs.extend(args.iter().map(|a| self.expr(a)));
                match recv {
                    V::Num(_) => self.builtin(method, &vs).unwrap_or(V::Num(D::Unknown)),
                    _ => V::Num(D::Unknown),
                }
            }
            Expr::FieldAccess { object, field } => match self.expr(object) {
                V::Struct(s) => {
                    let ty = self
                        .structs
                        .get(&s)
                        .and_then(|fs| fs.iter().find(|(n, _)| n == field))
                        .map(|(_, t)| t.clone());
                    match ty {
                        Some(t) => self.of_type(&t),
                        None => V::Other,
                    }
                }
                _ => V::Other,
            },
            Expr::Index { object, index } => {
                let o = self.expr(object);
                self.expr(index);
                match o {
                    V::Seq(inner) => *inner,
                    _ => V::Other,
                }
            }
            Expr::StructLit { path, fields } => {
                let name = path.last().cloned().unwrap_or_default();
                let decl = self.structs.get(&name).cloned();
                for f in fields {
                    let got = match &f.value {
                        Some(v) => self.expr(v),
                        None => self.lookup(&f.name).unwrap_or(V::Other),
                    };
                    let ty = decl
                        .as_ref()
                        .and_then(|fs| fs.iter().find(|(n, _)| *n == f.name))
                        .map(|(_, t)| t.clone());
                    if let Some(t) = ty {
                        let want = self.of_type(&t);
                        if let (D::Known(w), D::Known(g)) = (want.d(), got.d())
                            && w != g
                        {
                            self.mismatch(format!(
                                "field `{}` of `{name}` is {}, but was given {}",
                                f.name,
                                w.describe(),
                                g.describe()
                            ));
                        }
                    }
                }
                if decl.is_some() { V::Struct(name) } else { V::Other }
            }
            Expr::TupleLit { elements } => {
                for x in elements {
                    self.expr(x);
                }
                V::Other
            }
            Expr::ArrayLit { elements } => {
                let vs: Vec<V> = elements.iter().map(|x| self.expr(x)).collect();
                V::Seq(Box::new(vs.into_iter().next().unwrap_or(V::Num(D::Unknown))))
            }
            Expr::ArrayRepeat { value, count } => {
                let v = self.expr(value);
                self.expr(count);
                V::Seq(Box::new(v))
            }
            Expr::MapLit { entries } => {
                for (k, v) in entries {
                    self.expr(k);
                    self.expr(v);
                }
                V::Other
            }
            Expr::Closure { params, body } => {
                let mut scope = HashMap::new();
                for p in params {
                    let v = match p.ty {
                        Type::Inferred => V::Num(D::Unknown),
                        _ => self.of_type(&p.ty),
                    };
                    scope.insert(p.name.clone(), v);
                }
                self.scopes.push(scope);
                // A closure's `ret` is its own: do not check it against the
                // enclosing function's return.
                self.ret.push((String::new(), V::Other));
                self.expr(body);
                self.ret.pop();
                self.scopes.pop();
                V::Other
            }
            Expr::If { cond, then_block, else_block } => {
                self.expr(cond);
                let t = self.block(then_block);
                match else_block {
                    Some(b) => {
                        let f = self.block(b);
                        self.join("an `if`", vec![t, f])
                    }
                    None => V::Other,
                }
            }
            Expr::Match { scrutinee, arms } => {
                if let Some(s) = scrutinee {
                    self.expr(s);
                }
                let mut vs = Vec::new();
                for arm in arms {
                    self.scopes.push(HashMap::new());
                    self.bind_pattern(&arm.pattern, V::Other);
                    vs.push(self.expr(&arm.body));
                    self.scopes.pop();
                }
                self.join("a `match`", vs)
            }
            Expr::Handle { body, arms, .. } => {
                self.block(body);
                for arm in arms {
                    self.scopes.push(HashMap::new());
                    for p in &arm.params {
                        self.bind(p, V::Other);
                    }
                    self.expr(&arm.body);
                    self.scopes.pop();
                }
                V::Other
            }
            Expr::Loop { body } => {
                self.block(body);
                V::Other
            }
            Expr::While { cond, body } => {
                self.expr(cond);
                self.block(body);
                V::Other
            }
            Expr::For { pattern, iter, body } => {
                let it = self.expr(iter);
                self.scopes.push(HashMap::new());
                let elem = match it {
                    V::Seq(inner) => *inner,
                    _ => V::Other,
                };
                self.bind_pattern(pattern, elem);
                self.block(body);
                self.scopes.pop();
                V::Other
            }
            Expr::Block { block } => self.block(block),
            Expr::Return { value } => {
                if let Some(v) = value {
                    let got = self.expr(v);
                    if let Some((name, want)) = self.ret.last().cloned()
                        && !name.is_empty()
                    {
                        self.check_return(&name, &want, &got);
                    }
                }
                V::Num(D::Unknown)
            }
            Expr::Break { value } => {
                if let Some(v) = value {
                    self.expr(v);
                }
                V::Num(D::Unknown)
            }
            Expr::Continue => V::Num(D::Unknown),
            Expr::Try { expr } | Expr::Await { expr } => {
                self.expr(expr);
                V::Num(D::Unknown)
            }
            Expr::Cast { expr, ty } => {
                self.expr(expr);
                self.of_type(ty)
            }
            Expr::Assign { target, value } => {
                let t = self.expr(target);
                let v = self.expr(value);
                if let (D::Known(w), D::Known(g)) = (t.d(), v.d())
                    && w != g
                {
                    self.mismatch(format!(
                        "cannot assign {} to a {} place",
                        g.describe(),
                        w.describe()
                    ));
                }
                V::Other
            }
            Expr::Range { start, end, .. } => {
                let s = self.expr(start);
                let e = self.expr(end);
                let d = self.same("make a range from", s.d(), e.d());
                V::Seq(Box::new(V::Num(d)))
            }
            Expr::Pipeline { left, right } => {
                self.expr(left);
                self.expr(right);
                V::Num(D::Unknown)
            }
            Expr::Grad { value, wrt } => {
                self.expr(value);
                self.expr(wrt);
                V::Num(D::Unknown)
            }
            Expr::Is { expr, .. } => {
                self.expr(expr);
                V::Other
            }
            Expr::Todo | Expr::Unimplemented | Expr::Error { .. } => V::Num(D::Unknown),
        }
    }
}

/// The name a type is known by, for `impl` and `extend` targets.
fn head(t: &Type) -> Option<String> {
    match t {
        Type::Path { segments, .. } => segments.last().cloned(),
        _ => None,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fmt as mfmt, lexer, parser};

    fn print(m: &ast::Module, mode: mfmt::Mode) -> String {
        match mode {
            mfmt::Mode::Agent => mfmt::format_agent(m),
            mfmt::Mode::Human => mfmt::format_human(m),
        }
    }

    fn module(src: &str) -> ast::Module {
        parser::parse(&lexer::lex(src)).expect("parse failed")
    }

    /// Every diagnostic from the full type checker, as `(code, message)`.
    fn errors(src: &str) -> Vec<(String, String)> {
        crate::types::check(&module(src))
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| {
                (d.category.map(|c| c.code()).unwrap_or("").to_string(), d.message.clone())
            })
            .collect()
    }

    fn unit_errors(src: &str) -> Vec<String> {
        errors(src).into_iter().filter(|(c, _)| c == "E0562").map(|(_, m)| m).collect()
    }

    #[test]
    fn units_parse_to_exponent_vectors() {
        assert_eq!(parse_unit("m/s").unwrap().to_string(), "m/s");
        assert_eq!(parse_unit("kg*m/s^2").unwrap(), parse_unit("N").unwrap());
        assert_eq!(parse_unit("J").unwrap().to_string(), "kg*m^2/s^2");
        assert_eq!(parse_unit("1/s").unwrap(), parse_unit("Hz").unwrap());
        assert_eq!(parse_unit("1").unwrap(), Dim::ONE);
        assert_eq!(parse_unit("m/m").unwrap(), Dim::ONE);
        assert!(parse_unit("ft").unwrap_err().contains("unknown unit `ft`"));
    }

    #[test]
    fn the_canonical_spelling_merges_and_orders_factors() {
        let f = |v: &[(&str, i32)]| {
            canonical(&v.iter().map(|(n, e)| (n.to_string(), *e)).collect::<Vec<_>>())
        };
        assert_eq!(f(&[("m", 1), ("s", -1)]), "m/s");
        assert_eq!(f(&[("s", -1), ("m", 1)]), "m/s");
        assert_eq!(f(&[("s", -2)]), "1/s^2");
        assert_eq!(f(&[("m", 1), ("m", 1), ("kg", 1)]), "m^2*kg");
        assert_eq!(f(&[("m", 1), ("m", -1)]), "1");
    }

    /// An annotated type prints the same in both surfaces, and reparses to it.
    #[test]
    fn annotated_types_round_trip_through_both_printers() {
        let src = "f v(d: f64[m], t: f64[s]) -> f64[m/s] { d / t }\n\
                   f a(x: f32[kg*m/s^2], y: [f64[N]]~) -> f64[1/s] { 1.0 }\n\
                   f w(x: f64[s^-1*m]) -> f64[m/s] { x }\n";
        for (name, mode) in [("compact", mfmt::Mode::Agent), ("expand", mfmt::Mode::Human)] {
            let once = print(&module(src), mode);
            for want in ["f64[m]", "f64[m/s]", "f32[kg*m/s^2]", "[f64[N]]~", "f64[1/s]"] {
                assert!(once.contains(want), "{name} lost `{want}`:\n{once}");
            }
            // `s^-1*m` is printed in its canonical spelling.
            assert!(!once.contains("s^-1"), "{once}");
            let twice = print(&module(&once), mode);
            assert_eq!(once, twice, "{name} printing is not a fixed point");
        }
    }

    #[test]
    fn a_clause_is_checked_for_units() {
        let src = "@req(d + t > 0.0)\nf bad(d: f64[m], t: f64[s]) -> f64[m] { d }";
        assert_eq!(
            unit_errors(src),
            ["cannot add m and s in contract `@req(d + t > 0.0)` of `bad`"]
        );
        // In a spec too, with `result` carrying the return dimension.
        let src = "sp bad {\n    @ens(result > t)\n}\nf bad(d: f64[m], t: f64[s]) -> f64[m] { d }";
        assert_eq!(unit_errors(src).len(), 1, "{:?}", unit_errors(src));
        // `old(x)` is the parameter, with its dimension.
        let src = "@ens(result == old(d))\nf ok(d: f64[m]) -> f64[m] { d }";
        assert!(unit_errors(src).is_empty());
    }

    /// `seen` must not let a mismatch in the body hide the same one in a clause.
    #[test]
    fn the_same_mismatch_in_a_clause_and_the_body_is_reported_twice() {
        let src = "@req(d + t > 0.0)\nf bad(d: f64[m], t: f64[s]) -> f64[m] { d + t }";
        assert_eq!(unit_errors(src).len(), 2, "{:?}", unit_errors(src));
    }

    #[test]
    fn adding_metres_to_seconds_is_refused() {
        let src = "f bad(d: f64[m], t: f64[s]) -> f64[m] { d + t }";
        // Exactly one error, with the new code: the base types agree, so the
        // ordinary checker has nothing to say.
        assert_eq!(
            errors(src),
            [("E0562".to_string(), "cannot add m and s".to_string())]
        );
        // Subtraction too, with its own verb.
        let src = "f bad(d: f64[m], t: f64[s]) -> f64[m] { d - t }";
        assert_eq!(unit_errors(src), ["cannot subtract m and s"]);
    }

    #[test]
    fn dividing_metres_by_seconds_is_metres_per_second() {
        let src = "f speed(d: f64[m], t: f64[s]) -> f64[m/s] { d / t }";
        assert!(errors(src).is_empty(), "{:?}", errors(src));
        let report = check(&module(src));
        assert_eq!(report.inferred_of("speed").as_deref(), Some("m/s"));
        // Declared as anything else, the inferred unit is named.
        let src = "f speed(d: f64[m], t: f64[s]) -> f64[m] { d / t }";
        assert_eq!(unit_errors(src), ["`speed` returns m/s, but is declared to return m"]);
    }

    #[test]
    fn multiplication_adds_exponents() {
        let src = "f force(mass: f64[kg], a: f64[m/s^2]) -> f64[N] { mass * a }\n\
                   f area(x: f64[m], y: f64[m]) -> f64[m^2] { x * y }\n\
                   f work(push: f64[N], d: f64[m]) -> f64[J] { push * d }\n\
                   f rate(t: f64[s]) -> f64[Hz] { 1.0 / t }\n\
                   f root(a: f64[m^2]) -> f64[m] { sqrt(a) }\n";
        assert!(errors(src).is_empty(), "{:?}", errors(src));
        let report = check(&module(src));
        assert_eq!(report.inferred_of("force").as_deref(), Some("kg*m/s^2"));
        assert_eq!(report.inferred_of("area").as_deref(), Some("m^2"));
        assert_eq!(report.inferred_of("work").as_deref(), Some("kg*m^2/s^2"));
        assert_eq!(report.inferred_of("rate").as_deref(), Some("1/s"));
        let src = "f bad(x: f64[m]) -> f64 { sqrt(x) }";
        assert_eq!(
            unit_errors(src),
            ["cannot take the square root of m: its exponents are not all even"]
        );
    }

    /// A literal adopts the other operand's unit under `+`, `-` and the
    /// comparisons, and is a plain scalar under `*` and `/`.
    #[test]
    fn literals_are_polymorphic() {
        let src = "f a(x: f64[m]) -> f64[m] { x + 1.0 }\n\
                   f b(x: f64[m]) -> f64[m] { 2.0 * x - 0.5 }\n\
                   f c(x: f64[m]) -> bool { x > 0.0 }\n\
                   f d(x: f64[s]) -> f64[1/s] { 1.0 / x }\n\
                   f lit() -> f64[m] { val g = 9.0; g }\n\
                   f k() -> f64[m] { 3.0 }\n";
        assert!(errors(src).is_empty(), "{:?}", errors(src));
        // An annotated binding is no longer polymorphic.
        let src = "f bad(x: f64[m]) -> f64[m] { val g: f64[s] = 9.0; x + g }";
        assert_eq!(unit_errors(src), ["cannot add m and s"]);
        // A bare `f64` is dimensionless, not polymorphic.
        let src = "f bad(x: f64[m], y: f64) -> f64[m] { x + y }";
        assert_eq!(unit_errors(src), ["cannot add m and dimensionless"]);
    }

    #[test]
    fn a_call_checks_argument_units() {
        let src = "f speed(d: f64[m], t: f64[s]) -> f64[m/s] { d / t }\n\
                   f good(x: f64[m], y: f64[s]) -> f64[m/s] { speed(x, y) }\n";
        assert!(errors(src).is_empty(), "{:?}", errors(src));
        let src = "f speed(d: f64[m], t: f64[s]) -> f64[m/s] { d / t }\n\
                   f bad(x: f64[m], y: f64[s]) -> f64[m/s] { speed(y, x) }\n";
        assert_eq!(
            unit_errors(src),
            [
                "argument 1 of `speed` is s, but the parameter is m",
                "argument 2 of `speed` is m, but the parameter is s",
            ]
        );
        // The return carries its unit to the caller.
        let src = "f speed(d: f64[m], t: f64[s]) -> f64[m/s] { d / t }\n\
                   f bad(x: f64[m], y: f64[s]) -> f64[m] { x + speed(x, y) }\n";
        assert_eq!(unit_errors(src), ["cannot add m and m/s"]);
    }

    #[test]
    fn comparing_mismatched_units_is_refused() {
        let src = "f bad(d: f64[m], t: f64[s]) -> bool { d < t }";
        assert_eq!(unit_errors(src), ["cannot compare m and s"]);
        let src = "f ok(d: f64[m], e: f64[m]) -> bool { d == e }";
        assert!(errors(src).is_empty());
    }

    #[test]
    fn struct_fields_bindings_and_assignments_carry_units() {
        let src = "S State { x: f64[m], v: f64[m/s] }\n\
                   f step(st: State, dt: f64[s]) -> State {\n\
                       var x = st.x;\n\
                       x = x + st.v * dt;\n\
                       @State { x: x, v: st.v }\n\
                   }\n";
        assert!(errors(src).is_empty(), "{:?}", errors(src));
        let src = "S State { x: f64[m], v: f64[m/s] }\n\
                   f step(st: State, dt: f64[s]) -> State {\n\
                       var x = st.x;\n\
                       x = x + st.v;\n\
                       @State { x: st.v, v: st.v }\n\
                   }\n";
        assert_eq!(
            unit_errors(src),
            ["cannot add m and m/s", "field `x` of `State` is m, but was given m/s"]
        );
        let src = "f bad(d: f64[m], t: f64[s]) -> f64[m] { var x = d; x += t; x *= t; x }";
        assert_eq!(
            unit_errors(src),
            [
                "cannot add m and s",
                "`*=` by s would change the unit of a m value; write `x = x * y`",
            ]
        );
    }

    #[test]
    fn builtins_and_casts() {
        let src = "f a(x: f64[m], y: f64[m]) -> f64[m] { abs(x) + x.max(y) + min(x, y) }\n\
                   f b(x: f64[m], l: f64[m]) -> f64 { exp(x / l) }\n\
                   f c(x: f64[m]) -> f64 { x as f64 }\n";
        assert!(errors(src).is_empty(), "{:?}", errors(src));
        let src = "f bad(x: f64[m]) -> f64 { exp(x) }";
        assert_eq!(unit_errors(src), ["`exp` needs a dimensionless argument, but was given m"]);
        let src = "f bad(x: f64[m], t: f64[s]) -> f64[m] { x.max(t) }";
        assert_eq!(unit_errors(src), ["cannot take the max of m and s"]);
    }

    #[test]
    fn an_unknown_unit_is_an_unresolved_type() {
        let errs = errors("f bad(x: f64[ft]) -> f64[ft] { x }");
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert_eq!(errs[0].0, "E0412");
        assert!(errs[0].1.starts_with("unknown unit `ft`"), "{errs:?}");
    }

    /// Units are erased: the evaluator runs an annotated program exactly as
    /// it runs the unannotated one.
    #[test]
    fn units_are_erased_at_run_time() {
        let src = "f speed(d: f64[m], t: f64[s]) -> f64[m/s] { d / t }\n\
                   f main() -> f64 { speed(10.0, 4.0) as f64 }\n";
        let v = crate::eval::run_source(src, "main", &[]).expect("runs");
        assert!(matches!(v, crate::eval::Value::Float(x) if x == 2.5), "{v:?}");
    }
}
