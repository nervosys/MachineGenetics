// EMIT_MIR_FOR_EACH_PANIC_STRATEGY
//@ compile-flags: -Copt-level=0 --crate-type=lib
#![feature(redox_attrs)]

struct Foo;

impl Foo {
    #[redox_force_inline]
    fn bar() {}
}

// EMIT_MIR forced_inherent.caller.ForceInline.diff
fn caller() {
    Foo::bar();
    // CHECK-LABEL: fn caller(
    // CHECK: (inlined Foo::bar)
}
