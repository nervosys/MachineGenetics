use super::ALLOW_ATTRIBUTES;
use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::is_from_proc_macro;
use redox_ast::attr::AttributeExt;
use redox_ast::{AttrStyle, Attribute};
use redox_errors::Applicability;
use redox_lint::{EarlyContext, LintContext};

// Separate each crate's features.
pub fn check<'cx>(cx: &EarlyContext<'cx>, attr: &'cx Attribute) {
    if !attr.span.in_external_macro(cx.sess().source_map())
        && let AttrStyle::Outer = attr.style
        && let Some(path_span) = attr.path_span()
        && !is_from_proc_macro(cx, attr)
    {
        #[expect(clippy::collapsible_span_lint_calls, reason = "rust-clippy#7797")]
        span_lint_and_then(cx, ALLOW_ATTRIBUTES, path_span, "#[allow] attribute found", |diag| {
            diag.span_suggestion(path_span, "replace it with", "expect", Applicability::MachineApplicable);
        });
    }
}
