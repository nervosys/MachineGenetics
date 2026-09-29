//! The substrate: a MAGE program in, a byte sequence out — or a reason why not.
//!
//! Self-play pretraining (Cowsik et al., arXiv:2609.30063) needs a program
//! space in which every program terminates with bounded output, and it names
//! the expressiveness of its substrate — a Brainfuck-like machine — as the
//! limit on scaling. MAGE is the substrate here. A candidate must clear the
//! same gates any MAGE program does, in order, and the first it fails is the
//! reason it is refused:
//!
//! 1. it **parses**, and declares exactly `@role(candidate) f gen(s: usize) -> [usize]`;
//! 2. it **typechecks** with no errors;
//! 3. its effects are **pure** — inferred, not declared, so a program cannot
//!    reach a console, a file or the network by leaving an annotation off.
//!    The `candidate` role makes this the language's rule (§11.6, `E0551`);
//!    the explicit check below stays as a second, independent reading;
//! 4. it **runs within fuel** ([`mage_prototype::eval::run_bounded`]);
//! 5. it returns a **non-empty list of integers**, which become bytes mod 256.
//!
//! Every refusal is ordinary data. Most generated programs are expected to fail
//! somewhere, and the gate that catches them is the cheapest place in the whole
//! system to die — the sandbox side of `ARCHITECTURE.md`'s two regimes, where
//! failure is free and throughput is what matters.

use mage_prototype::eval::{run_metered, BoundedError, Value};
use mage_prototype::{ast, canon, effects, hir, lexer, parser, types};
use serde::{Deserialize, Serialize};

/// Why a program produced no data.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    Parse,
    Signature,
    Type,
    Effect,
    Fuel,
    Runtime,
    Empty,
    /// Not run: the kernel's energy budget or the agent's allowance was
    /// spent (plan 3.5).
    Budget,
}

impl Refusal {
    pub const ALL: [Refusal; 8] = [
        Refusal::Parse,
        Refusal::Signature,
        Refusal::Type,
        Refusal::Effect,
        Refusal::Fuel,
        Refusal::Runtime,
        Refusal::Empty,
        Refusal::Budget,
    ];
}

/// What running a program produced.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Bytes { bytes: Vec<u8>, fuel_used: u64 },
    Refused(Refusal, String),
}

/// The substrate's limits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Substrate {
    /// Evaluation budget per run, in [`mage_prototype::eval`] fuel units.
    pub fuel: u64,
    /// Output is truncated to this many bytes.
    pub max_bytes: usize,
}

impl Default for Substrate {
    fn default() -> Self {
        Substrate { fuel: 200_000, max_bytes: 1024 }
    }
}

/// The entry point every generated program declares.
pub const ENTRY: &str = "gen";

/// A program that cleared every static gate, with its content addresses.
#[derive(Debug, Clone)]
pub struct Prepared {
    module: ast::Module,
    /// `canon::definition_hash` — the same program, whatever its names.
    pub exact: String,
    /// `canon::shape_hash` — the same program, whatever its constants.
    pub shape: String,
}

impl Substrate {
    /// Check `source` and run `gen(seed)`: [`Substrate::prepare`] then
    /// [`Substrate::execute`].
    pub fn run(&self, source: &str, seed: u64) -> Outcome {
        match self.prepare(source) {
            Ok(p) => self.execute(&p, seed),
            Err((r, why)) => Outcome::Refused(r, why),
        }
    }

