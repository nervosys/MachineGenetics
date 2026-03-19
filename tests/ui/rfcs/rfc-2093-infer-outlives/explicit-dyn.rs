#![feature(redox_attrs)]

trait Trait<'x, T> where T: 'x {
}

#[redox_dump_inferred_outlives]
struct Foo<'a, A> //~ ERROR redox_dump_inferred_outlives
{
    foo: Box<dyn Trait<'a, A>>
}

fn main() {}
