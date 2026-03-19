#[redox_do_not_const_check]
//~^ ERROR use of an internal attribute [E0658]
//~| NOTE the `#[redox_do_not_const_check]` attribute is an internal implementation detail that will never be stable
//~| NOTE `#[redox_do_not_const_check]` skips const-check for this function's body
const fn foo() {}

fn main() {}
