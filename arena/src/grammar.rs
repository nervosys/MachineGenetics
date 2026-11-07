//! The generator's program space: typed MAGE expressions over integer lists.
//!
//! Programs are trees, not strings, so that every operator a search applies —
//! sampling, mutation, crossover — produces something with the right *shape*
//! by construction. Whether it is a valid *MAGE program* is still decided by
//! the compiler in [`crate::substrate`], not assumed here: the tree guarantees
//! well-formedness, the typechecker adjudicates.
//!
//! ## The vocabulary
//!
//! A list expression is one of `range(n)`, `[e; n]`, `xs.map(|x| e)`,
//! `xs.scan(e, |a, x| e)`, `xs.filter(|x| x % k == r)`, `xs.reverse()`,
//! `xs.sort()`, `xs.take(n)`, `flatten([xs, ys])` and `xs.map(|x| ys.fold(…))`
//! is left out deliberately (see below). An integer expression is a literal, a
//! variable in scope, or `+ − * %` of two integer expressions. Everything is
//! `usize`, because `range` is, and the typechecker refuses to mix it with
//! `i64` — measured, not assumed.
//!
//! That is a combinator language rather than a register machine, which is the
//! point: `scan` is iteration with state, `map` is element-wise composition,
//! `flatten` is concatenation, and nesting them is hierarchical composition —
//! the "generic predictive regularities" (copying, recursion, composition)
//! self-play pretraining found transfer to natural data.
//!
//! ## Sizes are bounded by construction
//!
//! `range` and `[e; n]` take literals in `1..=MAX_LEN`, and trees are depth
//! limited, so no program here can ask for a list the substrate would refuse
//! for size. Fuel still bounds everything — a mutation or an agent from outside
//! this module can write anything — but a policy should not spend its budget
//! learning not to write `range(10^12)`.
//!
//! ## The policy is a probabilistic grammar
//!
//! [`Policy`] holds one logit per production. Sampling records the trace of
//! choices; [`Policy::reinforce`] moves those logits by an advantage —
//! REINFORCE on a PCFG. It is the smallest learnable generator that is honestly
//! a policy, and it is the right size for what the arena needs to show first:
//! that a learned generator beats a fixed prior at equal cost. An LLM agent is
//! a [`crate::agents::Proposer`] like any other; nothing here assumes this one.

use serde::{Deserialize, Serialize};

pub const MAX_LEN: u64 = 48;
pub const MAX_DEPTH: usize = 5;

/// An integer-valued expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntExpr {
    Lit(u64),
    /// A variable by de Bruijn-free name: `s`, or a lambda parameter.
    Var(String),
    Add(Box<IntExpr>, Box<IntExpr>),
    Sub(Box<IntExpr>, Box<IntExpr>),
    Mul(Box<IntExpr>, Box<IntExpr>),
    /// Modulo by a literal in `2..=256`, so there is no division by zero.
    Mod(Box<IntExpr>, u64),
}

/// A list-valued expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ListExpr {
    Range(u64),
    Repeat(IntExpr, u64),
    Map(Box<ListExpr>, String, IntExpr),
    Scan(Box<ListExpr>, IntExpr, String, String, IntExpr),
    Filter(Box<ListExpr>, String, u64, u64),
    Reverse(Box<ListExpr>),
    Sort(Box<ListExpr>),
    Take(Box<ListExpr>, u64),
    Concat(Box<ListExpr>, Box<ListExpr>),
    /// `flatten([xs; n])` — xs, n times over. Copying.
    Copy(Box<ListExpr>, u64),
    /// `range(len).map(|v| [e₀, …, eₖ₋₁][v % k])` — a periodic template.
    Cycle(String, Vec<IntExpr>, u64),
    /// `flatten(range(n).map(|v| range(v % k + 1)))` — nested counting.
    Nest(String, u64, u64),
}

/// A whole generated program: `f gen(s: usize) -> [usize] { body }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Program {
    pub body: ListExpr,
}

impl IntExpr {
    fn render(&self, out: &mut String) {
        match self {
            IntExpr::Lit(n) => out.push_str(&n.to_string()),
            IntExpr::Var(v) => out.push_str(v),
            // Wrapping by name: MAGE's operators trap on overflow (§4.10), and
            // a generator of byte streams wants modular arithmetic, so it asks
            // for it where a reader can see it.
            IntExpr::Add(a, b) => call(out, "wrapping_add", a, b),
            IntExpr::Sub(a, b) => call(out, "wrapping_sub", a, b),
            IntExpr::Mul(a, b) => call(out, "wrapping_mul", a, b),
            IntExpr::Mod(a, k) => {
                out.push('(');
                a.render(out);
                out.push_str(&format!(" % {k})"));
            }
        }
    }

    fn size(&self) -> usize {
        match self {
            IntExpr::Lit(_) | IntExpr::Var(_) => 1,
            IntExpr::Add(a, b) | IntExpr::Sub(a, b) | IntExpr::Mul(a, b) => 1 + a.size() + b.size(),
            IntExpr::Mod(a, _) => 1 + a.size(),
        }
    }
}

fn call(out: &mut String, f: &str, a: &IntExpr, b: &IntExpr) {
    out.push_str(f);
    out.push('(');
    a.render(out);
    out.push_str(", ");
    b.render(out);
    out.push(')');
}

impl ListExpr {
    fn render(&self, out: &mut String) {
        match self {
            ListExpr::Range(n) => out.push_str(&format!("range({n})")),
            ListExpr::Repeat(e, n) => {
                out.push('[');
                e.render(out);
                out.push_str(&format!("; {n}]"));
            }
            ListExpr::Map(xs, v, e) => {
                xs.render(out);
                out.push_str(&format!(".map(|{v}| "));
                e.render(out);
                out.push(')');
            }
            ListExpr::Scan(xs, init, a, x, e) => {
                xs.render(out);
                out.push_str(".scan(");
                init.render(out);
                out.push_str(&format!(", |{a}, {x}| "));
                e.render(out);
                out.push(')');
            }
            ListExpr::Filter(xs, v, k, r) => {
                xs.render(out);
                out.push_str(&format!(".filter(|{v}| {v} % {k} == {r})"));
            }
            ListExpr::Reverse(xs) => {
                xs.render(out);
                out.push_str(".reverse()");
            }
            ListExpr::Sort(xs) => {
                xs.render(out);
                out.push_str(".sort()");
            }
            ListExpr::Take(xs, n) => {
                xs.render(out);
                out.push_str(&format!(".take({n})"));
            }
            ListExpr::Concat(a, b) => {
                out.push_str("flatten([");
                a.render(out);
                out.push_str(", ");
                b.render(out);
                out.push_str("])");
            }
            ListExpr::Copy(xs, n) => {
                out.push_str("flatten([");
                xs.render(out);
                out.push_str(&format!("; {n}])"));
            }
            ListExpr::Cycle(v, elems, len) => {
                out.push_str(&format!("range({len}).map(|{v}| ["));
                for (i, e) in elems.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    e.render(out);
                }
                out.push_str(&format!("][{v} % {}])", elems.len()));
            }
            ListExpr::Nest(v, n, k) => {
                out.push_str(&format!("flatten(range({n}).map(|{v}| range({v} % {k} + 1)))"));
            }
        }
    }

    pub fn size(&self) -> usize {
        match self {
            ListExpr::Range(_) => 1,
            ListExpr::Repeat(e, _) => 1 + e.size(),
            ListExpr::Map(xs, _, e) => 1 + xs.size() + e.size(),
            ListExpr::Scan(xs, i, _, _, e) => 1 + xs.size() + i.size() + e.size(),
            ListExpr::Filter(xs, ..) | ListExpr::Reverse(xs) | ListExpr::Sort(xs) | ListExpr::Take(xs, _) => {
                1 + xs.size()
            }
            ListExpr::Concat(a, b) => 1 + a.size() + b.size(),
            ListExpr::Copy(xs, _) => 1 + xs.size(),
            ListExpr::Cycle(_, elems, _) => 1 + elems.iter().map(IntExpr::size).sum::<usize>(),
            ListExpr::Nest(..) => 1,
        }
    }
}

