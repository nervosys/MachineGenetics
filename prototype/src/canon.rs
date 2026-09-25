//! Content addressing: a definition's identity is the hash of its normal form.
//!
//! Two hashes, answering two questions:
//!
//! * [`definition_hash`] — *is this the same program?* Bound variables are
//!   renamed `_0`, `_1`, … in binding order, and the function's own name is
//!   dropped, so `|x| x + 1` and `|y| y + 1` are one definition, and so are
//!   two functions that differ only in what they are called. Everything else
//!   — literals, operators, the names of free functions and fields — is part
//!   of identity.
//! * [`shape_hash`] — *is this the same idea?* The definition hash with every
//!   literal's value erased (its kind is kept). `[s; 4]` and `[s; 7]` are two
//!   definitions and one shape.
//!
//! The shape hash is what the arena measures novelty on. Its generators found
//! a way to be "new" without being new: `[s; k]` is a constant run whose value
//! comes from the seed, so every seed produced bytes the novelty count had
//! never seen, and the reward paid for the same program again and again. A
//! program's shape does not depend on its input or its constants.
//!
//! ## How it is computed
//!
//! Over the AST's serde form, not the AST types: every enum is internally
//! tagged (`"type": "Closure"`), blocks are `{stmts, tail_expr}`, and a walk
//! that knows the binding forms can ignore every field it does not care about
//! — so a new AST field changes hashes (as a semantic change should) without
//! needing an edit here. The binding forms handled: function parameters,
//! closure parameters, `let` (scoped to the rest of its block, value walked
//! first so `v x = x + 1` refers to the outer `x`), `for`, and `match` arms.
//! Struct-field shorthand `P { x }` binds `x` while keeping the field name.
//!
//! Not handled, and deterministic regardless: effect-handler operation
//! parameters and generic type parameters keep their names. Two definitions
//! differing only there hash differently, which is conservative — it can miss
//! an equivalence, never invent one.

use crate::ast::FunctionDef;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The normal form of `fd`, with literals kept (`shape == false`) or erased.
pub fn canonical(fd: &FunctionDef, shape: bool) -> Value {
    let mut v = serde_json::to_value(fd).expect("the AST serialises");
    if let Value::Object(m) = &mut v {
        m.insert("name".into(), Value::String(String::new()));
    }
    let mut c = Canon { scopes: vec![Vec::new()], next: 0, shape };
    if let Value::Object(m) = &mut v {
        if let Some(Value::Array(params)) = m.get_mut("params") {
            for p in params {
                if let Some(Value::String(name)) = p.get_mut("name") {
                    *name = c.bind(name);
                }
            }
        }
        for (k, child) in m.iter_mut() {
            if k != "params" {
                c.walk(child);
            }
        }
    }
    v
}

/// SHA-256 of the normal form, literals kept.
pub fn definition_hash(fd: &FunctionDef) -> String {
    digest(&canonical(fd, false))
}

/// SHA-256 of the normal form with literal values erased.
pub fn shape_hash(fd: &FunctionDef) -> String {
    digest(&canonical(fd, true))
}

fn digest(v: &Value) -> String {
    format!("{:x}", Sha256::digest(v.to_string().as_bytes()))
}

struct Canon {
    /// Innermost last: (source name, canonical name).
    scopes: Vec<Vec<(String, String)>>,
    next: usize,
    shape: bool,
}

impl Canon {
    fn bind(&mut self, name: &str) -> String {
        let canon = format!("_{}", self.next);
        self.next += 1;
        self.scopes.last_mut().expect("a scope").push((name.to_string(), canon.clone()));
        canon
    }

