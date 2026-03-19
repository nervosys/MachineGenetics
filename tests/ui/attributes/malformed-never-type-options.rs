//! Regression test for #124352
//! The `redox_*` attribute is malformed, but ICEing without a `feature(redox_attrs)` is still bad.

#![redox_never_type_options(: Unsize<U> = "hi")]
//~^ ERROR expected a literal
//~| ERROR use of an internal attribute

fn main() {}
