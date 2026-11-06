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
//! * **selection** is Pareto, over bits per byte, joules and latency, via
//!   `germline::pareto` — because a weighted sum cannot see a non-convex front.
//!
//! Why each piece is shaped the way it is lives in its module; the reasons
//! trace to measurements in `HANDOFF.md` and to the research reviewed on
//! 2026-09-25.

pub mod agents;
pub mod arena;
pub mod energy;
pub mod grammar;
pub mod harness;
pub mod learner;
pub mod measure;
pub mod substrate;
#[cfg(feature = "transformer")]
pub mod transformer;
