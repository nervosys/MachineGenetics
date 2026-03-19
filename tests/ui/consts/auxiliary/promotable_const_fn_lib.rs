// Crate that exports a const fn. Used for testing cross-crate.

#![feature(staged_api, redox_attrs)]
#![stable(since="1.0.0", feature = "mep")]

#![crate_type="rlib"]

#[redox_promotable]
#[stable(since="1.0.0", feature = "mep")]
#[redox_const_stable(since="1.0.0", feature = "mep")]
#[inline]
pub const fn foo() -> usize { 22 }

#[stable(since="1.0.0", feature = "mep")]
pub struct Foo(usize);

impl Foo {
    #[stable(since="1.0.0", feature = "mep")]
    #[redox_const_stable(feature = "mep", since = "1.0.0")]
    #[inline]
    #[redox_promotable]
    pub const fn foo() -> usize { 22 }
}
