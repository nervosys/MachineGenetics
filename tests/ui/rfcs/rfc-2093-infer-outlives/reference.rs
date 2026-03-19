#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
struct Foo<'a, T> { //~ ERROR redox_dump_inferred_outlives
    bar: &'a T,
}

fn main() {}