impl Program {
    /// The MAGE source this tree denotes.
    pub fn source(&self) -> String {
        let mut body = String::new();
        self.body.render(&mut body);
        // `@role(candidate)`: the language, not this crate, is what forbids a
        // generated program from reading held-out data or acting (§11.6).
        format!("@role(candidate)\nf gen(s: usize) -> [usize] {{\n    {body}\n}}\n")
    }

    /// Length in MAGE tokens — the description length the Solomonoff prior
    /// charges for. Measured with the real lexer, so what "short" means is
    /// what MAGE's token-minimal surface makes it mean.
    pub fn token_len(&self) -> usize {
        mage_prototype::lexer::lex(&self.source()).len()
    }

    /// Length in the configured unit (plan S1): `Tokens` is [`Program::token_len`],
    /// `Bytes` is the UTF-8 length of [`Program::source`], the text it lexes.
    pub fn length(&self, unit: crate::arena::LengthUnit) -> usize {
        match unit {
            crate::arena::LengthUnit::Tokens => self.token_len(),
            crate::arena::LengthUnit::Bytes => self.source().len(),
        }
    }

    pub fn size(&self) -> usize {
        self.body.size()
    }
}

// ── Randomness ───────────────────────────────────────────────────────────

/// SplitMix64, the same generator germline uses, so a seed means one thing
/// across the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in `lo..=hi`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.next_u64() % (hi - lo + 1)
    }

    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

// ── The policy ───────────────────────────────────────────────────────────

/// List productions, in logit order.
///
/// `copy`, `cycle` and `nest` were added on 2026-09-25 after the transformer
/// learner could fit the generated data only to 5.9 bits per byte: chained
/// modular arithmetic hashes its inputs, so most programs emitted noise, and
/// the only learnable structure was constant runs. These three produce the
/// regularities self-play pretraining found to transfer — copying, periodic
/// templates, and nested (recursive) counting.
pub const LIST_RULES: [&str; 12] = [
    "range", "repeat", "map", "scan", "filter", "reverse", "sort", "take", "concat", "copy", "cycle", "nest",
];

/// List productions allowed at the depth limit: the leaves.
const LIST_LEAVES: [usize; 4] = [0, 1, 10, 11];
/// Integer productions, in logit order.
pub const INT_RULES: [&str; 6] = ["lit", "var", "add", "sub", "mul", "mod"];

/// One choice made while sampling: which nonterminal, which production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub list: bool,
    pub rule: usize,
}

/// A probabilistic grammar with learnable production logits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub list_logits: Vec<f64>,
    pub int_logits: Vec<f64>,
}

impl Default for Policy {
    /// Uniform over productions — the fixed prior a learned policy must beat.
    fn default() -> Self {
        Policy { list_logits: vec![0.0; LIST_RULES.len()], int_logits: vec![0.0; INT_RULES.len()] }
    }
}

fn softmax(logits: &[f64]) -> Vec<f64> {
    let m = logits.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let e: Vec<f64> = logits.iter().map(|l| (l - m).exp()).collect();
    let z: f64 = e.iter().sum();
    e.into_iter().map(|x| x / z).collect()
}

fn pick(rng: &mut Rng, probs: &[f64], allowed: &[bool]) -> usize {
    let mass: f64 = probs.iter().zip(allowed).filter(|(_, a)| **a).map(|(p, _)| p).sum();
    let mut u = rng.next_f64() * mass;
    for (i, (p, a)) in probs.iter().zip(allowed).enumerate() {
        if !*a {
            continue;
        }
        if u < *p {
            return i;
        }
        u -= p;
    }
    allowed.iter().rposition(|a| *a).expect("some production is always allowed")
}

struct Sampler<'a> {
    policy: &'a Policy,
    rng: &'a mut Rng,
    trace: Vec<Choice>,
    fresh: usize,
}

impl Sampler<'_> {
    fn var(&mut self) -> String {
        self.fresh += 1;
        format!("v{}", self.fresh)
    }

    fn list(&mut self, depth: usize, scope: &[String]) -> ListExpr {
        let probs = softmax(&self.policy.list_logits);
        // At the depth limit only leaves are allowed.
        let leaf = depth >= MAX_DEPTH;
        let allowed: Vec<bool> = (0..LIST_RULES.len()).map(|i| !leaf || LIST_LEAVES.contains(&i)).collect();
        let rule = pick(self.rng, &probs, &allowed);
        self.trace.push(Choice { list: true, rule });
        match rule {
            0 => ListExpr::Range(self.rng.range(1, MAX_LEN)),
            1 => ListExpr::Repeat(self.int(depth + 1, scope), self.rng.range(1, MAX_LEN / 4)),
            2 => {
                let xs = self.list(depth + 1, scope);
                let v = self.var();
                let inner = [scope, std::slice::from_ref(&v)].concat();
                ListExpr::Map(Box::new(xs), v, self.int(depth + 1, &inner))
            }
            3 => {
                let xs = self.list(depth + 1, scope);
                let init = self.int(depth + 1, scope);
                let (a, x) = (self.var(), self.var());
                let inner = [scope, &[a.clone(), x.clone()]].concat();
                let body = self.int(depth + 1, &inner);
                ListExpr::Scan(Box::new(xs), init, a, x, body)
            }
            4 => {
                let xs = self.list(depth + 1, scope);
                let k = self.rng.range(2, 7);
                let r = self.rng.range(0, k - 1);
                ListExpr::Filter(Box::new(xs), self.var(), k, r)
            }
            5 => ListExpr::Reverse(Box::new(self.list(depth + 1, scope))),
            6 => ListExpr::Sort(Box::new(self.list(depth + 1, scope))),
            7 => ListExpr::Take(Box::new(self.list(depth + 1, scope)), self.rng.range(1, MAX_LEN)),
            8 => ListExpr::Concat(
                Box::new(self.list(depth + 1, scope)),
                Box::new(self.list(depth + 1, scope)),
            ),
            9 => ListExpr::Copy(Box::new(self.list(depth + 1, scope)), self.rng.range(2, 6)),
            10 => {
                // Template elements are literals or `s` — never arithmetic,
                // which is what hashed the output in the first place.
                let k = self.rng.range(2, 6) as usize;
                let elems = (0..k)
                    .map(|_| {
                        if self.rng.next_f64() < 0.25 {
                            IntExpr::Var(scope[self.rng.below(scope.len())].clone())
                        } else {
                            IntExpr::Lit(self.rng.range(0, 255))
                        }
                    })
                    .collect();
                ListExpr::Cycle(self.var(), elems, self.rng.range(4, MAX_LEN))
            }
            _ => ListExpr::Nest(self.var(), self.rng.range(2, 12), self.rng.range(2, 8)),
        }
    }

    fn int(&mut self, depth: usize, scope: &[String]) -> IntExpr {
        let probs = softmax(&self.policy.int_logits);
        let leaf = depth >= MAX_DEPTH + 2;
        let allowed: Vec<bool> =
            (0..INT_RULES.len()).map(|i| (!leaf || i <= 1) && (i != 1 || !scope.is_empty())).collect();
        let rule = pick(self.rng, &probs, &allowed);
        self.trace.push(Choice { list: false, rule });
        match rule {
            0 => IntExpr::Lit(self.rng.range(0, 255)),
            1 => IntExpr::Var(scope[self.rng.below(scope.len())].clone()),
            2 => IntExpr::Add(Box::new(self.int(depth + 1, scope)), Box::new(self.int(depth + 1, scope))),
            3 => IntExpr::Sub(Box::new(self.int(depth + 1, scope)), Box::new(self.int(depth + 1, scope))),
            4 => IntExpr::Mul(Box::new(self.int(depth + 1, scope)), Box::new(self.int(depth + 1, scope))),
            _ => IntExpr::Mod(Box::new(self.int(depth + 1, scope)), self.rng.range(2, 256)),
        }
    }
}

