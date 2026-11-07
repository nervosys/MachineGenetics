//! The grammar in `kb/grammar.mg`, run directly (NEUROSYMBOLIC_PLAN.md
//! Phase 2): an Earley recogniser over the lexer's token kinds, built from
//! the knowledge base's `rhs` facts and nothing else.
//!
//! An `rhs(P, "…")` fact is one alternative of production `P`, written in
//! EBNF over production names and `TokenKind` names: juxtaposition is
//! sequence, `|` alternation, `?` `*` `+` the usual repetitions, and
//! parentheses group. A quoted spelling (`'block'`) is a terminal that
//! matches a token by its text, whatever its kind: where the parser tests an
//! identifier's spelling, the grammar can say which. Earley accepts any context-free grammar, left
//! recursion and ambiguity included, so the facts can say what the language
//! is without being shaped for a particular parsing method.
//!
//! What the grammar cannot say: the parser reads layout (an indented body
//! with no braces) and statement ends (a newline in place of `;`) from line
//! and column, which a grammar over token kinds does not see. The
//! recogniser drops nothing and inserts nothing, so a source that relies on
//! either is not recognised; the corpus test lists such files by name.

use std::collections::{BTreeSet, HashMap, HashSet};

/// One symbol of a rule's right-hand side.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Sym {
    /// A token kind, by name.
    T(String),
    /// A token with exactly this text, of any kind.
    S(String),
    /// A nonterminal, by index into `Grammar::names`.
    N(usize),
}

/// The grammar facts, lowered to plain BNF.
pub struct Grammar {
    names: Vec<String>,
    rules: Vec<(usize, Vec<Sym>)>,
    by_lhs: Vec<Vec<usize>>,
    nullable: Vec<bool>,
    /// The augmented start rule's left-hand side.
    start: usize,
}

/// Where recognition stopped: the token index, and the token kinds that
/// would have let it continue.
#[derive(Debug, PartialEq, Eq)]
pub struct Stall {
    pub at: usize,
    pub expected: BTreeSet<String>,
}

/// The names an EBNF right-hand side mentions, in order, or why it does not
/// read. The knowledge base derives `derives` facts from these.
pub fn names_in(rhs: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for t in tokens(rhs)? {
        if let Tok::Name(n) = t {
            out.push(n);
        }
    }
    let mut r = Reader { toks: tokens(rhs)?, pos: 0 };
    r.alts(&mut |_| 0, &mut Vec::new())?;
    if r.pos != r.toks.len() {
        return Err(format!("unexpected `{}`", r.toks[r.pos]));
    }
    Ok(out)
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Name(String),
    Punct(char),
    Lit(String),
}

impl std::fmt::Display for Tok {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tok::Name(n) => f.write_str(n),
            Tok::Punct(c) => write!(f, "{c}"),
            Tok::Lit(s) => write!(f, "'{s}'"),
        }
    }
}

fn tokens(rhs: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut chars = rhs.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_alphabetic() || c == '_' {
            let mut n = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_alphanumeric() || c == '_' {
                    n.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(Tok::Name(n));
        } else if "()|?*+".contains(c) {
            out.push(Tok::Punct(c));
            chars.next();
        } else if c == '\'' {
            chars.next();
            let mut s = String::new();
            loop {
                match chars.next() {
                    Some('\'') => break,
                    Some(c) => s.push(c),
                    None => return Err("a `'` is not closed".to_string()),
                }
            }
            if s.is_empty() {
                return Err("an empty spelling `''`".to_string());
            }
            out.push(Tok::Lit(s));
        } else {
            return Err(format!("`{c}` is not EBNF here"));
        }
    }
    Ok(out)
}

/// A recursive-descent reader of EBNF. `name` resolves a name to a symbol
/// index (or 0 when only checking), and `fresh` collects the rules of the
/// helper nonterminals that groups and repetitions introduce.
struct Reader {
    toks: Vec<Tok>,
    pos: usize,
}

