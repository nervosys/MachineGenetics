//! Formal LL(1) grammar specification for Redox canonical syntax.
//!
//! This crate defines the grammar rules for Redox's canonical syntax mode as structured
//! data. It computes FIRST and FOLLOW sets and validates that the grammar is LL(1)-safe
//! (no conflicts in the parse table). This serves as the single source of truth for the
//! parser implementation in `redox_parse`.
//!
//! # Grammar Representation
//!
//! The grammar is defined as a set of `Rule`s, each mapping a non-terminal `Symbol` to
//! one or more `Production`s (alternatives). Each production is a sequence of `Symbol`s
//! (terminals or non-terminals). The grammar analysis computes FIRST and FOLLOW sets and
//! checks for LL(1) conflicts.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

// ── Symbol types ───────────────────────────────────────────────────────────

/// A grammar symbol — either a terminal token or a non-terminal production rule.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Symbol {
    /// A terminal symbol (token kind name from the lexer).
    Terminal(String),
    /// A non-terminal symbol (grammar rule name).
    NonTerminal(String),
    /// The empty production (epsilon / ε).
    Epsilon,
    /// End-of-input marker ($).
    Eof,
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Symbol::Terminal(s) => write!(f, "'{s}'"),
            Symbol::NonTerminal(s) => write!(f, "{s}"),
            Symbol::Epsilon => write!(f, "ε"),
            Symbol::Eof => write!(f, "$"),
        }
    }
}

/// Convenience constructors.
impl Symbol {
    pub fn term(s: &str) -> Self {
        Symbol::Terminal(s.to_string())
    }
    pub fn nt(s: &str) -> Self {
        Symbol::NonTerminal(s.to_string())
    }
}

// ── Production / Rule ──────────────────────────────────────────────────────

/// A single production alternative: a sequence of symbols.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Production {
    pub symbols: Vec<Symbol>,
}

impl Production {
    pub fn new(symbols: Vec<Symbol>) -> Self {
        Self { symbols }
    }

    /// An epsilon production (empty).
    pub fn epsilon() -> Self {
        Self { symbols: vec![Symbol::Epsilon] }
    }

    pub fn is_epsilon(&self) -> bool {
        self.symbols.len() == 1 && self.symbols[0] == Symbol::Epsilon
    }
}

impl fmt::Display for Production {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.symbols.iter().map(|s| s.to_string()).collect();
        write!(f, "{}", parts.join(" "))
    }
}

/// A grammar rule: a non-terminal with its alternative productions.
#[derive(Clone, Debug)]
pub struct Rule {
    pub name: String,
    pub productions: Vec<Production>,
}

impl Rule {
    pub fn new(name: &str, productions: Vec<Production>) -> Self {
        Self { name: name.to_string(), productions }
    }
}

// ── Grammar ────────────────────────────────────────────────────────────────

/// A complete grammar specification.
#[derive(Clone, Debug)]
pub struct Grammar {
    /// The start symbol of the grammar.
    pub start: String,
    /// The rules, keyed by non-terminal name.
    pub rules: BTreeMap<String, Rule>,
}

impl Grammar {
    pub fn new(start: &str) -> Self {
        Self { start: start.to_string(), rules: BTreeMap::new() }
    }

    /// Add a rule to the grammar. Panics if the non-terminal already exists.
    pub fn add_rule(&mut self, rule: Rule) {
        assert!(
            !self.rules.contains_key(&rule.name),
            "duplicate rule for non-terminal '{}'",
            rule.name
        );
        self.rules.insert(rule.name.clone(), rule);
    }

    /// Return all non-terminal names.
    pub fn non_terminals(&self) -> Vec<&str> {
        self.rules.keys().map(|s| s.as_str()).collect()
    }

    /// Return all terminal symbols used in the grammar.
    pub fn terminals(&self) -> BTreeSet<String> {
        let mut terms = BTreeSet::new();
        for rule in self.rules.values() {
            for prod in &rule.productions {
                for sym in &prod.symbols {
                    if let Symbol::Terminal(t) = sym {
                        terms.insert(t.clone());
                    }
                }
            }
        }
        terms
    }
}

// ── FIRST / FOLLOW sets ───────────────────────────────────────────────────

/// Result of LL(1) grammar analysis.
#[derive(Debug)]
pub struct GrammarAnalysis {
    /// FIRST sets for each non-terminal.
    pub first: HashMap<String, BTreeSet<Symbol>>,
    /// FOLLOW sets for each non-terminal.
    pub follow: HashMap<String, BTreeSet<Symbol>>,
    /// LL(1) parse table: (non-terminal, terminal) → production index.
    pub table: HashMap<(String, Symbol), usize>,
    /// LL(1) conflicts found during table construction.
    pub conflicts: Vec<Conflict>,
}

