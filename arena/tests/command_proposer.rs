//! The command-backed proposer, end to end: the backend's protocol, its failure
//! and timeout paths, and whole runs in which a command agent's programs go
//! through the same gate, budget and energy accounting as any agent's.

use arena::agents::Role;
use arena::arena::{run, Config, Corpus};
use arena::command::{Backend, CommandBackend};
use arena::energy::{Basis, Meter, Reading};
use arena::grammar::Program;
use arena::kernel::PROGRAMS;
use arena::learner::{LearnerConfig, LearnerSpec};
use arena::substrate::Substrate;
use std::time::{Duration, Instant};

const PROPOSER: &str = env!("CARGO_BIN_EXE_fixed-proposer");

/// A file under the temp directory holding `contents`; returns its path.
fn fixture(name: &str, contents: &str) -> String {
    let dir = std::env::temp_dir().join(format!("command-proposer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temp directory");
    let path = dir.join(name);
    std::fs::write(&path, contents).expect("a fixture file");
    path.to_string_lossy().into_owned()
}

/// A backend that runs the fixed proposer with `args` after its path.
fn backend(args: &[&str], timeout: Duration) -> CommandBackend {
    let mut argv = vec![PROPOSER.to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    CommandBackend { argv, timeout }
}

/// A hardware counter that reads exactly one joule per span, so the accounting
/// is exact.
struct OneJoule;

impl Meter for OneJoule {
    fn name(&self) -> String {
        "one-joule".into()
    }
    fn start(&mut self) {}
    fn stop(&mut self) -> Reading {
        Reading { meter: self.name(), joules: Some(1.0), basis: Basis::Measured }
    }
}

fn heldout() -> Vec<Corpus> {
    vec![Corpus::new("text", b"the quick brown fox jumps over the lazy dog. ".repeat(20))]
}

/// A short run whose only agent is a command, running `argv`.
fn command_run(argv: Vec<String>) -> Config {
    Config {
        rounds: 3,
        programs_per_round: 4,
        substrate: Substrate { fuel: 50_000, max_bytes: 128 },
        learners: vec![LearnerSpec::Ngram(LearnerConfig { orders: 2, log2_buckets: 8, lr: 0.05 })],
        agents: vec![("command".into(), 1)],
        eval_every: 3,
        replay_per_round: 4,
        proposer_command: Some(argv),
        ..Config::default()
    }
}

const KNOWN: &str = "range(8)\n---\nrange(16).map(|x| wrapping_mul(x, 3))\n---\n[7; 5]\n";

#[test]
fn the_backend_returns_what_the_command_prints() {
    let file = fixture("reply.txt", KNOWN);
    let mut b = backend(&[&file], Duration::from_secs(30));
    assert_eq!(b.complete("a prompt the command ignores"), Ok(KNOWN.to_string()));
}

#[test]
fn a_failing_command_is_an_error_carrying_its_stderr() {
    let file = fixture("fail.txt", KNOWN);
    let err = backend(&[&file, "--fail"], Duration::from_secs(30)).complete("p").expect_err("it fails");
    assert!(err.contains("failing on request"), "{err}");
    assert!(err.contains("exited with"), "{err}");
}

#[test]
fn a_command_past_its_timeout_is_killed_and_returns_early() {
    let file = fixture("slow.txt", KNOWN);
    let started = Instant::now();
    let err = backend(&[&file, "--sleep", "3000"], Duration::from_secs(1)).complete("p").expect_err("it times out");
    let took = started.elapsed();
    assert!(err.contains("timeout"), "{err}");
    assert!(took < Duration::from_secs(3), "returned after {took:?}");
}

#[test]
fn a_command_agent_runs_in_a_full_run_and_its_proposals_are_metered() {
    let file = fixture("known.txt", KNOWN);
    let r = run(command_run(vec![PROPOSER.into(), file]), heldout(), vec![Box::new(OneJoule)], vec![])
        .expect("the run completes");
    assert_eq!(r.rounds_completed, 3);
    let a = &r.agents[0];
    assert_eq!(a.kind, "command");
    assert_eq!(a.role, Role::Proposer);
    assert!(a.proposed > 0, "{a:?}");
    assert!(a.produced > 0, "{a:?}");
    // One joule for each round's proposal call, and one for each candidate's
    // own span: the proposal is charged to the agent that made it.
    assert_eq!(a.joules, (r.rounds_completed + a.proposed) as f64, "{a:?}");
    assert_eq!(r.energy.by_role.get("proposer").copied(), Some(a.joules));
    assert_eq!(r.energy.by_kind.get("command").copied(), Some(a.joules));
}

#[test]
fn a_command_agent_competes_with_a_grammar_agent_on_the_same_run() {
    let file = fixture("known.txt", KNOWN);
    let cfg = Config { agents: vec![("command".into(), 1), ("grammar".into(), 1)], ..command_run(vec![PROPOSER.into(), file]) };
    let r = run(cfg, heldout(), vec![Box::new(OneJoule)], vec![]).expect("the run completes");
    assert_eq!(r.agents.len(), 2);
    assert!(r.agents.iter().all(|a| a.proposed > 0 && a.joules > 0.0), "{:?}", r.agents);
    // F2: the command and the grammar agent are separate accounts by kind, and
    // both still sit in the one `proposer` role.
    let by_kind = &r.energy.by_kind;
    let (command, grammar) = (by_kind.get("command").copied(), by_kind.get("grammar").copied());
    assert!(command.is_some_and(|j| j > 0.0), "{by_kind:?}");
    assert!(grammar.is_some_and(|j| j > 0.0), "{by_kind:?}");
    assert_eq!(by_kind.len(), 2, "{by_kind:?}");
    let kind_sum: f64 = by_kind.values().sum();
    assert!((kind_sum - r.energy.account(PROGRAMS)).abs() < 1e-6, "{kind_sum} vs the PROGRAMS account");
    let proposer = r.energy.by_role.get("proposer").copied().unwrap_or(0.0);
    assert!((proposer - kind_sum).abs() < 1e-6, "proposer {proposer} vs by_kind {kind_sum}");
}

/// No bypass: a program the substrate refuses is refused the same way when a
/// command writes it. Fuel is the refusal the grammar can reach (the substrate
/// truncates long outputs rather than refusing them), so the budget is set
/// between a light program's cost and a heavy one's, measured here.
#[test]
fn a_program_the_gate_refuses_is_refused_in_the_commands_standing() {
    let light = "range(4)";
    let heavy = "flatten([flatten([range(48); 6]); 6])";
    let measure = Substrate { fuel: 10_000_000, max_bytes: 128 };
    let fuel_of = |body: &str| {
        let source = Program::parse(body).expect("a grammar program").source();
        let prepared = measure.prepare(&source).expect("a well-formed program");
        measure.execute_metered(&prepared, 3).1
    };
    let (light_fuel, heavy_fuel) = (fuel_of(light), fuel_of(heavy));
    assert!(heavy_fuel > light_fuel, "{light_fuel} vs {heavy_fuel}");
    let file = fixture("heavy.txt", &format!("{light}\n---\n{heavy}\n"));
    let cfg = Config {
        substrate: Substrate { fuel: (light_fuel + heavy_fuel) / 2, max_bytes: 128 },
        ..command_run(vec![PROPOSER.into(), file])
    };
    let r = run(cfg, heldout(), vec![Box::new(OneJoule)], vec![]).expect("the run completes");
    let a = &r.agents[0];
    assert!(a.refusals.get("fuel").copied().unwrap_or(0) > 0, "the heavy program was not refused: {:?}", a.refusals);
    assert!(a.produced > 0, "the light program ran: {a:?}");
    assert_eq!(r.refusals.get("fuel"), a.refusals.get("fuel"), "the run's refusals are the agent's");
}
