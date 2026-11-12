//! # The arena
//!
//! MAGE's self-driven improvement loop, built to the objective set on
//! 2026-09-25: **intelligence per second per watt for a fixed unit of
//! compute**, optimised over convex and non-convex decision variables, by one
//! to many agents that collaborate and compete.
//!
//! The design is self-play pretraining (Cowsik et al., arXiv:2609.30063) with
//! MAGE as the program substrate:
//!
//! * **agents** ([`agents`]) write MAGE programs — the combinatorial,
//!   non-convex half of the search, over [`grammar`]'s typed program trees;
//! * the **compiler** ([`substrate`]) parses, typechecks, effect-checks and
//!   fuel-bounds every one, and turns its output into bytes;
//! * **learners** ([`learner`]) predict those bytes, and reward each program by
//!   the learning progress it causes — not by how hard it is;
//! * **held-out natural data** scores the learners in bits per byte, an exact
//!   likelihood no agent can reach or edit;
//! * **energy** ([`energy`]) is read from hardware counters where they exist
//!   and labelled an estimate where they do not;
//! * the **trusted kernel** ([`kernel`]) is what the loop cannot vary: it owns
//!   held-out data, and moving authority takes the gate's approval;
//! * **selection** is Pareto where it decides a promotion or a parent, and
//!   scalar where it decides what the arena keeps. Pareto: promotion under
//!   `arena succession --objectives` (`germline::gate`'s dominance rule over
//!   capability, efficiency and latency); parent choice under the Pareto
//!   selection mode (`germline::variation::Selection::Pareto`); and the reported
//!   learners' front and hypervolume credit (`germline::pareto`, in
//!   [`arena`]). Scalar, by reward: program retention in the pool
//!   (`agents::Pool::add_with`), the mutator's and attacker's elite
//!   (`agents::Pool::top`, called from `MutatorAgent` and `AttackerAgent`), and
//!   the prompt examples the command proposer is shown (`command`). A weighted
//!   sum cannot see a non-convex front, which is why promotion and the reported
//!   front are not scalar.
//!
//! Why each piece is shaped the way it is lives in its module; the reasons
//! trace to measurements in `HANDOFF.md` and to the research reviewed on
//! 2026-09-25.

pub mod agents;
pub mod arena;
pub mod command;
pub mod energy;
pub mod entropy;
pub mod evidence;
pub mod ssm;
pub mod grammar;
pub mod harness;
pub mod kernel;
pub mod learner;
pub mod limit_proposer;
pub mod measure;
pub mod physics;
pub mod physics_task;
pub mod rollout;
pub mod substrate;
pub mod succession;
pub mod transfer;
#[cfg(feature = "transformer")]
pub mod transformer;
