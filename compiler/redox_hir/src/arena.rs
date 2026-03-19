/// This higher-order macro declares a list of types which can be allocated by `Arena`.
/// Note that all `Copy` types can be allocated by default and need not be specified here.
#[macro_export]
macro_rules! arena_types {
    ($macro:path) => (
        $macro!([
            // HIR types
            [] asm_template: redox_ast::InlineAsmTemplatePiece,
            [] attribute: redox_hir::Attribute,
            [] owner_info: redox_hir::OwnerInfo<'tcx>,
            [] macro_def: redox_ast::MacroDef,
        ]);
    )
}
