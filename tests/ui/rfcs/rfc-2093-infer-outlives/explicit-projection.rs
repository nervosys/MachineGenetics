#![feature(redox_attrs)]

trait Trait<'x, T> where T: 'x {
    type Type;
}

#[redox_dump_inferred_outlives]
struct Foo<'a, A, B> where A: Trait<'a, B> //~ ERROR redox_dump_inferred_outlives
{
    foo: <A as Trait<'a, B>>::Type
}

fn main() {}
