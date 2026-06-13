//! A focused tree-walking evaluator that RUNS the §8 standard vocabulary and the
//! arithmetic / control flow around it, so vocabulary programs compute real
//! results — the combinators' runtime. Pure subset (data, functions, the
//! vocabulary); IO/structs/traits are out of scope and report an honest error.

use crate::ast::{Block, Expr, FunctionDef, ItemKind, LiteralKind, Module, Pattern, Stmt, Type};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// A block's group of nested functions, shared so they can call each other.
/// Closures hold this `Rc` strong; it holds them back via `Weak`, so there is
/// no reference cycle — the group frees with the block scope that owns the
/// closures (escaping closures simply lose siblings they no longer co-own).
type SiblingGroup = Rc<RefCell<Vec<(String, Weak<ClosureData>)>>>;

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
    /// A struct value: type name + named fields.
    Struct(String, Vec<(String, Value)>),
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
    /// Set for named nested functions (`f helper(){…}` inside a block). On each
    /// call the name is re-bound to the closure in its own scope, so it can
    /// recurse. Anonymous `fn(x) => …` closures leave this `None`.
    name: Option<String>,
    /// The sibling nested functions declared in the same block. Injected at call
    /// time so they can call each other (mutual recursion). `None` for anonymous
    /// closures and the only entry for a lone nested function is itself.
    siblings: Option<SiblingGroup>,
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
            Value::Struct(name, fields) => {
                let parts: Vec<String> = fields.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{name} {{ {} }}", parts.join(", "))
            }
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
            (Struct(n1, f1), Struct(n2, f2)) => n1 == n2 && f1 == f2,
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
    /// A mutable reference to an existing binding — the root of a mutable path.
    fn get_mut(&mut self, name: &str) -> Option<&mut Value> {
        self.scopes.iter_mut().rev().find_map(|s| s.get_mut(name))
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
        // `defer`red expressions run on the way out (LIFO), no matter how the
        // block exits — normal fall-through, early return/break, or an error.
        let mut deferred: Vec<&Expr> = Vec::new();
        let mut out = Value::Unit;
        let mut early: Option<Control> = None;
        // Nested functions in this block share one group (built incrementally as
        // they are declared) so they can call each other.
        let mut nested_group: Option<SiblingGroup> = None;
        for s in &b.stmts {
            let step: Result<(), Control> = match s {
                Stmt::Let { pattern, value, .. } => {
                    // Full pattern binding: `val x = …`, `val (a, b) = …`, etc.
                    match self.eval(value, env) {
                        Ok(v) => {
                            self.match_pat(pattern, &v, env);
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                Stmt::Expr { expr } => self.eval(expr, env).map(|v| out = v),
                // `guard cond else { … }` — when the condition fails, run the
                // else block (which normally diverges via return/break), and let
                // its control flow propagate. A non-diverging else falls through.
                Stmt::Guard { cond, else_block } => match self.eval(cond, env) {
                    Ok(c) if truthy(&c) => Ok(()),
                    Ok(_) => self.eval_block(else_block, env).map(|_| ()),
                    Err(e) => Err(e),
                },
                Stmt::Defer { expr } => {
                    deferred.push(expr);
                    Ok(())
                }
                // Nested function declaration: bind it as a named closure that
                // captures the current scope and shares the block's sibling group.
                // Self- and mutual recursion work (apply injects the group);
                // outer locals are captured at the definition point, so a sibling
                // must be declared before the call that reaches it executes.
                Stmt::Item { item } => {
                    if let ItemKind::Function(fd) = &item.kind {
                        let body = match &fd.body_expr {
                            Some(be) => (**be).clone(),
                            None => Expr::Block { block: fd.body.clone() },
                        };
                        let group = nested_group
                            .get_or_insert_with(|| Rc::new(RefCell::new(Vec::new())))
                            .clone();
                        let rc = Rc::new(ClosureData {
                            params: fd.params.iter().map(|p| p.name.clone()).collect(),
                            body,
                            env: env.flatten(),
                            name: Some(fd.name.clone()),
                            siblings: Some(group.clone()),
                        });
                        group.borrow_mut().push((fd.name.clone(), Rc::downgrade(&rc)));
                        env.define(fd.name.clone(), Value::Closure(rc));
                    }
                    Ok(())
                }
            };
            if let Err(e) = step {
                early = Some(e);
                break;
            }
        }
        let result = if let Some(e) = early {
            Err(e)
        } else if let Some(te) = &b.tail_expr {
            self.eval(te, env)
        } else {
            Ok(out)
        };
        for d in deferred.iter().rev() {
            let _ = self.eval(d, env);
        }
        env.pop();
        result
    }

    fn eval(&self, e: &Expr, env: &mut Env) -> R {
        match e {
            Expr::Literal { value, kind } => match kind {
                LiteralKind::FormatString => self.eval_format_string(value, env),
                _ => parse_literal(value, kind),
            },
            Expr::Ident { name } => match env.get(name) {
                Some(v) => Ok(v),
                None if name == "None" => Ok(Value::Opt(None)),
                // `true`/`false` lex as bare identifiers (the canonical bool
                // literals are `1b`/`0b`); map the words so both forms work.
                None if name == "true" => Ok(Value::Bool(true)),
                None if name == "false" => Ok(Value::Bool(false)),
                None => Ok(Value::Func(name.clone())),
            },
            Expr::Await { expr } => {
                // `e.await` — this evaluator is synchronous (no event loop), so
                // an async fn already runs to completion when called. Awaiting is
                // therefore the identity on its already-available result.
                self.eval(expr, env)
            }
            Expr::Try { expr } => {
                // `e?` — unwrap an option, or early-return `None` from the fn.
                match self.eval(expr, env)? {
                    Value::Opt(Some(inner)) => Ok(*inner),
                    Value::Opt(None) => Err(Control::Return(Value::Opt(None))),
                    other => Ok(other),
                }
            }
            Expr::Unary { op, operand } => {
                let v = self.eval(operand, env)?;
                match (op.as_str(), v) {
                    ("-", Value::Int(n)) => Ok(Value::Int(-n)),
                    ("-", Value::Float(f)) => Ok(Value::Float(-f)),
                    // `!` negates truthiness, so `!0` / `![]` work, not just bools.
                    ("!", v) => Ok(Value::Bool(!truthy(&v))),
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
                // Compound assignment: `x += e` is `x = x <op> e`, writing back
                // to the lvalue. The parser emits these as a Binary node (only
                // plain `=` becomes Expr::Assign), so unfold them here. Matched
                // explicitly — `==`/`<=`/`>=`/`!=` also end in `=` but aren't this.
                let compound = match op.as_str() {
                    "+=" => Some("+"),
                    "-=" => Some("-"),
                    "*=" => Some("*"),
                    "/=" => Some("/"),
                    "%=" => Some("%"),
                    _ => None,
                };
                if let Some(base) = compound {
                    let cur = self.eval(left, env)?;
                    let r = self.eval(right, env)?;
                    let new = binop(base, cur, r)?;
                    self.assign_target(left, new, env)?;
                    return Ok(Value::Unit);
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
            Expr::Match { scrutinee, arms } => {
                let v = match scrutinee {
                    Some(e) => self.eval(e, env)?,
                    None => Value::Unit,
                };
                for arm in arms {
                    env.push();
                    if self.match_pat(&arm.pattern, &v, env) {
                        let r = self.eval(&arm.body, env);
                        env.pop();
                        return r;
                    }
                    env.pop();
                }
                err("no match arm matched")
            }
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
                // Slice syntax: `xs[a..b]`, `s[a..]`, `xs[..b]`, `xs[..]`. The
                // parser makes the index a Range; open ends use an `_` sentinel.
                if let Expr::Range { start, end, inclusive } = index.as_ref() {
                    let len = match &o {
                        Value::List(xs) => xs.len() as i64,
                        Value::Str(s) => s.chars().count() as i64,
                        _ => return err("cannot slice this value"),
                    };
                    let lo = self.slice_bound(start, env, 0)?.clamp(0, len);
                    let raw = self.slice_bound(end, env, len)?;
                    let hi = (if *inclusive { raw + 1 } else { raw }).clamp(lo, len);
                    return match o {
                        Value::List(xs) => Ok(Value::List(xs[lo as usize..hi as usize].to_vec())),
                        Value::Str(s) => {
                            Ok(Value::Str(s.chars().skip(lo as usize).take((hi - lo) as usize).collect()))
                        }
                        _ => unreachable!(),
                    };
                }
                let i = self.eval(index, env)?;
                match (o, i) {
                    (Value::List(xs), Value::Int(n)) => xs
                        .get(n as usize)
                        .cloned()
                        .ok_or(Control::Err("index out of bounds".into())),
                    // `s[i]` — the i-th character as a one-char string.
                    (Value::Str(s), Value::Int(n)) => s
                        .chars()
                        .nth(n as usize)
                        .map(|c| Value::Str(c.to_string()))
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
                name: None,
                siblings: None,
            }))),
            Expr::Assign { target, value } => {
                let v = self.eval(value, env)?;
                self.assign_target(target, v, env)?;
                Ok(Value::Unit)
            }
            Expr::For { pattern, iter, body } => {
                let seq = self.eval(iter, env)?;
                let items = as_list(&seq)?;
                for it in items {
                    env.push();
                    // Destructure the loop variable: `for x in …`, `for (i, v) in …`.
                    self.match_pat(pattern, &it, env);
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
            Expr::MethodCall { receiver, method, args, .. } => {
                // `recv.method(a, b)` desugars to `method(recv, a, b)` — so the
                // vocabulary works in method position (`xs.filter(e).map(d)`).
                let mut av = vec![self.eval(receiver, env)?];
                for a in args {
                    av.push(self.eval(a, env)?);
                }
                if let Some(fd) = self.funcs.get(method) {
                    self.call_user(fd, av)
                } else {
                    self.call_builtin(method, av)
                }
            }
            Expr::StructLit { path, fields } => {
                let name = path.last().cloned().unwrap_or_default();
                let mut fvals = Vec::with_capacity(fields.len());
                for fi in fields {
                    let v = match &fi.value {
                        Some(e) => self.eval(e, env)?,
                        None => env.get(&fi.name).unwrap_or(Value::Unit), // field shorthand
                    };
                    fvals.push((fi.name.clone(), v));
                }
                Ok(Value::Struct(name, fvals))
            }
            Expr::FieldAccess { object, field } => {
                let o = self.eval(object, env)?;
                match o {
                    Value::Struct(_, fields) => fields
                        .iter()
                        .find(|(k, _)| k == field)
                        .map(|(_, v)| v.clone())
                        .ok_or(Control::Err(format!("no field `{field}`"))),
                    // tuple.0 / tuple.1 positional access
                    Value::Tuple(xs) => field
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| xs.get(i).cloned())
                        .ok_or(Control::Err(format!("no tuple field `{field}`"))),
                    _ => err("field access on a non-struct value"),
                }
            }
            Expr::Loop { body } => {
                // `loop { ... break v }` — infinite loop whose value is the break.
                loop {
                    match self.eval_block(body, env) {
                        Ok(_) | Err(Control::Continue) => {}
                        Err(Control::Break(v)) => return Ok(v),
                        Err(other) => return Err(other),
                    }
                }
            }
            Expr::ArrayRepeat { value, count } => {
                // `[x; n]` — a list of `n` copies of `x`.
                let v = self.eval(value, env)?;
                let n = as_int(&self.eval(count, env)?)?;
                Ok(Value::List(vec![v; n.max(0) as usize]))
            }
            Expr::Cast { expr, ty } => {
                // Numeric casts: `x as f64` / `x as i64` (the well-defined ones).
                let v = self.eval(expr, env)?;
                let target = match ty {
                    Type::Path { segments, .. } => segments.last().map(|s| s.as_str()).unwrap_or(""),
                    _ => "",
                };
                match target {
                    "f64" | "f32" => Ok(Value::Float(match v {
                        Value::Int(n) => n as f64,
                        Value::Float(f) => f,
                        _ => return err("cannot cast this value to a float"),
                    })),
                    "i64" | "i32" | "u64" | "u32" | "usize" | "isize" => Ok(Value::Int(match v {
                        Value::Float(f) => f as i64,
                        Value::Int(n) => n,
                        Value::Bool(b) => b as i64,
                        _ => return err("cannot cast this value to an integer"),
                    })),
                    other => err(format!("unsupported cast to `{other}`")),
                }
            }
            Expr::Is { expr, pattern } => {
                // `x is Pattern` — a boolean test that also flow-binds into the
                // current scope, so `if c is Some(v) { ..v.. }` can use `v`.
                let v = self.eval(expr, env)?;
                Ok(Value::Bool(self.match_pat(pattern, &v, env)))
            }
            other => err(format!("evaluator does not support {} yet", variant(other))),
        }
    }

    /// A slice bound: the `_` sentinel (an open `..` end) becomes `default`;
    /// any other expression is evaluated to an integer.
    fn slice_bound(&self, e: &Expr, env: &mut Env, default: i64) -> Result<i64, Control> {
        if let Expr::Ident { name } = e {
            if name == "_" {
                return Ok(default);
            }
        }
        as_int(&self.eval(e, env)?)
    }

    /// Evaluate an f-string: `f"x = {x}, sum = {sum(xs)}"`. The raw token text
    /// is the whole source slice (`f"…"`); we strip the delimiters, then splice
    /// literal runs with `{expr}` holes. `{{`/`}}` are literal-brace escapes;
    /// each hole is parsed as a real expression and evaluated in `env`.
    fn eval_format_string(&self, raw: &str, env: &mut Env) -> R {
        let inner = match (raw.find('"'), raw.rfind('"')) {
            (Some(a), Some(b)) if b > a => &raw[a + 1..b],
            _ => "",
        };
        let mut out = String::new();
        let mut chars = inner.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    out.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    out.push('}');
                }
                '{' => {
                    // Collect the hole's source up to the matching `}`, tracking
                    // nested braces so map literals (`{k: v}`) pass through whole.
                    let mut depth = 1usize;
                    let mut src = String::new();
                    for nc in chars.by_ref() {
                        match nc {
                            '{' => depth += 1,
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        src.push(nc);
                    }
                    if depth != 0 {
                        return err("unterminated `{` in format string");
                    }
                    let v = self.eval_embedded(src.trim(), env)?;
                    out.push_str(&interp_str(&v));
                }
                // Backslash escapes in the literal portions (the same set as
                // plain strings); holes are evaluated expressions, untouched.
                '\\' => match chars.next() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('0') => out.push('\0'),
                    Some('\\') => out.push('\\'),
                    Some('"') => out.push('"'),
                    Some('\'') => out.push('\''),
                    Some(o) => {
                        out.push('\\');
                        out.push(o);
                    }
                    None => out.push('\\'),
                },
                _ => out.push(c),
            }
        }
        Ok(Value::Str(out))
    }

    /// Parse one embedded expression (a format-string hole) and evaluate it in
    /// the current scope. We wrap it in a throwaway function so the real parser
    /// handles the full expression grammar, then pull the body expression back.
    fn eval_embedded(&self, src: &str, env: &mut Env) -> R {
        if src.is_empty() {
            return err("empty `{}` in format string");
        }
        let wrapped = format!("f __interp__() {{ {src} }}");
        let toks = crate::lexer::lex(&wrapped);
        let module = crate::parser::parse(&toks)
            .map_err(|e| Control::Err(format!("format expr parse error in `{src}`: {e:?}")))?;
        let expr = module.items.iter().find_map(|it| match &it.kind {
            ItemKind::Function(fd) if fd.name == "__interp__" => fd
                .body_expr
                .clone()
                .or_else(|| fd.body.tail_expr.clone())
                .map(|b| *b)
                .or_else(|| fd.body.stmts.iter().rev().find_map(|s| match s {
                    Stmt::Expr { expr } => Some(expr.clone()),
                    _ => None,
                })),
            _ => None,
        });
        match expr {
            Some(e) => self.eval(&e, env),
            None => err(format!("could not parse format expression `{src}`")),
        }
    }

    /// Assign `v` to a target: a plain variable (introducing it if new), a
    /// tuple/list destructuring pattern (`(a, b) = pair`, possibly nested), or
    /// an arbitrary lvalue path (delegated to `assign_place`).
    fn assign_target(&self, target: &Expr, v: Value, env: &mut Env) -> Result<(), Control> {
        match target {
            Expr::Ident { name } => {
                env.assign(name, v);
                Ok(())
            }
            Expr::TupleLit { elements } | Expr::ArrayLit { elements } => {
                let parts = match v {
                    Value::Tuple(xs) | Value::List(xs) => xs,
                    _ => return err("destructuring assignment expects a tuple or list"),
                };
                if parts.len() != elements.len() {
                    return err("destructuring assignment arity mismatch");
                }
                for (t, pv) in elements.iter().zip(parts) {
                    self.assign_target(t, pv, env)?;
                }
                Ok(())
            }
            _ => self.assign_place(target, v, env),
        }
    }

    /// Assign `v` to an arbitrary lvalue path: `a.b.c = v`, `xs[i][j] = v`,
    /// `grid[r][c].field = v`. The path is decomposed root-to-leaf (index
    /// expressions are evaluated up front, before the mutable borrow), then we
    /// descend mutably through the value and write the final slot.
    fn assign_place(&self, target: &Expr, v: Value, env: &mut Env) -> Result<(), Control> {
        let mut accessors: Vec<Accessor> = Vec::new();
        let mut cur = target;
        let root = loop {
            match cur {
                Expr::Ident { name } => break name.as_str(),
                Expr::FieldAccess { object, field } => {
                    accessors.push(Accessor::Field(field.clone()));
                    cur = object;
                }
                Expr::Index { object, index } => {
                    accessors.push(Accessor::Index(self.eval(index, env)?));
                    cur = object;
                }
                _ => return err("assignment target must root in a variable"),
            }
        };
        accessors.reverse();
        let mut place = env.get_mut(root).ok_or(Control::Err(format!("unknown `{root}`")))?;
        for acc in &accessors {
            place = descend(place, acc)?;
        }
        *place = v;
        Ok(())
    }

    fn eval_call(&self, func: &Expr, args: &[Expr], env: &mut Env) -> R {
        // Evaluate args.
        let mut av = Vec::with_capacity(args.len());
        for a in args {
            av.push(self.eval(a, env)?);
        }
        if let Expr::Ident { name } = func {
            // A locally-bound function value (nested `f`, or a closure in a
            // variable) shadows globals — check the environment first, but only
            // intercept when it actually holds something callable.
            if let Some(v @ (Value::Closure(_) | Value::Func(_))) = env.get(name) {
                return self.apply(&v, av);
            }
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
                // A named nested function sees itself, so it can recurse.
                if let Some(n) = &c.name {
                    env.define(n.clone(), Value::Closure(Rc::clone(c)));
                }
                // …and its siblings declared in the same block, so two nested
                // functions can call each other (mutual recursion). Weak refs
                // upgrade while the defining block scope is still live.
                if let Some(group) = &c.siblings {
                    for (name, weak) in group.borrow().iter() {
                        if let Some(rc) = weak.upgrade() {
                            env.define(name.clone(), Value::Closure(rc));
                        }
                    }
                }
                env.push();
                for (p, v) in c.params.iter().zip(args.into_iter()) {
                    env.define(p.clone(), v);
                }
                // Catch `return` so it stays within the callee (named nested
                // functions use it); harmless for `=> expr` closures.
                match self.eval(&c.body, &mut env) {
                    Err(Control::Return(v)) => Ok(v),
                    other => other,
                }
            }
            _ => err("value is not callable"),
        }
    }

    /// Try to match `pat` against `val`, binding variables into the current
    /// scope on success. Supports the patterns vocabulary results need:
    /// `Some(x)`/`None` (totality), literals, tuples, wildcards, idents, `or`.
    fn match_pat(&self, pat: &Pattern, val: &Value, env: &mut Env) -> bool {
        match pat {
            Pattern::Wildcard => true,
            Pattern::Ident { name } => {
                // `None` written as a bare ident still matches the empty option.
                if name == "None" {
                    matches!(val, Value::Opt(None))
                } else {
                    env.define(name.clone(), val.clone());
                    true
                }
            }
            Pattern::Literal { value } => lit_matches(value, val),
            Pattern::Enum { path, elements } => {
                match (path.last().map(|s| s.as_str()).unwrap_or(""), val) {
                    ("Some", Value::Opt(Some(inner))) => {
                        elements.first().map(|p| self.match_pat(p, inner, env)).unwrap_or(true)
                    }
                    ("None", Value::Opt(None)) => true,
                    _ => false,
                }
            }
            Pattern::Tuple { elements } => match val {
                Value::Tuple(vs) if vs.len() == elements.len() => {
                    elements.iter().zip(vs).all(|(p, v)| self.match_pat(p, v, env))
                }
                _ => false,
            },
            Pattern::Slice { elements, rest, rest_name } => match val {
                Value::List(vs) => {
                    // Without `..`: exact arity. With `..`: at least `elements`.
                    let fits = if *rest { vs.len() >= elements.len() } else { vs.len() == elements.len() };
                    if !fits {
                        return false;
                    }
                    if !elements.iter().zip(vs).all(|(p, v)| self.match_pat(p, v, env)) {
                        return false;
                    }
                    // Bind the named tail (`..tail`) to the remaining elements.
                    if let Some(name) = rest_name {
                        let tail = vs[elements.len()..].to_vec();
                        env.define(name.clone(), Value::List(tail));
                    }
                    true
                }
                _ => false,
            },
            Pattern::Struct { path, fields } => match val {
                Value::Struct(name, vfields) => {
                    // The struct tag must match (`@Circle{..}` won't match a Square),
                    // then every named field pattern must match its value.
                    if let Some(want) = path.last() {
                        if want != name {
                            return false;
                        }
                    }
                    fields.iter().all(|fp| match vfields.iter().find(|(k, _)| k == &fp.name) {
                        Some((_, fv)) => match &fp.pattern {
                            Some(p) => self.match_pat(p, fv, env),
                            None => {
                                // Shorthand `{ x }` binds the field's value to `x`.
                                env.define(fp.name.clone(), fv.clone());
                                true
                            }
                        },
                        None => false,
                    })
                }
                _ => false,
            },
            Pattern::Or { patterns } => patterns.iter().any(|p| self.match_pat(p, val, env)),
            Pattern::Ref { pattern } => self.match_pat(pattern, val, env),
            _ => false,
        }
    }

    fn call_builtin(&self, name: &str, a: Vec<Value>) -> R {
        let arg = |i: usize| a.get(i).cloned().unwrap_or(Value::Unit);
        match name {
            "len" | "count" => match arg(0) {
                Value::Map(m) => Ok(Value::Int(m.len() as i64)),
                Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                other => Ok(Value::Int(as_list(&other)?.len() as i64)),
            },
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
            "contains" => match arg(0) {
                // Substring test for strings, key membership for maps, else
                // element membership for lists.
                Value::Str(s) => Ok(Value::Bool(s.contains(&as_str(&arg(1))?))),
                Value::Map(m) => Ok(Value::Bool(m.iter().any(|(k, _)| *k == arg(1)))),
                other => Ok(Value::Bool(as_list(&other)?.iter().any(|x| *x == arg(1)))),
            },
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
            // String / text vocabulary.
            "split" => {
                let s = as_str(&arg(0))?;
                let sep = as_str(&arg(1))?;
                let parts: Vec<Value> = if sep.is_empty() {
                    s.chars().map(|c| Value::Str(c.to_string())).collect()
                } else {
                    s.split(&sep).map(|p| Value::Str(p.to_string())).collect()
                };
                Ok(Value::List(parts))
            }
            "words" => Ok(Value::List(
                as_str(&arg(0))?.split_whitespace().map(|w| Value::Str(w.to_string())).collect(),
            )),
            "lines" => Ok(Value::List(
                as_str(&arg(0))?.lines().map(|l| Value::Str(l.to_string())).collect(),
            )),
            "chars" => Ok(Value::List(
                as_str(&arg(0))?.chars().map(|c| Value::Str(c.to_string())).collect(),
            )),
            "join" => {
                let parts: Vec<String> = as_list(&arg(0))?
                    .iter()
                    .map(|v| match v {
                        Value::Str(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .collect();
                Ok(Value::Str(parts.join(&as_str(&arg(1))?)))
            }
            "upper" => Ok(Value::Str(as_str(&arg(0))?.to_uppercase())),
            "lower" => Ok(Value::Str(as_str(&arg(0))?.to_lowercase())),
            // Option construction — pairs with the §8 totality story (first/find/
            // reduce return `?A`; now you can build and thread options too).
            "Some" => Ok(Value::Opt(Some(Box::new(arg(0))))),
            "None" => Ok(Value::Opt(None)),
            other => err(format!("unknown function `{other}`")),
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────

/// Render a value for string interpolation: bare strings come through without
/// the debug quotes `Display` adds, everything else uses its `Display` form.
fn interp_str(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        other => other.to_string(),
    }
}

/// One step of an lvalue path: `.field` or `[index]` (the index pre-evaluated).
enum Accessor {
    Field(String),
    Index(Value),
}

/// Descend one accessor into a mutable place, returning a reference to the
/// targeted slot. A missing map key or struct field is created (so `m[k] = v`
/// and `p.newfield = v` work); a list index out of bounds is an error.
fn descend<'a>(place: &'a mut Value, acc: &Accessor) -> Result<&'a mut Value, Control> {
    match (place, acc) {
        (Value::List(xs), Accessor::Index(Value::Int(n))) => {
            xs.get_mut(*n as usize).ok_or(Control::Err("index out of bounds".into()))
        }
        (Value::Map(m), Accessor::Index(k)) => {
            match m.iter().position(|(ek, _)| ek == k) {
                Some(i) => Ok(&mut m[i].1),
                None => {
                    m.push((k.clone(), Value::Unit));
                    let i = m.len() - 1;
                    Ok(&mut m[i].1)
                }
            }
        }
        (Value::Struct(_, fields), Accessor::Field(f)) => {
            match fields.iter().position(|(k, _)| k == f) {
                Some(i) => Ok(&mut fields[i].1),
                None => {
                    fields.push((f.clone(), Value::Unit));
                    let i = fields.len() - 1;
                    Ok(&mut fields[i].1)
                }
            }
        }
        _ => err("assignment path does not match the value's shape"),
    }
}

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
        // Iterating a string yields its characters; a map yields `(k, v)` pairs.
        // This makes `for c in "ab"`, `for (k, v) in m`, and the vocabulary
        // combinators (map/filter/…) work uniformly over strings and maps.
        Value::Str(s) => Ok(s.chars().map(|c| Value::Str(c.to_string())).collect()),
        Value::Map(m) => Ok(m.iter().map(|(k, v)| Value::Tuple(vec![k.clone(), v.clone()])).collect()),
        _ => err("expected a collection"),
    }
}

