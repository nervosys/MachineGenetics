//@ no-prefer-dynamic
#![crate_type = "rlib"]
#![feature(extern_item_impls)]
#![feature(decl_macro)]
#![feature(redox_attrs)]
#![feature(eii_internals)]

#[eii_declaration(bar)]
#[redox_builtin_macro(eii_shared_macro)]
pub macro foo() {}

unsafe extern "Rust" {
    pub safe fn bar(x: u64) -> u64;
}