/// A helper nonterminal's alternatives, before it has an index.
type Helper = Vec<Vec<SymOrHelper>>;

#[derive(Clone, Debug)]
enum SymOrHelper {
    Name(String),
    Lit(String),
    /// Index into the helper list.
    Helper(usize),
}

impl Reader {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn alts(
        &mut self,
        name: &mut dyn FnMut(&str) -> usize,
        helpers: &mut Vec<Helper>,
    ) -> Result<Vec<Vec<SymOrHelper>>, String> {
        let mut alts = vec![self.seq(name, helpers)?];
        while self.peek() == Some(&Tok::Punct('|')) {
            self.pos += 1;
            alts.push(self.seq(name, helpers)?);
        }
        Ok(alts)
    }

    fn seq(
        &mut self,
        name: &mut dyn FnMut(&str) -> usize,
        helpers: &mut Vec<Helper>,
    ) -> Result<Vec<SymOrHelper>, String> {
        let mut seq = Vec::new();
        loop {
            let mut item = match self.peek() {
                Some(Tok::Name(n)) => {
                    let n = n.clone();
                    self.pos += 1;
                    name(&n);
                    SymOrHelper::Name(n)
                }
                Some(Tok::Lit(s)) => {
                    let s = s.clone();
                    self.pos += 1;
                    SymOrHelper::Lit(s)
                }
                Some(Tok::Punct('(')) => {
                    self.pos += 1;
                    let inner = self.alts(name, helpers)?;
                    if self.peek() != Some(&Tok::Punct(')')) {
                        return Err("a `(` is not closed".to_string());
                    }
                    self.pos += 1;
                    helpers.push(inner);
                    SymOrHelper::Helper(helpers.len() - 1)
                }
                _ => return Ok(seq),
            };
            while let Some(Tok::Punct(op @ ('?' | '*' | '+'))) = self.peek() {
                let op = *op;
                self.pos += 1;
                let me = helpers.len();
                helpers.push(match op {
                    '?' => vec![vec![], vec![item]],
                    '*' => vec![vec![], vec![SymOrHelper::Helper(me), item]],
                    _ => vec![vec![item.clone()], vec![SymOrHelper::Helper(me), item]],
                });
                item = SymOrHelper::Helper(me);
            }
            seq.push(item);
        }
    }
}