fn as_map(v: &Value) -> Result<Vec<(Value, Value)>, Control> {
    match v {
        Value::Map(m) => Ok(m.clone()),
        _ => err("expected a map"),
    }
}

fn as_str(v: &Value) -> Result<String, Control> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        _ => err("expected a string"),
    }
}

fn cmp_value(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        // Mixed numerics compare by promoting the integer to float, matching
        // how arithmetic promotes (so `2 < 3.5` and `5 == 5.0` behave sanely).
        (Value::Int(x), Value::Float(y)) => (*x as f64).partial_cmp(y).unwrap_or(Ordering::Equal),
        (Value::Float(x), Value::Int(y)) => x.partial_cmp(&(*y as f64)).unwrap_or(Ordering::Equal),
        (Value::Str(x), Value::Str(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
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
        ("%", Float(a), Float(b)) => Ok(Float(a % b)),
        ("+", Str(a), Str(b)) => Ok(Str(a + &b)),
        // List concatenation: `[1,2] + [3,4]` => `[1,2,3,4]` (so fold over a
        // list accumulator works: `fold(xs, [], fn(a, x) => a + [f(x)])`).
        ("+", List(mut a), List(b)) => {
            a.extend(b);
            Ok(List(a))
        }
        // Bitwise / shift operators on integers (`|` is bit-or, hence closures
        // are written `fn(x)`, not `|x|`). Shift counts are masked to 0..63.
        ("&", Int(a), Int(b)) => Ok(Int(a & b)),
        ("|", Int(a), Int(b)) => Ok(Int(a | b)),
        ("^", Int(a), Int(b)) => Ok(Int(a ^ b)),
        ("<<", Int(a), Int(b)) => Ok(Int(a.wrapping_shl(b as u32))),
        (">>", Int(a), Int(b)) => Ok(Int(a.wrapping_shr(b as u32))),
        // Mixed numerics promote to float (so `n / 2.0` and `x as f64 / 2` work).
        (o @ ("+" | "-" | "*" | "/" | "%"), Int(a), Float(b)) => binop(o, Float(a as f64), Float(b)),
        (o @ ("+" | "-" | "*" | "/" | "%"), Float(a), Int(b)) => binop(o, Float(a), Float(b as f64)),
        // Mixed-numeric equality promotes (matches arithmetic and ordering).
        ("==", Int(a), Float(b)) => Ok(Bool(a as f64 == b)),
        ("==", Float(a), Int(b)) => Ok(Bool(a == b as f64)),
        ("!=", Int(a), Float(b)) => Ok(Bool(a as f64 != b)),
        ("!=", Float(a), Int(b)) => Ok(Bool(a != b as f64)),
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
        LiteralKind::Int | LiteralKind::Byte => parse_int_literal(value).map(Value::Int),
        LiteralKind::Float => parse_float_literal(value).map(Value::Float),
        // Bool literals: canonical `1b`/`0b` (the lexer keeps the text) and the
        // Rust-style words `true`/`false`. Anything else (incl. `0b`) is false.
        LiteralKind::Bool => Ok(Value::Bool(value == "true" || value == "1b")),
        LiteralKind::String | LiteralKind::FormatString => {
            Ok(Value::Str(unescape(value.trim_matches('"'))))
        }
        LiteralKind::Char => Ok(Value::Str(unescape(value.trim_matches('\'')))),
    }
}

/// Parse an integer literal: `0xFF` / `0o17` / `0b1010` (radix prefixes),
/// `1_000_000` (digit separators), a trailing type suffix (`5i64`), an optional
/// leading `-`, and the byte-char form `b'A'` (its code point).
fn parse_int_literal(value: &str) -> Result<i64, Control> {
    let bad = || Control::Err(format!("bad int `{value}`"));
    let v = value.trim();
    if let Some(rest) = v.strip_prefix("b'") {
        let inner = rest.strip_suffix('\'').unwrap_or(rest);
        return unescape(inner).chars().next().map(|c| c as i64).ok_or_else(bad);
    }
    let neg = v.starts_with('-');
    let v = v.strip_prefix('-').unwrap_or(v);
    let (radix, digits) = if let Some(h) = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
        (16u32, h)
    } else if let Some(o) = v.strip_prefix("0o").or_else(|| v.strip_prefix("0O")) {
        (8, o)
    } else if let Some(b) = v.strip_prefix("0b").or_else(|| v.strip_prefix("0B")) {
        (2, b)
    } else {
        (10, v)
    };
    // Take valid digits (and `_`), which also drops a trailing type suffix.
    let cleaned: String = digits
        .chars()
        .take_while(|c| c.is_digit(radix) || *c == '_')
        .filter(|c| *c != '_')
        .collect();
    let n = i64::from_str_radix(&cleaned, radix).map_err(|_| bad())?;
    Ok(if neg { -n } else { n })
}

