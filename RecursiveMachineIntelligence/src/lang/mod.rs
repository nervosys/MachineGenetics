//! # RMIL — Recursive Machine Intelligence Language
//!
//! A compact, binary, neurosymbolic language optimized for AI agents.
//!
//! ## Design principles
//!
//! 1. **Machine-first**: binary is canonical, with optional text syntax
//! 2. **Algebraic composition**: `>>` (sequential) and `|` (parallel) operators
//! 3. **Content-addressed**: every sub-expression has a deterministic u64 hash
//! 4. **Self-describing**: programs carry their own instruction set metadata
//! 5. **Neurosymbolic native**: first-class neural AND symbolic operations
//! 6. **Agent-native**: send/recv/spawn are language primitives
//!
//! ## Building programs
//!
//! ```
//! use rmi::lang::*;
//!
//! // A transformer block:
//! let block =
//!     Expr::op1(Op::LAYER_NORM)
//!     >> Expr::op1(Op::ATTN)
//!     >> Expr::op1(Op::DROP)
//!     >> Expr::op1(Op::LAYER_NORM)
//!     >> Expr::op1(Op::LINEAR)
//!     >> Expr::op1(Op::GELU)
//!     >> Expr::op1(Op::LINEAR)
//!     >> Expr::op1(Op::DROP);
//!
//! // Wire-format size:
//! let bytes = codec::wire_size(&block);
//! assert!(bytes < 200); // entire transformer block in ~60 bytes
//! ```

pub mod agent_bridge;
pub mod codec;
pub mod debugger;
pub mod expr;
pub mod ffi;
pub mod grad;
pub mod incremental;
pub mod jit;
pub mod lsp;
pub mod op;
pub mod pattern_match;
pub mod quantize;
pub mod registry;
pub mod sparse;
pub mod sym;
pub mod syntax;
pub mod tensor_rt;
pub mod ty;
pub mod vm;

// ── Re-exports ───────────────────────────────────────────────────────────────

pub use expr::{patterns, Expr, Val};
pub use op::{Op, OpFamily, OpMeta};
pub use sym::{Sym, SymbolTable};
pub use ty::{Dtype, Shape, Ty, DYN};
pub use vm::Vm;
