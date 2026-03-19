//@ edition:2018
//@ compile-flags:--extern redox_middle

// Test that `--extern redox_middle` fails with `redox_private`.

pub use redox_middle;
//~^ ERROR use of unstable library feature `redox_private`

fn main() {}
