/// MAGE Logic Inference Engine — Datalog-style reasoning for KB blocks.
///
/// Compiles `kb` definitions into an in-memory fact store with
/// bottom-up, semi-naive evaluation of rules, one stratum at a time.
///
/// Supports:
///   - Ground facts: `fact parent("alice", "bob");`
///   - Rules whose body is a conjunction of predicate calls:
///     `rule ancestor(x: str, z: str) { parent(x, y) && ancestor(y, z) }`
///   - Stratified negation: `rule orphan(x: str) { person(x) && !parent(y, x) }`
///   - Queries: `kb.query("ancestor", &["alice", "?"])`
///
/// **What a rule means (MAGE_SPEC §7.1).** Every identifier in a rule is a
/// logic variable — the head's parameters and any name that appears only in
/// the body — and constants are string or number literals. The body is the
/// rule's `where` conditions and its block, joined by `&&`. A rule is refused
/// (E0570) rather than dropped when its body holds something that is not a
/// predicate call, when a head parameter or a negated variable is not bound by
/// a positive literal, or when its negation cannot be stratified.
///
/// This used to read only the `where` conditions, ignore the block, and treat
/// a lowercase parameter as a constant, so the spec's own example
/// `rule integer(t: str) { numeric(t) }` derived nothing, and a fact argument
/// it could not read was dropped, changing the fact's arity.
///
/// The engine generates MLIR-compatible operation sequences for
/// materializing the knowledge base at compile time.
use crate::ast;
use crate::hir::{Diagnostic, DiagnosticCategory, Severity};
use std::collections::{BTreeSet, HashMap, HashSet};

// ── Terms and atoms ─────────────────────────────────────────────────

/// A term in a logic formula.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Term {
    /// A constant value (ground).
    Const(String),
    /// A logic variable.
    Var(String),
}

impl Term {
    pub fn is_var(&self) -> bool {
        matches!(self, Term::Var(_))
    }
}

/// An atom: predicate(term, term, ...).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Atom {
    pub predicate: String,
    pub args: Vec<Term>,
}

impl std::fmt::Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let args: Vec<String> = self.args.iter().map(|t| match t {
            Term::Const(c) => format!("\"{c}\""),
            Term::Var(v) => v.clone(),
        }).collect();
        write!(f, "{}({})", self.predicate, args.join(", "))
    }
}

/// A ground atom (all terms are constants).
pub type GroundAtom = Atom;

/// A body literal: an atom, possibly negated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Literal {
    pub atom: Atom,
    pub negated: bool,
}

impl Literal {
    /// A positive literal.
    pub fn pos(atom: Atom) -> Self {
        Literal { atom, negated: false }
    }

    /// A negated literal: true when no fact matches the atom.
    pub fn neg(atom: Atom) -> Self {
        Literal { atom, negated: true }
    }
}

impl std::fmt::Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.negated {
            write!(f, "!{}", self.atom)
        } else {
            write!(f, "{}", self.atom)
        }
    }
}

/// A rule: head :- body₁, body₂, ..., bodyₙ
#[derive(Debug, Clone)]
pub struct Rule {
    pub head: Atom,
    pub body: Vec<Literal>,
}

// ── Substitution (variable bindings) ────────────────────────────────

type Binding = HashMap<String, String>;

fn apply_binding(term: &Term, binding: &Binding) -> Term {
    match term {
        Term::Var(v) => {
            if let Some(val) = binding.get(v) {
                Term::Const(val.clone())
            } else {
                term.clone()
            }
        }
        Term::Const(_) => term.clone(),
    }
}

fn apply_binding_atom(atom: &Atom, binding: &Binding) -> Atom {
    Atom {
        predicate: atom.predicate.clone(),
        args: atom.args.iter().map(|t| apply_binding(t, binding)).collect(),
    }
}

fn is_ground(atom: &Atom) -> bool {
    atom.args.iter().all(|t| matches!(t, Term::Const(_)))
}

