pub mod api;
pub mod cli;
pub mod compat;
pub mod models;
pub mod project;
pub mod manifest;
pub mod germline;
pub mod registry;

/// The build engine, re-exported.
///
/// It lives in its own crate (`../ribosome`) because a build system that ships
/// inside a package registry can only ever be that registry's build system.
/// [`germline`] drives it, so it is re-exported here rather than made a second
/// thing callers have to know about.
pub use ribosome;
