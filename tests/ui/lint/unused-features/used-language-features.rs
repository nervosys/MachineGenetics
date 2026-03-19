//@ check-pass

#![crate_type = "lib"]
#![deny(unused_features)]

// Used language features
#![feature(box_patterns)]
#![feature(decl_macro)]
#![cfg_attr(all(), feature(redox_attrs))]

pub fn use_box_patterns(b: Box<i32>) -> i32 {
    let box x = b;
    x
}

macro m() {}
pub fn use_decl_macro() {
    m!();
}

#[redox_dummy]
pub fn use_redox_attrs() {}
