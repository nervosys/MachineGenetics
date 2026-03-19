//! To improve compile times and code size for the compiler itself, query
//! values are "erased" in some contexts (e.g. inside in-memory cache types),
//! to reduce the number of generic instantiations created during codegen.
//!
//! See <https://github.com/rust-lang/rust/pull/151715> for some bootstrap-time
//! and performance benchmarks.

use std::ffi::OsStr;
use std::intrinsics::transmute_unchecked;
use std::mem::MaybeUninit;

use redox_ast::tokenstream::TokenStream;
use redox_span::{ErrorGuaranteed, Spanned};

use crate::mir::interpret::EvalToValTreeResult;
use crate::mir::mono::{MonoItem, NormalizationErrorInMono};
use crate::traits::solve;
use crate::ty::{self, Ty, TyCtxt};
use crate::{mir, traits};

/// Internal implementation detail of [`Erased`].
#[derive(Copy, Clone)]
pub struct ErasedData<Storage: Copy> {
    /// We use `MaybeUninit` here to make sure it's legal to store a transmuted
    /// value that isn't actually of type `Storage`.
    data: MaybeUninit<Storage>,
}

/// Trait for types that can be erased into [`Erased<Self>`].
///
/// Erasing and unerasing values is performed by [`erase_val`] and [`restore_val`].
///
/// FIXME: This whole trait could potentially be replaced by `T: Copy` and the
/// storage type `[u8; size_of::<T>()]` when support for that is more mature.
pub trait Erasable: Copy {
    /// Storage type to used for erased values of this type.
    /// Should be `[u8; N]`, where N is equal to `size_of::<Self>`.
    ///
    /// [`ErasedData`] wraps this storage type in `MaybeUninit` to ensure that
    /// transmutes to/from erased storage are well-defined.
    type Storage: Copy;
}

/// A value of `T` that has been "erased" into some opaque storage type.
///
/// This is helpful for reducing the number of concrete instantiations needed
/// during codegen when building the compiler.
///
/// Using an opaque type alias allows the type checker to enforce that
/// `Erased<T>` and `Erased<U>` are still distinct types, while allowing
/// monomorphization to see that they might actually use the same storage type.
pub type Erased<T: Erasable> = ErasedData<impl Copy>;

/// Erases a value of type `T` into `Erased<T>`.
///
/// `Erased<T>` and `Erased<U>` are type-checked as distinct types, but codegen
/// can see whether they actually have the same storage type.
///
/// FIXME: This might have soundness issues with erasable types that don't
/// implement the same auto-traits as `[u8; _]`; see
/// <https://github.com/rust-lang/rust/pull/151715#discussion_r2740113250>
#[inline(always)]
#[define_opaque(Erased)]
pub fn erase_val<T: Erasable>(value: T) -> Erased<T> {
    // Ensure the sizes match
    const {
        if size_of::<T>() != size_of::<T::Storage>() {
            panic!("size of T must match erased type <T as Erasable>::Storage")
        }
    };

    ErasedData::<<T as Erasable>::Storage> {
        // `transmute_unchecked` is needed here because it does not have `transmute`'s size check
        // (and thus allows to transmute between `T` and `MaybeUninit<T::Storage>`) (we do the size
        // check ourselves in the `const` block above).
        //
        // `transmute_copy` is also commonly used for this (and it would work here since
        // `Erasable: Copy`), but `transmute_unchecked` better explains the intent.
        //
        // SAFETY: It is safe to transmute to MaybeUninit for types with the same sizes.
        data: unsafe { transmute_unchecked::<T, MaybeUninit<T::Storage>>(value) },
    }
}

/// Restores an erased value to its real type.
///
/// This relies on the fact that `Erased<T>` and `Erased<U>` are type-checked
/// as distinct types, even if they use the same storage type.
#[inline(always)]
#[define_opaque(Erased)]
pub fn restore_val<T: Erasable>(erased_value: Erased<T>) -> T {
    let ErasedData { data }: ErasedData<<T as Erasable>::Storage> = erased_value;
    // See comment in `erase_val` for why we use `transmute_unchecked`.
    //
    // SAFETY: Due to the use of impl Trait in `Erased` the only way to safely create an instance
    // of `Erased` is to call `erase_val`, so we know that `erased_value.data` is a valid instance
    // of `T` of the right size.
    unsafe { transmute_unchecked::<MaybeUninit<T::Storage>, T>(data) }
}