fn vars_of(atom: &Atom) -> impl Iterator<Item = &str> {
    atom.args.iter().filter_map(|t| match t {
        Term::Var(v) => Some(v.as_str()),
        Term::Const(_) => None,
    })
}

// ── Unification ─────────────────────────────────────────────────────

/// Try to unify an atom with a ground fact's arguments, returning bindings if
/// successful.
fn unify_args(pattern: &Atom, fact: &[String], binding: &Binding) -> Option<Binding> {
    if pattern.args.len() != fact.len() {
        return None;
    }

    let mut result = binding.clone();
    for (pat, val) in pattern.args.iter().zip(fact) {
        match apply_binding(pat, &result) {
            Term::Const(a) => {
                if &a != val {
                    return None;
                }
            }
            Term::Var(v) => {
                result.insert(v, val.clone());
            }
        }
    }
    Some(result)
}

// ── Knowledge base ──────────────────────────────────────────────────

type Relation = BTreeSet<Vec<String>>;

/// An in-memory knowledge base with facts and rules.
pub struct KnowledgeBase {
    pub name: String,
    /// Ground facts, indexed by predicate name.
    facts: HashMap<String, Relation>,
    /// Rules for deriving new facts.
    rules: Vec<Rule>,
    /// Whether fixpoint has been reached.
    materialized: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl KnowledgeBase {
    pub fn new(name: &str) -> Self {
        KnowledgeBase {
            name: name.to_string(),
            facts: HashMap::new(),
            rules: Vec::new(),
            materialized: false,
            diagnostics: Vec::new(),
        }
    }

    /// Add a ground fact.
    pub fn add_fact(&mut self, predicate: &str, args: Vec<String>) {
        self.facts.entry(predicate.to_string()).or_default().insert(args);
        self.materialized = false;
    }

    /// Add a rule. It is checked for safety when the base is materialized; an
    /// unsafe rule is reported there and derives nothing.
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
        self.materialized = false;
    }

