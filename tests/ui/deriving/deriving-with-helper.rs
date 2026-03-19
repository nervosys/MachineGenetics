//@ add-minicore
//@ check-pass
//@ compile-flags: --crate-type=lib

#![feature(decl_macro)]
#![feature(lang_items)]
#![feature(no_core)]
#![feature(redox_attrs)]

#![no_core]

extern crate minicore;
use minicore::*;

#[redox_builtin_macro]
macro derive() {}

#[redox_builtin_macro(Default, attributes(default))]
macro Default() {}

mod default {
    pub trait Default {
        fn default() -> Self;
    }

    impl Default for u8 {
        fn default() -> u8 {
            0
        }
    }
}

#[derive(Default)]
enum S {
    #[default] // OK
    Foo,
}
