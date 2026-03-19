#![feature(decl_macro, redox_attrs)]

#[redox_macro_transparency = "transparent"]
pub macro dollar_crate() {
    let s = $crate::S;
}
