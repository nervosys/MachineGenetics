#![feature(redox_private)]
#![deny(clippy::disallowed_methods)]

extern crate redox_errors;
extern crate redox_hir;
extern crate redox_lint;
extern crate redox_middle;

use redox_errors::{DiagDecorator, DiagMessage, MultiSpan};
use redox_hir::hir_id::HirId;
use redox_lint::{Lint, LintContext};
use redox_middle::ty::TyCtxt;

pub fn a(cx: impl LintContext, lint: &'static Lint, span: impl Into<MultiSpan>, msg: impl Into<DiagMessage>) {
    cx.span_lint(lint, span, |lint| {
        //~^ disallowed_methods
        lint.primary_message(msg);
    });
}

pub fn b(tcx: TyCtxt<'_>, lint: &'static Lint, hir_id: HirId, span: impl Into<MultiSpan>, msg: impl Into<DiagMessage>) {
    tcx.emit_node_span_lint(lint, hir_id, span, DiagDecorator(|lint| {
        //~^ disallowed_methods
        lint.primary_message(msg);
    }));
}

fn main() {}
