//! A focused tree-walking evaluator that RUNS the §8 standard vocabulary and the
//! arithmetic / control flow around it, so vocabulary programs compute real
//! results — the combinators' runtime. Pure subset (data, functions, the
//! vocabulary); IO/structs/traits are out of scope and report an honest error.

use crate::ast::{Block, Expr, FunctionDef, ItemKind, LiteralKind, Module, Pattern, Stmt};
use std::collections::HashMap;
use std::rc::Rc;

/// A runtime value.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Tuple(Vec<Value>),
    /// Optional (the result of `first`/`last`/`find`/`reduce`).
    Opt(Option<Box<Value>>),
    /// A named function (user-defined or a builtin) usable as a value.
    Func(String),
    /// A closure: parameter names, body, and captured environment.
    Closure(Rc<ClosureData>),
    Unit,
}

#[derive(Debug, Clone)]
pub struct ClosureData {
    params: Vec<String>,
    body: Expr,
    env: HashMap<String, Value>,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s:?}"),
            Value::List(xs) => {
                let parts: Vec<String> = xs.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Value::Tuple(xs) => {
                let parts: Vec<String> = xs.iter().map(|v| v.to_string()).collect();
                write!(f, "({})", parts.join(", "))
            }
            Value::Map(m) => {
                let parts: Vec<String> = m.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
            Value::Opt(Some(v)) => write!(f, "Some({v})"),
            Value::Opt(None) => write!(f, "None"),
            Value::Func(n) => write!(f, "<fn {n}>"),
            Value::Closure(_) => write!(f, "<closure>"),
            Value::Unit => write!(f, "()"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value::*;
        match (self, other) {
            (Int(a), Int(b)) => a == b,
            (Float(a), Float(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (List(a), List(b)) => a == b,
            (Map(a), Map(b)) => a == b,
            (Tuple(a), Tuple(b)) => a == b,
            (Opt(a), Opt(b)) => a == b,
            (Func(a), Func(b)) => a == b,
            (Unit, Unit) => true,
            // Closures (and mismatched variants) are not comparable.
            _ => false,
        }
    }
}

/// Non-local control flow during evaluation.
enum Control {
    Return(Value),
    Break(Value),
    Continue,
    Err(String),
}
type R = Result<Value, Control>;

fn err<T>(m: impl Into<String>) -> Result<T, Control> {
    Err(Control::Err(m.into()))
}

/// Lexical environment: a stack of scopes.
struct Env {
    scopes: Vec<HashMap<String, Value>>,
}
impl Env {
    fn new() -> Self {
        Env { scopes: vec![HashMap::new()] }
    }
    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn pop(&mut self) {
        self.scopes.pop();
    }
    fn get(&self, name: &str) -> Option<Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }
    /// Define a binding in the current scope.
    fn define(&mut self, name: String, v: Value) {
        self.scopes.last_mut().unwrap().insert(name, v);
    }
    /// Assign to an existing binding (or define in the current scope if new).
    fn assign(&mut self, name: &str, v: Value) {
        for s in self.scopes.iter_mut().rev() {
            if s.contains_key(name) {
                s.insert(name.to_string(), v);
                return;
            }
        }
        self.define(name.to_string(), v);
    }
    fn flatten(&self) -> HashMap<String, Value> {
        let mut out = HashMap::new();
        for s in &self.scopes {
            for (k, v) in s {
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }
}

pub struct Interp {
    funcs: HashMap<String, FunctionDef>,
}

impl Interp {
    pub fn new(module: &Module) -> Self {
        let mut funcs = HashMap::new();
        for item in &module.items {
            if let ItemKind::Function(fd) = &item.kind {
                funcs.insert(fd.name.clone(), fd.clone());
            }
        }
        Interp { funcs }
    }

    /// Run `name(args)` and return its value, or a human-readable error.
    pub fn run(&self, name: &str, args: Vec<Value>) -> Result<Value, String> {
        let fd = self.funcs.get(name).ok_or_else(|| format!("no function `{name}`"))?;
        match self.call_user(fd, args) {
            Ok(v) => Ok(v),
            Err(Control::Err(e)) => Err(e),
            Err(_) => Err("unexpected control flow at top level".to_string()),
        }
    }

    fn call_user(&self, fd: &FunctionDef, args: Vec<Value>) -> R {
        let mut env = Env::new();
        for (p, v) in fd.params.iter().zip(args.into_iter()) {
            env.define(p.name.clone(), v);
        }
        if let Some(be) = &fd.body_expr {
            return self.eval(be, &mut env);
        }
        match self.eval_block(&fd.body, &mut env) {
            Err(Control::Return(v)) => Ok(v),
            other => other,
        }
    }

    fn eval_block(&self, b: &Block, env: &mut Env) -> R {
        env.push();
        let mut out = Value::Unit;
        for s in &b.stmts {
            match s {
                Stmt::Let { pattern, value, .. } => {
                    let v = self.eval(value, env)?;
                    if let Pattern::Ident { name } = pattern {
                        env.define(name.clone(), v);
                    }
                }
                Stmt::Expr { expr } => {
                    out = self.eval(expr, env)?;
                }
                _ => {}
            }
        }
        if let Some(te) = &b.tail_expr {
            out = self.eval(te, env)?;
        }
        env.pop();
        Ok(out)
    }

    fn eval(&self, e: &Expr, env: &mut Env) -> R {
        match e {
            Expr::Literal { value, kind } => parse_literal(value, kind),
            Expr::Ident { name } => Ok(env.get(name).unwrap_or_else(|| Value::Func(name.clone()))),
            Expr::Unary { op, operand } => {
                let v = self.eval(operand, env)?;
                match (op.as_str(), v) {
                    ("-", Value::Int(n)) => Ok(Value::Int(-n)),
                    ("-", Value::Float(f)) => Ok(Value::Float(-f)),
                    ("!", Value::Bool(b)) => Ok(Value::Bool(!b)),
                    (o, _) => err(format!("unsupported unary `{o}`")),
                }
            }
            Expr::Binary { op, left, right } => {
                if op == "&&" || op == "||" {
                    let l = truthy(&self.eval(left, env)?);
                    return Ok(Value::Bool(if op == "&&" {
                        l && truthy(&self.eval(right, env)?)
                    } else {
                        l || truthy(&self.eval(right, env)?)
                    }));
                }
                let l = self.eval(left, env)?;
                let r = self.eval(right, env)?;
                binop(op, l, r)
            }
            Expr::If { cond, then_block, else_block } => {
                if truthy(&self.eval(cond, env)?) {
                    self.eval_block(then_block, env)
                } else if let Some(eb) = else_block {
                    self.eval_block(eb, env)
                } else {
                    Ok(Value::Unit)
                }
            }
            Expr::Block { block } => self.eval_block(block, env),
            Expr::ArrayLit { elements } => {
                let mut v = Vec::with_capacity(elements.len());
                for el in elements {
                    v.push(self.eval(el, env)?);
                }
                Ok(Value::List(v))
            }
            Expr::TupleLit { elements } => {
                let mut v = Vec::with_capacity(elements.len());
                for el in elements {
                    v.push(self.eval(el, env)?);
                }
                Ok(Value::Tuple(v))
            }
            Expr::MapLit { entries } => {
                let mut m = Vec::new();
                for (k, val) in entries {
                    m.push((self.eval(k, env)?, self.eval(val, env)?));
                }
                Ok(Value::Map(m))
            }
            Expr::Index { object, index } => {
                let o = self.eval(object, env)?;
                let i = self.eval(index, env)?;
                match (o, i) {
                    (Value::List(xs), Value::Int(n)) => xs
                        .get(n as usize)
                        .cloned()
                        .ok_or(Control::Err("index out of bounds".into())),
                    (Value::Map(m), key) => Ok(m
                        .iter()
                        .find(|(k, _)| *k == key)
                        .map(|(_, v)| v.clone())
                        .unwrap_or(Value::Unit)),
                    _ => err("cannot index this value"),
                }
            }
            Expr::Range { start, end, inclusive } => {
                let s = as_int(&self.eval(start, env)?)?;
                let e = as_int(&self.eval(end, env)?)?;
                let hi = if *inclusive { e + 1 } else { e };
                Ok(Value::List((s..hi).map(Value::Int).collect()))
            }
            Expr::Closure { params, body } => Ok(Value::Closure(Rc::new(ClosureData {
                params: params.iter().map(|p| p.name.clone()).collect(),
                body: (**body).clone(),
                env: env.flatten(),
            }))),
            Expr::Assign { target, value } => {
                let v = self.eval(value, env)?;
                if let Expr::Ident { name } = target.as_ref() {
                    env.assign(name, v);
                    Ok(Value::Unit)
                } else {
                    err("only simple `x = ...` assignment is supported")
                }
            }
            Expr::For { pattern, iter, body } => {
                let seq = self.eval(iter, env)?;
                let items = as_list(&seq)?;
                let var = if let Pattern::Ident { name } = pattern { name.clone() } else { "_".into() };
                for it in items {
                    env.push();
                    env.define(var.clone(), it);
                    let r = self.eval_block(body, env);
                    env.pop();
                    match r {
                        Ok(_) => {}
                        Err(Control::Continue) => {}
                        Err(Control::Break(_)) => break,
                        Err(other) => return Err(other),
                    }
                }
                Ok(Value::Unit)
            }
            Expr::While { cond, body } => {
                while truthy(&self.eval(cond, env)?) {
                    match self.eval_block(body, env) {
                        Ok(_) | Err(Control::Continue) => {}
                        Err(Control::Break(_)) => break,
                        Err(other) => return Err(other),
                    }
                }
                Ok(Value::Unit)
            }
            Expr::Return { value } => {
                let v = match value {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Unit,
                };
                Err(Control::Return(v))
            }
            Expr::Break { value } => {
                let v = match value {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Unit,
                };
                Err(Control::Break(v))
            }
            Expr::Continue => Err(Control::Continue),
            Expr::Pipeline { left, right } => {
                // `x |> f(a, b)`  ==  `f(x, a, b)`.
                if let Expr::Call { func, args } = right.as_ref() {
                    let mut all = Vec::with_capacity(args.len() + 1);
                    all.push((**left).clone());
                    all.extend(args.iter().cloned());
                    self.eval_call(func, &all, env)
                } else {
                    let f = self.eval(right, env)?;
                    let x = self.eval(left, env)?;
                    self.apply(&f, vec![x])
                }
            }
            Expr::Call { func, args } => {
                let argv: Vec<Expr> = args.clone();
                self.eval_call(func, &argv, env)
            }
            other => err(format!("evaluator does not support {} yet", variant(other))),
        }
    }

    fn eval_call(&self, func: &Expr, args: &[Expr], env: &mut Env) -> R {
        // Evaluate args.
        let mut av = Vec::with_capacity(args.len());
        for a in args {
            av.push(self.eval(a, env)?);
        }
        if let Expr::Ident { name } = func {
            // User function?
            if let Some(fd) = self.funcs.get(name) {
                return self.call_user(fd, av);
            }
            // Builtin / vocabulary?
            return self.call_builtin(name, av);
        }
        // Indirect: evaluate the callee to a function value.
        let f = self.eval(func, env)?;
        self.apply(&f, av)
    }

    fn apply(&self, f: &Value, args: Vec<Value>) -> R {
        match f {
            Value::Func(name) => {
                if let Some(fd) = self.funcs.get(name) {
                    self.call_user(fd, args)
                } else {
                    self.call_builtin(name, args)
                }
            }
            Value::Closure(c) => {
                let mut env = Env::new();
                for (k, v) in &c.env {
                    env.define(k.clone(), v.clone());
                }
                env.push();
                for (p, v) in c.params.iter().zip(args.into_iter()) {
                    env.define(p.clone(), v);
                }
                self.eval(&c.body, &mut env)
            }
            _ => err("value is not callable"),
        }
    }

    fn call_builtin(&self, name: &str, a: Vec<Value>) -> R {
        let arg = |i: usize| a.get(i).cloned().unwrap_or(Value::Unit);
        match name {
            "len" | "count" => Ok(Value::Int(as_list(&arg(0))?.len() as i64)),
            "sum" => {
                let xs = as_list(&arg(0))?;
                let mut acc = Value::Int(0);
                for x in xs {
                    acc = binop("+", acc, x)?;
                }
                Ok(acc)
            }
            "first" => Ok(Value::Opt(as_list(&arg(0))?.first().cloned().map(Box::new))),
            "last" => Ok(Value::Opt(as_list(&arg(0))?.last().cloned().map(Box::new))),
            "reverse" => {
                let mut xs = as_list(&arg(0))?;
                xs.reverse();
                Ok(Value::List(xs))
            }
            "sort" => {
                let mut xs = as_list(&arg(0))?;
                xs.sort_by(cmp_value);
                Ok(Value::List(xs))
            }
            "take" => {
                let xs = as_list(&arg(0))?;
                let n = as_int(&arg(1))? as usize;
                Ok(Value::List(xs.into_iter().take(n).collect()))
            }
            "contains" => {
                let xs = as_list(&arg(0))?;
                Ok(Value::Bool(xs.iter().any(|x| *x == arg(1))))
            }
            "min" | "max" => {
                // Either min(a, b) or min(list).
                let items = if a.len() == 1 { as_list(&arg(0))? } else { a.clone() };
                let want_max = name == "max";
                let mut best: Option<Value> = None;
                for x in items {
                    best = Some(match best {
                        None => x,
                        Some(b) => {
                            let keep_b = if want_max { cmp_value(&b, &x).is_ge() } else { cmp_value(&b, &x).is_le() };
                            if keep_b { b } else { x }
                        }
                    });
                }
                best.ok_or(Control::Err(format!("`{name}` of empty input")))
            }
            "abs" => match arg(0) {
                Value::Int(n) => Ok(Value::Int(n.abs())),
                Value::Float(f) => Ok(Value::Float(f.abs())),
                _ => err("abs expects a number"),
            },
            "range" => {
                let n = as_int(&arg(0))?;
                Ok(Value::List((0..n).map(Value::Int).collect()))
            }
            "zip" => {
                let xs = as_list(&arg(0))?;
                let ys = as_list(&arg(1))?;
                Ok(Value::List(
                    xs.into_iter().zip(ys).map(|(x, y)| Value::Tuple(vec![x, y])).collect(),
                ))
            }
            "flatten" => {
                let xs = as_list(&arg(0))?;
                let mut out = Vec::new();
                for x in xs {
                    out.extend(as_list(&x)?);
                }
                Ok(Value::List(out))
            }
            "freq" => {
                let xs = as_list(&arg(0))?;
                let mut m: Vec<(Value, Value)> = Vec::new();
                for x in xs {
                    if let Some(slot) = m.iter_mut().find(|(k, _)| *k == x) {
                        if let Value::Int(c) = &mut slot.1 {
                            *c += 1;
                        }
                    } else {
                        m.push((x, Value::Int(1)));
                    }
                }
                Ok(Value::Map(m))
            }
            "keys" => Ok(Value::List(as_map(&arg(0))?.into_iter().map(|(k, _)| k).collect())),
            "values" => Ok(Value::List(as_map(&arg(0))?.into_iter().map(|(_, v)| v).collect())),
            "map" => {
                let xs = as_list(&arg(0))?;
                let f = arg(1);
                let mut out = Vec::with_capacity(xs.len());
                for x in xs {
                    out.push(self.apply(&f, vec![x])?);
                }
                Ok(Value::List(out))
            }
            "filter" => {
                let xs = as_list(&arg(0))?;
                let f = arg(1);
                let mut out = Vec::new();
                for x in xs {
                    if truthy(&self.apply(&f, vec![x.clone()])?) {
                        out.push(x);
                    }
                }
                Ok(Value::List(out))
            }
            "any" => {
                let xs = as_list(&arg(0))?;
                let f = arg(1);
                for x in xs {
                    if truthy(&self.apply(&f, vec![x])?) {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            "all" => {
                let xs = as_list(&arg(0))?;
                let f = arg(1);
                for x in xs {
                    if !truthy(&self.apply(&f, vec![x])?) {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            "find" => {
                let xs = as_list(&arg(0))?;
                let f = arg(1);
                for x in xs {
                    if truthy(&self.apply(&f, vec![x.clone()])?) {
                        return Ok(Value::Opt(Some(Box::new(x))));
                    }
                }
                Ok(Value::Opt(None))
            }
            "fold" => {
                let xs = as_list(&arg(0))?;
                let mut acc = arg(1);
                let f = arg(2);
                for x in xs {
                    acc = self.apply(&f, vec![acc, x])?;
                }
                Ok(acc)
            }
            "reduce" => {
                let xs = as_list(&arg(0))?;
                let f = arg(1);
                let mut it = xs.into_iter();
                match it.next() {
                    None => Ok(Value::Opt(None)),
                    Some(mut acc) => {
                        for x in it {
                            acc = self.apply(&f, vec![acc, x])?;
                        }
                        Ok(Value::Opt(Some(Box::new(acc))))
                    }
                }
            }
            "scan" => {
                let xs = as_list(&arg(0))?;
                let mut acc = arg(1);
                let f = arg(2);
                let mut out = vec![acc.clone()];
                for x in xs {
                    acc = self.apply(&f, vec![acc, x])?;
                    out.push(acc.clone());
                }
                Ok(Value::List(out))
            }
            "group" => {
                let xs = as_list(&arg(0))?;
                let f = arg(1);
                let mut m: Vec<(Value, Value)> = Vec::new();
                for x in xs {
                    let k = self.apply(&f, vec![x.clone()])?;
                    if let Some(slot) = m.iter_mut().find(|(key, _)| *key == k) {
                        if let Value::List(l) = &mut slot.1 {
                            l.push(x);
                        }
                    } else {
                        m.push((k, Value::List(vec![x])));
                    }
                }
                Ok(Value::Map(m))
            }
            other => err(format!("unknown function `{other}`")),
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────

fn truthy(v: &Value) -> bool {
    matches!(v, Value::Bool(true)) || matches!(v, Value::Int(n) if *n != 0)
}

fn as_int(v: &Value) -> Result<i64, Control> {
    match v {
        Value::Int(n) => Ok(*n),
        _ => err("expected an integer"),
    }
}

fn as_list(v: &Value) -> Result<Vec<Value>, Control> {
    match v {
        Value::List(xs) => Ok(xs.clone()),
        Value::Tuple(xs) => Ok(xs.clone()),
        _ => err("expected a collection"),
    }
}

fn as_map(v: &Value) -> Result<Vec<(Value, Value)>, Control> {
    match v {
        Value::Map(m) => Ok(m.clone()),
        _ => err("expected a map"),
    }
}

fn cmp_value(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        _ => Ordering::Equal,
    }
}

fn binop(op: &str, l: Value, r: Value) -> R {
    use Value::*;
    match (op, l, r) {
        ("+", Int(a), Int(b)) => Ok(Int(a + b)),
        ("-", Int(a), Int(b)) => Ok(Int(a - b)),
        ("*", Int(a), Int(b)) => Ok(Int(a * b)),
        ("/", Int(a), Int(b)) if b != 0 => Ok(Int(a / b)),
        ("/", Int(_), Int(_)) => err("division by zero"),
        ("%", Int(a), Int(b)) if b != 0 => Ok(Int(a % b)),
        ("+", Float(a), Float(b)) => Ok(Float(a + b)),
        ("-", Float(a), Float(b)) => Ok(Float(a - b)),
        ("*", Float(a), Float(b)) => Ok(Float(a * b)),
        ("/", Float(a), Float(b)) => Ok(Float(a / b)),
        ("+", Str(a), Str(b)) => Ok(Str(a + &b)),
        ("==", a, b) => Ok(Bool(a == b)),
        ("!=", a, b) => Ok(Bool(a != b)),
        ("<", a, b) => Ok(Bool(cmp_value(&a, &b).is_lt())),
        ("<=", a, b) => Ok(Bool(cmp_value(&a, &b).is_le())),
        (">", a, b) => Ok(Bool(cmp_value(&a, &b).is_gt())),
        (">=", a, b) => Ok(Bool(cmp_value(&a, &b).is_ge())),
        (o, _, _) => err(format!("unsupported binary `{o}` on these types")),
    }
}

fn parse_literal(value: &str, kind: &LiteralKind) -> R {
    match kind {
        LiteralKind::Int | LiteralKind::Byte => {
            let cleaned: String = value.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
            cleaned.parse::<i64>().map(Value::Int).map_err(|_| Control::Err(format!("bad int `{value}`")))
        }
        LiteralKind::Float => {
            let cleaned: String = value.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
            cleaned.parse::<f64>().map(Value::Float).map_err(|_| Control::Err(format!("bad float `{value}`")))
        }
        LiteralKind::Bool => Ok(Value::Bool(value == "true")),
        LiteralKind::String | LiteralKind::FormatString => {
            Ok(Value::Str(value.trim_matches('"').to_string()))
        }
        LiteralKind::Char => Ok(Value::Str(value.trim_matches('\'').to_string())),
    }
}

fn variant(e: &Expr) -> &'static str {
    match e {
        Expr::Match { .. } => "match",
        Expr::MethodCall { .. } => "method calls",
        Expr::FieldAccess { .. } => "field access",
        Expr::StructLit { .. } => "struct literals",
        _ => "this expression",
    }
}

/// Convenience for the CLI / tests: parse, then run `name` with integer args.
pub fn run_source(src: &str, name: &str, args: &[i64]) -> Result<Value, String> {
    let toks = crate::lexer::lex(src);
    let module = crate::parser::parse(&toks).map_err(|e| format!("parse error: {e:?}"))?;
    let interp = Interp::new(&module);
    interp.run(name, args.iter().map(|n| Value::Int(*n)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str, f: &str, args: &[i64]) -> Value {
        run_source(src, f, args).expect("run failed")
    }

    #[test]
    fn sum_and_len() {
        assert_eq!(run("f s() { sum([1, 2, 3, 4]) }", "s", &[]), Value::Int(10));
        assert_eq!(run("f s() { len([9, 8, 7]) }", "s", &[]), Value::Int(3));
    }

    #[test]
    fn map_filter_fold_pipeline() {
        let src = "f even(n) { n % 2 == 0 }\nf dbl(n) { n * 2 }\nf add(a, x) { a + x }\n\
                   f s() { fold(map(filter([1,2,3,4,5,6], even), dbl), 0, add) }";
        // evens [2,4,6] → doubled [4,8,12] → sum 24
        assert_eq!(run(src, "s", &[]), Value::Int(24));
    }

    #[test]
    fn first_is_total() {
        assert_eq!(run("f s() { first([5, 6]) }", "s", &[]), Value::Opt(Some(Box::new(Value::Int(5)))));
        assert_eq!(run("f s() { first([]) }", "s", &[]), Value::Opt(None));
    }

    #[test]
    fn freq_counts() {
        let v = run("f s() { freq([1, 1, 2, 1, 2]) }", "s", &[]);
        if let Value::Map(m) = v {
            assert!(m.contains(&(Value::Int(1), Value::Int(3))));
            assert!(m.contains(&(Value::Int(2), Value::Int(2))));
        } else {
            panic!("expected a map, got {v:?}");
        }
    }

    #[test]
    fn handrolled_loop_runs() {
        let src = "f s() { var t = 0\n for x in [1,2,3,4] { t = t + x }\n t }";
        assert_eq!(run(src, "s", &[]), Value::Int(10));
    }

    #[test]
    fn recursion_runs() {
        let src = "f fact(n) { if n <= 1 { 1 } else { n * fact(n - 1) } }";
        assert_eq!(run(src, "fact", &[5]), Value::Int(120));
    }

    #[test]
    fn reduce_and_range_and_sort() {
        assert_eq!(run("f m(a, b) { if a > b { a } else { b } }\nf s() { reduce([3,1,4,1,5], m) }", "s", &[]),
                   Value::Opt(Some(Box::new(Value::Int(5)))));
        assert_eq!(run("f s() { sum(range(5)) }", "s", &[]), Value::Int(10)); // 0+1+2+3+4
        assert_eq!(run("f s() { first(sort([3, 1, 2])) }", "s", &[]), Value::Opt(Some(Box::new(Value::Int(1)))));
    }

    #[test]
    fn closures_work() {
        // MechGen closure syntax is `fn(x) => expr`.
        assert_eq!(run("f s() { map([1, 2, 3], fn(x) => x * 10) }", "s", &[]),
                   Value::List(vec![Value::Int(10), Value::Int(20), Value::Int(30)]));
    }
}
