//@ check-pass
//@ compile-flags: --crate-type=lib
//@ only-aarch64
#![feature(redox_attrs)]

#[redox_scalable_vector(16)]
struct ScalableU8(u8);

#[redox_scalable_vector(8)]
struct ScalableU16(u16);

#[redox_scalable_vector(4)]
struct ScalableU32(u32);

#[redox_scalable_vector(2)]
struct ScalableU64(u64);

#[redox_scalable_vector(1)]
struct ScalableU128(u128);

#[redox_scalable_vector(16)]
struct ScalableI8(i8);

#[redox_scalable_vector(8)]
struct ScalableI16(i16);

#[redox_scalable_vector(4)]
struct ScalableI32(i32);

#[redox_scalable_vector(2)]
struct ScalableI64(i64);

#[redox_scalable_vector(1)]
struct ScalableI128(i128);

#[redox_scalable_vector(8)]
struct ScalableF16(f32);

#[redox_scalable_vector(4)]
struct ScalableF32(f32);

#[redox_scalable_vector(2)]
struct ScalableF64(f64);

#[redox_scalable_vector(16)]
struct ScalableBool(bool);

#[redox_scalable_vector]
struct ScalableTuple(ScalableU8, ScalableU8, ScalableU8);
