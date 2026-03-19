use redox_hir::def_id::DefId;
pub use redox_type_ir::fast_reject::*;

use super::TyCtxt;

pub type DeepRejectCtxt<
    'tcx,
    const INSTANTIATE_LHS_WITH_INFER: bool,
    const INSTANTIATE_RHS_WITH_INFER: bool,
> = redox_type_ir::fast_reject::DeepRejectCtxt<
    TyCtxt<'tcx>,
    INSTANTIATE_LHS_WITH_INFER,
    INSTANTIATE_RHS_WITH_INFER,
>;

pub type SimplifiedType = redox_type_ir::fast_reject::SimplifiedType<DefId>;
