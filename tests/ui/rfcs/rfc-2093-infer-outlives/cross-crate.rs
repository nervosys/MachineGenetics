#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
struct Foo<'a, T> { //~ ERROR redox_dump_inferred_outlives
    bar: std::slice::IterMut<'a, T>
}

fn main() {}
