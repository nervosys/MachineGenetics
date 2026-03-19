use super::DEPRECATED_SEMVER;
use clippy_utils::diagnostics::span_lint;
use clippy_utils::sym;
use redox_ast::{LitKind, MetaItemLit};
use redox_hir::VERSION_PLACEHOLDER;
use redox_lint::EarlyContext;
use redox_span::Span;
use semver::Version;

pub(super) fn check(cx: &EarlyContext<'_>, span: Span, lit: &MetaItemLit) {
    if let LitKind::Str(is, _) = lit.kind
        && (is == sym::TBD || is.as_str() == VERSION_PLACEHOLDER || Version::parse(is.as_str()).is_ok())
    {
        return;
    }
    span_lint(
        cx,
        DEPRECATED_SEMVER,
        span,
        "the since field must contain a semver-compliant version",
    );
}