impl Policy {
    /// Sample a program and the trace of choices that produced it.
    pub fn sample(&self, rng: &mut Rng) -> (Program, Vec<Choice>) {
        let mut s = Sampler { policy: self, rng, trace: Vec::new(), fresh: 0 };
        let body = s.list(0, &["s".to_string()]);
        (Program { body }, s.trace)
    }

    /// REINFORCE: raise the log-probability of `trace` by `lr · advantage`.
    ///
    /// ∂ log π / ∂ logit_j = 1[j = chosen] − p_j for each choice, summed along
    /// the trace. Computed against the probabilities *before* the update, as
    /// the estimator requires.
    pub fn reinforce(&mut self, trace: &[Choice], advantage: f64, lr: f64) {
        let pl = softmax(&self.list_logits);
        let pi = softmax(&self.int_logits);
        let mut gl = vec![0.0; pl.len()];
        let mut gi = vec![0.0; pi.len()];
        for c in trace {
            let (g, p) = if c.list { (&mut gl, &pl) } else { (&mut gi, &pi) };
            for j in 0..g.len() {
                g[j] += (if j == c.rule { 1.0 } else { 0.0 }) - p[j];
            }
        }
        for (l, g) in self.list_logits.iter_mut().zip(gl) {
            *l += lr * advantage * g;
        }
        for (l, g) in self.int_logits.iter_mut().zip(gi) {
            *l += lr * advantage * g;
        }
    }

    /// Production probabilities, for the report.
    pub fn probabilities(&self) -> (Vec<f64>, Vec<f64>) {
        (softmax(&self.list_logits), softmax(&self.int_logits))
    }
}

// ── Variation on existing programs ───────────────────────────────────────

/// Every list-valued subtree, pre-order, as paths of child indices.
fn list_paths(e: &ListExpr, here: Vec<usize>, out: &mut Vec<Vec<usize>>) {
    out.push(here.clone());
    let kids: Vec<&ListExpr> = match e {
        ListExpr::Map(xs, ..)
        | ListExpr::Scan(xs, ..)
        | ListExpr::Filter(xs, ..)
        | ListExpr::Reverse(xs)
        | ListExpr::Sort(xs)
        | ListExpr::Take(xs, _) => vec![xs],
        ListExpr::Concat(a, b) => vec![a, b],
        ListExpr::Copy(xs, _) => vec![xs],
        ListExpr::Range(_) | ListExpr::Repeat(..) | ListExpr::Cycle(..) | ListExpr::Nest(..) => vec![],
    };
    for (i, k) in kids.into_iter().enumerate() {
        let mut p = here.clone();
        p.push(i);
        list_paths(k, p, out);
    }
}

fn at_mut<'a>(e: &'a mut ListExpr, path: &[usize]) -> &'a mut ListExpr {
    let Some((&first, rest)) = path.split_first() else { return e };
    let child: &mut ListExpr = match e {
        ListExpr::Map(xs, ..)
        | ListExpr::Scan(xs, ..)
        | ListExpr::Filter(xs, ..)
        | ListExpr::Reverse(xs)
        | ListExpr::Sort(xs)
        | ListExpr::Take(xs, _)
        | ListExpr::Copy(xs, _) => xs,
        ListExpr::Concat(a, b) => {
            if first == 0 {
                a
            } else {
                b
            }
        }
        _ => unreachable!("path descends into a leaf"),
    };
    at_mut(child, rest)
}

fn at<'a>(e: &'a ListExpr, path: &[usize]) -> &'a ListExpr {
    let Some((&first, rest)) = path.split_first() else { return e };
    let child: &ListExpr = match e {
        ListExpr::Map(xs, ..)
        | ListExpr::Scan(xs, ..)
        | ListExpr::Filter(xs, ..)
        | ListExpr::Reverse(xs)
        | ListExpr::Sort(xs)
        | ListExpr::Take(xs, _)
        | ListExpr::Copy(xs, _) => xs,
        ListExpr::Concat(a, b) => {
            if first == 0 {
                a
            } else {
                b
            }
        }
        _ => unreachable!("path descends into a leaf"),
    };
    at(child, rest)
}

/// Is `e` closed over `scope` — does every variable it names exist there?
fn closed(e: &ListExpr, scope: &mut Vec<String>) -> bool {
    fn int_closed(e: &IntExpr, scope: &[String]) -> bool {
        match e {
            IntExpr::Lit(_) => true,
            IntExpr::Var(v) => scope.contains(v),
            IntExpr::Add(a, b) | IntExpr::Sub(a, b) | IntExpr::Mul(a, b) => {
                int_closed(a, scope) && int_closed(b, scope)
            }
            IntExpr::Mod(a, _) => int_closed(a, scope),
        }
    }
    match e {
        ListExpr::Range(_) => true,
        ListExpr::Repeat(i, _) => int_closed(i, scope),
        ListExpr::Map(xs, v, body) => {
            closed(xs, scope) && {
                scope.push(v.clone());
                let ok = int_closed(body, scope);
                scope.pop();
                ok
            }
        }
        ListExpr::Scan(xs, init, a, x, body) => {
            closed(xs, scope) && int_closed(init, scope) && {
                scope.push(a.clone());
                scope.push(x.clone());
                let ok = int_closed(body, scope);
                scope.pop();
                scope.pop();
                ok
            }
        }
        ListExpr::Filter(xs, ..) | ListExpr::Reverse(xs) | ListExpr::Sort(xs) | ListExpr::Take(xs, _) => {
            closed(xs, scope)
        }
        ListExpr::Concat(a, b) => closed(a, scope) && closed(b, scope),
        ListExpr::Copy(xs, _) => closed(xs, scope),
        ListExpr::Cycle(_, elems, _) => elems.iter().all(|e| int_closed(e, scope)),
        ListExpr::Nest(..) => true,
    }
}

