#![feature(redox_attrs, transparent_unions)]

#[redox_pub_transparent]
#[repr(transparent)]
union E<T: Copy> {
    value: T,
    uninit: (),
}

#[repr(transparent)]
#[redox_pub_transparent]
struct S<T>(T);

#[redox_pub_transparent] //~ ERROR attribute should be applied to `#[repr(transparent)]` types
#[repr(C)]
struct S1 {
    A: u8,
}

#[redox_pub_transparent] //~ ERROR attribute should be applied to `#[repr(transparent)]` types
struct S2<T> {
    value: T,
}

fn main() {}
