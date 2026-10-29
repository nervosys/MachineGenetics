//! `arena physics-task`: the physics program task (plan C3) as an arena task kind.
//!
//! The harness is `prototype/examples/physics_task/task.mg.in`. It owns the
//! energy, the `@ens` on `advance` that every step passes through, the rollout,
//! the held-out reference and `score`. A candidate is one
//! `@role(candidate) f step(s: [f64], k: f64, dt: f64) -> [f64]`, and this
//! module judges it the way `prototype/tests/physics_task.rs` does:
//!
//! 1. **admit** it: it parses, it is exactly one function, `step`, roled
//!    `candidate`, and it passes `--check --core` on its own;
//! 2. **compose** it with the harness, and check the composition with `--check`;
//! 3. **run** `score()` under fuel ([`mage_prototype::eval::run_metered`]).
//!
//! Every candidate ends in one of two verdicts. It is *scored* (the RMS position
//! error over the held-out cases, lower is better), or it is *refused* with a
//! reason class. A contract violation is a [`Refusal::Contract`]; running out of
//! fuel is a [`Refusal::Fuel`]. The two are kept apart on purpose: fuel says the
//! candidate is too expensive, a contract says it is wrong.
//!
//! The splice is not shared with `prototype/tests/physics_task.rs`, which keeps
//! its own copy: `mage-prototype` cannot depend on the arena, and the arena
//! depends on it. The splice is one line, `task` then `candidate`, and the
//! tests here assert the same five verdicts that file does.

use mage_prototype::eval::{run_metered, BoundedError, Value};
use mage_prototype::{abl_shape, ast, core_subset, differentiable, effects, elision, hir, lexer, parser, resolve, types, verify};
use std::cmp::Ordering;

/// The harness every candidate is spliced into, read at build time.
pub const TEMPLATE: &str = include_str!("../../prototype/examples/physics_task/task.mg.in");

/// The five candidates the repository ships, as `(file name, source)`. They are
/// the default population; file arguments replace them.
pub const CANDIDATES: [(&str, &str); 5] = [
    ("velocity_verlet.mg", include_str!("../../prototype/examples/physics_task/velocity_verlet.mg")),
    ("symplectic_euler.mg", include_str!("../../prototype/examples/physics_task/symplectic_euler.mg")),
    ("frozen.mg", include_str!("../../prototype/examples/physics_task/frozen.mg")),
    ("explicit_euler.mg", include_str!("../../prototype/examples/physics_task/explicit_euler.mg")),
    ("verlet_missing_half.mg", include_str!("../../prototype/examples/physics_task/verlet_missing_half.mg")),
];

/// Fuel for one scoring run: the same figure `prototype/tests/physics_task.rs`
/// and `mage-parse --eval --fuel` use. A correct integrator spends about
/// 18,000 to 23,000 of it.
pub const FUEL: u64 = 1_000_000;

/// The reason class of a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The candidate, or the composed program, does not lex or parse.
    Parse,
    /// The candidate is not exactly one `@role(candidate) f step`.
    Signature,
    /// The static gate (`--check`) rejects it: a type, name, effect, shape or
    /// core-subset error.
    Type,
    /// The harness's `@ens` (or `@req`) is violated at run time.
    Contract,
    /// The fuel ran out before `score` returned. The candidate may be correct
    /// and merely slow, so this is not a verdict on its maths.
    Fuel,
    /// Any other run-time error, or a `score` that did not return a float.
    Runtime,
}

impl Refusal {
    pub fn label(self) -> &'static str {
        match self {
            Refusal::Parse => "parse",
            Refusal::Signature => "signature",
            Refusal::Type => "type",
            Refusal::Contract => "contract",
            Refusal::Fuel => "fuel",
            Refusal::Runtime => "runtime",
        }
    }
}