/// Rename every bound variable to a fresh name, so a subtree grafted from
/// another program cannot capture or be captured.
fn freshen(e: &mut ListExpr, tag: &str) {
    fn rename_int(e: &mut IntExpr, from: &str, to: &str) {
        match e {
            IntExpr::Var(v) if v == from => *v = to.to_string(),
            IntExpr::Add(a, b) | IntExpr::Sub(a, b) | IntExpr::Mul(a, b) => {
                rename_int(a, from, to);
                rename_int(b, from, to);
            }
            IntExpr::Mod(a, _) => rename_int(a, from, to),
            _ => {}
        }
    }
    match e {
        ListExpr::Map(xs, v, body) => {
            freshen(xs, tag);
            let n = format!("{v}{tag}");
            rename_int(body, v, &n);
            *v = n;
        }
        ListExpr::Scan(xs, _, a, x, body) => {
            freshen(xs, tag);
            let (na, nx) = (format!("{a}{tag}"), format!("{x}{tag}"));
            rename_int(body, a, &na);
            rename_int(body, x, &nx);
            *a = na;
            *x = nx;
        }
        ListExpr::Filter(xs, v, ..) => {
            freshen(xs, tag);
            *v = format!("{v}{tag}");
        }
        ListExpr::Reverse(xs) | ListExpr::Sort(xs) | ListExpr::Take(xs, _) => freshen(xs, tag),
        ListExpr::Concat(a, b) => {
            freshen(a, tag);
            freshen(b, tag);
        }
        ListExpr::Copy(xs, _) => freshen(xs, tag),
        // The bound variable is used only in the rendering, never in `elems`.
        ListExpr::Cycle(v, ..) | ListExpr::Nest(v, ..) => *v = format!("{v}{tag}"),
        ListExpr::Range(_) | ListExpr::Repeat(..) => {}
    }
}

/// Replace one random list subtree with a fresh sample from `policy`.
pub fn mutate(p: &Program, policy: &Policy, rng: &mut Rng) -> Program {
    let mut paths = Vec::new();
    list_paths(&p.body, vec![], &mut paths);
    let path = &paths[rng.below(paths.len())];
    let mut out = p.clone();
    let (mut fresh, _) = policy.sample(rng);
    freshen(&mut fresh.body, &format!("m{}", rng.next_u64() % 10_000));
    *at_mut(&mut out.body, path) = fresh.body;
    out
}

/// Graft a list subtree of `b` into `a` — cross-lineage hybridisation when the
/// two parents come from different agents. Only subtrees that are closed over
/// the top-level scope (`s`) are grafted, so no variable can dangle.
pub fn crossover(a: &Program, b: &Program, rng: &mut Rng) -> Program {
    let mut donors = Vec::new();
    list_paths(&b.body, vec![], &mut donors);
    donors.retain(|path| closed(at(&b.body, path), &mut vec!["s".to_string()]));
    let mut sites = Vec::new();
    list_paths(&a.body, vec![], &mut sites);
    if donors.is_empty() {
        return a.clone();
    }
    let mut graft = at(&b.body, &donors[rng.below(donors.len())]).clone();
    freshen(&mut graft, &format!("c{}", rng.next_u64() % 10_000));
    let mut out = a.clone();
    let site = &sites[rng.below(sites.len())];
    *at_mut(&mut out.body, site) = graft;
    out
}

// ── Reading programs back ────────────────────────────────────────────────

/// Literal and length bounds, inclusive. The parser refuses outside them and
/// the prompt states them. Sampling draws within the same ranges (see
/// [`Sampler`]); the round-trip test holds the two together.
pub mod bounds {
    use super::MAX_LEN;
    pub const LITERAL: (u64, u64) = (0, 255);
    pub const MOD: (u64, u64) = (2, 256);
    pub const RANGE: (u64, u64) = (1, MAX_LEN);
    pub const TAKE: (u64, u64) = (1, MAX_LEN);
    pub const REPEAT: (u64, u64) = (1, MAX_LEN / 4);
    pub const CYCLE_LEN: (u64, u64) = (4, MAX_LEN);
    pub const CYCLE_ELEMS: (u64, u64) = (2, 6);
    pub const COPY: (u64, u64) = (2, 6);
    pub const NEST_COUNT: (u64, u64) = (2, 12);
    pub const NEST_MOD: (u64, u64) = (2, 8);
    pub const FILTER_MOD: (u64, u64) = (2, 7);
}

/// One example of each production, rendered by this module's own renderers, in
/// [`LIST_RULES`] and [`INT_RULES`] order. The arena's prompt is built from
/// these, so what a proposer is told the grammar is cannot drift from what
/// `render` writes, and a test checks every example parses back.
pub fn production_examples() -> (Vec<(&'static str, String)>, Vec<(&'static str, String)>) {
    fn text_of_list(e: &ListExpr) -> String {
        let mut s = String::new();
        e.render(&mut s);
        s
    }
    fn text_of_int(e: &IntExpr) -> String {
        let mut s = String::new();
        e.render(&mut s);
        s
    }
    let range = |n: u64| Box::new(ListExpr::Range(n));
    let var = |v: &str| IntExpr::Var(v.into());
    let lists: [(&str, ListExpr); 12] = [
        ("range", ListExpr::Range(8)),
        ("repeat", ListExpr::Repeat(IntExpr::Lit(3), 4)),
        ("map", ListExpr::Map(range(4), "x".into(), IntExpr::Mul(Box::new(var("x")), Box::new(IntExpr::Lit(3))))),
        (
            "scan",
            ListExpr::Scan(
                range(4),
                var("s"),
                "a".into(),
                "x".into(),
                IntExpr::Add(Box::new(var("a")), Box::new(var("x"))),
            ),
        ),
        ("filter", ListExpr::Filter(range(8), "x".into(), 3, 1)),
        ("reverse", ListExpr::Reverse(range(5))),
        ("sort", ListExpr::Sort(range(5))),
        ("take", ListExpr::Take(range(9), 4)),
        ("concat", ListExpr::Concat(range(3), range(2))),
        ("copy", ListExpr::Copy(range(3), 2)),
        ("cycle", ListExpr::Cycle("v".into(), vec![IntExpr::Lit(1), var("s"), IntExpr::Lit(0)], 6)),
        ("nest", ListExpr::Nest("v".into(), 4, 3)),
    ];
    let ints: [(&str, IntExpr); 6] = [
        ("lit", IntExpr::Lit(7)),
        ("var", var("s")),
        ("add", IntExpr::Add(Box::new(var("s")), Box::new(IntExpr::Lit(3)))),
        ("sub", IntExpr::Sub(Box::new(var("s")), Box::new(IntExpr::Lit(3)))),
        ("mul", IntExpr::Mul(Box::new(var("s")), Box::new(IntExpr::Lit(3)))),
        ("mod", IntExpr::Mod(Box::new(var("s")), 5)),
    ];
    (
        lists.iter().map(|(n, e)| (*n, text_of_list(e))).collect(),
        ints.iter().map(|(n, e)| (*n, text_of_int(e))).collect(),
    )
}

