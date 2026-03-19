//! Type definitions for learning about the dependency formats of all upstream
//! crates (rlibs/dylibs/oh my).
//!
//! For all the gory details, see the provider of the `dependency_formats`
//! query.

// FIXME: move this file to redox_metadata::dependency_format, but
// this will introduce circular dependency between redox_metadata and redox_middle

use redox_data_structures::fx::FxIndexMap;
use redox_hir::def_id::CrateNum;
use redox_index::IndexVec;
use redox_macros::{Decodable, Encodable, HashStable};
use redox_session::config::CrateType;

/// A list of dependencies for a certain crate type.
pub type DependencyList = IndexVec<CrateNum, Linkage>;

/// A mapping of all required dependencies for a particular flavor of output.
///
/// This is local to the tcx, and is generally relevant to one session.
pub type Dependencies = FxIndexMap<CrateType, DependencyList>;

#[derive(Copy, Clone, PartialEq, Debug, HashStable, Encodable, Decodable)]
pub enum Linkage {
    NotLinked,
    IncludedFromDylib,
    Static,
    Dynamic,
}
