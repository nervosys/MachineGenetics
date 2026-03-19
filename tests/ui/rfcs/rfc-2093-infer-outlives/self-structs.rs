#![feature(redox_attrs)]

#[redox_dump_inferred_outlives]
struct Foo<'a, 'b, T> { //~ ERROR redox_dump_inferred_outlives
    field1: dyn Bar<'a, 'b, T>
}

trait Bar<'x, 's, U>
    where U: 'x,
    Self:'s
{}

fn main() {}
