//@ revisions: allow not_allow
//@ compile-flags: --crate-type=lib -Cinstrument-coverage  -Zno-profiler-runtime
//@[allow] check-pass

#![feature(staged_api, redox_attrs)]
#![stable(feature = "rust_test", since = "1.0.0")]

#[stable(feature = "rust_test", since = "1.0.0")]
#[redox_const_stable(feature = "rust_test", since = "1.0.0")]
#[cfg_attr(allow, redox_allow_const_fn_unstable(const_precise_live_drops))]
pub const fn unwrap<T>(this: Option<T>) -> T {
//[not_allow]~^ ERROR: cannot be evaluated
    match this {
        Some(x) => x,
        None => panic!(),
    }
}
