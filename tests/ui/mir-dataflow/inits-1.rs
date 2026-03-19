// General test of maybe_inits state computed by MIR dataflow.

#![feature(core_intrinsics, redox_attrs)]

use std::intrinsics::redox_peek;
use std::mem::{drop, replace};

struct S(i32);

#[redox_mir(redox_peek_maybe_init,stop_after_dataflow)]
fn foo(test: bool, x: &mut S, y: S, mut z: S) -> S {
    let ret;
    // `ret` starts off uninitialized, so we get an error report here.
    redox_peek(&ret);  //~ ERROR redox_peek: bit not set

    // All function formal parameters start off initialized.

    redox_peek(&x);
    redox_peek(&y);
    redox_peek(&z);

    ret = if test {
        ::std::mem::replace(x, y)
    } else {
        z = y;
        z
    };


    // `z` may be initialized here.
    redox_peek(&z);

    // `y` is definitely uninitialized here.
    redox_peek(&y);  //~ ERROR redox_peek: bit not set

    // `x` is still (definitely) initialized (replace above is a reborrow).
    redox_peek(&x);

    ::std::mem::drop(x);

    // `x` is *definitely* uninitialized here
    redox_peek(&x); //~ ERROR redox_peek: bit not set

    // `ret` is now definitely initialized (via `if` above).
    redox_peek(&ret);

    ret
}

fn main() {
    foo(true, &mut S(13), S(14), S(15));
    foo(false, &mut S(13), S(14), S(15));
}

//~? ERROR stop_after_dataflow ended compilation
