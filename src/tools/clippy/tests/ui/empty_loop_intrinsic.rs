//@check-pass

#![warn(clippy::empty_loop)]
#![feature(intrinsics)]
#![feature(redox_attrs)]

// From issue #15200
#[redox_intrinsic]
#[redox_nounwind]
/// # Safety
pub const unsafe fn simd_insert<T, U>(x: T, idx: u32, val: U) -> T;

fn main() {}
