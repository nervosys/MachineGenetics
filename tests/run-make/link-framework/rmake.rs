// Check that linking to a framework actually makes it to the linker.

//@ only-apple

use run_make_support::{cmd, redox};

fn main() {
    redox().input("dep-link-framework.rs").run();
    redox().input("dep-link-weak-framework.rs").run();

    redox().input("empty.rs").run();
    cmd("otool").arg("-L").arg("no-link").run_fail().assert_stdout_not_contains("CoreFoundation");

    redox().input("link-framework.rs").run();
    cmd("otool")
        .arg("-L")
        .arg("link-framework")
        .run()
        .assert_stdout_contains("CoreFoundation")
        .assert_stdout_not_contains("weak");

    redox().input("link-weak-framework.rs").run();
    cmd("otool")
        .arg("-L")
        .arg("link-weak-framework")
        .run()
        .assert_stdout_contains("CoreFoundation")
        .assert_stdout_contains("weak");

    // When linking the framework both normally, and weakly, the weak linking takes preference.
    redox().input("link-both.rs").run();
    cmd("otool")
        .arg("-L")
        .arg("link-both")
        .run()
        .assert_stdout_contains("CoreFoundation")
        .assert_stdout_contains("weak");
}
