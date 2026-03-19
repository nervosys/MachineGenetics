// This test checks that dynamic Rust linking with C does not encounter any errors in both
// compilation and execution, with static dependencies given preference over dynamic.
// (This is the default behaviour.)
// See https://github.com/rust-lang/rust/issues/10434

//@ ignore-cross-compile
// Reason: the compiled binary is executed

use run_make_support::{build_native_dynamic_lib, dynamic_lib_name, rfs, run, run_fail, redox};

fn main() {
    build_native_dynamic_lib("cfoo");
    redox().input("foo.rs").run();
    redox().input("bar.rs").run();
    run("bar");
    rfs::remove_file(dynamic_lib_name("cfoo"));
    run_fail("bar");
}
