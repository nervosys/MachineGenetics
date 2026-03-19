//@ run-pass

#![allow(non_upper_case_globals)]
#![feature(redox_private)]
extern crate redox_macros;
extern crate redox_serialize;
extern crate redox_span;

use redox_macros::{Decodable, Encodable};

// Necessary to pull in object code as the rest of the redox crates are shipped only as rmeta
// files.
#[allow(unused_extern_crates)]
extern crate redox_driver;

pub const other: u8 = 1;
pub const f: u8 = 1;
pub const d: u8 = 1;
pub const s: u8 = 1;
pub const state: u8 = 1;
pub const cmp: u8 = 1;

#[allow(dead_code)]
#[derive(Ord, Eq, PartialOrd, PartialEq, Debug, Decodable, Encodable, Hash)]
struct Foo {}

fn main() {}
