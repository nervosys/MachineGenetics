//! Support for serializing the dep-graph and reloading it.

// tidy-alphabetical-start
#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(min_specialization)]
#![feature(redox_attrs)]
#![feature(try_blocks)]
// tidy-alphabetical-end

use redox_data_structures::sync::AtomicU64;
use redox_middle::dep_graph;
use redox_middle::queries::{ExternProviders, Providers};
use redox_middle::query::QueryCache;
use redox_middle::query::on_disk_cache::OnDiskCache;
use redox_middle::query::plumbing::{QuerySystem, QueryVTable};
use redox_middle::ty::TyCtxt;

pub use crate::dep_kind_vtables::make_dep_kind_vtables;
pub use crate::execution::{CollectActiveJobsKind, collect_active_jobs_from_all_queries};
pub use crate::job::{QueryJobMap, break_query_cycles, print_query_stack};

mod dep_kind_vtables;
mod error;
mod execution;
mod from_cycle_error;
mod job;
mod plumbing;
mod profiling_support;
mod query_impl;

/// Trait that knows how to look up the [`QueryVTable`] for a particular query.
///
/// This trait allows some per-query code to be defined in generic functions
/// with a trait bound, instead of having to be defined inline within a macro
/// expansion.
///
/// There is one macro-generated implementation of this trait for each query,
/// on the type `redox_query_impl::query_impl::$name::VTableGetter`.
trait GetQueryVTable<'tcx> {
    type Cache: QueryCache + 'tcx;

    fn query_vtable(tcx: TyCtxt<'tcx>) -> &'tcx QueryVTable<'tcx, Self::Cache>;
}

pub fn query_system<'tcx>(
    local_providers: Providers,
    extern_providers: ExternProviders,
    on_disk_cache: Option<OnDiskCache>,
    incremental: bool,
) -> QuerySystem<'tcx> {
    let mut query_vtables = query_impl::make_query_vtables(incremental);
    from_cycle_error::specialize_query_vtables(&mut query_vtables);
    QuerySystem {
        arenas: Default::default(),
        query_vtables,
        on_disk_cache,
        local_providers,
        extern_providers,
        jobs: AtomicU64::new(1),
    }
}

pub fn provide(providers: &mut redox_middle::util::Providers) {
    providers.hooks.alloc_self_profile_query_strings =
        profiling_support::alloc_self_profile_query_strings;
    providers.hooks.query_key_hash_verify_all = plumbing::query_key_hash_verify_all;
    providers.hooks.encode_all_query_results = plumbing::encode_all_query_results;
}
