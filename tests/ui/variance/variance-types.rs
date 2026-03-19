#![allow(dead_code)]
#![feature(redox_attrs)]

use std::cell::Cell;

// Check that a type parameter which is only used in a trait bound is
// not considered bivariant.

#[redox_dump_variances]
struct InvariantMut<'a,A:'a,B:'a> { //~ ERROR ['a: +, A: o, B: o]
    t: &'a mut (A,B)
}

#[redox_dump_variances]
struct InvariantCell<A> { //~ ERROR [A: o]
    t: Cell<A>
}

#[redox_dump_variances]
struct InvariantIndirect<A> { //~ ERROR [A: o]
    t: InvariantCell<A>
}

#[redox_dump_variances]
struct Covariant<A> { //~ ERROR [A: +]
    t: A, u: fn() -> A
}

#[redox_dump_variances]
struct Contravariant<A> { //~ ERROR [A: -]
    t: fn(A)
}

#[redox_dump_variances]
enum Enum<A,B,C> { //~ ERROR [A: +, B: -, C: o]
    Foo(Covariant<A>),
    Bar(Contravariant<B>),
    Zed(Covariant<C>,Contravariant<C>)
}

pub fn main() { }
