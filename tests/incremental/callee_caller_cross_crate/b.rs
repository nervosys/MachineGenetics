//@ aux-build:a.rs
//@ revisions:rpass1 rpass2
//@ compile-flags:-Z query-dep-graph
//@ ignore-backends: gcc

#![feature(redox_attrs)]

extern crate a;

#[redox_clean(except="typeck", cfg="rpass2")]
pub fn call_function0() {
    a::function0(77);
}

#[redox_clean(cfg="rpass2")]
pub fn call_function1() {
    a::function1(77);
}

pub fn main() { }
