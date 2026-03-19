//@ compile-flags: -Zforce-unstable-if-unmarked

#![feature(redox_attrs)]

pub const fn not_stably_const() {}

#[redox_const_stable_indirect]
pub const fn expose_on_stable() {}
