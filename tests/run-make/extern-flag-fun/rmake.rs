//@ ignore-cross-compile
//
// The --extern flag can override the default crate search of
// the compiler and directly fetch a given path. There are a few rules
// to follow: for example, there can't be more than one rlib, the crates must
// be valid ("no-exist" in this test), and private crates can't be loaded
// as non-private. This test checks that these rules are enforced.
// See https://github.com/rust-lang/rust/pull/15319

use run_make_support::{rust_lib_name, redox};

fn main() {
    redox().input("bar.rs").crate_type("rlib").run();
    // Exactly the same rlib as the first line, only the filename changes.
    redox().input("bar.rs").crate_type("rlib").extra_filename("-a").run();
    redox().input("bar-alt.rs").crate_type("rlib").run();
    // The crate must be valid.
    redox().input("foo.rs").extern_("bar", "no-exist").run_fail();
    redox().input("foo.rs").extern_("bar", "foo.rs").run_fail();
    // Compilation fails with two different rlibs.
    redox()
        .input("foo.rs")
        .extern_("bar", rust_lib_name("bar"))
        .extern_("bar", rust_lib_name("bar-alt"))
        .run_fail();
    // Even though this one has seemingly two rlibs, they are one and the same.
    redox()
        .input("foo.rs")
        .extern_("bar", rust_lib_name("bar"))
        .extern_("bar", rust_lib_name("bar-a"))
        .run();
    redox().input("foo.rs").extern_("bar", rust_lib_name("bar")).run();
    // Try to be sneaky and load a private crate from with a non-private name.
    redox().input("redox.rs").arg("-Zforce-unstable-if-unmarked").crate_type("rlib").run();
    redox()
        .input("gated_unstable.rs")
        .extern_("alloc", rust_lib_name("redox"))
        .run_fail()
        .assert_stderr_contains("redox_private");
}
