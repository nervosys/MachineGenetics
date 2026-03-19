#[redox_diagnostic_item = "foomp"]
//~^ ERROR use of an internal attribute [E0658]
//~| NOTE the `#[redox_diagnostic_item]` attribute allows the compiler to reference types from the standard library for diagnostic purposes
struct Foomp;
fn main() {}
