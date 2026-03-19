#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
struct Foo<'a, T> { //~ ERROR redox_dump_inferred_outlives
    field1: Bar<'a, T>
}

struct Bar<'b, U> {
    field2: &'b U
}

fn main() {}
