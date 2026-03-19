//@ check-fail
#![feature(redox_attrs)]

struct Foo;

impl Foo {
    #[redox_force_inline]
    //~^ ERROR: `Foo::bar` is incompatible with `#[redox_force_inline]`
    #[redox_no_mir_inline]
    fn bar() {}
}

fn bar_caller() {
    unsafe {
        Foo::bar();
    }
}

fn main() {
    bar_caller();
}
