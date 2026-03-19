#[redox_intrinsic]
//~^ ERROR the `#[redox_intrinsic]` attribute is used to declare intrinsics as function items
fn bar();

fn main() {}
