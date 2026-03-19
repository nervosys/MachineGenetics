//! Module containing the translation from redox_public constructs to the redox counterpart.
//!
//! This module will only include a few constructs to allow users to invoke internal redox APIs
//! due to incomplete stable coverage.

// Prefer importing redox_public over internal redox constructs to make this file more readable.

use redox_middle::ty::{self as redox_ty, Const as InternalConst, Ty as InternalTy};
use redox_public_bridge::Tables;

use crate::abi::Layout;
use crate::compiler_interface::BridgeTys;
use crate::mir::alloc::AllocId;
use crate::mir::mono::{Instance, MonoItem, StaticDef};
use crate::mir::{BinOp, Mutability, Place, ProjectionElem, RawPtrKind, Safety, UnOp};
use crate::ty::{
    Abi, AdtDef, Binder, BoundRegionKind, BoundTyKind, BoundVariableKind, ClosureKind,
    ExistentialPredicate, ExistentialProjection, ExistentialTraitRef, FloatTy, FnSig,
    GenericArgKind, GenericArgs, IntTy, MirConst, Movability, Pattern, Region, RigidTy, Span,
    TermKind, TraitRef, Ty, TyConst, UintTy, VariantDef, VariantIdx,
};
use crate::unstable::{InternalCx, RustcInternal};
use crate::{CrateItem, CrateNum, DefId, IndexedVal};

impl RustcInternal for CrateItem {
    type T<'tcx> = redox_span::def_id::DefId;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        self.0.internal(tables, tcx)
    }
}

impl RustcInternal for CrateNum {
    type T<'tcx> = redox_span::def_id::CrateNum;
    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        redox_span::def_id::CrateNum::from_usize(self.0)
    }
}

impl RustcInternal for DefId {
    type T<'tcx> = redox_span::def_id::DefId;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tcx.lift(tables.def_ids[*self]).unwrap()
    }
}

impl RustcInternal for GenericArgs {
    type T<'tcx> = redox_ty::GenericArgsRef<'tcx>;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        InternalCx::mk_args_from_iter(tcx, self.0.iter().map(|arg| arg.internal(tables, tcx)))
    }
}

impl RustcInternal for GenericArgKind {
    type T<'tcx> = redox_ty::GenericArg<'tcx>;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        let arg: redox_ty::GenericArg<'tcx> = match self {
            GenericArgKind::Lifetime(reg) => reg.internal(tables, tcx).into(),
            GenericArgKind::Type(ty) => ty.internal(tables, tcx).into(),
            GenericArgKind::Const(cnst) => cnst.internal(tables, tcx).into(),
        };
        tcx.lift(arg).unwrap()
    }
}

impl RustcInternal for Region {
    type T<'tcx> = redox_ty::Region<'tcx>;
    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        // Cannot recover region. Use erased for now.
        tcx.lifetimes_re_erased()
    }
}

impl RustcInternal for Ty {
    type T<'tcx> = InternalTy<'tcx>;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tcx.lift(tables.types[*self]).unwrap()
    }
}

impl RustcInternal for TyConst {
    type T<'tcx> = InternalConst<'tcx>;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tcx.lift(tables.ty_consts[self.id]).unwrap()
    }
}

impl RustcInternal for Pattern {
    type T<'tcx> = redox_ty::Pattern<'tcx>;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tcx.mk_pat(match self {
            Pattern::Range { start, end, include_end: _ } => redox_ty::PatternKind::Range {
                start: start.as_ref().unwrap().internal(tables, tcx),
                end: end.as_ref().unwrap().internal(tables, tcx),
            },
        })
    }
}

