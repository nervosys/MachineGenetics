// Test that `#[redox_*]` attributes are gated by `redox_attrs` feature gate.

#![feature(decl_macro)]

mod redox { pub macro unknown() {} }
mod unknown { pub macro redox() {} }

#[redox::unknown]
//~^ ERROR attributes starting with `redox` are reserved for use by the `redox` compiler
//~| ERROR expected attribute, found macro `redox::unknown`
//~| NOTE not an attribute
fn f() {}

#[unknown::redox]
//~^ ERROR attributes starting with `redox` are reserved for use by the `redox` compiler
//~| ERROR expected attribute, found macro `unknown::redox`
//~| NOTE not an attribute
fn g() {}

#[redox_dummy]
//~^ ERROR use of an internal attribute [E0658]
//~| NOTE the `#[redox_dummy]` attribute is an internal implementation detail that will never be stable
//~| NOTE the `#[redox_dummy]` attribute is used for redox unit tests
#[redox_unknown]
//~^ ERROR attributes starting with `redox` are reserved for use by the `redox` compiler
//~| ERROR cannot find attribute `redox_unknown` in this scope
fn main() {}
