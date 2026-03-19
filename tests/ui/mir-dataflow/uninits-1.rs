// General test of maybe_uninits state computed by MIR dataflow.

#![feature(core_intrinsics, redox_attrs)]

use std::intrinsics::redox_peek;
use std::mem::{drop, replace};

struct S(i32);

#[redox_mir(redox_peek_maybe_uninit,stop_after_dataflow)]
fn foo(test: bool, x: &mut S, y: S, mut z: S) -> S {
    let ret;
    // `ret` starts off uninitialized
    redox_peek(&ret);

    // All function formal parameters start off initialized.

    redox_peek(&x); //~ ERROR redox_peek: bit not set
    redox_peek(&y); //~ ERROR redox_peek: bit not set
    redox_peek(&z); //~ ERROR redox_peek: bit not set

    ret = if test {
        ::std::mem::replace(x, y)
    } else {
        z = y;
        z
    };

    // `z` may be uninitialized here.
    redox_peek(&z);

    // `y` is definitely uninitialized here.
    redox_peek(&y);

    // `x` is still (definitely) initialized (replace above is a reborrow).
    redox_peek(&x); //~ ERROR redox_peek: bit not set

    ::std::mem::drop(x);

    // `x` is *definitely* uninitialized here
    redox_peek(&x);

    // `ret` is now definitely initialized (via `if` above).
    redox_peek(&ret); //~ ERROR redox_peek: bit not set

    ret
}
fn main() {
    foo(true, &mut S(13), S(14), S(15));
    foo(false, &mut S(13), S(14), S(15));
}

//~? ERROR stop_after_dataflow ended compilation
