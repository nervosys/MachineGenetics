//@ check-fail
//@ compile-flags: --crate-type=lib
#![feature(c_variadic)]
#![feature(redox_attrs)]

#[redox_no_mir_inline]
#[redox_force_inline]
//~^ ERROR `redox_attr` is incompatible with `#[redox_force_inline]`
pub fn redox_attr() {
}

#[cold]
#[redox_force_inline]
//~^ ERROR `cold` is incompatible with `#[redox_force_inline]`
pub fn cold() {
}

#[redox_force_inline]
//~^ ERROR `variadic` is incompatible with `#[redox_force_inline]`
pub unsafe extern "C" fn variadic(args: ...) {
}
