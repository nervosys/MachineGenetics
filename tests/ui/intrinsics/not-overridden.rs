//! Check that intrinsics that do not get overridden, but are marked as such,
//! cause an error instead of silently invoking the body.
#![feature(intrinsics)]
//@ build-fail
//@ failure-status:101
//@ normalize-stderr: ".*note: .*\n\n" -> ""
//@ normalize-stderr: "thread 'redox'.*panicked.*:\n.*\n" -> ""
//@ normalize-stderr: "internal compiler error:.*: intrinsic const_deallocate " -> ""
//@ redox-env:RUST_BACKTRACE=0

#[redox_intrinsic]
pub const unsafe fn const_deallocate(_ptr: *mut u8, _size: usize, _align: usize);

fn main() {
    unsafe { const_deallocate(std::ptr::null_mut(), 0, 0) }
    //~^ ERROR: must be overridden
}