    /// Get all facts for a predicate.
    pub fn get_facts(&self, predicate: &str) -> Vec<Vec<String>> {
        self.facts
            .get(predicate)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Total number of ground facts across all predicates.
    pub fn fact_count(&self) -> usize {
        self.facts.values().map(|s| s.len()).sum()
    }

    fn error(&mut self, msg: String) {
        self.diagnostics.push(Diagnostic::categorized(
            Severity::Error,
            msg,
            DiagnosticCategory::KbRule,
            None,
        ));
    }

    /// The reason a rule cannot be evaluated, if any: a head variable or a
    /// negated variable that no positive literal binds. Without that, the
    /// rule's results would depend on the whole universe of constants.
    fn unsafe_reason(rule: &Rule) -> Option<String> {
        let bound: HashSet<&str> = rule
            .body
            .iter()
            .filter(|l| !l.negated)
            .flat_map(|l| vars_of(&l.atom))
            .collect();
        if let Some(v) = vars_of(&rule.head).find(|v| !bound.contains(v)) {
            return Some(format!(
                "head parameter `{v}` is not bound by any positive literal in the body"
            ));
        }
        for lit in rule.body.iter().filter(|l| l.negated) {
            if let Some(v) = vars_of(&lit.atom).find(|v| !bound.contains(v)) {
                return Some(format!(
                    "`{v}` in `{lit}` is not bound by any positive literal, so the negation has no meaning"
                ));
            }
        }
        None
    }

    /// Assign each derived predicate a stratum, so that a predicate is
    /// computed only after everything it negates is complete. `Err` names a
    /// predicate that depends negatively on itself.
    fn strata(rules: &[&Rule]) -> Result<HashMap<String, usize>, String> {
        let mut stratum: HashMap<String, usize> =
            rules.iter().map(|r| (r.head.predicate.clone(), 0)).collect();
        let limit = stratum.len();
        loop {
            let mut changed = false;
            for rule in rules {
                for lit in &rule.body {
                    let below = stratum.get(&lit.atom.predicate).copied().unwrap_or(0);
                    let need = if lit.negated { below + 1 } else { below };
                    let head = stratum.get_mut(&rule.head.predicate).expect("every head has a stratum");
                    if *head < need {
                        if need > limit {
                            return Err(rule.head.predicate.clone());
                        }
                        *head = need;
                        changed = true;
                    }
                }
            }
            if !changed {
                return Ok(stratum);
            }
        }
    }

    /// Evaluate all rules to fixpoint, stratum by stratum, semi-naively. A
    /// rule that is unsafe is reported and skipped; if the rules' negation
    /// cannot be stratified, it is reported and no rule is evaluated.
    pub fn materialize(&mut self) {
        if self.materialized {
            return;
        }
        self.materialized = true;

        let rules = std::mem::take(&mut self.rules);
        let mut usable: Vec<&Rule> = Vec::new();
        for rule in &rules {
            match Self::unsafe_reason(rule) {
                Some(why) => {
                    let name = self.name.clone();
                    self.error(format!(
                        "kb `{name}`: rule `{}` cannot be evaluated: {why}",
                        rule.head.predicate
                    ));
                }
                None => usable.push(rule),
            }
        }

        match Self::strata(&usable) {
            Err(pred) => {
                let name = self.name.clone();
                self.error(format!(
                    "kb `{name}`: `{pred}` depends on its own negation, so its rules cannot be \
                     stratified and none was evaluated"
                ));
            }
            Ok(stratum) => {
                let top = stratum.values().copied().max().unwrap_or(0);
                for level in 0..=top {
                    let layer: Vec<&Rule> = usable
                        .iter()
                        .copied()
                        .filter(|r| stratum[&r.head.predicate] == level)
                        .collect();
                    self.fixpoint(&layer);
                }
            }
        }

        self.rules = rules;
    }

    /// Semi-naive fixpoint of one stratum. After the first round, a rule fires
    /// only through at least one fact derived in the previous round. Datalog
    /// has no function symbols, so the set of possible facts is finite and
    /// this terminates.
    fn fixpoint(&mut self, rules: &[&Rule]) {
        let mut delta: Option<HashMap<String, Relation>> = None;
        loop {
            let mut round: HashMap<String, Relation> = HashMap::new();
            for rule in rules {
                for args in self.evaluate_rule(rule, delta.as_ref()) {
                    let known = self
                        .facts
                        .get(&rule.head.predicate)
                        .is_some_and(|r| r.contains(&args));
                    if !known {
                        round.entry(rule.head.predicate.clone()).or_default().insert(args);
                    }
                }
            }
            if round.values().all(|r| r.is_empty()) {
                return;
            }
            for (pred, rel) in &round {
                self.facts.entry(pred.clone()).or_default().extend(rel.iter().cloned());
            }
            delta = Some(round);
        }
    }

    /// Evaluate a single rule against current facts. With `delta`, only
    /// derivations that use at least one fact from it are produced.
    fn evaluate_rule(&self, rule: &Rule, delta: Option<&HashMap<String, Relation>>) -> Vec<Vec<String>> {
        let positive: Vec<&Atom> = rule.body.iter().filter(|l| !l.negated).map(|l| &l.atom).collect();
        let negative: Vec<&Atom> = rule.body.iter().filter(|l| l.negated).map(|l| &l.atom).collect();
        let empty = Relation::new();
        let full = |p: &str| self.facts.get(p).unwrap_or(&empty);

        // Each pass `d` takes literal `d` from the delta and the rest from the
        // full relation; without a delta there is one pass over the full set.
        let passes: Vec<Option<usize>> = match delta {
            None => vec![None],
            Some(d) => (0..positive.len())
                .filter(|&i| d.get(&positive[i].predicate).is_some_and(|r| !r.is_empty()))
                .map(Some)
                .collect(),
        };

        let mut results = Vec::new();
        for pass in passes {
            let mut bindings = vec![Binding::new()];
            for (i, atom) in positive.iter().enumerate() {
                let rel = match (pass, delta) {
                    (Some(d), Some(dm)) if d == i => dm.get(&atom.predicate).unwrap_or(&empty),
                    _ => full(&atom.predicate),
                };
                let mut next = Vec::new();
                for binding in &bindings {
                    for fact in rel {
                        if let Some(b) = unify_args(atom, fact, binding) {
                            next.push(b);
                        }
                    }
                }
                bindings = next;
                if bindings.is_empty() {
                    break;
                }
            }
            for binding in &bindings {
                let blocked = negative.iter().any(|atom| {
                    let ground = apply_binding_atom(atom, binding);
                    let args: Vec<String> = ground.args.iter().map(|t| match t {
                        Term::Const(c) => c.clone(),
                        Term::Var(_) => unreachable!("a safe rule binds every negated variable"),
                    }).collect();
                    full(&atom.predicate).contains(&args)
                });
                if blocked {
                    continue;
                }
                let derived = apply_binding_atom(&rule.head, binding);
                if is_ground(&derived) {
                    results.push(derived.args.into_iter().map(|t| match t {
                        Term::Const(c) => c,
                        Term::Var(_) => unreachable!("checked ground"),
                    }).collect());
                }
            }
        }
        results
    }

    /// Query the knowledge base. `"?"` in args means wildcard.
    pub fn query(&mut self, predicate: &str, args: &[&str]) -> Vec<Vec<String>> {
        self.materialize();

        let all_facts = self.get_facts(predicate);
        all_facts
            .into_iter()
            .filter(|fact_args| {
                if fact_args.len() != args.len() {
                    return false;
                }
                args.iter().zip(fact_args.iter()).all(|(q, f)| *q == "?" || *q == f)
            })
            .collect()
    }

    /// Generate MLIR operations for materializing this KB.
    pub fn emit_mlir(&mut self) -> Vec<String> {
        self.materialize();

        let mut ops = Vec::new();
        ops.push(format!("MAGE.kb @{} {{", self.name));

        for (pred, fact_set) in &self.facts {
            for args in fact_set {
                let formatted: Vec<String> = args.iter().map(|a| format!("\"{a}\"")).collect();
                ops.push(format!(
                    "  MAGE.kb.fact \"{pred}\"({}) : ()",
                    formatted.join(", ")
                ));
            }
        }

        for (i, rule) in self.rules.iter().enumerate() {
            let head_str = format!("{}", rule.head);
            let body_strs: Vec<String> = rule.body.iter().map(|l| format!("{l}")).collect();
            ops.push(format!(
                "  MAGE.kb.rule @rule_{i} \"{head_str}\" :- {}",
                body_strs.join(", ")
            ));
        }

        ops.push("}".to_string());
        ops
    }
}

// ── AST → KB builder ───────────────────────────────────────────────

/// A constant from a fact argument or a rule literal: a string (unquoted),
/// a number or `true`/`false`, or a negated number.
fn literal_constant(expr: &ast::Expr) -> Option<String> {
    match expr {
        ast::Expr::Literal { value, kind: ast::LiteralKind::String } => {
            Some(value.trim_matches('"').to_string())
        }
        ast::Expr::Literal { value, .. } => Some(value.clone()),
        ast::Expr::Unary { op, operand } if op == "-" => match operand.as_ref() {
            ast::Expr::Literal { value, kind } if !matches!(kind, ast::LiteralKind::String) => {
                Some(format!("-{value}"))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Build a KnowledgeBase from an AST KbDef. Anything that cannot be read as a
/// fact or a rule is reported as an error (E0570) in `diagnostics` and left
/// out whole — a fact is never stored with fewer arguments than it was written
/// with.
pub fn build_kb(def: &ast::KbDef) -> KnowledgeBase {
    let mut kb = KnowledgeBase::new(&def.name);

    for fact in &def.facts {
        let mut args = Vec::with_capacity(fact.args.len());
        let mut bad = None;
        for (i, e) in fact.args.iter().enumerate() {
            match e {
                ast::Expr::Ident { name } => args.push(name.clone()),
                _ => match literal_constant(e) {
                    Some(c) => args.push(c),
                    None => {
                        bad = Some(i + 1);
                        break;
                    }
                },
            }
        }
        match bad {
            None => kb.add_fact(&fact.name, args),
            Some(n) => kb.error(format!(
                "kb `{}`: argument {n} of fact `{}` is not a constant; a fact's arguments are \
                 names, strings or numbers",
                def.name, fact.name
            )),
        }
    }

    for rule in &def.rules {
        match rule_of(rule) {
            Ok(r) => kb.add_rule(r),
            Err(why) => kb.error(format!(
                "kb `{}`: rule `{}` cannot be read: {why}",
                def.name, rule.name
            )),
        }
    }

    kb
}

/// Build and materialize every `kb` block in `module`, returning what the
/// engine reported. `--check` and `--check --json` call this: until
/// 2026-10-29 only `--pipeline` ran the logic engine, so a rule that could not
/// be evaluated passed `--check` with `Errors: 0`.
pub fn module_diagnostics(module: &ast::Module) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for item in &module.items {
        if let ast::ItemKind::Kb(def) = &item.kind {
            let mut kb = build_kb(def);
            kb.materialize();
            out.append(&mut kb.diagnostics);
        }
    }
    out
}

/// The body expressions of a rule: its `where` conditions, then its block's
/// statements and tail, each split at `&&`.
fn rule_body_exprs(rule: &ast::RuleDef) -> Vec<&ast::Expr> {
    fn conjuncts<'a>(e: &'a ast::Expr, out: &mut Vec<&'a ast::Expr>) {
        match e {
            ast::Expr::Binary { op, left, right } if op == "&&" => {
                conjuncts(left, out);
                conjuncts(right, out);
            }
            _ => out.push(e),
        }
    }
    let mut out = Vec::new();
    for c in &rule.conditions {
        conjuncts(c, &mut out);
    }
    for stmt in &rule.body.stmts {
        if let ast::Stmt::Expr { expr } = stmt {
            conjuncts(expr, &mut out);
        }
    }
    if let Some(tail) = &rule.body.tail_expr {
        conjuncts(tail, &mut out);
    }
    out
}

/// Read a rule's head and body literals. Every identifier is a variable.
fn rule_of(rule: &ast::RuleDef) -> Result<Rule, String> {
    if rule.body.stmts.iter().any(|s| !matches!(s, ast::Stmt::Expr { .. })) {
        return Err("its body holds a statement; a rule body is predicate calls joined by `&&`".into());
    }
    let head = Atom {
        predicate: rule.name.clone(),
        args: rule.params.iter().map(|p| Term::Var(p.name.clone())).collect(),
    };
    let mut body = Vec::new();
    let mut fresh = 0usize;
    for e in rule_body_exprs(rule) {
        let (call, negated) = match e {
            ast::Expr::Unary { op, operand } if op == "!" => (operand.as_ref(), true),
            _ => (e, false),
        };
        let atom = atom_of(call, &mut fresh).ok_or_else(|| {
            "its body holds something that is not a predicate call; write each condition as \
             `p(x, ..)` or `!p(x, ..)`, joined by `&&`"
                .to_string()
        })?;
        body.push(Literal { atom, negated });
    }
    if body.is_empty() {
        return Err("its body has no predicate calls".into());
    }
    Ok(Rule { head, body })
}

/// Read `p(a, ..)` as an atom: identifiers are variables (`_` a fresh one
/// each time), literals are constants.
fn atom_of(expr: &ast::Expr, fresh: &mut usize) -> Option<Atom> {
    let ast::Expr::Call { func, args } = expr else { return None };
    let ast::Expr::Ident { name: predicate } = func.as_ref() else { return None };
    let mut terms = Vec::with_capacity(args.len());
    for a in args {
        terms.push(match a {
            ast::Expr::Ident { name } if name == "_" => {
                *fresh += 1;
                Term::Var(format!("_{fresh}"))
            }
            ast::Expr::Ident { name } => Term::Var(name.clone()),
            _ => Term::Const(literal_constant(a)?),
        });
    }
    Some(Atom { predicate: predicate.clone(), args: terms })
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(p: &str, args: &[&str]) -> Atom {
        Atom {
            predicate: p.into(),
            args: args
                .iter()
                .map(|a| if a.starts_with(char::is_uppercase) { Term::Var(a.to_string()) } else { Term::Const(a.to_string()) })
                .collect(),
        }
    }

    /// Parse `src` (one `kb` block) and build it.
    fn kb_of(src: &str) -> KnowledgeBase {
        let module = crate::parser::parse(&crate::lexer::lex(src)).expect("parses");
        let def = module
            .items
            .iter()
            .find_map(|i| match &i.kind {
                ast::ItemKind::Kb(k) => Some(k),
                _ => None,
            })
            .expect("a kb block");
        build_kb(def)
    }

    fn errors(kb: &KnowledgeBase) -> Vec<String> {
        kb.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.message.clone())
            .collect()
    }

    #[test]
    fn simple_fact_query() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("parent", vec!["alice".into(), "bob".into()]);
        kb.add_fact("parent", vec!["bob".into(), "charlie".into()]);

        let results = kb.query("parent", &["alice", "?"]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], vec!["alice", "bob"]);
    }

