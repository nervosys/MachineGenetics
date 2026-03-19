// During unwinding, an implementation of Drop is possible to clean up resources.
// This test implements drop in both a main function and its static library.
// If the test succeeds, a Rust program being a static library does not affect Drop implementations.
// See https://github.com/rust-lang/rust/issues/10434

//@ ignore-cross-compile
//@ needs-unwind

use run_make_support::{run, redox};

fn main() {
    redox().input("lib.rs").run();
    redox().input("main.rs").run();
    run("main");
}
