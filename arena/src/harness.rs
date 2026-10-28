//! The harness in MAGE: policies the loop is judged by, as MAGE programs.
//!
//! Plan task 5.1. The redesign's L1 is a small trusted kernel with everything
//! else in MAGE, because the loop can vary a MAGE program and cannot vary the
//! Rust it is compiled into. The compression reward is the first policy moved:
//! [`RewardProgram`] loads `harness/reward.mg` (or a replacement), checks it
//! through every gate a MAGE program passes plus the `evaluator` role, and
//! runs it under fuel for every program the agents write. The budget
//! allocation is the second (plan 5.2): [`AllocationProgram`] loads
//! `harness/allocation.mg` under the `candidate` role, which allows no
//! effects, and the kernel rounds its weights to shares.

use mage_prototype::eval::{run_bounded, BoundedError, Value};
use mage_prototype::{ast, effects, hir, lexer, parser, types};

/// The reward shipped with the arena.
pub const DEFAULT_REWARD: &str = include_str!("../harness/reward.mg");

/// The allocation policy shipped with the arena (plan 5.2).
pub const DEFAULT_ALLOCATION: &str = include_str!("../harness/allocation.mg");

/// Parse and gate a harness policy: `source` defines a function `name` of
/// `arity` parameters with `@role(role)`, which is MAGE-core, typechecks and
/// raises no effect error. `what` prefixes every error.
fn load_policy(what: &str, source: &str, name: &str, arity: usize, role: &str) -> Result<ast::Module, String> {
    let module = parser::parse(&lexer::lex(source)).map_err(|e| format!("{what}: parse: {}", e.message))?;
    let item = module
        .items
        .iter()
        .find(|i| matches!(&i.kind, ast::ItemKind::Function(f) if f.name == name))
        .ok_or(format!("{what}: no function `{name}`"))?;
    let ast::ItemKind::Function(f) = &item.kind else { unreachable!() };
    if f.params.len() != arity {
        return Err(format!("{what}: `{name}` takes {arity} parameters, not {}", f.params.len()));
    }
    match item.attributes.iter().find(|a| a.name == "role").and_then(|a| a.args.first()) {
        Some(r) if r == role => {}
        other => return Err(format!("{what}: `{name}` must have @role({role}), found {other:?}")),
    }
    let errors = |ds: &[hir::Diagnostic]| {
        ds.iter().find(|d| d.severity == hir::Severity::Error).map(|d| d.message.clone())
    };
    if let Some(e) = errors(&mage_prototype::core_subset::check(&module)) {
        return Err(format!("{what}: not MAGE-core: {e}"));
    }
    if let Some(e) = errors(&types::check(&module).diagnostics) {
        return Err(format!("{what}: type: {e}"));
    }
    if let Some(e) = errors(&effects::infer_effects(&module).diagnostics) {
        return Err(format!("{what}: effect: {e}"));
    }
    Ok(module)
}

/// Run `name` on `args` under `fuel`, as a number.
fn run_number(what: &str, module: &ast::Module, name: &str, args: Vec<Value>, fuel: u64) -> Result<f64, String> {
    match run_bounded(module, name, args, fuel) {
        Ok(Value::Float(r)) => Ok(r),
        Ok(Value::Int(n)) => Ok(n as f64),
        Ok(other) => Err(format!("{what} returned `{other}`, not a number")),
        Err(BoundedError::FuelExhausted) => Err(format!("{what} exhausted its fuel")),
        Err(BoundedError::Error(e)) => Err(format!("{what}: {e}")),
    }
}

/// A checked, loaded allocation policy (plan 5.2).
pub struct AllocationProgram {
    module: ast::Module,
    pub fuel: u64,
}

impl AllocationProgram {
    /// Parse and gate `source`: one function `weight(score, best,
    /// temperature)` with `@role(candidate)`.
    pub fn load(source: &str) -> Result<AllocationProgram, String> {
        Ok(AllocationProgram { module: load_policy("allocation", source, "weight", 3, "candidate")?, fuel: 10_000 })
    }