    #[test]
    fn wildcard_query() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("parent", vec!["alice".into(), "bob".into()]);
        kb.add_fact("parent", vec!["alice".into(), "carol".into()]);
        kb.add_fact("parent", vec!["bob".into(), "dave".into()]);

        let results = kb.query("parent", &["alice", "?"]);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn transitive_rule() {
        let mut kb = KnowledgeBase::new("family");
        kb.add_fact("parent", vec!["alice".into(), "bob".into()]);
        kb.add_fact("parent", vec!["bob".into(), "charlie".into()]);
        kb.add_fact("parent", vec!["charlie".into(), "dave".into()]);

        // ancestor(X, Y) :- parent(X, Y)
        kb.add_rule(Rule {
            head: atom("ancestor", &["X", "Y"]),
            body: vec![Literal::pos(atom("parent", &["X", "Y"]))],
        });

        // ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z)
        kb.add_rule(Rule {
            head: atom("ancestor", &["X", "Z"]),
            body: vec![
                Literal::pos(atom("parent", &["X", "Y"])),
                Literal::pos(atom("ancestor", &["Y", "Z"])),
            ],
        });

        let results = kb.query("ancestor", &["alice", "?"]);
        assert_eq!(results.len(), 3); // bob, charlie, dave
    }

    #[test]
    fn rule_fixpoint() {
        let mut kb = KnowledgeBase::new("graph");
        kb.add_fact("edge", vec!["a".into(), "b".into()]);
        kb.add_fact("edge", vec!["b".into(), "c".into()]);
        kb.add_fact("edge", vec!["c".into(), "d".into()]);

        // reachable(X, Y) :- edge(X, Y)
        kb.add_rule(Rule {
            head: atom("reachable", &["X", "Y"]),
            body: vec![Literal::pos(atom("edge", &["X", "Y"]))],
        });

        // reachable(X, Z) :- edge(X, Y), reachable(Y, Z)
        kb.add_rule(Rule {
            head: atom("reachable", &["X", "Z"]),
            body: vec![
                Literal::pos(atom("edge", &["X", "Y"])),
                Literal::pos(atom("reachable", &["Y", "Z"])),
            ],
        });

        let all = kb.query("reachable", &["a", "?"]);
        assert_eq!(all.len(), 3); // b, c, d
    }

