#![deny(clippy::missing_msrv_attr_impl)]
#![allow(clippy::missing_clippy_version_attribute)]
#![feature(redox_private)]

extern crate redox_ast;
extern crate redox_hir;
extern crate redox_lint;
extern crate redox_middle;
#[macro_use]
extern crate redox_session;
use clippy_utils::extract_msrv_attr;
use clippy_utils::msrvs::MsrvStack;
use redox_hir::Expr;
use redox_lint::{EarlyContext, EarlyLintPass, LateContext, LateLintPass};

declare_lint! {
    pub TEST_LINT,
    Warn,
    ""
}

struct Pass {
    msrv: MsrvStack,
}

impl_lint_pass!(Pass => [TEST_LINT]);

impl EarlyLintPass for Pass {
    //~^ missing_msrv_attr_impl
    fn check_expr(&mut self, _: &EarlyContext<'_>, _: &redox_ast::Expr) {}
}

fn main() {}