    /// One agent's weight. The kernel refuses one that is negative or not
    /// finite, since rounding could not honour it.
    pub fn weight(&self, score: f64, best: f64, temperature: f64) -> Result<f64, String> {
        let args = vec![Value::Float(score), Value::Float(best), Value::Float(temperature)];
        let w = run_number("allocation", &self.module, "weight", args, self.fuel)?;
        if w.is_finite() && w >= 0.0 {
            Ok(w)
        } else {
            Err(format!("allocation: weight {w} for score {score} is negative or not finite"))
        }
    }

    /// Divide `total` evaluations among agents by `score`: the program's
    /// weights, then the kernel's floor and rounding
    /// ([`crate::agents::allocate_weights`]). A score that is not finite is
    /// passed as 0, and `best` is the highest score, or 0.
    pub fn allocate(&self, total: usize, score: &[f64], floor: usize, temperature: f64) -> Result<Vec<usize>, String> {
        let best = score.iter().cloned().fold(0.0f64, f64::max);
        let weights = score
            .iter()
            .map(|&s| self.weight(if s.is_finite() { s } else { 0.0 }, best, temperature))
            .collect::<Result<Vec<f64>, String>>()?;
        Ok(crate::agents::allocate_weights(total, &weights, floor))
    }
}

/// What the reward is computed from.
#[derive(Debug, Clone, Copy)]
pub struct RewardInputs {
    /// Probe bits/byte before and after the round's training.
    pub before: f64,
    pub after: f64,
    /// Probe length in bytes.
    pub len: f64,
    /// Times this shape was seen before.
    pub seen: f64,
    /// Program length in MAGE tokens.
    pub tokens: f64,
    /// Description-length charge per token.
    pub charge: f64,
    /// The structure factor (1 unless the entropy floor is on).
    pub structure: f64,
}

/// A checked, loaded reward program.
pub struct RewardProgram {
    module: ast::Module,
    pub fuel: u64,
}

impl RewardProgram {
    /// Parse and gate `source`: one function `reward` of seven `f64`s, which
    /// typechecks, has role `evaluator`, and raises no effect error.
    pub fn load(source: &str) -> Result<RewardProgram, String> {
        Ok(RewardProgram { module: load_policy("reward", source, "reward", 7, "evaluator")?, fuel: 10_000 })
    }