/// An LL(1) conflict: two productions for the same (non-terminal, terminal) pair.
#[derive(Debug, Clone)]
pub struct Conflict {
    pub non_terminal: String,
    pub terminal: Symbol,
    pub production_indices: Vec<usize>,
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LL(1) conflict for ({}, {}): productions {:?}",
            self.non_terminal, self.terminal, self.production_indices
        )
    }
}

/// Compute FIRST set for a sequence of symbols.
fn first_of_sequence(
    seq: &[Symbol],
    first_sets: &HashMap<String, BTreeSet<Symbol>>,
) -> BTreeSet<Symbol> {
    let mut result = BTreeSet::new();
    if seq.is_empty() {
        result.insert(Symbol::Epsilon);
        return result;
    }

    for sym in seq {
        match sym {
            Symbol::Terminal(_) | Symbol::Eof => {
                result.insert(sym.clone());
                return result; // Terminal stops the scan.
            }
            Symbol::Epsilon => {
                // Epsilon in sequence — continue to next symbol.
                continue;
            }
            Symbol::NonTerminal(name) => {
                if let Some(first) = first_sets.get(name) {
                    let has_epsilon = first.contains(&Symbol::Epsilon);
                    for s in first {
                        if *s != Symbol::Epsilon {
                            result.insert(s.clone());
                        }
                    }
                    if !has_epsilon {
                        return result;
                    }
                    // Epsilon is in FIRST(name), so continue to next symbol.
                } else {
                    // Unknown non-terminal — treat as empty.
                    return result;
                }
            }
        }
    }
    // All symbols could derive epsilon.
    result.insert(Symbol::Epsilon);
    result
}

/// Analyze a grammar: compute FIRST sets, FOLLOW sets, and build the LL(1) parse table.
pub fn analyze(grammar: &Grammar) -> GrammarAnalysis {
    let first = compute_first_sets(grammar);
    let follow = compute_follow_sets(grammar, &first);
    let (table, conflicts) = build_parse_table(grammar, &first, &follow);
    GrammarAnalysis { first, follow, table, conflicts }
}

/// Compute FIRST sets for all non-terminals using a fixed-point iteration.
fn compute_first_sets(grammar: &Grammar) -> HashMap<String, BTreeSet<Symbol>> {
    let mut first: HashMap<String, BTreeSet<Symbol>> = HashMap::new();
    for name in grammar.rules.keys() {
        first.insert(name.clone(), BTreeSet::new());
    }

    let mut changed = true;
    while changed {
        changed = false;
        for (name, rule) in &grammar.rules {
            for prod in &rule.productions {
                let prod_first = first_of_sequence(&prod.symbols, &first);
                let set = first.get_mut(name).unwrap();
                for s in prod_first {
                    if set.insert(s) {
                        changed = true;
                    }
                }
            }
        }
    }

    first
}

