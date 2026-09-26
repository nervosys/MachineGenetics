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
}
