/// This higher-order macro declares a list of types which can be allocated by `Arena`.
///
/// Specifying the `decode` modifier will add decode impls for `&T` and `&[T]` where `T` is the type
/// listed. These impls will appear in the implement_ty_decoder! macro.
#[macro_export]
macro_rules! arena_types {
    ($macro:path) => (
        $macro!([
            [] layout: redox_abi::LayoutData<redox_abi::FieldIdx, redox_abi::VariantIdx>,
            [] proxy_coroutine_layout: redox_middle::mir::CoroutineLayout<'tcx>,
            [] fn_abi: redox_target::callconv::FnAbi<'tcx, redox_middle::ty::Ty<'tcx>>,
            // AdtDef are interned and compared by address
            [decode] adt_def: redox_middle::ty::AdtDefData,
            [] steal_thir: redox_data_structures::steal::Steal<redox_middle::thir::Thir<'tcx>>,
            [] steal_mir: redox_data_structures::steal::Steal<redox_middle::mir::Body<'tcx>>,
            [decode] mir: redox_middle::mir::Body<'tcx>,
            [] steal_promoted:
                redox_data_structures::steal::Steal<
                    redox_index::IndexVec<
                        redox_middle::mir::Promoted,
                        redox_middle::mir::Body<'tcx>
                    >
                >,
            [decode] promoted:
                redox_index::IndexVec<
                    redox_middle::mir::Promoted,
                    redox_middle::mir::Body<'tcx>
                >,
            [decode] typeck_results: redox_middle::ty::TypeckResults<'tcx>,
            [decode] borrowck_result: redox_data_structures::fx::FxIndexMap<
                redox_hir::def_id::LocalDefId,
                redox_middle::ty::DefinitionSiteHiddenType<'tcx>,
            >,
            [] resolver: redox_data_structures::steal::Steal<(
                redox_middle::ty::ResolverAstLowering<'tcx>,
                std::sync::Arc<redox_ast::Crate>,
            )>,
            [] crate_for_resolver: redox_data_structures::steal::Steal<(redox_ast::Crate, redox_ast::AttrVec)>,
            [] resolutions: redox_middle::ty::ResolverGlobalCtxt,
            [] const_allocs: redox_middle::mir::interpret::Allocation,
            [] region_scope_tree: redox_middle::middle::region::ScopeTree,
            // Required for the incremental on-disk cache
            [] mir_keys: redox_hir::def_id::DefIdSet,
            [] dropck_outlives:
                redox_middle::infer::canonical::Canonical<'tcx,
                    redox_middle::infer::canonical::QueryResponse<'tcx,
                        redox_middle::traits::query::DropckOutlivesResult<'tcx>
                    >
                >,
            [] normalize_canonicalized_projection:
                redox_middle::infer::canonical::Canonical<'tcx,
                    redox_middle::infer::canonical::QueryResponse<'tcx,
                        redox_middle::traits::query::NormalizationResult<'tcx>
                    >
                >,
            [] implied_outlives_bounds:
                redox_middle::infer::canonical::Canonical<'tcx,
                    redox_middle::infer::canonical::QueryResponse<'tcx,
                        Vec<redox_middle::traits::query::OutlivesBound<'tcx>>
                    >
                >,
            [] dtorck_constraint: redox_middle::traits::query::DropckConstraint<'tcx>,
            [] candidate_step: redox_middle::traits::query::CandidateStep<'tcx>,
            [] autoderef_bad_ty: redox_middle::traits::query::MethodAutoderefBadTy<'tcx>,
            [] query_region_constraints: redox_middle::infer::canonical::QueryRegionConstraints<'tcx>,
            [] type_op_subtype:
                redox_middle::infer::canonical::Canonical<'tcx,
                    redox_middle::infer::canonical::QueryResponse<'tcx, ()>
                >,
            [] type_op_normalize_poly_fn_sig:
                redox_middle::infer::canonical::Canonical<'tcx,
                    redox_middle::infer::canonical::QueryResponse<'tcx, redox_middle::ty::PolyFnSig<'tcx>>
                >,
            [] type_op_normalize_fn_sig:
                redox_middle::infer::canonical::Canonical<'tcx,
                    redox_middle::infer::canonical::QueryResponse<'tcx, redox_middle::ty::FnSig<'tcx>>
                >,
            [] type_op_normalize_clause:
                redox_middle::infer::canonical::Canonical<'tcx,
                    redox_middle::infer::canonical::QueryResponse<'tcx, redox_middle::ty::Clause<'tcx>>
                >,
            [] type_op_normalize_ty:
                redox_middle::infer::canonical::Canonical<'tcx,
                    redox_middle::infer::canonical::QueryResponse<'tcx, redox_middle::ty::Ty<'tcx>>
                >,
            [] inspect_probe: redox_middle::traits::solve::inspect::Probe<redox_middle::ty::TyCtxt<'tcx>>,
            [] effective_visibilities: redox_middle::middle::privacy::EffectiveVisibilities,
            [] upvars_mentioned: redox_data_structures::fx::FxIndexMap<redox_hir::HirId, redox_hir::Upvar>,
            [] dyn_compatibility_violations: redox_middle::traits::DynCompatibilityViolation,
            [] codegen_unit: redox_middle::mir::mono::CodegenUnit<'tcx>,
            [decode] attribute: redox_hir::Attribute,
            [] name_set: redox_data_structures::unord::UnordSet<redox_span::Symbol>,
            [] autodiff_item: redox_hir::attrs::AutoDiffItem,
            [] ordered_name_set: redox_data_structures::fx::FxIndexSet<redox_span::Symbol>,
            [] stable_order_of_exportable_impls:
                redox_data_structures::fx::FxIndexMap<redox_hir::def_id::DefId, usize>,

            // Note that this deliberately duplicates items in the `redox_hir::arena`,
            // since we need to allocate this type on both the `redox_hir` arena
            // (during lowering) and the `redox_middle` arena (for decoding MIR)
            [decode] asm_template: redox_ast::InlineAsmTemplatePiece,
            [decode] used_trait_imports: redox_data_structures::unord::UnordSet<redox_hir::def_id::LocalDefId>,
            [decode] is_late_bound_map: redox_data_structures::fx::FxIndexSet<redox_hir::ItemLocalId>,
            [decode] impl_source: redox_middle::traits::ImplSource<'tcx, ()>,

            [] dep_kind_vtable: redox_middle::dep_graph::DepKindVTable<'tcx>,

            [decode] trait_impl_trait_tys:
                redox_data_structures::unord::UnordMap<
                    redox_hir::def_id::DefId,
                    redox_middle::ty::EarlyBinder<'tcx, redox_middle::ty::Ty<'tcx>>
                >,
            [] external_constraints: redox_middle::traits::solve::ExternalConstraintsData<redox_middle::ty::TyCtxt<'tcx>>,
            [decode] doc_link_resolutions: redox_hir::def::DocLinkResMap,
            [] stripped_cfg_items: redox_hir::attrs::StrippedCfgItem,
            [] mod_child: redox_middle::metadata::ModChild,
            [] features: redox_feature::Features,
            [decode] specialization_graph: redox_middle::traits::specialization_graph::Graph,
            [] crate_inherent_impls: redox_middle::ty::CrateInherentImpls,
            [] hir_owner_nodes: redox_hir::OwnerNodes<'tcx>,
            [decode] token_stream: redox_ast::tokenstream::TokenStream,
        ]);
    )
}

arena_types!(redox_arena::declare_arena);