impl Grammar {
    /// Build the grammar from the knowledge base: productions are the
    /// `Production` instances, terminals the `TokenKind` instances, the
    /// alternatives the `rhs` facts, and the start the `start_symbol`.
    pub fn from_kb(kb: &crate::kb::Kb) -> Result<Grammar, Vec<String>> {
        let class = |c: &str| -> BTreeSet<String> {
            kb.facts("instance_of").into_iter().filter(|f| f[1] == c).map(|f| f[0].clone()).collect()
        };
        let productions = class("Production");
        let kinds = class("TokenKind");
        let mut names: Vec<String> = productions.iter().cloned().collect();
        let index: HashMap<String, usize> = names.iter().cloned().enumerate().map(|(i, n)| (n, i)).collect();
        let mut rules: Vec<(usize, Vec<Sym>)> = Vec::new();
        let mut errors = Vec::new();

        for f in kb.facts("rhs") {
            let (lhs, text) = (&f[0], &f[1]);
            let Some(&l) = index.get(lhs) else {
                errors.push(format!("`rhs({lhs}, …)`: `{lhs}` is not a Production"));
                continue;
            };
            let toks = match tokens(text) {
                Ok(t) => t,
                Err(e) => {
                    errors.push(format!("the rhs `{text}` of `{lhs}`: {e}"));
                    continue;
                }
            };
            let mut bad = Vec::new();
            let mut helpers = Vec::new();
            let mut r = Reader { toks, pos: 0 };
            let alts = r.alts(
                &mut |n: &str| {
                    if !index.contains_key(n) && !kinds.contains(n) {
                        bad.push(n.to_string());
                    }
                    0
                },
                &mut helpers,
            );
            let alts = match alts {
                Ok(a) if r.pos == r.toks.len() => a,
                Ok(_) => {
                    errors.push(format!("the rhs `{text}` of `{lhs}`: unexpected `{}`", r.toks[r.pos]));
                    continue;
                }
                Err(e) => {
                    errors.push(format!("the rhs `{text}` of `{lhs}`: {e}"));
                    continue;
                }
            };
            for n in bad {
                errors.push(format!("the rhs `{text}` of `{lhs}` names `{n}`, which is neither a Production nor a TokenKind"));
            }
            // Give each helper an index, then lower everything.
            let base = names.len();
            for i in 0..helpers.len() {
                names.push(format!("{lhs}#{i}"));
            }
            let lower = |s: &SymOrHelper| -> Sym {
                match s {
                    SymOrHelper::Name(n) => match index.get(n) {
                        Some(&i) => Sym::N(i),
                        None => Sym::T(n.clone()),
                    },
                    SymOrHelper::Lit(s) => Sym::S(s.clone()),
                    SymOrHelper::Helper(h) => Sym::N(base + h),
                }
            };
            for alt in &alts {
                rules.push((l, alt.iter().map(lower).collect()));
            }
            for (i, h) in helpers.iter().enumerate() {
                for alt in h {
                    rules.push((base + i, alt.iter().map(lower).collect()));
                }
            }
        }

        let start_name = match kb.facts("start_symbol").as_slice() {
            [one] => one[0].clone(),
            other => {
                errors.push(format!("the grammar needs one start_symbol, and has {}", other.len()));
                String::new()
            }
        };
        if !errors.is_empty() {
            return Err(errors);
        }
        let start = names.len();
        names.push("^start".to_string());
        rules.push((start, vec![Sym::N(index[&start_name])]));

        let mut by_lhs = vec![Vec::new(); names.len()];
        for (i, (l, _)) in rules.iter().enumerate() {
            by_lhs[*l].push(i);
        }
        // Nullable nonterminals, to a fixpoint.
        let mut nullable = vec![false; names.len()];
        loop {
            let mut changed = false;
            for (l, rhs) in &rules {
                if !nullable[*l] && rhs.iter().all(|s| matches!(s, Sym::N(n) if nullable[*n])) {
                    nullable[*l] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(Grammar { names, rules, by_lhs, nullable, start })
    }

    /// The number of BNF rules, helpers included.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Recognise a sequence of tokens, each (kind name, text), no `Eof`.
    pub fn recognise(&self, input: &[(String, String)]) -> Result<(), Stall> {
        // An item: (rule, dot, origin).
        type Item = (usize, usize, usize);
        let n = input.len();
        let mut sets: Vec<Vec<Item>> = vec![Vec::new(); n + 1];
        let mut seen: Vec<HashSet<Item>> = vec![HashSet::new(); n + 1];
        let add = |sets: &mut Vec<Vec<Item>>, seen: &mut Vec<HashSet<Item>>, k: usize, it: Item| {
            if seen[k].insert(it) {
                sets[k].push(it);
            }
        };
        for &r in &self.by_lhs[self.start] {
            add(&mut sets, &mut seen, 0, (r, 0, 0));
        }
        for k in 0..=n {
            let mut i = 0;
            while i < sets[k].len() {
                let (r, dot, origin) = sets[k][i];
                let (lhs, rhs) = &self.rules[r];
                match rhs.get(dot) {
                    Some(Sym::N(b)) => {
                        for &r2 in &self.by_lhs[*b] {
                            add(&mut sets, &mut seen, k, (r2, 0, k));
                        }
                        if self.nullable[*b] {
                            add(&mut sets, &mut seen, k, (r, dot + 1, origin));
                        }
                    }
                    Some(Sym::T(t)) => {
                        if k < n && input[k].0 == *t {
                            add(&mut sets, &mut seen, k + 1, (r, dot + 1, origin));
                        }
                    }
                    Some(Sym::S(s)) => {
                        if k < n && input[k].1 == *s {
                            add(&mut sets, &mut seen, k + 1, (r, dot + 1, origin));
                        }
                    }
                    None => {
                        // Complete: advance every item in the origin set
                        // waiting on `lhs`. The origin set may be this one.
                        let mut j = 0;
                        while j < sets[origin].len() {
                            let (r2, d2, o2) = sets[origin][j];
                            if self.rules[r2].1.get(d2) == Some(&Sym::N(*lhs)) {
                                add(&mut sets, &mut seen, k, (r2, d2 + 1, o2));
                            }
                            j += 1;
                        }
                    }
                }
                i += 1;
            }
            if k < n && sets[k + 1].is_empty() {
                return Err(self.stall(&sets[k], k));
            }
        }
        let accepted = sets[n]
            .iter()
            .any(|&(r, dot, origin)| origin == 0 && self.rules[r].0 == self.start && dot == self.rules[r].1.len());
        if accepted {
            Ok(())
        } else {
            Err(self.stall(&sets[n], n))
        }
    }

    fn stall(&self, set: &[(usize, usize, usize)], at: usize) -> Stall {
        let expected = set
            .iter()
            .filter_map(|&(r, dot, _)| match self.rules[r].1.get(dot) {
                Some(Sym::T(t)) => Some(t.clone()),
                Some(Sym::S(s)) => Some(format!("'{s}'")),
                _ => None,
            })
            .collect();
        Stall { at, expected }
    }

    /// A production's name, for messages (helpers print as `P#i`).
    pub fn name(&self, i: usize) -> &str {
        &self.names[i]
    }
}

impl Grammar {
    /// Each nonterminal's height: the depth of its shallowest complete
    /// derivation. `usize::MAX` for one that derives no finite sentence.
    /// A rule that needs a token kind `spellable` refuses derives nothing.
    fn heights(&self, spellable: &HashSet<String>) -> Vec<usize> {
        let mut h = vec![usize::MAX; self.names.len()];
        loop {
            let mut changed = false;
            for (l, rhs) in &self.rules {
                let mut deepest = 0usize;
                let mut finite = true;
                for s in rhs {
                    match s {
                        Sym::N(n) if h[*n] == usize::MAX => finite = false,
                        Sym::N(n) => deepest = deepest.max(h[*n]),
                        Sym::T(t) if !spellable.contains(t) => finite = false,
                        _ => {}
                    }
                    if !finite {
                        break;
                    }
                }
                if finite && deepest + 1 < h[*l] {
                    h[*l] = deepest + 1;
                    changed = true;
                }
            }
            if !changed {
                return h;
            }
        }
    }

    /// One sentence, derived at random from the start symbol: token kind
    /// names, and quoted spellings as `'text'`. Rules that need a kind not in
    /// `spellable` are never chosen. Past `depth` levels every choice takes
    /// the shallowest alternative, so the derivation ends. Deterministic in
    /// `seed`.
    pub fn sample(&self, seed: u64, depth: usize, spellable: &HashSet<String>) -> Vec<String> {
        let heights = self.heights(spellable);
        let mut rng = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut out = Vec::new();
        // A stack of (symbol, depth); expanded leftmost first.
        let mut stack: Vec<(Sym, usize)> = vec![(Sym::N(self.start), 0)];
        while let Some((sym, d)) = stack.pop() {
            match sym {
                Sym::T(t) => out.push(t),
                Sym::S(s) => out.push(format!("'{s}'")),
                Sym::N(n) => {
                    let rules = &self.by_lhs[n];
                    let height = |r: usize| {
                        self.rules[r].1.iter().fold(0usize, |m, s| match s {
                            Sym::N(c) => m.max(heights[*c]),
                            Sym::T(t) if !spellable.contains(t) => usize::MAX,
                            _ => m,
                        })
                    };
                    let r = if d >= depth || out.len() > 400 {
                        *rules.iter().min_by_key(|&&r| height(r)).expect("a nonterminal has a rule")
                    } else {
                        let finite: Vec<usize> = rules.iter().copied().filter(|&r| height(r) != usize::MAX).collect();
                        finite[(next() % finite.len() as u64) as usize]
                    };
                    for s in self.rules[r].1.iter().rev() {
                        stack.push((s.clone(), d + 1));
                    }
                }
            }
        }
        out
    }
}

/// The spelling the sampler writes for each token kind: a plain `Token`
/// before a sigil or keyword, then the shortest, then the first in order.
pub fn spellings(kb: &crate::kb::Kb) -> HashMap<String, String> {
    let plain: HashSet<String> = kb
        .facts("instance_of")
        .into_iter()
        .filter(|f| f[1] == "Token")
        .map(|f| f[0].clone())
        .collect();
    let mut best: HashMap<String, String> = HashMap::new();
    for f in kb.facts("lexes_as") {
        let (s, k) = (&f[0], &f[1]);
        let key = |s: &str| (!plain.contains(s), s.chars().count(), s.to_string());
        let better = match best.get(k) {
            None => true,
            Some(cur) => key(s) < key(cur),
        };
        if better {
            best.insert(k.clone(), s.clone());
        }
    }
    best
}

/// A source text for a sampled sentence: each kind written with its
/// spelling, each quoted spelling as itself, separated by spaces.
pub fn spell(spellings: &HashMap<String, String>, sentence: &[String]) -> Result<String, String> {
    let mut words = Vec::with_capacity(sentence.len());
    for k in sentence {
        if let Some(text) = k.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')) {
            words.push(text.to_string());
        } else {
            words.push(spellings.get(k).cloned().ok_or_else(|| format!("no spelling for {k}"))?);
        }
    }
    Ok(words.join(" "))
}

/// The token kind names of `src`, as the recogniser reads them.
pub fn kinds_of(src: &str) -> Vec<(String, String, usize)> {
    tokens_of(&crate::lexer::lex(src))
}

/// Lexer tokens as the recogniser reads them: (kind, text, line), no `Eof`.
pub fn tokens_of(toks: &[crate::lexer::Token]) -> Vec<(String, String, usize)> {
    toks.iter()
        .map(|t| (format!("{:?}", t.kind), t.text.clone(), t.span.line))
        .filter(|(k, _, _)| k != "Eof")
        .collect()
}

/// Recognise `src` with the knowledge base's grammar. The error names the
/// line, the token found and the kinds that would have continued.
pub fn recognise_source(g: &Grammar, src: &str) -> Result<(), String> {
    let toks = kinds_of(src);
    let input: Vec<(String, String)> = toks.iter().map(|(k, t, _)| (k.clone(), t.clone())).collect();
    g.recognise(&input).map_err(|s| {
        let (found, line) = toks.get(s.at).map(|(k, _, l)| (k.as_str(), *l)).unwrap_or(("end of input", 0));
        let expected: Vec<&str> = s.expected.iter().map(|e| e.as_str()).collect();
        format!("line {line}: found {found}; expected one of {}", expected.join(" "))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(grammar: &str) -> crate::kb::Kb {
        let src = format!("kb G {{\n{grammar}\n}}\n");
        let files = [crate::kb::FILES[0], crate::kb::FILES[1], ("kb/g.mg", src.as_str())];
        crate::kb::load_files(&files).unwrap_or_else(|e| panic!("{e:#?}"))
    }

    fn words(s: &str) -> Vec<(String, String)> {
        s.split_whitespace().map(|w| (w.to_string(), w.to_string())).collect()
    }

    /// Left recursion, nullable helpers and groups, which a recursive-descent
    /// reading of the same facts would not survive.
    #[test]
    fn earley_accepts_left_recursion_and_repetition() {
        let k = kb("fact instance_of(Sum, Production);\n\
                    fact instance_of(List, Production);\n\
                    fact start_symbol(List);\n\
                    fact rhs(Sum, \"Sum Plus IntLiteral\");\n\
                    fact rhs(Sum, \"IntLiteral\");\n\
                    fact rhs(List, \"LBrack (Sum (Comma Sum)* Comma?)? RBrack\");");
        let g = Grammar::from_kb(&k).unwrap_or_else(|e| panic!("{e:#?}"));
        let i = "IntLiteral";
        for ok in ["LBrack RBrack".to_string(), format!("LBrack {i} RBrack"), format!("LBrack {i} Plus {i} Comma {i} Comma RBrack")] {
            assert_eq!(g.recognise(&words(&ok)), Ok(()), "{ok}");
        }
        let e = g.recognise(&words(&format!("LBrack {i} {i} RBrack"))).unwrap_err();
        assert_eq!(e.at, 2);
        assert!(e.expected.contains("Plus") && e.expected.contains("Comma") && e.expected.contains("RBrack"), "{e:?}");
        assert!(g.recognise(&words(&format!("LBrack {i}"))).is_err());
    }

    /// A quoted spelling matches a token by its text, whatever its kind,
    /// and is not a name: it derives nothing.
    #[test]
    fn a_quoted_spelling_matches_by_text() {
        let k = kb("fact instance_of(Block, Production);
                    fact start_symbol(Block);
                    fact rhs(Block, \"'block' Ident LBrace RBrace\");");
        let g = Grammar::from_kb(&k).unwrap_or_else(|e| panic!("{e:#?}"));
        let toks = |a: &str| -> Vec<(String, String)> {
            vec![("Ident".into(), a.into()), ("Ident".into(), "b".into()), ("LBrace".into(), "{".into()), ("RBrace".into(), "}".into())]
        };
        assert_eq!(g.recognise(&toks("block")), Ok(()));
        let e = g.recognise(&toks("other")).unwrap_err();
        assert_eq!((e.at, e.expected.iter().cloned().collect::<Vec<_>>()), (0, vec!["'block'".to_string()]));
        assert_eq!(names_in("'block' Ident").unwrap(), vec!["Ident".to_string()]);
    }

    /// A name that is neither a production nor a token kind is refused.
    #[test]
    fn unknown_symbols_are_refused() {
        let k = kb("fact instance_of(A, Production);\n\
                    fact start_symbol(A);\n\
                    fact rhs(A, \"IntLiteral Frobnicate\");");
        let e = Grammar::from_kb(&k).err().expect("refused");
        assert!(e.iter().any(|m| m.contains("names `Frobnicate`")), "{e:#?}");
    }

    /// Sources the grammar does not recognise yet, by path. Every other
    /// corpus source must be recognised, and a listed one that is recognised
    /// fails too, so the list only shrinks.
    const NOT_YET: &[&str] = &[];

    /// The corpus direction of conformance: the kernel parser accepts every
    /// `.mg` source in the repository, and so must the grammar facts. The
    /// files in `kb/` are part of the corpus, so the knowledge base is
    /// recognised by its own grammar.
    #[test]
    fn the_grammar_recognises_the_corpus() {
        let kb = crate::kb::load().unwrap_or_else(|e| panic!("{e:#?}"));
        let g = Grammar::from_kb(&kb).unwrap_or_else(|e| panic!("{e:#?}"));
        let mut failures = Vec::new();
        for (path, src) in crate::kb::corpus_files() {
            let r = recognise_source(&g, &src);
            if NOT_YET.contains(&path.as_str()) {
                assert!(r.is_err(), "{path} is recognised now: take it off NOT_YET");
            } else if let Err(e) = r {
                failures.push(format!("{path}: {e}"));
            }
        }
        assert!(failures.is_empty(), "{} source(s) not recognised:\n{}", failures.len(), failures.join("\n"));
    }

    /// The grammar is not a sieve that passes everything: delete one token
    /// from each corpus source at three places, keep the mutants the kernel
    /// parser rejects, and the grammar must reject nearly all of them too.
    /// The ones it accepts are its documented supersets: with no statement
    /// ends in view, two expressions side by side read as two statements.
    /// (2026-10-29: 243 of 249 rejected.)
    #[test]
    fn the_grammar_rejects_what_the_parser_rejects() {
        let kb = crate::kb::load().expect("loads");
        let g = Grammar::from_kb(&kb).expect("builds");
        let (mut total, mut rejected) = (0, 0);
        for (_, src) in crate::kb::corpus_files() {
            let toks = crate::lexer::lex(&src);
            let n = toks.len().saturating_sub(1);
            for cut in [n / 3, n / 2, 2 * n / 3] {
                if cut >= n {
                    continue;
                }
                let mut mutant = toks.clone();
                mutant.remove(cut);
                if crate::parser::parse(&mutant).is_ok() {
                    continue;
                }
                total += 1;
                let input: Vec<(String, String)> =
                    tokens_of(&mutant).into_iter().map(|(k, t, _)| (k, t)).collect();
                if g.recognise(&input).is_err() {
                    rejected += 1;
                }
            }
        }
        assert!(total >= 200, "only {total} mutants: the corpus or the cut points changed");
        assert!(rejected * 100 >= total * 95, "the grammar rejects only {rejected} of {total} mutants the parser rejects");
    }

    /// The other direction: sentences sampled from the grammar, written out
    /// with the lexicon's spellings, parse. Where the parser refuses one, it
    /// stopped in a production the grammar marks `GAP` (a superset the token
    /// kinds cannot narrow): `Generics`, which the parser commits to on `[`
    /// after `I` where the grammar may read a type, and `DoComposeBody`, whose
    /// block references must name a declared block.
    #[test]
    fn sampled_sentences_parse() {
        const GAPS: &[&str] = &["Generics", "DoComposeBody"];
        let kb = crate::kb::load().expect("loads");
        let g = Grammar::from_kb(&kb).expect("builds");
        let sp = spellings(&kb);
        let spellable: HashSet<String> = sp.keys().cloned().collect();
        let (mut ok, total) = (0, 300);
        for seed in 1..=total as u64 {
            let src = spell(&sp, &g.sample(seed, 8, &spellable)).expect("spells");
            match crate::parser::parse(&crate::lexer::lex(&src)) {
                Ok(_) => ok += 1,
                Err(e) => assert!(
                    e.production.is_some_and(|p| GAPS.contains(&p)),
                    "seed {seed}: {}\n{src}",
                    e.describe()
                ),
            }
        }
        assert!(ok * 100 >= total * 98, "only {ok} of {total} sampled sentences parse");
    }

    /// A malformed right-hand side is refused when the knowledge base loads,
    /// with the file and line.
    #[test]
    fn bad_ebnf_is_refused_at_load() {
        let src = "kb G {\nfact instance_of(A, Production);\nfact start_symbol(A);\nfact rhs(A, \"(IntLiteral\");\n}\n";
        let files = [crate::kb::FILES[0], crate::kb::FILES[1], ("kb/g.mg", src)];
        let e = crate::kb::load_files(&files).err().expect("refused");
        assert!(e.iter().any(|m| m.starts_with("kb/g.mg:4: the rhs `(IntLiteral` of `A` does not read")), "{e:#?}");
    }

    /// Every production a parse error can name is a production of the
    /// grammar: the parser's `within("…")` names, read from its source.
    #[test]
    fn parse_errors_name_grammar_productions() {
        let kb = crate::kb::load().expect("loads");
        let productions: std::collections::BTreeSet<String> = kb
            .facts("instance_of")
            .into_iter()
            .filter(|f| f[1] == "Production")
            .map(|f| f[0].clone())
            .collect();
        let src = include_str!("parser.rs");
        let names: Vec<&str> = src
            .split("self.within(\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect();
        assert!(names.len() >= 40, "found {} within() names", names.len());
        for n in names {
            assert!(productions.contains(n), "parser.rs names `{n}`, which kb/grammar.mg does not define");
        }
    }
}

