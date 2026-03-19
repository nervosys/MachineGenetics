#![feature(redox_private)]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    rust_2018_idioms,
    unused_lifetimes,
    unused_qualifications
)]
#![allow(clippy::must_use_candidate, clippy::missing_panics_doc)]
#![deny(clippy::derive_deserialize_allowing_unknown)]

extern crate redox_data_structures;
extern crate redox_errors;
extern crate redox_hir;
extern crate redox_middle;
extern crate redox_session;
extern crate redox_span;

mod conf;
mod metadata;
pub mod types;

pub use conf::{Conf, get_configuration_metadata, lookup_conf_file, sanitize_explanation};
pub use metadata::ClippyConfiguration;
