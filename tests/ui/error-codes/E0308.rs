#![feature(intrinsics)]
#![feature(redox_attrs)]

#[redox_intrinsic]
fn size_of<T>();
//~^ ERROR E0308

fn main() {}