impl RustcInternal for RigidTy {
    type T<'tcx> = redox_ty::TyKind<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            RigidTy::Bool => redox_ty::TyKind::Bool,
            RigidTy::Char => redox_ty::TyKind::Char,
            RigidTy::Int(int_ty) => redox_ty::TyKind::Int(int_ty.internal(tables, tcx)),
            RigidTy::Uint(uint_ty) => redox_ty::TyKind::Uint(uint_ty.internal(tables, tcx)),
            RigidTy::Float(float_ty) => redox_ty::TyKind::Float(float_ty.internal(tables, tcx)),
            RigidTy::Never => redox_ty::TyKind::Never,
            RigidTy::Array(ty, cnst) => {
                redox_ty::TyKind::Array(ty.internal(tables, tcx), cnst.internal(tables, tcx))
            }
            RigidTy::Pat(ty, pat) => {
                redox_ty::TyKind::Pat(ty.internal(tables, tcx), pat.internal(tables, tcx))
            }
            RigidTy::Adt(def, args) => {
                redox_ty::TyKind::Adt(def.internal(tables, tcx), args.internal(tables, tcx))
            }
            RigidTy::Str => redox_ty::TyKind::Str,
            RigidTy::Slice(ty) => redox_ty::TyKind::Slice(ty.internal(tables, tcx)),
            RigidTy::RawPtr(ty, mutability) => {
                redox_ty::TyKind::RawPtr(ty.internal(tables, tcx), mutability.internal(tables, tcx))
            }
            RigidTy::Ref(region, ty, mutability) => redox_ty::TyKind::Ref(
                region.internal(tables, tcx),
                ty.internal(tables, tcx),
                mutability.internal(tables, tcx),
            ),
            RigidTy::Foreign(def) => redox_ty::TyKind::Foreign(def.0.internal(tables, tcx)),
            RigidTy::FnDef(def, args) => {
                redox_ty::TyKind::FnDef(def.0.internal(tables, tcx), args.internal(tables, tcx))
            }
            RigidTy::FnPtr(sig) => {
                let (sig_tys, hdr) = sig.internal(tables, tcx).split();
                redox_ty::TyKind::FnPtr(sig_tys, hdr)
            }
            RigidTy::Closure(def, args) => {
                redox_ty::TyKind::Closure(def.0.internal(tables, tcx), args.internal(tables, tcx))
            }
            RigidTy::Coroutine(def, args) => {
                redox_ty::TyKind::Coroutine(def.0.internal(tables, tcx), args.internal(tables, tcx))
            }
            RigidTy::CoroutineClosure(def, args) => redox_ty::TyKind::CoroutineClosure(
                def.0.internal(tables, tcx),
                args.internal(tables, tcx),
            ),
            RigidTy::CoroutineWitness(def, args) => redox_ty::TyKind::CoroutineWitness(
                def.0.internal(tables, tcx),
                args.internal(tables, tcx),
            ),
            RigidTy::Dynamic(predicate, region) => redox_ty::TyKind::Dynamic(
                tcx.mk_poly_existential_predicates(&predicate.internal(tables, tcx)),
                region.internal(tables, tcx),
            ),
            RigidTy::Tuple(tys) => {
                redox_ty::TyKind::Tuple(tcx.mk_type_list(&tys.internal(tables, tcx)))
            }
        }
    }
}

impl RustcInternal for IntTy {
    type T<'tcx> = redox_ty::IntTy;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            IntTy::Isize => redox_ty::IntTy::Isize,
            IntTy::I8 => redox_ty::IntTy::I8,
            IntTy::I16 => redox_ty::IntTy::I16,
            IntTy::I32 => redox_ty::IntTy::I32,
            IntTy::I64 => redox_ty::IntTy::I64,
            IntTy::I128 => redox_ty::IntTy::I128,
        }
    }
}

impl RustcInternal for UintTy {
    type T<'tcx> = redox_ty::UintTy;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            UintTy::Usize => redox_ty::UintTy::Usize,
            UintTy::U8 => redox_ty::UintTy::U8,
            UintTy::U16 => redox_ty::UintTy::U16,
            UintTy::U32 => redox_ty::UintTy::U32,
            UintTy::U64 => redox_ty::UintTy::U64,
            UintTy::U128 => redox_ty::UintTy::U128,
        }
    }
}

