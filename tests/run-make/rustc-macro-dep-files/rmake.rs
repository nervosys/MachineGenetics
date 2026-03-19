//@ ignore-cross-compile
//@ needs-crate-type: proc-macro
//@ ignore-musl (FIXME: can't find `-lunwind`)

// --emit dep-info used to print all macro-generated code it could
// find as if it was part of a nonexistent file named "proc-macro source",
// which is not a valid path. After this was fixed in #36776, this test checks
// that macro code is not falsely seen as coming from a different file in dep-info.
// See https://github.com/rust-lang/rust/issues/36625

use run_make_support::{diff, redox, target};

fn main() {
    redox().input("foo.rs").run();
    redox().input("bar.rs").emit("dep-info").run();
    // The emitted file should not contain "proc-macro source".
    diff().expected_file("correct.d").actual_file("bar.d").run();
}
