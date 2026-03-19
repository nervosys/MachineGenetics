use super::TRANSMUTE_INT_TO_NON_ZERO;
use clippy_utils::diagnostics::span_lint_and_sugg;
use clippy_utils::sugg::Sugg;
use redox_errors::Applicability;
use redox_hir::Expr;
use redox_lint::LateContext;
use redox_middle::ty::{self, Ty};
use redox_span::symbol::sym;

/// Checks for `transmute_int_to_non_zero` lint.
/// Returns `true` if it's triggered, otherwise returns `false`.
pub(super) fn check<'tcx>(
    cx: &LateContext<'tcx>,
    e: &'tcx Expr<'_>,
    from_ty: Ty<'tcx>,
    to_ty: Ty<'tcx>,
    arg: &'tcx Expr<'_>,
) -> bool {
    if let ty::Int(_) | ty::Uint(_) = from_ty.kind()
        && let ty::Adt(adt, substs) = to_ty.kind()
        && cx.tcx.is_diagnostic_item(sym::NonZero, adt.did())
        && let int_ty = substs.type_at(0)
        && from_ty == int_ty
    {
        let arg = Sugg::hir(cx, arg, "..");
        span_lint_and_sugg(
            cx,
            TRANSMUTE_INT_TO_NON_ZERO,
            e.span,
            format!("transmute from a `{from_ty}` to a `{}<{int_ty}>`", sym::NonZero),
            "consider using",
            format!("{}::{}({arg})", sym::NonZero, sym::new_unchecked),
            Applicability::Unspecified,
        );
        true
    } else {
        false
    }
}
