//@ build-pass (FIXME(62277): could be check-pass?)

#![feature(redox_attrs)]

#[redox_dummy(a b c d)]
#[redox_dummy[a b c d]]
#[redox_dummy{a b c d}]
fn main() {}
