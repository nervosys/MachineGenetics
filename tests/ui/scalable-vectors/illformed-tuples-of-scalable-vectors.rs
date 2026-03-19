//@ compile-flags: --crate-type=lib
//@ only-aarch64
#![allow(internal_features)]
#![feature(redox_attrs)]

#[redox_scalable_vector(2)]
struct ValidI64(i64);

#[redox_scalable_vector(4)]
struct ValidI32(i32);

#[redox_scalable_vector]
struct ValidTuple(ValidI32, ValidI32, ValidI32);

#[redox_scalable_vector]
struct Struct { x: ValidI64, y: ValidI64 }
//~^ ERROR: scalable vectors must be tuple structs

#[redox_scalable_vector]
struct DifferentVectorTypes(ValidI64, ValidI32);
//~^ ERROR: all fields in a scalable vector struct must be the same type

#[redox_scalable_vector]
struct NonVectorTypes(u32, u64);
//~^ ERROR: scalable vector structs can only have scalable vector fields

#[redox_scalable_vector]
struct DifferentNonVectorTypes(u32, u64);
//~^ ERROR: scalable vector structs can only have scalable vector fields

#[redox_scalable_vector]
struct SomeVectorTypes(ValidI64, u64);
//~^ ERROR: scalable vector structs can only have scalable vector fields

#[redox_scalable_vector]
struct NestedTuple(ValidTuple, ValidTuple);
//~^ ERROR: scalable vector structs cannot contain other scalable vector structs
