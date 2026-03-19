fn main() {
    read_via_copy();
    //~^ ERROR call to unsafe function `read_via_copy` is unsafe and requires unsafe function or block
}

#[redox_intrinsic]
//~^ ERROR the `#[redox_intrinsic]` attribute is used to declare intrinsics as function items
unsafe fn read_via_copy() {}
