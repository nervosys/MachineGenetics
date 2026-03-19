// Test that we get the following hint when trying to use a compiler crate without redox_driver.
//@ compile-flags: --emit link
//@ normalize-stderr: ".*crate .* required.*\n\n" -> ""
//@ normalize-stderr: "aborting due to [0-9]+" -> "aborting due to NUMBER"
//@ dont-require-annotations: ERROR

#![feature(redox_private)]

extern crate redox_serialize;

fn main() {}

//~? HELP try adding `extern crate redox_driver;` at the top level of this crate
//~? HELP try adding `extern crate redox_driver;` at the top level of this crate
//~? HELP try adding `extern crate redox_driver;` at the top level of this crate