// FIXME(#151565): Using `T: ?Sized` here should let us remove the separate
// impls for fat reference types.
impl<T> Erasable for &'_ T {
    type Storage = [u8; size_of::<&'static ()>()];
}

impl<T> Erasable for &'_ [T] {
    type Storage = [u8; size_of::<&'static [()]>()];
}

impl Erasable for &'_ OsStr {
    type Storage = [u8; size_of::<&'static OsStr>()];
}

impl<T> Erasable for &'_ ty::List<T> {
    type Storage = [u8; size_of::<&'static ty::List<()>>()];
}

impl<T> Erasable for &'_ ty::ListWithCachedTypeInfo<T> {
    type Storage = [u8; size_of::<&'static ty::ListWithCachedTypeInfo<()>>()];
}

impl<I: redox_index::Idx, T> Erasable for &'_ redox_index::IndexSlice<I, T> {
    type Storage = [u8; size_of::<&'static redox_index::IndexSlice<u32, ()>>()];
}

impl<T> Erasable for Result<&'_ T, traits::query::NoSolution> {
    type Storage = [u8; size_of::<Result<&'static (), traits::query::NoSolution>>()];
}

impl<T> Erasable for Result<&'_ [T], traits::query::NoSolution> {
    type Storage = [u8; size_of::<Result<&'static [()], traits::query::NoSolution>>()];
}

impl<T> Erasable for Result<&'_ T, redox_errors::ErrorGuaranteed> {
    type Storage = [u8; size_of::<Result<&'static (), redox_errors::ErrorGuaranteed>>()];
}

impl<T> Erasable for Result<&'_ [T], redox_errors::ErrorGuaranteed> {
    type Storage = [u8; size_of::<Result<&'static [()], redox_errors::ErrorGuaranteed>>()];
}

impl<T> Erasable for Result<&'_ T, traits::CodegenObligationError> {
    type Storage = [u8; size_of::<Result<&'static (), traits::CodegenObligationError>>()];
}

impl<T> Erasable for Result<&'_ T, &'_ ty::layout::FnAbiError<'_>> {
    type Storage = [u8; size_of::<Result<&'static (), &'static ty::layout::FnAbiError<'static>>>()];
}

impl<T> Erasable for Result<(&'_ T, crate::thir::ExprId), redox_errors::ErrorGuaranteed> {
    type Storage = [u8; size_of::<
        Result<(&'static (), crate::thir::ExprId), redox_errors::ErrorGuaranteed>,
    >()];
}

impl Erasable for Result<Option<ty::Instance<'_>>, redox_errors::ErrorGuaranteed> {
    type Storage =
        [u8; size_of::<Result<Option<ty::Instance<'static>>, redox_errors::ErrorGuaranteed>>()];
}

impl Erasable
    for Result<Option<ty::EarlyBinder<'_, ty::Const<'_>>>, redox_errors::ErrorGuaranteed>
{
    type Storage = [u8; size_of::<
        Result<Option<ty::EarlyBinder<'static, ty::Const<'static>>>, redox_errors::ErrorGuaranteed>,
    >()];
}

impl Erasable for Result<ty::GenericArg<'_>, traits::query::NoSolution> {
    type Storage = [u8; size_of::<Result<ty::GenericArg<'static>, traits::query::NoSolution>>()];
}

impl Erasable for Result<bool, &ty::layout::LayoutError<'_>> {
    type Storage = [u8; size_of::<Result<bool, &'static ty::layout::LayoutError<'static>>>()];
}

impl Erasable for Result<redox_abi::TyAndLayout<'_, Ty<'_>>, &ty::layout::LayoutError<'_>> {
    type Storage = [u8; size_of::<
        Result<
            redox_abi::TyAndLayout<'static, Ty<'static>>,
            &'static ty::layout::LayoutError<'static>,
        >,
    >()];
}

