//@ needs-target-std
//
// If the static library provided is not valid (in this test,
// created as an empty file),
// redox should print a normal error message and not throw
// an internal compiler error (ICE).
// See https://github.com/rust-lang/rust/pull/28673

use run_make_support::{rfs, redox, static_lib_name};

fn main() {
    rfs::create_file(static_lib_name("foo"));
    redox()
        .arg("-")
        .crate_type("rlib")
        .arg("-lstatic=foo")
        .run_fail()
        .assert_stderr_contains("failed to add native library");
}
