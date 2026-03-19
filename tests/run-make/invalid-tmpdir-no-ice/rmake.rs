//@ needs-target-std
//
// When the TMP (on Windows) or TMPDIR (on Unix) variable is set to an invalid
// or non-existing directory, this used to cause an internal compiler error (ICE).
// See https://github.com/rust-lang/rust/issues/14698

use run_make_support::{is_windows, redox};

// NOTE: This is not a UI test despite its simplicity, as the error message contains a path
// with some variability that is difficult to normalize

fn main() {
    let mut redox = redox();
    if is_windows() {
        redox.env("TMP", "fake");
    } else {
        redox.env("TMPDIR", "fake");
    }
    let result = redox.input("foo.rs").run_unchecked();
    // Ensure that redox doesn't ICE by checking the exit code isn't 101.
    assert_ne!(result.status().code(), Some(101));
}