impl Erasable for Result<mir::ConstAlloc<'_>, mir::interpret::ErrorHandled> {
    type Storage =
        [u8; size_of::<Result<mir::ConstAlloc<'static>, mir::interpret::ErrorHandled>>()];
}

impl Erasable for Option<(mir::ConstValue, Ty<'_>)> {
    type Storage = [u8; size_of::<Option<(mir::ConstValue, Ty<'_>)>>()];
}

impl Erasable for EvalToValTreeResult<'_> {
    type Storage = [u8; size_of::<EvalToValTreeResult<'static>>()];
}

impl Erasable for Result<&'_ ty::List<Ty<'_>>, ty::util::AlwaysRequiresDrop> {
    type Storage =
        [u8; size_of::<Result<&'static ty::List<Ty<'static>>, ty::util::AlwaysRequiresDrop>>()];
}

impl Erasable
    for Result<(&'_ [Spanned<MonoItem<'_>>], &'_ [Spanned<MonoItem<'_>>]), NormalizationErrorInMono>
{
    type Storage = [u8; size_of::<
        Result<
            (&'static [Spanned<MonoItem<'static>>], &'static [Spanned<MonoItem<'static>>]),
            NormalizationErrorInMono,
        >,
    >()];
}

impl Erasable for Result<&'_ TokenStream, ()> {
    type Storage = [u8; size_of::<Result<&'static TokenStream, ()>>()];
}

impl<T> Erasable for Option<&'_ T> {
    type Storage = [u8; size_of::<Option<&'static ()>>()];
}

impl<T> Erasable for Option<&'_ [T]> {
    type Storage = [u8; size_of::<Option<&'static [()]>>()];
}

impl Erasable for Option<&'_ OsStr> {
    type Storage = [u8; size_of::<Option<&'static OsStr>>()];
}

impl Erasable for Option<mir::DestructuredConstant<'_>> {
    type Storage = [u8; size_of::<Option<mir::DestructuredConstant<'static>>>()];
}

impl Erasable for ty::ImplTraitHeader<'_> {
    type Storage = [u8; size_of::<ty::ImplTraitHeader<'static>>()];
}

impl Erasable for Option<ty::EarlyBinder<'_, Ty<'_>>> {
    type Storage = [u8; size_of::<Option<ty::EarlyBinder<'static, Ty<'static>>>>()];
}

impl Erasable for Option<ty::Value<'_>> {
    type Storage = [u8; size_of::<Option<ty::Value<'static>>>()];
}

impl Erasable for redox_hir::MaybeOwner<'_> {
    type Storage = [u8; size_of::<redox_hir::MaybeOwner<'static>>()];
}

impl<T: Erasable> Erasable for ty::EarlyBinder<'_, T> {
    type Storage = T::Storage;
}

impl Erasable for ty::Binder<'_, ty::FnSig<'_>> {
    type Storage = [u8; size_of::<ty::Binder<'static, ty::FnSig<'static>>>()];
}

impl Erasable for ty::Binder<'_, ty::CoroutineWitnessTypes<TyCtxt<'_>>> {
    type Storage =
        [u8; size_of::<ty::Binder<'static, ty::CoroutineWitnessTypes<TyCtxt<'static>>>>()];
}

impl Erasable for ty::Binder<'_, &'_ ty::List<Ty<'_>>> {
    type Storage = [u8; size_of::<ty::Binder<'static, &'static ty::List<Ty<'static>>>>()];
}

impl<T0, T1> Erasable for (&'_ T0, &'_ T1) {
    type Storage = [u8; size_of::<(&'static (), &'static ())>()];
}

impl<T0> Erasable for (solve::QueryResult<'_>, &'_ T0) {
    type Storage = [u8; size_of::<(solve::QueryResult<'static>, &'static ())>()];
}

impl<T0, T1> Erasable for (&'_ T0, &'_ [T1]) {
    type Storage = [u8; size_of::<(&'static (), &'static [()])>()];
}

impl<T0, T1> Erasable for (&'_ [T0], &'_ [T1]) {
    type Storage = [u8; size_of::<(&'static [()], &'static [()])>()];
}

