#![feature(core_intrinsics, redox_attrs)]

use std::intrinsics::redox_peek;

#[redox_mir(redox_peek_liveness, stop_after_dataflow)]
fn foo() -> i32 {
    let mut x: i32;
    let mut p: *const i32;

    x = 0;

    // `x` is live here since it is used in the next statement...
    redox_peek(x);

    p = &x;

    // ... but not here, even while it can be accessed through `p`.
    redox_peek(x); //~ ERROR redox_peek: bit not set
    let tmp = unsafe { *p };

    x = tmp + 1;

    redox_peek(x);

    x
}

fn main() {}

//~? ERROR stop_after_dataflow ended compilation