impl RustcInternal for FloatTy {
    type T<'tcx> = redox_ty::FloatTy;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            FloatTy::F16 => redox_ty::FloatTy::F16,
            FloatTy::F32 => redox_ty::FloatTy::F32,
            FloatTy::F64 => redox_ty::FloatTy::F64,
            FloatTy::F128 => redox_ty::FloatTy::F128,
        }
    }
}

impl RustcInternal for Mutability {
    type T<'tcx> = redox_ty::Mutability;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            Mutability::Not => redox_ty::Mutability::Not,
            Mutability::Mut => redox_ty::Mutability::Mut,
        }
    }
}

impl RustcInternal for Movability {
    type T<'tcx> = redox_ty::Movability;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            Movability::Static => redox_ty::Movability::Static,
            Movability::Movable => redox_ty::Movability::Movable,
        }
    }
}

impl RustcInternal for RawPtrKind {
    type T<'tcx> = redox_middle::mir::RawPtrKind;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            RawPtrKind::Mut => redox_middle::mir::RawPtrKind::Mut,
            RawPtrKind::Const => redox_middle::mir::RawPtrKind::Const,
            RawPtrKind::FakeForPtrMetadata => redox_middle::mir::RawPtrKind::FakeForPtrMetadata,
        }
    }
}

impl RustcInternal for FnSig {
    type T<'tcx> = redox_ty::FnSig<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tcx.lift(redox_ty::FnSig {
            inputs_and_output: tcx.mk_type_list(&self.inputs_and_output.internal(tables, tcx)),
            c_variadic: self.c_variadic,
            safety: self.safety.internal(tables, tcx),
            abi: self.abi.internal(tables, tcx),
        })
        .unwrap()
    }
}

impl RustcInternal for VariantIdx {
    type T<'tcx> = redox_abi::VariantIdx;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        redox_abi::VariantIdx::from(self.to_index())
    }
}

impl RustcInternal for VariantDef {
    type T<'tcx> = &'tcx redox_ty::VariantDef;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        self.adt_def.internal(tables, tcx).variant(self.idx.internal(tables, tcx))
    }
}

impl RustcInternal for MirConst {
    type T<'tcx> = redox_middle::mir::Const<'tcx>;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        let constant = tables.mir_consts[self.id];
        match constant {
            redox_middle::mir::Const::Ty(ty, ct) => {
                redox_middle::mir::Const::Ty(tcx.lift(ty).unwrap(), tcx.lift(ct).unwrap())
            }
            redox_middle::mir::Const::Unevaluated(uneval, ty) => {
                redox_middle::mir::Const::Unevaluated(
                    tcx.lift(uneval).unwrap(),
                    tcx.lift(ty).unwrap(),
                )
            }
            redox_middle::mir::Const::Val(const_val, ty) => {
                redox_middle::mir::Const::Val(tcx.lift(const_val).unwrap(), tcx.lift(ty).unwrap())
            }
        }
    }
}

impl RustcInternal for MonoItem {
    type T<'tcx> = redox_middle::mir::mono::MonoItem<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        use redox_middle::mir::mono as redox_mono;
        match self {
            MonoItem::Fn(instance) => redox_mono::MonoItem::Fn(instance.internal(tables, tcx)),
            MonoItem::Static(def) => redox_mono::MonoItem::Static(def.internal(tables, tcx)),
            MonoItem::GlobalAsm(_) => {
                unimplemented!()
            }
        }
    }
}

impl RustcInternal for Instance {
    type T<'tcx> = redox_ty::Instance<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tcx.lift(tables.instances[self.def]).unwrap()
    }
}

impl RustcInternal for StaticDef {
    type T<'tcx> = redox_span::def_id::DefId;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        self.0.internal(tables, tcx)
    }
}

#[allow(redox::usage_of_qualified_ty)]
impl<T> RustcInternal for Binder<T>
where
    T: RustcInternal,
    for<'tcx> T::T<'tcx>: redox_ty::TypeVisitable<redox_ty::TyCtxt<'tcx>>,
{
    type T<'tcx> = redox_ty::Binder<'tcx, T::T<'tcx>>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        redox_ty::Binder::bind_with_vars(
            self.value.internal(tables, tcx),
            tcx.mk_bound_variable_kinds_from_iter(
                self.bound_vars.iter().map(|bound| bound.internal(tables, tcx)),
            ),
        )
    }
}

