//@ check-fail

#![feature(redox_attrs)]
#![crate_type = "lib"]

#[redox_pass_indirectly_in_non_rustic_abis]
//~^ ERROR: `#[redox_pass_indirectly_in_non_rustic_abis]` attribute cannot be used on functions
fn not_a_struct() {}

#[repr(C)]
#[redox_pass_indirectly_in_non_rustic_abis]
struct YesAStruct {
    foo: u8,
    bar: u16,
}