/// The grammar as a proposer is told it: every production as its rendered
/// example, and the bounds. Generated from [`production_examples`] and
/// [`bounds`], never written out by hand.
pub fn describe_grammar() -> String {
    let (lists, ints) = production_examples();
    let range = |(lo, hi): (u64, u64)| format!("{lo}..={hi}");
    let mut s = String::from("A body is one list expression over the seed `s`. Its forms, one example each:\n");
    for (name, text) in &lists {
        s.push_str(&format!("  {name:<8} {text}\n"));
    }
    s.push_str("Integer expressions, inside any of them:\n");
    for (name, text) in &ints {
        s.push_str(&format!("  {name:<8} {text}\n"));
    }
    s.push_str(&format!(
        "Bounds: range and take lengths {}; repeat lengths {}; literals {}; modulus {}; \
         cycle templates have {} elements and length {}; copy counts {}; nest counts {} and moduli {}; \
         filter moduli {}. Lists nest at most {MAX_DEPTH} levels deep. Every variable must be \
         `s` or a closure parameter in scope.\n",
        range(bounds::RANGE),
        range(bounds::REPEAT),
        range(bounds::LITERAL),
        range(bounds::MOD),
        range(bounds::CYCLE_ELEMS),
        range(bounds::CYCLE_LEN),
        range(bounds::COPY),
        range(bounds::NEST_COUNT),
        range(bounds::NEST_MOD),
        range(bounds::FILTER_MOD),
    ));
    s
}

/// How many method calls in a row, and how deeply nested, the parser follows
/// before it refuses. Every grammar tree is far shallower; this bounds the
/// stack, so no input can exhaust it.
const MAX_NEST: usize = 64;

impl Program {
    /// The inverse of [`Program::source`]. Accepts the whole `source()` text, or
    /// just the body expression. Whitespace between tokens, and around the text,
    /// is not significant. Anything outside the grammar is refused with a reason
    /// and a byte offset: an unknown method, a literal or length outside
    /// [`bounds`], a variable not in scope, a modulus or nesting the sampler
    /// never writes (nesting is limited by [`MAX_DEPTH`]).
    ///
    /// It does not judge meaning. A text that parses here can still be refused
    /// by the substrate, for fuel or types. That is the gate's job.
    pub fn parse(src: &str) -> Result<Program, String> {
        let mut p = Parser { src, pos: 0, nest: 0 };
        let scope = ["s".to_string()];
        let body = if src.trim_start().starts_with('@') {
            for tok in ["@role", "(", "candidate", ")", "f", "gen", "(", "s", ":", "usize", ")", "->", "[", "usize", "]", "{"] {
                p.expect(tok)?;
            }
            let body = p.list(&scope)?;
            p.expect("}")?;
            body
        } else {
            p.list(&scope)?
        };
        p.end()?;
        check_list_depth(&body, 0)?;
        Ok(Program { body })
    }
}

/// The depth limit, as [`Sampler`] applies it. A list node at [`MAX_DEPTH`] is
/// a leaf; at deeper levels there are none. Integer operators stop two levels
/// below the list depth limit.
fn check_list_depth(e: &ListExpr, d: usize) -> Result<(), String> {
    let leaf = matches!(e, ListExpr::Range(_) | ListExpr::Repeat(..) | ListExpr::Cycle(..) | ListExpr::Nest(..));
    if d > MAX_DEPTH || (d == MAX_DEPTH && !leaf) {
        return Err(format!("nested deeper than MAX_DEPTH = {MAX_DEPTH}"));
    }
    match e {
        ListExpr::Range(_) | ListExpr::Cycle(..) | ListExpr::Nest(..) => Ok(()),
        ListExpr::Repeat(i, _) => check_int_depth(i, d + 1),
        ListExpr::Map(xs, _, b) => {
            check_list_depth(xs, d + 1)?;
            check_int_depth(b, d + 1)
        }
        ListExpr::Scan(xs, init, _, _, b) => {
            check_list_depth(xs, d + 1)?;
            check_int_depth(init, d + 1)?;
            check_int_depth(b, d + 1)
        }
        ListExpr::Filter(xs, ..) | ListExpr::Reverse(xs) | ListExpr::Sort(xs) | ListExpr::Take(xs, _) => {
            check_list_depth(xs, d + 1)
        }
        ListExpr::Concat(a, b) => {
            check_list_depth(a, d + 1)?;
            check_list_depth(b, d + 1)
        }
        ListExpr::Copy(xs, _) => check_list_depth(xs, d + 1),
    }
}

fn check_int_depth(e: &IntExpr, d: usize) -> Result<(), String> {
    let leaf = matches!(e, IntExpr::Lit(_) | IntExpr::Var(_));
    if d > MAX_DEPTH + 2 || (d == MAX_DEPTH + 2 && !leaf) {
        return Err(format!("integer expression nested deeper than MAX_DEPTH = {MAX_DEPTH}"));
    }
    match e {
        IntExpr::Lit(_) | IntExpr::Var(_) => Ok(()),
        IntExpr::Add(a, b) | IntExpr::Sub(a, b) | IntExpr::Mul(a, b) => {
            check_int_depth(a, d + 1)?;
            check_int_depth(b, d + 1)
        }
        IntExpr::Mod(a, _) => check_int_depth(a, d + 1),
    }
}

fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// A hand-written recursive-descent parser over the surface [`render`] writes.
struct Parser<'a> {
    src: &'a str,
    pos: usize,
    nest: usize,
}

impl<'a> Parser<'a> {
    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn skip_ws(&mut self) {
        let rest = self.rest();
        self.pos += rest.len() - rest.trim_start().len();
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_ws();
        self.rest().chars().next()
    }

    fn starts_name(&mut self) -> bool {
        self.peek().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    }

    /// Consume `tok` if the text says it next. A token that ends in a word
    /// character must not run on into one, so `range` is not `ranges`.
    fn eat(&mut self, tok: &str) -> bool {
        self.skip_ws();
        let rest = self.rest();
        if !rest.starts_with(tok) {
            return false;
        }
        let word_ends = tok.chars().last().is_some_and(word_char);
        if word_ends && rest[tok.len()..].chars().next().is_some_and(word_char) {
            return false;
        }
        self.pos += tok.len();
        true
    }

    fn expect(&mut self, tok: &str) -> Result<(), String> {
        if self.eat(tok) {
            Ok(())
        } else {
            Err(self.error(&format!("expected `{tok}`")))
        }
    }

    fn error(&mut self, what: &str) -> String {
        self.skip_ws();
        let found: String = self.rest().chars().take(16).collect();
        let found = if found.is_empty() { "the end of the text".to_string() } else { format!("`{found}`") };
        format!("{what} at byte {}, found {found}", self.pos)
    }

    fn end(&mut self) -> Result<(), String> {
        self.skip_ws();
        if self.rest().is_empty() {
            Ok(())
        } else {
            Err(self.error("unexpected text after the program"))
        }
    }

