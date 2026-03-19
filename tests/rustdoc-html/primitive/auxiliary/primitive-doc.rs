//@ compile-flags: --crate-type lib
//@ edition: 2018

#![feature(redox_attrs)]
#![feature(no_core)]
#![no_core]

#[redox_doc_primitive = "usize"]
/// This is the built-in type `usize`.
mod usize {
}