/// Compute FOLLOW sets for all non-terminals using a fixed-point iteration.
fn compute_follow_sets(
    grammar: &Grammar,
    first: &HashMap<String, BTreeSet<Symbol>>,
) -> HashMap<String, BTreeSet<Symbol>> {
    let mut follow: HashMap<String, BTreeSet<Symbol>> = HashMap::new();
    for name in grammar.rules.keys() {
        follow.insert(name.clone(), BTreeSet::new());
    }

    // Rule 1: $ ∈ FOLLOW(start).
    follow.get_mut(&grammar.start).unwrap().insert(Symbol::Eof);

    let mut changed = true;
    while changed {
        changed = false;
        for (lhs_name, rule) in &grammar.rules {
            for prod in &rule.productions {
                for (i, sym) in prod.symbols.iter().enumerate() {
                    if let Symbol::NonTerminal(b) = sym {
                        let rest = &prod.symbols[i + 1..];
                        let first_rest = first_of_sequence(rest, first);

                        let follow_b = follow.get_mut(b).unwrap();
                        // Add non-epsilon symbols from FIRST(rest) to FOLLOW(B).
                        for s in &first_rest {
                            if *s != Symbol::Epsilon && follow_b.insert(s.clone()) {
                                changed = true;
                            }
                        }

                        // If rest can derive epsilon (or is empty), add FOLLOW(LHS) to FOLLOW(B).
                        if first_rest.contains(&Symbol::Epsilon) {
                            let follow_lhs: Vec<Symbol> =
                                follow.get(lhs_name).cloned().unwrap_or_default().into_iter().collect();
                            let follow_b = follow.get_mut(b).unwrap();
                            for s in follow_lhs {
                                if follow_b.insert(s) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    follow
}

/// Build the LL(1) parse table and report any conflicts.
fn build_parse_table(
    grammar: &Grammar,
    first: &HashMap<String, BTreeSet<Symbol>>,
    follow: &HashMap<String, BTreeSet<Symbol>>,
) -> (HashMap<(String, Symbol), usize>, Vec<Conflict>) {
    let mut table: HashMap<(String, Symbol), usize> = HashMap::new();
    let mut multi: HashMap<(String, Symbol), Vec<usize>> = HashMap::new();

    for (name, rule) in &grammar.rules {
        for (idx, prod) in rule.productions.iter().enumerate() {
            let prod_first = first_of_sequence(&prod.symbols, first);

            for s in &prod_first {
                if *s == Symbol::Epsilon {
                    continue;
                }
                let key = (name.clone(), s.clone());
                multi.entry(key).or_default().push(idx);
            }

            if prod_first.contains(&Symbol::Epsilon) {
                if let Some(follow_set) = follow.get(name) {
                    for s in follow_set {
                        let key = (name.clone(), s.clone());
                        multi.entry(key).or_default().push(idx);
                    }
                }
            }
        }
    }

    let mut conflicts = Vec::new();
    for (key, indices) in &multi {
        if indices.len() > 1 {
            conflicts.push(Conflict {
                non_terminal: key.0.clone(),
                terminal: key.1.clone(),
                production_indices: indices.clone(),
            });
        }
        // Store the first entry in the table regardless.
        table.insert(key.clone(), indices[0]);
    }

    (table, conflicts)
}

// ── Redox Canonical Grammar Definition ─────────────────────────────────────

/// Build the canonical Redox grammar specification.
///
/// This grammar covers the canonical syntax mode constructs:
/// - Items: functions, structs, enums, type aliases, modules, impl blocks
/// - Compact keywords: `f`→fn, `v`→let, `s`→struct, `e`→enum, etc.
/// - Sigil-fn prefixes: `+fn` (async), `-fn` (const), `!fn` (unsafe), `*fn` (extern)
/// - Compact attributes: `@d`, `@r`, `@t`, `@i`, `@as`, `@ac`, `@ax`, `@ao`, `@ae`
/// - Type abbreviations: `?T`→Option<T>, `R[T,E]`→Result<T,E>, `V[T]`→Vec<T>
/// - Spec blocks, contracts, effects, capabilities (future steps)
///
/// The grammar uses non-terminal names matching the production rules. Terminal names
/// correspond to `TokenKind` variant names or keyword strings.
pub fn canonical_grammar() -> Grammar {
    let mut g = Grammar::new("Program");

    // ── Top-level ──────────────────────────────────────────────────────

    g.add_rule(Rule::new("Program", vec![
        Production::new(vec![Symbol::nt("ItemList"), Symbol::Eof]),
    ]));

    g.add_rule(Rule::new("ItemList", vec![
        Production::new(vec![Symbol::nt("Item"), Symbol::nt("ItemList")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("Item", vec![
        Production::new(vec![Symbol::nt("OptAttrs"), Symbol::nt("OptVis"), Symbol::nt("ItemKind")]),
    ]));

    // ── Attributes ─────────────────────────────────────────────────────

    g.add_rule(Rule::new("OptAttrs", vec![
        Production::new(vec![Symbol::nt("Attr"), Symbol::nt("OptAttrs")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("Attr", vec![
        // Standard attribute: #[...]
        Production::new(vec![Symbol::term("Pound"), Symbol::term("OpenBracket"), Symbol::nt("AttrContent"), Symbol::term("CloseBracket")]),
        // Compact attribute form: @d(...), @t, etc.
        Production::new(vec![Symbol::term("CompactAttribute"), Symbol::nt("OptAttrArgs")]),
    ]));

    g.add_rule(Rule::new("OptAttrArgs", vec![
        Production::new(vec![Symbol::term("OpenParen"), Symbol::nt("AttrArgList"), Symbol::term("CloseParen")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("AttrContent", vec![
        Production::new(vec![Symbol::nt("Path"), Symbol::nt("OptAttrArgs")]),
    ]));

    g.add_rule(Rule::new("AttrArgList", vec![
        Production::new(vec![Symbol::nt("AttrArg"), Symbol::nt("AttrArgListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("AttrArgListTail", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("AttrArg"), Symbol::nt("AttrArgListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("AttrArg", vec![
        Production::new(vec![Symbol::term("Ident")]),
    ]));

    // ── Visibility ─────────────────────────────────────────────────────

    g.add_rule(Rule::new("OptVis", vec![
        Production::new(vec![Symbol::term("kw_pub"), Symbol::nt("OptVisRestriction")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("OptVisRestriction", vec![
        Production::new(vec![Symbol::term("OpenParen"), Symbol::nt("VisPath"), Symbol::term("CloseParen")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("VisPath", vec![
        Production::new(vec![Symbol::term("kw_crate")]),
        Production::new(vec![Symbol::term("kw_super")]),
        Production::new(vec![Symbol::term("kw_in"), Symbol::nt("Path")]),
    ]));

    // ── Item kinds ─────────────────────────────────────────────────────

    g.add_rule(Rule::new("ItemKind", vec![
        Production::new(vec![Symbol::nt("FnItem")]),
        Production::new(vec![Symbol::nt("StructItem")]),
        Production::new(vec![Symbol::nt("EnumItem")]),
        Production::new(vec![Symbol::nt("TypeItem")]),
        Production::new(vec![Symbol::nt("ModItem")]),
        Production::new(vec![Symbol::nt("ImplItem")]),
        Production::new(vec![Symbol::nt("UseItem")]),
        Production::new(vec![Symbol::nt("LetItem")]),
        Production::new(vec![Symbol::nt("TraitItem")]),
    ]));

    // ── Function ───────────────────────────────────────────────────────

    g.add_rule(Rule::new("FnItem", vec![
        // Regular fn (with optional keyword qualifiers)
        Production::new(vec![Symbol::nt("OptKeywordQualifier"), Symbol::term("kw_fn"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::term("OpenParen"), Symbol::nt("ParamList"), Symbol::term("CloseParen"), Symbol::nt("OptReturnType"), Symbol::nt("Block")]),
        // Sigil-fn forms (qualifier + fn fused into single token)
        Production::new(vec![Symbol::term("PlusFn"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::term("OpenParen"), Symbol::nt("ParamList"), Symbol::term("CloseParen"), Symbol::nt("OptReturnType"), Symbol::nt("Block")]),
        Production::new(vec![Symbol::term("MinusFn"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::term("OpenParen"), Symbol::nt("ParamList"), Symbol::term("CloseParen"), Symbol::nt("OptReturnType"), Symbol::nt("Block")]),
        Production::new(vec![Symbol::term("BangFn"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::term("OpenParen"), Symbol::nt("ParamList"), Symbol::term("CloseParen"), Symbol::nt("OptReturnType"), Symbol::nt("Block")]),
        Production::new(vec![Symbol::term("StarFn"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::term("OpenParen"), Symbol::nt("ParamList"), Symbol::term("CloseParen"), Symbol::nt("OptReturnType"), Symbol::nt("Block")]),
    ]));

    // Keyword-based qualifiers (legacy-style, non-fused).
    g.add_rule(Rule::new("OptKeywordQualifier", vec![
        Production::new(vec![Symbol::term("kw_async")]),
        Production::new(vec![Symbol::term("kw_const")]),
        Production::new(vec![Symbol::term("kw_unsafe")]),
        Production::new(vec![Symbol::term("kw_extern")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("OptGenericParams", vec![
        Production::new(vec![Symbol::term("Lt"), Symbol::nt("GenericParamList"), Symbol::term("Gt")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("GenericParamList", vec![
        Production::new(vec![Symbol::nt("GenericParam"), Symbol::nt("GenericParamListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("GenericParamListTail", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("GenericParam"), Symbol::nt("GenericParamListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("GenericParam", vec![
        Production::new(vec![Symbol::term("Ident"), Symbol::nt("OptBounds")]),
        Production::new(vec![Symbol::term("Lifetime")]),
    ]));

    g.add_rule(Rule::new("OptBounds", vec![
        Production::new(vec![Symbol::term("Colon"), Symbol::nt("TypeBound"), Symbol::nt("TypeBoundTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("TypeBound", vec![
        Production::new(vec![Symbol::nt("Path")]),
    ]));

    g.add_rule(Rule::new("TypeBoundTail", vec![
        Production::new(vec![Symbol::term("Plus"), Symbol::nt("TypeBound"), Symbol::nt("TypeBoundTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("ParamList", vec![
        Production::new(vec![Symbol::nt("Param"), Symbol::nt("ParamListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("ParamListTail", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("Param"), Symbol::nt("ParamListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("Param", vec![
        Production::new(vec![Symbol::nt("Pattern"), Symbol::term("Colon"), Symbol::nt("Type")]),
        Production::new(vec![Symbol::term("kw_self")]),
        Production::new(vec![Symbol::term("And"), Symbol::nt("RefSelfSuffix")]),
    ]));

    g.add_rule(Rule::new("RefSelfSuffix", vec![
        Production::new(vec![Symbol::term("kw_self")]),
        Production::new(vec![Symbol::term("kw_mut"), Symbol::term("kw_self")]),
    ]));

    g.add_rule(Rule::new("OptReturnType", vec![
        Production::new(vec![Symbol::term("RArrow"), Symbol::nt("Type")]),
        Production::epsilon(),
    ]));

    // ── Struct ─────────────────────────────────────────────────────────

    g.add_rule(Rule::new("StructItem", vec![
        Production::new(vec![Symbol::term("kw_struct"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::nt("StructBody")]),
    ]));

    g.add_rule(Rule::new("StructBody", vec![
        Production::new(vec![Symbol::term("OpenBrace"), Symbol::nt("FieldList"), Symbol::term("CloseBrace")]),
        Production::new(vec![Symbol::term("OpenParen"), Symbol::nt("TupleFieldList"), Symbol::term("CloseParen"), Symbol::term("Semi")]),
        Production::new(vec![Symbol::term("Semi")]),
    ]));

    g.add_rule(Rule::new("FieldList", vec![
        Production::new(vec![Symbol::nt("Field"), Symbol::nt("FieldListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("FieldListTail", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("FieldListCont")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("FieldListCont", vec![
        Production::new(vec![Symbol::nt("Field"), Symbol::nt("FieldListTail")]),
        Production::epsilon(), // trailing comma
    ]));

    g.add_rule(Rule::new("Field", vec![
        Production::new(vec![Symbol::nt("OptVis"), Symbol::term("Ident"), Symbol::term("Colon"), Symbol::nt("Type")]),
    ]));

    g.add_rule(Rule::new("TupleFieldList", vec![
        Production::new(vec![Symbol::nt("TupleField"), Symbol::nt("TupleFieldListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("TupleFieldListTail", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("TupleField"), Symbol::nt("TupleFieldListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("TupleField", vec![
        Production::new(vec![Symbol::nt("Type")]),
    ]));

    // ── Enum ───────────────────────────────────────────────────────────

    g.add_rule(Rule::new("EnumItem", vec![
        Production::new(vec![Symbol::term("kw_enum"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::term("OpenBrace"), Symbol::nt("VariantList"), Symbol::term("CloseBrace")]),
    ]));

    g.add_rule(Rule::new("VariantList", vec![
        Production::new(vec![Symbol::nt("Variant"), Symbol::nt("VariantListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("VariantListTail", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("VariantListCont")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("VariantListCont", vec![
        Production::new(vec![Symbol::nt("Variant"), Symbol::nt("VariantListTail")]),
        Production::epsilon(), // trailing comma
    ]));

    g.add_rule(Rule::new("Variant", vec![
        Production::new(vec![Symbol::term("Ident"), Symbol::nt("OptVariantData")]),
    ]));

    g.add_rule(Rule::new("OptVariantData", vec![
        Production::new(vec![Symbol::term("OpenParen"), Symbol::nt("TupleFieldList"), Symbol::term("CloseParen")]),
        Production::new(vec![Symbol::term("OpenBrace"), Symbol::nt("FieldList"), Symbol::term("CloseBrace")]),
        Production::new(vec![Symbol::term("Eq"), Symbol::nt("Expr")]),
        Production::epsilon(),
    ]));

    // ── Type alias ─────────────────────────────────────────────────────

    g.add_rule(Rule::new("TypeItem", vec![
        Production::new(vec![Symbol::term("kw_type"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::term("Eq"), Symbol::nt("Type"), Symbol::term("Semi")]),
    ]));

    // ── Module ─────────────────────────────────────────────────────────

    g.add_rule(Rule::new("ModItem", vec![
        Production::new(vec![Symbol::term("kw_mod"), Symbol::term("Ident"), Symbol::nt("ModBody")]),
    ]));

    g.add_rule(Rule::new("ModBody", vec![
        Production::new(vec![Symbol::term("OpenBrace"), Symbol::nt("ItemList"), Symbol::term("CloseBrace")]),
        Production::new(vec![Symbol::term("Semi")]),
    ]));

    // ── Impl ───────────────────────────────────────────────────────────

    g.add_rule(Rule::new("ImplItem", vec![
        Production::new(vec![Symbol::term("kw_impl"), Symbol::nt("OptGenericParams"), Symbol::nt("Type"), Symbol::nt("OptForType"), Symbol::term("OpenBrace"), Symbol::nt("ImplItemList"), Symbol::term("CloseBrace")]),
    ]));

    g.add_rule(Rule::new("OptForType", vec![
        Production::new(vec![Symbol::term("kw_for"), Symbol::nt("Type")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("ImplItemList", vec![
        Production::new(vec![Symbol::nt("Item"), Symbol::nt("ImplItemList")]),
        Production::epsilon(),
    ]));

    // ── Trait ──────────────────────────────────────────────────────────

    g.add_rule(Rule::new("TraitItem", vec![
        Production::new(vec![Symbol::term("kw_trait"), Symbol::term("Ident"), Symbol::nt("OptGenericParams"), Symbol::nt("OptBounds"), Symbol::term("OpenBrace"), Symbol::nt("TraitItemList"), Symbol::term("CloseBrace")]),
    ]));

    g.add_rule(Rule::new("TraitItemList", vec![
        Production::new(vec![Symbol::nt("Item"), Symbol::nt("TraitItemList")]),
        Production::epsilon(),
    ]));

    // ── Use ────────────────────────────────────────────────────────────

    g.add_rule(Rule::new("UseItem", vec![
        Production::new(vec![Symbol::term("kw_use"), Symbol::nt("UsePath"), Symbol::term("Semi")]),
    ]));

    g.add_rule(Rule::new("UsePath", vec![
        Production::new(vec![Symbol::nt("Path"), Symbol::nt("OptUseAlias")]),
    ]));

    g.add_rule(Rule::new("OptUseAlias", vec![
        Production::new(vec![Symbol::term("kw_as"), Symbol::term("Ident")]),
        Production::epsilon(),
    ]));

    // ── Let binding ────────────────────────────────────────────────────

    g.add_rule(Rule::new("LetItem", vec![
        Production::new(vec![Symbol::term("kw_let"), Symbol::nt("Pattern"), Symbol::nt("OptTypeAnnot"), Symbol::nt("OptInit"), Symbol::term("Semi")]),
    ]));

    g.add_rule(Rule::new("OptTypeAnnot", vec![
        Production::new(vec![Symbol::term("Colon"), Symbol::nt("Type")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("OptInit", vec![
        Production::new(vec![Symbol::term("Eq"), Symbol::nt("Expr")]),
        Production::epsilon(),
    ]));

    // ── Types ──────────────────────────────────────────────────────────

    g.add_rule(Rule::new("Type", vec![
        Production::new(vec![Symbol::nt("Path"), Symbol::nt("OptTypeArgs")]),
        Production::new(vec![Symbol::term("And"), Symbol::nt("OptLifetime"), Symbol::nt("OptMut"), Symbol::nt("Type")]),
        Production::new(vec![Symbol::term("Star"), Symbol::nt("ConstOrMut"), Symbol::nt("Type")]),
        Production::new(vec![Symbol::term("OpenBracket"), Symbol::nt("Type"), Symbol::nt("OptArrayLen"), Symbol::term("CloseBracket")]),
        Production::new(vec![Symbol::term("OpenParen"), Symbol::nt("TupleTypeList"), Symbol::term("CloseParen")]),
        // Option sugar: ?T → Option<T>
        Production::new(vec![Symbol::term("Question"), Symbol::nt("Type")]),
        // Vec sugar: V[T] (handled as Path + bracket args)
        // Result sugar: R[T,E] (handled as Path + bracket args)
        Production::new(vec![Symbol::term("kw_dyn"), Symbol::nt("TypeBound"), Symbol::nt("TypeBoundTail")]),
        Production::new(vec![Symbol::term("kw_impl"), Symbol::nt("TypeBound"), Symbol::nt("TypeBoundTail")]),
        Production::new(vec![Symbol::term("Bang")]), // never type
    ]));

    g.add_rule(Rule::new("OptTypeArgs", vec![
        Production::new(vec![Symbol::term("Lt"), Symbol::nt("TypeArgList"), Symbol::term("Gt")]),
        Production::new(vec![Symbol::term("OpenBracket"), Symbol::nt("TypeArgList"), Symbol::term("CloseBracket")]), // V[T], R[T,E] sugar
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("TypeArgList", vec![
        Production::new(vec![Symbol::nt("Type"), Symbol::nt("TypeArgListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("TypeArgListTail", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("Type"), Symbol::nt("TypeArgListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("OptLifetime", vec![
        Production::new(vec![Symbol::term("Lifetime")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("OptMut", vec![
        Production::new(vec![Symbol::term("kw_mut")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("ConstOrMut", vec![
        Production::new(vec![Symbol::term("kw_const")]),
        Production::new(vec![Symbol::term("kw_mut")]),
    ]));

    g.add_rule(Rule::new("OptArrayLen", vec![
        Production::new(vec![Symbol::term("Semi"), Symbol::nt("Expr")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("TupleTypeList", vec![
        Production::new(vec![Symbol::nt("Type"), Symbol::term("Comma"), Symbol::nt("TupleTypeListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("TupleTypeListTail", vec![
        Production::new(vec![Symbol::nt("Type"), Symbol::nt("OptCommaAndMore")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("OptCommaAndMore", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("TupleTypeListTail")]),
        Production::epsilon(),
    ]));

    // ── Patterns ───────────────────────────────────────────────────────

    g.add_rule(Rule::new("Pattern", vec![
        Production::new(vec![Symbol::term("Ident"), Symbol::nt("OptPatternBinding")]),
        Production::new(vec![Symbol::term("kw_mut"), Symbol::term("Ident")]),
        Production::new(vec![Symbol::term("kw_ref"), Symbol::term("Ident")]),
        Production::new(vec![Symbol::term("Underscore")]),
        Production::new(vec![Symbol::term("OpenParen"), Symbol::nt("PatternList"), Symbol::term("CloseParen")]),
        Production::new(vec![Symbol::term("Literal")]),
    ]));

    g.add_rule(Rule::new("OptPatternBinding", vec![
        Production::new(vec![Symbol::term("At"), Symbol::nt("Pattern")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("PatternList", vec![
        Production::new(vec![Symbol::nt("Pattern"), Symbol::nt("PatternListTail")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("PatternListTail", vec![
        Production::new(vec![Symbol::term("Comma"), Symbol::nt("Pattern"), Symbol::nt("PatternListTail")]),
        Production::epsilon(),
    ]));

    // ── Expressions (simplified — not full expression grammar) ─────────

    g.add_rule(Rule::new("Expr", vec![
        Production::new(vec![Symbol::term("Ident")]),
        Production::new(vec![Symbol::term("Literal")]),
        Production::new(vec![Symbol::term("OpenParen"), Symbol::nt("Expr"), Symbol::term("CloseParen")]),
    ]));

    // ── Blocks ─────────────────────────────────────────────────────────

    g.add_rule(Rule::new("Block", vec![
        Production::new(vec![Symbol::term("OpenBrace"), Symbol::nt("StmtList"), Symbol::term("CloseBrace")]),
    ]));

    g.add_rule(Rule::new("StmtList", vec![
        Production::new(vec![Symbol::nt("Stmt"), Symbol::nt("StmtList")]),
        Production::epsilon(),
    ]));

    g.add_rule(Rule::new("Stmt", vec![
        Production::new(vec![Symbol::nt("LetItem")]),
        Production::new(vec![Symbol::nt("ExprStmt")]),
    ]));

    g.add_rule(Rule::new("ExprStmt", vec![
        Production::new(vec![Symbol::nt("Expr"), Symbol::term("Semi")]),
    ]));

    // ── Path ───────────────────────────────────────────────────────────

    g.add_rule(Rule::new("Path", vec![
        Production::new(vec![Symbol::term("Ident"), Symbol::nt("PathTail")]),
        Production::new(vec![Symbol::term("PathSep"), Symbol::term("Ident"), Symbol::nt("PathTail")]),
        Production::new(vec![Symbol::term("kw_crate"), Symbol::nt("PathTail")]),
        Production::new(vec![Symbol::term("kw_super"), Symbol::nt("PathTail")]),
        Production::new(vec![Symbol::term("kw_self"), Symbol::nt("PathTail")]),
    ]));

    g.add_rule(Rule::new("PathTail", vec![
        Production::new(vec![Symbol::term("PathSep"), Symbol::term("Ident"), Symbol::nt("PathTail")]),
        Production::epsilon(),
    ]));

    g
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_is_well_formed() {
        let g = canonical_grammar();
        // Every non-terminal referenced in productions should be defined.
        for rule in g.rules.values() {
            for prod in &rule.productions {
                for sym in &prod.symbols {
                    if let Symbol::NonTerminal(name) = sym {
                        assert!(
                            g.rules.contains_key(name),
                            "non-terminal '{name}' used in rule '{}' but not defined",
                            rule.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn grammar_has_start_symbol() {
        let g = canonical_grammar();
        assert!(g.rules.contains_key(&g.start), "start symbol '{}' not defined", g.start);
    }

    #[test]
    fn first_sets_computed() {
        let g = canonical_grammar();
        let analysis = analyze(&g);
        // FIRST(Program) should not be empty.
        let first_program = &analysis.first["Program"];
        assert!(!first_program.is_empty(), "FIRST(Program) should not be empty");
    }

    #[test]
    fn follow_sets_contain_eof_for_start() {
        let g = canonical_grammar();
        let analysis = analyze(&g);
        assert!(
            analysis.follow["Program"].contains(&Symbol::Eof),
            "FOLLOW(start) must contain $"
        );
    }

    #[test]
    fn no_ll1_conflicts() {
        let g = canonical_grammar();
        let analysis = analyze(&g);
        if !analysis.conflicts.is_empty() {
            let msgs: Vec<String> = analysis.conflicts.iter().map(|c| c.to_string()).collect();
            panic!("LL(1) conflicts found:\n{}", msgs.join("\n"));
        }
    }

    #[test]
    fn first_of_fn_item() {
        let g = canonical_grammar();
        let analysis = analyze(&g);
        // FIRST(FnItem) should contain function-related terminals.
        let first = &analysis.first["FnItem"];
        assert!(
            first.contains(&Symbol::term("PlusFn"))
                || first.contains(&Symbol::term("kw_fn"))
                || first.contains(&Symbol::term("kw_async")),
            "FIRST(FnItem) should contain function-related tokens, got: {first:?}"
        );
    }

    #[test]
    fn terminals_include_expected_tokens() {
        let g = canonical_grammar();
        let terms = g.terminals();
        assert!(terms.contains("Ident"), "terminals should include Ident");
        assert!(terms.contains("kw_fn"), "terminals should include kw_fn");
        assert!(terms.contains("CompactAttribute"), "terminals should include CompactAttribute");
        assert!(terms.contains("PlusFn"), "terminals should include PlusFn");
    }

    // ── Simple grammar unit tests ──────────────────────────────────────

    #[test]
    fn simple_grammar_first_sets() {
        let mut g = Grammar::new("S");
        g.add_rule(Rule::new("S", vec![
            Production::new(vec![Symbol::term("a"), Symbol::nt("B")]),
        ]));
        g.add_rule(Rule::new("B", vec![
            Production::new(vec![Symbol::term("b")]),
            Production::epsilon(),
        ]));
        let analysis = analyze(&g);
        assert!(analysis.first["S"].contains(&Symbol::term("a")));
        assert!(analysis.first["B"].contains(&Symbol::term("b")));
        assert!(analysis.first["B"].contains(&Symbol::Epsilon));
        assert!(analysis.conflicts.is_empty());
    }

    #[test]
    fn simple_grammar_follow_sets() {
        let mut g = Grammar::new("S");
        g.add_rule(Rule::new("S", vec![
            Production::new(vec![Symbol::nt("A"), Symbol::term("b")]),
        ]));
        g.add_rule(Rule::new("A", vec![
            Production::new(vec![Symbol::term("a")]),
            Production::epsilon(),
        ]));
        let analysis = analyze(&g);
        // FOLLOW(A) should contain "b" (since S → A b).
        assert!(analysis.follow["A"].contains(&Symbol::term("b")));
        // FOLLOW(S) should contain $.
        assert!(analysis.follow["S"].contains(&Symbol::Eof));
    }

    #[test]
    fn detects_ll1_conflict() {
        let mut g = Grammar::new("S");
        // Both productions for S start with the same terminal — an LL(1) conflict.
        g.add_rule(Rule::new("S", vec![
            Production::new(vec![Symbol::term("a"), Symbol::term("b")]),
            Production::new(vec![Symbol::term("a"), Symbol::term("c")]),
        ]));
        let analysis = analyze(&g);
        assert!(!analysis.conflicts.is_empty(), "should detect LL(1) conflict");
        assert_eq!(analysis.conflicts[0].non_terminal, "S");
    }

    #[test]
    fn grammar_non_terminals_complete() {
        let g = canonical_grammar();
        let nts = g.non_terminals();
        // Check a sample of expected non-terminals.
        let expected = vec![
            "Program", "ItemList", "Item", "FnItem", "StructItem", "EnumItem",
            "Type", "Pattern", "Block", "Path", "Expr",
        ];
        for name in expected {
            assert!(nts.contains(&name), "missing non-terminal '{name}'");
        }
    }
}
