use super::INLINE_ALWAYS;
use clippy_utils::diagnostics::span_lint;
use redox_hir::attrs::InlineAttr;
use redox_hir::{Attribute, find_attr};
use redox_lint::LateContext;
use redox_span::Span;
use redox_span::symbol::Symbol;

pub(super) fn check(cx: &LateContext<'_>, span: Span, name: Symbol, attrs: &[Attribute]) {
    if span.from_expansion() {
        return;
    }

    if let Some(span) = find_attr!(attrs, Inline(InlineAttr::Always, span) => *span) {
        span_lint(
            cx,
            INLINE_ALWAYS,
            span,
            format!("you have declared `#[inline(always)]` on `{name}`. This is usually a bad idea"),
        );
    }
}
