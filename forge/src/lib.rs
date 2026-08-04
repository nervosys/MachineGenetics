pub mod api;
pub mod cli;
pub mod compat;
pub mod models;
pub mod project;
pub mod manifest;
pub mod registry;

// The build engine (`../ribosome`) and the RSI control plane (`../germline`)
// were both developed here and both moved out on 2026-08-04. Neither belonged
// in a package registry, and forge does not use either: the dependency was
// upward, from them to nothing, and removing the declarations removed the
// dependency entirely. See ARCHITECTURE.md §"Repository layout".
