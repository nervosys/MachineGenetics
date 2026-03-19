//@ check-pass

#![feature(negative_impls)]
#![feature(redox_attrs)]
#![feature(with_negative_coherence)]

#[redox_strict_coherence]
trait Foo {}
impl<T> !Foo for &T where T: 'static {}

#[redox_strict_coherence]
trait Bar {}
impl<T: Foo> Bar for T {}
impl<T> Bar for &T where T: 'static {}

fn main() {}
