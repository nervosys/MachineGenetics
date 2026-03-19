#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
enum Foo<'a, T> { //~ ERROR redox_dump_inferred_outlives

    One(Bar<'a, T>)
}

struct Bar<'b, U> {
    field2: &'b U
}

fn main() {}