    fn lookup(&self, name: &str) -> Option<&str> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|(n, _)| n == name)
            .map(|(_, c)| c.as_str())
    }

    fn scoped(&mut self, f: impl FnOnce(&mut Self)) {
        self.scopes.push(Vec::new());
        f(self);
        self.scopes.pop();
    }

    /// Bind every name a pattern introduces, renaming it in place.
    fn bind_pattern(&mut self, p: &mut Value) {
        match p {
            Value::Object(m) => {
                if m.get("type").and_then(Value::as_str) == Some("Ident") {
                    if let Some(Value::String(name)) = m.get_mut("name") {
                        *name = self.bind(name);
                    }
                    return;
                }
                // Field shorthand `P { x }`: binds `x`, keeps the field name.
                if m.contains_key("name") && m.get("pattern") == Some(&Value::Null) {
                    let field = m.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
                    let canon = self.bind(&field);
                    m.insert("pattern".into(), ident(canon));
                    return;
                }
                if let Some(Value::String(rest)) = m.get_mut("rest_name") {
                    *rest = self.bind(rest);
                }
                if self.shape && m.get("type").and_then(Value::as_str) == Some("Literal") {
                    m.insert("value".into(), Value::String(String::new()));
                }
                for (k, child) in m.iter_mut() {
                    if k != "rest_name" && k != "name" {
                        self.bind_pattern(child);
                    }
                }
            }
            Value::Array(xs) => xs.iter_mut().for_each(|x| self.bind_pattern(x)),
            _ => {}
        }
    }

    fn walk(&mut self, v: &mut Value) {
        match v {
            Value::Array(xs) => xs.iter_mut().for_each(|x| self.walk(x)),
            Value::Object(m) => {
                let ty = m.get("type").and_then(Value::as_str).map(str::to_string);
                match ty.as_deref() {
                    Some("Ident") => {
                        if let Some(Value::String(name)) = m.get_mut("name") {
                            if let Some(c) = self.lookup(name) {
                                *name = c.to_string();
                            }
                        }
                    }
                    Some("Literal") => {
                        if self.shape {
                            m.insert("value".into(), Value::String(String::new()));
                        }
                    }
                    Some("Closure") => self.scoped(|c| {
                        if let Some(Value::Array(params)) = m.get_mut("params") {
                            for p in params {
                                if let Some(Value::String(name)) = p.get_mut("name") {
                                    *name = c.bind(name);
                                }
                            }
                        }
                        walk_except(c, m, &["params"]);
                    }),
                    Some("For") => {
                        if let Some(iter) = m.get_mut("iter") {
                            self.walk(iter);
                        }
                        self.scoped(|c| {
                            if let Some(p) = m.get_mut("pattern") {
                                c.bind_pattern(p);
                            }
                            walk_except(c, m, &["iter", "pattern"]);
                        });
                    }
                    Some("Match") => {
                        if let Some(s) = m.get_mut("scrutinee") {
                            self.walk(s);
                        }
                        if let Some(Value::Array(arms)) = m.get_mut("arms") {
                            for arm in arms {
                                self.scoped(|c| {
                                    if let Some(p) = arm.get_mut("pattern") {
                                        c.bind_pattern(p);
                                    }
                                    if let Value::Object(am) = arm {
                                        walk_except(c, am, &["pattern"]);
                                    }
                                });
                            }
                        }
                        walk_except(self, m, &["scrutinee", "arms"]);
                    }
                    _ if m.contains_key("stmts") && !m.contains_key("type") => self.scoped(|c| {
                        if let Some(Value::Array(stmts)) = m.get_mut("stmts") {
                            for st in stmts {
                                let is_let = st.get("type").and_then(Value::as_str) == Some("Let");
                                if is_let {
                                    if let Value::Object(sm) = st {
                                        walk_except(c, sm, &["pattern"]);
                                        if let Some(p) = sm.get_mut("pattern") {
                                            c.bind_pattern(p);
                                        }
                                    }
                                } else {
                                    c.walk(st);
                                }
                            }
                        }
                        walk_except(c, m, &["stmts"]);
                    }),
                    _ => walk_except(self, m, &[]),
                }
            }
            _ => {}
        }
    }
}

