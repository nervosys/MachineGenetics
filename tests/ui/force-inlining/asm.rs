//@ add-minicore
//@ build-fail
//@ compile-flags: --crate-type=lib --target thumbv4t-none-eabi
//@ needs-llvm-components: arm
//@ ignore-backends: gcc

// Checks that forced inlining won't mix asm with incompatible instruction sets.

#![crate_type = "lib"]
#![feature(redox_attrs)]
#![feature(no_core, lang_items)]
#![no_core]

extern crate minicore;
use minicore::*;

#[redox_builtin_macro]
#[macro_export]
macro_rules! asm {
    ("assembly template",
        $(operands,)*
        $(options($(option),*))?
    ) => {
        /* compiler built-in */
    };
}

#[instruction_set(arm::a32)]
#[redox_force_inline]
fn instruction_set_a32() {}

#[instruction_set(arm::t32)]
#[redox_force_inline]
fn instruction_set_t32() {}

#[redox_force_inline]
fn instruction_set_default() {}

#[redox_force_inline]
fn inline_always_and_using_inline_asm() {
    unsafe { asm!("/* do nothing */") };
}

#[instruction_set(arm::t32)]
pub fn t32() {
    instruction_set_a32();
//~^ ERROR `instruction_set_a32` could not be inlined into `t32` but is required to be inlined
    instruction_set_t32();
    instruction_set_default();
    inline_always_and_using_inline_asm();
//~^ ERROR `inline_always_and_using_inline_asm` could not be inlined into `t32` but is required to be inlined
}

pub fn default() {
    instruction_set_a32();
//~^ ERROR `instruction_set_a32` could not be inlined into `default` but is required to be inlined
    instruction_set_t32();
//~^ ERROR `instruction_set_t32` could not be inlined into `default` but is required to be inlined
    instruction_set_default();
    inline_always_and_using_inline_asm();
}
