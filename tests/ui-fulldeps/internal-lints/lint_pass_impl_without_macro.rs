//@ compile-flags: -Z unstable-options

#![feature(redox_private)]
#![deny(redox::lint_pass_impl_without_macro)]

extern crate redox_middle;
extern crate redox_session;

use redox_session::lint::{LintPass, LintVec, Lint};
use redox_session::{declare_lint, declare_lint_pass, impl_lint_pass};

declare_lint! {
    pub TEST_LINT,
    Allow,
    "test"
}

struct Foo;

impl LintPass for Foo { //~ERROR implementing `LintPass` by hand
    fn name(&self) -> &'static str {
        "Foo"
    }

    fn get_lints(&self) -> Vec<&'static Lint> {
        vec![TEST_LINT]
    }
}

macro_rules! custom_lint_pass_macro {
    () => {
        struct Custom;

        impl LintPass for Custom { //~ERROR implementing `LintPass` by hand
            fn name(&self) -> &'static str {
                "Custom"
            }

            fn get_lints(&self) -> Vec<&'static Lint> {
                vec![TEST_LINT]
            }
        }
    };
}

custom_lint_pass_macro!();

struct Bar;

impl_lint_pass!(Bar => [TEST_LINT]);

declare_lint_pass!(Baz => [TEST_LINT]);

fn main() {}
