//! # MAGE Standard Library
//!
//! The `std` module is the root of the MAGE standard library.
//! All standard types, traits, and functions are accessible through
//! double-colon-separated paths: `std::io`, `std::col::Map`, `std::agent::Swarm`, etc.
//!
//! ## Design Principles
//!
//! - **Intelligence-first**: neural networks, knowledge bases, evolutionary
//!   algorithms, and autonomous agents are first-class constructs
//! - **C-family keywords**: `fn`, `let`, `struct`, `enum`, `trait`, `impl`, `pub`, `match`, etc.
//! - **Effect-aware**: I/O, GPU, LLM, and networking modules declare their effects
//! - **SKB-integrated**: Safety rules are embedded in type contracts

/// I/O: File I/O, streams, buffering
pub mod io;

/// Networking: TCP, UDP, HTTP, DNS
pub mod net;

/// File system: read, write, create, remove, walk
pub mod fs;

/// Collections: Map, Set, BTree, VecDeque, LinkedList
pub mod col;

/// Synchronization: Mutex, RwLock, Channel, Barrier, Atomic
pub mod sync;

/// Async runtime: spawn, join, select, Stream
pub mod async;

/// Formatting: Display, Debug, format, print
pub mod fmt;

/// String utilities: split, join, trim, regex, encode
pub mod str;

/// Mathematics: trig, exp, log, random, simd
pub mod math;

/// Date and time: Instant, Duration, SystemTime
pub mod time;

/// JSON: parse, stringify, Value, Serialize, Deserialize
pub mod json;

/// Environment: args, vars, current_dir
pub mod env;

/// Process management: Command, exit, signal
pub mod process;

/// Agent primitives: Agent, Swarm, Message, Lease, Region, Bus
pub mod agent;

/// Safety Knowledge Base: Rule, query, validate
pub mod skb;

/// Effect system: Effect, handle, perform
pub mod effect;

/// Formal contract verification: @req, @ens, @inv
pub mod spec;

/// Testing framework: #[test], assert, benchmark
pub mod test;

// ---- MAGE AI Modules ----

/// Neural networks: net definitions, layers, activations, training
pub mod neural;

/// Tensor algebra: Tensor, Param, shape-checked operations, autograd
pub mod tensor;

/// Evolutionary computation: evolve blocks, Genome, selection, crossover, mutation
pub mod evolve;

/// Knowledge bases: facts, rules, queries, symbolic inference
pub mod kb;

/// Language model integration: LLM, Prompt, Response, embedding
pub mod llm;

/// Reinforcement learning: Env, Policy, PPO, A3C, Trajectory
pub mod rl;

/// Contract system: require, ensure, invariant, verify
pub mod spec;

/// Testing: assert, assert_eq, bench, prop
pub mod test;
