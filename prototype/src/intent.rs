//! What a program says it is for, read from the program itself.
//!
//! The self-healing loop (`self_heal.rs`) may only change a program in ways
//! that keep its intent. "Intent" here is narrow on purpose: it is the part of
//! a program that *states* what the program must do, and nothing inferred from
//! how a name reads or what a comment says. Four sources, each recorded:
//!
//! - **signature**: a function's parameter names and types, its return type,
//!   its generics and `where` clause;
//! - **declared effects**: the `/ io`-style effects written on the signature;
//! - **contracts**: each `@req` / `@ens` on a function or in a `sp` block, and
//!   each `@inv` on a struct, as written;
//! - **tests**: each zero-argument top-level function carrying `@test` (or its
//!   abbreviation `@t`). A test is an executable expectation: it holds when
//!   the fuel-bounded evaluator returns without error.
//!
//! An item records which of these it came from, so a report can say *why* a
//! change was refused, not merely that it was.

use serde::Serialize;

use crate::ast::{ContractClauseKind, FunctionDef, Item, ItemKind, Module};
use crate::fmt::{type_to_string, Mode};

/// Where one piece of intent was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentSource {
    Signature,
    DeclaredEffects,
    Contract,
    StructInvariant,
    TestAttribute,
}

impl IntentSource {
    pub fn label(&self) -> &'static str {
        match self {
            IntentSource::Signature => "signature",
            IntentSource::DeclaredEffects => "declared effects",
            IntentSource::Contract => "contract",
            IntentSource::StructInvariant => "struct invariant",
            IntentSource::TestAttribute => "@test",
        }
    }
}

/// One stated fact about the program, with its source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntentItem {
    /// The function or struct the fact is about.
    pub subject: String,
    pub source: IntentSource,
    /// The fact, as text: a signature, an effect list, a clause as written.
    pub claim: String,
}

/// The intent of one top-level function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FnIntent {
    pub name: String,
    /// `name<G>(a: T, b: U) -> R where …`, rendered by the formatter.
    pub signature: String,
    /// Declared effects, sorted.
    pub effects: Vec<String>,
    /// `@req`/`@ens` clauses as written (`@req(n > 0)`), in source order.
    pub contracts: Vec<String>,
    /// Whether this is a `@test` function (zero arguments, `@test`/`@t`).
    pub is_test: bool,
}

/// The intent of a whole module.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Intent {
    pub functions: Vec<FnIntent>,
    /// `(struct, its @inv clauses as written)`, for structs with any.
    pub invariants: Vec<(String, Vec<String>)>,
    /// Names of the `@test` functions, in source order.
    pub tests: Vec<String>,
    /// Every fact above, flattened, each naming its source.
    pub items: Vec<IntentItem>,
}

/// Whether an item's attributes mark it a test: `@test`, or `@t`, which the
/// elision table expands to `test`.
fn has_test_attribute(item: &Item) -> bool {
    item.attributes.iter().any(|a| {
        a.name == "test" || crate::elision::expand_attribute_name(&a.name) == Some("test")
    })
}

fn signature_of(fd: &FunctionDef) -> String {
    let mut s = fd.name.clone();
    if !fd.generics.is_empty() {
        let gs: Vec<String> = fd
            .generics
            .iter()
            .map(|g| {
                if g.bounds.is_empty() {
                    g.name.clone()
                } else {
                    format!("{}: {}", g.name, g.bounds.join(" + "))
                }
            })
            .collect();
        s.push_str(&format!("<{}>", gs.join(", ")));
    }
    let ps: Vec<String> =
        fd.params.iter().map(|p| format!("{}: {}", p.name, type_to_string(&p.ty, Mode::Human))).collect();
    s.push_str(&format!("({})", ps.join(", ")));
    if let Some(r) = &fd.return_type {
        s.push_str(&format!(" -> {}", type_to_string(r, Mode::Human)));
    }
    if fd.is_async {
        s.insert_str(0, "async ");
    }
    if fd.is_unsafe {
        s.insert_str(0, "unsafe ");
    }
    if !fd.where_clause.is_empty() {
        let ws: Vec<String> =
            fd.where_clause.iter().map(|w| format!("{}: {}", w.type_param, w.bounds.join(" + "))).collect();
        s.push_str(&format!(" where {}", ws.join(", ")));
    }
    s
}

