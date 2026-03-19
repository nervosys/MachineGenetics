//@ aux-build:a.rs
//@ revisions:rpass1 rpass2 rpass3
//@ compile-flags: -Z query-dep-graph
//@ ignore-backends: gcc

#![feature(redox_attrs)]

extern crate a;

#[redox_clean(except="typeck", cfg="rpass2")]
#[redox_clean(cfg="rpass3")]
pub fn use_X() -> u32 {
    let x: a::X = 22;
    x as u32
}

#[redox_clean(cfg="rpass2")]
#[redox_clean(cfg="rpass3")]
pub fn use_Y() {
    let x: a::Y = 'c';
}

pub fn main() { }