/// What the arena says about one candidate.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// The RMS position error over the held-out cases; lower is better.
    Scored { score: f64, fuel_spent: u64 },
    /// Refused, with its reason class and the gate's or evaluator's message.
    Refused { reason: Refusal, message: String, fuel_spent: u64 },
}

/// One candidate's line in the ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub verdict: Verdict,
}

/// The message prefix the evaluator gives a violated contract (`eval.rs`).
const CONTRACT_PREFIX: &str = "contract violated:";

fn refuse(reason: Refusal, message: String) -> Verdict {
    Verdict::Refused { reason, message, fuel_spent: 0 }
}

/// Lex and parse, as `mage-parse --check` does: a lexer error is a parse error.
fn parse_source(src: &str) -> Result<ast::Module, String> {
    let tokens = lexer::lex(src);
    if let Some(t) = tokens.iter().find(|t| t.kind == lexer::TokenKind::Error) {
        return Err(format!("{}:{}: lexer error: unexpected character", t.span.line, t.span.col));
    }
    parser::parse(&tokens).map_err(|e| format!("{}:{}: parse error: {}", e.line, e.col, e.message))
}

fn errors(diags: &[hir::Diagnostic]) -> Vec<String> {
    diags.iter().filter(|d| d.severity == hir::Severity::Error).map(|d| d.to_string()).collect()
}

/// Every error `mage-parse --check` (with `--core`, when `core`) would count,
/// in the order it counts them. The printing and the self-healing are left out.
fn static_errors(module: &ast::Module, core: bool) -> Vec<String> {
    let module = elision::elide(module);
    let mut out = errors(&resolve::resolve(&module).diagnostics);
    out.extend(errors(&types::check(&module).diagnostics));
    let eff = effects::infer_effects(&module);
    out.extend(errors(&eff.diagnostics));
    let diff = differentiable::infer(&module, &eff);
    out.extend(errors(&differentiable::check_grad_obligations(&module, &diff)));
    out.extend(verify::diagnostics(&verify::verify_module(&module)).iter().map(|d| d.to_string()));
    out.extend(abl_shape::check_module_shapes(&module).into_iter().map(|d| d.message));
    if core {
        out.extend(errors(&core_subset::check(&module)));
    }
    out
}

/// Admit `candidate`, splice it into `task`, and run `score` under `fuel`.
pub fn judge(task: &str, candidate: &str, fuel: u64) -> Verdict {
    // Admission: exactly one item, `step`, roled `candidate`. A candidate that
    // brought its own `advance` or `sp` could replace the task's contract.
    let module = match parse_source(candidate) {
        Ok(m) => m,
        Err(e) => return refuse(Refusal::Parse, e),
    };
    let is_step = module.items.len() == 1
        && matches!(&module.items[0].kind, ast::ItemKind::Function(fd) if fd.name == "step");
    if !is_step {
        return refuse(Refusal::Signature, "a candidate is exactly one function, `step`".into());
    }
    if !module.items[0].attributes.iter().any(|a| a.name == "role" && a.args == ["candidate"]) {
        return refuse(Refusal::Signature, "`step` must be @role(candidate)".into());
    }
    if let Some(e) = static_errors(&module, true).into_iter().next() {
        return refuse(Refusal::Type, format!("--check --core: {e}"));
    }

    // Composition: the harness, a newline, the candidate. The task's line
    // endings are normalised so a checkout that converts them still composes.
    let program = format!("{}\n{}", task.replace("\r\n", "\n"), candidate);
    let module = match parse_source(&program) {
        Ok(m) => m,
        Err(e) => return refuse(Refusal::Parse, format!("composed: {e}")),
    };
    if let Some(e) = static_errors(&module, false).into_iter().next() {
        return refuse(Refusal::Type, format!("composed --check: {e}"));
    }

    let (result, fuel_spent) = run_metered(&module, "score", vec![], fuel);
    match result {
        Ok(Value::Float(score)) => Verdict::Scored { score, fuel_spent },
        Ok(other) => Verdict::Refused {
            reason: Refusal::Runtime,
            message: format!("`score` returned `{other}`, not a float"),
            fuel_spent,
        },
        Err(BoundedError::FuelExhausted) => Verdict::Refused {
            reason: Refusal::Fuel,
            message: "fuel exhausted".into(),
            fuel_spent,
        },
        Err(BoundedError::Error(e)) if e.starts_with(CONTRACT_PREFIX) => {
            Verdict::Refused { reason: Refusal::Contract, message: e, fuel_spent }
        }
        Err(BoundedError::Error(e)) => Verdict::Refused { reason: Refusal::Runtime, message: e, fuel_spent },
    }
}

