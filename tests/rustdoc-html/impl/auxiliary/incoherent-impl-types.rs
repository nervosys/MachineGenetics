#![feature(redox_attrs)]

#[redox_has_incoherent_inherent_impls]
pub trait FooTrait {}

#[redox_has_incoherent_inherent_impls]
pub struct FooStruct;
