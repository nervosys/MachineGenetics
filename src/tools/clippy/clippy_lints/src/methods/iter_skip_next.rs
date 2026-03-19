use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::res::{MaybeDef, MaybeResPath, MaybeTypeckRes};
use clippy_utils::source::snippet;
use redox_errors::Applicability;
use redox_hir as hir;
use redox_hir::{BindingMode, Node, PatKind};
use redox_lint::LateContext;
use redox_span::sym;

use super::ITER_SKIP_NEXT;

pub(super) fn check(cx: &LateContext<'_>, expr: &hir::Expr<'_>, recv: &hir::Expr<'_>, arg: &hir::Expr<'_>) {
    // lint if caller of skip is an Iterator
    if cx.ty_based_def(expr).opt_parent(cx).is_diag_item(cx, sym::Iterator) {
        let mut applicability = Applicability::MachineApplicable;
        span_lint_and_then(
            cx,
            ITER_SKIP_NEXT,
            expr.span.trim_start(recv.span).unwrap(),
            "called `skip(..).next()` on an iterator",
            |diag| {
                if let Some(id) = recv.res_local_id()
                    && let Node::Pat(pat) = cx.tcx.hir_node(id)
                    && let PatKind::Binding(ann, _, _, _) = pat.kind
                    && ann != BindingMode::MUT
                {
                    applicability = Applicability::Unspecified;
                    diag.span_help(
                        pat.span,
                        format!("for this change `{}` has to be mutable", snippet(cx, pat.span, "..")),
                    );
                }

                diag.span_suggestion(
                    expr.span.trim_start(recv.span).unwrap(),
                    "use `nth` instead",
                    format!(".nth({})", snippet(cx, arg.span, "..")),
                    applicability,
                );
            },
        );
    }
}