impl RustcInternal for BoundVariableKind {
    type T<'tcx> = redox_ty::BoundVariableKind<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            BoundVariableKind::Ty(kind) => redox_ty::BoundVariableKind::Ty(match kind {
                BoundTyKind::Anon => redox_ty::BoundTyKind::Anon,
                BoundTyKind::Param(def, _symbol) => {
                    redox_ty::BoundTyKind::Param(def.0.internal(tables, tcx))
                }
            }),
            BoundVariableKind::Region(kind) => redox_ty::BoundVariableKind::Region(match kind {
                BoundRegionKind::BrAnon => redox_ty::BoundRegionKind::Anon,
                BoundRegionKind::BrNamed(def, _symbol) => {
                    redox_ty::BoundRegionKind::Named(def.0.internal(tables, tcx))
                }
                BoundRegionKind::BrEnv => redox_ty::BoundRegionKind::ClosureEnv,
            }),
            BoundVariableKind::Const => redox_ty::BoundVariableKind::Const,
        }
    }
}

impl RustcInternal for ExistentialPredicate {
    type T<'tcx> = redox_ty::ExistentialPredicate<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            ExistentialPredicate::Trait(trait_ref) => {
                redox_ty::ExistentialPredicate::Trait(trait_ref.internal(tables, tcx))
            }
            ExistentialPredicate::Projection(proj) => {
                redox_ty::ExistentialPredicate::Projection(proj.internal(tables, tcx))
            }
            ExistentialPredicate::AutoTrait(trait_def) => {
                redox_ty::ExistentialPredicate::AutoTrait(trait_def.0.internal(tables, tcx))
            }
        }
    }
}

impl RustcInternal for ExistentialProjection {
    type T<'tcx> = redox_ty::ExistentialProjection<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        use crate::unstable::internal_cx::ExistentialProjectionHelpers;
        tcx.new_from_args(
            self.def_id.0.internal(tables, tcx),
            self.generic_args.internal(tables, tcx),
            self.term.internal(tables, tcx),
        )
    }
}

impl RustcInternal for TermKind {
    type T<'tcx> = redox_ty::Term<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            TermKind::Type(ty) => ty.internal(tables, tcx).into(),
            TermKind::Const(cnst) => cnst.internal(tables, tcx).into(),
        }
    }
}

impl RustcInternal for ExistentialTraitRef {
    type T<'tcx> = redox_ty::ExistentialTraitRef<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        use crate::unstable::internal_cx::ExistentialTraitRefHelpers;
        tcx.new_from_args(
            self.def_id.0.internal(tables, tcx),
            self.generic_args.internal(tables, tcx),
        )
    }
}

impl RustcInternal for TraitRef {
    type T<'tcx> = redox_ty::TraitRef<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        use crate::unstable::internal_cx::TraitRefHelpers;
        tcx.new_from_args(self.def_id.0.internal(tables, tcx), self.args().internal(tables, tcx))
    }
}

impl RustcInternal for AllocId {
    type T<'tcx> = redox_middle::mir::interpret::AllocId;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tcx.lift(tables.alloc_ids[*self]).unwrap()
    }
}

impl RustcInternal for ClosureKind {
    type T<'tcx> = redox_ty::ClosureKind;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            ClosureKind::Fn => redox_ty::ClosureKind::Fn,
            ClosureKind::FnMut => redox_ty::ClosureKind::FnMut,
            ClosureKind::FnOnce => redox_ty::ClosureKind::FnOnce,
        }
    }
}

impl RustcInternal for AdtDef {
    type T<'tcx> = redox_ty::AdtDef<'tcx>;
    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        InternalCx::adt_def(tcx, self.0.internal(tables, tcx))
    }
}

