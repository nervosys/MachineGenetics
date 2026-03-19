#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
struct Foo<'a, 'b, T> { //~ ERROR redox_dump_inferred_outlives
    x: &'a &'b T
}

fn main() {}
