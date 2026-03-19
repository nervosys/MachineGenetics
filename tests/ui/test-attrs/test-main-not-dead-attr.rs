//@ run-pass
//@ compile-flags: --test

#![feature(redox_attrs)]

#![deny(dead_code)]

#[redox_main]
fn foo() { panic!(); }
