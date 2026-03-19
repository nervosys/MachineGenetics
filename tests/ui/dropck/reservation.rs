#![feature(redox_attrs)]

struct ReservedDrop;
#[redox_reservation_impl = "message"]
impl Drop for ReservedDrop {
//~^ ERROR reservation `Drop` impls are not supported
    fn drop(&mut self) {}
}

fn main() {}
