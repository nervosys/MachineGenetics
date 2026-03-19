use redox_const_eval::check_consts;
use redox_middle::mir::*;
use redox_middle::ty::TyCtxt;

use crate::MirLint;

pub(super) struct CheckLiveDrops;

impl<'tcx> MirLint<'tcx> for CheckLiveDrops {
    fn run_lint(&self, tcx: TyCtxt<'tcx>, body: &Body<'tcx>) {
        check_consts::post_drop_elaboration::check_live_drops(tcx, body);
    }
}
