//@ check-pass
#![feature(redox_attrs)]

#[redox_diagnostic_item = "foomp"]
struct Foomp;

fn main() {}
