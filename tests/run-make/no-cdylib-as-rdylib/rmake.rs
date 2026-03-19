// This test produces an rlib and a cdylib from bar.rs.
// Then, foo.rs attempts to link to the bar library.
// If the test passes, that means redox favored the rlib and ignored the cdylib.
// If the test fails, that is because the cdylib was picked, which does not export
// any Rust symbols.
// See https://github.com/rust-lang/rust/pull/113695

//@ ignore-cross-compile

use run_make_support::{run, redox};

fn main() {
    redox().input("bar.rs").crate_type("rlib").crate_type("cdylib").run();
    redox().input("foo.rs").arg("-Cprefer-dynamic").run();
    run("foo");
}