/// Read the intent of `module`.
pub fn infer(module: &Module) -> Intent {
    let mut intent = Intent::default();
    // Contracts by function, from `contract::collect`, which also gathers the
    // clauses of a `sp` block naming the function.
    let mut contracts: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    // A `sp` block's `@inv` names no struct; it is kept with the function the
    // spec is named after.
    for (clause, _) in crate::contract::collect(module) {
        contracts.entry(clause.function.clone()).or_default().push(clause.display());
    }
    for item in &module.items {
        match &item.kind {
            ItemKind::Function(fd) => {
                let is_test = has_test_attribute(item) && fd.params.is_empty();
                let mut effects = fd.effects.clone();
                effects.sort();
                let f = FnIntent {
                    name: fd.name.clone(),
                    signature: signature_of(fd),
                    effects,
                    contracts: contracts.remove(&fd.name).unwrap_or_default(),
                    is_test,
                };
                intent.items.push(IntentItem {
                    subject: f.name.clone(),
                    source: IntentSource::Signature,
                    claim: f.signature.clone(),
                });
                if !f.effects.is_empty() {
                    intent.items.push(IntentItem {
                        subject: f.name.clone(),
                        source: IntentSource::DeclaredEffects,
                        claim: format!("/ {}", f.effects.join(", ")),
                    });
                }
                for c in &f.contracts {
                    intent.items.push(IntentItem {
                        subject: f.name.clone(),
                        source: IntentSource::Contract,
                        claim: c.clone(),
                    });
                }
                if is_test {
                    intent.tests.push(f.name.clone());
                    intent.items.push(IntentItem {
                        subject: f.name.clone(),
                        source: IntentSource::TestAttribute,
                        claim: format!("{}() runs without error", f.name),
                    });
                }
                intent.functions.push(f);
            }
            ItemKind::Struct(sd) => {
                let invs: Vec<String> = sd
                    .contracts
                    .iter()
                    .filter(|c| c.kind == ContractClauseKind::Invariant)
                    .map(|c| match &c.message {
                        Some(m) => format!("@inv({}, \"{m}\")", c.condition),
                        None => format!("@inv({})", c.condition),
                    })
                    .collect();
                if !invs.is_empty() {
                    for c in &invs {
                        intent.items.push(IntentItem {
                            subject: sd.name.clone(),
                            source: IntentSource::StructInvariant,
                            claim: c.clone(),
                        });
                    }
                    intent.invariants.push((sd.name.clone(), invs));
                }
            }
            _ => {}
        }
    }
    // Clauses of a `sp` block with no function of its name: still stated
    // intent, kept so that deleting them is seen.
    let mut orphans: Vec<(String, Vec<String>)> = contracts.into_iter().collect();
    orphans.sort();
    for (name, cs) in orphans {
        for c in &cs {
            intent.items.push(IntentItem { subject: name.clone(), source: IntentSource::Contract, claim: c.clone() });
        }
        intent.invariants.push((format!("sp {name}"), cs));
    }
    intent
}

/// Read the intent of `source`, or `None` if it does not parse.
pub fn infer_source(source: &str) -> Option<Intent> {
    let module = crate::parser::parse(&crate::lexer::lex(source)).ok()?;
    Some(infer(&module))
}

