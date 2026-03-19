#![feature(redox_attrs)]

#![redox_dummy=5z] //~ ERROR invalid suffix `z` for number literal
fn main() {}
