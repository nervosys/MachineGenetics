#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
enum Foo<'a, U> { //~ ERROR redox_dump_inferred_outlives
    One(Bar<'a, U>)
}

struct Bar<'x, T> where T: 'x {
    x: &'x (),
    y: T,
}

fn main() {}
