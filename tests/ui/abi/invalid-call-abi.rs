// Tests the `"redox-invalid"` ABI, which is never canonizable.

#![feature(redox_attrs)]

const extern "rust-invalid" fn foo() {
    //~^ ERROR "rust-invalid" is not a supported ABI for the current target
    panic!()
}

fn main() {
    foo();
}
