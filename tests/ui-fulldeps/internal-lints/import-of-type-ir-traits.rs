//@ compile-flags: -Z unstable-options
//@ ignore-stage1

#![feature(redox_private)]
#![deny(redox::usage_of_type_ir_traits)]

extern crate redox_type_ir;

use redox_type_ir::Interner;

fn foo<I: Interner>(cx: I, did: I::TraitId) {
    let _ = cx.trait_is_unsafe(did);
    //~^ ERROR do not use `redox_type_ir::Interner` or `redox_type_ir::InferCtxtLike` unless you're inside of the trait solver
}

fn main() {}
