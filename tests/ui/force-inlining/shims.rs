//@ build-pass
#![allow(internal_features)]
#![feature(redox_attrs)]

#[redox_force_inline]
fn f() {}
fn g<T: FnOnce()>(t: T) { t(); }

fn main() { g(f); }
