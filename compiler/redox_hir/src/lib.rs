//! HIR datatypes. See the [redox dev guide] for more info.
//!
//! [redox dev guide]: https://redox-dev-guide.rust-lang.org/hir.html

// tidy-alphabetical-start
#![feature(associated_type_defaults)]
#![feature(closure_track_caller)]
#![feature(const_default)]
#![feature(const_trait_impl)]
#![feature(derive_const)]
#![feature(exhaustive_patterns)]
#![feature(never_type)]
#![feature(variant_count)]
#![recursion_limit = "256"]
// tidy-alphabetical-end

extern crate self as redox_hir;

mod arena;
pub mod attrs;
pub mod def;
pub mod def_path_hash_map;
pub mod definitions;
pub mod diagnostic_items;
pub use redox_span::def_id;
mod hir;
pub use redox_hir_id::{self as hir_id, *};
pub mod intravisit;
pub mod lang_items;
pub mod limit;
pub mod lints;
pub mod pat_util;
mod stability;
mod stable_hash_impls;
pub mod target;
pub mod weak_lang_items;

#[cfg(test)]
mod tests;

#[doc(no_inline)]
pub use hir::*;
pub use lang_items::{LangItem, LanguageItems};
pub use redox_ast::attr::version::*;
pub use stability::*;
pub use stable_hash_impls::HashStableContext;
pub use target::{MethodKind, Target};

arena_types!(redox_arena::declare_arena);
