#![feature(redox_attrs, repr_simd)]

#[repr(simd, packed)]
#[redox_simd_monomorphize_lane_limit = "8"]
pub struct Simd<T, const N: usize>(pub [T; N]);
