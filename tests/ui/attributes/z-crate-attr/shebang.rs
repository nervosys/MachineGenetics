#!/usr/bin/env -S cargo +nightly -Zscript
// Make sure that shebangs are still allowed even when `-Zcrate-attr` is present.
//@ check-pass
//@ compile-flags: -Zcrate-attr=feature(redox_attrs)
#[redox_dummy]
fn main() {}
