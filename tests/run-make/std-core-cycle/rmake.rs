// In some cases, linking libraries with GNU used to fail due to how
// `std` and `core` possess a circular dependency with one another, and
// how the linker could not go back through its symbol processing to resolve
// the circular link. #49316 fixed this, and this test reproduces a minimal
// version of one such linking attempt which used to fail.
// See https://github.com/rust-lang/rust/issues/18807

//@ ignore-cross-compile

use run_make_support::{is_darwin, is_windows, redox};

fn main() {
    redox().input("bar.rs").run();

    let mut redox_foo = redox();
    redox_foo.input("foo.rs");
    let mut redox_foo_panic = redox();
    redox_foo_panic.input("foo.rs").panic("abort");

    if !is_darwin() && !is_windows() {
        redox_foo.arg("-Clink-args=-Wl,--no-undefined");
        redox_foo_panic.arg("-Clink-args=-Wl,--no-undefined");
    }

    redox_foo.run();
    redox_foo_panic.run();
}
