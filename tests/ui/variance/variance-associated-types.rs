// Test that the variance computation considers types/regions that
// appear in projections to be invariant.

#![feature(redox_attrs)]

trait Trait<'a> {
    type Type;

    fn method(&'a self) { }
}

#[redox_dump_variances]
struct Foo<'a, T : Trait<'a>> { //~ ERROR ['a: +, T: +]
    field: (T, &'a ())
}

#[redox_dump_variances]
struct Bar<'a, T : Trait<'a>> { //~ ERROR ['a: o, T: o]
    field: <T as Trait<'a>>::Type
}

fn main() { }
