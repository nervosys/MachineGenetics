//! The knowledge base (NEUROSYMBOLIC_PLAN.md, P3): the `kb/*.mg` files,
//! embedded at build time, parsed by the kernel parser, materialised by
//! [`crate::logic`], and checked against `kb/schema.mg` before anything may
//! read it. A knowledge base that breaks its schema does not load.
//!
//! What "breaks its schema" means:
//! - a fact's predicate is neither one of the schema's own ([`META`]) nor a
//!   declared `relation`;
//! - a relation fact has a different number of arguments than its
//!   `arg_class` facts, or an argument that is not an instance of the class
//!   its `arg_class` names (`Text` takes any constant);
//! - an `invariant` derives anything, or names no rule;
//! - the logic engine refuses a fact or rule (E0570).

use std::collections::{HashMap, HashSet};

use crate::ast;
use crate::logic::{self, KnowledgeBase};

/// The knowledge-base files, in load order: the schema first.
pub const FILES: &[(&str, &str)] = &[
    ("kb/schema.mg", include_str!("../../kb/schema.mg")),
    ("kb/lexicon.mg", include_str!("../../kb/lexicon.mg")),
    ("kb/grammar.mg", include_str!("../../kb/grammar.mg")),
];

/// The schema's own predicates: a fact may use these without a `relation`.
pub const META: &[&str] =
    &["schema_version", "class", "subclass_of", "relation", "arg_class", "invariant", "instance_of"];

/// A loaded knowledge base: every fact and every rule's consequences.
pub struct Kb {
    base: KnowledgeBase,
}

impl Kb {
    /// The facts matching `args`, where `"?"` matches anything.
    pub fn query(&mut self, predicate: &str, args: &[&str]) -> Vec<Vec<String>> {
        self.base.query(predicate, args)
    }

    /// Every fact of `predicate`.
    pub fn facts(&self, predicate: &str) -> Vec<Vec<String>> {
        self.base.get_facts(predicate)
    }
}

/// Load the embedded knowledge base, or say every way it breaks its schema.
pub fn load() -> Result<Kb, Vec<String>> {
    load_files(FILES)
}