    fn ident(&mut self) -> Result<String, String> {
        if !self.starts_name() {
            return Err(self.error("expected a name"));
        }
        let rest = self.rest();
        let n = rest.find(|c: char| !word_char(c)).unwrap_or(rest.len());
        self.pos += n;
        Ok(rest[..n].to_string())
    }

    /// A decimal literal in `lo..=hi`: no sign, and no leading zero.
    fn number(&mut self, (lo, hi): (u64, u64), what: &str) -> Result<u64, String> {
        self.skip_ws();
        let rest = self.rest();
        let n = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
        if n == 0 || (n > 1 && rest.starts_with('0')) {
            return Err(self.error(&format!("expected {what}")));
        }
        let at = self.pos;
        self.pos += n;
        match rest[..n].parse::<u64>() {
            Ok(v) if (lo..=hi).contains(&v) => Ok(v),
            Ok(v) => Err(format!("{what} {v} at byte {at} is outside the grammar's {lo}..={hi}")),
            Err(_) => Err(format!("{what} at byte {at} is too large for the grammar's {lo}..={hi}")),
        }
    }

    fn enter(&mut self) -> Result<(), String> {
        self.nest += 1;
        if self.nest > MAX_NEST {
            Err(format!("nested more than {MAX_NEST} levels deep"))
        } else {
            Ok(())
        }
    }

    fn list(&mut self, scope: &[String]) -> Result<ListExpr, String> {
        self.enter()?;
        let e = self.list_chain(scope)?;
        self.nest -= 1;
        Ok(e)
    }

    fn int(&mut self, scope: &[String]) -> Result<IntExpr, String> {
        self.enter()?;
        let e = self.int_expr(scope)?;
        self.nest -= 1;
        Ok(e)
    }

    /// An atom, then any `.method(…)` calls on it, left to right.
    fn list_chain(&mut self, scope: &[String]) -> Result<ListExpr, String> {
        let mut e = self.list_atom(scope)?;
        let mut calls = 0;
        while self.eat(".") {
            calls += 1;
            if calls > MAX_NEST {
                return Err(format!("more than {MAX_NEST} method calls in a row"));
            }
            e = self.method(e, scope)?;
        }
        Ok(e)
    }

    fn list_atom(&mut self, scope: &[String]) -> Result<ListExpr, String> {
        if self.eat("[") {
            let e = self.int(scope)?;
            self.expect(";")?;
            let n = self.number(bounds::REPEAT, "repeat length")?;
            self.expect("]")?;
            return Ok(ListExpr::Repeat(e, n));
        }
        if !self.starts_name() {
            return Err(self.error("expected a list expression"));
        }
        self.skip_ws();
        let at = self.pos;
        match self.ident()?.as_str() {
            "range" => {
                self.expect("(")?;
                let n = self.number(bounds::RANGE, "range length")?;
                self.expect(")")?;
                Ok(ListExpr::Range(n))
            }
            "flatten" => self.flatten(scope),
            other => Err(format!("unknown list expression `{other}` at byte {at}")),
        }
    }

    /// `flatten(…)` is nested counting, concatenation or copying.
    fn flatten(&mut self, scope: &[String]) -> Result<ListExpr, String> {
        self.expect("(")?;
        if self.eat("range") {
            // flatten(range(n).map(|v| range(v % k + 1)))
            self.expect("(")?;
            let n = self.number(bounds::NEST_COUNT, "nest count")?;
            self.expect(")")?;
            self.expect(".")?;
            self.expect("map")?;
            self.expect("(")?;
            self.expect("|")?;
            let v = self.ident()?;
            self.expect("|")?;
            self.expect("range")?;
            self.expect("(")?;
            if self.ident()? != v {
                return Err(self.error(&format!("the nest counts by `{v}`, which is not the variable used")));
            }
            self.expect("%")?;
            let k = self.number(bounds::NEST_MOD, "nest modulus")?;
            self.expect("+")?;
            self.expect("1")?;
            self.expect(")")?; // range
            self.expect(")")?; // map
            self.expect(")")?; // flatten
            Ok(ListExpr::Nest(v, n, k))
        } else {
            // flatten([a, b]) or flatten([a; n])
            self.expect("[")?;
            let a = self.list(scope)?;
            if self.eat(",") {
                let b = self.list(scope)?;
                self.expect("]")?;
                self.expect(")")?;
                Ok(ListExpr::Concat(Box::new(a), Box::new(b)))
            } else {
                self.expect(";")?;
                let n = self.number(bounds::COPY, "copy count")?;
                self.expect("]")?;
                self.expect(")")?;
                Ok(ListExpr::Copy(Box::new(a), n))
            }
        }
    }

    fn method(&mut self, xs: ListExpr, scope: &[String]) -> Result<ListExpr, String> {
        self.skip_ws();
        let at = self.pos;
        let xs = Box::new(xs);
        Ok(match self.ident()?.as_str() {
            "reverse" => {
                self.expect("(")?;
                self.expect(")")?;
                ListExpr::Reverse(xs)
            }
            "sort" => {
                self.expect("(")?;
                self.expect(")")?;
                ListExpr::Sort(xs)
            }
            "take" => {
                self.expect("(")?;
                let n = self.number(bounds::TAKE, "take count")?;
                self.expect(")")?;
                ListExpr::Take(xs, n)
            }
            "filter" => {
                self.expect("(")?;
                self.expect("|")?;
                let v = self.ident()?;
                self.expect("|")?;
                if self.ident()? != v {
                    return Err(self.error(&format!("the filter tests a variable other than `{v}`")));
                }
                self.expect("%")?;
                let k = self.number(bounds::FILTER_MOD, "filter modulus")?;
                self.expect("==")?;
                let r = self.number((0, k - 1), "filter remainder")?;
                self.expect(")")?;
                ListExpr::Filter(xs, v, k, r)
            }
            "scan" => {
                self.expect("(")?;
                let init = self.int(scope)?;
                self.expect(",")?;
                self.expect("|")?;
                let a = self.ident()?;
                self.expect(",")?;
                let x = self.ident()?;
                self.expect("|")?;
                let inner = [scope, &[a.clone(), x.clone()]].concat();
                let body = self.int(&inner)?;
                self.expect(")")?;
                ListExpr::Scan(xs, init, a, x, body)
            }
            "map" => {
                self.expect("(")?;
                self.expect("|")?;
                let v = self.ident()?;
                self.expect("|")?;
                if self.peek() == Some('[') {
                    self.template(*xs, v, scope)?
                } else {
                    let inner = [scope, std::slice::from_ref(&v)].concat();
                    let body = self.int(&inner)?;
                    self.expect(")")?;
                    ListExpr::Map(xs, v, body)
                }
            }
            other => return Err(format!("unknown method `{other}` at byte {at}")),
        })
    }

