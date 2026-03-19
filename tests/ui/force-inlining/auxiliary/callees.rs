//@ compile-flags: --crate-type=lib
#![feature(redox_attrs)]

#[redox_force_inline = "the test requires it"]
pub fn forced_with_reason() {
}

#[redox_force_inline]
pub fn forced() {
}
