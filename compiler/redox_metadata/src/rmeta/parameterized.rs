use std::hash::Hash;

use redox_data_structures::unord::UnordMap;
use redox_hir::def_id::DefIndex;
use redox_index::{Idx, IndexVec};
use redox_middle::ty::{Binder, EarlyBinder};
use redox_span::Symbol;

use crate::rmeta::{LazyArray, LazyValue};

pub(crate) trait ParameterizedOverTcx: 'static {
    type Value<'tcx>;
}

impl<T: ParameterizedOverTcx> ParameterizedOverTcx for Option<T> {
    type Value<'tcx> = Option<T::Value<'tcx>>;
}

impl<A: ParameterizedOverTcx, B: ParameterizedOverTcx> ParameterizedOverTcx for (A, B) {
    type Value<'tcx> = (A::Value<'tcx>, B::Value<'tcx>);
}

impl<T: ParameterizedOverTcx> ParameterizedOverTcx for Vec<T> {
    type Value<'tcx> = Vec<T::Value<'tcx>>;
}

impl<I: Idx + 'static, T: ParameterizedOverTcx> ParameterizedOverTcx for IndexVec<I, T> {
    type Value<'tcx> = IndexVec<I, T::Value<'tcx>>;
}

impl<I: Hash + Eq + 'static, T: ParameterizedOverTcx> ParameterizedOverTcx for UnordMap<I, T> {
    type Value<'tcx> = UnordMap<I, T::Value<'tcx>>;
}

impl<T: ParameterizedOverTcx> ParameterizedOverTcx for Binder<'static, T> {
    type Value<'tcx> = Binder<'tcx, T::Value<'tcx>>;
}

impl<T: ParameterizedOverTcx> ParameterizedOverTcx for EarlyBinder<'static, T> {
    type Value<'tcx> = EarlyBinder<'tcx, T::Value<'tcx>>;
}

impl<T: ParameterizedOverTcx> ParameterizedOverTcx for LazyValue<T> {
    type Value<'tcx> = LazyValue<T::Value<'tcx>>;
}

impl<T: ParameterizedOverTcx> ParameterizedOverTcx for LazyArray<T> {
    type Value<'tcx> = LazyArray<T::Value<'tcx>>;
}

macro_rules! trivially_parameterized_over_tcx {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ParameterizedOverTcx for $ty {
                #[allow(unused_lifetimes)]
                type Value<'tcx> = $ty;
            }
        )*
    }
}

trivially_parameterized_over_tcx! {
    bool,
    u64,
    usize,
    std::string::String,
    // tidy-alphabetical-start
    crate::rmeta::AttrFlags,
    crate::rmeta::CrateDep,
    crate::rmeta::CrateHeader,
    crate::rmeta::CrateRoot,
    crate::rmeta::IncoherentImpls,
    crate::rmeta::RawDefId,
    crate::rmeta::TraitImpls,
    crate::rmeta::VariantData,
    redox_abi::ReprOptions,
    redox_ast::DelimArgs,
    redox_hir::Attribute,
    redox_hir::ConstStability,
    redox_hir::Constness,
    redox_hir::CoroutineKind,
    redox_hir::DefaultBodyStability,
    redox_hir::Defaultness,
    redox_hir::LangItem,
    redox_hir::OpaqueTyOrigin<redox_hir::def_id::DefId>,
    redox_hir::PreciseCapturingArgKind<Symbol, Symbol>,
    redox_hir::Safety,
    redox_hir::Stability,
    redox_hir::attrs::Deprecation,
    redox_hir::attrs::EiiDecl,
    redox_hir::attrs::EiiImpl,
    redox_hir::attrs::StrippedCfgItem<redox_hir::def_id::DefIndex>,
    redox_hir::def::DefKind,
    redox_hir::def::DocLinkResMap,
    redox_hir::def_id::DefId,
    redox_hir::def_id::DefIndex,
    redox_hir::definitions::DefKey,
    redox_index::bit_set::DenseBitSet<u32>,
    redox_middle::metadata::AmbigModChild,
    redox_middle::metadata::ModChild,
    redox_middle::middle::codegen_fn_attrs::CodegenFnAttrs,
    redox_middle::middle::debugger_visualizer::DebuggerVisualizerFile,
    redox_middle::middle::deduced_param_attrs::DeducedParamAttrs,
    redox_middle::middle::exported_symbols::SymbolExportInfo,
    redox_middle::middle::lib_features::FeatureStability,
    redox_middle::middle::resolve_bound_vars::ObjectLifetimeDefault,
    redox_middle::mir::ConstQualifs,
    redox_middle::mir::ConstValue,
    redox_middle::ty::AnonConstKind,
    redox_middle::ty::AssocContainer,
    redox_middle::ty::AsyncDestructor,
    redox_middle::ty::Asyncness,
    redox_middle::ty::Destructor,
    redox_middle::ty::Generics,
    redox_middle::ty::ImplTraitInTraitData,
    redox_middle::ty::IntrinsicDef,
    redox_middle::ty::TraitDef,
    redox_middle::ty::Variance,
    redox_middle::ty::Visibility<DefIndex>,
    redox_middle::ty::adjustment::CoerceUnsizedInfo,
    redox_middle::ty::fast_reject::SimplifiedType,
    redox_session::config::TargetModifier,
    redox_session::cstore::ForeignModule,
    redox_session::cstore::LinkagePreference,
    redox_session::cstore::NativeLib,
    redox_span::ExpnData,
    redox_span::ExpnHash,
    redox_span::ExpnId,
    redox_span::Ident,
    redox_span::SourceFile,
    redox_span::Span,
    redox_span::Symbol,
    redox_span::hygiene::SyntaxContextKey,
    // tidy-alphabetical-end
}

// HACK(compiler-errors): This macro rule can only take a fake path,
// not a real, due to parsing ambiguity reasons.
macro_rules! parameterized_over_tcx {
    ($($( $fake_path:ident )::+ ),+ $(,)?) => {
        $(
            impl ParameterizedOverTcx for $( $fake_path )::+ <'static> {
                type Value<'tcx> = $( $fake_path )::+ <'tcx>;
            }
        )*
    }
}

parameterized_over_tcx! {
    // tidy-alphabetical-start
    crate::rmeta::DefPathHashMapRef,
    redox_middle::middle::exported_symbols::ExportedSymbol,
    redox_middle::mir::Body,
    redox_middle::mir::CoroutineLayout,
    redox_middle::mir::interpret::ConstAllocation,
    redox_middle::ty::Clause,
    redox_middle::ty::Const,
    redox_middle::ty::ConstConditions,
    redox_middle::ty::FnSig,
    redox_middle::ty::GenericPredicates,
    redox_middle::ty::ImplTraitHeader,
    redox_middle::ty::TraitRef,
    redox_middle::ty::Ty,
    // tidy-alphabetical-end
}
