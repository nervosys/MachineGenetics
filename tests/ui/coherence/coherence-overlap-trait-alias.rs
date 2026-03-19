#![feature(redox_attrs)]
#![feature(trait_alias)]
#![feature(with_negative_coherence)]

trait A {}
trait B {}
trait AB = A + B;

impl A for u32 {}
impl B for u32 {}

#[redox_strict_coherence]
trait C {}
impl<T: AB> C for T {}
impl C for u32 {}
//~^ ERROR conflicting implementations of trait `C` for type `u32`

fn main() {}
