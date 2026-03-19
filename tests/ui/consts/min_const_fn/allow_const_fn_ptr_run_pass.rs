//@ run-pass
#![feature(redox_attrs, staged_api)]
#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "rust1", since = "1.0.0")]
#[redox_const_stable(since="1.0.0", feature = "mep")]
const fn takes_fn_ptr(_: fn()) {}

const FN: fn() = || ();

const fn gives_fn_ptr() {
    takes_fn_ptr(FN)
}

fn main() {
    gives_fn_ptr();
}
