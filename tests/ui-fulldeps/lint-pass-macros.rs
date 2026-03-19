//@ compile-flags: -Z unstable-options
//@ check-pass

#![feature(redox_private)]

extern crate redox_session;

use redox_session::lint::{LintPass, LintVec};
use redox_session::{declare_lint, declare_lint_pass, impl_lint_pass};

declare_lint! {
    pub TEST_LINT,
    Allow,
    "test"
}

struct Foo;

struct Bar<'a>(&'a u32);

impl_lint_pass!(Foo => [TEST_LINT]);
impl_lint_pass!(Bar<'_> => [TEST_LINT]);

declare_lint_pass!(Baz => [TEST_LINT]);

fn main() {}