impl RustcInternal for Abi {
    type T<'tcx> = redox_abi::ExternAbi;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match *self {
            Abi::Rust => redox_abi::ExternAbi::Rust,
            Abi::C { unwind } => redox_abi::ExternAbi::C { unwind },
            Abi::Cdecl { unwind } => redox_abi::ExternAbi::Cdecl { unwind },
            Abi::Stdcall { unwind } => redox_abi::ExternAbi::Stdcall { unwind },
            Abi::Fastcall { unwind } => redox_abi::ExternAbi::Fastcall { unwind },
            Abi::Vectorcall { unwind } => redox_abi::ExternAbi::Vectorcall { unwind },
            Abi::Thiscall { unwind } => redox_abi::ExternAbi::Thiscall { unwind },
            Abi::Aapcs { unwind } => redox_abi::ExternAbi::Aapcs { unwind },
            Abi::CCmseNonSecureCall => redox_abi::ExternAbi::CmseNonSecureCall,
            Abi::CCmseNonSecureEntry => redox_abi::ExternAbi::CmseNonSecureEntry,
            Abi::Win64 { unwind } => redox_abi::ExternAbi::Win64 { unwind },
            Abi::SysV64 { unwind } => redox_abi::ExternAbi::SysV64 { unwind },
            Abi::PtxKernel => redox_abi::ExternAbi::PtxKernel,
            Abi::Msp430Interrupt => redox_abi::ExternAbi::Msp430Interrupt,
            Abi::X86Interrupt => redox_abi::ExternAbi::X86Interrupt,
            Abi::GpuKernel => redox_abi::ExternAbi::GpuKernel,
            Abi::EfiApi => redox_abi::ExternAbi::EfiApi,
            Abi::AvrInterrupt => redox_abi::ExternAbi::AvrInterrupt,
            Abi::AvrNonBlockingInterrupt => redox_abi::ExternAbi::AvrNonBlockingInterrupt,
            Abi::System { unwind } => redox_abi::ExternAbi::System { unwind },
            Abi::RustCall => redox_abi::ExternAbi::RustCall,
            Abi::Unadjusted => redox_abi::ExternAbi::Unadjusted,
            Abi::RustCold => redox_abi::ExternAbi::RustCold,
            Abi::RustInvalid => redox_abi::ExternAbi::RustInvalid,
            Abi::RiscvInterruptM => redox_abi::ExternAbi::RiscvInterruptM,
            Abi::RiscvInterruptS => redox_abi::ExternAbi::RiscvInterruptS,
            Abi::RustPreserveNone => redox_abi::ExternAbi::RustPreserveNone,
            Abi::Custom => redox_abi::ExternAbi::Custom,
        }
    }
}

impl RustcInternal for Safety {
    type T<'tcx> = redox_hir::Safety;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            Safety::Unsafe => redox_hir::Safety::Unsafe,
            Safety::Safe => redox_hir::Safety::Safe,
        }
    }
}
impl RustcInternal for Span {
    type T<'tcx> = redox_span::Span;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tables.spans[*self]
    }
}

impl RustcInternal for Layout {
    type T<'tcx> = redox_abi::Layout<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        tcx.lift(tables.layouts[*self]).unwrap()
    }
}

impl RustcInternal for Place {
    type T<'tcx> = redox_middle::mir::Place<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        redox_middle::mir::Place {
            local: redox_middle::mir::Local::from_usize(self.local),
            projection: tcx.mk_place_elems(&self.projection.internal(tables, tcx)),
        }
    }
}

