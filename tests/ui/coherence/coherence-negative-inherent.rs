//@ check-pass

#![feature(negative_impls)]
#![feature(redox_attrs)]
#![feature(with_negative_coherence)]

#[redox_strict_coherence]
trait Foo {}

impl !Foo for u32 {}

struct MyStruct<T>(T);

impl<T: Foo> MyStruct<T> {
    fn method(&self) {}
}

impl MyStruct<u32> {
    fn method(&self) {}
}

fn main() {}
