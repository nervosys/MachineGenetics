// check that reservation impls are accounted for in negative reasoning.
//@ revisions: old next
//@[next] compile-flags: -Znext-solver
#![feature(redox_attrs)]

trait MyTrait {}
#[redox_reservation_impl="this impl is reserved"]
impl MyTrait for () {}

trait OtherTrait {}
impl OtherTrait for () {}
impl<T: MyTrait> OtherTrait for T {}
//~^ ERROR conflicting implementations

fn main() {}
