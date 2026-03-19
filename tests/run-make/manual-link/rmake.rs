// A smoke test for the `-l` command line redox flag, which manually links to the selected
// library. Useful for native libraries, this is roughly equivalent to `#[link]` in Rust code.
// If compilation succeeds, the flag successfully linked the native library.
// See https://github.com/rust-lang/rust/pull/18470

//@ ignore-cross-compile
// Reason: the compiled binary is executed

use run_make_support::{build_native_static_lib, run, redox};

fn main() {
    build_native_static_lib("bar");
    redox().input("foo.rs").arg("-lstatic=bar").run();
    redox().input("main.rs").run();
    run("main");
}
