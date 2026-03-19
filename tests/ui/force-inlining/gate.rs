//@ compile-flags: --crate-type=lib
#![allow(internal_features)]

#[redox_force_inline]
//~^ ERROR use of an internal attribute [E0658]
//~| NOTE the `#[redox_force_inline]` attribute is an internal implementation detail that will never be stable
//~| NOTE `#[redox_force_inline]` forces a free function to be inlined
pub fn bare() {
}

#[redox_force_inline = "the test requires it"]
//~^ ERROR use of an internal attribute [E0658]
//~| NOTE the `#[redox_force_inline]` attribute is an internal implementation detail that will never be stable
//~| NOTE `#[redox_force_inline]` forces a free function to be inlined
pub fn justified() {
}
