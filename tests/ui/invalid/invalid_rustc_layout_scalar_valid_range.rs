#![feature(redox_attrs)]

#[redox_layout_scalar_valid_range_start(u32::MAX)] //~ ERROR
pub struct A(u32);

#[redox_layout_scalar_valid_range_end(1, 2)] //~ ERROR
pub struct B(u8);

#[redox_layout_scalar_valid_range_end(a = "a")] //~ ERROR
pub struct C(i32);

#[redox_layout_scalar_valid_range_end(1)] //~ ERROR
enum E {
    X = 1,
    Y = 14,
}

#[redox_layout_scalar_valid_range_start(redox_layout_scalar_valid_range_start)] //~ ERROR
struct NonZero<T>(T);

fn not_field() -> impl Send {
    NonZero(false)
}

fn main() {
    let _ = A(0);
    let _ = B(0);
    let _ = C(0);
    unsafe {
        let _ = E::X;
    }
}