fn walk_except(c: &mut Canon, m: &mut Map<String, Value>, skip: &[&str]) {
    for (k, child) in m.iter_mut() {
        if !skip.contains(&k.as_str()) {
            c.walk(child);
        }
    }
}

fn ident(name: String) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String("Ident".into()));
    m.insert("name".into(), Value::String(name));
    Value::Object(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn func(src: &str) -> FunctionDef {
        let module = crate::parser::parse(&crate::lexer::lex(src)).expect("parses");
        module
            .items
            .into_iter()
            .find_map(|i| match i.kind {
                crate::ast::ItemKind::Function(f) => Some(f),
                _ => None,
            })
            .expect("a function")
    }

    fn same(a: &str, b: &str) -> bool {
        definition_hash(&func(a)) == definition_hash(&func(b))
    }

    #[test]
    fn alpha_equivalent_definitions_hash_equal() {
        assert!(same(
            "f g(s: usize) -> [usize] { range(3).map(|x| x + s) }",
            "f other(t: usize) -> [usize] { range(3).map(|y| y + t) }",
        ));
        assert!(same(
            "f g(a: i64) -> i64 { v b = a + 1\n b * b }",
            "f g(p: i64) -> i64 { v q = p + 1\n q * q }",
        ));
        assert!(same(
            "f g(xs: [i64]) -> i64 { m t = 0\n for x in xs { t = t + x }\n t }",
            "f g(ys: [i64]) -> i64 { m acc = 0\n for y in ys { acc = acc + y }\n acc }",
        ));
    }

    #[test]
    fn different_definitions_hash_differently() {
        // A different literal, operator, free function or structure is a
        // different definition.
        let base = "f g(s: usize) -> [usize] { range(3).map(|x| x + s) }";
        for other in [
            "f g(s: usize) -> [usize] { range(4).map(|x| x + s) }",
            "f g(s: usize) -> [usize] { range(3).map(|x| x * s) }",
            "f g(s: usize) -> [usize] { range(3).map(|x| x + s).reverse() }",
            "f g(s: usize) -> [usize] { range(3).map(|x| s + x) }",
        ] {
            assert!(!same(base, other), "{other}");
        }
    }

    #[test]
    fn shadowing_and_capture_are_respected() {
        // `v x = x + 1` reads the *outer* x. Renaming must not merge it with a
        // program where the value reads the new binding (which would not even
        // be well-scoped), nor confuse a free name with a bound one.
        assert!(!same(
            "f g(x: i64) -> i64 { v y = x + 1\n y }",
            "f g(x: i64) -> i64 { v y = z + 1\n y }",
        ));
        assert!(same(
            "f g(x: i64) -> i64 { v x = x + 1\n x }",
            "f g(a: i64) -> i64 { v b = a + 1\n b }",
        ));
    }

    #[test]
    fn shape_erases_literals_and_nothing_else() {
        let shape = |s: &str| shape_hash(&func(s));
        // The arena's exploit: one shape for every constant.
        assert_eq!(
            shape("f gen(s: usize) -> [usize] { [s; 4] }"),
            shape("f gen(s: usize) -> [usize] { [s; 7] }")
        );
        assert_ne!(
            definition_hash(&func("f gen(s: usize) -> [usize] { [s; 4] }")),
            definition_hash(&func("f gen(s: usize) -> [usize] { [s; 7] }"))
        );
        // Structure still distinguishes shapes.
        assert_ne!(
            shape("f gen(s: usize) -> [usize] { [s; 4] }"),
            shape("f gen(s: usize) -> [usize] { range(4) }")
        );
    }

    #[test]
    fn hashing_is_deterministic() {
        let src = "f g(s: usize) -> [usize] { range(3).scan(s, |a, x| a + x).filter(|v| v % 2 == 0) }";
        let h = definition_hash(&func(src));
        for _ in 0..10 {
            assert_eq!(definition_hash(&func(src)), h);
        }
        assert_eq!(h.len(), 64);
    }
}
