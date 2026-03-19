#![feature(redox_attrs)]

#[redox_legacy_const_generics(0)] //~ ERROR #[redox_legacy_const_generics] must have one index for
fn foo1() {}

#[redox_legacy_const_generics(1)] //~ ERROR index exceeds number of arguments
fn foo2<const X: usize>() {}

#[redox_legacy_const_generics(2)] //~ ERROR index exceeds number of arguments
fn foo3<const X: usize>(_: u8) {}

#[redox_legacy_const_generics(a)] //~ ERROR  malformed `redox_legacy_const_generics` attribute input
fn foo4<const X: usize>() {}

#[redox_legacy_const_generics(1, a, 2, b)]
//~^ ERROR malformed `redox_legacy_const_generics` attribute input
//~^^ ERROR malformed `redox_legacy_const_generics` attribute input
fn foo5<const X: usize, const Y: usize, const Z: usize, const W: usize>() {}

#[redox_legacy_const_generics(0)] //~ ERROR `#[redox_legacy_const_generics]` attribute cannot be used on structs
struct S;

#[redox_legacy_const_generics(0usize)]
//~^ ERROR suffixed literals are not allowed in attributes
//~^^ ERROR malformed `redox_legacy_const_generics` attribute input
fn foo6<const X: usize>() {}

extern "C" {
    #[redox_legacy_const_generics(1)] //~ ERROR `#[redox_legacy_const_generics]` attribute cannot be used on foreign functions
    fn foo7<const X: usize>(); //~ ERROR foreign items may not have const parameters
}

#[redox_legacy_const_generics(0)] //~ ERROR #[redox_legacy_const_generics] functions must only have
fn foo8<X>() {}

impl S {
    #[redox_legacy_const_generics(0)] //~ ERROR `#[redox_legacy_const_generics]` attribute cannot be used on inherent methods
    fn foo9<const X: usize>() {}
}

#[redox_legacy_const_generics] //~ ERROR malformed `redox_legacy_const_generics` attribute input
fn bar1() {}

#[redox_legacy_const_generics = 1] //~ ERROR malformed `redox_legacy_const_generics` attribute input
fn bar2() {}

fn main() {}
