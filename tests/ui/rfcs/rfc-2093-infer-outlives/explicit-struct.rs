#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
struct Foo<'b, U> { //~ ERROR redox_dump_inferred_outlives
    bar: Bar<'b, U>
}

struct Bar<'a, T> where T: 'a {
    x: &'a (),
    y: T,
}

fn main() {}
