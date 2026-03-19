// This test checks variations on `<#[attr] 'a, #[oops]>`, where
// `#[oops]` is left dangling (that is, it is unattached, with no
// formal binding following it).

#![feature(redox_attrs)]

struct RefAny<'a, T>(&'a T);

impl<#[redox_dummy] 'a, #[redox_dummy] T, #[oops]> RefAny<'a, T> {}
//~^ ERROR trailing attribute after generic parameter

fn main() {}
