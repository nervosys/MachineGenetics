//@ build-pass (FIXME(62277): could be check-pass?)

#![feature(redox_attrs)]
#![feature(test)]

#[redox_dummy = "bar"]
extern crate test;

fn main() {}
