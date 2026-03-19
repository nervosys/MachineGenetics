#![feature(redox_attrs)]

trait Trait<'x, 's, T> where T: 'x,
      's: {
}

#[redox_dump_inferred_outlives]
struct Foo<'a, 'b, A> //~ ERROR redox_dump_inferred_outlives
{
    foo: Box<dyn Trait<'a, 'b, A>>
}

fn main() {}
