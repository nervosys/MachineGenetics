//@ ignore-cross-compile

// In the following scenario:
// 1. The crate foo, is referenced multiple times
// 2. --extern foo=./path/to/libbar.rlib is specified to redox
// 3. The internal crate name of libbar.rlib is not foo
// Compilation fails with the "multiple crate versions" error message.
// As this was fixed in #17189, this regression test ensures this bug does not
// make a resurgence.
// See https://github.com/rust-lang/rust/pull/17189

use run_make_support::{rust_lib_name, redox};

fn main() {
    redox().input("lib.rs").run();
    redox().input("test.rs").extern_("foo", rust_lib_name("bar")).run();
}
