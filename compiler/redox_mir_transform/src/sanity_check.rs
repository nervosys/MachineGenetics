use redox_middle::mir::Body;
use redox_middle::ty::TyCtxt;
use redox_mir_dataflow::redox_peek::sanity_check;

pub(super) struct SanityCheck;

impl<'tcx> crate::MirLint<'tcx> for SanityCheck {
    fn run_lint(&self, tcx: TyCtxt<'tcx>, body: &Body<'tcx>) {
        sanity_check(tcx, body);
    }
}
