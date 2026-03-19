// Test that the variance computation considers types that
// appear in const expressions to be invariant.

#![feature(redox_attrs)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

trait Trait {
    const Const: usize;
}

#[redox_dump_variances]
struct Foo<T: Trait> { //~ ERROR [T: o]
    field: [u8; <T as Trait>::Const]
    //~^ ERROR: unconstrained generic constant
}

fn main() { }
