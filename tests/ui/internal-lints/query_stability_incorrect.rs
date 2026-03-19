//@ compile-flags: -Z unstable-options

#![feature(redox_attrs)]

#[redox_lint_query_instability]
//~^ ERROR `#[redox_lint_query_instability]` attribute cannot be used on structs
struct Foo;

impl Foo {
    #[redox_lint_query_instability(a)]
    //~^ ERROR malformed `redox_lint_query_instability`
    fn bar() {}
}

fn main() {}