    /// Every static gate, and the program's content addresses. Split from
    /// execution so a caller can key novelty and caching on a program's
    /// identity before spending any fuel on it.
    pub fn prepare(&self, source: &str) -> Result<Prepared, (Refusal, String)> {
        let module = match parser::parse(&lexer::lex(source)) {
            Ok(m) => m,
            Err(e) => return Err((Refusal::Parse, e.message)),
        };
        if let Err(why) = check_signature(&module) {
            return Err((Refusal::Signature, why));
        }
        // MAGE-core (§4.12): the subset the kernel runs. The signature check
        // above is this consumer's own contract; this is the language's.
        if let Some(d) = mage_prototype::core_subset::check(&module).into_iter().next() {
            return Err((Refusal::Signature, d.message));
        }
        let typed = types::check(&module);
        if let Some(d) = typed.diagnostics.iter().find(|d| d.severity == hir::Severity::Error) {
            return Err((Refusal::Type, d.message.clone()));
        }
        let fx = effects::infer_effects(&module);
        if let Some(d) = fx.diagnostics.iter().find(|d| d.severity == hir::Severity::Error) {
            return Err((Refusal::Effect, d.message.clone()));
        }
        match fx.inferred.get(ENTRY) {
            Some(set) if set.is_empty() => {}
            Some(set) => {
                let names: Vec<String> = set.iter().map(|e| e.to_string()).collect();
                return Err((
                    Refusal::Effect,
                    format!("`{ENTRY}` performs {{ {} }}; generated data must be pure", names.join(", ")),
                ));
            }
            None => return Err((Refusal::Effect, format!("no effect verdict for `{ENTRY}`"))),
        }
        let ast::ItemKind::Function(fd) = &module.items[0].kind else {
            return Err((Refusal::Signature, "the item is not a function".into()));
        };
        let (exact, shape) = (canon::definition_hash(fd), canon::shape_hash(fd));
        Ok(Prepared { module, exact, shape })
    }

    /// Run a prepared program's `gen(seed)` within fuel.
    pub fn execute(&self, prepared: &Prepared, seed: u64) -> Outcome {
        self.execute_metered(prepared, seed).0
    }

    /// [`Substrate::execute`], also returning the fuel spent, *refusals
    /// included*: a program that exhausts its budget spent all of it, and a
    /// calibration that counted only successes would charge that energy to
    /// nothing.
    pub fn execute_metered(&self, prepared: &Prepared, seed: u64) -> (Outcome, u64) {
        // Seeds are kept small so `s` stays in the byte-ish range a program's
        // arithmetic was written for; the seed's job is variety, not magnitude.
        let arg = Value::Int((seed % 256) as i64);
        let (result, fuel_used) = run_metered(&prepared.module, ENTRY, vec![arg], self.fuel);
        (self.outcome(result, fuel_used), fuel_used)
    }

    fn outcome(&self, result: Result<Value, BoundedError>, fuel_used: u64) -> Outcome {
        match result {
            Ok(Value::List(xs)) => {
                let mut bytes = Vec::with_capacity(xs.len().min(self.max_bytes));
                for x in xs.iter().take(self.max_bytes) {
                    match x {
                        Value::Int(n) => bytes.push((*n).rem_euclid(256) as u8),
                        other => {
                            return Outcome::Refused(
                                Refusal::Runtime,
                                format!("element `{other}` is not an integer"),
                            );
                        }
                    }
                }
                if bytes.is_empty() {
                    return Outcome::Refused(Refusal::Empty, "empty output".into());
                }
                Outcome::Bytes { bytes, fuel_used }
            }
            Ok(other) => Outcome::Refused(Refusal::Runtime, format!("returned `{other}`, not a list")),
            Err(BoundedError::FuelExhausted) => Outcome::Refused(Refusal::Fuel, "fuel exhausted".into()),
            Err(BoundedError::Error(e)) => Outcome::Refused(Refusal::Runtime, e),
        }
    }
}

