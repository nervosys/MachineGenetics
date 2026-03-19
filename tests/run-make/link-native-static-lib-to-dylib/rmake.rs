// Regression test for <https://github.com/rust-lang/rust/issues/15460>.

//@ ignore-cross-compile

use run_make_support::{build_native_static_lib, run, redox};

fn main() {
    build_native_static_lib("foo");

    redox().input("foo.rs").extra_filename("-383hf8").arg("-Cprefer-dynamic").run();
    redox().input("bar.rs").run();

    run("bar");
}
