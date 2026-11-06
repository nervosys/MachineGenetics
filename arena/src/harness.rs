//! The harness in MAGE: policies the loop is judged by, as MAGE programs.
//!
//! Plan task 5.1. The redesign's L1 is a small trusted kernel with everything
//! else in MAGE, because the loop can vary a MAGE program and cannot vary the
//! Rust it is compiled into. The compression reward is the first policy moved:
//! [`RewardProgram`] loads `harness/reward.mg` (or a replacement), checks it
//! through every gate a MAGE program passes plus the `evaluator` role, and
//! runs it under fuel for every program the agents write.

use mage_prototype::eval::{run_bounded, BoundedError, Value};
use mage_prototype::{ast, effects, hir, lexer, parser, types};

/// The reward shipped with the arena.
pub const DEFAULT_REWARD: &str = include_str!("../harness/reward.mg");

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
        let module = parser::parse(&lexer::lex(source)).map_err(|e| format!("reward: parse: {}", e.message))?;
        let item = module
            .items
            .iter()
            .find(|i| matches!(&i.kind, ast::ItemKind::Function(f) if f.name == "reward"))
            .ok_or("reward: no function `reward`")?;
        let ast::ItemKind::Function(f) = &item.kind else { unreachable!() };
        if f.params.len() != 7 {
            return Err(format!("reward: `reward` takes 7 parameters, not {}", f.params.len()));
        }
        match item.attributes.iter().find(|a| a.name == "role").and_then(|a| a.args.first()) {
            Some(r) if r == "evaluator" => {}
            other => return Err(format!("reward: `reward` must have @role(evaluator), found {other:?}")),
        }
        let errors = |ds: &[hir::Diagnostic]| {
            ds.iter().find(|d| d.severity == hir::Severity::Error).map(|d| d.message.clone())
        };
        if let Some(e) = errors(&mage_prototype::core_subset::check(&module)) {
            return Err(format!("reward: not MAGE-core: {e}"));
        }
        if let Some(e) = errors(&types::check(&module).diagnostics) {
            return Err(format!("reward: type: {e}"));
        }
        if let Some(e) = errors(&effects::infer_effects(&module).diagnostics) {
            return Err(format!("reward: effect: {e}"));
        }
        Ok(RewardProgram { module, fuel: 10_000 })
    }

    /// The reward for `x`, or why the program could not produce one.
    pub fn eval(&self, x: &RewardInputs) -> Result<f64, String> {
        let args = [x.before, x.after, x.len, x.seen, x.tokens, x.charge, x.structure]
            .into_iter()
            .map(Value::Float)
            .collect();
        match run_bounded(&self.module, "reward", args, self.fuel) {
            Ok(Value::Float(r)) => Ok(r),
            Ok(Value::Int(n)) => Ok(n as f64),
            Ok(other) => Err(format!("reward returned `{other}`, not a number")),
            Err(BoundedError::FuelExhausted) => Err("reward exhausted its fuel".into()),
            Err(BoundedError::Error(e)) => Err(format!("reward: {e}")),
        }
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
    fn a_replacement_reward_changes_the_credit() {
        // The point of moving it: a different MAGE program is a different
        // policy, with no Rust change.
        let flat = "@role(evaluator)\nf reward(a: f64, b: f64, c: f64, d: f64, e: f64, g: f64, h: f64) -> f64 {\n    1.0\n}\n";
        let p = RewardProgram::load(flat).unwrap();
        let x = RewardInputs { before: 8.0, after: 2.0, len: 10.0, seen: 0.0, tokens: 1.0, charge: 0.0, structure: 1.0 };
        assert_eq!(p.eval(&x).unwrap(), 1.0);
    }
}