    /// The reward for `x`, or why the program could not produce one.
    pub fn eval(&self, x: &RewardInputs) -> Result<f64, String> {
        let args = [x.before, x.after, x.len, x.seen, x.tokens, x.charge, x.structure]
            .into_iter()
            .map(Value::Float)
            .collect();
        run_number("reward", &self.module, "reward", args, self.fuel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The formula the Rust arena computed before the reward moved, written in
    /// the same operation order as `reward.mg`.
    fn rust_reference(x: &RewardInputs) -> f64 {
        (x.before - x.after).max(0.0) * x.len / (1.0 + x.seen) * (0.0 - x.charge * x.tokens).exp() * x.structure
    }

    #[test]
    fn the_mage_reward_computes_what_the_rust_reward_did() {
        let p = RewardProgram::load(DEFAULT_REWARD).expect("the shipped reward loads");
        let mut rng = crate::grammar::Rng(17);
        for _ in 0..200 {
            let x = RewardInputs {
                before: 4.0 + 4.0 * rng.next_f64(),
                after: 3.0 + 5.0 * rng.next_f64(),
                len: (1 + rng.below(1024)) as f64,
                seen: rng.below(20) as f64,
                tokens: (5 + rng.below(300)) as f64,
                charge: 0.003,
                structure: rng.next_f64(),
            };
            let (m, r) = (p.eval(&x).expect("evaluates"), rust_reference(&x));
            assert!((m - r).abs() <= 1e-12 * r.abs().max(1.0), "{m} vs {r} for {x:?}");
        }
    }

    #[test]
    fn a_reward_that_reaches_outside_is_refused_by_the_language() {
        let src = "@role(evaluator)\nf reward(a: f64, b: f64, c: f64, d: f64, e: f64, g: f64, h: f64) -> f64 / io {\n    println(\"hi\")\n    a\n}\n";
        let e = RewardProgram::load(src).err().expect("refused");
        assert!(e.contains("effect"), "{e}");
    }

    #[test]
    fn a_reward_without_the_evaluator_role_is_refused() {
        let src = "f reward(a: f64, b: f64, c: f64, d: f64, e: f64, g: f64, h: f64) -> f64 {\n    a\n}\n";
        assert!(RewardProgram::load(src).is_err());
    }

    #[test]
    fn the_mage_allocation_gives_the_shares_the_rust_allocation_did() {
        let p = AllocationProgram::load(DEFAULT_ALLOCATION).expect("the shipped allocation loads");
        let mut rng = crate::grammar::Rng(23);
        for case in 0..400 {
            let n = 1 + rng.below(6);
            let score: Vec<f64> = (0..n)
                .map(|_| match rng.below(8) {
                    0 => 0.0,
                    1 => f64::NAN,
                    2 => f64::INFINITY,
                    3 => 1.0, // ties
                    _ => 10.0 * rng.next_f64(),
                })
                .collect();
            let total = rng.below(120);
            let floor = rng.below(5);
            let temperature = [0.0, 0.05, 0.5, 1.0, 3.0][rng.below(5)];
            assert_eq!(
                p.allocate(total, &score, floor, temperature).expect("allocates"),
                crate::agents::allocate(total, &score, floor, temperature),
                "case {case}: {score:?}, total {total}, floor {floor}, t {temperature}"
            );
        }
    }

    #[test]
    fn a_replacement_allocation_changes_the_shares_and_the_kernel_keeps_the_sum() {
        let flat = "@role(candidate)\nf weight(s: f64, b: f64, t: f64) -> f64 {\n    1.0\n}\n";
        let p = AllocationProgram::load(flat).unwrap();
        assert_eq!(p.allocate(12, &[9.0, 0.0, 3.0], 0, 0.5).unwrap(), vec![4, 4, 4]);
        // A policy that gives everything to the best agent still leaves every
        // agent its floor.
        let greedy = "@role(candidate)\nf weight(s: f64, b: f64, t: f64) -> f64 {\n    if s == b { 1.0 } else { 0.0 }\n}\n";
        let p = AllocationProgram::load(greedy).unwrap();
        assert_eq!(p.allocate(12, &[9.0, 0.0, 3.0], 2, 0.5).unwrap(), vec![8, 2, 2]);
    }

    #[test]
    fn the_kernel_refuses_a_negative_weight_and_the_language_refuses_a_role() {
        let negative = "@role(candidate)\nf weight(s: f64, b: f64, t: f64) -> f64 {\n    0.0 - 1.0\n}\n";
        let e = AllocationProgram::load(negative).unwrap().allocate(10, &[1.0], 0, 1.0).unwrap_err();
        assert!(e.contains("negative"), "{e}");
        // An allocation that may read held-out data is refused at load.
        let evaluator = "@role(evaluator)\nf weight(s: f64, b: f64, t: f64) -> f64 {\n    1.0\n}\n";
        assert!(AllocationProgram::load(evaluator).err().expect("refused").contains("@role(candidate)"));
    }

    #[test]
    fn a_replacement_reward_changes_the_credit() {
        // The point of moving it: a different MAGE program is a different
        // policy, with no Rust change.
        let flat = "@role(evaluator)\nf reward(a: f64, b: f64, c: f64, d: f64, e: f64, g: f64, h: f64) -> f64 {\n    1.0\n}\n";
        let p = RewardProgram::load(flat).unwrap();
        let x = RewardInputs { before: 8.0, after: 2.0, len: 10.0, seen: 0.0, tokens: 1.0, charge: 0.0, structure: 1.0 };
        assert_eq!(p.eval(&x).unwrap(), 1.0);
    }
}
