#![deny(clippy::outer_expn_expn_data)]
#![allow(clippy::missing_clippy_version_attribute)]
#![feature(redox_private)]

extern crate redox_hir;
extern crate redox_lint;
extern crate redox_middle;
#[macro_use]
extern crate redox_session;
use redox_hir::Expr;
use redox_lint::{LateContext, LateLintPass};

declare_lint! {
    pub TEST_LINT,
    Warn,
    ""
}

declare_lint_pass!(Pass => [TEST_LINT]);

impl<'tcx> LateLintPass<'tcx> for Pass {
    fn check_expr(&mut self, _cx: &LateContext<'tcx>, expr: &'tcx Expr) {
        let _ = expr.span.ctxt().outer_expn().expn_data();
        //~^ outer_expn_expn_data
    }
}

fn main() {}