/// Exactly one item, `f gen(s: usize) -> [usize]`.
fn check_signature(module: &ast::Module) -> Result<(), String> {
    let fns: Vec<&ast::FunctionDef> = module
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ast::ItemKind::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    if module.items.len() != 1 || fns.len() != 1 {
        return Err(format!("expected exactly one item, `f {ENTRY}`; found {}", module.items.len()));
    }
    let f = fns[0];
    let role = module.items[0].attributes.iter().find(|a| a.name == "role");
    match role.and_then(|a| a.args.first()) {
        Some(r) if r == "candidate" => {}
        Some(r) => return Err(format!("`{ENTRY}` must have role `candidate`, not `{r}`")),
        None => return Err(format!("`{ENTRY}` must declare `@role(candidate)`")),
    }
    if f.name != ENTRY {
        return Err(format!("the function must be named `{ENTRY}`, not `{}`", f.name));
    }
    if f.params.len() != 1 {
        return Err(format!("`{ENTRY}` takes exactly one parameter"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub() -> Substrate {
        Substrate { fuel: 100_000, max_bytes: 64 }
    }

    #[test]
    fn a_valid_program_yields_bytes() {
        let src = "@role(candidate)\nf gen(s: usize) -> [usize] { range(4).map(|x| x * 2 + s) }";
        match sub().run(src, 3) {
            Outcome::Bytes { bytes, .. } => assert_eq!(bytes, vec![3, 5, 7, 9]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn output_is_taken_mod_256_and_truncated() {
        let src = "@role(candidate)\nf gen(s: usize) -> [usize] { range(100).map(|x| x + 250) }";
        match sub().run(src, 0) {
            Outcome::Bytes { bytes, .. } => {
                assert_eq!(bytes.len(), 64);
                assert_eq!(&bytes[..7], &[250, 251, 252, 253, 254, 255, 0]);
            }
            other => panic!("{other:?}"),
        }
    }

    fn refusal(src: &str) -> Refusal {
        match sub().run(src, 1) {
            Outcome::Refused(r, _) => r,
            other => panic!("expected a refusal for {src}, got {other:?}"),
        }
    }

    #[test]
    fn each_gate_refuses_for_its_own_reason() {
        assert_eq!(refusal("@role(candidate)\nf gen(s: usize) -> [usize] { range( }"), Refusal::Parse);
        assert_eq!(refusal("f other(s: usize) -> [usize] { range(3) }"), Refusal::Signature);
        assert_eq!(
            refusal("@role(candidate)\nf gen(s: usize) -> [usize] { range(3) }\nf h() -> usize { 1 }"),
            Refusal::Signature
        );
        assert_eq!(refusal("@role(candidate)\nf gen(s: usize) -> [usize] { \"abc\" }"), Refusal::Type);
        // Typechecked clean until method calls were typed (item 39): the
        // type gate let it through and the evaluator refused it at run time.
        assert_eq!(refusal("@role(candidate)\nf gen(s: usize) -> [usize] { range(3).map(|x| x + \"a\") }"), Refusal::Type);
        assert_eq!(
            refusal("@role(candidate)\nf gen(s: usize) -> [usize] { m i = 0\n @w 1b { i = i + 1 }\n [i] }"),
            Refusal::Fuel
        );
        assert_eq!(refusal("@role(candidate)\nf gen(s: usize) -> [usize] { range(0) }"), Refusal::Empty);
    }

    #[test]
    fn a_program_without_the_candidate_role_is_refused() {
        for src in [
            "f gen(s: usize) -> [usize] { range(3) }",
            "@role(evaluator)\nf gen(s: usize) -> [usize] { range(3) }",
        ] {
            assert_eq!(refusal(src), Refusal::Signature, "{src}");
        }
    }

    #[test]
    fn a_candidate_that_reads_held_out_data_is_refused_by_the_language() {
        let src = "@role(candidate)\nf gen(s: usize) -> [usize] { heldout.read(\"test\") }";
        match sub().run(src, 1) {
            Outcome::Refused(Refusal::Effect, why) => assert!(why.contains("candidate"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_effectful_program_is_refused_even_undeclared() {
        // No `/ io` on the signature: the verdict is inferred, not trusted.
        let src = "@role(candidate)\nf gen(s: usize) -> [usize] { println(\"hi\")\n range(3) }";
        let r = sub().run(src, 1);
        assert!(
            matches!(r, Outcome::Refused(Refusal::Effect, _) | Outcome::Refused(Refusal::Type, _)),
            "{r:?}"
        );
    }
}
