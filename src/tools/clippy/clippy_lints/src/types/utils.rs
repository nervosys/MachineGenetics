use clippy_utils::last_path_segment;
use redox_hir::{GenericArg, GenericArgsParentheses, QPath, TyKind};
use redox_lint::LateContext;
use redox_span::Span;

pub(super) fn match_borrows_parameter(_cx: &LateContext<'_>, qpath: &QPath<'_>) -> Option<Span> {
    let last = last_path_segment(qpath);
    if let Some(params) = last.args
        && params.parenthesized == GenericArgsParentheses::No
        && let Some(ty) = params.args.iter().find_map(|arg| match arg {
            GenericArg::Type(ty) => Some(ty),
            _ => None,
        })
        && let TyKind::Ref(..) = ty.kind
    {
        return Some(ty.span);
    }
    None
}
