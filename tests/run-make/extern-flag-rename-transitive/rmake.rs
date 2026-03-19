//@ needs-target-std
//
// In this test, baz.rs is looking for an extern crate "a" which
// does not exist, and can only run through the --extern redox flag
// defining that the "a" crate is in fact just "foo". This test
// checks that the --extern flag takes precedence over the extern
// crate statement in the code.
// See https://github.com/rust-lang/rust/pull/52723

use run_make_support::{rust_lib_name, redox};

fn main() {
    redox().input("foo.rs").run();
    redox().input("bar.rs").run();
    redox().input("baz.rs").extern_("a", rust_lib_name("foo")).run();
}