/// Judge every candidate, then rank: scored ones by score, lowest first, then
/// the refused ones in the order they were given.
pub fn rank(task: &str, candidates: &[(String, String)], fuel: u64) -> Vec<Entry> {
    let mut entries: Vec<Entry> = candidates
        .iter()
        .map(|(name, src)| Entry { name: name.clone(), verdict: judge(task, src, fuel) })
        .collect();
    entries.sort_by(|a, b| match (&a.verdict, &b.verdict) {
        (Verdict::Scored { score: x, .. }, Verdict::Scored { score: y, .. }) => x.total_cmp(y),
        (Verdict::Scored { .. }, Verdict::Refused { .. }) => Ordering::Less,
        (Verdict::Refused { .. }, Verdict::Scored { .. }) => Ordering::Greater,
        (Verdict::Refused { .. }, Verdict::Refused { .. }) => Ordering::Equal,
    });
    entries
}

/// The ranking as a table: one line per candidate, scored lines numbered.
pub fn report(entries: &[Entry], fuel: u64) -> String {
    let mut out = format!(
        "physics task (plan C3): spring, two held-out cases of 100 steps, fuel {fuel} per candidate\n"
    );
    out.push_str(&format!("{:<5} {:<24} {}\n", "rank", "candidate", "verdict"));
    let mut rank = 0;
    for e in entries {
        match &e.verdict {
            Verdict::Scored { score, fuel_spent } => {
                rank += 1;
                out.push_str(&format!(
                    "{:<5} {:<24} scored {score:.6}  (fuel {fuel_spent})\n",
                    rank, e.name
                ));
            }
            Verdict::Refused { reason, message, fuel_spent } => {
                out.push_str(&format!(
                    "{:<5} {:<24} refused [{}] {message}  (fuel {fuel_spent})\n",
                    "-",
                    e.name,
                    reason.label()
                ));
            }
        }
    }
    out
}

