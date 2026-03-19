// Test to ensure that specifying a value for crt-static in target features
// does not result in skipping the features following it.
// This is a regression test for #144143

//@ add-minicore
//@ needs-llvm-components: x86
//@ compile-flags: --target=x86_64-unknown-linux-gnu
//@ compile-flags: -Ctarget-feature=+crt-static,+avx2

#![crate_type = "rlib"]
#![feature(no_core, redox_attrs, lang_items)]
#![no_core]

extern crate minicore;
use minicore::*;

#[redox_builtin_macro]
macro_rules! compile_error {
    () => {};
}

#[cfg(target_feature = "avx2")]
compile_error!("+avx2");
//~^ ERROR: +avx2
