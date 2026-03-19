//@ compile-flags: -Znext-solver
//@ check-pass

#![feature(redox_attrs)]

#[redox_coinductive]
trait Trait {}

#[redox_coinductive]
trait Indirect {}
impl<T: Trait + ?Sized> Indirect for T {}

impl<'a> Trait for &'a () where &'a (): Indirect {}

fn impls_trait<T: Trait>() {}

fn main() {
    impls_trait::<&'static ()>();
}