/// `arena physics-task [--fuel N] [FILE…]`: rank the given candidate files, or
/// the five shipped ones, against the physics task. A refusal is an outcome,
/// not an error; the command fails only when it cannot read its inputs.
pub fn run_cli(args: &[String]) -> Result<(), String> {
    let mut fuel = FUEL;
    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fuel" => {
                fuel = args
                    .get(i + 1)
                    .and_then(|v| v.parse().ok())
                    .ok_or("physics-task: --fuel needs a whole number")?;
                i += 1;
            }
            other if other.starts_with("--") => {
                return Err(format!("physics-task: unknown argument `{other}`"));
            }
            other => files.push(other.to_string()),
        }
        i += 1;
    }
    let candidates: Vec<(String, String)> = if files.is_empty() {
        CANDIDATES.iter().map(|(n, s)| (n.to_string(), s.to_string())).collect()
    } else {
        files
            .iter()
            .map(|f| {
                std::fs::read_to_string(f)
                    .map(|s| (f.clone(), s))
                    .map_err(|e| format!("physics-task: read {f}: {e}"))
            })
            .collect::<Result<_, _>>()?
    };
    let entries = rank(&TEMPLATE.replace("\r\n", "\n"), &candidates, fuel);
    print!("{}", report(&entries, fuel));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> String {
        TEMPLATE.replace("\r\n", "\n")
    }

    fn candidate(name: &str) -> &'static str {
        CANDIDATES.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("{name}")).1
    }

    fn score_of(v: &Verdict) -> f64 {
        match v {
            Verdict::Scored { score, .. } => *score,
            Verdict::Refused { message, .. } => panic!("refused: {message}"),
        }
    }

    fn refusal_of(v: &Verdict) -> (Refusal, &str) {
        match v {
            Verdict::Refused { reason, message, .. } => (*reason, message.as_str()),
            Verdict::Scored { score, .. } => panic!("accepted with score {score}"),
        }
    }

    #[test]
    fn the_population_ranks_and_refuses_as_the_prototype_test_says() {
        let candidates: Vec<(String, String)> =
            CANDIDATES.iter().map(|(n, s)| (n.to_string(), s.to_string())).collect();
        let ranked = rank(&task(), &candidates, FUEL);

        let order: Vec<&str> = ranked.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            order,
            ["velocity_verlet.mg", "symplectic_euler.mg", "frozen.mg", "explicit_euler.mg", "verlet_missing_half.mg"]
        );
        let verlet = score_of(&ranked[0].verdict);
        let euler = score_of(&ranked[1].verdict);
        let frozen = score_of(&ranked[2].verdict);
        // The same thresholds as `prototype/tests/physics_task.rs`.
        assert!(verlet < 0.002, "Verlet RMS error {verlet}");
        assert!(euler < 0.03, "symplectic Euler RMS error {euler}");
        assert!(verlet * 10.0 < euler, "Verlet {verlet} vs symplectic Euler {euler}");
        assert!(frozen > 0.5, "frozen RMS error {frozen}");

        for e in &ranked[3..] {
            let (reason, message) = refusal_of(&e.verdict);
            assert_eq!(reason, Refusal::Contract, "{}: {message}", e.name);
            assert!(message.contains(CLAUSE), "{}: {message}", e.name);
        }
    }

    /// The clause a contract refusal quotes, as `prototype/tests/physics_task.rs` has it.
    const CLAUSE: &str = "`@ens(abs(energy(result, k) - e0) <= DRIFT * e0, \
                          \"energy stays within DRIFT of its initial value\")` of `advance` (sp advance)";

    #[test]
    fn explicit_euler_is_refused_at_the_step_that_crosses_the_bound() {
        let v = judge(&task(), candidate("explicit_euler.mg"), FUEL);
        let (reason, message) = refusal_of(&v);
        assert_eq!(reason, Refusal::Contract);
        assert_eq!(
            message,
            format!(
                "contract violated: {CLAUSE} is false: `advance` returned [0.5707904498999999, -1.7650160200000002]"
            )
        );
    }

    #[test]
    fn a_tiny_fuel_is_a_fuel_refusal_not_a_contract_one() {
        // Explicit Euler is the candidate that would violate the contract, and
        // the contract is a verdict on its maths. Under 100 units it runs out
        // of fuel before any contract is checked, so the class must be fuel.
        for name in ["velocity_verlet.mg", "explicit_euler.mg"] {
            let v = judge(&task(), candidate(name), 100);
            let (reason, message) = refusal_of(&v);
            assert_eq!(reason, Refusal::Fuel, "{name}: {message}");
            match v {
                Verdict::Refused { fuel_spent, .. } => assert_eq!(fuel_spent, 100, "{name} spends all of it"),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn a_candidate_that_is_not_one_roled_step_is_refused_at_admission() {
        let unroled = "f step(s: [f64], k: f64, dt: f64) -> [f64] { s }";
        let (reason, _) = refusal_of(&judge(&task(), unroled, FUEL));
        assert_eq!(reason, Refusal::Signature);
        let (reason, _) = refusal_of(&judge(&task(), "@role(candidate)\nf step(s: [f64], k: f64, dt: f64) -> [f64] {", FUEL));
        assert_eq!(reason, Refusal::Parse);
    }
}
