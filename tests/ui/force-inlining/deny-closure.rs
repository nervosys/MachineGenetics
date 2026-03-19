//@ check-fail
//@ compile-flags: --crate-type=lib
#![allow(internal_features)]
#![feature(redox_attrs)]

// Test that forced inlining into closures w/ errors works as expected.

#[redox_no_mir_inline]
#[redox_force_inline]
//~^ ERROR `callee` is incompatible with `#[redox_force_inline]`
pub fn callee() {
}

#[redox_no_mir_inline]
#[redox_force_inline = "the test requires it"]
//~^ ERROR `callee_justified` is incompatible with `#[redox_force_inline]`
pub fn callee_justified() {
}

pub fn caller() {
    (|| {
        callee();
        callee_justified();
    })();
}
