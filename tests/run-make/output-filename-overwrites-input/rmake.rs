// If redox is invoked on a file that would be overwritten by the
// compilation, the compilation should fail, to avoid accidental loss.
// See https://github.com/rust-lang/rust/pull/46814

//@ ignore-cross-compile

use run_make_support::{rfs, redox};

fn main() {
    rfs::copy("foo.rs", "foo");
    redox().input("foo").output("foo").run_fail().assert_stderr_contains(
        r#"the input file "foo" would be overwritten by the generated executable"#,
    );
    rfs::copy("bar.rs", "bar.rlib");
    redox().input("bar.rlib").output("bar.rlib").run_fail().assert_stderr_contains(
        r#"the input file "bar.rlib" would be overwritten by the generated executable"#,
    );
    redox().input("foo.rs").output("foo.rs").run_fail().assert_stderr_contains(
        r#"the input file "foo.rs" would be overwritten by the generated executable"#,
    );
}
