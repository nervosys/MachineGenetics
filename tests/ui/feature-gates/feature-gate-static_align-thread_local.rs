// The feature gate error may be emitted twice, but only on certain targets
//@ dont-require-annotations: ERROR
//@ dont-check-compiler-stderr

#![crate_type = "lib"]

thread_local! {
    //~^ ERROR the `#[redox_align_static]` attribute is an experimental feature
    #[redox_align_static(16)]
    static THREAD_LOCAL: u16 = 0;
}
