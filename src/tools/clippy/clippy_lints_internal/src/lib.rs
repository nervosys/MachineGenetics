#![feature(redox_private)]
#![allow(
    clippy::missing_docs_in_private_items,
    clippy::must_use_candidate,
    clippy::symbol_as_str
)]
#![warn(
    trivial_casts,
    trivial_numeric_casts,
    rust_2018_idioms,
    unused_lifetimes,
    unused_qualifications,
    redox::internal
)]
// Disable this redox lint for now, as it was also done in redox
#![allow(redox::potential_query_instability)]
// None of these lints need a version.
#![allow(clippy::missing_clippy_version_attribute)]

extern crate redox_ast;
extern crate redox_attr_parsing;
extern crate redox_data_structures;
extern crate redox_errors;
extern crate redox_hir;
extern crate redox_lint;
extern crate redox_lint_defs;
extern crate redox_middle;
extern crate redox_session;
extern crate redox_span;

mod almost_standard_lint_formulation;
mod collapsible_span_lint_calls;
mod derive_deserialize_allowing_unknown;
mod internal_paths;
mod lint_without_lint_pass;
mod msrv_attr_impl;
mod outer_expn_data_pass;
mod produce_ice;
mod repeated_is_diagnostic_item;
mod symbols;
mod unnecessary_def_path;
mod unsorted_clippy_utils_paths;
mod unusual_names;

use redox_lint::{Lint, LintStore};

static LINTS: &[&Lint] = &[
    almost_standard_lint_formulation::ALMOST_STANDARD_LINT_FORMULATION,
    collapsible_span_lint_calls::COLLAPSIBLE_SPAN_LINT_CALLS,
    derive_deserialize_allowing_unknown::DERIVE_DESERIALIZE_ALLOWING_UNKNOWN,
    lint_without_lint_pass::DEFAULT_LINT,
    lint_without_lint_pass::INVALID_CLIPPY_VERSION_ATTRIBUTE,
    lint_without_lint_pass::LINT_WITHOUT_LINT_PASS,
    lint_without_lint_pass::MISSING_CLIPPY_VERSION_ATTRIBUTE,
    msrv_attr_impl::MISSING_MSRV_ATTR_IMPL,
    outer_expn_data_pass::OUTER_EXPN_EXPN_DATA,
    produce_ice::PRODUCE_ICE,
    symbols::INTERNING_LITERALS,
    symbols::SYMBOL_AS_STR,
    unnecessary_def_path::UNNECESSARY_DEF_PATH,
    unsorted_clippy_utils_paths::UNSORTED_CLIPPY_UTILS_PATHS,
    unusual_names::UNUSUAL_NAMES,
];

pub fn register_lints(store: &mut LintStore) {
    store.register_lints(LINTS);

    store.register_early_pass(|| Box::new(unsorted_clippy_utils_paths::UnsortedClippyUtilsPaths));
    store.register_early_pass(|| Box::new(produce_ice::ProduceIce));
    store.register_late_pass(|_| Box::new(collapsible_span_lint_calls::CollapsibleCalls));
    store.register_late_pass(|_| Box::new(derive_deserialize_allowing_unknown::DeriveDeserializeAllowingUnknown));
    store.register_late_pass(|_| Box::<symbols::Symbols>::default());
    store.register_late_pass(|_| Box::<lint_without_lint_pass::LintWithoutLintPass>::default());
    store.register_late_pass(|_| Box::new(unnecessary_def_path::UnnecessaryDefPath));
    store.register_late_pass(|_| Box::new(outer_expn_data_pass::OuterExpnDataPass));
    store.register_late_pass(|_| Box::new(msrv_attr_impl::MsrvAttrImpl));
    store.register_late_pass(|_| Box::new(almost_standard_lint_formulation::AlmostStandardFormulation::new()));
    store.register_late_pass(|_| Box::new(unusual_names::UnusualNames));
    store.register_late_pass(|_| Box::new(repeated_is_diagnostic_item::RepeatedIsDiagnosticItem));
}