/// Parse a float literal: digit separators (`1_000.5`), exponents (`2e10`,
/// `1.5e-2`), and a trailing `f32`/`f64` suffix.
fn parse_float_literal(value: &str) -> Result<f64, Control> {
    let mut cleaned: String = value.trim().chars().filter(|c| *c != '_').collect();
    for suffix in ["f64", "f32"] {
        if let Some(stripped) = cleaned.strip_suffix(suffix) {
            cleaned.truncate(stripped.len());
            break;
        }
    }
    cleaned.parse::<f64>().map_err(|_| Control::Err(format!("bad float `{value}`")))
}

/// Resolve the common backslash escapes in a string/char literal body. Unknown
/// escapes are passed through verbatim (backslash kept), matching a lenient
/// Rust-ish reading. The lexer already keeps `\"`/`\\` from ending the string.
fn unescape(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn lit_matches(lit: &str, val: &Value) -> bool {
    match val {
        Value::Int(n) => parse_int_literal(lit).map(|x| x == *n).unwrap_or(false),
        Value::Bool(b) => lit == if *b { "true" } else { "false" },
        Value::Str(s) => &unescape(lit.trim_matches('"')) == s,
        _ => false,
    }
}

fn variant(e: &Expr) -> &'static str {
    match e {
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

    /// Continuous benchmark: correctness coverage + throughput. Run with
    ///   cargo test --release eval_bench -- --ignored --nocapture
    #[test]
    #[ignore]
    fn eval_bench() {
        use std::time::Instant;
        // Correctness coverage — programs the evaluator must compute exactly.
        let cases: &[(&str, &str, &[i64], Value)] = &[
            ("f s() { sum([1,2,3,4]) }", "s", &[], Value::Int(10)),
            ("f e(n){n%2==0}\nf s(){ len(filter([1,2,3,4,5,6], e)) }", "s", &[], Value::Int(3)),
            ("f a(x,y){x+y}\nf s(){ fold([1,2,3,4], 0, a) }", "s", &[], Value::Int(10)),
            ("f s(){ sum(range(10)) }", "s", &[], Value::Int(45)),
            ("f fib(n){ if n<2 {n} else {fib(n-1)+fib(n-2)} }", "fib", &[15], Value::Int(610)),
            ("f g(x){x>2}\nf s(){ match find([1,2,3,4], g) { Some(v) => v, None => 0 } }", "s", &[], Value::Int(3)),
            ("f s(){ match first([]) { Some(v) => v, None => 99 } }", "s", &[], Value::Int(99)),
            ("f e(n){n%2==0}\nf d(n){n*2}\nf s(){ sum([1,2,3,4,5,6].filter(e).map(d)) }", "s", &[], Value::Int(24)),
            ("f s(){ sum(map(range(5), fn(x) => x * x)) }", "s", &[], Value::Int(30)),
            ("f s(){ len(zip([1,2,3], [4,5,6])) }", "s", &[], Value::Int(3)),
            // Struct construction (`@Name { ... }`) + field access.
            ("S P { x: i32, y: i32 }\nf d2(p){ p.x*p.x + p.y*p.y }\nf s(){ d2(@P { x: 3, y: 4 }) }", "s", &[], Value::Int(25)),
            // Diverse real programs.
            ("f s(){ sum(flatten([[1,2],[3,4],[5]])) }", "s", &[], Value::Int(15)),
            ("f a(x,y){x+y}\nf s(){ last(scan([1,2,3,4], 0, a)) }", "s", &[], Value::Opt(Some(Box::new(Value::Int(10))))),
            ("f s(){ var i = 0\n var t = 0\n while i < 5 { t = t + i\n i = i + 1 }\n t }", "s", &[], Value::Int(10)),
            ("f big(xs){ for x in xs { if x > 3 { return x } }\n 0 }\nf s(){ big([1,2,3,7,2]) }", "s", &[], Value::Int(7)),
            ("f p(n){ n > 0 }\nf s(){ all([1,2,3], p) }", "s", &[], Value::Bool(true)),
            // String / text vocabulary.
            ("f s(){ len(words(\"the quick brown fox\")) }", "s", &[], Value::Int(4)),
            ("f s(){ join([\"a\", \"b\", \"c\"], \"-\") }", "s", &[], Value::Str("a-b-c".into())),
            ("f s(){ upper(\"hi\") }", "s", &[], Value::Str("HI".into())),
            ("f s(){ len(keys(freq(chars(\"banana\")))) }", "s", &[], Value::Int(3)),
            // Option construction + the `?` operator (early-return on None).
            ("f h(xs){ val x = first(xs)?\n Some(x * 2) }\nf s(){ match h([5,6]) { Some(v) => v, None => 0 } }", "s", &[], Value::Int(10)),
            ("f h(xs){ val x = first(xs)?\n Some(x * 2) }\nf s(){ match h([]) { Some(v) => v, None => 0 } }", "s", &[], Value::Int(0)),
            // loop + break value; [x; n] repeat; numeric cast; `is` pattern test.
            ("f s(){ var i = 0\n loop { if i == 5 { break i * 10 }\n i = i + 1 } }", "s", &[], Value::Int(50)),
            ("f s(){ sum([7; 4]) }", "s", &[], Value::Int(28)),
            ("f s(){ 7 as f64 / 2 as f64 }", "s", &[], Value::Float(3.5)),
            ("f s(){ if first([9]) is Some(v) { v } else { 0 } }", "s", &[], Value::Int(9)),
            // Mixed Int/Float arithmetic promotes to float.
            ("f s(){ 7 / 2 as f64 }", "s", &[], Value::Float(3.5)),
            // Indexed element assignment and struct field assignment.
            ("f s(){ var xs = [1, 2, 3]\n xs[1] = 20\n sum(xs) }", "s", &[], Value::Int(24)),
            ("S P { x: i32, y: i32 }\nf s(){ var p = @P { x: 1, y: 2 }\n p.x = 10\n p.x + p.y }", "s", &[], Value::Int(12)),
            // f-string interpolation: a simple binding, an embedded vocabulary
            // call, and a `{{`/`}}` literal-brace escape.
            ("f s(){ val n = 6\n f\"n={n} sq={n * n}\" }", "s", &[], Value::Str("n=6 sq=36".into())),
            ("f s(){ f\"total={sum([1,2,3,4])}\" }", "s", &[], Value::Str("total=10".into())),
            ("f s(){ f\"{{literal}} {upper(\"hi\")}\" }", "s", &[], Value::Str("{literal} HI".into())),
            // Nested lvalue paths: grid[r][c] = v, and struct field through index.
            ("f s(){ var g = [[1, 2], [3, 4]]\n g[1][0] = 30\n g[0][1] = 20\n g[0][1] + g[1][0] }", "s", &[], Value::Int(50)),
            ("S P { x: i32, y: i32 }\nf s(){ var ps = [@P { x: 1, y: 1 }]\n ps[0].x = 9\n ps[0].x + ps[0].y }", "s", &[], Value::Int(10)),
            // Tuple destructuring: bare assignment (`(a, b) = pair`) and a `for`
            // loop over zip(...). NB a leading `(a,b)=…` must head its block — a
            // preceding value-ending line merges into a `0(a, b)` call (layout);
            // the `val (a, b) = …` let form below has no such constraint.
            ("f s(){ (a, b) = (3, 4)\n a * 10 + b }", "s", &[], Value::Int(34)),
            ("f s(){ var t = 0\n for (i, x) in zip([1,2,3], [10,20,30]) { t = t + i * x }\n t }", "s", &[], Value::Int(140)),
            // Slice patterns: exact-arity destructure, and head/tail recursion.
            ("f s(){ match [4, 5] { [a, b] => a * b, _ => 0 } }", "s", &[], Value::Int(20)),
            ("f rsum(xs){ match xs { [] => 0, [h, ..t] => h + rsum(t) } }\nf s(){ rsum([1,2,3,4,5]) }", "s", &[], Value::Int(15)),
            // Destructuring `let`: tuple and slice (head/tail) binders.
            ("f s(){ val (a, b) = (3, 4)\n a * 10 + b }", "s", &[], Value::Int(34)),
            ("f s(){ val [h, ..t] = [10, 1, 2, 3]\n h + sum(t) }", "s", &[], Value::Int(16)),
            // Struct patterns (`@Name { … }`): field destructure, and tag
            // discrimination across two struct types in one match.
            ("S P { x: i32, y: i32 }\nf s(){ match @P { x: 3, y: 4 } { @P { x, y } => x * 10 + y } }", "s", &[], Value::Int(34)),
            ("S Circle { r: i32 }\nS Square { s: i32 }\nf ar(sh){ match sh { @Circle { r } => r * r, @Square { s } => s * s } }\nf s(){ ar(@Circle { r: 6 }) + ar(@Square { s: 2 }) }", "s", &[], Value::Int(40)),
            // A newline-leading `if … else` as a non-final statement: previously
            // mis-parsed as `(prev?) …` because `if` and try share the `?` token.
            ("f s(){ var t = 0\n if 9 > 3 { t = 1 } else { t = 2 }\n t }", "s", &[], Value::Int(1)),
            ("f s(){ val z = 5\n if z < 3 { 100 } else { z * 2 } }", "s", &[], Value::Int(10)),
            // Postfix `.await` — synchronous run-to-completion: identity on the
            // value, and awaiting the result of an `af` (async fn) call.
            ("f s(){ (20 + 1).await * 2 }", "s", &[], Value::Int(42)),
            ("af dbl(n){ n * 2 }\nf s(){ dbl(21).await }", "s", &[], Value::Int(42)),
            // `guard cond else { … }` — early exit when the precondition fails.
            ("f sd(a, b){ guard b != 0 else { return 99 }\n a / b }\nf s(){ sd(8, 0) + sd(20, 5) }", "s", &[], Value::Int(103)),
            // `defer` — runs at block exit, LIFO: x+5 then x*2 => (0+5)*2 = 10.
            ("f s(){ var x = 0\n { defer x = x * 2\n defer x = x + 5 }\n x }", "s", &[], Value::Int(10)),
            // Nested function declarations: a local helper, and self-recursion.
            ("f s(){ f dbl(n){ n * 2 }\n dbl(21) }", "s", &[], Value::Int(42)),
            ("f s(){ f fac(n){ if n < 2 { 1 } else { n * fac(n - 1) } }\n fac(5) }", "s", &[], Value::Int(120)),
            // Mutual recursion between two nested functions (even/odd).
            ("f s(){ f ev(n){ if n == 0 { 1b } else { od(n - 1) } }\n f od(n){ if n == 0 { 0b } else { ev(n - 1) } }\n if ev(10) { 1 } else { 0 } }", "s", &[], Value::Int(1)),
            // Bitwise / shift operators on integers.
            ("f s(){ (5 | 2) + (6 & 3) * 10 }", "s", &[], Value::Int(27)),
            ("f s(){ (5 ^ 1) + (1 << 4) + (255 >> 4) }", "s", &[], Value::Int(35)),
            // Compound assignment, on a variable and on an indexed element.
            ("f s(){ var x = 5\n x += 3\n x *= 4\n x -= 2\n x }", "s", &[], Value::Int(30)),
            ("f s(){ var xs = [10, 20, 30]\n xs[1] += 5\n xs[2] /= 3\n sum(xs) }", "s", &[], Value::Int(45)),
            // String escape sequences: `\n`/`\t` are one char each, so this is
            // length 3 (was 5 when backslashes were kept verbatim).
            ("f s(){ len(\"a\\nb\") + len(\"x\\ty\") }", "s", &[], Value::Int(6)),
            // `split` on a real tab from an escape, then join with a real newline.
            ("f s(){ len(split(\"a\\tb\\tc\", \"\\t\")) }", "s", &[], Value::Int(3)),
            // Numeric literal formats: hex, binary, octal, digit separators.
            ("f s(){ 0xFF + 0b1010 + 0o17 + 1_000 }", "s", &[], Value::Int(1280)),
            // Float exponents and separators: (1000.5 + 15.0) as i64 = 1015.
            ("f s(){ (1_000.5 + 1.5e1) as i64 }", "s", &[], Value::Int(1015)),
            // Float modulo (incl. mixed Int%Float promotion) and string indexing.
            ("f s(){ ((5.5 % 2.0) + (7 % 2.5)) as i64 }", "s", &[], Value::Int(3)),
            ("f s(){ join([\"hello\"[0], \"world\"[0]], \"\") }", "s", &[], Value::Str("hw".into())),
            // Mixed Int/Float comparison: equality and ordering promote.
            ("f s(){ var n = 0\n if 5 == 5.0 { n += 1 }\n if 2 < 3.5 { n += 1 }\n if 7.0 > 3 { n += 1 }\n n }", "s", &[], Value::Int(3)),
            // `contains` on a string (substring) and a map (key membership).
            ("f s(){ var n = 0\n if contains(\"hello\", \"ell\") { n += 1 }\n if contains(freq(chars(\"aabb\")), \"a\") { n += 10 }\n n }", "s", &[], Value::Int(11)),
            // Iterating a string (chars) and a map (key/value pairs), plus a
            // vocabulary combinator (filter) applied directly to a string.
            ("f s(){ var n = 0\n for c in \"abcde\" { n += 1 }\n n }", "s", &[], Value::Int(5)),
            ("f s(){ var t = 0\n for (k, v) in freq(chars(\"banana\")) { t += v }\n t }", "s", &[], Value::Int(6)),
            ("f s(){ len(filter(\"hello world\", fn(c) => c != \" \")) }", "s", &[], Value::Int(10)),
            // Compound assignment to a map element (`m[k] += …`) — the classic
            // histogram build. `m`/`v` are KwM/KwV but must read as variables.
            ("f s(){ var m = {\"a\": 10}\n m[\"a\"] += 5\n m[\"b\"] = 1\n m[\"a\"] + m[\"b\"] }", "s", &[], Value::Int(16)),
            ("f s(){ var v = [1, 2, 3]\n v[0] *= 100\n v[0] + v[1] }", "s", &[], Value::Int(102)),
            // Slice indexing: list sub-range, open-ended, and substring.
            ("f s(){ sum([10, 20, 30, 40, 50][1..3]) }", "s", &[], Value::Int(50)),
            ("f s(){ sum([1, 2, 3, 4, 5][2..]) + len([1,2,3,4,5][..2]) }", "s", &[], Value::Int(14)),
            ("f s(){ \"hello world\"[0..5] }", "s", &[], Value::Str("hello".into())),
            // Boolean literals (`1b`/`true` were silently false), `!` on a
            // non-bool, and list concatenation with `+`.
            ("f s(){ var n = 0\n if true { n += 1 }\n if 1b { n += 10 }\n if !0 { n += 100 }\n if !false { n += 1000 }\n n }", "s", &[], Value::Int(1111)),
            ("f s(){ sum(fold([1,2,3], [], fn(a, x) => a + [x * x])) + len([1,2] + [3,4,5]) }", "s", &[], Value::Int(19)),
        ];
        let mut ok = 0;
        for (src, f, args, want) in cases {
            match run_source(src, f, args) {
                Ok(v) if v == *want => ok += 1,
                other => println!("  MISS [{f}]: got {other:?}, want {want:?}"),
            }
        }
        println!("\n[eval-bench] correctness: {ok}/{} programs exact", cases.len());

        // Throughput.
        let t = Instant::now();
        let v = run_source("f fib(n){ if n<2 {n} else {fib(n-1)+fib(n-2)} }", "fib", &[28]);
        println!("[eval-bench] fib(28) = {v:?} in {:.1}ms", t.elapsed().as_secs_f64() * 1e3);

        let pipe = "f e(n){n%2==0}\nf d(n){n*2}\nf a(x,y){x+y}\n\
                    f s(){ fold(map(filter(range(1000), e), d), 0, a) }";
        let t = Instant::now();
        let iters = 200;
        let mut last = Value::Unit;
        for _ in 0..iters {
            last = run_source(pipe, "s", &[]).unwrap();
        }
        println!(
            "[eval-bench] vocab pipeline over range(1000) = {last:?}, {:.1}µs/run",
            t.elapsed().as_secs_f64() / iters as f64 * 1e6
        );
        assert_eq!(ok, cases.len(), "all benchmark programs must compute exactly");
    }

    #[test]
    fn closures_work() {
        // MAGE closure syntax is `fn(x) => expr`.
        assert_eq!(run("f s() { map([1, 2, 3], fn(x) => x * 10) }", "s", &[]),
                   Value::List(vec![Value::Int(10), Value::Int(20), Value::Int(30)]));
    }

    #[test]
    fn match_unwraps_option() {
        // The totality loop: a `?A` result is used via match — Some and None.
        let g = "f gt2(n) { n > 2 }\n";
        assert_eq!(
            run(&format!("{g}f s() {{ match find([1,2,3,4], gt2) {{ Some(v) => v, None => 0 }} }}"), "s", &[]),
            Value::Int(3),
        );
        assert_eq!(
            run("f s() { match first([]) { Some(v) => v, None => 42 } }", "s", &[]),
            Value::Int(42),
        );
    }

    #[test]
    fn method_chaining_desugars() {
        // `xs.filter(e).map(d)` == `map(filter(xs, e), d)`.
        let src = "f e(n) { n % 2 == 0 }\nf d(n) { n * 2 }\n\
                   f s() { sum([1,2,3,4,5,6].filter(e).map(d)) }";
        assert_eq!(run(src, "s", &[]), Value::Int(24));
    }

    #[test]
    fn struct_construction_and_field_access() {
        // MAGE struct-literal syntax is `@Name { field: value }`.
        let src = "S P { x: i32, y: i32 }\nf d2(p) { p.x * p.x + p.y * p.y }\n\
                   f s() { d2(@P { x: 3, y: 4 }) }";
        assert_eq!(run(src, "s", &[]), Value::Int(25));
    }
}
