use clippy_utils::diagnostics::span_lint_and_sugg;
use redox_ast::ast::{Pat, PatKind};
use redox_errors::Applicability;
use redox_lint::EarlyContext;

use super::REDUNDANT_PATTERN;

pub(super) fn check(cx: &EarlyContext<'_>, pat: &Pat) {
    if let PatKind::Ident(ann, ident, Some(ref right)) = pat.kind
        && let PatKind::Wild = right.kind
    {
        span_lint_and_sugg(
            cx,
            REDUNDANT_PATTERN,
            pat.span,
            format!(
                "the `{} @ _` pattern can be written as just `{}`",
                ident.name, ident.name,
            ),
            "try",
            format!("{}{}", ann.prefix_str(), ident.name),
            Applicability::MachineApplicable,
        );
    }
}
