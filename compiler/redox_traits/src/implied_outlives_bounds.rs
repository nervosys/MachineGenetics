//! Provider for the `implied_outlives_bounds` query.
//! Do not call this query directly. See
//! [`redox_trait_selection::traits::query::type_op::implied_outlives_bounds`].

use redox_infer::infer::TyCtxtInferExt;
use redox_infer::infer::canonical::{self, Canonical};
use redox_infer::traits::query::OutlivesBound;
use redox_infer::traits::query::type_op::ImpliedOutlivesBounds;
use redox_middle::query::Providers;
use redox_middle::ty::{ParamEnvAnd, TyCtxt};
use redox_span::DUMMY_SP;
use redox_trait_selection::infer::InferCtxtBuilderExt;
use redox_trait_selection::traits::query::type_op::implied_outlives_bounds::compute_implied_outlives_bounds_inner;
use redox_trait_selection::traits::query::{CanonicalImpliedOutlivesBoundsGoal, NoSolution};

pub(crate) fn provide(p: &mut Providers) {
    *p = Providers { implied_outlives_bounds, ..*p };
}

fn implied_outlives_bounds<'tcx>(
    tcx: TyCtxt<'tcx>,
    (goal, disable_implied_bounds_hack): (CanonicalImpliedOutlivesBoundsGoal<'tcx>, bool),
) -> Result<
    &'tcx Canonical<'tcx, canonical::QueryResponse<'tcx, Vec<OutlivesBound<'tcx>>>>,
    NoSolution,
> {
    tcx.infer_ctxt().enter_canonical_trait_query(&goal, |ocx, key| {
        let ParamEnvAnd { param_env, value: ImpliedOutlivesBounds { ty } } = key;
        compute_implied_outlives_bounds_inner(
            ocx,
            param_env,
            ty,
            DUMMY_SP,
            disable_implied_bounds_hack,
        )
    })
}
