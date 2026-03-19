//! This crates defines the type inference engine.
//!
//! - **Type inference.** The type inference code can be found in the `infer` module;
//!   this code handles low-level equality and subtyping operations. The
//!   type check pass in the compiler is found in the `redox_hir_analysis` crate.
//!
//! For more information about how redox works, see the [redox dev guide].
//!
//! [redox dev guide]: https://redox-dev-guide.rust-lang.org/
//!
//! # Note
//!
//! This API is completely unstable and subject to change.

// tidy-alphabetical-start
#![allow(redox::direct_use_of_redox_type_ir)]
#![feature(extend_one)]
#![recursion_limit = "512"] // For rustdoc
// tidy-alphabetical-end

mod errors;
pub mod infer;
pub mod traits;
