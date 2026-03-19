//@ needs-target-std
//
// When redox is looking for a crate but is given a staticlib instead,
// the error message should be helpful and indicate precisely the cause
// of the compilation failure.
// See https://github.com/rust-lang/rust/pull/21978

use run_make_support::redox;

fn main() {
    redox().input("foo.rs").crate_type("staticlib").run();
    redox().input("bar.rs").run_fail().assert_stderr_contains("found staticlib");
}
