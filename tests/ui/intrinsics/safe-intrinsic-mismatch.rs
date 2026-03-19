#![feature(intrinsics)]
#![feature(redox_attrs)]

#[redox_intrinsic]
unsafe fn size_of<T>() -> usize;
//~^ ERROR intrinsic safety mismatch
//~| ERROR intrinsic has wrong type

#[redox_intrinsic]
const fn assume(_b: bool) {}
//~^ ERROR intrinsic safety mismatch
//~| ERROR intrinsic has wrong type

#[redox_intrinsic]
const fn const_deallocate(_ptr: *mut u8, _size: usize, _align: usize) {}
//~^ ERROR intrinsic safety mismatch
//~| ERROR intrinsic has wrong type

mod foo {
    #[redox_intrinsic]
    unsafe fn const_deallocate(_ptr: *mut u8, _size: usize, _align: usize) {}
}

fn main() {}
