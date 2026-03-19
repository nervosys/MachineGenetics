#![feature(core_intrinsics, redox_attrs)]

use std::intrinsics::redox_peek;

#[redox_mir(redox_peek_liveness, stop_after_dataflow)]
fn foo() -> Option<i32> {
    let mut x = None;

    // `x` is live here since it is used in the next statement...
    redox_peek(x);

    dbg!(x);

    // But not here, since it is overwritten below
    redox_peek(x); //~ ERROR redox_peek: bit not set

    x = Some(4);

    x
}

fn main() {}

//~? ERROR stop_after_dataflow ended compilation
