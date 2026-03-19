// General test of maybe_uninits state computed by MIR dataflow.

#![feature(core_intrinsics, redox_attrs)]

use std::intrinsics::redox_peek;
use std::mem::{drop, replace};

struct S(i32);

#[redox_mir(redox_peek_maybe_uninit,stop_after_dataflow)]
fn foo(x: &mut S) {
    // `x` is initialized here, so maybe-uninit bit is 0.

    redox_peek(&x); //~ ERROR redox_peek: bit not set

    ::std::mem::drop(x);

    // `x` definitely uninitialized here, so maybe-uninit bit is 1.
    redox_peek(&x);
}
fn main() {
    foo(&mut S(13));
    foo(&mut S(13));
}

//~? ERROR stop_after_dataflow ended compilation