    #[test]
    fn empty_query() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("color", vec!["sky".into(), "blue".into()]);

        let results = kb.query("color", &["grass", "?"]);
        assert!(results.is_empty());
    }

    #[test]
    fn fact_count() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("a", vec!["1".into()]);
        kb.add_fact("a", vec!["2".into()]);
        kb.add_fact("b", vec!["x".into()]);
        assert_eq!(kb.fact_count(), 3);
    }

    #[test]
    fn mlir_emission() {
        let mut kb = KnowledgeBase::new("test_kb");
        kb.add_fact("parent", vec!["a".into(), "b".into()]);
        let ops = kb.emit_mlir();
        assert!(ops[0].contains("MAGE.kb @test_kb"));
        assert!(ops.iter().any(|op| op.contains("MAGE.kb.fact")));
    }

    #[test]
    fn duplicate_facts_deduplicated() {
        let mut kb = KnowledgeBase::new("test");
        kb.add_fact("color", vec!["sky".into(), "blue".into()]);
        kb.add_fact("color", vec!["sky".into(), "blue".into()]);
        assert_eq!(kb.fact_count(), 1);
    }

    /// MAGE_SPEC §7.1's example, verbatim. It derived nothing while the block
    /// body was ignored and lowercase `t` was read as a constant.
    #[test]
    fn the_spec_example_derives_from_its_block_body() {
        let mut kb = kb_of(
            "kb TypeRules {\n    fact numeric(\"i32\");\n    fact numeric(\"f64\");\n    \
             fact unsigned(\"u32\");\n    rule integer(t: str) { numeric(t) }\n    \
             rule castable(t: str) { numeric(t) }\n}\n",
        );
        assert!(errors(&kb).is_empty(), "{:?}", errors(&kb));
        assert_eq!(kb.query("integer", &["?"]), vec![vec!["f64"], vec!["i32"]]);
        assert_eq!(kb.query("castable", &["i32"]).len(), 1);
        assert_eq!(kb.fact_count(), 7);
    }

    #[test]
    fn a_body_variable_absent_from_the_head_joins() {
        let mut kb = kb_of(
            "kb F {\n    fact parent(a, b);\n    fact parent(b, c);\n    \
             rule grandparent(x: str, z: str) { parent(x, y) && parent(y, z) }\n}\n",
        );
        assert!(errors(&kb).is_empty(), "{:?}", errors(&kb));
        assert_eq!(kb.query("grandparent", &["?", "?"]), vec![vec!["a", "c"]]);
    }

    #[test]
    fn where_conditions_and_the_block_are_one_conjunction() {
        let mut kb = kb_of(
            "kb F {\n    fact p(1);\n    fact p(2);\n    fact q(2);\n    \
             rule both(x: i32) where p(x) { q(x) }\n}\n",
        );
        assert!(errors(&kb).is_empty(), "{:?}", errors(&kb));
        assert_eq!(kb.query("both", &["?"]), vec![vec!["2"]]);
    }

    #[test]
    fn a_string_literal_in_a_rule_is_a_constant() {
        let mut kb = kb_of(
            "kb F {\n    fact kind(x, \"form\");\n    fact kind(y, \"rule\");\n    \
             rule form(n: str) { kind(n, \"form\") }\n}\n",
        );
        assert_eq!(kb.query("form", &["?"]), vec![vec!["x"]]);
    }

    #[test]
    fn negation_subtracts_a_lower_stratum() {
        let mut kb = kb_of(
            "kb F {\n    fact rule_id(r1);\n    fact rule_id(r2);\n    fact enforced_by(r1, verifier);\n    \
             rule enforced(r: str) { enforced_by(r, _) }\n    \
             rule unenforced(r: str) { rule_id(r) && !enforced(r) }\n}\n",
        );
        assert!(errors(&kb).is_empty(), "{:?}", errors(&kb));
        assert_eq!(kb.query("unenforced", &["?"]), vec![vec!["r2"]]);
    }

    /// Stratification, not rule order, decides when a negation is read: the
    /// negated predicate is declared after the rule that negates it.
    #[test]
    fn negation_waits_for_the_predicate_it_negates() {
        let mut kb = kb_of(
            "kb F {\n    fact node(a);\n    fact node(b);\n    fact node(c);\n    fact edge(a, b);\n    \
             rule unreached(n: str) { node(n) && !reached(n) }\n    \
             rule reached(n: str) { edge(\"a\", n) }\n    \
             rule reached(n: str) { reached(m) && edge(m, n) }\n}\n",
        );
        assert!(errors(&kb).is_empty(), "{:?}", errors(&kb));
        assert_eq!(kb.query("unreached", &["?"]), vec![vec!["a"], vec!["c"]]);
    }

    #[test]
    fn a_negation_cycle_is_refused_and_evaluates_nothing() {
        let mut kb = kb_of(
            "kb F {\n    fact s(a);\n    rule p(x: str) { s(x) && !q(x) }\n    \
             rule q(x: str) { s(x) && !p(x) }\n}\n",
        );
        kb.materialize();
        let errs = errors(&kb);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("its own negation"), "{errs:?}");
        assert!(kb.query("p", &["?"]).is_empty() && kb.query("q", &["?"]).is_empty());
    }

    #[test]
    fn an_unbound_head_parameter_is_refused() {
        let mut kb = kb_of("kb F {\n    fact p(a);\n    rule r(x: str, y: str) { p(x) }\n}\n");
        kb.materialize();
        let errs = errors(&kb);
        assert!(errs.iter().any(|e| e.contains("`y` is not bound")), "{errs:?}");
        assert!(kb.query("r", &["?", "?"]).is_empty());
    }

    #[test]
    fn an_unbound_negated_variable_is_refused() {
        let mut kb = kb_of("kb F {\n    fact p(a);\n    rule r(x: str) { p(x) && !q(x, y) }\n}\n");
        kb.materialize();
        assert!(errors(&kb).iter().any(|e| e.contains("`y` in `!q(x, y)`")), "{:?}", errors(&kb));
    }

    /// `prototype/examples/unified.mg` had `rule grandparent(..) { x }`, which
    /// was accepted and meant nothing.
    #[test]
    fn a_body_that_is_not_a_predicate_call_is_refused() {
        let kb = kb_of("kb F {\n    fact p(a);\n    rule r(x: i32) { x }\n}\n");
        let errs = errors(&kb);
        assert!(errs.iter().any(|e| e.contains("not a predicate call")), "{errs:?}");
        assert!(kb.diagnostics.iter().all(|d| d.category == Some(DiagnosticCategory::KbRule)));
    }

    #[test]
    fn module_diagnostics_reports_every_kb_block() {
        let src = "kb A {\n    fact p(a);\n    rule r(x: i32) { x }\n}\n\
                   kb B {\n    fact q(a);\n    rule s(x: str) { q(x) }\n}\n";
        let module = crate::parser::parse(&crate::lexer::lex(src)).expect("parses");
        let diags = module_diagnostics(&module);
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert!(diags[0].message.contains("kb `A`: rule `r`"), "{diags:?}");
        assert_eq!(diags[0].category.map(|c| c.code()), Some("E0570"));
    }

    #[test]
    fn a_fact_argument_that_is_not_a_constant_refuses_the_whole_fact() {
        let mut kb = kb_of("kb F {\n    fact p(a, 1 + 2);\n    fact p(b, -3);\n}\n");
        assert!(errors(&kb).iter().any(|e| e.contains("argument 2 of fact `p`")), "{:?}", errors(&kb));
        assert_eq!(kb.query("p", &["?", "?"]), vec![vec!["b", "-3"]]);
    }
}