impl Intent {
    /// Every way `after` fails to keep this intent, one sentence each; empty
    /// when it keeps all of it.
    ///
    /// Kept: every function still exists with the same signature, declared
    /// effects and contracts; every struct invariant is unchanged; no `@test`
    /// was removed. Functions `after` adds are not a violation.
    pub fn violations_in(&self, after: &Intent) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.functions {
            let Some(g) = after.functions.iter().find(|g| g.name == f.name) else {
                out.push(format!("function `{}` was removed", f.name));
                continue;
            };
            if f.signature != g.signature {
                out.push(format!("signature of `{}` changed: `{}` -> `{}`", f.name, f.signature, g.signature));
            }
            if f.effects != g.effects {
                out.push(format!(
                    "declared effects of `{}` changed: [{}] -> [{}]",
                    f.name,
                    f.effects.join(", "),
                    g.effects.join(", ")
                ));
            }
            if f.contracts != g.contracts {
                out.push(format!("contracts of `{}` changed", f.name));
            }
        }
        for (s, invs) in &self.invariants {
            match after.invariants.iter().find(|(t, _)| t == s) {
                Some((_, after_invs)) if after_invs == invs => {}
                Some(_) => out.push(format!("invariants of `{s}` changed")),
                None => out.push(format!("invariants of `{s}` were removed")),
            }
        }
        for t in &self.tests {
            if !after.tests.contains(t) {
                out.push(format!("@test `{t}` was removed"));
            }
        }
        out
    }

    /// One line per function, then one per struct invariant set.
    pub fn summary(&self) -> Vec<String> {
        let mut out = Vec::new();
        for f in &self.functions {
            let mut line = format!("{}f {}", if f.is_test { "@test " } else { "" }, f.signature);
            if !f.effects.is_empty() {
                line.push_str(&format!(" / {}", f.effects.join(", ")));
            }
            for c in &f.contracts {
                line.push_str(&format!(" {c}"));
            }
            out.push(line);
        }
        for (s, invs) in &self.invariants {
            out.push(format!("S {s} {}", invs.join(" ")));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(src: &str) -> Intent {
        infer_source(src).expect("parses")
    }

    #[test]
    fn intent_records_signature_effects_contracts_and_tests() {
        let i = intent(
            "@req(n >= 0)\n@ens(result >= n)\nf inc(n: i64) -> i64 { n + 1 }\n\
             f say() -> i64 / io { 1 }\n\
             @t\nf t_inc() { assert(inc(1) == 2, \"inc\") }\n\
             @test\nf t_say() { assert(say() == 1, \"say\") }\n",
        );
        let inc = i.functions.iter().find(|f| f.name == "inc").unwrap();
        assert_eq!(inc.signature, "inc(n: i64) -> i64");
        assert_eq!(inc.contracts, vec!["@req(n >= 0)", "@ens(result >= n)"]);
        let say = i.functions.iter().find(|f| f.name == "say").unwrap();
        assert_eq!(say.effects.len(), 1, "{:?}", say.effects);
        assert_eq!(i.tests, vec!["t_inc", "t_say"]);
        for src in [
            IntentSource::Signature,
            IntentSource::DeclaredEffects,
            IntentSource::Contract,
            IntentSource::TestAttribute,
        ] {
            assert!(i.items.iter().any(|it| it.source == src), "no item from {src:?}");
        }
    }

    #[test]
    fn intent_records_struct_invariants() {
        let i = intent("@inv(self.n >= 0)\nS Ctr { n: i64 }\n");
        assert_eq!(i.invariants.len(), 1, "{i:?}");
        assert!(i.items.iter().any(|it| it.source == IntentSource::StructInvariant));
    }

    /// A function whose name reads like a test is not one: intent comes from
    /// the attribute, never from the name.
    #[test]
    fn intent_does_not_read_names() {
        let i = intent("f test_thing() { assert(1b, \"x\") }\n@t\nf takes(a: i64) { }\n");
        assert!(i.tests.is_empty(), "{:?}", i.tests);
    }

    #[test]
    fn intent_violations_name_what_changed() {
        let a = intent("f g(a: i64) -> i64 { a }\n@t\nf t() { assert(g(1) == 1, \"g\") }\n");
        let b = intent("f g(a: i64) -> usize { 1 }\n");
        let v = a.violations_in(&b);
        assert!(v.iter().any(|s| s.contains("signature of `g`")), "{v:?}");
        assert!(v.iter().any(|s| s.contains("@test `t` was removed")), "{v:?}");
        assert!(a.violations_in(&a).is_empty());
    }
}
