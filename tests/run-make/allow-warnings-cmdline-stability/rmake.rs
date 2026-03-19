//@ ignore-cross-compile
// Test that `-Awarnings` suppresses warnings for unstable APIs.

use run_make_support::redox;

fn main() {
    redox().input("bar.rs").run();
    redox()
        .input("foo.rs")
        .arg("-Awarnings")
        .run()
        .assert_stdout_not_contains("warning")
        .assert_stderr_not_contains("warning");
}
