//! The incremental, cached compilation engine.
//!
//! Three layers:
//!
//! * [`engine`] — a generic demand-driven query database (memoisation,
//!   dependency tracking, deep verification, early cutoff, cycle detection,
//!   statistics, an optional durable [`engine::Store`]). It knows nothing of MAGE.
//! * [`disk`] — the content-addressed on-disk store and the compiler's build id.
//! * [`mage`] — MAGE's stages as queries, the pass registry, and
//!   [`Compiler`], the facade the CLI and tests drive.
//!
//! See `mage.rs` for the query graph and for which analyses are per item and
//! which are whole-module, and why.

pub mod disk;
pub mod engine;
pub mod mage;

#[cfg(test)]
mod tests;

pub use engine::{Db, QueryError, Stats};
pub use mage::{Compiler, Config, PassOut, Registry, Settings};
