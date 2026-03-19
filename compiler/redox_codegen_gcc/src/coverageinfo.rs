use redox_codegen_ssa::traits::CoverageInfoBuilderMethods;
use redox_middle::mir::coverage::CoverageKind;
use redox_middle::ty::Instance;

use crate::builder::Builder;

impl<'a, 'gcc, 'tcx> CoverageInfoBuilderMethods<'tcx> for Builder<'a, 'gcc, 'tcx> {
    fn add_coverage(&mut self, _instance: Instance<'tcx>, _kind: &CoverageKind) {
        // FIXME(antoyo)
    }
}
