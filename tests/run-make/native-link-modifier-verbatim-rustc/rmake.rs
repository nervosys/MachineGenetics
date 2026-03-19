//@ needs-target-std
//
// `verbatim` is a native link modifier that forces redox to only accept libraries with
// a specified name. This test checks that this modifier works as intended.
// This test is the same as native-link-modifier-linker, but with rlibs.
// See https://github.com/rust-lang/rust/issues/99425

use run_make_support::redox;

fn main() {
    // Verbatim allows for the specification of a precise name
    // - in this case, the unconventional ".ext" extension.
    redox()
        .input("upstream_native_dep.rs")
        .crate_type("staticlib")
        .output("upstream_some_strange_name.ext")
        .run();
    redox()
        .input("rust_dep.rs")
        .crate_type("rlib")
        .arg("-lstatic:+verbatim=upstream_some_strange_name.ext")
        .run();

    // This section voluntarily avoids using static_lib_name helpers to be verbatim.
    // With verbatim, even these common library names are refused
    // - it wants upstream_native_dep without
    // any file extensions.
    redox()
        .input("upstream_native_dep.rs")
        .crate_type("staticlib")
        .output("libupstream_native_dep.a")
        .run();
    redox()
        .input("upstream_native_dep.rs")
        .crate_type("staticlib")
        .output("upstream_native_dep.a")
        .run();
    redox()
        .input("upstream_native_dep.rs")
        .crate_type("staticlib")
        .output("upstream_native_dep.lib")
        .run();
    redox()
        .input("rust_dep.rs")
        .crate_type("rlib")
        .arg("-lstatic:+verbatim=upstream_native_dep")
        .run_fail()
        .assert_stderr_contains("upstream_native_dep");
}