    /// `range(len).map(|v| [e, …][v % k])`: a periodic template over `range`.
    fn template(&mut self, xs: ListExpr, v: String, scope: &[String]) -> Result<ListExpr, String> {
        let ListExpr::Range(len) = xs else {
            return Err(self.error("a literal template is written over `range(n)` and nothing else"));
        };
        if !(bounds::CYCLE_LEN.0..=bounds::CYCLE_LEN.1).contains(&len) {
            return Err(format!("template length {len} is outside the grammar's {:?}", bounds::CYCLE_LEN));
        }
        self.expect("[")?;
        let mut elems = Vec::new();
        loop {
            elems.push(self.template_elem(scope)?);
            if !self.eat(",") {
                break;
            }
        }
        self.expect("]")?;
        let k = elems.len() as u64;
        if !(bounds::CYCLE_ELEMS.0..=bounds::CYCLE_ELEMS.1).contains(&k) {
            return Err(format!("template has {k} elements, outside the grammar's {:?}", bounds::CYCLE_ELEMS));
        }
        self.expect("[")?;
        if self.ident()? != v {
            return Err(self.error(&format!("the template indexes by a variable other than `{v}`")));
        }
        self.expect("%")?;
        self.number((k, k), "template index modulus")?;
        self.expect("]")?;
        self.expect(")")?;
        Ok(ListExpr::Cycle(v, elems, len))
    }

    /// A template element is a literal or a variable in scope, never arithmetic.
    fn template_elem(&mut self, scope: &[String]) -> Result<IntExpr, String> {
        match self.peek() {
            Some(c) if c.is_ascii_digit() => Ok(IntExpr::Lit(self.number(bounds::LITERAL, "literal")?)),
            Some(_) if self.starts_name() => self.bound_var(scope).map(IntExpr::Var),
            _ => Err(self.error("expected a literal or a bound variable in a template")),
        }
    }

    fn bound_var(&mut self, scope: &[String]) -> Result<String, String> {
        self.skip_ws();
        let at = self.pos;
        let name = self.ident()?;
        if scope.contains(&name) {
            Ok(name)
        } else {
            Err(format!("`{name}` is not bound here (byte {at})"))
        }
    }