/// Load `files` (path, source) as one knowledge base. Each message names the
/// file it comes from, or the invariant it breaks.
pub fn load_files(files: &[(&str, &str)]) -> Result<Kb, Vec<String>> {
    let mut base = KnowledgeBase::new("mage");
    let mut refused = Vec::new();
    // Every stated fact, with the file it came from, for the schema checks.
    let mut stated: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut rule_heads: HashSet<String> = HashSet::new();

    for &(path, src) in files {
        let module = match crate::parser::parse(&crate::lexer::lex(src)) {
            Ok(m) => m,
            Err(e) => {
                refused.push(format!("{path}: does not parse: {e:?}"));
                continue;
            }
        };
        for item in &module.items {
            let ast::ItemKind::Kb(def) = &item.kind else {
                refused.push(format!("{path}: holds an item that is not a `kb` block"));
                continue;
            };
            for fact in &def.facts {
                match logic::read_fact(fact) {
                    Ok(args) => {
                        stated.push((format!("{path}:{}", fact.line), fact.name.clone(), args.clone()));
                        base.add_fact(&fact.name, args);
                    }
                    Err(n) => refused.push(format!(
                        "{path}:{}: argument {n} of fact `{}` is not a constant",
                        fact.line, fact.name
                    )),
                }
            }
            for rule in &def.rules {
                match logic::read_rule(rule) {
                    Ok(r) => {
                        rule_heads.insert(r.head.predicate.clone());
                        base.add_rule(r);
                    }
                    Err(why) => refused.push(format!("{path}:{}: rule `{}` cannot be read: {why}", rule.line, rule.name)),
                }
            }
        }
    }

    // `derives` is read off the grammar's `rhs` facts: every name an
    // alternative mentions is derived by its production.
    for (path, pred, args) in &stated {
        if pred != "rhs" || args.len() != 2 {
            continue;
        }
        match crate::recognise::names_in(&args[1]) {
            Ok(names) => {
                for n in names {
                    base.add_fact("derives", vec![args[0].clone(), n]);
                }
            }
            Err(e) => refused.push(format!("{path}: the rhs `{}` of `{}` does not read: {e}", args[1], args[0])),
        }
    }

    base.materialize();
    refused.extend(base.diagnostics.iter().map(|d| d.message.clone()));

    // Relations and the class of each argument, by position.
    let mut arg_classes: HashMap<String, Vec<(usize, String)>> = HashMap::new();
    for r in base.get_facts("relation") {
        arg_classes.entry(r[0].clone()).or_default();
    }
    for a in base.get_facts("arg_class") {
        let pos = a[1].parse::<usize>().unwrap_or(0);
        arg_classes.entry(a[0].clone()).or_default().push((pos, a[2].clone()));
    }

    for (path, pred, args) in &stated {
        if META.contains(&pred.as_str()) {
            continue;
        }
        let Some(classes) = arg_classes.get(pred) else {
            refused.push(format!("{path}: fact `{pred}` uses a predicate the schema does not declare"));
            continue;
        };
        if classes.len() != args.len() {
            refused.push(format!(
                "{path}: fact `{pred}` has {} argument(s); the schema gives it {}",
                args.len(),
                classes.len()
            ));
            continue;
        }
        for (pos, class) in classes {
            let Some(arg) = args.get(pos.wrapping_sub(1)) else { continue };
            if class != "Text" && base.query("is_a", &[arg, class]).is_empty() {
                refused.push(format!(
                    "{path}: argument {pos} of `{pred}`, `{arg}`, is not a `{class}`"
                ));
            }
        }
    }

    for inv in base.get_facts("invariant") {
        let (name, says) = (&inv[0], &inv[1]);
        if !rule_heads.contains(name) {
            refused.push(format!("invariant `{name}` names no rule"));
            continue;
        }
        for v in base.get_facts(name) {
            // Where to look: the first stated fact that mentions the
            // violation's first value.
            let at = stated
                .iter()
                .find(|(_, _, args)| args.first() == v.first())
                .map(|(at, p, args)| format!(" — see {at}: {p}({})", args.join(", ")))
                .unwrap_or_default();
            refused.push(format!("invariant `{name}` ({says}) is broken by ({}){at}", v.join(", ")));
        }
    }

    if refused.is_empty() { Ok(Kb { base }) } else { Err(refused) }
}