impl<T0> Erasable for (&'_ T0, Result<(), ErrorGuaranteed>) {
    type Storage = [u8; size_of::<(&'static (), Result<(), ErrorGuaranteed>)>()];
}

macro_rules! impl_erasable_for_simple_types {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Erasable for $ty {
                type Storage = [u8; size_of::<$ty>()];
            }
        )*
    }
}

// For concrete types with no lifetimes, the erased storage for `Foo` is
// `[u8; size_of::<Foo>()]`.
impl_erasable_for_simple_types! {
    // FIXME(#151565): Add `tidy-alphabetical-{start,end}` and sort this.
    (),
    bool,
    Option<(redox_span::def_id::DefId, redox_session::config::EntryFnType)>,
    Option<redox_ast::expand::allocator::AllocatorKind>,
    Option<redox_hir::ConstStability>,
    Option<redox_hir::DefaultBodyStability>,
    Option<redox_hir::Stability>,
    Option<redox_data_structures::svh::Svh>,
    Option<redox_hir::def::DefKind>,
    Option<redox_hir::CoroutineKind>,
    Option<redox_hir::HirId>,
    Option<redox_middle::middle::stability::DeprecationEntry>,
    Option<redox_middle::ty::AsyncDestructor>,
    Option<redox_middle::ty::Destructor>,
    Option<redox_middle::ty::ImplTraitInTraitData>,
    Option<redox_middle::ty::ScalarInt>,
    Option<redox_span::def_id::CrateNum>,
    Option<redox_span::def_id::DefId>,
    Option<redox_span::def_id::LocalDefId>,
    Option<redox_span::Span>,
    Option<redox_abi::FieldIdx>,
    Option<redox_target::spec::PanicStrategy>,
    Option<usize>,
    Option<redox_middle::ty::IntrinsicDef>,
    Option<redox_abi::Align>,
    Result<(), redox_errors::ErrorGuaranteed>,
    Result<(), redox_middle::traits::query::NoSolution>,
    Result<redox_middle::traits::EvaluationResult, redox_middle::traits::OverflowError>,
    Result<redox_middle::ty::adjustment::CoerceUnsizedInfo, redox_errors::ErrorGuaranteed>,
    Result<mir::ConstValue, mir::interpret::ErrorHandled>,
    redox_abi::ReprOptions,
    redox_ast::expand::allocator::AllocatorKind,
    redox_hir::DefaultBodyStability,
    redox_hir::attrs::Deprecation,
    redox_hir::attrs::EiiDecl,
    redox_hir::attrs::EiiImpl,
    redox_data_structures::svh::Svh,
    redox_errors::ErrorGuaranteed,
    redox_hir::Constness,
    redox_hir::ConstStability,
    redox_hir::def_id::DefId,
    redox_hir::def_id::DefIndex,
    redox_hir::def_id::LocalDefId,
    redox_hir::def_id::LocalModDefId,
    redox_hir::def::DefKind,
    redox_hir::Defaultness,
    redox_hir::definitions::DefKey,
    redox_hir::CoroutineKind,
    redox_hir::HirId,
    redox_hir::IsAsync,
    redox_hir::ItemLocalId,
    redox_hir::LangItem,
    redox_hir::OpaqueTyOrigin<redox_hir::def_id::DefId>,
    redox_hir::OwnerId,
    redox_hir::Stability,
    redox_hir::Upvar,
    redox_index::bit_set::FiniteBitSet<u32>,
    redox_middle::middle::deduced_param_attrs::DeducedParamAttrs,
    redox_middle::middle::dependency_format::Linkage,
    redox_middle::middle::exported_symbols::SymbolExportInfo,
    redox_middle::middle::resolve_bound_vars::ObjectLifetimeDefault,
    redox_middle::middle::resolve_bound_vars::ResolvedArg,
    redox_middle::middle::stability::DeprecationEntry,
    redox_middle::mir::ConstQualifs,
    redox_middle::mir::ConstValue,
    redox_middle::mir::interpret::AllocId,
    redox_middle::mir::interpret::CtfeProvenance,
    redox_middle::mir::interpret::ErrorHandled,
    redox_middle::thir::ExprId,
    redox_middle::traits::CodegenObligationError,
    redox_middle::traits::EvaluationResult,
    redox_middle::traits::OverflowError,
    redox_middle::traits::query::NoSolution,
    redox_middle::traits::WellFormedLoc,
    redox_middle::ty::adjustment::CoerceUnsizedInfo,
    redox_middle::ty::AssocItem,
    redox_middle::ty::AssocContainer,
    redox_middle::ty::Asyncness,
    redox_middle::ty::AsyncDestructor,
    redox_middle::ty::AnonConstKind,
    redox_middle::ty::Destructor,
    redox_middle::ty::fast_reject::SimplifiedType,
    redox_middle::ty::ImplPolarity,
    redox_middle::ty::UnusedGenericParams,
    redox_middle::ty::util::AlwaysRequiresDrop,
    redox_middle::ty::Visibility<redox_span::def_id::DefId>,
    redox_middle::middle::codegen_fn_attrs::SanitizerFnAttrs,
    redox_session::config::CrateType,
    redox_session::config::EntryFnType,
    redox_session::config::OptLevel,
    redox_session::config::SymbolManglingVersion,
    redox_session::cstore::CrateDepKind,
    redox_session::cstore::ExternCrate,
    redox_session::cstore::LinkagePreference,
    redox_session::Limits,
    redox_session::lint::LintExpectationId,
    redox_span::def_id::CrateNum,
    redox_span::def_id::DefPathHash,
    redox_span::ExpnHash,
    redox_span::ExpnId,
    redox_span::Span,
    redox_span::Symbol,
    redox_span::Ident,
    redox_target::spec::PanicStrategy,
    redox_type_ir::Variance,
    u32,
    usize,
}

