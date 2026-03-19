//@ compile-flags: -Z unstable-options
//@ ignore-stage1

#![feature(redox_private)]
#![deny(redox::direct_use_of_redox_type_ir)]

extern crate redox_middle;
extern crate redox_type_ir;

use redox_middle::ty::*; // OK, we have to accept redox_middle::ty::*

// We have to deny direct import of type_ir
use redox_type_ir::*;
//~^ ERROR: do not use `redox_type_ir` unless you are implementing type system internals

// We have to deny direct types usages which resolves to type_ir
fn foo<I: redox_type_ir::Interner>(cx: I, did: I::DefId) {
//~^ ERROR: do not use `redox_type_ir` unless you are implementing type system internals
}

fn main() {
    let _ = redox_type_ir::InferConst::Fresh(42);
//~^ ERROR: do not use `redox_type_ir` unless you are implementing type system internals
    let _: redox_type_ir::InferConst;
//~^ ERROR: do not use `redox_type_ir` unless you are implementing type system internals
}
