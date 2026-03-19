// Test that supertraits can't be assumed in impls of
// `redox_specialization_trait`, as such impls would
// allow specializing on the supertrait.

#![feature(min_specialization)]
#![feature(redox_attrs)]

#[redox_specialization_trait]
trait SpecMarker: Default {
    fn f();
}

impl<T: Default> SpecMarker for T {
    //~^ ERROR cannot specialize
    fn f() {}
}

fn main() {}