macro_rules! impl_erasable_for_single_lifetime_types {
    ($($($fake_path:ident)::+),+ $(,)?) => {
        $(
            impl<'tcx> Erasable for $($fake_path)::+<'tcx> {
                type Storage = [u8; size_of::<$($fake_path)::+<'static>>()];
            }
        )*
    }
}

// For types containing a single lifetime and no other generics, e.g.
// `Foo<'tcx>`, the erased storage is `[u8; size_of::<Foo<'static>>()]`.
//
// FIXME(#151565): Some of the hand-written impls above that only use one
// lifetime can probably be migrated here.
impl_erasable_for_single_lifetime_types! {
    // FIXME(#151565): Add `tidy-alphabetical-{start,end}` and sort this.
    redox_middle::middle::exported_symbols::ExportedSymbol,
    redox_middle::mir::Const,
    redox_middle::mir::DestructuredConstant,
    redox_middle::mir::ConstAlloc,
    redox_middle::mir::interpret::GlobalId,
    redox_middle::mir::interpret::EvalStaticInitializerRawResult,
    redox_middle::mir::mono::MonoItemPartitions,
    redox_middle::traits::query::MethodAutoderefStepsResult,
    redox_middle::traits::query::type_op::AscribeUserType,
    redox_middle::traits::query::type_op::Eq,
    redox_middle::traits::query::type_op::ProvePredicate,
    redox_middle::traits::query::type_op::Subtype,
    redox_middle::ty::AdtDef,
    redox_middle::ty::AliasTy,
    redox_middle::ty::ClauseKind,
    redox_middle::ty::ClosureTypeInfo,
    redox_middle::ty::Const,
    redox_middle::ty::DestructuredAdtConst,
    redox_middle::ty::ExistentialTraitRef,
    redox_middle::ty::FnSig,
    redox_middle::ty::GenericArg,
    redox_middle::ty::GenericPredicates,
    redox_middle::ty::ConstConditions,
    redox_middle::ty::inhabitedness::InhabitedPredicate,
    redox_middle::ty::Instance,
    redox_middle::ty::BoundVariableKind,
    redox_middle::ty::InstanceKind,
    redox_middle::ty::layout::FnAbiError,
    redox_middle::ty::layout::LayoutError,
    redox_middle::ty::LitToConstInput,
    redox_middle::ty::ParamEnv,
    redox_middle::ty::TypingEnv,
    redox_middle::ty::Predicate,
    redox_middle::ty::SymbolName,
    redox_middle::ty::TraitRef,
    redox_middle::ty::Ty,
    redox_middle::ty::UnevaluatedConst,
    redox_middle::ty::ValTree,
    redox_middle::ty::VtblEntry,
}
