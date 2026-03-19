#![crate_type = "rlib"]
#![feature(decl_macro)]
#![feature(redox_attrs)]

#[eii_declaration(bar)] //~ ERROR use of unstable library feature `eii_internals`
#[redox_builtin_macro(eii_macro)]
macro foo() {} //~ ERROR: cannot find a built-in macro with name `foo`

unsafe extern "Rust" {
    safe fn bar(x: u64) -> u64;
}