    fn int_expr(&mut self, scope: &[String]) -> Result<IntExpr, String> {
        match self.peek() {
            Some(c) if c.is_ascii_digit() => Ok(IntExpr::Lit(self.number(bounds::LITERAL, "literal")?)),
            Some('(') => {
                // (a % k)
                self.expect("(")?;
                let a = self.int(scope)?;
                self.expect("%")?;
                let k = self.number(bounds::MOD, "modulus")?;
                self.expect(")")?;
                Ok(IntExpr::Mod(Box::new(a), k))
            }
            Some(_) if self.starts_name() => {
                self.skip_ws();
                let at = self.pos;
                let name = self.ident()?;
                let op: Option<fn(Box<IntExpr>, Box<IntExpr>) -> IntExpr> = match name.as_str() {
                    "wrapping_add" => Some(IntExpr::Add),
                    "wrapping_sub" => Some(IntExpr::Sub),
                    "wrapping_mul" => Some(IntExpr::Mul),
                    _ => None,
                };
                match op {
                    Some(build) => {
                        self.expect("(")?;
                        let a = self.int(scope)?;
                        self.expect(",")?;
                        let b = self.int(scope)?;
                        self.expect(")")?;
                        Ok(build(Box::new(a), Box::new(b)))
                    }
                    None if scope.contains(&name) => Ok(IntExpr::Var(name)),
                    None => Err(format!("`{name}` is not bound here (byte {at})")),
                }
            }
            _ => Err(self.error("expected an integer expression")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::substrate::{Outcome, Refusal, Substrate};

    #[test]
    fn rendering_is_the_mage_we_measured() {
        let p = Program {
            body: ListExpr::Scan(
                Box::new(ListExpr::Range(4)),
                IntExpr::Var("s".into()),
                "a".into(),
                "x".into(),
                IntExpr::Mod(
                    Box::new(IntExpr::Add(
                        Box::new(IntExpr::Mul(Box::new(IntExpr::Var("a".into())), Box::new(IntExpr::Lit(3)))),
                        Box::new(IntExpr::Var("x".into())),
                    )),
                    256,
                ),
            ),
        };
        assert!(
            p.source().contains("range(4).scan(s, |a, x| (wrapping_add(wrapping_mul(a, 3), x) % 256))"),
            "{}",
            p.source()
        );
        match Substrate::default().run(&p.source(), 7) {
            Outcome::Bytes { bytes, .. } => assert_eq!(bytes, vec![7, 21, 64, 194, 73]),
            other => panic!("{other:?}"),
        }
    }

    /// The property the whole design leans on: the uniform prior writes
    /// programs the compiler accepts. Parse and signature refusals would mean
    /// the renderer is wrong; type refusals would mean the vocabulary is.
    #[test]
    fn sampled_programs_clear_the_static_gates() {
        let policy = Policy::default();
        let sub = Substrate { fuel: 50_000, max_bytes: 256 };
        let mut rng = Rng(42);
        let mut counts = std::collections::HashMap::new();
        for _ in 0..300 {
            let (p, _) = policy.sample(&mut rng);
            let key = match sub.run(&p.source(), rng.next_u64()) {
                Outcome::Bytes { .. } => None,
                Outcome::Refused(r, why) => {
                    assert!(
                        !matches!(r, Refusal::Parse | Refusal::Signature | Refusal::Type | Refusal::Effect),
                        "static refusal {r:?}: {why}\n{}",
                        p.source()
                    );
                    Some(r)
                }
            };
            *counts.entry(key).or_insert(0usize) += 1;
        }
        let ok = counts.get(&None).copied().unwrap_or(0);
        assert!(ok >= 200, "only {ok}/300 produced bytes: {counts:?}");
    }

    #[test]
    fn mutation_and_crossover_stay_well_formed() {
        let policy = Policy::default();
        let sub = Substrate { fuel: 50_000, max_bytes: 256 };
        let mut rng = Rng(7);
        for _ in 0..100 {
            let (a, _) = policy.sample(&mut rng);
            let (b, _) = policy.sample(&mut rng);
            for child in [mutate(&a, &policy, &mut rng), crossover(&a, &b, &mut rng)] {
                if let Outcome::Refused(r, why) = sub.run(&child.source(), 3) {
                    assert!(
                        !matches!(r, Refusal::Parse | Refusal::Signature | Refusal::Type),
                        "{r:?}: {why}\n{}",
                        child.source()
                    );
                }
            }
        }
    }

    #[test]
    fn the_structural_productions_render_to_checked_mage_with_the_intended_output() {
        let sub = Substrate { fuel: 50_000, max_bytes: 256 };
        let cases = [
            (ListExpr::Copy(Box::new(ListExpr::Range(3)), 3), vec![0, 1, 2, 0, 1, 2, 0, 1, 2]),
            (
                ListExpr::Cycle("v".into(), vec![IntExpr::Lit(9), IntExpr::Var("s".into())], 5),
                vec![9, 4, 9, 4, 9],
            ),
            (ListExpr::Nest("v".into(), 4, 3), vec![0, 0, 1, 0, 1, 2, 0]),
        ];
        for (body, want) in cases {
            let p = Program { body };
            match sub.run(&p.source(), 4) {
                Outcome::Bytes { bytes, .. } => assert_eq!(bytes, want, "{}", p.source()),
                other => panic!("{other:?}\n{}", p.source()),
            }
        }
    }

    #[test]
    fn reinforce_raises_the_probability_of_a_rewarded_trace() {
        let mut policy = Policy::default();
        let trace = [Choice { list: true, rule: 3 }];
        let before = policy.probabilities().0[3];
        policy.reinforce(&trace, 1.0, 0.5);
        assert!(policy.probabilities().0[3] > before);
        policy.reinforce(&trace, -2.0, 0.5);
        assert!(policy.probabilities().0[3] < before);
    }

    #[test]
    fn token_length_uses_the_real_lexer() {
        let short = Program { body: ListExpr::Range(3) };
        let long = Program {
            body: ListExpr::Concat(Box::new(ListExpr::Range(3)), Box::new(ListExpr::Range(4))),
        };
        assert!(long.token_len() > short.token_len());
    }

    #[test]
    fn byte_length_is_the_utf8_length_of_the_source() {
        use crate::arena::LengthUnit;
        let p = Program {
            body: ListExpr::Concat(Box::new(ListExpr::Range(3)), Box::new(ListExpr::Range(4))),
        };
        assert_eq!(p.length(LengthUnit::Bytes), p.source().len());
        assert_eq!(p.length(LengthUnit::Tokens), p.token_len());
    }

    fn assert_round_trips(p: &Program) {
        let mut body = String::new();
        p.body.render(&mut body);
        assert_eq!(Program::parse(&p.source()), Ok(p.clone()), "{}", p.source());
        assert_eq!(Program::parse(&body), Ok(p.clone()), "{body}");
    }

    /// The property the proposer's replies rely on: `parse` inverts `render`.
    /// Sampled programs, and their mutations and crossovers, under a fixed seed.
    /// Mutation and crossover graft subtrees sampled from depth 0, so a graft
    /// can sit deeper than MAX_DEPTH. Those variants are refused, not misread,
    /// and only the ones that fit are required to round-trip.
    #[test]
    fn parse_inverts_render_on_sampled_mutated_and_crossed_programs() {
        let policy = Policy::default();
        let mut rng = Rng(2026);
        let (mut too_deep, mut checked) = (0, 0);
        for _ in 0..2000 {
            let (p, _) = policy.sample(&mut rng);
            assert_round_trips(&p);
            checked += 1;
            let (q, _) = policy.sample(&mut rng);
            for v in [mutate(&p, &policy, &mut rng), crossover(&p, &q, &mut rng)] {
                if check_list_depth(&v.body, 0).is_ok() {
                    assert_round_trips(&v);
                    checked += 1;
                } else {
                    too_deep += 1;
                    assert!(Program::parse(&v.source()).is_err(), "{}", v.source());
                }
            }
        }
        assert!(checked >= 2000, "only {checked} programs checked");
        assert!(too_deep < checked, "every variant was too deep to check");
    }

    #[test]
    fn parse_takes_the_whole_source_and_tolerates_whitespace() {
        let want = Program { body: ListExpr::Copy(Box::new(ListExpr::Range(3)), 2) };
        let whole = "@role(candidate)\nf gen(s: usize) -> [usize] {\n    flatten([range(3); 2])\n}\n";
        assert_eq!(Program::parse(whole), Ok(want.clone()));
        assert_eq!(Program::parse("  \n flatten( [ range( 3 ) ;2 ] )  \n"), Ok(want));
    }

    #[test]
    fn parse_refuses_what_the_grammar_does_not_write() {
        let refused = |src: &str| Program::parse(src).expect_err(src);
        assert!(refused("range(3).flatMap(|x| x)").contains("unknown method `flatMap`"));
        assert!(refused("range(3).map(|x| y)").contains("`y` is not bound"));
        assert!(refused("range(3).map(|x| x).map(|y| x)").contains("`x` is not bound"));
        assert!(refused("range(49)").contains("outside the grammar"));
        assert!(refused("range(0)").contains("outside the grammar"));
        assert!(refused("range(99999999999999999999999999)").contains("too large"));
        assert!(refused("[256; 3]").contains("literal 256"));
        assert!(refused("[1; 13]").contains("repeat length 13"));
        assert!(refused("flatten([range(3); 7])").contains("copy count 7"));
        assert!(refused("range(3).map(|x| (x % 1))").contains("modulus 1"));
        assert!(refused("range(3).map(|x| (x % 257))").contains("modulus 257"));
        assert!(refused("range(8).filter(|x| x % 2 == 2)").contains("filter remainder 2"));
        assert!(refused("range(8).filter(|x| y % 3 == 1)").contains("tests a variable other"));
        assert!(refused("range(6).map(|v| [1, 2][w % 2])").contains("other than `v`"));
        assert!(refused("range(6).map(|v| [1, 2, 3][v % 2])").contains("modulus"));
        assert!(refused("range(3).map(|v| [1, 2][v % 2])").contains("length 3"));
        assert!(refused("range(4).reverse().map(|v| [1, 2][v % 2])").contains("over `range(n)`"));
        assert!(refused("flatten(range(13).map(|v| range(v % 3 + 1)))").contains("nest count 13"));
        for src in ["", "hello", "range(", "range(3) range(4)", "range(3).", "[1; 2", "f other(s: usize) -> [usize] { range(3) }"] {
            assert!(Program::parse(src).is_err(), "accepted {src:?}");
        }
    }

    #[test]
    fn parse_refuses_nesting_past_max_depth() {
        // Five reverses over a range: the range sits at the last level, a leaf.
        let chain = |n: usize| format!("range(3){}", ".reverse()".repeat(n));
        assert!(Program::parse(&chain(MAX_DEPTH)).is_ok());
        let deep = Program::parse(&chain(MAX_DEPTH + 1)).expect_err("six levels");
        assert!(deep.contains("MAX_DEPTH"), "{deep}");
        // Integer nesting: a map's body sits one level down, and operators stop
        // two levels below the list limit, so six nested adds fit and seven do not.
        let adds = |n: usize| (0..n).fold("s".to_string(), |e, _| format!("wrapping_add({e}, 1)"));
        assert!(Program::parse(&format!("range(3).map(|x| {})", adds(6))).is_ok());
        let deep = Program::parse(&format!("range(3).map(|x| {})", adds(7))).expect_err("seven adds");
        assert!(deep.contains("integer expression nested deeper"), "{deep}");
    }

    #[test]
    fn the_prompt_grammar_names_every_production_and_its_examples_parse() {
        let (lists, ints) = production_examples();
        let names: Vec<&str> = lists.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, LIST_RULES.to_vec());
        let names: Vec<&str> = ints.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, INT_RULES.to_vec());
        for (_, text) in &lists {
            assert_eq!(Program::parse(text).map(|p| { let mut s = String::new(); p.body.render(&mut s); s }), Ok(text.clone()));
        }
        for (_, text) in &ints {
            let wrapped = format!("range(3).map(|x| {text})");
            assert!(Program::parse(&wrapped).is_ok(), "{wrapped}");
        }
        let described = describe_grammar();
        for name in LIST_RULES.iter().chain(INT_RULES.iter()) {
            assert!(described.contains(name), "{name} missing from the description");
        }
    }
}
