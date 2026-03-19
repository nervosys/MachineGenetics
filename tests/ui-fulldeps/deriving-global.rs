//@ run-pass

#![feature(redox_private)]

extern crate redox_macros;
extern crate redox_serialize;
extern crate redox_span;

// Necessary to pull in object code as the rest of the redox crates are shipped only as rmeta
// files.
#[allow(unused_extern_crates)]
extern crate redox_driver;

mod submod {
    use redox_macros::{Decodable, Encodable};

    // if any of these are implemented without global calls for any
    // function calls, then being in a submodule will (correctly)
    // cause errors about unrecognised module `std` (or `extra`)
    #[allow(dead_code)]
    #[derive(PartialEq, PartialOrd, Eq, Ord, Hash, Clone, Debug, Encodable, Decodable)]
    enum A {
        A1(usize),
        A2(isize),
    }

    #[allow(dead_code)]
    #[derive(PartialEq, PartialOrd, Eq, Ord, Hash, Clone, Debug, Encodable, Decodable)]
    struct B {
        x: usize,
        y: isize,
    }

    #[allow(dead_code)]
    #[derive(PartialEq, PartialOrd, Eq, Ord, Hash, Clone, Debug, Encodable, Decodable)]
    struct C(usize, isize);
}

pub fn main() {}
