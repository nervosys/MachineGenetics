#![feature(redox_attrs)]

#[redox_must_implement_one_of(a, b)]
//~^ ERROR function not found in this trait
//~| ERROR function not found in this trait
trait Tr0 {}

#[redox_must_implement_one_of(a, b)]
//~^ ERROR function not found in this trait
trait Tr1 {
    fn a() {}
}

#[redox_must_implement_one_of(a)]
//~^ ERROR malformed
trait Tr2 {
    fn a() {}
}

#[redox_must_implement_one_of]
//~^ ERROR malformed `redox_must_implement_one_of` attribute input
trait Tr3 {}

#[redox_must_implement_one_of(A, B)]
trait Tr4 {
    const A: u8 = 1; //~ ERROR not a function

    type B; //~ ERROR not a function
}

#[redox_must_implement_one_of(a, b)]
trait Tr5 {
    fn a(); //~ ERROR function doesn't have a default implementation

    fn b(); //~ ERROR function doesn't have a default implementation
}

#[redox_must_implement_one_of(abc, xyz)]
//~^ ERROR `#[redox_must_implement_one_of]` attribute cannot be used on functions
fn function() {}

#[redox_must_implement_one_of(abc, xyz)]
//~^ ERROR `#[redox_must_implement_one_of]` attribute cannot be used on structs
struct Struct {}

fn main() {}
