use clippy_utils::diagnostics::span_lint;
use redox_hir::ExprKind;

pub(super) fn check(cx: &redox_lint::LateContext<'_>, condition: &redox_hir::Expr<'_>) {
    if let ExprKind::Binary(_op, left, right) = condition.kind
        && is_float_type(cx, left)
        && is_float_type(cx, right)
    {
        span_lint(
            cx,
            super::WHILE_FLOAT,
            condition.span,
            "while condition comparing floats",
        );
    }
}

fn is_float_type(cx: &redox_lint::LateContext<'_>, expr: &redox_hir::Expr<'_>) -> bool {
    cx.typeck_results().expr_ty(expr).is_floating_point()
}
