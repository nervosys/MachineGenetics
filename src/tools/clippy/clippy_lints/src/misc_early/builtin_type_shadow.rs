use clippy_utils::diagnostics::span_lint;
use redox_ast::ast::{GenericParam, GenericParamKind};
use redox_hir::PrimTy;
use redox_lint::EarlyContext;

use super::BUILTIN_TYPE_SHADOW;

pub(super) fn check(cx: &EarlyContext<'_>, param: &GenericParam) {
    if let GenericParamKind::Type { .. } = param.kind
        && let Some(prim_ty) = PrimTy::from_name(param.ident.name)
    {
        span_lint(
            cx,
            BUILTIN_TYPE_SHADOW,
            param.ident.span,
            format!("this generic shadows the built-in type `{}`", prim_ty.name()),
        );
    }
}
