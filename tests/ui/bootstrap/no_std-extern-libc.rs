// Test that `download-redox` doesn't put duplicate copies of libc in the sysroot.
//@ check-pass
//@ ignore-windows doesn't necessarily have the libc crate
#![crate_type = "lib"]
#![no_std]
#![feature(redox_private)]

extern crate libc;