/// Every `.mg` source in the repository, as (path from the repo root, text),
/// sorted by path. The corpus the conformance tests hold the kernel and the
/// knowledge base to.
#[cfg(test)]
pub(crate) fn corpus_files() -> Vec<(String, String)> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !matches!(name.as_str(), "target" | "node_modules" | ".git" | ".claude") {
                    walk(root, &p, out);
                }
            } else if name.ends_with(".mg") {
                let rel = p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.push((rel, std::fs::read_to_string(&p).expect("readable")));
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut out = Vec::new();
    for d in ["arena", "benchmarks", "examples", "framework", "kb", "prototype/examples"] {
        walk(&root, &root.join(d), &mut out);
    }
    out.sort();
    assert!(out.len() >= 114, "the corpus walk found {} sources", out.len());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schema plus `extra` facts, as a second file.
    fn with(extra: &str) -> Result<Kb, Vec<String>> {
        let bad = format!("kb Extra {{\n{extra}\n}}\n");
        load_files(&[FILES[0], ("kb/extra.mg", &bad)])
    }

    fn refused(extra: &str) -> Vec<String> {
        match with(extra) {
            Ok(_) => Vec::new(),
            Err(e) => e,
        }
    }

    #[test]
    fn the_schema_alone_loads() {
        let mut kb = load_files(&FILES[..1]).unwrap_or_else(|e| panic!("{e:#?}"));
        assert_eq!(kb.facts("schema_version"), vec![vec!["4".to_string()]]);
        assert!(!kb.query("below", &["Keyword", "Construct"]).is_empty());
    }

    /// One fact set per invariant that breaks it and nothing it does not
    /// need to. The test below requires an entry for every invariant.
    const BREAKS: &[(&str, &str)] = &[
        ("class_cycle", "fact subclass_of(Form, Hir);"),
        ("undeclared_class", "fact instance_of(x, Nonesuch);"),
        ("unenforced_rule", "fact instance_of(no_alias, SafetyRule);"),
        (
            "rule_enforced_twice",
            "fact instance_of(no_alias, SafetyRule);\n\
             fact instance_of(parser, SymbolicComponent);\n\
             fact instance_of(checker, SymbolicComponent);\n\
             fact enforced_by(no_alias, parser);\n\
             fact enforced_by(no_alias, checker);",
        ),
        (
            "keyword_two_sigils",
            "fact instance_of(plus_f, Sigil);\n\
             fact instance_of(at_f, Sigil);\n\
             fact instance_of(fn_kw, Keyword);\n\
             fact abbreviates(plus_f, fn_kw);\n\
             fact abbreviates(at_f, fn_kw);",
        ),
        (
            "unaccounted_lowering",
            "fact instance_of(ast, Ast);\n\
             fact instance_of(hir, Hir);\n\
             fact lowers_to(ast, hir);",
        ),
        ("unreachable_production", "fact instance_of(item_fn, Production);"),
        ("unbuilt_ast_kind", "fact instance_of(FnItem, AstKind);"),
        (
            "untested_diagnostic",
            "fact instance_of(E0570, Diagnostic);\n\
             fact instance_of(logic, SymbolicComponent);\n\
             fact enforced_by(E0570, logic);",
        ),
        ("unnamed_refusal", "fact instance_of(r1, Refusal);"),
        (
            "spelling_two_kinds",
            "fact instance_of(KwF, TokenKind);\n\
             fact instance_of(KwV, TokenKind);\n\
             fact instance_of(\"fn\", Token);\n\
             fact lexes_as(\"fn\", KwF);\n\
             fact lexes_as(\"fn\", KwV);",
        ),
    ];

    #[test]
    fn each_invariant_refuses_a_fact_that_breaks_it() {
        for (inv, facts) in BREAKS {
            let e = refused(facts);
            assert!(
                e.iter().any(|m| m.starts_with(&format!("invariant `{inv}`"))),
                "`{inv}` did not fire on:\n{facts}\ngot: {e:#?}"
            );
        }
    }

    #[test]
    fn every_invariant_has_a_breaking_example() {
        let kb = load().expect("loads");
        for inv in kb.facts("invariant") {
            assert!(BREAKS.iter().any(|(n, _)| *n == inv[0]), "no breaking example for `{}`", inv[0]);
        }
        assert_eq!(kb.facts("invariant").len(), BREAKS.len());
    }

    #[test]
    fn an_undeclared_predicate_is_refused() {
        let e = refused("fact frobnicates(a);");
        // Line 2: `with` puts the extra facts after `kb Extra {`.
        assert!(e.iter().any(|m| m == "kb/extra.mg:2: fact `frobnicates` uses a predicate the schema does not declare"), "{e:#?}");
    }

    /// A violation points at a stated fact that takes part in it, by line.
    #[test]
    fn an_invariant_violation_names_a_line() {
        let e = refused("fact instance_of(plus_f, Sigil);\nfact instance_of(at_f, Sigil);\nfact instance_of(fn_kw, Keyword);\nfact abbreviates(plus_f, fn_kw);\nfact abbreviates(at_f, fn_kw);");
        let m = e.iter().find(|m| m.starts_with("invariant `keyword_two_sigils`")).expect("fires");
        assert!(m.contains("see kb/extra.mg:4: instance_of(fn_kw, Keyword)"), "{m}");
    }

    #[test]
    fn a_relation_fact_at_the_wrong_arity_is_refused() {
        let e = refused("fact instance_of(plus_f, Sigil);\nfact abbreviates(plus_f);");
        assert!(e.iter().any(|m| m.contains("`abbreviates` has 1 argument(s); the schema gives it 2")), "{e:#?}");
    }

    #[test]
    fn an_argument_of_the_wrong_class_is_refused() {
        let e = refused(
            "fact instance_of(parser, SymbolicComponent);\n\
             fact instance_of(Linear, Layer);\n\
             fact enforced_by(Linear, parser);",
        );
        assert!(e.iter().any(|m| m.contains("argument 1 of `enforced_by`, `Linear`, is not a `Rule`")), "{e:#?}");
    }

    #[test]
    fn the_whole_knowledge_base_loads() {
        load().unwrap_or_else(|e| panic!("{e:#?}"));
    }

    /// `TokenKind`'s variants, read from the lexer's source: the enum has no
    /// runtime list, and a hand-written one here would be the copy this test
    /// exists to remove.
    fn token_kind_variants() -> std::collections::BTreeSet<String> {
        let src = include_str!("lexer.rs").replace("\r\n", "\n");
        let start = src.find("pub enum TokenKind {").expect("the enum");
        let body = &src[start..start + src[start..].find("\n}\n").expect("its end")];
        body.lines()
            .skip(1)
            .filter_map(|l| {
                let t = l.strip_prefix("    ")?;
                let name: String = t.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                (name.starts_with(|c: char| c.is_ascii_uppercase())
                    && t[name.len()..].trim_start().starts_with([',', '(', '{']))
                    .then_some(name)
            })
            .collect()
    }

    /// The lexer's token kinds are exactly the lexicon's, both ways.
    #[test]
    fn the_lexicon_names_every_token_kind() {
        let kb = load().expect("loads");
        let stated: std::collections::BTreeSet<String> = kb
            .facts("instance_of")
            .into_iter()
            .filter(|f| f[1] == "TokenKind")
            .map(|f| f[0].clone())
            .collect();
        let real = token_kind_variants();
        assert_eq!(real.len(), 181, "the variant reader still reads the enum");
        for v in &real {
            assert!(stated.contains(v), "TokenKind::{v} is not in kb/lexicon.mg");
        }
        for v in &stated {
            assert!(real.contains(v), "kb/lexicon.mg names `{v}`, which TokenKind does not have");
        }
    }

    /// Every entry of `lexer::KEYWORDS` is a `lexes_as` fact, and every
    /// `lexes_as` fact is true: its spelling lexes as one token of that kind.
    /// The second direction covers the spellings the lexer scans by character
    /// (`?=`, `@w`) and the identifiers with a keyword reading (`cons`).
    #[test]
    fn the_lexicon_states_every_keyword() {
        let kb = load().expect("loads");
        let stated: std::collections::BTreeSet<(String, String)> =
            kb.facts("lexes_as").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
        for (s, k) in crate::lexer::KEYWORDS {
            let k = format!("{k:?}");
            assert!(stated.contains(&(s.to_string(), k.clone())), "KEYWORDS has `{s}` -> {k}; kb/lexicon.mg does not");
        }
        for (s, k) in &stated {
            let kinds: Vec<String> = crate::lexer::lex(s)
                .iter()
                .map(|t| format!("{:?}", t.kind))
                .filter(|k| k != "Eof")
                .collect();
            assert_eq!(kinds, vec![k.clone()], "kb/lexicon.mg says `{s}` lexes as {k}");
        }
    }

    /// Every `.mg` source in the repository, and a supplement for the pairs
    /// the corpus does not use.
    fn corpus() -> Vec<String> {
        let mut out: Vec<String> = corpus_files().into_iter().map(|(_, src)| src).collect();
        out.push(
            "Y Id = i32;\nZ N: i32 = 1;\nM inner {\n    f g() {}\n}\nu std.io;\n\
             f h[T](x: T) -> i32 ~> T: Ord {\n    @@ {\n        v a = 1;\n        >>;\n    }\n    \
             @@ {\n        !;\n    }\n    0\n}\n"
                .to_string(),
        );
        out
    }

    /// The formatter prints exactly the lexicon's pairs. Every source is
    /// printed in both modes and each spelling counted; the agent count minus
    /// the human count is `d`. A pair moves occurrences from its keyword to
    /// its sigil, so within each connected group of `abbreviates` pairs the
    /// `d`s sum to zero, each sigil's is positive and each keyword's negative
    /// (`xn`/`extern` excepted: reserved, so nothing prints them). A spelling
    /// in no pair prints the same in both modes: `d` is zero. An unstated pair
    /// breaks the last rule; a stated pair the formatter does not print breaks
    /// the sign rules.
    #[test]
    fn the_formatter_prints_the_lexicon_pairs() {
        use std::collections::{BTreeMap, BTreeSet};
        let kb = load().expect("loads");
        let pairs: Vec<(String, String)> =
            kb.facts("abbreviates").into_iter().map(|f| (f[0].clone(), f[1].clone())).collect();
        let spellings: BTreeSet<String> = kb.facts("lexes_as").into_iter().map(|f| f[0].clone()).collect();
        let mut d: BTreeMap<String, i64> = BTreeMap::new();
        for src in corpus() {
            let m = crate::parser::parse(&crate::lexer::lex(&src)).expect("the corpus parses");
            for t in crate::lexer::lex(&crate::fmt::format_agent(&m)) {
                *d.entry(t.text.clone()).or_default() += 1;
            }
            for t in crate::lexer::lex(&crate::fmt::format_human(&m)) {
                *d.entry(t.text.clone()).or_default() -= 1;
            }
        }
        let d = |s: &str| d.get(s).copied().unwrap_or(0);
        // Connected groups, by repeated merging.
        let mut groups: Vec<BTreeSet<String>> = Vec::new();
        for (s, k) in &pairs {
            let mut g: BTreeSet<String> = [s.clone(), k.clone()].into();
            groups.retain(|o| {
                if o.contains(s) || o.contains(k) {
                    g.extend(o.iter().cloned());
                    false
                } else {
                    true
                }
            });
            groups.push(g);
        }
        for g in &groups {
            let sum: i64 = g.iter().map(|s| d(s)).sum();
            assert_eq!(sum, 0, "the pairs {g:?} do not balance: {:?}", g.iter().map(|s| (s, d(s))).collect::<Vec<_>>());
        }
        for (s, k) in &pairs {
            if s == "xn" {
                continue;
            }
            assert!(d(s) > 0, "the formatter never prints `{s}` in agent mode for `{k}` (d = {})", d(s));
            assert!(d(k) < 0, "the formatter never prints `{k}` in human mode for `{s}` (d = {})", d(k));
        }
        let paired: BTreeSet<&String> = pairs.iter().flat_map(|(s, k)| [s, k]).collect();
        for s in spellings.iter().filter(|s| !paired.contains(s)) {
            assert_eq!(d(s), 0, "`{s}` prints differently in the two modes, and no `abbreviates` pair says so");
        }
    }

    /// MAGE_ONTOLOGY.md §11.1 documents the schema. Every backticked name in
    /// its three tables' first two columns that looks like a class, relation
    /// or invariant (or one of the schema's own predicates) must be one, and
    /// every one the schema declares must be
    /// named there: the page cannot drift from `kb/schema.mg` in either
    /// direction.
    #[test]
    fn the_ontology_names_exactly_the_schema() {
        let doc = include_str!("../../MAGE_ONTOLOGY.md").replace("\r\n", "\n");
        let start = doc.find("#### 11.1 The top-level schema").expect("§11.1");
        let end = start + doc[start..].find("\n### 12.").expect("§12 follows");
        let section = &doc[start..end];
        let kb = load().expect("loads");
        let col = |p: &str| -> std::collections::BTreeSet<String> {
            kb.facts(p).into_iter().map(|f| f[0].clone()).collect()
        };
        let (classes, relations, invariants) = (col("class"), col("relation"), col("invariant"));
        // Names in the first two cells of each table row.
        let mut named = std::collections::BTreeSet::new();
        for line in section.lines().filter(|l| l.starts_with("| `")) {
            let cells: Vec<&str> = line.split('|').skip(1).take(2).collect();
            for cell in cells {
                for (i, part) in cell.split('`').enumerate() {
                    if i % 2 == 1 {
                        named.insert(part.to_string());
                    }
                }
            }
        }
        for n in &named {
            assert!(
                classes.contains(n) || relations.contains(n) || invariants.contains(n) || META.contains(&n.as_str()),
                "MAGE_ONTOLOGY.md §11.1 names `{n}`, which kb/schema.mg does not declare"
            );
        }
        for n in classes.iter().chain(&relations).chain(&invariants) {
            assert!(named.contains(n), "kb/schema.mg declares `{n}`, which MAGE_ONTOLOGY.md §11.1 does not name");
        }
    }

    /// A subclass instance satisfies its superclass: a `Diagnostic` is a
    /// `Rule`, so it may be `enforced_by` something.
    #[test]
    fn a_subclass_instance_is_accepted_where_its_superclass_is_named() {
        let r = with(
            "fact instance_of(E0570, Diagnostic);\n\
             fact instance_of(logic, SymbolicComponent);\n\
             fact enforced_by(E0570, logic);\n\
             fact tested_by(E0570, \"logic::tests\");",
        );
        assert!(r.is_ok(), "{:#?}", r.err());
    }
}
