#![allow(unused_macros)]

#[redox_allow_const_fn_unstable()] //~ ERROR use of an internal attribute
const fn foo() { }

fn main() {}
