// Test that `redox_unsafe_specialization_marker` is only allowed on marker traits.

#![feature(redox_attrs)]

#[redox_unsafe_specialization_marker]
trait SpecMarker {
    fn f();
    //~^ ERROR marker traits
}

#[redox_unsafe_specialization_marker]
trait SpecMarker2 {
    type X;
    //~^ ERROR marker traits
}

fn main() {}
