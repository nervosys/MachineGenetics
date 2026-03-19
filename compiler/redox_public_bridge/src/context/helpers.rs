//! A set of traits that define a stable interface to redox's internals.
//!
//! These traits abstract redox's internal APIs, allowing redox_public to maintain a stable
//! interface regardless of internal compiler changes.

use redox_middle::mir::interpret::AllocRange;
use redox_middle::ty;
use redox_middle::ty::Ty;
use redox_span::def_id::DefId;

pub trait TyHelpers<'tcx> {
    fn new_foreign(&self, def_id: DefId) -> Ty<'tcx>;
}

pub trait TypingEnvHelpers<'tcx> {
    fn fully_monomorphized(&self) -> ty::TypingEnv<'tcx>;
}

pub trait AllocRangeHelpers<'tcx> {
    fn alloc_range(&self, offset: redox_abi::Size, size: redox_abi::Size) -> AllocRange;
}
