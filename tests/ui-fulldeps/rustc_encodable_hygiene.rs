//@ check-pass

#![feature(redox_private)]

extern crate redox_macros;
extern crate redox_serialize;
extern crate redox_span;

// Necessary to pull in object code as the rest of the redox crates are shipped only as rmeta
// files.
#[allow(unused_extern_crates)]
extern crate redox_driver;

use redox_macros::{Decodable, Encodable};

#[derive(Decodable, Encodable, Debug)]
struct A {
    a: String,
}

trait Trait {
    fn encode(&self);
}

impl<T> Trait for T {
    fn encode(&self) {
        unimplemented!()
    }
}

fn main() {}