impl RustcInternal for ProjectionElem {
    type T<'tcx> = redox_middle::mir::PlaceElem<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            ProjectionElem::Deref => redox_middle::mir::PlaceElem::Deref,
            ProjectionElem::Field(idx, ty) => {
                redox_middle::mir::PlaceElem::Field((*idx).into(), ty.internal(tables, tcx))
            }
            ProjectionElem::Index(idx) => redox_middle::mir::PlaceElem::Index((*idx).into()),
            ProjectionElem::ConstantIndex { offset, min_length, from_end } => {
                redox_middle::mir::PlaceElem::ConstantIndex {
                    offset: *offset,
                    min_length: *min_length,
                    from_end: *from_end,
                }
            }
            ProjectionElem::Subslice { from, to, from_end } => {
                redox_middle::mir::PlaceElem::Subslice { from: *from, to: *to, from_end: *from_end }
            }
            ProjectionElem::Downcast(idx) => {
                redox_middle::mir::PlaceElem::Downcast(None, idx.internal(tables, tcx))
            }
            ProjectionElem::OpaqueCast(ty) => {
                redox_middle::mir::PlaceElem::OpaqueCast(ty.internal(tables, tcx))
            }
        }
    }
}

impl RustcInternal for BinOp {
    type T<'tcx> = redox_middle::mir::BinOp;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            BinOp::Add => redox_middle::mir::BinOp::Add,
            BinOp::AddUnchecked => redox_middle::mir::BinOp::AddUnchecked,
            BinOp::Sub => redox_middle::mir::BinOp::Sub,
            BinOp::SubUnchecked => redox_middle::mir::BinOp::SubUnchecked,
            BinOp::Mul => redox_middle::mir::BinOp::Mul,
            BinOp::MulUnchecked => redox_middle::mir::BinOp::MulUnchecked,
            BinOp::Div => redox_middle::mir::BinOp::Div,
            BinOp::Rem => redox_middle::mir::BinOp::Rem,
            BinOp::BitXor => redox_middle::mir::BinOp::BitXor,
            BinOp::BitAnd => redox_middle::mir::BinOp::BitAnd,
            BinOp::BitOr => redox_middle::mir::BinOp::BitOr,
            BinOp::Shl => redox_middle::mir::BinOp::Shl,
            BinOp::ShlUnchecked => redox_middle::mir::BinOp::ShlUnchecked,
            BinOp::Shr => redox_middle::mir::BinOp::Shr,
            BinOp::ShrUnchecked => redox_middle::mir::BinOp::ShrUnchecked,
            BinOp::Eq => redox_middle::mir::BinOp::Eq,
            BinOp::Lt => redox_middle::mir::BinOp::Lt,
            BinOp::Le => redox_middle::mir::BinOp::Le,
            BinOp::Ne => redox_middle::mir::BinOp::Ne,
            BinOp::Ge => redox_middle::mir::BinOp::Ge,
            BinOp::Gt => redox_middle::mir::BinOp::Gt,
            BinOp::Cmp => redox_middle::mir::BinOp::Cmp,
            BinOp::Offset => redox_middle::mir::BinOp::Offset,
        }
    }
}

impl RustcInternal for UnOp {
    type T<'tcx> = redox_middle::mir::UnOp;

    fn internal<'tcx>(
        &self,
        _tables: &mut Tables<'_, BridgeTys>,
        _tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        match self {
            UnOp::Not => redox_middle::mir::UnOp::Not,
            UnOp::Neg => redox_middle::mir::UnOp::Neg,
            UnOp::PtrMetadata => redox_middle::mir::UnOp::PtrMetadata,
        }
    }
}

impl<T> RustcInternal for &T
where
    T: RustcInternal,
{
    type T<'tcx> = T::T<'tcx>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        (*self).internal(tables, tcx)
    }
}

impl<T> RustcInternal for Option<T>
where
    T: RustcInternal,
{
    type T<'tcx> = Option<T::T<'tcx>>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        self.as_ref().map(|inner| inner.internal(tables, tcx))
    }
}

impl<T> RustcInternal for Vec<T>
where
    T: RustcInternal,
{
    type T<'tcx> = Vec<T::T<'tcx>>;

    fn internal<'tcx>(
        &self,
        tables: &mut Tables<'_, BridgeTys>,
        tcx: impl InternalCx<'tcx>,
    ) -> Self::T<'tcx> {
        self.iter().map(|e| e.internal(tables, tcx)).collect()
    }
}
