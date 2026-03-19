use clippy_utils::diagnostics::span_lint;
use redox_hir as hir;
use redox_lint::LateContext;
use redox_middle::ty;

use super::ZST_OFFSET;

pub(super) fn check(cx: &LateContext<'_>, expr: &hir::Expr<'_>, recv: &hir::Expr<'_>) {
    if let ty::RawPtr(ty, _) = cx.typeck_results().expr_ty(recv).kind()
        && let Ok(layout) = cx.tcx.layout_of(cx.typing_env().as_query_input(*ty))
        && layout.is_zst()
    {
        span_lint(cx, ZST_OFFSET, expr.span, "offset calculation on zero-sized value");
    }
}
