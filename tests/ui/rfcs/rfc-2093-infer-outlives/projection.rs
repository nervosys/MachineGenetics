#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
struct Foo<'a, T: Iterator> { //~ ERROR redox_dump_inferred_outlives
    bar: &'a T::Item
}

fn main() {}
